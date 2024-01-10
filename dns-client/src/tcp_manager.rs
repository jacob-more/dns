// Note about TCP wire format: the TCP wire format prepends 2 bytes to the
// start of the packet. These two bytes are a u16 representing the remaining
// number of bytes in the packet (the message's wire format length) excluding
// those first two length bytes.

use std::{sync::Arc, collections::HashMap, net::{SocketAddr, IpAddr}};

use async_recursion::async_recursion;
use dns_lib::{query::message::Message, serde::wire::{write_wire::WriteWire, compression_map::CompressionMap, to_wire::ToWire, read_wire::ReadWire, from_wire::FromWire}};
use tokio::{net::{tcp::{OwnedWriteHalf, OwnedReadHalf}, TcpStream}, sync::{Mutex, RwLock, broadcast::{Sender, Receiver, self}, Notify}, io::{self, AsyncWriteExt, AsyncReadExt}, task::JoinHandle};

use crate::{IPV4_ENABLED, IPV6_ENABLED};

const MAX_MESSAGE_SIZE: usize = 4096;

#[derive(Debug)]
enum TCPConnection {
    Stream(Arc<Mutex<OwnedWriteHalf>>, RwLock<HashMap<u16, Sender<Message>>>, JoinHandle<()>),
    Establishing(Arc<Notify>),
}

#[derive(Debug)]
pub struct TCPManager {
    tcp_connections: RwLock<HashMap<SocketAddr, TCPConnection>>,
}

impl TCPManager {
    #[inline]
    pub fn new() -> Self {
        Self { tcp_connections: RwLock::new(HashMap::new()) }
    }

    #[async_recursion]
    async fn get_connection(manager: Arc<Self>, upstream_socket: SocketAddr) -> io::Result<Arc<Mutex<OwnedWriteHalf>>> {
        let notifier;

        let read_locked_connections = manager.tcp_connections.read().await;
        if let Some(tcp_connection) = read_locked_connections.get(&upstream_socket) {
            match tcp_connection {
                TCPConnection::Stream(write_tcp_stream, _, _) => {
                    println!("Already connected to '{upstream_socket}'");
                    let answer = write_tcp_stream.clone();
                    drop(read_locked_connections);
                    return Ok(answer)
                },
                TCPConnection::Establishing(notifier) => {
                    println!("Waiting for connection to '{upstream_socket}' to be established");
                    let notifier = notifier.clone();
                    drop(read_locked_connections);
                    notifier.notified().await;
                    return Self::get_connection(manager, upstream_socket).await;
                },
            }
        }

        drop(read_locked_connections);
        // Since we switched to a write lock, we should verify that nothing was inserted while we
        // were waiting.
        let mut write_locked_connections = manager.tcp_connections.write().await;
        if let Some(tcp_connection) = write_locked_connections.get(&upstream_socket) {
            match tcp_connection {
                TCPConnection::Stream(write_tcp_stream, _, _) => {
                    println!("Already connected to '{upstream_socket}'");
                    let answer = write_tcp_stream.clone();
                    drop(write_locked_connections);
                    return Ok(answer)
                },
                TCPConnection::Establishing(notifier) => {
                    println!("Waiting for connection to '{upstream_socket}' to be established");
                    let notifier = notifier.clone();
                    drop(write_locked_connections);
                    notifier.notified().await;
                    return Self::get_connection(manager, upstream_socket).await;
                },
            }
        }

        // Since we hold an exclusive write lock, we are the only one who can insert the
        // Establishing status. Therefore, we don't need to keep checking for a race condition since
        // once we drop the lock, all other processes will block at `notifier.notified().await`.
        println!("Setting up connection to '{upstream_socket}'");
        notifier = Arc::new(Notify::new());
        write_locked_connections.insert(upstream_socket, TCPConnection::Establishing(notifier.clone()));
        drop(write_locked_connections);

        let new_connection = match TcpStream::connect(upstream_socket).await {
            Ok(new_connection) => new_connection,
            Err(error) => {
                println!("Failed to establish TCP connection to '{upstream_socket}'");
                // Before returning, we must ensure that the "Establishing" status gets cleared
                // since we failed to establish the connection.
                let mut write_locked_connections = manager.tcp_connections.write().await;
                write_locked_connections.remove(&upstream_socket);
                drop(write_locked_connections);
                // Notify all of the waiters. This sort of creates a race condition where the first
                // one to the write lock gets to set the status back to "Establishing" and the rest
                // all have to wait on that processes notifier.
                notifier.notify_waiters();
                return Err(error);
            },
        };
        let (read_tcp, write_tcp) = new_connection.into_split();
        let write_tcp = Arc::new(Mutex::new(write_tcp));
        println!("Connected to '{upstream_socket}'");


        let mut write_locked_connections = manager.tcp_connections.write().await;
        // Start the monitor WHILE the write lock is held. Otherwise, if the monitor immediately
        // failed to read, it could try to clean up the connection before it is inserted. Then, the
        // connection would be inserted into the map but the monitor would already have exited and
        // there would not be a process monitoring the stream.
        let stream_monitor_handle = Self::start_tcp_stream_monitor(manager.clone(), upstream_socket, read_tcp);

        write_locked_connections.insert(
            upstream_socket,
            TCPConnection::Stream(
                write_tcp.clone(),
                RwLock::new(HashMap::new()),
                stream_monitor_handle
            )
        );
        drop(write_locked_connections);

        notifier.notify_waiters();
        return Ok(write_tcp);
    }

    #[inline]
    fn start_tcp_stream_monitor(manager: Arc<Self>, upstream_socket: SocketAddr, tcp_stream: OwnedReadHalf) -> JoinHandle<()> {
        println!("Starting task to manage '{upstream_socket}'");
        tokio::spawn(Self::monitor_tcp_stream(manager, upstream_socket, tcp_stream))
    }

    #[inline]
    async fn cleanup_responder(&self, socket: SocketAddr, id: u16) {
        let locked_tcp_manager = self.tcp_connections.read().await;
        if let Some(TCPConnection::Stream(_, response_manager, _)) = locked_tcp_manager.get(&socket) {
            let mut locked_response_manager = response_manager.write().await;
            locked_response_manager.remove(&id);
            drop(locked_response_manager);
        }
        drop(locked_tcp_manager);
    }

    #[inline]
    async fn monitor_tcp_stream(manager: Arc<Self>, upstream_address: SocketAddr, mut tcp_stream: OwnedReadHalf) {
        loop {
            match read_tcp_message(&mut tcp_stream).await {
                Ok(response) => {
                    let read_locked_connections = manager.tcp_connections.read().await;
                    if let Some(TCPConnection::Stream(_, response_manager, _)) = read_locked_connections.get(&upstream_address) {
                        let read_locked_response_manager = response_manager.read().await;
                        let response_id = response.id;
                        match read_locked_response_manager.get(&response_id) {
                            Some(sender) => {
                                match sender.send(response) {
                                    Ok(_) => drop(read_locked_response_manager),
                                    Err(error) => {
                                        drop(read_locked_response_manager);
                                        // Normally, the receiving processes is supposed to clean up. However,
                                        // if the receiving process is missing or otherwise unable to receive,
                                        // it is safest to make sure that it gets cleaned up here.
                                        manager.cleanup_responder(upstream_address, response_id).await;
                                        println!("TCP Monitor for {upstream_address} unable to send message {response_id} to waiting processes: {error}");
                                    },
                                }
                            },
                            None => {
                                drop(read_locked_response_manager);
                                println!("No processes are waiting for message {response_id}");
                            },
                        };
                    };
                },
                Err(error) => match error.kind() {
                    io::ErrorKind::ConnectionRefused => {println!("TCP Monitor for {upstream_address} unable to read from stream (fatal). Connection Refused: {error}"); break;},
                    io::ErrorKind::ConnectionReset => {println!("TCP Monitor for {upstream_address} unable to read from stream (fatal). Connection Reset: {error}"); break;},
                    io::ErrorKind::ConnectionAborted => {println!("TCP Monitor for {upstream_address} unable to read from stream (fatal). Connection Aborted: {error}"); break;},
                    io::ErrorKind::NotConnected => {println!("TCP Monitor for {upstream_address} unable to read from stream (fatal). Not Connected: {error}"); break;},
                    io::ErrorKind::AddrInUse => {println!("TCP Monitor for {upstream_address} unable to read from stream (fatal). Address In Use: {error}"); break;},
                    io::ErrorKind::AddrNotAvailable => {println!("TCP Monitor for {upstream_address} unable to read from stream (fatal). Address Not Available: {error}"); break;},
                    io::ErrorKind::TimedOut => {println!("TCP Monitor for {upstream_address} unable to read from stream (fatal). Timed Out: {error}"); break;},
                    io::ErrorKind::Unsupported => {println!("TCP Monitor for {upstream_address} unable to read from stream (fatal). Unsupported: {error}"); break;},
                    io::ErrorKind::BrokenPipe => {println!("TCP Monitor for {upstream_address} unable to read from stream (fatal). Broken Pipe: {error}"); break;},
                    io::ErrorKind::UnexpectedEof => (),   //< This error usually occurs a bunch of times and fills up the logs. Don't want to print it.
                    _ => println!("TCP Monitor for {upstream_address} unable to read from stream (non-fatal). {error}"),
                },
            }
        }

        let mut locked_tcp_connections = manager.tcp_connections.write().await;
        locked_tcp_connections.remove(&upstream_address);
        drop(locked_tcp_connections);
    }

    #[async_recursion]
    async fn get_responder(&self, socket: SocketAddr, id: u16) -> Option<Receiver<Message>> {
        let read_locked_tcp_manager = self.tcp_connections.read().await;
        match read_locked_tcp_manager.get(&socket) {
            Some(TCPConnection::Stream(_, response_manager, _)) => {
                let locked_response_manager = response_manager.read().await;
                // Work with the read lock first.
                if let Some(responder) = locked_response_manager.get(&id) {
                    let responder = responder.subscribe();
                    drop(locked_response_manager);
                    return Some(responder);
                }
                drop(locked_response_manager);

                let mut locked_response_manager = response_manager.write().await;
                // Work with the write lock if the read lock found nothing.
                // Since we dropped the lock, we need to check if a responder was added while the
                // lock was dropped.
                let responder = match locked_response_manager.get(&id) {
                    Some(responder) => responder.subscribe(),
                    None => {
                        let (sender, responder) = broadcast::channel(1);
                        locked_response_manager.insert(id, sender);
                        responder
                    },
                };
                drop(locked_response_manager);
                drop(read_locked_tcp_manager);
                return Some(responder)
            },
            Some(TCPConnection::Establishing(notifier)) => {
                notifier.notified().await;
                drop(read_locked_tcp_manager);
                return self.get_responder(socket, id).await;
            },
            None => {
                drop(read_locked_tcp_manager);
                return None;
            },
        }
    }

    pub async fn query_tcp(manager: Arc<Self>, upstream_socket: SocketAddr, question: &Message) -> io::Result<Message> {
        match upstream_socket.ip() {
            IpAddr::V4(_) if !IPV4_ENABLED => return Err(io::Error::from(io::ErrorKind::Unsupported)),
            IpAddr::V6(_) if !IPV6_ENABLED => return Err(io::Error::from(io::ErrorKind::Unsupported)),
            _ => (),
        };

        let message_id = question.id;
        println!("Connecting to {upstream_socket} via TCP...");
        let tcp_stream = TCPManager::get_connection(manager.clone(), upstream_socket).await?;

        let mut responder = match manager.get_responder(upstream_socket, message_id).await {
            Some(responder) => responder,
            None => return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                format!("Failed to get the responder for socket {upstream_socket} to send message {message_id}. A connection must be established in order to get a responder.")
            )),
        };

        // IMPORTANT: All `return ...` after this point must clean up the `response_manager` first.

        println!("Querying DNS Server...\nSending: {question:#?}");
        if let Err(error) = write_tcp_message(&tcp_stream, question).await {
            manager.cleanup_responder(upstream_socket, message_id).await;
            return Err(io::Error::new(error.kind(), error));
        }

        let response = match responder.recv().await {
            Ok(response) => response,
            Err(error) => {
                manager.cleanup_responder(upstream_socket, message_id).await;
                return Err(io::Error::new(io::ErrorKind::ConnectionAborted, error));
            },
        };
        println!("Response: {response:#?}\n");

        manager.cleanup_responder(upstream_socket, message_id).await;
        return Ok(response);
    }
}

#[inline]
pub async fn write_tcp_message(tcp_stream: &Arc<Mutex<OwnedWriteHalf>>, message: &Message) -> io::Result<()> {
    // Step 1: Serialize Data
    let raw_message = &mut [0_u8; MAX_MESSAGE_SIZE];
    let mut raw_message = WriteWire::from_bytes(raw_message);
    // Push two bytes onto the wire. These will be replaced with the u16 that indicates
    // the wire length.
    if let Err(error) = raw_message.write_bytes(&[0, 0]) {
        return Err(io::Error::new(io::ErrorKind::InvalidData, error));
    };

    if let Err(wire_error) = message.to_wire_format(&mut raw_message, &mut Some(CompressionMap::new())) {
        return Err(io::Error::new(io::ErrorKind::InvalidData, wire_error));
    };

    // Now, replace those two bytes from earlier with the wire length.
    let wire_length = raw_message.len();
    let message_wire_length = (wire_length - 2) as u16;
    let bytes = message_wire_length.to_be_bytes();
    if let Err(error) = raw_message.write_bytes_at(&bytes, 0) {
        return Err(io::Error::new(io::ErrorKind::InvalidData, error));
    };

    // Step 2: Bounds check against the configurations.
    //  TODO: No configuration options have been defined yet.

    // Step 3: Send the message via TCP.
    let mut locked_tcp_stream = tcp_stream.lock().await;
    let bytes_written = locked_tcp_stream.write(raw_message.current_state()).await?;
    drop(locked_tcp_stream);
    // Verify that the correct number of bytes were written.
    if bytes_written != wire_length {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Incorrect number of bytes sent to TCO stream; expected {wire_length} bytes but sent {bytes_written} bytes"),
        ));
    }

    return Ok(());
}

#[inline]
pub async fn read_tcp_message(tcp_stream: &mut OwnedReadHalf) -> io::Result<Message> {
    // Step 1: Deserialize the u16 representing the size of the rest of the data. This is the first
    //         2 bytes of data.
    let mut wire_size = [0, 0];
    let bytes_read = tcp_stream.read_exact(&mut wire_size).await?;
    if bytes_read != 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Expected 2 bytes but got {bytes_read}")
        ));
    }
    let expected_message_size = u16::from_be_bytes(wire_size) as usize;
    if expected_message_size > MAX_MESSAGE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("The length byte cannot exceed {MAX_MESSAGE_SIZE}; length was {expected_message_size}"),
        ));
    }

    // Step 2: Read the rest of the packet.
    // Note: It MUST be the size of the previous u16 (expected_message_size).
    let mut tcp_buffer = [0; MAX_MESSAGE_SIZE];
    let tcp_buffer = &mut tcp_buffer[..MAX_MESSAGE_SIZE];
    let bytes_read = tcp_stream.read_exact(&mut tcp_buffer[..expected_message_size]).await?;
    if bytes_read != expected_message_size {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Expected {expected_message_size} bytes but got {bytes_read}")
        ));
    }

    // Step 3: Deserialize the Message from the buffer.
    let mut wire = ReadWire::from_bytes(&mut tcp_buffer[..expected_message_size]);
    let message = match Message::from_wire_format(&mut wire) {
        Ok(message) => message,
        Err(wire_error) => return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            wire_error,
        )),
    };

    return Ok(message);
}

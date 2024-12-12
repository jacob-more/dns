use std::{collections::HashSet, io::ErrorKind, net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6}, sync::{atomic::{AtomicBool, Ordering}, Arc}};

use async_lib::awake_token::AwakeToken;
use dns_lib::{query::message::Message, serde::wire::{from_wire::FromWire, read_wire::ReadWire, to_wire::ToWire, write_wire::WriteWire}, types::c_domain_name::CompressionMap};
use quinn::{ConnectError, Connection, ConnectionError, Endpoint, ReadExactError, RecvStream, VarInt};
use tokio::{io, pin, select, sync::{broadcast, RwLock, RwLockReadGuard}};


const MAX_MESSAGE_SIZE: usize = 4096;

const LOCAL_V4_SOCKET: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 0));
const LOCAL_V6_SOCKET: SocketAddr = SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0), 0, 0, 0));


enum QuicState {
    Connected(Connection, AwakeToken),
    Establishing(broadcast::Sender<(Connection, AwakeToken)>),
    None,
    Blocked,
}

/// The shared mutable state for the QUIC socket. This struct is stored behind a lock.
struct SharedQuic { state: QuicState }

pub struct QuicSocket {
    quic_shared: RwLock<SharedQuic>,

    upstream_socket: SocketAddr,
    server_name: String,
    in_flight: RwLock<HashSet<u16>>,

    // Counters used to determine when the socket should be closed.
    recent_messages_sent: AtomicBool,
    recent_messages_received: AtomicBool,
}

impl QuicSocket {
    #[inline]
    pub fn new(upstream_socket: SocketAddr, server_name: String) -> Arc<Self> {
        Arc::new(Self {
            quic_shared: RwLock::new(SharedQuic { state: QuicState::None }),

            upstream_socket,
            server_name,
            in_flight: RwLock::new(HashSet::new()),

            recent_messages_sent: AtomicBool::new(false),
            recent_messages_received: AtomicBool::new(false),
        })
    }

    #[inline]
    pub fn recent_messages_sent_or_received(&self) -> bool {
        self.recent_messages_sent.load(Ordering::SeqCst)
        || self.recent_messages_received.load(Ordering::SeqCst)
    }

    #[inline]
    pub fn recent_messages_sent_and_received(&self) -> (bool, bool) {
        (
            self.recent_messages_sent.load(Ordering::SeqCst),
            self.recent_messages_received.load(Ordering::SeqCst)
        )
    }

    #[inline]
    pub fn recent_messages_sent(&self) -> bool {
        self.recent_messages_sent.load(Ordering::SeqCst)
    }

    #[inline]
    pub fn recent_messages_received(&self) -> bool {
        self.recent_messages_received.load(Ordering::SeqCst)
    }

    #[inline]
    pub fn reset_recent_messages_sent_and_received(&self) -> (bool, bool) {
        (
            self.recent_messages_sent.swap(false, Ordering::SeqCst),
            self.recent_messages_received.swap(false, Ordering::SeqCst)
        )
    }

    #[inline]
    pub fn reset_recent_messages_sent(&self) -> bool {
        self.recent_messages_sent.swap(false, Ordering::SeqCst)
    }

    #[inline]
    pub fn reset_recent_messages_received(&self) -> bool {
        self.recent_messages_received.swap(false, Ordering::SeqCst)
    }

    #[inline]
    pub async fn start_quic(self: Arc<Self>) -> io::Result<()> {
        match self.init_quic().await {
            Ok(_) => Ok(()),
            Err(error) => Err(error),
        }
    }

    #[inline]
    pub async fn shutdown_quic(self: Arc<Self>) -> io::Result<()> {
        let r_quic = self.quic_shared.read().await;
        if let QuicState::Connected(quic_connection, quic_kill) = &r_quic.state {
            let quic_connection = quic_connection.clone();
            let quic_kill = quic_kill.clone();
            drop(r_quic);

            println!("Shutting down QUIC connection {}", self.upstream_socket);
            // TODO: provide a better reason than default and an empty reason
            quic_connection.close(VarInt::default(), &[]);

            quic_kill.awake();

            // Note: this task is not responsible for actual cleanup.
        }
        Ok(())
    }

    #[inline]
    pub async fn disable_quic(self: Arc<Self>) -> io::Result<()> {
        println!("Disabling QUIC connection {}", self.upstream_socket);

        let mut w_quic = self.quic_shared.write().await;
        match &w_quic.state {
            QuicState::Connected(quic_connection, quic_kill) => {
                // Since we are removing the reference the quic_kill by setting state to Blocked, we
                // need to kill them now since the listener won't be able to kill them.
                let quic_kill = quic_kill.clone();
                let quic_connection = quic_connection.clone();
                w_quic.state = QuicState::Blocked;
                drop(w_quic);

                println!("Shutting down QUIC connection {}", self.upstream_socket);
                // TODO: provide a better reason than default and an empty reason
                quic_connection.close(VarInt::default(), &[]);

                quic_kill.awake();

                Ok(())
            },
            QuicState::Establishing(_) => todo!("Recursively call self once connection is setup"),
            QuicState::None => {
                w_quic.state = QuicState::Blocked;
                drop(w_quic);
                Ok(())
            },
            QuicState::Blocked => { //< Already disabled
                drop(w_quic);
                Ok(())
            },
        }
    }

    #[inline]
    pub async fn enable_quic(self: Arc<Self>) -> io::Result<()> {
        println!("Enabling QUIC connection {}", self.upstream_socket);

        let mut w_quic = self.quic_shared.write().await;
        match &w_quic.state {
            QuicState::Connected(_, _) => (), //< Already enabled
            QuicState::Establishing(_) => (), //< Already enabled
            QuicState::None => (),            //< Already enabled
            QuicState::Blocked => w_quic.state = QuicState::None,
        }
        drop(w_quic);
        return Ok(());
    }

    #[inline]
    async fn init_quic(self: Arc<Self>) -> io::Result<(Connection, AwakeToken)> {
        // Initially, verify if the connection has already been established.
        let r_quic = self.quic_shared.read().await;
        match &r_quic.state {
            QuicState::Connected(quic_connection, quic_kill) => return Ok((quic_connection.clone(), quic_kill.clone())),
            QuicState::Establishing(sender) => {
                let mut receiver = sender.subscribe();
                drop(r_quic);
                match receiver.recv().await {
                    Ok((quic_connection, quic_kill)) => return Ok((quic_connection.clone(), quic_kill.clone())),
                    Err(_) => {
                        eprintln!("Failed to establish QUIC connection to {}", self.upstream_socket);
                        return Err(io::Error::from(io::ErrorKind::Interrupted));
                    },
                }
            },
            QuicState::None => (),
            QuicState::Blocked => {
                drop(r_quic);
                return Err(io::Error::from(io::ErrorKind::ConnectionAborted));
            },
        }
        drop(r_quic);

        // Setup for once the write lock is obtained.
        let (quic_connection_sender, _) = broadcast::channel(1);

        // Need to re-verify state with new lock. State could have changed in between.
        let mut w_quic = self.quic_shared.write().await;
        match &w_quic.state {
            QuicState::Connected(quic_connection, quic_kill) => return Ok((quic_connection.clone(), quic_kill.clone())),
            QuicState::Establishing(sender) => {
                let mut receiver = sender.subscribe();
                drop(w_quic);
                match receiver.recv().await {
                    Ok((quic_connection, quic_kill)) => return Ok((quic_connection.clone(), quic_kill.clone())),
                    Err(_) => {
                        eprintln!("Failed to establish QUIC connection to {}", self.upstream_socket);
                        return Err(io::Error::from(io::ErrorKind::Interrupted));
                    },
                }
            },
            QuicState::None => (),
            QuicState::Blocked => {
                drop(w_quic);
                return Err(io::Error::from(io::ErrorKind::ConnectionAborted));
            },
        }

        w_quic.state = QuicState::Establishing(quic_connection_sender.clone());
        drop(w_quic);
        println!("Initializing QUIC connection to {}", self.upstream_socket);

        // Since state has been set to Establishing, this process is now fully
        // in charge of establishing the QUIC connection. Next time the write
        // lock is obtained, it won't need to check the state.

        let local_socket = match self.upstream_socket.ip() {
            IpAddr::V4(_) => LOCAL_V4_SOCKET,
            IpAddr::V6(_) => LOCAL_V6_SOCKET,
        };

        let quic_endpoint = match Endpoint::client(local_socket) {
            Ok(quic_endpoint) => quic_endpoint,
            Err(error) => {
                eprintln!("Failed to establish QUIC connection to {}", self.upstream_socket);

                // Before returning, we must ensure that the "Establishing" status gets cleared
                // since we failed to establish the connection.
                let mut w_quic = self.quic_shared.write().await;
                w_quic.state = QuicState::None;
                drop(w_quic);

                // Notify all of the waiters by dropping the sender. This
                // causes the receivers to receiver an error.

                // It might be worth adding another state that blocks future QUIC connections.
                drop(quic_connection_sender);
                return Err(error);
            },
        };

        let quic_connecting = match quic_endpoint.connect(self.upstream_socket, &self.server_name) {
            Ok(quic_connecting) => quic_connecting,
            Err(error) => {
                eprintln!("Failed to establish QUIC connection to {}", self.upstream_socket);

                // Before returning, we must ensure that the "Establishing" status gets cleared
                // since we failed to establish the connection.
                let mut w_quic = self.quic_shared.write().await;
                w_quic.state = QuicState::None;
                drop(w_quic);

                // Notify all of the waiters by dropping the sender. This
                // causes the receivers to receiver an error.

                // It might be worth adding another state that blocks future QUIC connections.
                drop(quic_connection_sender);
                match error {
                    ConnectError::UnsupportedVersion => return Err(io::Error::new(io::ErrorKind::Unsupported, error)),
                    error => return Err(io::Error::new(io::ErrorKind::Other, error)),
                }
            },
        };

        let quic_connection = match quic_connecting.await {
            Ok(quic_connection) => quic_connection,
            Err(error) => {
                eprintln!("Failed to establish QUIC connection to {}", self.upstream_socket);

                // Before returning, we must ensure that the "Establishing" status gets cleared
                // since we failed to establish the connection.
                let mut w_quic = self.quic_shared.write().await;
                w_quic.state = QuicState::None;
                drop(w_quic);

                // Notify all of the waiters by dropping the sender. This
                // causes the receivers to receiver an error.

                // It might be worth adding another state that blocks future QUIC connections.
                drop(quic_connection_sender);
                match error {
                    ConnectionError::VersionMismatch => return Err(io::Error::new(io::ErrorKind::Unsupported, error)),
                    ConnectionError::ConnectionClosed(_) | ConnectionError::ApplicationClosed(_) => return Err(io::Error::new(io::ErrorKind::ConnectionAborted, error)),
                    ConnectionError::Reset => return Err(io::Error::new(io::ErrorKind::ConnectionReset, error)),
                    ConnectionError::TimedOut => return Err(io::Error::new(io::ErrorKind::TimedOut, error)),
                    error => return Err(io::Error::new(io::ErrorKind::Other, error)),
                }
            },
        };

        let quic_kill = AwakeToken::new();
        let mut w_quic = self.quic_shared.write().await;
        w_quic.state = QuicState::Connected(quic_connection.clone(), quic_kill.clone());
        drop(w_quic);

        let _ = quic_connection_sender.send((quic_connection.clone(), quic_kill.clone()));

        return Ok((quic_connection, quic_kill));
    }

    #[inline]
    async fn cleanup_query(self: Arc<Self>, query_id: u16) {
        let mut w_in_flight = self.in_flight.write().await;
        w_in_flight.remove(&query_id);
        drop(w_in_flight);
    }

    #[inline]
    async fn query_quic_rsocket<'a>(self: Arc<Self>, r_quic: RwLockReadGuard<'a, SharedQuic>, query: Message) -> io::Result<Message> {
        match &r_quic.state {
            QuicState::Connected(quic_connection, quic_kill) => {
                let quic_connection = quic_connection.clone();
                let quic_kill = quic_kill.clone();
                drop(r_quic);
                return self.query_quic(quic_connection, quic_kill, query).await;
            },
            QuicState::Establishing(quic_connection_sender) => {
                let mut quic_connection_receiver = quic_connection_sender.subscribe();
                drop(r_quic);
                match quic_connection_receiver.recv().await {
                    Ok((quic_connection, quic_kill)) => return self.query_quic(quic_connection, quic_kill, query).await,
                    Err(_) => Err(io::Error::from(ErrorKind::Interrupted)),
                }
            },
            QuicState::None => {
                drop(r_quic);
                let (quic_connection, quic_kill) = self.clone().init_quic().await?;
                return self.query_quic(quic_connection, quic_kill, query).await;
            },
            QuicState::Blocked => {
                drop(r_quic);
                return Err(io::Error::from(io::ErrorKind::ConnectionAborted));
            },
        }
    }

    #[inline]
    async fn query_quic(self: Arc<Self>, quic_connection: Connection, quic_kill: AwakeToken, mut query: Message) -> io::Result<Message> {
        pin!(
            let quic_kill_awoken = quic_kill.awoken();
        );
        // Step 1: Register the query as an in-flight message.

        // This is the initial query ID. However, it could change if it is already in use.
        query.id = rand::random();

        select! {
            mut w_in_flight = self.in_flight.write() => {
                // verify that ID is unique.
                while w_in_flight.contains(&query.id) {
                    query.id = rand::random();
                    // FIXME: should this fail after some number of non-unique keys? May want to verify that the list isn't full.
                }
                w_in_flight.insert(query.id);
                drop(w_in_flight);
            },
            _ = &mut quic_kill_awoken => return Err(io::Error::new(io::ErrorKind::Interrupted, "The connection was canceled locally")),
        }

        // IMPORTANT: This task is responsible for cleaning up the entry in `in_flight` for all
        //            return points after this,

        // Step 2: Serialize Data
        let raw_message = &mut [0_u8; MAX_MESSAGE_SIZE];
        let mut raw_message = WriteWire::from_bytes(raw_message);
        // Push two bytes onto the wire. These will be replaced with the u16 that indicates
        // the wire length.
        if let Err(error) = raw_message.write_bytes(&[0, 0]) {
            self.cleanup_query(query.id).await;
            return Err(io::Error::new(io::ErrorKind::InvalidData, error));
        };

        if let Err(wire_error) = query.to_wire_format(&mut raw_message, &mut Some(CompressionMap::new())) {
            self.cleanup_query(query.id).await;
            return Err(io::Error::new(io::ErrorKind::InvalidData, wire_error));
        };

        // Now, replace those two bytes from earlier with the wire length.
        let wire_length = raw_message.current_len();
        let message_wire_length: u16 = (wire_length - 2) as u16;
        let bytes = message_wire_length.to_be_bytes();
        if let Err(error) = raw_message.write_bytes_at(&bytes, 0) {
            self.cleanup_query(query.id).await;
            return Err(io::Error::new(io::ErrorKind::InvalidData, error));
        };

        // Step 3: Bounds check against the configurations.
        //  TODO: No configuration options have been defined yet.

        // Now that the message is registered, set up a stream to send the message over.
        let query_id = query.id;
        let (mut send_stream, mut receive_stream) = match select! {
            connection_result = quic_connection.open_bi() => connection_result,
            _ = &mut quic_kill_awoken => {
                self.clone().cleanup_query(query.id).await;
                return Err(io::Error::new(io::ErrorKind::Interrupted, format!("QUIC connection to {} was canceled locally", self.upstream_socket)))
            },
        } {
            Ok(streams) => streams,
            Err(error) => {
                eprintln!("Failed to open a bidirectional QUIC stream to {}", self.upstream_socket);
                self.cleanup_query(query.id).await;
                match error {
                    ConnectionError::VersionMismatch => return Err(io::Error::new(io::ErrorKind::Unsupported, error)),
                    ConnectionError::ConnectionClosed(_) | ConnectionError::ApplicationClosed(_) => return Err(io::Error::new(io::ErrorKind::ConnectionAborted, error)),
                    ConnectionError::Reset => return Err(io::Error::new(io::ErrorKind::ConnectionReset, error)),
                    ConnectionError::TimedOut => return Err(io::Error::new(io::ErrorKind::TimedOut, error)),
                    error => return Err(io::Error::new(io::ErrorKind::Other, error)),
                }
            },
        };

        // Step 4: Send the message via QUIC.
        self.recent_messages_sent.store(true, Ordering::SeqCst);
        println!("Sending on QUIC connection {} :: {:?}", self.upstream_socket, query);
        let bytes_written = match select! {
            send_result = send_stream.write(raw_message.current()) => send_result,
            _ = &mut quic_kill_awoken => {
                self.clone().cleanup_query(query.id).await;
                return Err(io::Error::new(io::ErrorKind::Interrupted, format!("QUIC connection to {} was canceled locally", self.upstream_socket)))
            },
        } {
            Ok(bytes_written) => bytes_written,
            Err(error) => {
                eprintln!("Failed to send message on QUIC connection to {}", self.upstream_socket);
                self.cleanup_query(query.id).await;
                return Err(io::Error::new(io::ErrorKind::Other, error));
            },
        };
        // Verify that the correct number of bytes were written.
        if bytes_written != wire_length {
            self.cleanup_query(query_id).await;
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Incorrect number of bytes sent to TCP stream; expected {wire_length} bytes but sent {bytes_written} bytes"),
            ));
        }

        let response = match select! {
            response = read_quic_message(&mut receive_stream) => response,
            _ = &mut quic_kill_awoken => {
                self.clone().cleanup_query(query.id).await;
                return Err(io::Error::new(io::ErrorKind::Interrupted, format!("QUIC connection to {} was canceled locally", self.upstream_socket)))
            },
        } {
            Ok(message) => {
                self.recent_messages_received.store(true, Ordering::SeqCst);
                Ok(message)
            },
            Err(error) => {
                println!("Failed to receive message on QUIC connection to {}", self.upstream_socket);
                Err(error)
            },
        };
        self.cleanup_query(query_id).await;
        return response;
    }

    pub async fn query(self: Arc<Self>, query: Message) -> io::Result<Message> {
        let self_lock = self.clone();
        let r_quic = self_lock.quic_shared.read().await;
        self.query_quic_rsocket(r_quic, query).await
    }
}

impl Drop for QuicSocket {
    fn drop(&mut self) {
        println!("Dropping socket {}", self.upstream_socket);
    }
}

#[inline]
async fn read_quic_message(quic_read_stream: &mut RecvStream) -> io::Result<Message> {
    // Step 1: Deserialize the u16 representing the size of the rest of the data. This is the first
    //         2 bytes of data.
    let mut wire_size = [0, 0];
    match quic_read_stream.read_exact(&mut wire_size).await {
        Ok(()) => (),
        Err(error @ ReadExactError::FinishedEarly(_)) => return Err(io::Error::new(io::ErrorKind::UnexpectedEof, error)),
        Err(error) => return Err(io::Error::new(io::ErrorKind::Other, error)),
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
    let mut quic_buffer = [0; MAX_MESSAGE_SIZE];
    let quic_buffer = &mut quic_buffer[..MAX_MESSAGE_SIZE];
    match quic_read_stream.read_exact(&mut quic_buffer[..expected_message_size]).await {
        Ok(()) => (),
        Err(error @ ReadExactError::FinishedEarly(_)) => return Err(io::Error::new(io::ErrorKind::UnexpectedEof, error)),
        Err(error) => return Err(io::Error::new(io::ErrorKind::Other, error)),
    };

    // Step 3: Deserialize the Message from the buffer.
    let mut wire = ReadWire::from_bytes(&mut quic_buffer[..expected_message_size]);
    let message = match Message::from_wire_format(&mut wire) {
        Ok(message) => message,
        Err(wire_error) => return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            wire_error,
        )),
    };

    return Ok(message);
}

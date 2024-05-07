use std::{collections::HashMap, io::ErrorKind, net::SocketAddr, sync::Arc, time::Duration};

use dns_lib::{query::message::Message, serde::wire::{compression_map::CompressionMap, from_wire::FromWire, read_wire::ReadWire, to_wire::ToWire, write_wire::WriteWire}};
use socket2::{SockRef, TcpKeepalive};
use tokio::{io::{self, AsyncReadExt, AsyncWriteExt}, net::{tcp::{OwnedReadHalf, OwnedWriteHalf}, TcpStream, UdpSocket}, pin, select, sync::{broadcast::{self}, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard}, task::{self, JoinHandle}, time};

use crate::cancel::Cancel;


const MAX_MESSAGE_SIZE: usize = 4096;


pub enum QueryOptions {
    TcpOnly,
    Both,
}

enum TcpState {
    Managed(Arc<Mutex<OwnedWriteHalf>>),
    Establishing(broadcast::Sender<Arc<Mutex<OwnedWriteHalf>>>),
    None,
    Blocked,
}

enum UdpState {
    // FIXME: Not sure if a lock is needed or not. As best I can tell, it is.
    Managed(Arc<UdpSocket>),
    None,
    Blocked,
}

struct InFlight {
    send_response: broadcast::Sender<Message>,
    payload: Message,
    
}

pub struct MixedSocket {
    udp_retransmit: Duration,
    udp_timeout_count: u8,
    udp_connection: UdpState,
    udp_listener: Option<JoinHandle<()>>,
    udp_kill: Arc<Cancel>,

    tcp_timeout: Duration,
    tcp_connection: TcpState,
    tcp_listener: Option<JoinHandle<()>>,
    tcp_kill: Arc<Cancel>,

    upstream_socket: SocketAddr,
    in_flight: Arc<RwLock<HashMap<u16, InFlight>>>,
}

impl MixedSocket {
    #[inline]
    pub fn new(upstream_socket: SocketAddr) -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(MixedSocket {
            udp_retransmit: Duration::from_millis(100),
            udp_timeout_count: 0,
            udp_connection: UdpState::None,
            udp_listener: None,
            udp_kill: Arc::new(Cancel::new()),

            tcp_timeout: Duration::from_millis(500),
            tcp_connection: TcpState::None,
            tcp_listener: None,
            tcp_kill: Arc::new(Cancel::new()),

            upstream_socket: upstream_socket,
            in_flight: Arc::new(RwLock::new(HashMap::new())),
        }))
    }

    #[inline]
    async fn listen_udp(socket: Arc<RwLock<Self>>, udp_reader: Arc<UdpSocket>, in_flight: Arc<RwLock<HashMap<u16, InFlight>>>) {
        let upstream_socket = match udp_reader.peer_addr() {
            Ok(upstream_socket) => upstream_socket,
            Err(_) => {
                Self::listen_udp_cleanup(socket).await;
                return;
            },
        };

        loop {
            match read_udp_message(udp_reader.clone()).await {
                Ok(response) => {
                    // Note: if truncation flag is set, that will be dealt with by the caller.
                    let response_id = response.id;
                    let r_in_flight = in_flight.read().await;
                    if let Some(InFlight{ send_response: sender, payload: _ }) = r_in_flight.get(&response_id) {
                        match sender.send(response) {
                            Ok(_) => (),
                            Err(_) => println!("No processes are waiting for message {}", response_id),
                        };
                        drop(r_in_flight);
                        // Cleanup is handled by the management processes. This
                        // process is free to move on.
                    };
                },
                Err(error) => match error.kind() {
                    io::ErrorKind::ConnectionRefused => {println!("UDP Listener for {upstream_socket} unable to read from stream (fatal). Connection Refused: {error}"); break;},
                    io::ErrorKind::ConnectionReset => {println!("UDP Listener for {upstream_socket} unable to read from stream (fatal). Connection Reset: {error}"); break;},
                    io::ErrorKind::ConnectionAborted => {println!("UDP Listener for {upstream_socket} unable to read from stream (fatal). Connection Aborted: {error}"); break;},
                    io::ErrorKind::NotConnected => {println!("UDP Listener for {upstream_socket} unable to read from stream (fatal). Not Connected: {error}"); break;},
                    io::ErrorKind::AddrInUse => {println!("UDP Listener for {upstream_socket} unable to read from stream (fatal). Address In Use: {error}"); break;},
                    io::ErrorKind::AddrNotAvailable => {println!("UDP Listener for {upstream_socket} unable to read from stream (fatal). Address Not Available: {error}"); break;},
                    io::ErrorKind::TimedOut => {println!("UDP Listener for {upstream_socket} unable to read from stream (fatal). Timed Out: {error}"); break;},
                    io::ErrorKind::Unsupported => {println!("UDP Listener for {upstream_socket} unable to read from stream (fatal). Unsupported: {error}"); break;},
                    io::ErrorKind::BrokenPipe => {println!("UDP Listener for {upstream_socket} unable to read from stream (fatal). Broken Pipe: {error}"); break;},
                    io::ErrorKind::UnexpectedEof => (),   //< This error usually occurs a bunch of times and fills up the logs. Don't want to print it.
                    _ => println!("UDP Listener for {upstream_socket} unable to read from stream (non-fatal). {error}"),
                },
            }
        }

        Self::listen_udp_cleanup(socket).await;
    }

    #[inline]
    async fn listen_udp_cleanup(socket: Arc<RwLock<Self>>) {
        let mut w_socket = socket.write().await;
        println!("Cleaning up UDP socket {}", w_socket.upstream_socket);
        w_socket.udp_connection = UdpState::None;

        // We don't want anyone else to register for this cancellation token. Instead, we'll replace
        // it with a new cancel that new tasks can subscribe to.
        let old_udp_kill = w_socket.udp_kill.clone();
        w_socket.udp_kill = Arc::new(Cancel::new());
        drop(w_socket);

        old_udp_kill.cancel();
    }

    #[inline]
    async fn listen_tcp(socket: Arc<RwLock<Self>>, mut tcp_reader: OwnedReadHalf, in_flight: Arc<RwLock<HashMap<u16, InFlight>>>) {
        let upstream_socket = match tcp_reader.peer_addr() {
            Ok(upstream_socket) => upstream_socket,
            Err(_) => {
                Self::listen_tcp_cleanup(socket).await;
                return;
            },
        };

        loop {
            match read_tcp_message(&mut tcp_reader).await {
                Ok(response) => {
                    let response_id = response.id;
                    let r_in_flight = in_flight.read().await;
                    if let Some(InFlight{ send_response: sender, payload: _ }) = r_in_flight.get(&response_id) {
                        match sender.send(response) {
                            Ok(_) => (),
                            Err(_) => println!("No processes are waiting for message {}", response_id),
                        };
                        drop(r_in_flight);
                        // Cleanup is handled by the management processes. This
                        // process is free to move on.
                    };
                },
                Err(error) => match error.kind() {
                    io::ErrorKind::ConnectionRefused => {println!("TCP Listener for {upstream_socket} unable to read from stream (fatal). Connection Refused: {error}"); break;},
                    io::ErrorKind::ConnectionReset => {println!("TCP Listener for {upstream_socket} unable to read from stream (fatal). Connection Reset: {error}"); break;},
                    io::ErrorKind::ConnectionAborted => {println!("TCP Listener for {upstream_socket} unable to read from stream (fatal). Connection Aborted: {error}"); break;},
                    io::ErrorKind::NotConnected => {println!("TCP Listener for {upstream_socket} unable to read from stream (fatal). Not Connected: {error}"); break;},
                    io::ErrorKind::AddrInUse => {println!("TCP Listener for {upstream_socket} unable to read from stream (fatal). Address In Use: {error}"); break;},
                    io::ErrorKind::AddrNotAvailable => {println!("TCP Listener for {upstream_socket} unable to read from stream (fatal). Address Not Available: {error}"); break;},
                    io::ErrorKind::TimedOut => {println!("TCP Listener for {upstream_socket} unable to read from stream (fatal). Timed Out: {error}"); break;},
                    io::ErrorKind::Unsupported => {println!("TCP Listener for {upstream_socket} unable to read from stream (fatal). Unsupported: {error}"); break;},
                    io::ErrorKind::BrokenPipe => {println!("TCP Listener for {upstream_socket} unable to read from stream (fatal). Broken Pipe: {error}"); break;},
                    io::ErrorKind::UnexpectedEof => (),   //< This error usually occurs a bunch of times and fills up the logs. Don't want to print it.
                    _ => println!("TCP Listener for {upstream_socket} unable to read from stream (non-fatal). {error}"),
                },
            }
        }

        Self::listen_tcp_cleanup(socket).await;
    }

    #[inline]
    async fn listen_tcp_cleanup(socket: Arc<RwLock<Self>>) {
        let mut w_socket = socket.write().await;
        println!("Cleaning up TCP socket {}", w_socket.upstream_socket);
        w_socket.tcp_connection = TcpState::None;

        // We don't want anyone else to register for this cancellation token. Instead, we'll replace
        // it with a new cancel that new tasks can subscribe to.
        let old_tcp_kill = w_socket.tcp_kill.clone();
        w_socket.tcp_kill = Arc::new(Cancel::new());
        drop(w_socket);

        old_tcp_kill.cancel();
    }

    #[inline]
    pub async fn shutdown_udp(socket: Arc<RwLock<Self>>) -> io::Result<()> {
        let r_socket = socket.read().await;
        if let UdpState::Managed(udp_socket) = &r_socket.udp_connection {
            let udp_socket = udp_socket.clone();
            let socket_address = r_socket.upstream_socket;
            drop(r_socket);
            println!("Shutting down UDP socket {socket_address}");

            SockRef::from(udp_socket.as_ref()).shutdown(std::net::Shutdown::Both)?;
        }
        Ok(())
    }

    #[inline]
    pub async fn shutdown_tcp(socket: Arc<RwLock<Self>>) -> io::Result<()> {
        let r_socket = socket.read().await;
        if let TcpState::Managed(tcp_socket) = &r_socket.tcp_connection {
            let tcp_socket = tcp_socket.clone();
            let socket_address = r_socket.upstream_socket;
            drop(r_socket);
            println!("Shutting down TCP socket {socket_address}");

            let w_tcp_socket = tcp_socket.lock().await;
            SockRef::from(w_tcp_socket.as_ref()).shutdown(std::net::Shutdown::Both)?;
            drop(w_tcp_socket);
        }
        Ok(())
    }

    #[inline]
    async fn init_udp(socket: Arc<RwLock<Self>>) -> io::Result<Arc<UdpSocket>> {
        // Initially, verify if the connection has already been established.
        let r_socket = socket.read().await;
        let upstream_socket = r_socket.upstream_socket.clone();
        let in_flight = r_socket.in_flight.clone();

        match &r_socket.udp_connection {
            UdpState::Managed(udp_socket) => return Ok(udp_socket.clone()),
            UdpState::None => (),
            UdpState::Blocked => {
                drop(r_socket);
                return Err(io::Error::from(io::ErrorKind::ConnectionAborted));
            },
        }
        drop(r_socket);

        let udp_socket = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);
        udp_socket.connect(upstream_socket).await?;
        let udp_reader = udp_socket.clone();
        let udp_writer = udp_socket;

        let listener = task::spawn(Self::listen_udp(socket.clone(), udp_reader, in_flight));

        // Since there is no intermediate state while the UDP socket is being
        // set up and the lock is dropped, it is possible that another process
        // was doing the same task.

        let mut w_socket = socket.write().await;
        match &w_socket.udp_connection {
            UdpState::Managed(existing_udp_socket) => {
                listener.abort();
                return Ok(existing_udp_socket.clone());
            },
            UdpState::None => {
                w_socket.udp_connection = UdpState::Managed(udp_writer.clone());
            },
            UdpState::Blocked => {
                drop(w_socket);
                return Err(io::Error::from(io::ErrorKind::ConnectionAborted));
            },
        }
        drop(w_socket);

        return Ok(udp_writer);
    }

    #[inline]
    async fn init_tcp(socket: Arc<RwLock<Self>>) -> io::Result<Arc<Mutex<OwnedWriteHalf>>> {
        // Initially, verify if the connection has already been established.
        let r_socket = socket.read().await;
        let upstream_socket = r_socket.upstream_socket.clone();
        let in_flight = r_socket.in_flight.clone();

        match &r_socket.tcp_connection {
            TcpState::Managed(tcp_socket) => return Ok(tcp_socket.clone()),
            TcpState::Establishing(sender) => {
                let mut receiver = sender.subscribe();
                drop(r_socket);
                match receiver.recv().await {
                    Ok(tcp_socket) => return Ok(tcp_socket.clone()),
                    Err(_) => {
                        eprintln!("Failed to establish TCP connection to {upstream_socket}");
                        return Err(io::Error::from(io::ErrorKind::Interrupted));
                    },
                }
            },
            TcpState::None => (),
            TcpState::Blocked => {
                drop(r_socket);
                return Err(io::Error::from(io::ErrorKind::ConnectionAborted));
            },
        }
        drop(r_socket);

        // Setup for once the write lock is obtained.
        let (tcp_socket_sender, _) = broadcast::channel(1);

        // Need to re-verify state with new lock. State could have changed in between.
        let mut w_socket = socket.write().await;
        match &w_socket.tcp_connection {
            TcpState::Managed(tcp_socket) => return Ok(tcp_socket.clone()),
            TcpState::Establishing(sender) => {
                let mut receiver = sender.subscribe();
                drop(w_socket);
                match receiver.recv().await {
                    Ok(tcp_socket) => return Ok(tcp_socket.clone()),
                    Err(_) => {
                        eprintln!("Failed to establish TCP connection to {upstream_socket}");
                        return Err(io::Error::from(io::ErrorKind::Interrupted));
                    },
                }
            },
            TcpState::None => (),
            TcpState::Blocked => {
                drop(w_socket);
                return Err(io::Error::from(io::ErrorKind::ConnectionAborted));
            },
        }

        w_socket.tcp_connection = TcpState::Establishing(tcp_socket_sender.clone());
        drop(w_socket);
        println!("Initializing TCP connection to {upstream_socket}");

        // Since state has been set to Establishing, this process is now fully
        // in charge of establishing the TCP connection. Next time the write
        // lock is obtained, it won't need to check the state.

        let tcp_socket = match TcpStream::connect(upstream_socket).await {
            Ok(tcp_socket) => tcp_socket,
            Err(error) => {
                eprintln!("Failed to establish TCP connection to {upstream_socket}");

                // Before returning, we must ensure that the "Establishing" status gets cleared
                // since we failed to establish the connection.
                let mut w_socket = socket.write().await;
                w_socket.tcp_connection = TcpState::None;
                drop(w_socket);

                // Notify all of the waiters by dropping the sender. This
                // causes the receivers to receiver an error.

                // It might be worth adding another state that blocks future TCP connections.
                drop(tcp_socket_sender);
                return Err(error);
            },
        };
        // Keep-alive configs
        let keep_alive = TcpKeepalive::new()
            .with_time(Duration::from_secs(2))
            .with_interval(Duration::from_secs(2))
            .with_retries(4);
        if let Err(_) = SockRef::from(&tcp_socket).set_tcp_keepalive(&keep_alive) {
            // This is not a fatal error, but it is not ideal either.
            eprintln!("WARNING: Failed to establish the keepalive settings for TCP connection to {upstream_socket}")
        };
        let (tcp_reader, tcp_writer) = tcp_socket.into_split();
        let tcp_writer = Arc::new(Mutex::new(tcp_writer));

        task::spawn(Self::listen_tcp(socket.clone(), tcp_reader, in_flight));

        let mut w_socket = socket.write().await;
        w_socket.tcp_connection = TcpState::Managed(tcp_writer.clone());
        drop(w_socket);

        let _ = tcp_socket_sender.send(tcp_writer.clone());

        return Ok(tcp_writer);
    }

    #[inline]
    pub async fn start_udp(socket: Arc<RwLock<Self>>) -> io::Result<()> {
        match Self::init_udp(socket).await {
            Ok(_) => Ok(()),
            Err(error) => Err(error),
        }
    }

    #[inline]
    pub async fn start_tcp(socket: Arc<RwLock<Self>>) -> io::Result<()> {
        match Self::init_tcp(socket).await {
            Ok(_) => Ok(()),
            Err(error) => Err(error),
        }
    }

    #[inline]
    async fn cleanup_query(in_flight: Arc<RwLock<HashMap<u16, InFlight>>>, query_id: u16) {
        let mut w_in_flight = in_flight.write().await;
        // Removing the message will cause the sender to be dropped. If there
        // was no response, tasks still awaiting a response will receive an error.
        w_in_flight.remove(&query_id);
        drop(w_in_flight);
    }

    #[inline]
    async fn manage_udp_query(udp_socket: Arc<UdpSocket>, socket: Arc<RwLock<Self>>, in_flight: Arc<RwLock<HashMap<u16, InFlight>>>, udp_retransmit: Duration, tcp_timeout: Duration, in_flight_sender: broadcast::Sender<Message>, query: Message) {
        let mut in_flight_receiver = in_flight_sender.subscribe();
        drop(in_flight_sender);
        let query_id = query.id;

        let r_socket = socket.read().await;
        let udp_kill = r_socket.udp_kill.clone();
        drop(r_socket);
        pin!{
            let udp_canceled = udp_kill.canceled();
        }

        // Timeout Case 1: resend with UDP
        select! {
            biased;
            response = in_flight_receiver.recv() => {
                match response {
                    Ok(_) => println!("UDP Cleanup: Response received with ID {}", query_id),
                    Err(broadcast::error::RecvError::Closed) => println!("UDP Cleanup: Channel closed for query with ID {}", query_id),
                    Err(broadcast::error::RecvError::Lagged(skipped_messages)) => println!("UDP Cleanup: Channel lagged for query with ID {}, skipping {skipped_messages} messages", query_id),
                }
                return Self::cleanup_query(in_flight, query_id).await;
            },
            _ = &mut udp_canceled => {
                println!("UDP Cleanup: UDP canceled while waiting to receive message with ID {}", query_id);
                return Self::cleanup_query(in_flight, query_id).await;
            },
            _ = time::sleep(udp_retransmit) => {
                println!("UDP Timeout: Retransmitting message with ID {} via UDP", query_id);
                Self::retransmit_query_udp(udp_socket, &query).await.expect("Failed to retransmit message via UDP");
                // Also start the process of setting up a TCP connection. This
                // way, by the time we timeout a second time (if we do, at
                // least), there is a TCP connection ready to go.
                task::spawn(Self::init_tcp(socket.clone()));
            },
        }

        // Timeout Case 2: resend with TCP
        select! {
            biased;
            response = in_flight_receiver.recv() => {
                match response {
                    Ok(_) => println!("UDP Cleanup: Response received with ID {}", query_id),
                    Err(broadcast::error::RecvError::Closed) => println!("UDP Cleanup: Channel closed for query with ID {}", query_id),
                    Err(broadcast::error::RecvError::Lagged(skipped_messages)) => println!("UDP Cleanup: Channel lagged for query with ID {}, skipping {skipped_messages} messages", query_id),
                }
                return Self::cleanup_query(in_flight, query_id).await;
            },
            _ = udp_canceled => {
                println!("UDP Cleanup: UDP canceled while waiting to receive message with ID {}", query_id);
                return Self::cleanup_query(in_flight, query_id).await;
            },
            _ = time::sleep(udp_retransmit) => {
                match Self::init_tcp(socket.clone()).await {
                    Ok(tcp_writer) => {
                        println!("UDP Timeout: Retransmitting message with ID {} via TCP", query_id);
                        Self::retransmit_query_tcp(tcp_writer, query).await.expect("Failed to retransmit message via TCP");
                    },
                    Err(error) => eprintln!("Unable to retransmit via TCP; {error}"),
                };
            },
        }

        let r_socket = socket.read().await;
        let tcp_kill = r_socket.tcp_kill.clone();
        drop(r_socket);

        // Once TCP is used, no more retransmissions will be done via this
        // manager. Its last job is to clean up after the message is received
        // or there is an error.
        select! {
            biased;
            response = in_flight_receiver.recv() => {
                match response {
                    Ok(_) => println!("UDP Cleanup: Response received with ID {}", query_id),
                    Err(broadcast::error::RecvError::Closed) => println!("UDP Cleanup: Channel closed for query with ID {}", query_id),
                    Err(broadcast::error::RecvError::Lagged(skipped_messages)) => println!("UDP Cleanup: Channel lagged for query with ID {}, skipping {skipped_messages} messages", query_id),
                }
            },
            _ = tcp_kill.canceled() => println!("UDP Cleanup: TCP canceled while waiting to receive message with ID {}", query_id),
            _ = time::sleep(tcp_timeout) => println!("UDP Timeout: TCP query with ID {} took too long to respond", query_id),
            // Note: we don't want to await UDP canceled anymore. As far as we are concerned, we
            //       have transitioned into a TCP manager.
        }
        Self::cleanup_query(in_flight, query_id).await;
    }

    #[inline]
    async fn query_udp_rsocket<'a>(r_socket: RwLockReadGuard<'a, Self>, socket: Arc<RwLock<Self>>, udp_retransmit: Duration, tcp_timeout: Duration, query: Message) -> io::Result<broadcast::Receiver<Message>> {
        let udp_socket;
        let in_flight;
        match &r_socket.udp_connection {
            UdpState::Managed(state_udp_socket) => {
                udp_socket = state_udp_socket.clone();
                in_flight = r_socket.in_flight.clone();
            },
            UdpState::None => {
                drop(r_socket);
                return Self::query_udp_wsocket(socket.write().await, socket.clone(), udp_retransmit, tcp_timeout, query).await;
            },
            UdpState::Blocked => {
                drop(r_socket);
                return Err(io::Error::from(io::ErrorKind::ConnectionAborted));
            },
        }
        drop(r_socket);
        Self::query_udp(udp_socket.clone(), socket, in_flight, udp_retransmit, tcp_timeout, query).await
    }

    #[inline]
    async fn query_udp_wsocket<'a, 'b>(w_socket: RwLockWriteGuard<'b, Self>, socket: Arc<RwLock<Self>>, udp_retransmit: Duration, tcp_timeout: Duration, query: Message) -> io::Result<broadcast::Receiver<Message>> where 'a: 'b {
        let udp_socket;
        let in_flight;
        match &w_socket.udp_connection {
            UdpState::Managed(state_udp_socket) => {
                udp_socket = state_udp_socket.clone();
                in_flight = w_socket.in_flight.clone();
                drop(w_socket);
            },
            UdpState::None => {
                // TODO: Handle the error cases properly.
                in_flight = w_socket.in_flight.clone();
                drop(w_socket);
                udp_socket = Self::init_udp(socket.clone()).await.expect("Unxpected IO Error");
            },
            UdpState::Blocked => {
                drop(w_socket);
                return Err(io::Error::from(io::ErrorKind::ConnectionAborted));
            },
        }
        Self::query_udp(udp_socket, socket, in_flight, udp_retransmit, tcp_timeout, query).await
    }

    #[inline]
    async fn query_udp(udp_socket: Arc<UdpSocket>, socket: Arc<RwLock<Self>>, in_flight: Arc<RwLock<HashMap<u16, InFlight>>>, udp_retransmit: Duration, tcp_timeout: Duration, mut query: Message) -> io::Result<broadcast::Receiver<Message>> {
        // Step 1: Register the query as an in-flight message.
        let (sender, receiver) = broadcast::channel(1);

        // This is the initial query ID. However, it could change if it is already in use.
        query.id = rand::random();

        let mut w_in_flight = in_flight.write().await;
        // verify that ID is unique.
        while w_in_flight.contains_key(&query.id) {
            query.id = rand::random();
            // FIXME: should this fail after some number of non-unique keys? May want to verify that the list isn't full.
        }
        w_in_flight.insert(query.id, InFlight{ send_response: sender.clone(), payload: query.clone() });
        drop(w_in_flight);

        // IMPORTANT: Between inserting the query ID (above) and starting the
        //            management process (later), if there is a return, it is
        //            responsible for cleaning up the entry in `in_flight`

        // Step 2: Serialize Data
        let raw_message = &mut [0_u8; MAX_MESSAGE_SIZE];
        let mut raw_message = WriteWire::from_bytes(raw_message);
        if let Err(wire_error) = query.to_wire_format(&mut raw_message, &mut Some(CompressionMap::new())) {
            Self::cleanup_query(in_flight, query.id).await;
            return Err(io::Error::new(io::ErrorKind::InvalidData, wire_error));
        };
        let wire_length = raw_message.len();

        // Step 3: Bounds check against the configurations.
        //  TODO: No configuration options have been defined yet.

        // Now that the message is registered, set up a task to ensure the
        // message is retransmitted as needed and cleaned up once done.
        let query_id = query.id;
        task::spawn(Self::manage_udp_query(udp_socket.clone(), socket, in_flight.clone(), udp_retransmit, tcp_timeout, sender, query.clone()));

        // Step 4: Send the message via UDP.
        println!("Sending on UDP socket {} :: {:?}", udp_socket.peer_addr().unwrap(), query);
        let bytes_written = udp_socket.send(raw_message.current_state()).await?;
        drop(udp_socket);
        // Verify that the correct number of bytes were sent.
        if bytes_written != wire_length {
            // Although cleanup is not required at this point, it should cause
            // all receivers to receive an error sooner.
            Self::cleanup_query(in_flight, query_id).await;
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Incorrect number of bytes sent to UDP socket; Expected {wire_length} bytes but sent {bytes_written} bytes"),
            ));
        }

        return Ok(receiver);
    }

    #[inline]
    async fn retransmit_query_udp(udp_socket: Arc<UdpSocket>, query: &Message) -> io::Result<()> {
        // Step 1: Skip. We are resending, in_flight was setup for initial transmission.

        // Step 2: Serialize Data
        let raw_message = &mut [0_u8; MAX_MESSAGE_SIZE];
        let mut raw_message = WriteWire::from_bytes(raw_message);
        if let Err(wire_error) = query.to_wire_format(&mut raw_message, &mut Some(CompressionMap::new())) {
            return Err(io::Error::new(io::ErrorKind::InvalidData, wire_error));
        };
        let wire_length = raw_message.len();

        // Step 3: Bounds check against the configurations.
        //  TODO: No configuration options have been defined yet.

        // Step 4: Send the message via UDP.
        println!("Sending on UDP socket {} :: {:?}", udp_socket.peer_addr().unwrap(), query);
        let bytes_written = udp_socket.send(raw_message.current_state()).await?;
        // Verify that the correct number of bytes were sent.
        if bytes_written != wire_length {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Incorrect number of bytes sent to UDP socket; Expected {wire_length} bytes but sent {bytes_written} bytes"),
            ));
        }

        return Ok(());
    }

    #[inline]
    async fn retransmit_query_tcp(tcp_socket: Arc<Mutex<OwnedWriteHalf>>, query: Message) -> io::Result<()> {
        // Step 1: Skip. We are resending, in_flight was setup for initial transmission.

        // Step 2: Serialize Data
        let raw_message = &mut [0_u8; MAX_MESSAGE_SIZE];
        let mut raw_message = WriteWire::from_bytes(raw_message);
        // Push two bytes onto the wire. These will be replaced with the u16 that indicates
        // the wire length.
        if let Err(error) = raw_message.write_bytes(&[0, 0]) {
            return Err(io::Error::new(io::ErrorKind::InvalidData, error));
        };

        if let Err(wire_error) = query.to_wire_format(&mut raw_message, &mut Some(CompressionMap::new())) {
            return Err(io::Error::new(io::ErrorKind::InvalidData, wire_error));
        };

        // Now, replace those two bytes from earlier with the wire length.
        let wire_length = raw_message.len();
        let message_wire_length = (wire_length - 2) as u16;
        let bytes = message_wire_length.to_be_bytes();
        if let Err(error) = raw_message.write_bytes_at(&bytes, 0) {
            return Err(io::Error::new(io::ErrorKind::InvalidData, error));
        };

        // Step 3: Bounds check against the configurations.
        //  TODO: No configuration options have been defined yet.

        // Step 4: Send the message via TCP.
        let mut w_tcp_stream = tcp_socket.lock().await;
        println!("Sending on TCP socket {} :: {:?}", w_tcp_stream.peer_addr().unwrap(), query);
        let bytes_written = w_tcp_stream.write(raw_message.current_state()).await?;
        drop(w_tcp_stream);
        // Verify that the correct number of bytes were written.
        if bytes_written != wire_length {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Incorrect number of bytes sent to TCP stream; expected {wire_length} bytes but sent {bytes_written} bytes"),
            ));
        }

        return Ok(());
    }

    #[inline]
    async fn manage_tcp_query(socket: Arc<RwLock<Self>>, in_flight: Arc<RwLock<HashMap<u16, InFlight>>>, tcp_timeout: Duration, in_flight_sender: broadcast::Sender<Message>, query: Message) {
        let mut in_flight_receiver = in_flight_sender.subscribe();
        drop(in_flight_sender);

        let r_socket = socket.read().await;
        let tcp_kill = r_socket.tcp_kill.clone();
        drop(r_socket);

        // Once TCP is used, no more retransmissions will be done via this
        // manager. Its last job is to clean up after the message is received.
        select! {
            biased;
            response = in_flight_receiver.recv() => {
                match response {
                    Ok(_) => println!("TCP Cleanup: Response received with ID {}", query.id),
                    Err(broadcast::error::RecvError::Closed) => println!("TCP Cleanup: Channel closed for query with ID {}", query.id),
                    Err(broadcast::error::RecvError::Lagged(skipped_messages)) => println!("TCP Cleanup: Channel lagged for query with ID {}, skipping {skipped_messages} messages", query.id),
                }
            },
            _ = tcp_kill.canceled() => println!("TCP Cleanup: TCP canceled while waiting to receive message with ID {}", query.id),
            _ = time::sleep(tcp_timeout) => println!("TCP Timeout: TCP query with ID {} took too long to respond", query.id),
        }
        Self::cleanup_query(in_flight, query.id).await;
        return;
    }

    #[inline]
    async fn query_tcp_rsocket<'a>(r_socket: RwLockReadGuard<'a, Self>, socket: Arc<RwLock<Self>>, query: Message) -> io::Result<broadcast::Receiver<Message>> {
        let in_flight = r_socket.in_flight.clone();
        let tcp_timeout = r_socket.tcp_timeout;
        match &r_socket.tcp_connection {
            TcpState::Managed(tcp_socket) => {
                let tcp_socket = tcp_socket.clone();
                drop(r_socket);
                return Self::query_tcp(tcp_socket, socket, in_flight, tcp_timeout, query).await;
            },
            TcpState::Establishing(tcp_socket_sender) => {
                let mut tcp_socket_receiver = tcp_socket_sender.subscribe();
                drop(r_socket);
                match tcp_socket_receiver.recv().await {
                    Ok(tcp_socket) => return Self::query_tcp(tcp_socket, socket, in_flight, tcp_timeout, query).await,
                    Err(_) => Err(io::Error::from(ErrorKind::Interrupted)),
                }
            },
            TcpState::None => {
                drop(r_socket);
                let tcp_socket = Self::init_tcp(socket.clone()).await?;
                return Self::query_tcp(tcp_socket, socket, in_flight, tcp_timeout, query).await;
            },
            TcpState::Blocked => {
                drop(r_socket);
                return Err(io::Error::from(io::ErrorKind::ConnectionAborted));
            },
        }
    }

    // async fn query_tcp_wsocket<'a>(mut w_socket: RwLockWriteGuard<'a, Self>, socket: Arc<RwLock<Self>>, query: Message) -> io::Result<broadcast::Receiver<Message>> {
    //     match &w_socket.tcp_connection {
    //         TcpState::Managed(tcp_socket) => {
    //             let tcp_socket = tcp_socket.clone();
    //             let in_flight = w_socket.in_flight.clone();
    //             drop(w_socket);
    //             return Self::query_tcp(tcp_socket, in_flight, query).await;
    //         },
    //         TcpState::Establishing(tcp_socket_sender) => {
    //             let in_flight = w_socket.in_flight.clone();
    //             let mut tcp_socket_receiver = tcp_socket_sender.subscribe();
    //             drop(w_socket);
    //             match tcp_socket_receiver.recv().await {
    //                 Ok(tcp_socket) => return Self::query_tcp(tcp_socket, in_flight, query).await,
    //                 Err(_) => Err(io::Error::from(ErrorKind::Interrupted)),
    //             }
    //         },
    //         TcpState::None => {
    //             Self::init_tcp(socket);
    //             return Self::query_tcp_wsocket(socket.write().await, socket.clone(), query).await;
    //         },
    //     }
    // }

    #[inline]
    async fn query_tcp(tcp_socket: Arc<Mutex<OwnedWriteHalf>>, socket: Arc<RwLock<Self>>, in_flight: Arc<RwLock<HashMap<u16, InFlight>>>, tcp_timeout: Duration, mut query: Message) -> io::Result<broadcast::Receiver<Message>> {
        // Step 1: Register the query as an in-flight message.
        let (sender, receiver) = broadcast::channel(1);

        // This is the initial query ID. However, it could change if it is already in use.
        query.id = rand::random();

        let mut w_in_flight = in_flight.write().await;
        // verify that ID is unique.
        while w_in_flight.contains_key(&query.id) {
            query.id = rand::random();
            // FIXME: should this fail after some number of non-unique keys? May want to verify that the list isn't full.
        }
        w_in_flight.insert(query.id, InFlight{ send_response: sender.clone(), payload: query.clone() });
        drop(w_in_flight);

        // IMPORTANT: Between inserting the query ID (above) and starting the
        //            management process (later), if there is a return, it is
        //            responsible for cleaning up the entry in `in_flight`

        // Step 2: Serialize Data
        let raw_message = &mut [0_u8; MAX_MESSAGE_SIZE];
        let mut raw_message = WriteWire::from_bytes(raw_message);
        // Push two bytes onto the wire. These will be replaced with the u16 that indicates
        // the wire length.
        if let Err(error) = raw_message.write_bytes(&[0, 0]) {
            drop(sender);
            Self::cleanup_query(in_flight, query.id).await;
            return Err(io::Error::new(io::ErrorKind::InvalidData, error));
        };

        if let Err(wire_error) = query.to_wire_format(&mut raw_message, &mut Some(CompressionMap::new())) {
            drop(sender);
            Self::cleanup_query(in_flight, query.id).await;
            return Err(io::Error::new(io::ErrorKind::InvalidData, wire_error));
        };

        // Now, replace those two bytes from earlier with the wire length.
        let wire_length = raw_message.len();
        let message_wire_length: u16 = (wire_length - 2) as u16;
        let bytes = message_wire_length.to_be_bytes();
        if let Err(error) = raw_message.write_bytes_at(&bytes, 0) {
            drop(sender);
            Self::cleanup_query(in_flight, query.id).await;
            return Err(io::Error::new(io::ErrorKind::InvalidData, error));
        };

        // Step 3: Bounds check against the configurations.
        //  TODO: No configuration options have been defined yet.

        // Now that the message is registered, set up a task to ensure the
        // message is retransmitted as needed and cleaned up once done.
        let query_id = query.id;
        task::spawn(Self::manage_tcp_query(socket, in_flight.clone(), tcp_timeout, sender, query.clone()));

        // Step 4: Send the message via TCP.
        let mut w_tcp_stream = tcp_socket.lock().await;
        println!("Sending on TCP socket {} :: {:?}", w_tcp_stream.peer_addr().unwrap(), query);
        let bytes_written = w_tcp_stream.write(raw_message.current_state()).await?;
        drop(w_tcp_stream);
        // Verify that the correct number of bytes were written.
        if bytes_written != wire_length {
            // Although cleanup is not required at this point, it should cause
            // all receivers to receive an error sooner.
            Self::cleanup_query(in_flight, query_id).await;
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Incorrect number of bytes sent to TCP stream; expected {wire_length} bytes but sent {bytes_written} bytes"),
            ));
        }

        return Ok(receiver);
    }

    pub async fn query(socket: Arc<RwLock<Self>>, query: Message, options: QueryOptions) -> io::Result<Message> {
        let r_socket = socket.read().await;
        let udp_transmit = r_socket.udp_retransmit;
        let tcp_timeout = r_socket.tcp_timeout;
        let mut receiver = match (options, r_socket.udp_timeout_count, &r_socket.udp_connection, &r_socket.tcp_connection) {
            (QueryOptions::Both, 0..=3, UdpState::None | UdpState::Managed(_), _) => Self::query_udp_rsocket(r_socket, socket.clone(), udp_transmit, tcp_timeout, query).await?,
            // Too many UDP timeouts, no TCP socket has been established.
            (QueryOptions::Both, 4.., UdpState::None | UdpState::Managed(_), TcpState::None) => {
                // It will query via UDP but will start setting up a TCP connection to fall back on.
                task::spawn(Self::init_tcp(socket.clone()));
                Self::query_udp_rsocket(r_socket, socket.clone(), udp_transmit, tcp_timeout, query).await?
            },

            // Only TCP is allowed
            (QueryOptions::TcpOnly, _, _, TcpState::None | TcpState::Establishing(_) | TcpState::Managed(_)) => Self::query_tcp_rsocket(r_socket, socket.clone(), query).await?,
            // Too many UDP timeouts, a TCP socket is still being setup or already exists.
            (QueryOptions::Both, 4.., _, TcpState::Establishing(_) | TcpState::Managed(_)) => Self::query_tcp_rsocket(r_socket, socket.clone(), query).await?,

            // Cases where one or both of the sockets are blocked.
            (QueryOptions::TcpOnly, _, _, TcpState::Blocked) => {
                drop(r_socket);
                return Err(io::Error::from(io::ErrorKind::ConnectionAborted));
            },
            (_, _, UdpState::Blocked, TcpState::Blocked) => {
                drop(r_socket);
                return Err(io::Error::from(io::ErrorKind::ConnectionAborted));
            },
            (QueryOptions::Both, 4.., UdpState::None | UdpState::Managed(_), TcpState::Blocked) => Self::query_udp_rsocket(r_socket, socket.clone(), udp_transmit, tcp_timeout, query).await?,
            (QueryOptions::Both, _, UdpState::Blocked, TcpState::None | TcpState::Establishing(_) | TcpState::Managed(_)) => Self::query_tcp_rsocket(r_socket, socket.clone(), query).await?,
        };
        match receiver.recv().await {
            Ok(response) => return Ok(response),
            Err(_) => return Err(io::Error::from(io::ErrorKind::Other)),
        };
    }
}

#[inline]
async fn read_udp_message(udp_socket: Arc<UdpSocket>) -> io::Result<Message> {
    // Step 1: Setup buffer. Make sure it is within the configured size.
    let mut buffer = [0; MAX_MESSAGE_SIZE];
    let mut buffer = &mut buffer[..MAX_MESSAGE_SIZE];

    // Step 2: Get the bytes from the UDP socket.
    let received_byte_count = udp_socket.recv(&mut buffer).await?;

    // Step 3: Deserialize the Message received on UDP socket.
    let mut wire = ReadWire::from_bytes(&mut buffer[..received_byte_count]);
    let message = match Message::from_wire_format(&mut wire) {
        Ok(message) => message,
        Err(wire_error) => return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            wire_error,
        )),
    };

    return Ok(message);
}

#[inline]
async fn read_tcp_message(tcp_stream: &mut OwnedReadHalf) -> io::Result<Message> {
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

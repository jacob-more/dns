use std::{collections::HashMap, io::ErrorKind, net::SocketAddr, sync::{atomic::{AtomicBool, AtomicU8, Ordering}, Arc}, time::Duration};

use async_lib::awake_token::AwakeToken;
use dns_lib::{query::message::Message, serde::wire::{compression_map::CompressionMap, from_wire::FromWire, read_wire::ReadWire, to_wire::ToWire, write_wire::WriteWire}, types::{base64::Base64, base_conversions::BaseConversions}};
use socket2::SockRef;
use tokio::{io::{self, AsyncReadExt, AsyncWriteExt}, join, net::{tcp::{OwnedReadHalf, OwnedWriteHalf}, TcpStream, UdpSocket}, pin, select, sync::{broadcast::{self, error::RecvError}, Mutex, RwLock, RwLockReadGuard}, task, time};

const MAX_MESSAGE_SIZE: usize = 8192;
const UDP_RETRANSMIT_MS: u64 = 125;
const TCP_TIMEOUT_MS: u64 = 500;


const TCP_INIT_ESTABLISHING_WAIT_MS: u64 = 1000;
const TCP_INIT_CONNECTING_WAIT_MS: u64 = 1000;
const TCP_LISTEN_TIMEOUT_MS: u64 = 1000 * 60 * 2;

const UDP_LISTEN_TIMEOUT_MS: u64 = 1000 * 60 * 2;


pub enum QueryOptions {
    TcpOnly,
    Both,
}

struct InFlight { send_response: broadcast::Sender<Message> }

enum TcpState {
    Managed {
        socket: Arc<Mutex<OwnedWriteHalf>>,
        kill: Arc<AwakeToken>
    },
    Establishing {
        sender: broadcast::Sender<(Arc<Mutex<OwnedWriteHalf>>, Arc<AwakeToken>)>,
        kill: Arc<AwakeToken>
    },
    None,
    Blocked,
}

// Implement TCP functions on MixedSocket
impl MixedSocket {
    #[inline]
    async fn init_tcp_handle_foreign_establishing(self: Arc<Self>, mut receiver: broadcast::Receiver<(Arc<Mutex<OwnedWriteHalf>>, Arc<AwakeToken>)>, sender: broadcast::Sender<(Arc<Mutex<OwnedWriteHalf>>, Arc<AwakeToken>)>, kill_init_tcp: Arc<AwakeToken>) -> io::Result<(Arc<Mutex<OwnedWriteHalf>>, Arc<AwakeToken>)> {
        select! {
            biased;
            received = receiver.recv() => match received {
                Ok(socket) => {
                    // Ignore send errors. They just indicate that all receivers have been dropped.
                    let _ = sender.send(socket.clone());
                    Ok(socket.clone())
                },
                Err(_) => Err(io::Error::from(io::ErrorKind::Interrupted)),
            },
            () = tokio::time::sleep(Duration::from_millis(TCP_INIT_ESTABLISHING_WAIT_MS)) => Err(io::Error::from(io::ErrorKind::TimedOut)),
            () = kill_init_tcp.awoken() => Err(io::Error::from(io::ErrorKind::Interrupted)),
        }
    }

    #[inline]
    async fn init_tcp(self: Arc<Self>) -> io::Result<(Arc<Mutex<OwnedWriteHalf>>, Arc<AwakeToken>)> {
        // Initially, verify if the connection has already been established.
        let r_state = self.tcp.read().await;
        match &*r_state {
            TcpState::Managed { socket, kill } => return Ok((socket.clone(), kill.clone())),
            TcpState::Establishing { sender, kill } => {
                let mut receiver = sender.subscribe();
                let kill_establishing = kill.clone();
                drop(r_state);
                select! {
                    biased;
                    received = receiver.recv() => match received {
                        Ok(socket) => return Ok(socket.clone()),
                        Err(_) => return Err(io::Error::from(io::ErrorKind::Interrupted)),
                    },
                    () = tokio::time::sleep(Duration::from_millis(TCP_INIT_ESTABLISHING_WAIT_MS)) => return Err(io::Error::from(io::ErrorKind::TimedOut)),
                    () = kill_establishing.awoken() => return Err(io::Error::from(io::ErrorKind::Interrupted)),
                };
            },
            TcpState::None => (),
            TcpState::Blocked => {
                drop(r_state);
                return Err(io::Error::from(io::ErrorKind::ConnectionAborted));
            },
        }
        drop(r_state);

        // Setup for once the write lock is obtained.
        let (tcp_socket_sender, _) = broadcast::channel(1);
        let kill_init_tcp = Arc::new(AwakeToken::new());

        // Need to re-verify state with new lock. State could have changed in between.
        let mut w_state = self.tcp.write().await;
        match &*w_state {
            TcpState::Managed { socket, kill } => return Ok((socket.clone(), kill.clone())),
            TcpState::Establishing { sender, kill } => {
                let mut receiver = sender.subscribe();
                let kill_establishing = kill.clone();
                drop(w_state);
                select! {
                    biased;
                    received = receiver.recv() => match received {
                        Ok(socket) => return Ok(socket.clone()),
                        Err(_) => return Err(io::Error::from(io::ErrorKind::Interrupted)),
                    },
                    () = tokio::time::sleep(Duration::from_millis(TCP_INIT_ESTABLISHING_WAIT_MS)) => return Err(io::Error::from(io::ErrorKind::TimedOut)),
                    () = kill_establishing.awoken() => return Err(io::Error::from(io::ErrorKind::Interrupted)),
                };
            },
            TcpState::None => {
                *w_state = TcpState::Establishing {
                    sender: tcp_socket_sender.clone(),
                    kill: kill_init_tcp.clone()
                };
                drop(w_state);
            },
            TcpState::Blocked => {
                drop(w_state);
                return Err(io::Error::from(io::ErrorKind::ConnectionAborted));
            },
        }

        // Establish a TCP connection
        let socket = select! {
            biased;
            () = kill_init_tcp.clone().awoken() => {
                // Notify all of the waiters by dropping the sender. This causes the receivers to receiver an error.
                drop(tcp_socket_sender);
                println!("Failed to establish TCP connection to {} (Canceled)", self.upstream_socket);
                return Err(io::Error::from(io::ErrorKind::ConnectionAborted));
            },
            () = tokio::time::sleep(Duration::from_millis(TCP_INIT_CONNECTING_WAIT_MS)) => {
                drop(tcp_socket_sender);
                println!("Failed to establish TCP connection to {} (Timeout)", self.upstream_socket);
                return Err(io::Error::from(io::ErrorKind::TimedOut));
            },
            socket = TcpStream::connect(self.upstream_socket) => match socket {
                Ok(socket) => socket,
                Err(error) => {
                    println!("Failed to establish TCP connection to {} ({error})", self.upstream_socket);

                    // Before returning, we must ensure that the "Establishing" status gets cleared
                    // since we failed to establish the connection.
                    let mut w_state = self.tcp.write().await;
                    match &*w_state {
                        TcpState::Managed { socket, kill } => {
                            let socket = socket.clone();
                            let kill = kill.clone();
                            drop(w_state);
                            println!("Warning: TCP Establishing State was set to Managed state before Establishing was completed for socket {}", self.upstream_socket);
                            // Ignore send errors. They just indicate that all receivers have been dropped.
                            let _ = tcp_socket_sender.send((socket.clone(), kill.clone()));
                            return Ok((socket, kill));
                        },
                        TcpState::Establishing { sender: est_sender, kill: est_kill_init_tcp } => {
                            // If we are the one who set the state to Establishing...
                            if Arc::ptr_eq(&kill_init_tcp, est_kill_init_tcp) {
                                // Notify all of the waiters by dropping the sender. This causes the receivers to receiver an error.
                                drop(tcp_socket_sender);

                                *w_state = TcpState::None;
                                drop(w_state);
                                return Err(error);
                            // If some other process set the state to Establishing...
                            } else {
                                let kill_init_tcp = est_kill_init_tcp.clone();
                                let receiver = est_sender.subscribe();
                                drop(w_state);
                                println!("Warning: TCP Establishing State was set to Establishing state (by another process) before Establishing was completed for socket {}", self.upstream_socket);
                                return self.init_tcp_handle_foreign_establishing(receiver, tcp_socket_sender, kill_init_tcp).await;
                            }
                        },
                        TcpState::None => {
                            drop(w_state);
                            println!("Warning: TCP Establishing State was set to None state before Establishing was completed for socket {}", self.upstream_socket);
                            // Notify all of the waiters by dropping the sender. This causes the receivers to receiver an error.
                            drop(tcp_socket_sender);
                            return Err(error);
                        },
                        TcpState::Blocked => {
                            drop(w_state);
                            println!("Warning: TCP Establishing State was set to Blocked state before Establishing was completed for socket {}", self.upstream_socket);
                            // Notify all of the waiters by dropping the sender. This causes the receivers to receiver an error.
                            drop(tcp_socket_sender);
                            return Err(error);
                        },
                    }
                },
            },
        };

        let (tcp_reader, tcp_writer) = socket.into_split();
        let tcp_writer = Arc::new(Mutex::new(tcp_writer));
        // Reuse the awake token. This also means that if the socket is killed while establishing,
        // it will remember that once it is established and can shut down without being told again.
        let kill_tcp = kill_init_tcp;

        // Start socket management
        let mut w_state = self.tcp.write().await;
        match &*w_state {
            TcpState::Managed { socket, kill } => {
                let socket = socket.clone();
                let kill = kill.clone();
                drop(w_state);
                println!("Warning: TCP Establishing State was set to Managed state before Establishing was completed for socket {}", self.upstream_socket);
                drop(tcp_reader);
                drop(tcp_writer);
                // Ignore send errors. They just indicate that all receivers have been dropped.
                let _ = tcp_socket_sender.send((socket.clone(), kill.clone()));
                return Ok((socket, kill));
            },
            TcpState::Establishing { sender: est_sender, kill: est_kill_init_tcp } => {
                // If we are the one who set the state to Establishing...
                if Arc::ptr_eq(&kill_tcp, est_kill_init_tcp) {
                    *w_state = TcpState::Managed {
                        socket: tcp_writer.clone(),
                        kill: kill_tcp.clone()
                    };
                    drop(w_state);

                    task::spawn(self.clone().listen_tcp(tcp_reader, kill_tcp.clone()));

                    // Ignore send errors. They just indicate that all receivers have been dropped.
                    let _ = tcp_socket_sender.send((tcp_writer.clone(), kill_tcp.clone()));
                    return Ok((tcp_writer, kill_tcp));
                // If some other process set the state to Establishing...
                } else {
                    let kill_init_tcp = est_kill_init_tcp.clone();
                    let receiver = est_sender.subscribe();
                    drop(w_state);

                    println!("Warning: TCP Establishing State was set to Establishing state (by another process) before Establishing was completed for socket {}", self.upstream_socket);

                    drop(tcp_reader);
                    drop(tcp_writer);

                    // Although this task is no longer allowed to use the TCP socket that it initialized, it can wait
                    // for the process that set the state to Establishing to establish its own TCP connection.
                    return self.init_tcp_handle_foreign_establishing(receiver, tcp_socket_sender, kill_init_tcp).await;
                }
            },
            TcpState::None => {
                drop(w_state);
                // Notify all of the waiters by dropping the sender. This causes the receivers to receiver an error.
                drop(tcp_socket_sender);
                println!("Warning: TCP Establishing State was set to None state before Establishing was completed for socket {}", self.upstream_socket);
                return Err(io::Error::from(io::ErrorKind::ConnectionAborted));
            },
            TcpState::Blocked => {
                drop(w_state);
                // Notify all of the waiters by dropping the sender. This causes the receivers to receiver an error.
                drop(tcp_socket_sender);
                println!("Warning: TCP Establishing State was set to Blocked state before Establishing was completed for socket {}", self.upstream_socket);
                return Err(io::Error::from(io::ErrorKind::ConnectionAborted));
            },
        };
    }

    #[inline]
    pub async fn start_tcp(self: Arc<Self>) -> io::Result<()> {
        match self.init_tcp().await {
            Ok(_) => Ok(()),
            Err(error) => Err(error),
        }
    }

    #[inline]
    pub async fn shutdown_tcp(self: Arc<Self>) -> io::Result<()> {
        println!("Shutting down TCP socket {}", self.upstream_socket);

        let mut w_state = self.tcp.write().await;
        match &*w_state {
            TcpState::Managed { socket, kill } => {
                let socket = socket.clone();
                let tcp_kill = kill.clone();
                *w_state = TcpState::None;
                drop(w_state);

                tcp_kill.awake();

                let w_tcp_socket = socket.lock().await;
                SockRef::from(w_tcp_socket.as_ref()).shutdown(std::net::Shutdown::Both)?;
                drop(w_tcp_socket);

                // Note: this task is not responsible for actual cleanup. Once the listener closes, it
                // will kill any active queries and change the TcpState.
            },
            TcpState::Establishing { sender, kill } => {
                let mut receiver = sender.subscribe();
                let kill_init_tcp = kill.clone();
                *w_state = TcpState::None;
                drop(w_state);

                // Try to prevent the socket from being initialized.
                kill_init_tcp.awake();

                // If the socket still initialized, shut it down immediately.
                match receiver.recv().await {
                    Ok((socket, tcp_kill)) => {
                        tcp_kill.awake();

                        let w_tcp_socket = socket.lock().await;
                        SockRef::from(w_tcp_socket.as_ref()).shutdown(std::net::Shutdown::Both)?;
                        drop(w_tcp_socket);
                    },
                    Err(_) => (), //< Successful cancellation
                }
            },
            TcpState::None => drop(w_state),    //< Already shut down
            TcpState::Blocked => drop(w_state), //< Already shut down
        }
        Ok(())
    }

    #[inline]
    pub async fn enable_tcp(self: Arc<Self>) -> io::Result<()> {
        println!("Enabling TCP socket {}", self.upstream_socket);

        let mut w_state = self.tcp.write().await;
        match &*w_state {
            TcpState::Managed { socket: _, kill: _ } => (),      //< Already enabled
            TcpState::Establishing { sender: _, kill: _ } => (), //< Already enabled
            TcpState::None => (),                                //< Already enabled
            TcpState::Blocked => *w_state = TcpState::None,
        }
        drop(w_state);
        Ok(())
    }

    #[inline]
    pub async fn disable_tcp(self: Arc<Self>) -> io::Result<()> {
        println!("Disabling TCP socket {}", self.upstream_socket);

        let mut w_state = self.tcp.write().await;
        match &*w_state {
            TcpState::Managed { socket, kill } => {
                let socket = socket.clone();
                let kill_tcp = kill.clone();
                *w_state = TcpState::Blocked;
                drop(w_state);

                kill_tcp.awake();

                let w_tcp_socket = socket.lock().await;
                SockRef::from(w_tcp_socket.as_ref()).shutdown(std::net::Shutdown::Both)?;
                drop(w_tcp_socket);

                // Note: this task is not responsible for actual cleanup. Once the listener closes, it
                // will kill any active queries and change the TcpState.
            },
            TcpState::Establishing { sender, kill }=> {
                let mut receiver = sender.subscribe();
                let kill_init_tcp = kill.clone();
                *w_state = TcpState::Blocked;
                drop(w_state);

                // Try to prevent the socket from being initialized.
                kill_init_tcp.awake();

                // If the socket still initialized, shut it down immediately.
                match receiver.recv().await {
                    Ok((socket, kill_tcp)) => {
                        kill_tcp.awake();

                        let w_tcp_socket = socket.lock().await;
                        SockRef::from(w_tcp_socket.as_ref()).shutdown(std::net::Shutdown::Both)?;
                        drop(w_tcp_socket);
                    },
                    Err(_) => (), //< Successful cancellation
                }
            },
            TcpState::None => {
                *w_state = TcpState::Blocked;
                drop(w_state)
            },
            TcpState::Blocked => drop(w_state), //< Already disabled
        }
        Ok(())
    }

    #[inline]
    async fn listen_tcp(self: Arc<Self>, mut tcp_reader: OwnedReadHalf, kill_tcp: Arc<AwakeToken>) {
        pin!(let kill_tcp_awoken = kill_tcp.clone().awoken(););
        loop {
            select! {
                biased;
                () = &mut kill_tcp_awoken => {
                    println!("TCP Socket {} Canceled. Shutting down TCP Listener.", self.upstream_socket);
                    break;
                },
                () = tokio::time::sleep(Duration::from_millis(TCP_LISTEN_TIMEOUT_MS)) => {
                    println!("TCP Socket {} Timed Out. Shutting down TCP Listener.", self.upstream_socket);
                    break;
                },
                response = read_tcp_message(&mut tcp_reader) => {
                    match response {
                        Ok(response) => {
                            self.recent_messages_received.store(true, Ordering::Release);
                            let response_id = response.id;
                            let r_in_flight = self.in_flight.read().await;
                            if let Some(InFlight{ send_response: sender }) = r_in_flight.get(&response_id) {
                                match sender.send(response) {
                                    Ok(_) => (),
                                    Err(_) => println!("No processes are waiting for message {}", response_id),
                                };
                            };
                            drop(r_in_flight);
                            // Cleanup is handled by the management processes. This
                            // process is free to move on.
                        },
                        Err(error) => match error.kind() {
                            io::ErrorKind::NotFound => {println!("TCP Listener for {} unable to read from stream (fatal). Not Found: {error}", self.upstream_socket); break;},
                            io::ErrorKind::PermissionDenied => {println!("TCP Listener for {} unable to read from stream (fatal). Permission Denied: {error}", self.upstream_socket); break;},
                            io::ErrorKind::ConnectionRefused => {println!("TCP Listener for {} unable to read from stream (fatal). Connection Refused: {error}", self.upstream_socket); break;},
                            io::ErrorKind::ConnectionReset => {println!("TCP Listener for {} unable to read from stream (fatal). Connection Reset: {error}", self.upstream_socket); break;},
                            io::ErrorKind::ConnectionAborted => {println!("TCP Listener for {} unable to read from stream (fatal). Connection Aborted: {error}", self.upstream_socket); break;},
                            io::ErrorKind::NotConnected => {println!("TCP Listener for {} unable to read from stream (fatal). Not Connected: {error}", self.upstream_socket); break;},
                            io::ErrorKind::AddrInUse => {println!("TCP Listener for {} unable to read from stream (fatal). Address In Use: {error}", self.upstream_socket); break;},
                            io::ErrorKind::AddrNotAvailable => {println!("TCP Listener for {} unable to read from stream (fatal). Address Not Available: {error}", self.upstream_socket); break;},
                            io::ErrorKind::TimedOut => {println!("TCP Listener for {} unable to read from stream (fatal). Timed Out: {error}", self.upstream_socket); break;},
                            io::ErrorKind::Unsupported => {println!("TCP Listener for {} unable to read from stream (fatal). Unsupported: {error}", self.upstream_socket); break;},
                            io::ErrorKind::BrokenPipe => {println!("TCP Listener for {} unable to read from stream (fatal). Broken Pipe: {error}", self.upstream_socket); break;},
                            io::ErrorKind::UnexpectedEof => {println!("TCP Listener for {} unable to read from stream (fatal). Unexpected End of File: {error}", self.upstream_socket); break;},
                            _ => {println!("TCP Listener for {} unable to read from stream (fatal). {error}", self.upstream_socket); break},
                        },
                    }
                },
            }
        }

        self.listen_tcp_cleanup(kill_tcp).await;
    }

    #[inline]
    async fn listen_tcp_cleanup(self: Arc<Self>, kill_tcp: Arc<AwakeToken>) {
        println!("Cleaning up TCP socket {}", self.upstream_socket);

        let mut w_state = self.tcp.write().await;
        match &*w_state {
            TcpState::Managed { socket, kill: managed_kill_tcp } => {
                // If the managed socket is the one that we are cleaning up...
                if Arc::ptr_eq(&kill_tcp, managed_kill_tcp) {
                    // We are responsible for cleanup.
                    let socket = socket.clone();
                    *w_state = TcpState::None;
                    drop(w_state);
    
                    kill_tcp.awake();

                    let w_tcp_socket = socket.lock().await;
                    // Tries to shut down the socket. But error does not particularly matter.
                    let _ = SockRef::from(w_tcp_socket.as_ref()).shutdown(std::net::Shutdown::Both);
                    drop(w_tcp_socket);

                // If the managed socket isn't the one that we are cleaning up...
                } else {
                    // This is not our socket to clean up.
                    drop(w_state);
                }
            },
            TcpState::Establishing { sender: _, kill: _ } => drop(w_state), //< Not our socket to clean up
            TcpState::None => drop(w_state),               //< Not our socket to clean up
            TcpState::Blocked => drop(w_state),            //< Not our socket to clean up
        }
    }

    #[inline]
    async fn manage_tcp_query(self: Arc<Self>, kill_tcp: Arc<AwakeToken>, in_flight_sender: broadcast::Sender<Message>, query: Message) {
        let mut in_flight_receiver = in_flight_sender.subscribe();
        drop(in_flight_sender);

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
            () = kill_tcp.awoken() => println!("TCP Cleanup: TCP canceled while waiting to receive message with ID {}", query.id),
            () = time::sleep(self.tcp_timeout) => println!("TCP Timeout: TCP query with ID {} took too long to respond", query.id),
        }
        self.cleanup_query(query.id).await;
        return;
    }

    #[inline]
    async fn query_tcp_rsocket<'a>(self: Arc<Self>, r_tcp_state: RwLockReadGuard<'a, TcpState>, query: Message) -> io::Result<broadcast::Receiver<Message>> {
        match &*r_tcp_state {
            TcpState::Managed { socket, kill: kill_tcp } => {
                let tcp_socket = socket.clone();
                let kill_tcp = kill_tcp.clone();
                drop(r_tcp_state);

                return self.query_tcp(tcp_socket, kill_tcp, query).await;
            },
            TcpState::Establishing { sender, kill: _ } => {
                let mut tcp_socket_receiver = sender.subscribe();
                drop(r_tcp_state);

                match tcp_socket_receiver.recv().await {
                    Ok((tcp_socket, tcp_kill)) => return self.query_tcp(tcp_socket, tcp_kill, query).await,
                    Err(RecvError::Closed) => return Err(io::Error::from(ErrorKind::Interrupted)),
                    Err(RecvError::Lagged(num_sockets)) => println!("UDP Query: Channel lagged skipping {num_sockets} sockets. Will try again"),
                };

                // Will only try 1 extra time if the receiver lags.
                // 
                // This really should not happen since a socket should only
                // ever be sent once so we will only retry once.
                match tcp_socket_receiver.recv().await {
                    Ok((tcp_socket, tcp_kill)) => return self.query_tcp(tcp_socket, tcp_kill, query).await,
                    Err(RecvError::Closed) => return Err(io::Error::from(ErrorKind::Interrupted)),
                    Err(RecvError::Lagged(num_sockets)) => {
                        println!("UDP Query: Channel lagged skipping {num_sockets} sockets. Will not try again");
                        return Err(io::Error::from(ErrorKind::Interrupted));
                    },
                };
            },
            TcpState::None => {
                drop(r_tcp_state);

                let (tcp_socket, tcp_kill) = self.clone().init_tcp().await?;
                return self.query_tcp(tcp_socket, tcp_kill, query).await;
            },
            TcpState::Blocked => {
                drop(r_tcp_state);

                return Err(io::Error::from(io::ErrorKind::ConnectionAborted));
            },
        }
    }

    #[inline]
    async fn query_tcp(self: Arc<Self>, tcp_socket: Arc<Mutex<OwnedWriteHalf>>, tcp_kill: Arc<AwakeToken>, mut query: Message) -> io::Result<broadcast::Receiver<Message>> {
        // Step 1: Register the query as an in-flight message.
        let (sender, receiver) = broadcast::channel(1);

        // This is the initial query ID. However, it could change if it is already in use.
        query.id = rand::random();

        let mut w_in_flight = self.in_flight.write().await;
        // verify that ID is unique.
        while w_in_flight.contains_key(&query.id) {
            query.id = rand::random();
            // FIXME: should this fail after some number of non-unique keys? May want to verify that the list isn't full.
        }
        w_in_flight.insert(query.id, InFlight{ send_response: sender.clone() });
        drop(w_in_flight);

        // IMPORTANT: Between inserting the query ID (above) and starting the
        //            management process (later), if there is a return, it is
        //            responsible for cleaning up the entry in `in_flight`

        // Step 2: Serialize Data
        let raw_message = &mut [0_u8; MAX_MESSAGE_SIZE];
        let mut raw_message = WriteWire::from_bytes(raw_message);
        if let Err(wire_error) = query.to_wire_format_with_two_octet_length(&mut raw_message, &mut Some(CompressionMap::new())) {
            drop(sender);
            self.cleanup_query(query.id).await;
            return Err(io::Error::new(io::ErrorKind::InvalidData, wire_error));
        };
        let wire_length = raw_message.current_len();

        // Step 3: Bounds check against the configurations.
        //  TODO: No configuration options have been defined yet.

        // Now that the message is registered, set up a task to ensure the
        // message is retransmitted as needed and cleaned up once done.
        let query_id = query.id;
        task::spawn(self.clone().manage_tcp_query(tcp_kill, sender, query.clone()));

        // Step 4: Send the message via TCP.
        self.recent_messages_sent.store(true, Ordering::Release);
        let mut w_tcp_stream = tcp_socket.lock().await;
        println!("Sending on TCP socket {} :: {:?}", self.upstream_socket, query);
        let bytes_written = w_tcp_stream.write(raw_message.current()).await?;
        drop(w_tcp_stream);
        // Verify that the correct number of bytes were written.
        if bytes_written != wire_length {
            // Although cleanup is not required at this point, it should cause
            // all receivers to receive an error sooner.
            self.cleanup_query(query_id).await;
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Incorrect number of bytes sent to TCP stream; expected {wire_length} bytes but sent {bytes_written} bytes"),
            ));
        }

        return Ok(receiver);
    }

    #[inline]
    async fn retransmit_query_tcp(self: Arc<Self>, tcp_socket: Arc<Mutex<OwnedWriteHalf>>, query: Message) -> io::Result<()> {
        // Step 1: Skip. We are resending, `in_flight` was setup for initial transmission.

        // Because a management process has already been established, there is
        // no need to clean up the entry in `in_flight` if function returns
        // early.

        // Step 2: Serialize Data
        let raw_message = &mut [0_u8; MAX_MESSAGE_SIZE];
        let mut raw_message = WriteWire::from_bytes(raw_message);
        if let Err(wire_error) = query.to_wire_format_with_two_octet_length(&mut raw_message, &mut Some(CompressionMap::new())) {
            return Err(io::Error::new(io::ErrorKind::InvalidData, wire_error));
        };
        let wire_length = raw_message.current_len();

        // Step 3: Bounds check against the configurations.
        //  TODO: No configuration options have been defined yet.

        // Step 4: Send the message via TCP.
        self.recent_messages_sent.store(true, Ordering::Release);
        let mut w_tcp_stream = tcp_socket.lock().await;
        println!("Sending on TCP socket {} :: {:?}", self.upstream_socket, query);
        let bytes_written = w_tcp_stream.write(raw_message.current()).await?;
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
}

enum UdpState {
    Managed(Arc<UdpSocket>, Arc<AwakeToken>),
    None,
    Blocked,
}

// Implement UDP functions on MixedSocket
impl MixedSocket {
    #[inline]
    async fn init_udp(self: Arc<Self>) -> io::Result<(Arc<UdpSocket>, Arc<AwakeToken>)> {
        // Initially, verify if the connection has already been established.
        let r_state = self.udp.read().await;
        match &*r_state {
            UdpState::Managed(udp_socket, kill_udp) => return Ok((udp_socket.clone(), kill_udp.clone())),
            UdpState::None => (),
            UdpState::Blocked => {
                drop(r_state);
                return Err(io::Error::from(io::ErrorKind::ConnectionAborted));
            },
        }
        drop(r_state);

        let udp_socket = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);
        udp_socket.connect(self.upstream_socket).await?;
        let udp_reader = udp_socket.clone();
        let udp_writer = udp_socket;
        let kill_udp = Arc::new(AwakeToken::new());

        // Since there is no intermediate state while the UDP socket is being
        // set up and the lock is dropped, it is possible that another process
        // was doing the same task.

        let mut w_state = self.udp.write().await;
        match &*w_state {
            UdpState::Managed(existing_udp_socket, _) => {
                return Ok((existing_udp_socket.clone(), kill_udp.clone()));
            },
            UdpState::None => {
                *w_state = UdpState::Managed(udp_writer.clone(), kill_udp.clone());
                drop(w_state);

                task::spawn(self.clone().listen_udp(udp_reader, kill_udp.clone()));

                return Ok((udp_writer, kill_udp));
            },
            UdpState::Blocked => {
                drop(w_state);
                return Err(io::Error::from(io::ErrorKind::ConnectionAborted));
            },
        }
    }

    #[inline]
    pub async fn start_udp(self: Arc<Self>) -> io::Result<()> {
        match self.init_udp().await {
            Ok(_) => Ok(()),
            Err(error) => Err(error),
        }
    }

    #[inline]
    pub async fn shutdown_udp(self: Arc<Self>) -> io::Result<()> {
        println!("Shutting down UDP socket {}", self.upstream_socket);

        let r_state = self.udp.read().await;
        if let UdpState::Managed(udp_socket, kill_udp) = &*r_state {
            let udp_socket = udp_socket.clone();
            let kill_udp = kill_udp.clone();
            drop(r_state);

            kill_udp.awake();
            
            SockRef::from(udp_socket.as_ref()).shutdown(std::net::Shutdown::Both)?;

            // Note: this task is not responsible for actual cleanup. Once the listener closes, it
            // will kill any active queries and change the UdpState.
        } else {
            drop(r_state);
        }
        Ok(())
    }

    #[inline]
    pub async fn enable_udp(self: Arc<Self>) -> io::Result<()> {
        println!("Enabling UDP socket {}", self.upstream_socket);
        
        let mut w_state = self.udp.write().await;
        match &*w_state {
            UdpState::Managed(_, _) => (),  //< Already enabled
            UdpState::None => (),           //< Already enabled
            UdpState::Blocked => *w_state = UdpState::None,
        }
        drop(w_state);
        return Ok(());
    }

    #[inline]
    pub async fn disable_udp(self: Arc<Self>) -> io::Result<()> {
        println!("Disabling UDP socket {}", self.upstream_socket);
        
        let mut w_state = self.udp.write().await;
        match &*w_state {
            UdpState::Managed(udp_socket, kill_udp) => {
                // Since we are removing the reference the kill_udp by setting state to Blocked, we
                // need to kill them now since the listener won't be able to kill them.
                let kill_udp = kill_udp.clone();
                let udp_socket = udp_socket.clone();
                *w_state = UdpState::Blocked;
                drop(w_state);

                kill_udp.awake();

                SockRef::from(udp_socket.as_ref()).shutdown(std::net::Shutdown::Both)?;

                Ok(())
            },
            UdpState::None => {
                *w_state = UdpState::Blocked;
                drop(w_state);
                Ok(())
            },
            UdpState::Blocked => { //< Already disabled
                drop(w_state);
                Ok(())
            },
        }
    }

    #[inline]
    async fn listen_udp(self: Arc<Self>, udp_reader: Arc<UdpSocket>, kill_udp: Arc<AwakeToken>) {
        pin!(let kill_udp_awoken = kill_udp.awoken(););
        loop {
            select! {
                biased;
                () = &mut kill_udp_awoken => {
                    println!("UDP Socket {} Canceled. Shutting down UDP Listener.", self.upstream_socket);
                    break;
                },
                () = tokio::time::sleep(Duration::from_millis(UDP_LISTEN_TIMEOUT_MS)) => {
                    println!("UDP Socket {} Timed Out. Shutting down UDP Listener.", self.upstream_socket);
                    break;
                },
                response = read_udp_message(udp_reader.clone()) => {
                    match response {
                        Ok(response) => {
                            // Note: if truncation flag is set, that will be dealt with by the caller.
                            self.recent_messages_received.store(true, Ordering::Release);
                            let response_id = response.id;
                            let r_in_flight = self.in_flight.read().await;
                            if let Some(InFlight{ send_response: sender }) = r_in_flight.get(&response_id) {
                                match sender.send(response) {
                                    Ok(_) => (),
                                    Err(_) => println!("No processes are waiting for message {}", response_id),
                                };
                            };
                            drop(r_in_flight);
                            // Cleanup is handled by the management processes. This
                            // process is free to move on.
                        },
                        Err(error) => match error.kind() {
                            io::ErrorKind::NotFound => {println!("UDP Listener for {} unable to read from stream (fatal). Not Found: {error}", self.upstream_socket); break;},
                            io::ErrorKind::PermissionDenied => {println!("UDP Listener for {} unable to read from stream (fatal). Permission Denied: {error}", self.upstream_socket); break;},
                            io::ErrorKind::ConnectionRefused => {println!("UDP Listener for {} unable to read from stream (fatal). Connection Refused: {error}", self.upstream_socket); break;},
                            io::ErrorKind::ConnectionReset => {println!("UDP Listener for {} unable to read from stream (fatal). Connection Reset: {error}", self.upstream_socket); break;},
                            io::ErrorKind::ConnectionAborted => {println!("UDP Listener for {} unable to read from stream (fatal). Connection Aborted: {error}", self.upstream_socket); break;},
                            io::ErrorKind::NotConnected => {println!("UDP Listener for {} unable to read from stream (fatal). Not Connected: {error}", self.upstream_socket); break;},
                            io::ErrorKind::AddrInUse => {println!("UDP Listener for {} unable to read from stream (fatal). Address In Use: {error}", self.upstream_socket); break;},
                            io::ErrorKind::AddrNotAvailable => {println!("UDP Listener for {} unable to read from stream (fatal). Address Not Available: {error}", self.upstream_socket); break;},
                            io::ErrorKind::TimedOut => {println!("UDP Listener for {} unable to read from stream (fatal). Timed Out: {error}", self.upstream_socket); break;},
                            io::ErrorKind::Unsupported => {println!("UDP Listener for {} unable to read from stream (fatal). Unsupported: {error}", self.upstream_socket); break;},
                            io::ErrorKind::BrokenPipe => {println!("UDP Listener for {} unable to read from stream (fatal). Broken Pipe: {error}", self.upstream_socket); break;},
                            io::ErrorKind::UnexpectedEof => {println!("UDP Listener for {} unable to read from stream (fatal). Unexpected End of File: {error}", self.upstream_socket); break;},
                            _ => {println!("UDP Listener for {} unable to read from stream (fatal). {error}", self.upstream_socket); break},
                        },
                    }
                },
            }
        }

        self.listen_udp_cleanup().await;
    }

    #[inline]
    async fn listen_udp_cleanup(self: Arc<Self>) {
        println!("Cleaning up UDP socket {}", self.upstream_socket);

        let mut w_state = self.udp.write().await;
        match &*w_state {
            UdpState::Managed(_, kill_udp) => {
                let kill_udp = kill_udp.clone();
                *w_state = UdpState::None;
                drop(w_state);

                kill_udp.awake();
            },
            UdpState::None => (),
            UdpState::Blocked => (),
        }
    }

    #[inline]
    async fn manage_udp_query(self: Arc<Self>, udp_socket: Arc<UdpSocket>, kill_udp: Arc<AwakeToken>, in_flight_sender: broadcast::Sender<Message>, query: Message) {
        let mut in_flight_receiver = in_flight_sender.subscribe();
        drop(in_flight_sender);
        let query_id = query.id;

        pin!{let udp_canceled = kill_udp.clone().awoken();}

        // Timeout Case 1: resend with UDP
        select! {
            biased;
            response = in_flight_receiver.recv() => {
                match response {
                    Ok(_) => println!("UDP Cleanup: Response received with ID {}", query_id),
                    Err(broadcast::error::RecvError::Closed) => println!("UDP Cleanup: Channel closed for query with ID {}", query_id),
                    Err(broadcast::error::RecvError::Lagged(skipped_messages)) => println!("UDP Cleanup: Channel lagged for query with ID {}, skipping {skipped_messages} messages", query_id),
                }
                return self.cleanup_query(query_id).await;
            },
            () = &mut udp_canceled => {
                println!("UDP Cleanup: UDP canceled while waiting to receive message with ID {}", query_id);
                return self.cleanup_query(query_id).await;
            },
            () = time::sleep(self.udp_retransmit) => {
                println!("UDP Timeout: Retransmitting message with ID {} via UDP", query_id);
                task::spawn(self.clone().retransmit_query_udp(udp_socket, query.clone()));
                // Also start the process of setting up a TCP connection. This
                // way, by the time we timeout a second time (if we do, at
                // least), there is a TCP connection ready to go.
                task::spawn(self.clone().init_tcp());
            },
        }

        // Timeout Case 2: resend with TCP
        let kill_tcp = select! {
            biased;
            response = in_flight_receiver.recv() => {
                match response {
                    Ok(_) => println!("UDP Cleanup: Response received with ID {}", query_id),
                    Err(broadcast::error::RecvError::Closed) => println!("UDP Cleanup: Channel closed for query with ID {}", query_id),
                    Err(broadcast::error::RecvError::Lagged(skipped_messages)) => println!("UDP Cleanup: Channel lagged for query with ID {}, skipping {skipped_messages} messages", query_id),
                }
                return self.cleanup_query(query_id).await;
            },
            () = &mut udp_canceled => {
                println!("UDP Cleanup: UDP canceled while waiting to receive message with ID {}", query_id);
                return self.cleanup_query(query_id).await;
            },
            // Although it looks gross, the nested select statement here helps prevent responses
            // from being missed. Unlike the other cases where we can offload the heavy work to
            // other tasks, we need the result here so this seems to be the best way to do it.
            // Note that we do still offload the tcp initialization to another task so that it is
            // cancel-safe, but we then await the join handle.
            () = time::sleep(self.udp_retransmit) => select! {
                biased;
                response = in_flight_receiver.recv() => {
                    match response {
                        Ok(_) => println!("UDP Cleanup: Response received with ID {}", query_id),
                        Err(broadcast::error::RecvError::Closed) => println!("UDP Cleanup: Channel closed for query with ID {}", query_id),
                        Err(broadcast::error::RecvError::Lagged(skipped_messages)) => println!("UDP Cleanup: Channel lagged for query with ID {}, skipping {skipped_messages} messages", query_id),
                    }
                    return self.cleanup_query(query_id).await;
                },
                () = &mut udp_canceled => {
                    println!("UDP Cleanup: UDP canceled while waiting to receive message with ID {}", query_id);
                    return self.cleanup_query(query_id).await;
                },
                () = time::sleep(self.tcp_timeout) => {
                    println!("UDP Timeout: Took too long to establish TCP connection to receive message with ID {}", query_id);
                    return self.cleanup_query(query_id).await;
                },
                result = task::spawn(self.clone().init_tcp()) => {
                    match result {
                        Ok(Ok((tcp_writer, kill_tcp))) => {
                            println!("UDP Timeout: Retransmitting message with ID {} via TCP", query_id);
                            task::spawn(self.clone().retransmit_query_tcp(tcp_writer, query));
                            kill_tcp
                        },
                        Ok(Err(error)) => {
                            println!("UDP Timeout: Unable to retransmit via TCP (still waiting for UDP); {error}");
                            // If we cannot re-transmit with TCP, then we are still waiting on UDP. So,
                            // we are still actually interested in the UDP kill token since that's the
                            // socket that is going to give us our answer.
                            kill_udp
                        },
                        Err(join_error) => {
                            println!("UDP Timeout: Unable to retransmit via TCP (still waiting for UDP); {join_error}");
                            // If we cannot re-transmit with TCP, then we are still waiting on UDP. So,
                            // we are still actually interested in the UDP kill token since that's the
                            // socket that is going to give us our answer.
                            kill_udp
                        }
                    }
                },
            },
        };

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
                };
                return self.cleanup_query(query_id).await;
            },
            // Note: we don't want to await the UDP killer anymore. As far as
            // we are concerned, we have transitioned into a TCP manager.
            () = kill_tcp.awoken() => {
                println!("UDP Cleanup: TCP canceled while waiting to receive message with ID {}", query_id);
                return self.cleanup_query(query_id).await;
            },
            () = time::sleep(self.tcp_timeout) => {
                println!("UDP Timeout: TCP query with ID {} took too long to respond", query_id);
                return self.cleanup_query(query_id).await;
            },
        }
    }

    #[inline]
    async fn query_udp_rsocket<'a>(self: Arc<Self>, r_udp_state: RwLockReadGuard<'a, UdpState>, query: Message) -> io::Result<broadcast::Receiver<Message>> {
        let udp_socket;
        let kill_udp;

        match &*r_udp_state {
            UdpState::Managed(state_udp_socket, state_kill_udp) => {
                udp_socket = state_udp_socket.clone();
                kill_udp = state_kill_udp.clone();
                drop(r_udp_state);

                return self.query_udp(udp_socket.clone(), kill_udp, query).await;
            },
            UdpState::None => {
                drop(r_udp_state);

                (udp_socket, kill_udp) = self.clone().init_udp().await?;
                return self.query_udp(udp_socket, kill_udp, query).await;
            },
            UdpState::Blocked => {
                drop(r_udp_state);

                return Err(io::Error::from(io::ErrorKind::ConnectionAborted));
            },
        }
    }

    #[inline]
    async fn query_udp(self: Arc<Self>, udp_socket: Arc<UdpSocket>, kill_udp: Arc<AwakeToken>, mut query: Message) -> io::Result<broadcast::Receiver<Message>> {
        // Step 1: Register the query as an in-flight message.
        let (sender, receiver) = broadcast::channel(1);

        // This is the initial query ID. However, it could change if it is already in use.
        query.id = rand::random();

        let mut w_in_flight = self.in_flight.write().await;
        // verify that ID is unique.
        while w_in_flight.contains_key(&query.id) {
            query.id = rand::random();
            // FIXME: should this fail after some number of non-unique keys? May want to verify that the list isn't full.
        }
        w_in_flight.insert(query.id, InFlight{ send_response: sender.clone() });
        drop(w_in_flight);

        // IMPORTANT: Between inserting the query ID (above) and starting the
        //            management process (later), if there is a return, it is
        //            responsible for cleaning up the entry in `in_flight`

        // Step 2: Serialize Data
        let raw_message = &mut [0_u8; MAX_MESSAGE_SIZE];
        let mut raw_message = WriteWire::from_bytes(raw_message);
        if let Err(wire_error) = query.to_wire_format(&mut raw_message, &mut Some(CompressionMap::new())) {
            self.cleanup_query(query.id).await;
            return Err(io::Error::new(io::ErrorKind::InvalidData, wire_error));
        };
        let wire_length = raw_message.current_len();

        // Step 3: Bounds check against the configurations.
        //  TODO: No configuration options have been defined yet.

        // Now that the message is registered, set up a task to ensure the
        // message is retransmitted as needed and cleaned up once done.
        let query_id = query.id;
        task::spawn(self.clone().manage_udp_query(udp_socket.clone(), kill_udp, sender, query.clone()));

        // Step 4: Send the message via UDP.
        self.recent_messages_sent.store(true, Ordering::Release);
        println!("Sending on UDP socket {} :: {:?}", self.upstream_socket, query);
        let bytes_written = udp_socket.send(raw_message.current()).await?;
        drop(udp_socket);
        // Verify that the correct number of bytes were sent.
        if bytes_written != wire_length {
            // Although cleanup is not required at this point, it should cause
            // all receivers to receive an error sooner.
            self.cleanup_query(query_id).await;
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Incorrect number of bytes sent to UDP socket; Expected {wire_length} bytes but sent {bytes_written} bytes"),
            ));
        }

        return Ok(receiver);
    }

    #[inline]
    async fn retransmit_query_udp(self: Arc<Self>, udp_socket: Arc<UdpSocket>, query: Message) -> io::Result<()> {
        // Step 1: Skip. We are resending, in_flight was setup for initial transmission.

        // Step 2: Serialize Data
        let raw_message = &mut [0_u8; MAX_MESSAGE_SIZE];
        let mut raw_message = WriteWire::from_bytes(raw_message);
        if let Err(wire_error) = query.to_wire_format(&mut raw_message, &mut Some(CompressionMap::new())) {
            return Err(io::Error::new(io::ErrorKind::InvalidData, wire_error));
        };
        let wire_length = raw_message.current_len();

        // Step 3: Bounds check against the configurations.
        //  TODO: No configuration options have been defined yet.

        // Step 4: Send the message via UDP.
        self.recent_messages_sent.store(true, Ordering::Release);
        println!("Sending on UDP socket {} :: {:?}", self.upstream_socket, query);
        let bytes_written = udp_socket.send(raw_message.current()).await?;
        // Verify that the correct number of bytes were sent.
        if bytes_written != wire_length {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Incorrect number of bytes sent to UDP socket; Expected {wire_length} bytes but sent {bytes_written} bytes"),
            ));
        }

        return Ok(());
    }
}

pub struct MixedSocket {
    udp_retransmit: Duration,
    udp_timeout_count: AtomicU8,
    udp: RwLock<UdpState>,

    tcp_timeout: Duration,
    tcp: RwLock<TcpState>,

    upstream_socket: SocketAddr,
    in_flight: RwLock<HashMap<u16, InFlight>>,

    // Counters used to determine when the socket should be closed.
    recent_messages_sent: AtomicBool,
    recent_messages_received: AtomicBool,
}

impl MixedSocket {
    #[inline]
    pub fn new(upstream_socket: SocketAddr) -> Arc<Self> {
        Arc::new(MixedSocket {
            udp_retransmit: Duration::from_millis(UDP_RETRANSMIT_MS),
            udp_timeout_count: AtomicU8::new(0),
            udp: RwLock::new(UdpState::None),

            tcp_timeout: Duration::from_millis(TCP_TIMEOUT_MS),
            tcp: RwLock::new(TcpState::None),

            upstream_socket,
            in_flight: RwLock::new(HashMap::new()),

            recent_messages_sent: AtomicBool::new(false),
            recent_messages_received: AtomicBool::new(false),
        })
    }

    #[inline]
    pub fn recent_messages_sent_or_received(&self) -> bool {
        self.recent_messages_sent.load(Ordering::Acquire)
        || self.recent_messages_received.load(Ordering::Acquire)
    }

    #[inline]
    pub fn recent_messages_sent_and_received(&self) -> (bool, bool) {
        (
            self.recent_messages_sent.load(Ordering::Acquire),
            self.recent_messages_received.load(Ordering::Acquire)
        )
    }

    #[inline]
    pub fn recent_messages_sent(&self) -> bool {
        self.recent_messages_sent.load(Ordering::Acquire)
    }

    #[inline]
    pub fn recent_messages_received(&self) -> bool {
        self.recent_messages_received.load(Ordering::Acquire)
    }

    #[inline]
    pub fn reset_recent_messages_sent_and_received(&self) -> (bool, bool) {
        (
            self.recent_messages_sent.swap(false, Ordering::AcqRel),
            self.recent_messages_received.swap(false, Ordering::AcqRel)
        )
    }

    #[inline]
    pub fn reset_recent_messages_sent(&self) -> bool {
        self.recent_messages_sent.swap(false, Ordering::AcqRel)
    }

    #[inline]
    pub fn reset_recent_messages_received(&self) -> bool {
        self.recent_messages_received.swap(false, Ordering::AcqRel)
    }

    #[inline]
    pub async fn start_both(self: Arc<Self>) -> io::Result<()> {
        match join!(
            self.clone().start_udp(),
            self.start_tcp()
        ) {
            (Ok(()), Ok(())) => Ok(()),
            (Ok(()), Err(tcp_error)) => Err(tcp_error),
            (Err(udp_error), Ok(())) => Err(udp_error),
            // FIXME: it is probably worth deciding on a method of returning both errors, since they
            //        may not be the same.
            (Err(udp_error), Err(_tcp_error)) => Err(udp_error),
        }
    }

    #[inline]
    pub async fn shutdown_both(self: Arc<Self>) -> io::Result<()> {
        match join!(
            self.clone().shutdown_udp(),
            self.shutdown_tcp()
        ) {
            (Ok(()), Ok(())) => Ok(()),
            (Ok(()), Err(tcp_error)) => Err(tcp_error),
            (Err(udp_error), Ok(())) => Err(udp_error),
            // FIXME: it is probably worth deciding on a method of returning both errors, since they
            //        may not be the same.
            (Err(udp_error), Err(_tcp_error)) => Err(udp_error),
        }
    }

    #[inline]
    pub async fn enable_both(self: Arc<Self>) -> io::Result<()> {
        match join!(
            self.clone().enable_udp(),
            self.enable_tcp()
        ) {
            (Ok(()), Ok(())) => Ok(()),
            (Ok(()), Err(tcp_error)) => Err(tcp_error),
            (Err(udp_error), Ok(())) => Err(udp_error),
            // FIXME: it is probably worth deciding on a method of returning both errors, since they
            //        may not be the same.
            (Err(udp_error), Err(_tcp_error)) => Err(udp_error),
        }
    }

    #[inline]
    pub async fn disable_both(self: Arc<Self>) -> io::Result<()> {
        match join!(
            self.clone().disable_udp(),
            self.disable_tcp()
        ) {
            (Ok(()), Ok(())) => Ok(()),
            (Ok(()), Err(tcp_error)) => Err(tcp_error),
            (Err(udp_error), Ok(())) => Err(udp_error),
            // FIXME: it is probably worth deciding on a method of returning both errors, since they
            //        may not be the same.
            (Err(udp_error), Err(_tcp_error)) => Err(udp_error),
        }
    }

    #[inline]
    async fn cleanup_query(self: Arc<Self>, query_id: u16) {
        let mut w_in_flight = self.in_flight.write().await;
        // Removing the message will cause the sender to be dropped. If there
        // was no response, tasks still awaiting a response will receive an error.
        w_in_flight.remove(&query_id);
        drop(w_in_flight);
    }

    pub async fn query(self: Arc<Self>, query: Message, options: QueryOptions) -> io::Result<Message> {
        let self_lock_1 = self.clone();
        let self_lock_2 = self.clone();
        let (r_udp, r_tcp) = join!(
            self_lock_1.udp.read(),
            self_lock_2.tcp.read()
        );
        let udp_timeout_count = self.udp_timeout_count.load(Ordering::Acquire);
        let mut receiver = match (options, udp_timeout_count, &*r_udp, &*r_tcp) {
            (QueryOptions::Both, 0..=3, UdpState::None | UdpState::Managed(_, _), _) => {
                drop(r_tcp);
                self.query_udp_rsocket(r_udp, query).await?
            },
            // Too many UDP timeouts, no TCP socket has been established.
            (QueryOptions::Both, 4.., UdpState::None | UdpState::Managed(_, _), TcpState::None) => {
                drop(r_tcp);
                // It will query via UDP but will start setting up a TCP connection to fall back on.
                task::spawn(self.clone().init_tcp());
                self.query_udp_rsocket(r_udp, query).await?
            },

            // Only TCP is allowed
            (QueryOptions::TcpOnly, _, _, TcpState::None | TcpState::Establishing { sender: _, kill: _ } | TcpState::Managed { socket: _, kill: _ }) => {
                drop(r_udp);
                self.query_tcp_rsocket(r_tcp, query).await?
            },
            // Too many UDP timeouts, a TCP socket is still being setup or already exists.
            (QueryOptions::Both, 4.., _, TcpState::Establishing { sender: _, kill: _ } | TcpState::Managed { socket: _, kill: _ }) => {
                drop(r_udp);
                self.query_tcp_rsocket(r_tcp, query).await?
            },

            // Cases where one or both of the sockets are blocked.
            (QueryOptions::TcpOnly, _, _, TcpState::Blocked) => {
                drop(r_tcp);
                return Err(io::Error::from(io::ErrorKind::ConnectionAborted));
            },
            (_, _, UdpState::Blocked, TcpState::Blocked) => {
                drop(r_tcp);
                return Err(io::Error::from(io::ErrorKind::ConnectionAborted));
            },
            (QueryOptions::Both, 4.., UdpState::None | UdpState::Managed(_, _), TcpState::Blocked) => {
                drop(r_tcp);
                self.query_udp_rsocket(r_udp, query).await?
            },
            (QueryOptions::Both, _, UdpState::Blocked, TcpState::None | TcpState::Establishing { sender: _, kill: _ } | TcpState::Managed { socket: _, kill: _ }) => {
                drop(r_udp);
                self.query_tcp_rsocket(r_tcp, query).await?
            },
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
    let mut wire = ReadWire::from_bytes(&buffer[..received_byte_count]);
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

#[cfg(test)]
mod mixed_udp_tcp_tests {
    use std::{net::{IpAddr, Ipv4Addr, SocketAddr}, time::Duration};

    use dns_lib::{query::{message::Message, qr::QR, question::Question}, resource_record::{opcode::OpCode, rclass::RClass, rcode::RCode, resource_record::{RRHeader, ResourceRecord}, rtype::RType, time::Time, types::a::A}, serde::wire::{from_wire::FromWire, read_wire::ReadWire, to_wire::ToWire}, types::c_domain_name::CDomainName};
    use tinyvec::TinyVec;
    use tokio::{io::AsyncReadExt, select};
    use ux::u3;

    use crate::mixed_tcp_udp::{MixedSocket, QueryOptions};

    const LISTEN_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 65000);
    const SEND_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 65000);

    #[tokio::test(flavor = "multi_thread")]
    async fn udp_manager_no_responses() {
        // Setup
        let listen_udp_socket = tokio::net::UdpSocket::bind(LISTEN_ADDR).await.unwrap();
        let listen_tcp_socket = tokio::net::TcpListener::bind(LISTEN_ADDR).await.unwrap();

        let example_domain = CDomainName::from_utf8("example.org.").unwrap();
        let example_class = RClass::Internet;

        let question = Question::new(
            example_domain.clone(),
            RType::A,
            RClass::Internet
        );
        let answer = ResourceRecord::A(
            RRHeader::new(example_domain, example_class, Time::from_secs(3600)),
            A::new(Ipv4Addr::LOCALHOST),
        );
        let query = Message {
            id: 42,
            qr: QR::Query,
            opcode: OpCode::Query,
            authoritative_answer: false,
            truncation: false,
            recursion_desired: false,
            recursion_available: false,
            z: u3::new(0),
            rcode: RCode::NoError,
            question: TinyVec::from([question.clone()]),
            answer: vec![],
            authority: vec![],
            additional: vec![],
        };
        let response = Message {
            id: 42,
            qr: QR::Response,
            opcode: OpCode::Query,
            authoritative_answer: false,
            truncation: false,
            recursion_desired: false,
            recursion_available: false,
            z: u3::new(0),
            rcode: RCode::NoError,
            question: TinyVec::from([question.clone()]),
            answer: vec![answer],
            authority: vec![],
            additional: vec![],
        };

        let mixed_socket = MixedSocket::new(SEND_ADDR);

        // Test: Start Query
        let query_task = tokio::spawn(mixed_socket.clone().query(query.clone(), QueryOptions::Both));

        // Test: Receiver first query (no response + no TCP)
        let mut buffer = [0_u8; 512];
        let bytes_read = select! {
            bytes_read = listen_udp_socket.recv(&mut buffer) => bytes_read,
            () = tokio::time::sleep(Duration::from_secs(1)) => {
                panic!("Did not receive first message in time.")
            },
        };
        assert!(bytes_read.is_ok());
        let bytes_read = bytes_read.unwrap();
        assert!(bytes_read <= query.serial_length() as usize);

        let mut wire = ReadWire::from_bytes(&buffer[..(bytes_read as usize)]);
        let actual_query = Message::from_wire_format(&mut wire);
        assert!(actual_query.is_ok());
        let actual_query = actual_query.unwrap();
        let mut expected_query = query.clone();
        expected_query.id = actual_query.id;
        assert_eq!(actual_query, expected_query);

        tokio::time::sleep(Duration::from_millis(125)).await;

        // Test: Receiver second query (no response)
        let mut buffer = [0_u8; 512];
        let bytes_read = select! {
            bytes_read = listen_udp_socket.recv(&mut buffer) => bytes_read,
            () = tokio::time::sleep(Duration::from_secs(1)) => {
                panic!("Did not receive second message in time.")
            },
        };
        assert!(bytes_read.is_ok());
        let bytes_read = bytes_read.unwrap();
        assert!(bytes_read <= query.serial_length() as usize);

        let mut wire = ReadWire::from_bytes(&buffer[..(bytes_read as usize)]);
        let actual_query = Message::from_wire_format(&mut wire);
        assert!(actual_query.is_ok());
        let actual_query = actual_query.unwrap();
        assert_eq!(actual_query, expected_query);   //< no ID change allowed for same query

        // Test: TCP Connection is requested
        let tcp_receiver = select! {
            tcp_receiver = listen_tcp_socket.accept() => tcp_receiver,
            () = tokio::time::sleep(Duration::from_secs(1)) => {
                panic!("Did not receive TCP connection request in time.")
            },
        };
        assert!(tcp_receiver.is_ok());
        let mut tcp_receiver = tcp_receiver.unwrap().0;

        // Test: TCP request is not yet made
        let mut buffer = [0_u8; 512];
        let bytes_read = tcp_receiver.try_read(&mut buffer);
        assert!(bytes_read.is_err());

        tokio::time::sleep(Duration::from_millis(125)).await;

        // Test: TCP request
        let mut buffer = [0_u8; 2];
        let bytes_read = select! {
            bytes_read = tcp_receiver.read_exact(&mut buffer) => bytes_read,
            () = tokio::time::sleep(Duration::from_secs(1)) => {
                panic!("Did not receive third message in time (size bytes).")
            },
        };
        assert!(bytes_read.is_ok());
        let bytes_read = bytes_read.unwrap();
        assert_eq!(bytes_read, 2);
        let expected_bytes = u16::from_be_bytes(buffer);
        assert!(expected_bytes <= query.serial_length());

        let mut buffer = [0_u8; 512];
        let bytes_read = select! {
            bytes_read = tcp_receiver.read_exact(&mut buffer[..(expected_bytes as usize)]) => bytes_read,
            () = tokio::time::sleep(Duration::from_secs(1)) => {
                panic!("Did not receive third message in time (message bytes).")
            },
        };
        assert!(bytes_read.is_ok());
        let bytes_read = bytes_read.unwrap();
        assert_eq!(bytes_read, expected_bytes as usize);

        let mut wire = ReadWire::from_bytes(&buffer[..(expected_bytes as usize)]);
        let actual_query = Message::from_wire_format(&mut wire);
        assert!(actual_query.is_ok());
        let actual_query = actual_query.unwrap();
        assert_eq!(actual_query, expected_query);   //< no ID change allowed for same query

        // Test: Client connection failed
        let query_task_response = query_task.await;
        assert!(query_task_response.is_ok());   //< JoinError
        let query_task_response = query_task_response.unwrap();
        assert!(query_task_response.is_err());   //< io error

        // Cleanup
        assert!(mixed_socket.disable_both().await.is_ok());
    }
}

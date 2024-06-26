use std::{collections::HashMap, io::ErrorKind, net::SocketAddr, sync::{atomic::{AtomicBool, AtomicU8, Ordering}, Arc}, time::Duration};

use async_lib::awake_token::AwakeToken;
use async_recursion::async_recursion;
use dns_lib::{query::message::Message, serde::wire::{compression_map::CompressionMap, from_wire::FromWire, read_wire::ReadWire, to_wire::ToWire, write_wire::WriteWire}, types::{base64::Base64, base_conversions::BaseConversions}};
use socket2::{SockRef, TcpKeepalive};
use tokio::{io::{self, AsyncReadExt, AsyncWriteExt}, join, net::{tcp::{OwnedReadHalf, OwnedWriteHalf}, TcpStream, UdpSocket}, pin, select, sync::{broadcast, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard}, task, time};

const MAX_MESSAGE_SIZE: usize = 8192;
const UDP_RETRANSMIT_MS: u64 = 125;
const TCP_TIMEOUT_MS: u64 = 500;


pub enum QueryOptions {
    TcpOnly,
    Both,
}

struct InFlight { send_response: broadcast::Sender<Message> }

enum TcpState {
    Managed(Arc<Mutex<OwnedWriteHalf>>, Arc<AwakeToken>),
    Establishing(broadcast::Sender<(Arc<Mutex<OwnedWriteHalf>>, Arc<AwakeToken>)>, Arc<AwakeToken>),
    None,
    Blocked,
}

enum UdpState {
    Managed(Arc<UdpSocket>, Arc<AwakeToken>),
    None,
    Blocked,
}

/// The shared mutable state for the UDP socket. This struct is stored behind a lock.
struct SharedUdp { state: UdpState }

/// The shared mutable state for the TCP socket. This struct is stored behind a lock.
struct SharedTcp { state: TcpState }

pub struct MixedSocket {
    udp_retransmit: Duration,
    udp_timeout_count: AtomicU8,
    udp_shared: RwLock<SharedUdp>,

    tcp_timeout: Duration,
    tcp_shared: RwLock<SharedTcp>,

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
            udp_shared: RwLock::new(SharedUdp { state: UdpState::None }),

            tcp_timeout: Duration::from_millis(TCP_TIMEOUT_MS),
            tcp_shared: RwLock::new(SharedTcp { state: TcpState::None }),

            upstream_socket,
            in_flight: RwLock::new(HashMap::new()),

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
    async fn listen_udp(self: Arc<Self>, udp_reader: Arc<UdpSocket>, udp_kill: Arc<AwakeToken>) {
        pin!(let udp_kill_awoken = udp_kill.awoken(););
        loop {
            select! {
                biased;
                _ = &mut udp_kill_awoken => {
                    println!("UDP Socket {} Canceled. Shutting down UDP Listener.", self.upstream_socket);
                    break;
                },
                response = read_udp_message(udp_reader.clone()) => {
                    match response {
                        Ok(response) => {
                            // Note: if truncation flag is set, that will be dealt with by the caller.
                            self.recent_messages_received.store(true, Ordering::SeqCst);
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

        let mut w_udp = self.udp_shared.write().await;
        match &w_udp.state {
            UdpState::Managed(_, udp_kill) => {
                let udp_kill = udp_kill.clone();
                w_udp.state = UdpState::None;
                drop(w_udp);

                udp_kill.awake();
            },
            UdpState::None => (),
            UdpState::Blocked => (),
        }
    }

    #[inline]
    async fn listen_tcp(self: Arc<Self>, mut tcp_reader: OwnedReadHalf, tcp_kill: Arc<AwakeToken>) {
        pin!(let tcp_kill_awoken = tcp_kill.awoken(););
        loop {
            select! {
                biased;
                _ = &mut tcp_kill_awoken => {
                    println!("TCP Socket {} Canceled. Shutting down TCP Listener.", self.upstream_socket);
                    break;
                },
                response = read_tcp_message(&mut tcp_reader) => {
                    match response {
                        Ok(response) => {
                            self.recent_messages_received.store(true, Ordering::SeqCst);
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

        self.listen_tcp_cleanup().await;
    }

    #[inline]
    async fn listen_tcp_cleanup(self: Arc<Self>) {
        println!("Cleaning up TCP socket {}", self.upstream_socket);

        let mut w_tcp = self.tcp_shared.write().await;
        match &w_tcp.state {
            TcpState::Managed(_, tcp_kill) => {
                let tcp_kill = tcp_kill.clone();
                w_tcp.state = TcpState::None;
                drop(w_tcp);

                tcp_kill.awake();
            },
            TcpState::Establishing(_, _) => panic!("TCP listener exists but TcpState is TcpState::Establishing"),
            TcpState::None => (),
            TcpState::Blocked => (),
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
    pub async fn start_tcp(self: Arc<Self>) -> io::Result<()> {
        match self.init_tcp().await {
            Ok(_) => Ok(()),
            Err(error) => Err(error),
        }
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
    pub async fn shutdown_udp(self: Arc<Self>) -> io::Result<()> {
        let r_udp = self.udp_shared.read().await;
        if let UdpState::Managed(udp_socket, udp_kill) = &r_udp.state {
            let udp_socket = udp_socket.clone();
            let udp_kill = udp_kill.clone();
            drop(r_udp);
            
            println!("Shutting down UDP socket {}", self.upstream_socket);
            SockRef::from(udp_socket.as_ref()).shutdown(std::net::Shutdown::Both)?;

            udp_kill.awake();

            // Note: this task is not responsible for actual cleanup. Once the listener closes, it
            // will kill any active queries and change the UdpState.
        } else {
            drop(r_udp);
        }
        Ok(())
    }

    #[inline]
    pub async fn shutdown_tcp(self: Arc<Self>) -> io::Result<()> {
        let r_tcp = self.tcp_shared.read().await;
        match &r_tcp.state {
            TcpState::Managed(tcp_socket, tcp_kill) => {
                let tcp_socket = tcp_socket.clone();
                let tcp_kill = tcp_kill.clone();
                drop(r_tcp);
                
                println!("Shutting down TCP socket {}", self.upstream_socket);
                let w_tcp_socket = tcp_socket.lock().await;
                SockRef::from(w_tcp_socket.as_ref()).shutdown(std::net::Shutdown::Both)?;
                drop(w_tcp_socket);
    
                tcp_kill.awake();
    
                // Note: this task is not responsible for actual cleanup. Once the listener closes, it
                // will kill any active queries and change the TcpState.
            },
            TcpState::Establishing(sender, tcp_init_kill) => {
                let mut receiver = sender.subscribe();
                let tcp_init_kill = tcp_init_kill.clone();
                drop(r_tcp);

                // Try to prevent the socket from being initialized.
                tcp_init_kill.awake();

                // If the socket still initialized, shut it down immediately.
                match receiver.recv().await {
                    Ok((tcp_socket, tcp_kill)) => {
                        println!("Shutting down TCP socket {}", self.upstream_socket);
                        let w_tcp_socket = tcp_socket.lock().await;
                        SockRef::from(w_tcp_socket.as_ref()).shutdown(std::net::Shutdown::Both)?;
                        drop(w_tcp_socket);

                        tcp_kill.awake();
                    },
                    Err(_) => (),   //< Nothing to do, no connection established.
                }
            },
            TcpState::None => drop(r_tcp),
            TcpState::Blocked => drop(r_tcp),
        }
        Ok(())
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
    pub async fn disable_udp(self: Arc<Self>) -> io::Result<()> {
        println!("Disabling UDP socket {}", self.upstream_socket);
        
        let mut w_udp = self.udp_shared.write().await;
        match &w_udp.state {
            UdpState::Managed(udp_socket, udp_kill) => {
                // Since we are removing the reference the udp_kill by setting state to Blocked, we
                // need to kill them now since the listener won't be able to kill them.
                let udp_kill = udp_kill.clone();
                let udp_socket = udp_socket.clone();
                w_udp.state = UdpState::Blocked;
                drop(w_udp);

                println!("Shutting down UDP socket {}", self.upstream_socket);
                SockRef::from(udp_socket.as_ref()).shutdown(std::net::Shutdown::Both)?;

                udp_kill.awake();

                Ok(())
            },
            UdpState::None => {
                w_udp.state = UdpState::Blocked;
                drop(w_udp);
                Ok(())
            },
            UdpState::Blocked => { //< Already disabled
                drop(w_udp);
                Ok(())
            },
        }
    }

    #[inline]
    #[async_recursion]
    pub async fn disable_tcp(self: Arc<Self>) -> io::Result<()> {
        println!("Disabling TCP socket {}", self.upstream_socket);
        
        let mut w_tcp = self.tcp_shared.write().await;
        match &w_tcp.state {
            TcpState::Managed(tcp_socket, tcp_kill) => {
                // Since we are removing the reference the tcp_kill by setting state to Blocked, we
                // need to kill them now since the listener won't be able to kill them.
                let tcp_kill = tcp_kill.clone();
                let tcp_socket = tcp_socket.clone();
                w_tcp.state = TcpState::Blocked;
                drop(w_tcp);

                println!("Shutting down TCP socket {}", self.upstream_socket);
                let w_tcp_socket = tcp_socket.lock().await;
                SockRef::from(w_tcp_socket.as_ref()).shutdown(std::net::Shutdown::Both)?;
                drop(w_tcp_socket);

                tcp_kill.awake();

                Ok(())
            },
            TcpState::Establishing(sender, tcp_init_kill) => {
                let mut receiver = sender.subscribe();
                let tcp_init_kill = tcp_init_kill.clone();
                drop(w_tcp);

                // Try to prevent the socket from being initialized.
                tcp_init_kill.awake();

                // If the socket still initialized, shut it down immediately.
                match receiver.recv().await {
                    Ok((tcp_socket, tcp_kill)) => {
                        println!("Shutting down TCP socket {}", self.upstream_socket);
                        let w_tcp_socket = tcp_socket.lock().await;
                        SockRef::from(w_tcp_socket.as_ref()).shutdown(std::net::Shutdown::Both)?;
                        drop(w_tcp_socket);

                        tcp_kill.awake();
                    },
                    Err(_) => (),   //< Nothing to do, no connection established.
                }

                self.disable_tcp().await
            },
            TcpState::None => {
                w_tcp.state = TcpState::Blocked;
                drop(w_tcp);
                Ok(())
            },
            TcpState::Blocked => { //< Already disabled
                drop(w_tcp);
                Ok(())
            },
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
    pub async fn enable_udp(self: Arc<Self>) -> io::Result<()> {
        println!("Enabling UDP socket {}", self.upstream_socket);
        
        let mut w_udp = self.udp_shared.write().await;
        match &w_udp.state {
            UdpState::Managed(_, _) => (),  //< Already enabled
            UdpState::None => (),           //< Already enabled
            UdpState::Blocked => w_udp.state = UdpState::None,
        }
        drop(w_udp);
        return Ok(());
    }

    #[inline]
    pub async fn enable_tcp(self: Arc<Self>) -> io::Result<()> {
        println!("Enabling TCP socket {}", self.upstream_socket);
        
        let mut w_tcp = self.tcp_shared.write().await;
        match &w_tcp.state {
            TcpState::Managed(_, _) => (),      //< Already enabled
            TcpState::Establishing(_, _) => (), //< Already enabled
            TcpState::None => (),               //< Already enabled
            TcpState::Blocked => w_tcp.state = TcpState::None,
        }
        drop(w_tcp);
        return Ok(());
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
    async fn init_udp(self: Arc<Self>) -> io::Result<(Arc<UdpSocket>, Arc<AwakeToken>)> {
        // Initially, verify if the connection has already been established.
        let r_udp = self.udp_shared.read().await;
        match &r_udp.state {
            UdpState::Managed(udp_socket, udp_kill) => return Ok((udp_socket.clone(), udp_kill.clone())),
            UdpState::None => (),
            UdpState::Blocked => {
                drop(r_udp);
                return Err(io::Error::from(io::ErrorKind::ConnectionAborted));
            },
        }
        drop(r_udp);

        let udp_socket = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);
        udp_socket.connect(self.upstream_socket).await?;
        let udp_reader = udp_socket.clone();
        let udp_writer = udp_socket;
        let udp_kill = Arc::new(AwakeToken::new());

        // Since there is no intermediate state while the UDP socket is being
        // set up and the lock is dropped, it is possible that another process
        // was doing the same task.

        let mut w_udp = self.udp_shared.write().await;
        match &w_udp.state {
            UdpState::Managed(existing_udp_socket, _) => {
                return Ok((existing_udp_socket.clone(), udp_kill.clone()));
            },
            UdpState::None => {
                w_udp.state = UdpState::Managed(udp_writer.clone(), udp_kill.clone());
                drop(w_udp);
                task::spawn(self.clone().listen_udp(udp_reader, udp_kill.clone()));
            },
            UdpState::Blocked => {
                drop(w_udp);
                return Err(io::Error::from(io::ErrorKind::ConnectionAborted));
            },
        }

        return Ok((udp_writer, udp_kill));
    }

    #[inline]
    async fn init_tcp(self: Arc<Self>) -> io::Result<(Arc<Mutex<OwnedWriteHalf>>, Arc<AwakeToken>)> {
        // Initially, verify if the connection has already been established.
        let r_tcp = self.tcp_shared.read().await;
        match &r_tcp.state {
            TcpState::Managed(tcp_socket, udp_kill) => return Ok((tcp_socket.clone(), udp_kill.clone())),
            TcpState::Establishing(sender, _) => {
                let mut receiver = sender.subscribe();
                drop(r_tcp);
                match receiver.recv().await {
                    Ok((tcp_socket, udp_kill)) => return Ok((tcp_socket.clone(), udp_kill.clone())),
                    Err(_) => return Err(io::Error::from(io::ErrorKind::Interrupted)),
                }
            },
            TcpState::None => (),
            TcpState::Blocked => {
                drop(r_tcp);
                return Err(io::Error::from(io::ErrorKind::ConnectionAborted));
            },
        }
        drop(r_tcp);

        // Setup for once the write lock is obtained.
        let (tcp_socket_sender, _) = broadcast::channel(1);
        let tcp_init_kill = Arc::new(AwakeToken::new());

        // Need to re-verify state with new lock. State could have changed in between.
        let mut w_tcp = self.tcp_shared.write().await;
        match &w_tcp.state {
            TcpState::Managed(tcp_socket, udp_kill) => return Ok((tcp_socket.clone(), udp_kill.clone())),
            TcpState::Establishing(sender, _) => {
                let mut receiver = sender.subscribe();
                drop(w_tcp);
                match receiver.recv().await {
                    Ok((tcp_socket, udp_kill)) => return Ok((tcp_socket.clone(), udp_kill.clone())),
                    Err(_) => return Err(io::Error::from(io::ErrorKind::Interrupted)),
                }
            },
            TcpState::None => (),
            TcpState::Blocked => {
                drop(w_tcp);
                return Err(io::Error::from(io::ErrorKind::ConnectionAborted));
            },
        }

        w_tcp.state = TcpState::Establishing(tcp_socket_sender.clone(), tcp_init_kill.clone());
        drop(w_tcp);
        println!("Initializing TCP connection to {}", self.upstream_socket);

        // Since state has been set to Establishing, this process is now fully
        // in charge of establishing the TCP connection. Next time the write
        // lock is obtained, it won't need to check the state.

        let tcp_socket = select! {
            _ = tcp_init_kill.clone().awoken() => {
                eprintln!("Failed to establish TCP connection to {} (Canceled)", self.upstream_socket);
                // Notify all of the waiters by dropping the sender. This
                // causes the receivers to receiver an error.
                drop(tcp_socket_sender);
                return Err(io::Error::from(io::ErrorKind::ConnectionAborted))
            },
            tcp_socket = TcpStream::connect(self.upstream_socket) => match tcp_socket {
                Ok(tcp_socket) => tcp_socket,
                Err(error) => {
                    eprintln!("Failed to establish TCP connection to {} ({error})", self.upstream_socket);
    
                    // Before returning, we must ensure that the "Establishing" status gets cleared
                    // since we failed to establish the connection.
                    let mut w_tcp = self.tcp_shared.write().await;
                    w_tcp.state = TcpState::None;
                    drop(w_tcp);
    
                    // Notify all of the waiters by dropping the sender. This
                    // causes the receivers to receiver an error.
    
                    // It might be worth adding another state that blocks future TCP connections.
                    drop(tcp_socket_sender);
                    return Err(error);
                },
            },
        };

        // Keep-alive configs
        let keep_alive = TcpKeepalive::new()
            .with_time(Duration::from_secs(2))
            .with_interval(Duration::from_secs(2))
            .with_retries(4);
        if let Err(_) = SockRef::from(&tcp_socket).set_tcp_keepalive(&keep_alive) {
            // This is not a fatal error, but it is not ideal either.
            eprintln!("WARNING: Failed to establish the keepalive settings for TCP connection to {}", self.upstream_socket)
        };
        let (tcp_reader, tcp_writer) = tcp_socket.into_split();
        let tcp_writer = Arc::new(Mutex::new(tcp_writer));
        let tcp_kill = tcp_init_kill;

        let mut w_tcp = self.tcp_shared.write().await;
        w_tcp.state = TcpState::Managed(tcp_writer.clone(), tcp_kill.clone());
        drop(w_tcp);

        task::spawn(self.clone().listen_tcp(tcp_reader, tcp_kill.clone()));

        let _ = tcp_socket_sender.send((tcp_writer.clone(), tcp_kill.clone()));

        return Ok((tcp_writer, tcp_kill));
    }

    #[inline]
    async fn cleanup_query(self: Arc<Self>, query_id: u16) {
        let mut w_in_flight = self.in_flight.write().await;
        // Removing the message will cause the sender to be dropped. If there
        // was no response, tasks still awaiting a response will receive an error.
        w_in_flight.remove(&query_id);
        drop(w_in_flight);
    }

    #[inline]
    async fn manage_udp_query(self: Arc<Self>, udp_socket: Arc<UdpSocket>, udp_kill: Arc<AwakeToken>, in_flight_sender: broadcast::Sender<Message>, query: Message) {
        let mut in_flight_receiver = in_flight_sender.subscribe();
        drop(in_flight_sender);
        let query_id = query.id;

        pin!{let udp_canceled = udp_kill.clone().awoken();}

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
            _ = &mut udp_canceled => {
                println!("UDP Cleanup: UDP canceled while waiting to receive message with ID {}", query_id);
                return self.cleanup_query(query_id).await;
            },
            _ = time::sleep(self.udp_retransmit) => {
                println!("UDP Timeout: Retransmitting message with ID {} via UDP", query_id);
                task::spawn(self.clone().retransmit_query_udp(udp_socket, query.clone()));
                // Also start the process of setting up a TCP connection. This
                // way, by the time we timeout a second time (if we do, at
                // least), there is a TCP connection ready to go.
                task::spawn(self.clone().init_tcp());
            },
        }

        // Timeout Case 2: resend with TCP
        let tcp_kill = select! {
            biased;
            response = in_flight_receiver.recv() => {
                match response {
                    Ok(_) => println!("UDP Cleanup: Response received with ID {}", query_id),
                    Err(broadcast::error::RecvError::Closed) => println!("UDP Cleanup: Channel closed for query with ID {}", query_id),
                    Err(broadcast::error::RecvError::Lagged(skipped_messages)) => println!("UDP Cleanup: Channel lagged for query with ID {}, skipping {skipped_messages} messages", query_id),
                }
                return self.cleanup_query(query_id).await;
            },
            _ = &mut udp_canceled => {
                println!("UDP Cleanup: UDP canceled while waiting to receive message with ID {}", query_id);
                return self.cleanup_query(query_id).await;
            },
            // Although it looks gross, the nested select statement here helps prevent responses
            // from being missed. Unlike the other cases where we can offload the heavy work to
            // other tasks, we need the result here so this seems to be the best way to do it.
            // Note that we do still offload the tcp initialization to another task so that it is
            // cancel-safe, but we then await the join handle.
            _ = time::sleep(self.udp_retransmit) => select! {
                biased;
                response = in_flight_receiver.recv() => {
                    match response {
                        Ok(_) => println!("UDP Cleanup: Response received with ID {}", query_id),
                        Err(broadcast::error::RecvError::Closed) => println!("UDP Cleanup: Channel closed for query with ID {}", query_id),
                        Err(broadcast::error::RecvError::Lagged(skipped_messages)) => println!("UDP Cleanup: Channel lagged for query with ID {}, skipping {skipped_messages} messages", query_id),
                    }
                    return self.cleanup_query(query_id).await;
                },
                _ = &mut udp_canceled => {
                    println!("UDP Cleanup: UDP canceled while waiting to receive message with ID {}", query_id);
                    return self.cleanup_query(query_id).await;
                },
                result = task::spawn(self.clone().init_tcp()) => {
                    match result {
                        Ok(Ok((tcp_writer, tcp_kill))) => {
                            println!("UDP Timeout: Retransmitting message with ID {} via TCP", query_id);
                            task::spawn(self.clone().retransmit_query_tcp(tcp_writer, query));
                            tcp_kill
                        },
                        Ok(Err(error)) => {
                            eprintln!("UDP Timeout: Unable to retransmit via TCP; {error}");
                            // If we cannot re-transmit with TCP, then we are still waiting on UDP. So,
                            // we are still actually interested in the UDP kill token since that's the
                            // socket that is going to give us our answer.
                            udp_kill
                        },
                        Err(join_error) => {
                            eprintln!("UDP Timeout: Unable to retransmit via TCP; {join_error}");
                            // If we cannot re-transmit with TCP, then we are still waiting on UDP. So,
                            // we are still actually interested in the UDP kill token since that's the
                            // socket that is going to give us our answer.
                            udp_kill
                        }
                    }
                },
                _ = time::sleep(self.tcp_timeout) => {
                    println!("UDP Timeout: Took too long to establish TCP connection to receive message with ID {}", query_id);
                    return self.cleanup_query(query_id).await;
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
                }
            },
            _ = tcp_kill.awoken() => println!("UDP Cleanup: TCP canceled while waiting to receive message with ID {}", query_id),
            _ = time::sleep(self.tcp_timeout) => println!("UDP Timeout: TCP query with ID {} took too long to respond", query_id),
            // Note: we don't want to await UDP kill anymore. As far as we are concerned, we have
            // transitioned into a TCP manager.
        }
        self.cleanup_query(query_id).await;
    }

    #[inline]
    async fn query_udp_rsocket<'a>(self: Arc<Self>, r_udp: RwLockReadGuard<'a, SharedUdp>, query: Message) -> io::Result<broadcast::Receiver<Message>> {
        let udp_socket;
        let udp_kill;
        match &r_udp.state {
            UdpState::Managed(state_udp_socket, state_udp_kill) => {
                udp_socket = state_udp_socket.clone();
                udp_kill = state_udp_kill.clone();
                drop(r_udp);
            },
            UdpState::None => {
                drop(r_udp);
                return self.clone().query_udp_wsocket(self.udp_shared.write().await, query).await;
            },
            UdpState::Blocked => {
                drop(r_udp);
                return Err(io::Error::from(io::ErrorKind::ConnectionAborted));
            },
        }
        self.query_udp(udp_socket.clone(), udp_kill, query).await
    }

    #[inline]
    async fn query_udp_wsocket<'a, 'b>(self: Arc<Self>, w_udp: RwLockWriteGuard<'b, SharedUdp>, query: Message) -> io::Result<broadcast::Receiver<Message>> where 'a: 'b {
        let udp_socket;
        let udp_kill;
        match &w_udp.state {
            UdpState::Managed(state_udp_socket, state_udp_kill) => {
                udp_socket = state_udp_socket.clone();
                udp_kill = state_udp_kill.clone();
                drop(w_udp);
            },
            UdpState::None => {
                drop(w_udp);
                (udp_socket, udp_kill) = self.clone().init_udp().await?;
            },
            UdpState::Blocked => {
                drop(w_udp);
                return Err(io::Error::from(io::ErrorKind::ConnectionAborted));
            },
        }
        self.query_udp(udp_socket, udp_kill, query).await
    }

    #[inline]
    async fn query_udp(self: Arc<Self>, udp_socket: Arc<UdpSocket>, udp_kill: Arc<AwakeToken>, mut query: Message) -> io::Result<broadcast::Receiver<Message>> {
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
        task::spawn(self.clone().manage_udp_query(udp_socket.clone(), udp_kill, sender, query.clone()));

        // Step 4: Send the message via UDP.
        self.recent_messages_sent.store(true, Ordering::SeqCst);
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
        self.recent_messages_sent.store(true, Ordering::SeqCst);
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

    #[inline]
    async fn retransmit_query_tcp(self: Arc<Self>, tcp_socket: Arc<Mutex<OwnedWriteHalf>>, query: Message) -> io::Result<()> {
        // Step 1: Skip. We are resending, in_flight was setup for initial transmission.

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
        self.recent_messages_sent.store(true, Ordering::SeqCst);
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

    #[inline]
    async fn manage_tcp_query(self: Arc<Self>, tcp_kill: Arc<AwakeToken>, in_flight_sender: broadcast::Sender<Message>, query: Message) {
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
            _ = tcp_kill.awoken() => println!("TCP Cleanup: TCP canceled while waiting to receive message with ID {}", query.id),
            _ = time::sleep(self.tcp_timeout) => println!("TCP Timeout: TCP query with ID {} took too long to respond", query.id),
        }
        self.cleanup_query(query.id).await;
        return;
    }

    #[inline]
    async fn query_tcp_rsocket<'a>(self: Arc<Self>, r_tcp: RwLockReadGuard<'a, SharedTcp>, query: Message) -> io::Result<broadcast::Receiver<Message>> {
        match &r_tcp.state {
            TcpState::Managed(tcp_socket, tcp_kill) => {
                let tcp_socket = tcp_socket.clone();
                let tcp_kill = tcp_kill.clone();
                drop(r_tcp);
                return self.query_tcp(tcp_socket, tcp_kill, query).await;
            },
            TcpState::Establishing(tcp_socket_sender, _) => {
                let mut tcp_socket_receiver = tcp_socket_sender.subscribe();
                drop(r_tcp);
                match tcp_socket_receiver.recv().await {
                    Ok((tcp_socket, tcp_kill)) => return self.query_tcp(tcp_socket, tcp_kill, query).await,
                    Err(_) => Err(io::Error::from(ErrorKind::Interrupted)),
                }
            },
            TcpState::None => {
                drop(r_tcp);
                let (tcp_socket, tcp_kill) = self.clone().init_tcp().await?;
                return self.query_tcp(tcp_socket, tcp_kill, query).await;
            },
            TcpState::Blocked => {
                drop(r_tcp);
                return Err(io::Error::from(io::ErrorKind::ConnectionAborted));
            },
        }
    }

    // async fn query_tcp_wsocket<'a>(mut w_socket: RwLockWriteGuard<'a, Self>, self: Arc<Self>, query: Message) -> io::Result<broadcast::Receiver<Message>> {
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
        self.recent_messages_sent.store(true, Ordering::SeqCst);
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

    pub async fn query(self: Arc<Self>, query: Message, options: QueryOptions) -> io::Result<Message> {
        let self_lock_1 = self.clone();
        let self_lock_2 = self.clone();
        let (r_udp, r_tcp) = join!(
            self_lock_1.udp_shared.read(),
            self_lock_2.tcp_shared.read()
        );
        let udp_timeout_count = self.udp_timeout_count.load(Ordering::SeqCst);
        let mut receiver = match (options, udp_timeout_count, &r_udp.state, &r_tcp.state) {
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
            (QueryOptions::TcpOnly, _, _, TcpState::None | TcpState::Establishing(_, _) | TcpState::Managed(_, _)) => {
                drop(r_udp);
                self.query_tcp_rsocket(r_tcp, query).await?
            },
            // Too many UDP timeouts, a TCP socket is still being setup or already exists.
            (QueryOptions::Both, 4.., _, TcpState::Establishing(_, _) | TcpState::Managed(_, _)) => {
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
            (QueryOptions::Both, _, UdpState::Blocked, TcpState::None | TcpState::Establishing(_, _) | TcpState::Managed(_, _)) => {
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

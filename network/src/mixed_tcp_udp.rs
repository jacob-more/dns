use std::{borrow::{Borrow, BorrowMut}, collections::HashMap, future::Future, io::ErrorKind, net::SocketAddr, pin::Pin, sync::{atomic::{AtomicBool, AtomicU8, Ordering}, Arc}, task::Poll, time::Duration};

use async_lib::awake_token::{AwakeToken, AwokenToken};
use dns_lib::{query::{message::Message, question::Question}, serde::wire::{compression_map::CompressionMap, from_wire::FromWire, read_wire::ReadWire, to_wire::ToWire, write_wire::WriteWire}};
use futures::{future::BoxFuture, FutureExt};
use log::trace;
use pin_project::{pin_project, pinned_drop};
use socket2::SockRef;
use tokio::{io::{self, AsyncReadExt, AsyncWriteExt}, join, net::{tcp::{OwnedReadHalf, OwnedWriteHalf}, TcpStream, UdpSocket}, pin, select, sync::{broadcast::{self, error::RecvError}, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard}, task::{self, JoinHandle}, time::{self, Sleep}};

const MAX_MESSAGE_SIZE: usize = 8192;
const UDP_RETRANSMIT_MS: u64 = 125;
const TCP_TIMEOUT_MS: u64 = 500;


const TCP_INIT_TIMEOUT_MS: u64 = 5000;
const TCP_LISTEN_TIMEOUT_MS: u64 = 1000 * 60 * 2;

const UDP_LISTEN_TIMEOUT_MS: u64 = 1000 * 60 * 2;


pub enum QueryOptions {
    TcpOnly,
    Both,
}

struct InFlight { send_response: broadcast::Sender<Message> }


#[pin_project(project = QInFlightProj)]
enum QInFlight<'q, 'r, 's, 'u, 'v, QSend>
where
    'r: 'q,
    'v: 'u,
{
    Fresh,
    WriteInFlight(BoxFuture<'q, RwLockWriteGuard<'r, HashMap<u16, InFlight>>>),
    InFlight {
        result_receiver: BoxFuture<'s, Result<Message, RecvError>>,
        send_query: QSend,
    },
    RemoveInFlight {
        w_in_flight: BoxFuture<'u, RwLockWriteGuard<'v, HashMap<u16, InFlight>>>,
        response: Option<io::Result<Message>>,
    },
}

enum QSendQuery<'t> {
    Fresh,
    SendQuery(BoxFuture<'t, io::Result<()>>),
    Complete,
}

#[pin_project(project = TQSocketProj)]
enum TQSocket<'c, 'd, 'e>
where
    'd: 'c,
{
    GetTcpState(BoxFuture<'c, RwLockReadGuard<'d, TcpState>>),
    GetTcpEstablishing {
        receive_tcp_socket: BoxFuture<'e, Result<(Arc<Mutex<OwnedWriteHalf>>, Arc<AwakeToken>), RecvError>>,
    },
    InitTcp {
        #[pin]
        join_handle: JoinHandle<io::Result<(Arc<Mutex<OwnedWriteHalf>>, Arc<AwakeToken>)>>,
    },
    Acquired {
        tcp_socket: Arc<Mutex<OwnedWriteHalf>>,
        #[pin]
        kill_tcp: AwokenToken,
    },
    Closed,
}


#[derive(Debug)]
enum CleanupReason {
    Timeout,
    Killed,
    ConnectionError(io::Error),
}

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

#[pin_project(PinnedDrop)]
struct InitTcp<'a, 'b, 'c, 'd, 'e, 'f, 'k, 'l, 'm>
where
    'a: 'c + 'f + 'l
{
    socket: &'a Arc<MixedSocket>,
    kill_tcp_token: Arc<AwakeToken>,
    #[pin]
    kill_tcp: AwokenToken,
    tcp_socket_sender: broadcast::Sender<(Arc<Mutex<OwnedWriteHalf>>, Arc<AwakeToken>)>,
    #[pin]
    timeout: Sleep,
    inner: InnerInitTcp<'b, 'c, 'd, 'e, 'f, 'k, 'l, 'm>,
}

enum InnerInitTcp<'b, 'c, 'd, 'e, 'f, 'k, 'l, 'm>
where
    'c: 'b,
    'f: 'e,
    'l: 'k,
{
    Fresh,
    WriteEstablishing(BoxFuture<'b, RwLockWriteGuard<'c, TcpState>>),
    Connecting(BoxFuture<'d, io::Result<TcpStream>>),
    WriteNone {
        reason: CleanupReason,
        w_tcp_state: BoxFuture<'e, RwLockWriteGuard<'f, TcpState>>,
    },
    WriteManaged {
        w_tcp_state: BoxFuture<'k, RwLockWriteGuard<'l, TcpState>>,
        tcp_socket: Arc<Mutex<OwnedWriteHalf>>,
    },
    GetEstablishing {
        receive_tcp_socket: BoxFuture<'m, Result<(Arc<Mutex<OwnedWriteHalf>>, Arc<AwakeToken>), RecvError>>,
    },
    Complete,
}

impl<'a, 'b, 'c, 'd, 'e, 'f, 'k, 'l, 'm> InitTcp<'a, 'b, 'c, 'd, 'e, 'f, 'k, 'l, 'm> {
    pub fn new(socket: &'a Arc<MixedSocket>, timeout: Option<Duration>) -> Self {
        let kill_tcp_token = Arc::new(AwakeToken::new());
        let (tcp_socket_sender, _) = broadcast::channel(1);
        let timeout = timeout.unwrap_or(Duration::from_millis(TCP_INIT_TIMEOUT_MS));

        Self {
            socket,
            kill_tcp_token: kill_tcp_token.clone(),
            kill_tcp: kill_tcp_token.awoken(),
            tcp_socket_sender,
            timeout: tokio::time::sleep(timeout),
            inner: InnerInitTcp::Fresh,
        }
    }
}

impl<'a, 'b, 'c, 'd, 'e, 'f, 'k, 'l, 'm> Future for InitTcp<'a, 'b, 'c, 'd, 'e, 'f, 'k, 'l, 'm> {
    type Output = io::Result<(Arc<Mutex<OwnedWriteHalf>>, Arc<AwakeToken>)>;

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let mut this = self.as_mut().project();
        match this.inner.borrow() {
            InnerInitTcp::Fresh
          | InnerInitTcp::WriteEstablishing(_) => {
                if let Poll::Ready(()) = this.kill_tcp.as_mut().poll(cx) {
                    *this.inner = InnerInitTcp::Complete;

                    // Exit loop: query killed.
                    return Poll::Ready(Err(io::Error::from(io::ErrorKind::ConnectionAborted)));
                }

                if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                    *this.inner = InnerInitTcp::Complete;

                    // Exit loop: query timed out.
                    return Poll::Ready(Err(io::Error::from(io::ErrorKind::TimedOut)));
                }
            },
            InnerInitTcp::Connecting(_) => {
                if let Poll::Ready(()) = this.kill_tcp.as_mut().poll(cx) {
                    let w_tcp_state = this.socket.tcp.write().boxed();

                    *this.inner = InnerInitTcp::WriteNone { reason: CleanupReason::Timeout, w_tcp_state };

                    // First loop: poll the write lock.
                } else if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                    let w_tcp_state = this.socket.tcp.write().boxed();

                    *this.inner = InnerInitTcp::WriteNone { reason: CleanupReason::Killed, w_tcp_state };

                    // First loop: poll the write lock.
                }
            },
            InnerInitTcp::GetEstablishing { receive_tcp_socket: _ } => {
                // Does not poll `kill_tcp` because that gets awoken to kill
                // the listener (if it is set up).
                if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                    *this.inner = InnerInitTcp::Complete;

                    // Exit loop: query timed out.
                    return Poll::Ready(Err(io::Error::from(io::ErrorKind::TimedOut)));
                }
            },
            InnerInitTcp::WriteNone { reason: _, w_tcp_state: _ }
          | InnerInitTcp::WriteManaged { w_tcp_state: _, tcp_socket: _ }
          | InnerInitTcp::Complete => {
                // Not allowed to timeout or be killed. These are cleanup
                // states.
            },
        }

        loop {
            match this.inner.borrow_mut() {
                InnerInitTcp::Fresh => {
                    let w_tcp_state = this.socket.tcp.write().boxed();

                    *this.inner = InnerInitTcp::WriteEstablishing(w_tcp_state);

                    // Next loop: poll the write lock to get the TCP state
                    continue;
                }
                InnerInitTcp::WriteEstablishing(w_tcp_state) => {
                    match w_tcp_state.as_mut().poll(cx) {
                        Poll::Ready(mut tcp_state) => {
                            match &*tcp_state {
                                TcpState::Managed { socket, kill } => {
                                    let tcp_socket = socket.clone();
                                    let kill_tcp_token = kill.clone();

                                    // Ignore send errors. They just indicate that all receivers have been dropped.
                                    let _ = this.tcp_socket_sender.send((tcp_socket.clone(), kill_tcp_token.clone()));

                                    *this.inner = InnerInitTcp::Complete;

                                    // Exit loop: connection already setup.
                                    // Nothing to do.
                                    return Poll::Ready(Ok((tcp_socket, kill_tcp_token)));
                                },
                                TcpState::Establishing { sender: active_sender, kill: _ } => {
                                    let receive_tcp_socket = active_sender.subscribe();
                                    let receive_tcp_socket = async move {
                                        let mut receive_tcp_socket = receive_tcp_socket;
                                        receive_tcp_socket.recv().await
                                    }.boxed();

                                    *this.inner = InnerInitTcp::GetEstablishing { receive_tcp_socket };

                                    // Next loop: poll the receiver. Another
                                    // process is setting up the connection.
                                    continue;
                                },
                                TcpState::None => {
                                    let (tcp_socket_sender, _) = broadcast::channel(1);
                                    let kill_init_tcp = Arc::new(AwakeToken::new());
                                    let init_connection = TcpStream::connect(this.socket.upstream_socket).boxed();

                                    *tcp_state = TcpState::Establishing {
                                        sender: tcp_socket_sender.clone(),
                                        kill: kill_init_tcp.clone()
                                    };

                                    *this.inner = InnerInitTcp::Connecting(init_connection);

                                    // Next loop: poll the TCP stream and start
                                    // connecting.
                                    continue;
                                },
                                TcpState::Blocked => {
                                    *this.inner = InnerInitTcp::Complete;

                                    // Exit loop: connection not allowed.
                                    return Poll::Ready(Err(io::Error::from(io::ErrorKind::ConnectionAborted)));
                                },
                            }
                        }
                        Poll::Pending => {
                            // Exit loop. Will be woken up once the TcpState
                            // write lock is available, the timeout condition
                            // occurs, or the connection is killed.
                            return Poll::Pending;
                        },
                    }
                },
                InnerInitTcp::Connecting(init_connection) => {
                    match init_connection.as_mut().poll(cx) {
                        Poll::Ready(Ok(socket)) => {
                            let (tcp_reader, tcp_writer) = socket.into_split();
                            let tcp_socket = Arc::new(Mutex::new(tcp_writer));
                            let w_tcp_state = this.socket.tcp.write().boxed();
                            task::spawn(this.socket.clone().listen_tcp(tcp_reader, this.kill_tcp_token.clone()));

                            *this.inner = InnerInitTcp::WriteManaged { w_tcp_state, tcp_socket };

                            // Next loop: poll the write lock.
                            continue;
                        },
                        Poll::Ready(Err(error)) => {
                            let w_tcp_state = this.socket.tcp.write().boxed();

                            *this.inner = InnerInitTcp::WriteNone { reason: CleanupReason::ConnectionError(error), w_tcp_state };

                            // Next loop: poll the write lock.
                            continue;
                        },
                        Poll::Pending => {
                            // Exit loop. Will be woken up once TCP is
                            // connected, the timeout condition occurs, or the
                            // connection is killed.
                            return Poll::Pending;
                        },
                    }
                },
                InnerInitTcp::WriteNone { reason: CleanupReason::ConnectionError(error), w_tcp_state } => {
                    match w_tcp_state.as_mut().poll(cx) {
                        Poll::Ready(mut w_tcp_state) => {
                            match &*w_tcp_state {
                                TcpState::Managed { socket, kill } => {
                                    let tcp_socket = socket.clone();
                                    let kill_tcp_token = kill.clone();

                                    // Ignore send errors. They just indicate that all receivers have been dropped.
                                    let _ = this.tcp_socket_sender.send((tcp_socket.clone(), kill_tcp_token.clone()));

                                    *this.inner = InnerInitTcp::Complete;

                                    // Exit loop: connection already setup.
                                    // Nothing to do.
                                    return Poll::Ready(Ok((tcp_socket, kill_tcp_token)));
                                },
                                TcpState::Establishing { sender, kill: active_kill_tcp_token } => {
                                    // If we are the one who set the state to Establishing...
                                    if Arc::ptr_eq(this.kill_tcp_token, active_kill_tcp_token) {
                                        *w_tcp_state = TcpState::None;
                                        drop(w_tcp_state);
                                        let error = io::Error::from(error.kind());

                                        *this.inner = InnerInitTcp::Complete;

                                        // Exit loop: we received a connection
                                        // error.
                                        return Poll::Ready(Err(error));
                                    // If some other process set the state to Establishing...
                                    } else {
                                        let receiver = sender.subscribe();
                                        let receive_tcp_socket = async move {
                                            let mut receiver = receiver;
                                            receiver.recv().await
                                        }.boxed();

                                        *this.inner = InnerInitTcp::GetEstablishing { receive_tcp_socket };

                                        // Next loop: poll the receiver.
                                        continue;
                                    }
                                },
                                TcpState::None
                              | TcpState::Blocked => {
                                    drop(w_tcp_state);
                                    let error = io::Error::from(error.kind());

                                    *this.inner = InnerInitTcp::Complete;

                                    // Exit loop: we received a connection
                                    // error.
                                    return Poll::Ready(Err(error));
                                },
                            }
                        },
                        Poll::Pending => {
                            // Exit loop. Will be woken up once the TcpState
                            // write lock is available. Cannot time out or be
                            // killed.
                            return Poll::Pending;
                        },
                    }
                },
                InnerInitTcp::WriteNone { reason: CleanupReason::Timeout, w_tcp_state } => {
                    match w_tcp_state.as_mut().poll(cx) {
                        Poll::Ready(mut w_tcp_state) => {
                            match &*w_tcp_state {
                                TcpState::Managed { socket, kill } => {
                                    let tcp_socket = socket.clone();
                                    let kill_tcp_token = kill.clone();

                                    // Ignore send errors. They just indicate that all receivers have been dropped.
                                    let _ = this.tcp_socket_sender.send((tcp_socket.clone(), kill_tcp_token.clone()));

                                    *this.inner = InnerInitTcp::Complete;

                                    // Exit loop: connection already setup.
                                    // Nothing to do.
                                    return Poll::Ready(Ok((tcp_socket, kill_tcp_token)));
                                },
                                TcpState::Establishing { sender: _, kill: active_kill_tcp_token } => {
                                    // If we are the one who set the state to Establishing...
                                    if Arc::ptr_eq(this.kill_tcp_token, active_kill_tcp_token) {
                                        *w_tcp_state = TcpState::None;
                                    }
                                    drop(w_tcp_state);
                                    *this.inner = InnerInitTcp::Complete;

                                    // Exit loop: connection timed out.
                                    return Poll::Ready(Err(io::Error::from(io::ErrorKind::TimedOut)));
                                },
                                TcpState::None
                              | TcpState::Blocked => {
                                    drop(w_tcp_state);
                                    *this.inner = InnerInitTcp::Complete;

                                    // Exit loop: connection timed out.
                                    return Poll::Ready(Err(io::Error::from(io::ErrorKind::TimedOut)));
                                },
                            }
                        },
                        Poll::Pending => {
                            // Exit loop. Will be woken up once the TcpState
                            // write lock is available. Cannot time out or be
                            // killed.
                            return Poll::Pending;
                        },
                    }
                },
                InnerInitTcp::WriteNone { reason: CleanupReason::Killed, w_tcp_state } => {
                    match w_tcp_state.as_mut().poll(cx) {
                        Poll::Ready(mut w_tcp_state) => {
                            match &*w_tcp_state {
                                TcpState::Establishing { sender: _, kill: active_kill_tcp_token } => {
                                    // If we are the one who set the state to Establishing...
                                    if Arc::ptr_eq(this.kill_tcp_token, active_kill_tcp_token) {
                                        *w_tcp_state = TcpState::None;
                                    }
                                    drop(w_tcp_state);
                                    *this.inner = InnerInitTcp::Complete;

                                    // Exit loop: connection killed.
                                    return Poll::Ready(Err(io::Error::from(io::ErrorKind::ConnectionAborted)));
                                },
                                TcpState::Managed { socket: _, kill: _ }
                              | TcpState::None
                              | TcpState::Blocked => {
                                    drop(w_tcp_state);
                                    *this.inner = InnerInitTcp::Complete;

                                    // Exit loop: connection killed.
                                    return Poll::Ready(Err(io::Error::from(io::ErrorKind::ConnectionAborted)));
                                },
                            }
                        },
                        Poll::Pending => {
                            // Exit loop. Will be woken up once the TcpState
                            // write lock is available. Cannot time out or be
                            // killed.
                            return Poll::Pending;
                        },
                    }
                },
                InnerInitTcp::WriteManaged { w_tcp_state, tcp_socket } => {
                    match w_tcp_state.as_mut().poll(cx) {
                        Poll::Ready(mut w_tcp_state) => {
                            match &*w_tcp_state {
                                TcpState::Establishing { sender: active_sender, kill: active_kill_tcp_token } => {
                                    // If we are the one who set the state to Establishing...
                                    if Arc::ptr_eq(this.kill_tcp_token, active_kill_tcp_token) {
                                        *w_tcp_state = TcpState::Managed { socket: tcp_socket.clone(), kill: this.kill_tcp_token.clone() };
                                        drop(w_tcp_state);

                                        // Ignore send errors. They just indicate that all receivers have been dropped.
                                        let _ = this.tcp_socket_sender.send((tcp_socket.clone(), this.kill_tcp_token.clone()));

                                        let tcp_socket = tcp_socket.clone();
                                        let kill_tcp_token = this.kill_tcp_token.clone();

                                        *this.inner = InnerInitTcp::Complete;

                                        // Exit loop: connection setup
                                        // completed and registered.
                                        return Poll::Ready(Ok((tcp_socket, kill_tcp_token)));
                                    // If some other process set the state to Establishing...
                                    } else {
                                        let receive_tcp_socket = active_sender.subscribe();
                                        drop(w_tcp_state);
                                        let receive_tcp_socket = async move {
                                            let mut receive_tcp_socket = receive_tcp_socket;
                                            receive_tcp_socket.recv().await
                                        }.boxed();

                                        // Shutdown the listener we started.
                                        this.kill_tcp_token.awake();

                                        *this.inner = InnerInitTcp::GetEstablishing { receive_tcp_socket };

                                        // Next loop: poll the receiver.
                                        continue;
                                    }
                                },
                                TcpState::Managed { socket, kill } => {
                                    let tcp_socket = socket.clone();
                                    let kill_tcp_token = kill.clone();
                                    drop(w_tcp_state);

                                    // Shutdown the listener we started.
                                    this.kill_tcp_token.awake();

                                    // Ignore send errors. They just indicate that all receivers have been dropped.
                                    let _ = this.tcp_socket_sender.send((tcp_socket.clone(), kill_tcp_token.clone()));

                                    *this.inner = InnerInitTcp::Complete;

                                    // Exit loop: connection already setup.
                                    // Nothing to do.
                                    return Poll::Ready(Ok((tcp_socket, kill_tcp_token)));
                                },
                                TcpState::None
                              | TcpState::Blocked => {
                                    drop(w_tcp_state);

                                    // Shutdown the listener we started.
                                    this.kill_tcp_token.awake();

                                    *this.inner = InnerInitTcp::Complete;

                                    // Exit loop: state changed after this task
                                    // set it to Establishing. Indicates that
                                    // this task is no longer in charge.
                                    return Poll::Ready(Err(io::Error::from(io::ErrorKind::ConnectionAborted)));
                                },
                            }
                        },
                        Poll::Pending => {
                            // Exit loop. Will be woken up once the TcpState
                            // write lock is available. Cannot time out or be
                            // killed.
                            return Poll::Pending;
                        },
                    }
                },
                InnerInitTcp::GetEstablishing { receive_tcp_socket } => {
                    match receive_tcp_socket.as_mut().poll(cx) {
                        Poll::Ready(Ok((tcp_socket, tcp_kill))) => {
                            let tcp_socket = tcp_socket.clone();
                            let kill_tcp_token = tcp_kill.clone();

                            *this.inner = InnerInitTcp::Complete;

                            // Exit loop: connection setup completed and
                            // registered by a different init process.
                            return Poll::Ready(Ok((tcp_socket, kill_tcp_token)));
                        },
                        Poll::Ready(Err(RecvError::Closed)) => {
                            *this.inner = InnerInitTcp::Complete;

                            // Exit loop: all senders were dropped so it is not
                            // possible to receive a connection.
                            return Poll::Ready(Err(io::Error::from(ErrorKind::Interrupted)));
                        },
                        Poll::Ready(Err(RecvError::Lagged(num_sockets))) => {
                            trace!("Channel lagged. Skipping {num_sockets} sockets. Will poll again");

                            // Next loop: will poll the receiver again.
                            continue;
                        },
                        Poll::Pending => {
                            // Exit loop. Will be woken up once a TCP write
                            // handle is received or the timeout condition
                            // occurs. Cannot be killed because it may have
                            // already been killed by self.
                            return Poll::Pending;
                        },
                    }
                },
                InnerInitTcp::Complete => panic!("InitTcp was polled after completion"),
            }
        }
    }
}

#[pinned_drop]
impl<'a, 'b, 'c, 'd, 'e, 'f, 'k, 'l, 'm> PinnedDrop for InitTcp<'a, 'b, 'c, 'd, 'e, 'f, 'k, 'l, 'm> {
    fn drop(self: Pin<&mut Self>) {
        match &self.inner {
            InnerInitTcp::Fresh
          | InnerInitTcp::WriteEstablishing(_)
          | InnerInitTcp::GetEstablishing { receive_tcp_socket: _ }
          | InnerInitTcp::Complete => {
                // Nothing to do.
            },
            InnerInitTcp::Connecting(_)
          | InnerInitTcp::WriteNone { reason: _, w_tcp_state: _ } => {
                let tcp_socket = self.socket.clone();
                let kill_tcp_token = self.kill_tcp_token.clone();
                tokio::spawn(async move {
                    let mut w_tcp_state = tcp_socket.tcp.write().await;
                    match &*w_tcp_state {
                        TcpState::Establishing { sender: _, kill: active_kill_tcp_token } => {
                            // If we are the one who set the state to Establishing...
                            if Arc::ptr_eq(&kill_tcp_token, active_kill_tcp_token) {
                                *w_tcp_state = TcpState::None;
                            }
                            drop(w_tcp_state);
                        },
                        TcpState::Managed { socket: _, kill: _ }
                      | TcpState::None
                      | TcpState::Blocked => {
                            drop(w_tcp_state);
                        },
                    }
                });
            },
            // If this struct is dropped while it is trying to write the
            // connection to the TcpState, we will spawn a task to complete
            // this operation. This way, those that depend on receiving this
            // the connection don't unexpectedly receive errors and try to
            // re-initialize the connection.
            InnerInitTcp::WriteManaged { w_tcp_state: _, tcp_socket } => {
                let tcp_socket = tcp_socket.clone();
                let socket = self.socket.clone();
                let tcp_socket_sender = self.tcp_socket_sender.clone();
                let kill_tcp_token = self.kill_tcp_token.clone();
                tokio::spawn(async move {
                    let mut w_tcp_state = socket.tcp.write().await;
                    match &*w_tcp_state {
                        TcpState::Establishing { sender: _, kill: active_kill_tcp_token } => {
                            // If we are the one who set the state to Establishing...
                            if Arc::ptr_eq(&kill_tcp_token, active_kill_tcp_token) {
                                *w_tcp_state = TcpState::Managed { socket: tcp_socket.clone(), kill: kill_tcp_token.clone() };
                                drop(w_tcp_state);

                                // Ignore send errors. They just indicate that all receivers have been dropped.
                                let _ = tcp_socket_sender.send((tcp_socket.clone(), kill_tcp_token));
                            // If some other process set the state to Establishing...
                            } else {
                                drop(w_tcp_state);

                                // Shutdown the listener we started.
                                kill_tcp_token.awake();
                            }
                        },
                        TcpState::Managed { socket: _, kill: _ }
                      | TcpState::None
                      | TcpState::Blocked => {
                            drop(w_tcp_state);

                            // Shutdown the listener we started.
                            kill_tcp_token.awake();
                        },
                    }
                });
            },
        }
    }
}

#[pin_project(PinnedDrop)]
struct TcpQuery<'a, 'b, 'c, 'd, 'e, 'q, 'r, 's, 't, 'u, 'v>
where
    'a: 'd + 'r + 'u + 'v
{
    socket: &'a Arc<MixedSocket>,
    query: &'b mut Message,
    #[pin]
    timeout: Sleep,
    #[pin]
    inner: InnerTQ<'c, 'd, 'e, 'q, 'r, 's, 't, 'u, 'v>,
}

#[pin_project(project = InnerTQProj)]
enum InnerTQ<'c, 'd, 'e, 'q, 'r, 's, 't, 'u, 'v> {
    Fresh,
    Running {
        #[pin]
        tq_socket: TQSocket<'c, 'd, 'e>,
        #[pin]
        in_flight: QInFlight<'q, 'r, 's, 'u, 'v, QSendQuery<'t>>,
    },
    Complete,
}

impl<'a, 'b, 'c, 'd, 'e, 'q, 'r, 's, 't, 'u, 'v> TcpQuery<'a, 'b, 'c, 'd, 'e, 'q, 'r, 's, 't, 'u, 'v> {
    pub fn new(socket: &'a Arc<MixedSocket>, query: &'b mut Message, timeout: Option<Duration>) -> Self {
        let timeout = timeout.unwrap_or(Duration::from_millis(TCP_TIMEOUT_MS));

        Self {
            socket,
            query,
            timeout: tokio::time::sleep(timeout),
            inner: InnerTQ::Fresh,
        }
    }
}

impl<'a, 'b, 'c, 'd, 'e, 'q, 'r, 's, 't, 'u, 'v> Future for TcpQuery<'a, 'b, 'c, 'd, 'e, 'q, 'r, 's, 't, 'u, 'v> {
    type Output = io::Result<Message>;

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        // Poll the timeout, if the state allows for the query to time out.
        let mut this = self.as_mut().project();
        match this.inner.as_mut().project() {
            InnerTQProj::Fresh => {
                if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                    this.inner.set(InnerTQ::Complete);

                    // Exit loop forever: query timed out.
                    return Poll::Ready(Err(io::Error::from(io::ErrorKind::TimedOut)));
                }
            },
            InnerTQProj::Running { tq_socket: _, mut in_flight } => {
                let in_flight_projection = in_flight.as_mut().project();
                match in_flight_projection {
                    QInFlightProj::Fresh
                  | QInFlightProj::WriteInFlight(_) => {
                        if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                            this.inner.set(InnerTQ::Complete);

                            // Exit loop forever: query timed out.
                            return Poll::Ready(Err(io::Error::from(io::ErrorKind::TimedOut)));
                        }
                    },
                    QInFlightProj::InFlight { result_receiver: _, send_query: _ } => {
                        if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                            let w_in_flight = this.socket.in_flight.write().boxed();

                            in_flight.set(QInFlight::RemoveInFlight { w_in_flight, response: Some(Err(io::Error::from(io::ErrorKind::TimedOut))) });

                            // First loop: poll the write lock to start cleanup before returning the
                            // response.
                        }
                    },
                    QInFlightProj::RemoveInFlight { w_in_flight: _, response: _ } => {
                        // Not allowed to timeout. This is a cleanup state.
                    },
                }
            },
            InnerTQProj::Complete => {
                // Not allowed to timeout. This is a cleanup state.
            },
        }

        loop {
            match this.inner.as_mut().project() {
                InnerTQProj::Fresh => {
                    let r_tcp_state = this.socket.tcp.read().boxed();
                    let w_in_flight = this.socket.in_flight.write().boxed();

                    this.inner.set(InnerTQ::Running {
                        tq_socket: TQSocket::GetTcpState(r_tcp_state),
                        in_flight: QInFlight::WriteInFlight(w_in_flight),
                    });

                    // Next loop: poll tq_socket and in_flight to start getting the TCP socket and
                    // inserting the query ID into the in-flight map.
                    continue;
                },
                InnerTQProj::Running { mut tq_socket, mut in_flight } => {
                    let tq_socket_projection = tq_socket.as_mut().project();
                    let in_flight_projection = in_flight.as_mut().project();

                    enum PollTqSocket {
                        Error(io::Error),
                        Continue,
                        Pending,
                    }

                    /// Polls the TQSocket to try to get the active TCP socket if possible.
                    /// Initializes the TCP socket if needed. If the connection fails, is not
                    /// allowed, or is killed, PollTqSocket::Error will be returned with the error
                    /// and the TQSocket should not be polled again.
                    /// Even after the connection is Acquired, calling this function to poll the
                    /// kill token to be notified when the connection is killed.
                    fn poll_tq_socket(tq_socket: &mut Pin<&mut TQSocket<'_, '_, '_>>, socket: &mut &Arc<MixedSocket>, cx: &mut std::task::Context<'_>) -> PollTqSocket {
                        match tq_socket.as_mut().project() {
                            TQSocketProj::GetTcpState(r_tcp_state) => {
                                match r_tcp_state.as_mut().poll(cx) {
                                    Poll::Ready(tcp_state) => {
                                        match &*tcp_state {
                                            TcpState::Managed { socket, kill } => {
                                                let tcp_socket = socket.clone();
                                                let kill_tcp_token = kill.clone();
    
                                                tq_socket.set(TQSocket::Acquired { tcp_socket, kill_tcp: kill_tcp_token.awoken() });

                                                // Next loop should poll `kill_tcp`
                                                return PollTqSocket::Continue;
                                            },
                                            TcpState::Establishing { sender, kill: _ } => {
                                                let receiver = sender.subscribe();
                                                let receive_tcp_socket = async move {
                                                    let mut receiver = receiver;
                                                    receiver.recv().await
                                                }.boxed();
    
                                                tq_socket.set(TQSocket::GetTcpEstablishing { receive_tcp_socket });

                                                // Next loop should poll `receive_tcp_socket`
                                                return PollTqSocket::Continue;
                                            },
                                            TcpState::None => {
                                                let init_tcp = tokio::spawn(socket.clone().init_tcp());
    
                                                tq_socket.set(TQSocket::InitTcp { join_handle: init_tcp });

                                                // Next loop should poll `join_handle`
                                                return PollTqSocket::Continue;
                                            },
                                            TcpState::Blocked => {
                                                return PollTqSocket::Error(io::Error::from(io::ErrorKind::ConnectionAborted));
                                            },
                                        }
                                    },
                                    Poll::Pending => {
                                        return PollTqSocket::Pending;
                                    },
                                }
                            },
                            TQSocketProj::GetTcpEstablishing { receive_tcp_socket } => {
                                match receive_tcp_socket.as_mut().poll(cx) {
                                    Poll::Ready(Ok((tcp_socket, tcp_kill))) => {
                                        let tcp_socket = tcp_socket.clone();
                                        let kill_tcp_token = tcp_kill.clone();
    
                                        tq_socket.set(TQSocket::Acquired { tcp_socket, kill_tcp: kill_tcp_token.awoken() });

                                        // Next loop should poll `kill_tcp`
                                        return PollTqSocket::Continue;
                                    },
                                    Poll::Ready(Err(RecvError::Closed)) => {
                                        return PollTqSocket::Error(io::Error::from(io::ErrorKind::Interrupted));
                                    },
                                    Poll::Ready(Err(RecvError::Lagged(num_sockets))) => {
                                        trace!("Channel lagged. Skipping {num_sockets} sockets. Will poll again");

                                        // Next loop should re-poll `receive_tcp_socket`
                                        return PollTqSocket::Continue;
                                    },
                                    Poll::Pending => {
                                        return PollTqSocket::Pending;
                                    },
                                }
                            },
                            TQSocketProj::InitTcp { mut join_handle } => {
                                match join_handle.as_mut().poll(cx) {
                                    Poll::Ready(Ok(Ok((tcp_socket, kill_tcp_token)))) => {
                                        let tcp_socket = tcp_socket.clone();
                                        let kill_tcp_token = kill_tcp_token.clone();
    
                                        tq_socket.set(TQSocket::Acquired { tcp_socket, kill_tcp: kill_tcp_token.awoken() });

                                        // Next loop should poll `kill_tcp`
                                        return PollTqSocket::Continue;
                                    },
                                    Poll::Ready(Ok(Err(io_error))) => {
                                        let io_error = io_error;
    
                                        return PollTqSocket::Error(io_error);
                                    },
                                    Poll::Ready(Err(join_error)) => {
                                        let io_error = io::Error::from(join_error);
    
                                        return PollTqSocket::Error(io_error);
                                    },
                                    Poll::Pending => {
                                        return PollTqSocket::Pending;
                                    },
                                }
                            },
                            TQSocketProj::Acquired { tcp_socket: _, mut kill_tcp } => {
                                match kill_tcp.as_mut().poll(cx) {
                                    Poll::Ready(()) => {
                                        return PollTqSocket::Error(io::Error::from(io::ErrorKind::Interrupted));
                                    },
                                    Poll::Pending => {
                                        return PollTqSocket::Pending;
                                    },
                                }
                            },
                            TQSocketProj::Closed => {
                                return PollTqSocket::Error(io::Error::from(io::ErrorKind::ConnectionAborted));
                            },
                        }
                    }

                    match (in_flight_projection, tq_socket_projection) {
                        (QInFlightProj::Fresh, TQSocketProj::GetTcpState(_))
                      | (QInFlightProj::Fresh, TQSocketProj::GetTcpEstablishing { receive_tcp_socket: _ })
                      | (QInFlightProj::Fresh, TQSocketProj::InitTcp { join_handle: _ })
                      | (QInFlightProj::Fresh, TQSocketProj::Acquired { tcp_socket: _, kill_tcp: _ })
                      | (QInFlightProj::Fresh, TQSocketProj::Closed) => {
                            match poll_tq_socket(&mut tq_socket, this.socket, cx) {
                                PollTqSocket::Error(error) => {
                                    this.inner.set(InnerTQ::Complete);

                                    // Nothing to clean up. Not yet in-flight map.
                                    return Poll::Ready(Err(error));
                                },
                                PollTqSocket::Continue
                              | PollTqSocket::Pending => {
                                    let w_in_flight = this.socket.in_flight.write().boxed();
                                    in_flight.set(QInFlight::WriteInFlight(w_in_flight));

                                    // Another loop is needed to poll the in-flight map, even if the
                                    // TQSocket returned Pending.
                                    continue;
                                },
                            }
                        },
                        (QInFlightProj::WriteInFlight(w_in_flight), TQSocketProj::GetTcpState(_))
                      | (QInFlightProj::WriteInFlight(w_in_flight), TQSocketProj::GetTcpEstablishing { receive_tcp_socket: _ })
                      | (QInFlightProj::WriteInFlight(w_in_flight), TQSocketProj::InitTcp { join_handle: _ })
                      | (QInFlightProj::WriteInFlight(w_in_flight), TQSocketProj::Acquired { tcp_socket: _, kill_tcp: _ })
                      | (QInFlightProj::WriteInFlight(w_in_flight), TQSocketProj::Closed) => {
                            let poll_tq_socket_result = poll_tq_socket(&mut tq_socket, this.socket, cx);

                            match (w_in_flight.as_mut().poll(cx), poll_tq_socket_result) {
                                (_, PollTqSocket::Error(error)) => {
                                    this.inner.set(InnerTQ::Complete);

                                    // Nothing to clean up. Not yet in-flight map.
                                    return Poll::Ready(Err(error));
                                },
                                (Poll::Ready(mut w_in_flight), PollTqSocket::Continue | PollTqSocket::Pending) => {
                                    let (sender, result_receiver) = broadcast::channel(1);
                                    let result_receiver = async move {
                                        let mut result_receiver = result_receiver;
                                        result_receiver.recv().await
                                    }.boxed();

                                    // This is the initial query ID. However, it could change if it
                                    // is already in use.
                                    this.query.id = rand::random();

                                    // verify that ID is unique.
                                    while w_in_flight.contains_key(&this.query.id) {
                                        this.query.id = rand::random();
                                        // FIXME: should this fail after some number of non-unique
                                        // keys? May want to verify that the list isn't full.
                                    }
                                    w_in_flight.insert(this.query.id, InFlight { send_response: sender });
                                    drop(w_in_flight);

                                    in_flight.set(QInFlight::InFlight { result_receiver, send_query: QSendQuery::Fresh });

                                    // Next loop will either establish a QSendQuery if the TQSocket
                                    // is already Acquired or will poll the TQSocket until it is so
                                    // the query can be sent.
                                    continue;
                                },
                                (Poll::Pending, PollTqSocket::Continue) => {
                                    // If at least one of the futures says to loop again, we should.
                                    continue;
                                },
                                (Poll::Pending, PollTqSocket::Pending) => {
                                    // If both futures are pending, then the entire future is
                                    // pending. This will wake up again once TQSocket wakes us, the
                                    // in-flight map lock becomes available, or the query timeout
                                    // occurs.
                                    return Poll::Pending;
                                },
                            }
                        },
                        (QInFlightProj::InFlight { result_receiver: _, send_query: QSendQuery::Fresh }, TQSocketProj::GetTcpState(_))
                      | (QInFlightProj::InFlight { result_receiver: _, send_query: QSendQuery::Fresh }, TQSocketProj::GetTcpEstablishing { receive_tcp_socket: _ })
                      | (QInFlightProj::InFlight { result_receiver: _, send_query: QSendQuery::Fresh }, TQSocketProj::InitTcp { join_handle: _ })
                      | (QInFlightProj::InFlight { result_receiver: _, send_query: QSendQuery::Fresh }, TQSocketProj::Closed) => {
                            // We don't poll the receiver until the QSendQuery state is Complete.

                            match poll_tq_socket(&mut tq_socket, this.socket, cx) {
                                PollTqSocket::Error(error) => {
                                    let w_in_flight = this.socket.in_flight.write().boxed();

                                    in_flight.set(QInFlight::RemoveInFlight { w_in_flight, response: Some(Err(error)) });

                                    // Next loop will poll for the in-flight map lock to clean up
                                    // the query ID before returning the response.
                                    continue;
                                },
                                PollTqSocket::Continue => {
                                    continue;
                                },
                                PollTqSocket::Pending => {
                                    // The TQSocket is the only future that we are waiting on,
                                    // besides the timeout. We are already registered with the
                                    // in-flight map and cannot send or receive a query until a
                                    // socket is established.
                                    return Poll::Pending;
                                },
                            }
                        },
                        (QInFlightProj::InFlight { result_receiver: _, send_query: send_query_state @ QSendQuery::Fresh }, TQSocketProj::Acquired { tcp_socket, kill_tcp: _ }) => {
                            // We don't poll the receiver until the QSendQuery state is Complete.

                            let socket = this.socket.clone();
                            let tcp_socket = tcp_socket.clone();

                            if let PollTqSocket::Error(error) = poll_tq_socket(&mut tq_socket, this.socket, cx) {
                                let w_in_flight = this.socket.in_flight.write().boxed();

                                in_flight.set(QInFlight::RemoveInFlight { w_in_flight, response: Some(Err(error)) });

                                // Next loop will poll for the in-flight map lock to clean up the
                                // query ID before returning the response.
                                continue;
                            }

                            let mut raw_message = [0_u8; MAX_MESSAGE_SIZE];
                            let mut write_wire = WriteWire::from_bytes(&mut raw_message);
                            if let Err(wire_error) = this.query.to_wire_format_with_two_octet_length(&mut write_wire, &mut Some(CompressionMap::new())) {
                                let w_in_flight = this.socket.in_flight.write().boxed();

                                in_flight.set(QInFlight::RemoveInFlight { w_in_flight, response: Some(Err(io::Error::new(io::ErrorKind::InvalidData, wire_error))) });

                                // Next loop will poll for the in-flight map lock to clean up the
                                // query ID before returning the response.
                                continue;
                            };
                            let wire_length = write_wire.current_len();

                            println!("Sending on TCP socket {} :: {:?}", this.socket.upstream_socket, this.query);

                            let send_query = async move {
                                let socket = socket;
                                let tcp_socket = tcp_socket;
                                let wire_length = wire_length;

                                socket.recent_messages_sent.store(true, Ordering::Release);
                                let mut w_tcp_stream = tcp_socket.lock().await;
                                let bytes_written = w_tcp_stream.write(&raw_message[..wire_length]).await?;
                                drop(w_tcp_stream);
                                // Verify that the correct number of bytes were written.
                                if bytes_written != wire_length {
                                    return Err(io::Error::new(
                                        io::ErrorKind::InvalidData,
                                        format!("Incorrect number of bytes sent to TCP stream; expected {wire_length} bytes but sent {bytes_written} bytes"),
                                    ));
                                }

                                return Ok(());
                            }.boxed();

                            *send_query_state = QSendQuery::SendQuery(send_query);

                            // Next loop will begin to poll SendQuery. This will get the lock and
                            // the TcpStream and write the bytes out.
                            continue;
                        },
                        (QInFlightProj::InFlight { result_receiver: _, send_query: send_query_state @ QSendQuery::SendQuery(_) }, _) => {
                            // We don't poll the receiver until the QSendQuery state is Complete.

                            let send_query = match send_query_state {
                                QSendQuery::Fresh => panic!("Previous match guaranteed that send query state is QSendQuery::SendQuery but it was QSendQuery::Fresh"),
                                QSendQuery::SendQuery(send_query) => send_query,
                                QSendQuery::Complete => panic!("Previous match guaranteed that send query state is QSendQuery::SendQuery but it was QSendQuery::Complete"),
                            };

                            let poll_tq_socket_result = poll_tq_socket(&mut tq_socket, this.socket, cx);

                            match (send_query.as_mut().poll(cx), poll_tq_socket_result) {
                                (_, PollTqSocket::Error(error))
                              | (Poll::Ready(Err(error)), _) => {
                                    let w_in_flight = this.socket.in_flight.write().boxed();

                                    in_flight.set(QInFlight::RemoveInFlight { w_in_flight, response: Some(Err(error)) });

                                    // Next loop will poll for the in-flight map lock to clean up
                                    // the query ID before returning the response.
                                    continue;
                                },
                                (Poll::Ready(Ok(())), _) => {
                                    *send_query_state = QSendQuery::Complete;

                                    // Next loop will poll the receiver, now that a message has been
                                    // sent out.
                                    continue;
                                },
                                (Poll::Pending, PollTqSocket::Continue) => {
                                    // If at least one of our futures needs to loop again, we should
                                    // loop again unless an exit condition is reached.
                                    continue;
                                },
                                (Poll::Pending, PollTqSocket::Pending) => {
                                    // All tokens are pending. Will wake up if the TQSocket wakes
                                    // us, the in-flight map lock becomes available, or the timeout
                                    // occurs.
                                    return Poll::Pending;
                                },
                            }
                        },
                        (QInFlightProj::InFlight { result_receiver, send_query: QSendQuery::Complete }, _) => {
                            let poll_tq_socket_result = poll_tq_socket(&mut tq_socket, this.socket, cx);

                            match (result_receiver.as_mut().poll(cx), poll_tq_socket_result) {
                                (Poll::Ready(Ok(response)), _) => {
                                    let w_in_flight = this.socket.in_flight.write().boxed();

                                    in_flight.set(QInFlight::RemoveInFlight { w_in_flight, response: Some(Ok(response)) });

                                    // Next loop will poll for the in-flight map lock to clean up
                                    // the query ID before returning the response.
                                    continue;
                                },
                                (Poll::Ready(Err(RecvError::Lagged(num_messages))), PollTqSocket::Continue | PollTqSocket::Pending) => {
                                    trace!("Channel lagged. Skipping {num_messages} messages. Will poll again");

                                    // Next loop will re-poll the receiver.
                                    continue;
                                },
                                (Poll::Ready(Err(RecvError::Closed)), _) => {
                                    let w_in_flight = this.socket.in_flight.write().boxed();

                                    in_flight.set(QInFlight::RemoveInFlight { w_in_flight, response: Some(Err(io::Error::from(io::ErrorKind::Interrupted))) });

                                    // Next loop will poll for the in-flight map lock to clean up
                                    // the query ID before returning the response.
                                    continue;
                                },
                                (_, PollTqSocket::Error(error)) => {
                                    let w_in_flight = this.socket.in_flight.write().boxed();

                                    in_flight.set(QInFlight::RemoveInFlight { w_in_flight, response: Some(Err(error)) });

                                    // Next loop will poll for the in-flight map lock to clean up
                                    // the query ID before returning the response.
                                    continue;
                                },
                                (Poll::Pending, PollTqSocket::Continue) => {
                                    // If at least one of our futures needs to loop again, we should
                                    // loop again unless an exit condition is reached.
                                    continue;
                                },
                                (Poll::Pending, PollTqSocket::Pending) => {
                                    // All tokens are pending. Will wake up if the TQSocket wakes
                                    // us, the receiver has a response, or the timeout occurs.
                                    return Poll::Pending;
                                },
                            }
                        },
                        (QInFlightProj::RemoveInFlight { w_in_flight, response }, _) => {
                            // This is a cleanup state so we don't care about
                            // polling the socket.

                            match w_in_flight.as_mut().poll(cx) {
                                Poll::Ready(mut w_in_flight) => {
                                    w_in_flight.remove(&this.query.id);
                                    drop(w_in_flight);
                                    let response = response.take();

                                    this.inner.set(InnerTQ::Complete);

                                    // Cleanup is done. We can pull the response out of the Option
                                    // and return it. It is stored this way since the Error type
                                    // does not support Clone.
                                    match response {
                                        Some(response) => return Poll::Ready(response),
                                        None => panic!("Inconsistent state reached. response is only supposed to be None so the value can be taken out of it"),
                                    }
                                },
                                Poll::Pending => {
                                    // Exit loop. Will wake up once the in-flight map lock becomes
                                    // available.
                                    return Poll::Pending;
                                },
                            }
                        },
                    }
                },
                InnerTQProj::Complete => panic!("TcpQuery was polled after completion"),
            }
        }
    }
}

#[pinned_drop]
impl<'a, 'b, 'c, 'd, 'e, 'h, 'i, 'j, 'k, 'l, 'm, 'n, 'o, 'p, 'q, 'r, 's, 't, 'u, 'v> PinnedDrop for TcpQuery<'a, 'b, 'c, 'd, 'e, 'q, 'r, 's, 't, 'u, 'v> {
    fn drop(self: Pin<&mut Self>) {
        match &self.inner {
            InnerTQ::Fresh => (),
          | InnerTQ::Running { tq_socket: _, in_flight: QInFlight::Fresh }
          | InnerTQ::Running { tq_socket: _, in_flight: QInFlight::WriteInFlight(_) } => {
                // Nothing to do.
            },
            InnerTQ::Running { tq_socket: _, in_flight: QInFlight::InFlight { result_receiver: _, send_query: _ } }
          | InnerTQ::Running { tq_socket: _, in_flight: QInFlight::RemoveInFlight { w_in_flight: _, response: _ } } => {
                let tcp_socket = self.socket.clone();
                let query_id = self.query.id;
                tokio::spawn(async move {
                    let mut w_in_flight = tcp_socket.in_flight.write().await;
                    w_in_flight.remove(&query_id);
                    drop(w_in_flight);
                });
            },
            InnerTQ::Complete => {
                // Nothing to do.
            },
        }
    }
}

// Implement TCP functions on MixedSocket
impl MixedSocket {
    #[inline]
    async fn init_tcp(self: Arc<Self>) -> io::Result<(Arc<Mutex<OwnedWriteHalf>>, Arc<AwakeToken>)> {
        InitTcp::new(&self, None).await
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
    async fn manage_udp_query(self: Arc<Self>, udp_socket: Arc<UdpSocket>, cancel_query: Arc<AwakeToken>, kill_udp: Arc<AwakeToken>, in_flight_sender: broadcast::Sender<Message>, query: Message) {
        let mut in_flight_receiver = in_flight_sender.subscribe();
        drop(in_flight_sender);
        let query_id = query.id;

        pin!{
            let udp_canceled = kill_udp.clone().awoken();
            let query_canceled = cancel_query.clone().awoken();
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
                return self.cleanup_query(query_id).await;
            },
            () = &mut udp_canceled => {
                println!("UDP Cleanup: UDP canceled while waiting to receive message with ID {}", query_id);
                return self.cleanup_query(query_id).await;
            },
            () = &mut query_canceled => {
                println!("UDP Cleanup: Query canceled while waiting to receive message with ID {}", query_id);
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
            () = &mut query_canceled => {
                println!("UDP Cleanup: Query canceled while waiting to receive message with ID {}", query_id);
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
                () = &mut query_canceled => {
                    println!("UDP Cleanup: Query canceled while waiting to receive message with ID {}", query_id);
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
            () = &mut query_canceled => {
                println!("UDP Cleanup: Query canceled while waiting to receive message with ID {}", query_id);
                return self.cleanup_query(query_id).await;
            },
            () = time::sleep(self.tcp_timeout) => {
                println!("UDP Timeout: TCP query with ID {} took too long to respond", query_id);
                return self.cleanup_query(query_id).await;
            },
        }
    }

    #[inline]
    async fn query_udp<'a>(self: Arc<Self>, query: Message, cancel: Arc<AwakeToken>) -> io::Result<broadcast::Receiver<Message>> {
        let udp_socket;
        let kill_udp;

        let r_udp_state = self.udp.read().await;
        match &*r_udp_state {
            UdpState::Managed(state_udp_socket, state_kill_udp) => {
                udp_socket = state_udp_socket.clone();
                kill_udp = state_kill_udp.clone();
                drop(r_udp_state);

                return self.query_udp_socket(udp_socket.clone(), cancel, kill_udp, query).await;
            },
            UdpState::None => {
                drop(r_udp_state);

                (udp_socket, kill_udp) = self.clone().init_udp().await?;
                if cancel.try_awoken() {
                    return Err(io::Error::from(ErrorKind::Interrupted));
                }
                return self.query_udp_socket(udp_socket, cancel, kill_udp, query).await;
            },
            UdpState::Blocked => {
                drop(r_udp_state);

                return Err(io::Error::from(io::ErrorKind::ConnectionAborted));
            },
        }
    }

    #[inline]
    async fn query_udp_socket(self: Arc<Self>, udp_socket: Arc<UdpSocket>, cancel_query: Arc<AwakeToken>, kill_udp: Arc<AwakeToken>, mut query: Message) -> io::Result<broadcast::Receiver<Message>> {
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
        task::spawn(self.clone().manage_udp_query(udp_socket.clone(), cancel_query, kill_udp, sender, query.clone()));

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

    pub async fn query(self: Arc<Self>, query: Message, options: QueryOptions, timeout: Option<Duration>, kill_token: Option<Arc<AwakeToken>>) -> io::Result<Message> {
        // A local token to cancel the running query.
        // If a `kill_token` was provided, that will be used as the `cancel_token`.
        let cancel_token;
        if let Some(kill_token) = &kill_token {
            cancel_token = kill_token.clone();
        } else {
            cancel_token = Arc::new(AwakeToken::new());
        }

        let udp_timeout_count = self.udp_timeout_count.load(Ordering::Acquire);
        let query_task = match (options, udp_timeout_count) {
            (QueryOptions::Both, 0..=3) => {
                task::spawn(self.query_udp(query, cancel_token.clone()))
            },
            // Too many UDP timeouts.
            (QueryOptions::Both, 4) => {
                // It will query via UDP but will start setting up a TCP connection to fall back on.
                task::spawn(self.clone().init_tcp());
                task::spawn(self.query_udp(query, cancel_token.clone()))
            },
            // Too many UDP timeouts.
            (QueryOptions::Both, 5..) => {
                todo!()
            },
            // Only TCP is allowed
            (QueryOptions::TcpOnly, _) => {
                todo!()
            },
        };

        /// Awaits the receiver returned by a spawned tokio task.
        async fn task_receive(task: JoinHandle<io::Result<broadcast::Receiver<Message>>>) -> io::Result<Message> {
            let mut receiver = match task.await {
                Ok(Ok(receiver)) => receiver,
                Ok(Err(io_error)) => return Err(io_error),
                Err(_) => return Err(io::Error::from(io::ErrorKind::Other)),
            };

            match receiver.recv().await {
                Ok(message) => Ok(message),
                Err(_) => Err(io::Error::from(io::ErrorKind::Other)),
            }
        }

        match (timeout, kill_token) {
            (None, None) => task_receive(query_task).await,
            (None, Some(kill_token)) => select! {
                response = task_receive(query_task) => response,
                () = kill_token.awoken() => Err(io::Error::from(io::ErrorKind::Other)),
            },
            (Some(timeout), None) => select! {
                response = task_receive(query_task) => response,
                () = tokio::time::sleep(timeout) => {
                    cancel_token.awake();
                    Err(io::Error::from(io::ErrorKind::TimedOut))
                },
            },
            (Some(timeout), Some(kill_token)) => select! {
                response = task_receive(query_task) => response,
                () = tokio::time::sleep(timeout) => {
                    cancel_token.awake();
                    Err(io::Error::from(io::ErrorKind::TimedOut))
                },
                () = kill_token.awoken() => Err(io::Error::from(io::ErrorKind::Other)),
            },
        }
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
        let query_task = tokio::spawn(mixed_socket.clone().query(query.clone(), QueryOptions::Both, None, None));

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

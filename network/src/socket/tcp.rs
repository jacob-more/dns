use std::{future::Future, io, net::SocketAddr, pin::Pin, sync::Arc, task::Poll, time::Duration};

use async_lib::{awake_token::{AwakeToken, AwokenToken, SameAwakeToken}, once_watch::{self, OnceWatchSend, OnceWatchSubscribe}};
use async_trait::async_trait;
use futures::{future::BoxFuture, FutureExt};
use pin_project::{pin_project, pinned_drop};
use tokio::{net::{tcp::{OwnedReadHalf, OwnedWriteHalf}, TcpStream}, task::JoinHandle, time::Sleep};

use crate::{errors, mixed_tcp_udp::TCP_INIT_TIMEOUT};

use super::{FutureSocket, PollSocket};


const SOCKET_TYPE: errors::SocketType = errors::SocketType::Tcp;


pub(crate) enum TcpState {
    Managed {
        socket: Arc<tokio::sync::Mutex<OwnedWriteHalf>>,
        kill: AwakeToken,
    },
    Establishing {
        sender: once_watch::Sender<(Arc<tokio::sync::Mutex<OwnedWriteHalf>>, AwakeToken)>,
        kill: AwakeToken,
    },
    None,
    Blocked,
}

#[async_trait]
pub(crate) trait TcpSocket where Self: 'static + Sized + Send + Sync {
    fn peer_addr(&self) -> SocketAddr;
    fn state(&self) -> &std::sync::RwLock<TcpState>;

    #[inline]
    fn socket_type(&self) -> errors::SocketType { SOCKET_TYPE }

    /// Start the TCP listener and drive the TCP state to Managed.
    #[inline]
    async fn start(self: Arc<Self>) -> Result<(), errors::SocketError> {
        match self.init().await {
            Ok(_) => Ok(()),
            Err(error) => Err(error),
        }
    }

    /// Start the TCP listener and drive the TCP state to Managed.
    /// Returns a reference to the created TCP stream.
    #[inline]
    async fn init(self: Arc<Self>) -> Result<(Arc<tokio::sync::Mutex<OwnedWriteHalf> >, AwakeToken), errors::SocketError> {
        InitTcp::new(&self, None).await
    }

    /// Shut down the TCP listener and drive the TCP state to None.
    #[inline]
    async fn shutdown(self: Arc<Self>) {
        // The write guard cannot be held across an await point (the compiler doesn't care that it
        // was dropped) so any await points must occur after the block.
        let receiver;
        {
            let mut w_state = self.state().write().unwrap();
            match &*w_state {
                TcpState::Managed { socket: _, kill } => {
                    let tcp_kill = kill.clone();
                    *w_state = TcpState::None;
                    drop(w_state);
    
                    tcp_kill.awake();
    
                    // Note: this task is not responsible for actual cleanup. Once the listener closes, it
                    // will kill any active queries and change the TcpState.
                    return;
                },
                TcpState::Establishing { sender, kill } => {
                    let sender = sender.clone();
                    let kill_init_tcp = kill.clone();
                    *w_state = TcpState::None;
                    drop(w_state);
    
                    // Try to prevent the socket from being initialized.
                    kill_init_tcp.awake();
                    sender.close();
                    receiver = sender.subscribe();
                },
                TcpState::None
              | TcpState::Blocked => {
                    // Already shut down
                    drop(w_state);

                    return;
                },
            }
        }

        // If the socket still initialized, shut it down immediately.
        match receiver.await {
            Ok((_, tcp_kill)) => {
                tcp_kill.awake();
            },
            Err(_) => (), //< Successful cancellation
        }
    }

    /// If the TCP state is Blocked, changes it to None.
    #[inline]
    async fn enable(self: Arc<Self>) {
        let mut w_state = self.state().write().unwrap();
        match &*w_state {
            TcpState::Managed { socket: _, kill: _ } => (),      //< Already enabled
            TcpState::Establishing { sender: _, kill: _ } => (), //< Already enabled
            TcpState::None => (),                                //< Already enabled
            TcpState::Blocked => *w_state = TcpState::None,
        }
        drop(w_state);
    }

    /// Sets the TCP state to Blocked, shutting down the socket if needed.
    #[inline]
    async fn disable(self: Arc<Self>) {
        let receiver;
        {
            let mut w_state = self.state().write().unwrap();
            match &*w_state {
                TcpState::Managed { socket: _, kill } => {
                    let kill_tcp = kill.clone();
                    *w_state = TcpState::Blocked;
                    drop(w_state);
    
                    kill_tcp.awake();
    
                    // Note: this task is not responsible for actual cleanup. Once the listener closes, it
                    // will kill any active queries and change the TcpState.
                    return;
                },
                TcpState::Establishing { sender, kill }=> {
                    let sender = sender.clone();
                    let kill_init_tcp = kill.clone();
                    *w_state = TcpState::Blocked;
                    drop(w_state);
    
                    // Try to prevent the socket from being initialized.
                    kill_init_tcp.awake();
                    sender.close();
                    receiver = sender.subscribe();
                },
                TcpState::None => {
                    *w_state = TcpState::Blocked;
                    drop(w_state);
                    return;
                },
                TcpState::Blocked => {
                    // Already disabled
                    drop(w_state);
                    return;
                },
            }
        }

        // If the socket still initialized, shut it down immediately.
        match receiver.await {
            Ok((_, kill_tcp)) => {
                kill_tcp.awake();
            },
            Err(_) => (), //< Successful cancellation
        }
    }

    /// Starts a TCP listener to read data from the provided socket. This processes should stop
    /// when the `kill_tcp` token is awoken. This function is intended to be run as a
    /// semi-independent background task.
    async fn listen(self: Arc<Self>, mut tcp_reader: OwnedReadHalf, kill_tcp: AwakeToken);
}

#[pin_project(project = QTcpSocketProj)]
pub(crate) enum QTcpSocket {
    Fresh,
    GetTcpEstablishing {
        #[pin]
        receive_tcp_socket: once_watch::Receiver<(Arc<tokio::sync::Mutex<OwnedWriteHalf>>, AwakeToken)>,
    },
    InitTcp {
        #[pin]
        join_handle: JoinHandle<Result<(Arc<tokio::sync::Mutex<OwnedWriteHalf>>, AwakeToken), errors::SocketError>>,
    },
    Acquired {
        tcp_socket: Arc<tokio::sync::Mutex<OwnedWriteHalf>>,
        #[pin]
        kill_tcp: AwokenToken,
    },
    Closed(errors::SocketError),
}

impl<'a, 'e> QTcpSocket {
    #[inline]
    pub fn set_get_tcp_establishing(mut self: std::pin::Pin<&mut Self>, receiver: once_watch::Receiver<(Arc<tokio::sync::Mutex<OwnedWriteHalf>>, AwakeToken)>) {
        self.set(Self::GetTcpEstablishing { receive_tcp_socket: receiver });
    }

    #[inline]
    pub fn set_init_tcp<S: TcpSocket>(mut self: std::pin::Pin<&mut Self>, socket: &'a Arc<S>) {
        let init_tcp = tokio::spawn(socket.clone().init());

        self.set(Self::InitTcp { join_handle: init_tcp });
    }

    #[inline]
    pub fn set_acquired(mut self: std::pin::Pin<&mut Self>, tcp_socket: Arc<tokio::sync::Mutex<OwnedWriteHalf>>, kill_tcp_token: AwakeToken) {
        self.set(Self::Acquired { tcp_socket, kill_tcp: kill_tcp_token.awoken() });
    }

    #[inline]
    pub fn set_closed(mut self: std::pin::Pin<&mut Self>, reason: errors::SocketError) {
        self.set(Self::Closed(reason));
    }
}

impl<'a, 'c, 'd, S: TcpSocket> FutureSocket<'a, 'd, S, errors::SocketError> for QTcpSocket
where
    'a: 'c,
{
    fn poll(self: &mut Pin<&mut Self>, socket: &'a Arc<S>, cx: &mut std::task::Context<'_>) -> PollSocket<errors::SocketError> where 'a: 'd {
        match self.as_mut().project() {
            QTcpSocketProj::Fresh => {
                let r_tcp_state = socket.state().read().unwrap();
                match &*r_tcp_state {
                    TcpState::Managed { socket, kill } => {
                        let tcp_socket = socket.clone();
                        let kill_tcp = kill.clone();
                        drop(r_tcp_state);

                        self.as_mut().set_acquired(tcp_socket, kill_tcp);

                        // Next loop should poll `kill_tcp`
                        return PollSocket::Continue;
                    },
                    TcpState::Establishing { sender, kill: _ } => {
                        let receiver = sender.subscribe();
                        drop(r_tcp_state);

                        self.as_mut().set_get_tcp_establishing(receiver);

                        // Next loop should poll `receive_tcp_socket`
                        return PollSocket::Continue;
                    },
                    TcpState::None => {
                        drop(r_tcp_state);

                        self.as_mut().set_init_tcp(socket);

                        // Next loop should poll `join_handle`
                        return PollSocket::Continue;
                    },
                    TcpState::Blocked => {
                        drop(r_tcp_state);

                        let error = errors::SocketError::Disabled(
                            SOCKET_TYPE,
                            errors::SocketStage::Initialization,
                        );

                        self.as_mut().set_closed(error.clone());

                        return PollSocket::Error(error);
                    },
                }
            },
            QTcpSocketProj::GetTcpEstablishing { mut receive_tcp_socket } => {
                match receive_tcp_socket.as_mut().poll(cx) {
                    Poll::Ready(Ok((tcp_socket, tcp_kill))) => {
                        self.as_mut().set_acquired(tcp_socket, tcp_kill);

                        // Next loop should poll `kill_tcp`
                        return PollSocket::Continue;
                    },
                    Poll::Ready(Err(once_watch::RecvError::Closed)) => {
                        let error = errors::SocketError::Shutdown(
                            SOCKET_TYPE,
                            errors::SocketStage::Initialization,
                        );

                        self.as_mut().set_closed(error.clone());

                        return PollSocket::Error(error);
                    },
                    Poll::Pending => {
                        return PollSocket::Pending;
                    },
                }
            },
            QTcpSocketProj::InitTcp { mut join_handle } => {
                match join_handle.as_mut().poll(cx) {
                    Poll::Ready(Ok(Ok((tcp_socket, kill_tcp_token)))) => {
                        self.as_mut().set_acquired(tcp_socket, kill_tcp_token);

                        // Next loop should poll `kill_tcp`
                        return PollSocket::Continue;
                    },
                    Poll::Ready(Ok(Err(error))) => {
                        let error = errors::SocketError::from(error);

                        self.as_mut().set_closed(error.clone());

                        return PollSocket::Error(error);
                    },
                    Poll::Ready(Err(join_error)) => {
                        let error = errors::SocketError::from((
                            SOCKET_TYPE,
                            errors::SocketStage::Initialization,
                            join_error,
                        ));

                        self.as_mut().set_closed(error.clone());

                        return PollSocket::Error(error);
                    },
                    Poll::Pending => {
                        return PollSocket::Pending;
                    },
                }
            },
            QTcpSocketProj::Acquired { tcp_socket: _, mut kill_tcp } => {
                match kill_tcp.as_mut().poll(cx) {
                    Poll::Ready(()) => {
                        let error = errors::SocketError::Shutdown(
                            SOCKET_TYPE,
                            errors::SocketStage::Connected,
                        );

                        self.as_mut().set_closed(error.clone());

                        return PollSocket::Error(error);
                    },
                    Poll::Pending => {
                        return PollSocket::Pending;
                    },
                }
            },
            QTcpSocketProj::Closed(error) => {
                return PollSocket::Error(error.clone());
            },
        }
    }
}

#[derive(Debug)]
enum CleanupReason<E> {
    Timeout,
    Killed,
    ConnectionError(E),
}

#[pin_project(PinnedDrop)]
struct InitTcp<'a, 'd, S>
where
    'a: 'd,
    S: TcpSocket,
{
    socket: &'a Arc<S>,
    #[pin]
    kill_tcp: AwokenToken,
    tcp_socket_sender: once_watch::Sender<(Arc<tokio::sync::Mutex<OwnedWriteHalf>>, AwakeToken)>,
    #[pin]
    timeout: Sleep,
    #[pin]
    inner: InnerInitTcp<'d>,
}

#[pin_project(project = InnerInitTcpProj)]
enum InnerInitTcp<'d> {
    Fresh,
    Connecting(BoxFuture<'d, io::Result<TcpStream>>),
    WriteNone {
        reason: CleanupReason<errors::SocketError>,
    },
    WriteManaged {
        tcp_socket: Arc<tokio::sync::Mutex<OwnedWriteHalf>>,
    },
    GetEstablishing {
        #[pin]
        receive_tcp_socket: once_watch::Receiver<(Arc<tokio::sync::Mutex<OwnedWriteHalf>>, AwakeToken)>,
    },
    Complete,
}

impl<'a, 'd, S> InitTcp<'a, 'd, S>
where
    S: TcpSocket,
{
    #[inline]
    pub fn new(socket: &'a Arc<S>, timeout: Option<Duration>) -> Self {
        let kill_tcp_token = AwakeToken::new();
        let tcp_socket_sender = once_watch::Sender::new();
        let timeout = timeout.unwrap_or(TCP_INIT_TIMEOUT);

        Self {
            socket,
            kill_tcp: kill_tcp_token.awoken(),
            tcp_socket_sender,
            timeout: tokio::time::sleep(timeout),
            inner: InnerInitTcp::Fresh,
        }
    }
}

impl<'a, 'd, S> Future for InitTcp<'a, 'd, S>
where
    S: TcpSocket,
{
    type Output = Result<(Arc<tokio::sync::Mutex<OwnedWriteHalf>>, AwakeToken), errors::SocketError>;

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let mut this = self.as_mut().project();
        match this.inner.as_mut().project() {
            InnerInitTcpProj::Fresh => {
                if let Poll::Ready(()) = this.kill_tcp.as_mut().poll(cx) {
                    this.tcp_socket_sender.close();
                    this.kill_tcp.awake();
                    let error = errors::SocketError::Shutdown(
                        SOCKET_TYPE,
                        errors::SocketStage::Initialization,
                    );

                    *this.inner = InnerInitTcp::Complete;

                    // Exit loop: query killed.
                    return Poll::Ready(Err(error));
                }

                if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                    this.tcp_socket_sender.close();
                    this.kill_tcp.awake();
                    let error = errors::SocketError::Timeout(
                        SOCKET_TYPE,
                        errors::SocketStage::Initialization,
                    );

                    *this.inner = InnerInitTcp::Complete;

                    // Exit loop: query timed out.
                    return Poll::Ready(Err(error));
                }
            },
            InnerInitTcpProj::Connecting(_) => {
                if let Poll::Ready(()) = this.kill_tcp.as_mut().poll(cx) {
                    *this.inner = InnerInitTcp::WriteNone { reason: CleanupReason::Timeout };

                    // First loop: poll the write lock.
                } else if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                    *this.inner = InnerInitTcp::WriteNone { reason: CleanupReason::Killed };

                    // First loop: poll the write lock.
                }
            },
            InnerInitTcpProj::GetEstablishing { receive_tcp_socket: _ } => {
                // Does not poll `kill_tcp` because that gets awoken to kill
                // the listener (if it is set up).
                if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                    this.tcp_socket_sender.close();
                    this.kill_tcp.awake();
                    let error = errors::SocketError::Timeout(
                        SOCKET_TYPE,
                        errors::SocketStage::Initialization,
                    );

                    *this.inner = InnerInitTcp::Complete;

                    // Exit loop: query timed out.
                    return Poll::Ready(Err(error));
                }
            },
            InnerInitTcpProj::WriteNone { reason: _ }
          | InnerInitTcpProj::WriteManaged { tcp_socket: _ }
          | InnerInitTcpProj::Complete => {
                // Not allowed to timeout or be killed. These are cleanup
                // states.
            },
        }

        loop {
            match this.inner.as_mut().project() {
                InnerInitTcpProj::Fresh => {
                    let mut w_tcp_state = this.socket.state().write().unwrap();
                    match &*w_tcp_state {
                        TcpState::Managed { socket, kill } => {
                            let tcp_socket = socket.clone();
                            let kill_tcp_token = kill.clone();
                            drop(w_tcp_state);

                            let _ = this.tcp_socket_sender.send((tcp_socket.clone(), kill_tcp_token.clone()));
                            this.kill_tcp.awake();

                            *this.inner = InnerInitTcp::Complete;

                            // Exit loop: connection already setup.
                            // Nothing to do.
                            return Poll::Ready(Ok((tcp_socket, kill_tcp_token)));
                        },
                        TcpState::Establishing { sender: active_sender, kill: _ } => {
                            let receive_tcp_socket = active_sender.subscribe();
                            drop(w_tcp_state);

                            *this.inner = InnerInitTcp::GetEstablishing { receive_tcp_socket };

                            // Next loop: poll the receiver. Another
                            // process is setting up the connection.
                            continue;
                        },
                        TcpState::None => {
                            let tcp_socket_sender = this.tcp_socket_sender.clone();
                            let kill_init_tcp = this.kill_tcp.get_awake_token();
                            *w_tcp_state = TcpState::Establishing {
                                sender: tcp_socket_sender,
                                kill: kill_init_tcp,
                            };
                            drop(w_tcp_state);

                            let init_connection = TcpStream::connect(this.socket.peer_addr()).boxed();

                            *this.inner = InnerInitTcp::Connecting(init_connection);

                            // Next loop: poll the TCP stream and start
                            // connecting.
                            continue;
                        },
                        TcpState::Blocked => {
                            drop(w_tcp_state);

                            this.tcp_socket_sender.close();
                            this.kill_tcp.awake();
                            let error = errors::SocketError::Disabled(
                                SOCKET_TYPE,
                                errors::SocketStage::Initialization,
                            );

                            *this.inner = InnerInitTcp::Complete;

                            // Exit loop: connection not allowed.
                            return Poll::Ready(Err(error));
                        },
                    }
                },
                InnerInitTcpProj::Connecting(init_connection) => {
                    match init_connection.as_mut().poll(cx) {
                        Poll::Ready(Ok(socket)) => {
                            let (tcp_reader, tcp_writer) = socket.into_split();
                            let tcp_socket = Arc::new(tokio::sync::Mutex::new(tcp_writer));
                            tokio::spawn(this.socket.clone().listen(tcp_reader, this.kill_tcp.get_awake_token()));

                            *this.inner = InnerInitTcp::WriteManaged { tcp_socket };

                            // Next loop: poll the write lock.
                            continue;
                        },
                        Poll::Ready(Err(error)) => {
                            let io_error = errors::IoError::from(error);
                            let socket_error = errors::SocketError::Io {
                                socket_type: SOCKET_TYPE,
                                socket_stage: errors::SocketStage::Initialization,
                                error: io_error,
                            };

                            *this.inner = InnerInitTcp::WriteNone { reason: CleanupReason::ConnectionError(socket_error) };

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
                InnerInitTcpProj::WriteNone { reason: CleanupReason::ConnectionError(error) } => {
                    let mut w_tcp_state = this.socket.state().write().unwrap();
                    match &*w_tcp_state {
                        TcpState::Managed { socket, kill } => {
                            let tcp_socket = socket.clone();
                            let kill_tcp_token = kill.clone();
                            drop(w_tcp_state);

                            let _ = this.tcp_socket_sender.send((tcp_socket.clone(), kill_tcp_token.clone()));
                            this.kill_tcp.awake();

                            *this.inner = InnerInitTcp::Complete;

                            // Exit loop: connection already setup.
                            // Nothing to do.
                            return Poll::Ready(Ok((tcp_socket, kill_tcp_token)));
                        },
                        TcpState::Establishing { sender, kill: active_kill_tcp_token } => {
                            // If we are the one who set the state to Establishing...
                            if this.kill_tcp.same_awake_token(active_kill_tcp_token) {
                                *w_tcp_state = TcpState::None;
                                drop(w_tcp_state);

                                this.tcp_socket_sender.close();
                                this.kill_tcp.awake();
                                let error = error.clone();

                                *this.inner = InnerInitTcp::Complete;

                                // Exit loop: we received a connection
                                // error.
                                return Poll::Ready(Err(error));
                            // If some other process set the state to Establishing...
                            } else {
                                let receive_tcp_socket = sender.subscribe();
                                drop(w_tcp_state);

                                *this.inner = InnerInitTcp::GetEstablishing { receive_tcp_socket };

                                // Next loop: poll the receiver.
                                continue;
                            }
                        },
                        TcpState::None
                      | TcpState::Blocked => {
                            drop(w_tcp_state);

                            this.tcp_socket_sender.close();
                            this.kill_tcp.awake();
                            let error = error.clone();

                            *this.inner = InnerInitTcp::Complete;

                            // Exit loop: we received a connection
                            // error.
                            return Poll::Ready(Err(error));
                        },
                    }
                },
                InnerInitTcpProj::WriteNone { reason: CleanupReason::Timeout } => {
                    let mut w_tcp_state = this.socket.state().write().unwrap();
                    match &*w_tcp_state {
                        TcpState::Managed { socket, kill } => {
                            let tcp_socket = socket.clone();
                            let kill_tcp_token = kill.clone();
                            drop(w_tcp_state);

                            let _ = this.tcp_socket_sender.send((tcp_socket.clone(), kill_tcp_token.clone()));
                            this.kill_tcp.awake();

                            *this.inner = InnerInitTcp::Complete;

                            // Exit loop: connection already setup.
                            // Nothing to do.
                            return Poll::Ready(Ok((tcp_socket, kill_tcp_token)));
                        },
                        TcpState::Establishing { sender: _, kill: active_kill_tcp_token } => {
                            // If we are the one who set the state to Establishing...
                            if this.kill_tcp.same_awake_token(active_kill_tcp_token) {
                                *w_tcp_state = TcpState::None;
                            }
                            drop(w_tcp_state);

                            this.tcp_socket_sender.close();
                            this.kill_tcp.awake();

                            *this.inner = InnerInitTcp::Complete;

                            // Exit loop: connection timed out.
                            return Poll::Ready(Err(errors::SocketError::Timeout(
                                SOCKET_TYPE,
                                errors::SocketStage::Initialization,
                            )));
                        },
                        TcpState::None
                      | TcpState::Blocked => {
                            drop(w_tcp_state);
                            this.tcp_socket_sender.close();
                            this.kill_tcp.awake();

                            *this.inner = InnerInitTcp::Complete;

                            // Exit loop: connection timed out.
                            return Poll::Ready(Err(errors::SocketError::Timeout(
                                SOCKET_TYPE,
                                errors::SocketStage::Initialization,
                            )));
                        },
                    }
                },
                InnerInitTcpProj::WriteNone { reason: CleanupReason::Killed } => {
                    let mut w_tcp_state = this.socket.state().write().unwrap();
                    match &*w_tcp_state {
                        TcpState::Establishing { sender: _, kill: active_kill_tcp_token } => {
                            // If we are the one who set the state to Establishing...
                            if this.kill_tcp.same_awake_token(active_kill_tcp_token) {
                                *w_tcp_state = TcpState::None;
                            }
                            drop(w_tcp_state);

                            this.tcp_socket_sender.close();
                            this.kill_tcp.awake();

                            *this.inner = InnerInitTcp::Complete;

                            // Exit loop: connection killed.
                            return Poll::Ready(Err(errors::SocketError::Shutdown(
                                SOCKET_TYPE,
                                errors::SocketStage::Initialization,
                            )));
                        },
                        TcpState::Managed { socket: _, kill: _ }
                      | TcpState::None
                      | TcpState::Blocked => {
                            drop(w_tcp_state);

                            this.tcp_socket_sender.close();
                            this.kill_tcp.awake();

                            *this.inner = InnerInitTcp::Complete;

                            // Exit loop: connection killed.
                            return Poll::Ready(Err(errors::SocketError::Shutdown(
                                SOCKET_TYPE,
                                errors::SocketStage::Initialization,
                            )));
                        },
                    }
                },
                InnerInitTcpProj::WriteManaged { tcp_socket } => {
                    let mut w_tcp_state = this.socket.state().write().unwrap();
                    match &*w_tcp_state {
                        TcpState::Establishing { sender: active_sender, kill: active_kill_tcp_token } => {
                            // If we are the one who set the state to Establishing...
                            if this.kill_tcp.same_awake_token(active_kill_tcp_token) {
                                *w_tcp_state = TcpState::Managed { socket: tcp_socket.clone(), kill: this.kill_tcp.get_awake_token() };
                                drop(w_tcp_state);

                                let _ = this.tcp_socket_sender.send((tcp_socket.clone(), this.kill_tcp.get_awake_token()));

                                let tcp_socket = tcp_socket.clone();
                                let kill_tcp_token = this.kill_tcp.get_awake_token();

                                *this.inner = InnerInitTcp::Complete;

                                // Exit loop: connection setup
                                // completed and registered.
                                return Poll::Ready(Ok((tcp_socket, kill_tcp_token)));
                            // If some other process set the state to Establishing...
                            } else {
                                let receive_tcp_socket = active_sender.subscribe();
                                drop(w_tcp_state);

                                // Shutdown the listener we started.
                                this.kill_tcp.awake();

                                *this.inner = InnerInitTcp::GetEstablishing { receive_tcp_socket };

                                // Next loop: poll the receiver.
                                continue;
                            }
                        },
                        TcpState::Managed { socket, kill } => {
                            let tcp_socket = socket.clone();
                            let kill_tcp_token = kill.clone();
                            drop(w_tcp_state);

                            let _ = this.tcp_socket_sender.send((tcp_socket.clone(), kill_tcp_token.clone()));
                            // Shutdown the listener we started.
                            this.kill_tcp.awake();

                            *this.inner = InnerInitTcp::Complete;

                            // Exit loop: connection already setup.
                            // Nothing to do.
                            return Poll::Ready(Ok((tcp_socket, kill_tcp_token)));
                        },
                        TcpState::None
                      | TcpState::Blocked => {
                            drop(w_tcp_state);

                            this.tcp_socket_sender.close();
                            // Shutdown the listener we started.
                            this.kill_tcp.awake();

                            *this.inner = InnerInitTcp::Complete;

                            // Exit loop: state changed after this task
                            // set it to Establishing. Indicates that
                            // this task is no longer in charge.
                            return Poll::Ready(Err(errors::SocketError::Shutdown(
                                SOCKET_TYPE,
                                errors::SocketStage::Initialization,
                            )));
                        },
                    }
                },
                InnerInitTcpProj::GetEstablishing { mut receive_tcp_socket } => {
                    match receive_tcp_socket.as_mut().poll(cx) {
                        Poll::Ready(Ok((tcp_socket, kill_tcp_token))) => {
                            let _ = this.tcp_socket_sender.send((tcp_socket.clone(), kill_tcp_token.clone()));
                            this.kill_tcp.awake();

                            *this.inner = InnerInitTcp::Complete;

                            // Exit loop: connection setup completed and
                            // registered by a different init process.
                            return Poll::Ready(Ok((tcp_socket, kill_tcp_token)));
                        },
                        Poll::Ready(Err(once_watch::RecvError::Closed)) => {
                            this.tcp_socket_sender.close();
                            this.kill_tcp.awake();

                            *this.inner = InnerInitTcp::Complete;

                            // Exit loop: all senders were dropped so it is not
                            // possible to receive a connection.
                            return Poll::Ready(Err(errors::SocketError::Shutdown(
                                SOCKET_TYPE,
                                errors::SocketStage::Initialization,
                            )));
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
                InnerInitTcpProj::Complete => panic!("InitTcp was polled after completion"),
            }
        }
    }
}

#[pinned_drop]
impl<'a, 'd, S> PinnedDrop for InitTcp<'a, 'd, S>
where
    S: TcpSocket
{
    fn drop(self: Pin<&mut Self>) {
        match &self.inner {
            InnerInitTcp::Fresh
          | InnerInitTcp::GetEstablishing { receive_tcp_socket: _ }
          | InnerInitTcp::Complete => {
                // Nothing to do.
            },
            InnerInitTcp::Connecting(_)
          | InnerInitTcp::WriteNone { reason: _ } => {
                let mut w_tcp_state = self.socket.state().write().unwrap();
                match &*w_tcp_state {
                    TcpState::Establishing { sender: _, kill: active_kill_tcp_token } => {
                        // If we are the one who set the state to Establishing...
                        if self.kill_tcp.same_awake_token(active_kill_tcp_token) {
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
            },
            // If this struct is dropped while it is trying to write the
            // connection to the TcpState, we will spawn a task to complete
            // this operation. This way, those that depend on receiving this
            // the connection don't unexpectedly receive errors and try to
            // re-initialize the connection.
            InnerInitTcp::WriteManaged { tcp_socket } => {
                // let tcp_socket = tcp_socket.clone();
                // let tcp_socket_sender = self.tcp_socket_sender.clone();
                let mut w_tcp_state = self.socket.state().write().unwrap();
                match &*w_tcp_state {
                    TcpState::Establishing { sender: _, kill: active_kill_tcp_token } => {
                        // If we are the one who set the state to Establishing...
                        if self.kill_tcp.same_awake_token(active_kill_tcp_token) {
                            *w_tcp_state = TcpState::Managed { socket: tcp_socket.clone(), kill: self.kill_tcp.get_awake_token() };
                            drop(w_tcp_state);

                            // Ignore send errors. They just indicate that all receivers have been dropped.
                            let _ = self.tcp_socket_sender.send((tcp_socket.clone(), self.kill_tcp.get_awake_token()));
                        // If some other process set the state to Establishing...
                        } else {
                            drop(w_tcp_state);

                            // Shutdown the listener we started.
                            self.kill_tcp.awake();
                        }
                    },
                    TcpState::Managed { socket: _, kill: _ }
                  | TcpState::None
                  | TcpState::Blocked => {
                        drop(w_tcp_state);

                        // Shutdown the listener we started.
                        self.kill_tcp.awake();
                    },
                }
            },
        }
    }
}

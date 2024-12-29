use std::{future::Future, io, net::SocketAddr, pin::Pin, sync::Arc, task::Poll, time::Duration};

use async_lib::{awake_token::{AwakeToken, AwokenToken, SameAwakeToken}, once_watch::{self, OnceWatchSend, OnceWatchSubscribe}};
use async_trait::async_trait;
use futures::{future::BoxFuture, FutureExt};
use pin_project::{pin_project, pinned_drop};
use tokio::{net::{tcp::{OwnedReadHalf, OwnedWriteHalf}, TcpStream}, sync::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard}, task::JoinHandle, time::Sleep};

use crate::{errors, mixed_tcp_udp::TCP_INIT_TIMEOUT};

use super::{FutureSocket, PollSocket};


pub(crate) enum TcpState {
    Managed {
        socket: Arc<Mutex<OwnedWriteHalf>>,
        kill: AwakeToken,
    },
    Establishing {
        sender: once_watch::Sender<(Arc<Mutex<OwnedWriteHalf>>, AwakeToken)>,
        kill: AwakeToken,
    },
    None,
    Blocked,
}

#[async_trait]
pub(crate) trait TcpSocket where Self: 'static + Sized + Send + Sync {
    fn peer(&self) -> &SocketAddr;
    fn state(&self) -> &RwLock<TcpState>;

    /// Start the TCP listener and drive the TCP state to Managed.
    #[inline]
    async fn start(self: Arc<Self>) -> Result<(), errors::TcpInitError> {
        match self.init().await {
            Ok(_) => Ok(()),
            Err(error) => Err(error),
        }
    }

    /// Start the TCP listener and drive the TCP state to Managed.
    /// Returns a reference to the created TCP stream.
    #[inline]
    async fn init(self: Arc<Self>) -> Result<(Arc<Mutex<OwnedWriteHalf>>, AwakeToken), errors::TcpInitError> {
        InitTcp::new(&self, None).await
    }

    /// Shut down the TCP listener and drive the TCP state to None.
    #[inline]
    async fn shutdown(self: Arc<Self>) {
        let mut w_state = self.state().write().await;
        match &*w_state {
            TcpState::Managed { socket: _, kill } => {
                let tcp_kill = kill.clone();
                *w_state = TcpState::None;
                drop(w_state);

                tcp_kill.awake();

                // Note: this task is not responsible for actual cleanup. Once the listener closes, it
                // will kill any active queries and change the TcpState.
            },
            TcpState::Establishing { sender, kill } => {
                let sender = sender.clone();
                let kill_init_tcp = kill.clone();
                *w_state = TcpState::None;
                drop(w_state);

                // Try to prevent the socket from being initialized.
                kill_init_tcp.awake();
                sender.close();
                let receiver = sender.subscribe();

                // If the socket still initialized, shut it down immediately.
                match receiver.await {
                    Ok((_, tcp_kill)) => {
                        tcp_kill.awake();
                    },
                    Err(_) => (), //< Successful cancellation
                }
            },
            TcpState::None => drop(w_state),    //< Already shut down
            TcpState::Blocked => drop(w_state), //< Already shut down
        }
    }

    /// If the TCP state is Blocked, changes it to None.
    #[inline]
    async fn enable(self: Arc<Self>) {
        let mut w_state = self.state().write().await;
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
        let mut w_state = self.state().write().await;
        match &*w_state {
            TcpState::Managed { socket: _, kill } => {
                let kill_tcp = kill.clone();
                *w_state = TcpState::Blocked;
                drop(w_state);

                kill_tcp.awake();

                // Note: this task is not responsible for actual cleanup. Once the listener closes, it
                // will kill any active queries and change the TcpState.
            },
            TcpState::Establishing { sender, kill }=> {
                let sender = sender.clone();
                let kill_init_tcp = kill.clone();
                *w_state = TcpState::Blocked;
                drop(w_state);

                // Try to prevent the socket from being initialized.
                kill_init_tcp.awake();
                sender.close();
                let receiver = sender.subscribe();

                // If the socket still initialized, shut it down immediately.
                match receiver.await {
                    Ok((_, kill_tcp)) => {
                        kill_tcp.awake();
                    },
                    Err(_) => (), //< Successful cancellation
                }
            },
            TcpState::None => {
                *w_state = TcpState::Blocked;
                drop(w_state)
            },
            TcpState::Blocked => {
                // Already disabled
                drop(w_state)
            },
        }
    }

    /// Starts a TCP listener to read data from the provided socket. This processes should stop
    /// when the `kill_tcp` token is awoken. This function is intended to be run as a
    /// semi-independent background task.
    async fn listen(self: Arc<Self>, mut tcp_reader: OwnedReadHalf, kill_tcp: AwakeToken);
}

#[pin_project(project = QTcpSocketProj)]
pub(crate) enum QTcpSocket<'c, 'd>
where
    'd: 'c,
{
    Fresh,
    GetTcpState(BoxFuture<'c, RwLockReadGuard<'d, TcpState>>),
    GetTcpEstablishing {
        #[pin]
        receive_tcp_socket: once_watch::Receiver<(Arc<Mutex<OwnedWriteHalf>>, AwakeToken)>,
    },
    InitTcp {
        #[pin]
        join_handle: JoinHandle<Result<(Arc<Mutex<OwnedWriteHalf>>, AwakeToken), errors::TcpInitError>>,
    },
    Acquired {
        tcp_socket: Arc<Mutex<OwnedWriteHalf>>,
        #[pin]
        kill_tcp: AwokenToken,
    },
    Closed(errors::TcpSocketError),
}

impl<'a, 'c, 'd, 'e> QTcpSocket<'c, 'd>
where
    'a: 'd,
    'd: 'c,
{
    #[inline]
    pub fn set_get_tcp_state<S: TcpSocket>(mut self: std::pin::Pin<&mut Self>, socket: &'a Arc<S>) {
        let r_tcp_state = socket.state().read().boxed();

        self.set(Self::GetTcpState(r_tcp_state));
    }

    #[inline]
    pub fn set_get_tcp_establishing(mut self: std::pin::Pin<&mut Self>, receiver: once_watch::Receiver<(Arc<Mutex<OwnedWriteHalf>>, AwakeToken)>) {
        self.set(Self::GetTcpEstablishing { receive_tcp_socket: receiver });
    }

    #[inline]
    pub fn set_init_tcp<S: TcpSocket>(mut self: std::pin::Pin<&mut Self>, socket: &'a Arc<S>) {
        let init_tcp = tokio::spawn(socket.clone().init());

        self.set(Self::InitTcp { join_handle: init_tcp });
    }

    #[inline]
    pub fn set_acquired(mut self: std::pin::Pin<&mut Self>, tcp_socket: Arc<Mutex<OwnedWriteHalf>>, kill_tcp_token: AwakeToken) {
        self.set(Self::Acquired { tcp_socket, kill_tcp: kill_tcp_token.awoken() });
    }

    #[inline]
    pub fn set_closed(mut self: std::pin::Pin<&mut Self>, reason: errors::TcpSocketError) {
        self.set(Self::Closed(reason));
    }
}

impl<'c, 'd, S: TcpSocket> FutureSocket<'d, S, errors::TcpSocketError> for QTcpSocket<'c, 'd> {
    fn poll<'a>(self: &mut Pin<&mut Self>, socket: &'a Arc<S>, cx: &mut std::task::Context<'_>) -> PollSocket<errors::TcpSocketError> where 'a: 'd {
        match self.as_mut().project() {
            QTcpSocketProj::Fresh => {
                self.as_mut().set_get_tcp_state(socket);

                // Next loop should poll `r_tcp_state`
                return PollSocket::Continue;
            },
            QTcpSocketProj::GetTcpState(r_tcp_state) => {
                match r_tcp_state.as_mut().poll(cx) {
                    Poll::Ready(tcp_state) => {
                        match &*tcp_state {
                            TcpState::Managed { socket, kill } => {
                                self.as_mut().set_acquired(socket.clone(), kill.clone());

                                // Next loop should poll `kill_tcp`
                                return PollSocket::Continue;
                            },
                            TcpState::Establishing { sender, kill: _ } => {
                                self.as_mut().set_get_tcp_establishing(sender.subscribe());

                                // Next loop should poll `receive_tcp_socket`
                                return PollSocket::Continue;
                            },
                            TcpState::None => {
                                self.as_mut().set_init_tcp(socket);

                                // Next loop should poll `join_handle`
                                return PollSocket::Continue;
                            },
                            TcpState::Blocked => {
                                let error = errors::TcpSocketError::Disabled;

                                self.as_mut().set_closed(error.clone());

                                return PollSocket::Error(error);
                            },
                        }
                    },
                    Poll::Pending => {
                        return PollSocket::Pending;
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
                        let error = errors::TcpSocketError::Shutdown;

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
                        let error = errors::TcpSocketError::from(error);

                        self.as_mut().set_closed(error.clone());

                        return PollSocket::Error(error);
                    },
                    Poll::Ready(Err(join_error)) => {
                        let error = errors::TcpSocketError::from(
                            errors::TcpInitError::from(join_error)
                        );

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
                        let error = errors::TcpSocketError::Shutdown;

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
struct InitTcp<'a, 'b, 'c, 'd, 'e, 'f, 'k, 'l, S>
where
    'a: 'c + 'd + 'f + 'l,
    S: TcpSocket,
{
    socket: &'a Arc<S>,
    #[pin]
    kill_tcp: AwokenToken,
    tcp_socket_sender: once_watch::Sender<(Arc<Mutex<OwnedWriteHalf>>, AwakeToken)>,
    #[pin]
    timeout: Sleep,
    #[pin]
    inner: InnerInitTcp<'b, 'c, 'd, 'e, 'f, 'k, 'l>,
}

#[pin_project(project = InnerInitTcpProj)]
enum InnerInitTcp<'b, 'c, 'd, 'e, 'f, 'k, 'l>
where
    'c: 'b,
    'f: 'e,
    'l: 'k,
{
    Fresh,
    WriteEstablishing(BoxFuture<'b, RwLockWriteGuard<'c, TcpState>>),
    Connecting(BoxFuture<'d, io::Result<TcpStream>>),
    WriteNone {
        reason: CleanupReason<errors::TcpInitError>,
        w_tcp_state: BoxFuture<'e, RwLockWriteGuard<'f, TcpState>>,
    },
    WriteManaged {
        w_tcp_state: BoxFuture<'k, RwLockWriteGuard<'l, TcpState>>,
        tcp_socket: Arc<Mutex<OwnedWriteHalf>>,
    },
    GetEstablishing {
        #[pin]
        receive_tcp_socket: once_watch::Receiver<(Arc<Mutex<OwnedWriteHalf>>, AwakeToken)>,
    },
    Complete,
}

impl<'a, 'b, 'c, 'd, 'e, 'f, 'k, 'l, S> InitTcp<'a, 'b, 'c, 'd, 'e, 'f, 'k, 'l, S>
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

impl<'a, 'b, 'c, 'd, 'e, 'f, 'k, 'l, S> Future for InitTcp<'a, 'b, 'c, 'd, 'e, 'f, 'k, 'l, S>
where
    S: TcpSocket,
{
    type Output = Result<(Arc<Mutex<OwnedWriteHalf>>, AwakeToken), errors::TcpInitError>;

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let mut this = self.as_mut().project();
        match this.inner.as_mut().project() {
            InnerInitTcpProj::Fresh
          | InnerInitTcpProj::WriteEstablishing(_) => {
                if let Poll::Ready(()) = this.kill_tcp.as_mut().poll(cx) {
                    this.tcp_socket_sender.close();
                    this.kill_tcp.awake();
                    let error = errors::TcpInitError::SocketShutdown;

                    *this.inner = InnerInitTcp::Complete;

                    // Exit loop: query killed.
                    return Poll::Ready(Err(error));
                }

                if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                    this.tcp_socket_sender.close();
                    this.kill_tcp.awake();
                    let error = errors::TcpInitError::Timeout;

                    *this.inner = InnerInitTcp::Complete;

                    // Exit loop: query timed out.
                    return Poll::Ready(Err(error));
                }
            },
            InnerInitTcpProj::Connecting(_) => {
                if let Poll::Ready(()) = this.kill_tcp.as_mut().poll(cx) {
                    let w_tcp_state = this.socket.state().write().boxed();

                    *this.inner = InnerInitTcp::WriteNone { reason: CleanupReason::Timeout, w_tcp_state };

                    // First loop: poll the write lock.
                } else if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                    let w_tcp_state = this.socket.state().write().boxed();

                    *this.inner = InnerInitTcp::WriteNone { reason: CleanupReason::Killed, w_tcp_state };

                    // First loop: poll the write lock.
                }
            },
            InnerInitTcpProj::GetEstablishing { receive_tcp_socket: _ } => {
                // Does not poll `kill_tcp` because that gets awoken to kill
                // the listener (if it is set up).
                if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                    this.tcp_socket_sender.close();
                    this.kill_tcp.awake();
                    let error = errors::TcpInitError::Timeout;

                    *this.inner = InnerInitTcp::Complete;

                    // Exit loop: query timed out.
                    return Poll::Ready(Err(error));
                }
            },
            InnerInitTcpProj::WriteNone { reason: _, w_tcp_state: _ }
          | InnerInitTcpProj::WriteManaged { w_tcp_state: _, tcp_socket: _ }
          | InnerInitTcpProj::Complete => {
                // Not allowed to timeout or be killed. These are cleanup
                // states.
            },
        }

        loop {
            match this.inner.as_mut().project() {
                InnerInitTcpProj::Fresh => {
                    let w_tcp_state = this.socket.state().write().boxed();

                    *this.inner = InnerInitTcp::WriteEstablishing(w_tcp_state);

                    // Next loop: poll the write lock to get the TCP state
                    continue;
                }
                InnerInitTcpProj::WriteEstablishing(w_tcp_state) => {
                    match w_tcp_state.as_mut().poll(cx) {
                        Poll::Ready(mut tcp_state) => {
                            match &*tcp_state {
                                TcpState::Managed { socket, kill } => {
                                    let tcp_socket = socket.clone();
                                    let kill_tcp_token = kill.clone();

                                    let _ = this.tcp_socket_sender.send((tcp_socket.clone(), kill_tcp_token.clone()));
                                    this.kill_tcp.awake();

                                    *this.inner = InnerInitTcp::Complete;

                                    // Exit loop: connection already setup.
                                    // Nothing to do.
                                    return Poll::Ready(Ok((tcp_socket, kill_tcp_token)));
                                },
                                TcpState::Establishing { sender: active_sender, kill: _ } => {
                                    let receive_tcp_socket = active_sender.subscribe();

                                    *this.inner = InnerInitTcp::GetEstablishing { receive_tcp_socket };

                                    // Next loop: poll the receiver. Another
                                    // process is setting up the connection.
                                    continue;
                                },
                                TcpState::None => {
                                    let tcp_socket_sender = this.tcp_socket_sender.clone();
                                    let kill_init_tcp = this.kill_tcp.get_awake_token();
                                    let init_connection = TcpStream::connect(this.socket.peer()).boxed();

                                    *tcp_state = TcpState::Establishing {
                                        sender: tcp_socket_sender,
                                        kill: kill_init_tcp,
                                    };

                                    *this.inner = InnerInitTcp::Connecting(init_connection);

                                    // Next loop: poll the TCP stream and start
                                    // connecting.
                                    continue;
                                },
                                TcpState::Blocked => {
                                    this.tcp_socket_sender.close();
                                    this.kill_tcp.awake();
                                    let error = errors::TcpInitError::SocketDisabled;

                                    *this.inner = InnerInitTcp::Complete;

                                    // Exit loop: connection not allowed.
                                    return Poll::Ready(Err(error));
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
                InnerInitTcpProj::Connecting(init_connection) => {
                    match init_connection.as_mut().poll(cx) {
                        Poll::Ready(Ok(socket)) => {
                            let (tcp_reader, tcp_writer) = socket.into_split();
                            let tcp_socket = Arc::new(Mutex::new(tcp_writer));
                            let w_tcp_state = this.socket.state().write().boxed();
                            tokio::spawn(this.socket.clone().listen(tcp_reader, this.kill_tcp.get_awake_token()));

                            *this.inner = InnerInitTcp::WriteManaged { w_tcp_state, tcp_socket };

                            // Next loop: poll the write lock.
                            continue;
                        },
                        Poll::Ready(Err(error)) => {
                            let w_tcp_state = this.socket.state().write().boxed();
                            let error = errors::TcpInitError::from(error);

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
                InnerInitTcpProj::WriteNone { reason: CleanupReason::ConnectionError(error), w_tcp_state } => {
                    match w_tcp_state.as_mut().poll(cx) {
                        Poll::Ready(mut w_tcp_state) => {
                            match &*w_tcp_state {
                                TcpState::Managed { socket, kill } => {
                                    let tcp_socket = socket.clone();
                                    let kill_tcp_token = kill.clone();

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
                        Poll::Pending => {
                            // Exit loop. Will be woken up once the TcpState
                            // write lock is available. Cannot time out or be
                            // killed.
                            return Poll::Pending;
                        },
                    }
                },
                InnerInitTcpProj::WriteNone { reason: CleanupReason::Timeout, w_tcp_state } => {
                    match w_tcp_state.as_mut().poll(cx) {
                        Poll::Ready(mut w_tcp_state) => {
                            match &*w_tcp_state {
                                TcpState::Managed { socket, kill } => {
                                    let tcp_socket = socket.clone();
                                    let kill_tcp_token = kill.clone();

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
                                    return Poll::Ready(Err(errors::TcpInitError::Timeout));
                                },
                                TcpState::None
                              | TcpState::Blocked => {
                                    drop(w_tcp_state);
                                    this.tcp_socket_sender.close();
                                    this.kill_tcp.awake();

                                    *this.inner = InnerInitTcp::Complete;

                                    // Exit loop: connection timed out.
                                    return Poll::Ready(Err(errors::TcpInitError::Timeout));
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
                InnerInitTcpProj::WriteNone { reason: CleanupReason::Killed, w_tcp_state } => {
                    match w_tcp_state.as_mut().poll(cx) {
                        Poll::Ready(mut w_tcp_state) => {
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
                                    return Poll::Ready(Err(errors::TcpInitError::SocketShutdown));
                                },
                                TcpState::Managed { socket: _, kill: _ }
                              | TcpState::None
                              | TcpState::Blocked => {
                                    drop(w_tcp_state);
                                    this.tcp_socket_sender.close();
                                    this.kill_tcp.awake();

                                    *this.inner = InnerInitTcp::Complete;

                                    // Exit loop: connection killed.
                                    return Poll::Ready(Err(errors::TcpInitError::SocketShutdown));
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
                InnerInitTcpProj::WriteManaged { w_tcp_state, tcp_socket } => {
                    match w_tcp_state.as_mut().poll(cx) {
                        Poll::Ready(mut w_tcp_state) => {
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
                                    return Poll::Ready(Err(errors::TcpInitError::SocketShutdown));
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
                            return Poll::Ready(Err(errors::TcpInitError::SocketShutdown));
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
impl<'a, 'b, 'c, 'd, 'e, 'f, 'k, 'l, S> PinnedDrop for InitTcp<'a, 'b, 'c, 'd, 'e, 'f, 'k, 'l, S>
where
    S: TcpSocket
{
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
                let kill_tcp_token = self.kill_tcp.get_awake_token();
                tokio::spawn(async move {
                    let mut w_tcp_state = tcp_socket.state().write().await;
                    match &*w_tcp_state {
                        TcpState::Establishing { sender: _, kill: active_kill_tcp_token } => {
                            // If we are the one who set the state to Establishing...
                            if &kill_tcp_token == active_kill_tcp_token {
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
                let kill_tcp_token = self.kill_tcp.get_awake_token();
                tokio::spawn(async move {
                    let mut w_tcp_state = socket.state().write().await;
                    match &*w_tcp_state {
                        TcpState::Establishing { sender: _, kill: active_kill_tcp_token } => {
                            // If we are the one who set the state to Establishing...
                            if &kill_tcp_token == active_kill_tcp_token {
                                *w_tcp_state = TcpState::Managed { socket: tcp_socket.clone(), kill: kill_tcp_token.clone() };
                                drop(w_tcp_state);

                                // Ignore send errors. They just indicate that all receivers have been dropped.
                                let _ = tcp_socket_sender.send((tcp_socket, kill_tcp_token));
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

use std::{future::Future, io, net::SocketAddr, pin::Pin, sync::Arc, task::Poll, time::Duration};

use async_lib::{awake_token::{AwakeToken, AwokenToken, SameAwakeToken}, once_watch::{self, OnceWatchSend, OnceWatchSubscribe}};
use async_trait::async_trait;
use dns_lib::types::c_domain_name::CDomainName;
use futures::{future::BoxFuture, FutureExt};
use pin_project::{pin_project, pinned_drop};
use rustls::pki_types::ServerName;
use tokio::{net::TcpStream, sync::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard}, task::JoinHandle, time::Sleep};
use tokio_rustls::{client::TlsStream, Connect, TlsConnector};

use crate::{errors, mixed_tcp_udp::TCP_INIT_TIMEOUT};

use super::{FutureSocket, PollSocket};


pub type TlsWriteHalf = tokio::io::WriteHalf<TlsStream<TcpStream>>;
pub type TlsReadHalf = tokio::io::ReadHalf<TlsStream<TcpStream>>;

pub(crate) enum TlsState {
    Managed {
        socket: Arc<Mutex<TlsWriteHalf>>,
        kill: AwakeToken,
    },
    Establishing {
        sender: once_watch::Sender<(Arc<Mutex<TlsWriteHalf>>, AwakeToken)>,
        kill: AwakeToken,
    },
    None,
    Blocked,
}

#[async_trait]
pub(crate) trait TlsSocket where Self: 'static + Sized + Send + Sync {
    fn peer_addr(&self) -> SocketAddr;
    fn peer_name(&self) -> &CDomainName;
    fn state(&self) -> &RwLock<TlsState>;
    fn client_config(&self) -> &Arc<rustls::ClientConfig>;

    /// Start the TLS listener and drive the TLS state to Managed.
    #[inline]
    async fn start(self: Arc<Self>) -> Result<(), errors::SocketError> {
        match self.init().await {
            Ok(_) => Ok(()),
            Err(error) => Err(error),
        }
    }

    /// Start the TLS listener and drive the TLS state to Managed.
    /// Returns a reference to the created TLS stream.
    #[inline]
    async fn init(self: Arc<Self>) -> Result<(Arc<Mutex<TlsWriteHalf>>, AwakeToken), errors::SocketError> {
        InitTls::new(&self, None).await
    }

    /// Shut down the TLS listener and drive the TLS state to None.
    #[inline]
    async fn shutdown(self: Arc<Self>) {
        let mut w_state = self.state().write().await;
        match &*w_state {
            TlsState::Managed { socket: _, kill } => {
                let tls_kill = kill.clone();
                *w_state = TlsState::None;
                drop(w_state);

                tls_kill.awake();

                // Note: this task is not responsible for actual cleanup. Once the listener closes, it
                // will kill any active queries and change the TlsState.
            },
            TlsState::Establishing { sender, kill } => {
                let sender = sender.clone();
                let kill_init_tls = kill.clone();
                *w_state = TlsState::None;
                drop(w_state);

                // Try to prevent the socket from being initialized.
                kill_init_tls.awake();
                sender.close();
                let receiver = sender.subscribe();

                // If the socket still initialized, shut it down immediately.
                match receiver.await {
                    Ok((_, tls_kill)) => {
                        tls_kill.awake();
                    },
                    Err(_) => (), //< Successful cancellation
                }
            },
            TlsState::None => drop(w_state),    //< Already shut down
            TlsState::Blocked => drop(w_state), //< Already shut down
        }
    }

    /// If the TLS state is Blocked, changes it to None.
    #[inline]
    async fn enable(self: Arc<Self>) {
        let mut w_state = self.state().write().await;
        match &*w_state {
            TlsState::Managed { socket: _, kill: _ } => (),      //< Already enabled
            TlsState::Establishing { sender: _, kill: _ } => (), //< Already enabled
            TlsState::None => (),                                //< Already enabled
            TlsState::Blocked => *w_state = TlsState::None,
        }
        drop(w_state);
    }

    /// Sets the TLS state to Blocked, shutting down the socket if needed.
    #[inline]
    async fn disable(self: Arc<Self>) {
        let mut w_state = self.state().write().await;
        match &*w_state {
            TlsState::Managed { socket: _, kill } => {
                let kill_tls = kill.clone();
                *w_state = TlsState::Blocked;
                drop(w_state);

                kill_tls.awake();

                // Note: this task is not responsible for actual cleanup. Once the listener closes, it
                // will kill any active queries and change the TlsState.
            },
            TlsState::Establishing { sender, kill }=> {
                let sender = sender.clone();
                let kill_init_tls = kill.clone();
                *w_state = TlsState::Blocked;
                drop(w_state);

                // Try to prevent the socket from being initialized.
                kill_init_tls.awake();
                sender.close();
                let receiver = sender.subscribe();

                // If the socket still initialized, shut it down immediately.
                match receiver.await {
                    Ok((_, kill_tls)) => {
                        kill_tls.awake();
                    },
                    Err(_) => (), //< Successful cancellation
                }
            },
            TlsState::None => {
                *w_state = TlsState::Blocked;
                drop(w_state)
            },
            TlsState::Blocked => {
                // Already disabled
                drop(w_state)
            },
        }
    }

    /// Starts a TLS listener to read data from the provided socket. This processes should stop
    /// when the `kill_tls` token is awoken. This function is intended to be run as a
    /// semi-independent background task.
    async fn listen(self: Arc<Self>, mut tls_reader: TlsReadHalf, kill_tls: AwakeToken);
}

#[pin_project(project = QTlsSocketProj)]
pub(crate) enum QTlsSocket<'c, 'd>
where
    'd: 'c,
{
    Fresh,
    GetTlsState(BoxFuture<'c, RwLockReadGuard<'d, TlsState>>),
    GetTlsEstablishing {
        #[pin]
        receive_tls_socket: once_watch::Receiver<(Arc<Mutex<TlsWriteHalf>>, AwakeToken)>,
    },
    InitTls {
        #[pin]
        join_handle: JoinHandle<Result<(Arc<Mutex<TlsWriteHalf>>, AwakeToken), errors::SocketError>>,
    },
    Acquired {
        tls_socket: Arc<Mutex<TlsWriteHalf>>,
        #[pin]
        kill_tls: AwokenToken,
    },
    Closed(errors::SocketError),
}

impl<'a, 'c, 'd, 'e> QTlsSocket<'c, 'd>
where
    'a: 'd,
    'd: 'c,
{
    #[inline]
    pub fn set_get_tls_state<S: TlsSocket>(mut self: std::pin::Pin<&mut Self>, socket: &'a Arc<S>) {
        let r_tls_state = socket.state().read().boxed();

        self.set(Self::GetTlsState(r_tls_state));
    }

    #[inline]
    pub fn set_get_tls_establishing(mut self: std::pin::Pin<&mut Self>, receiver: once_watch::Receiver<(Arc<Mutex<TlsWriteHalf>>, AwakeToken)>) {
        self.set(Self::GetTlsEstablishing { receive_tls_socket: receiver });
    }

    #[inline]
    pub fn set_init_tls<S: TlsSocket>(mut self: std::pin::Pin<&mut Self>, socket: &'a Arc<S>) {
        let init_tls = tokio::spawn(socket.clone().init());

        self.set(Self::InitTls { join_handle: init_tls });
    }

    #[inline]
    pub fn set_acquired(mut self: std::pin::Pin<&mut Self>, tls_socket: Arc<Mutex<TlsWriteHalf>>, kill_tls_token: AwakeToken) {
        self.set(Self::Acquired { tls_socket, kill_tls: kill_tls_token.awoken() });
    }

    #[inline]
    pub fn set_closed(mut self: std::pin::Pin<&mut Self>, reason: errors::SocketError) {
        self.set(Self::Closed(reason));
    }
}

impl<'c, 'd, S: TlsSocket> FutureSocket<'d, S, errors::SocketError> for QTlsSocket<'c, 'd> {
    fn poll<'a>(self: &mut Pin<&mut Self>, socket: &'a Arc<S>, cx: &mut std::task::Context<'_>) -> PollSocket<errors::SocketError> where 'a: 'd {
        match self.as_mut().project() {
            QTlsSocketProj::Fresh => {
                self.as_mut().set_get_tls_state(socket);

                // Next loop should poll `r_tls_state`
                return PollSocket::Continue;
            },
            QTlsSocketProj::GetTlsState(r_tls_state) => {
                match r_tls_state.as_mut().poll(cx) {
                    Poll::Ready(tls_state) => {
                        match &*tls_state {
                            TlsState::Managed { socket, kill } => {
                                self.as_mut().set_acquired(socket.clone(), kill.clone());

                                // Next loop should poll `kill_tls`
                                return PollSocket::Continue;
                            },
                            TlsState::Establishing { sender, kill: _ } => {
                                self.as_mut().set_get_tls_establishing(sender.subscribe());

                                // Next loop should poll `receive_tls_socket`
                                return PollSocket::Continue;
                            },
                            TlsState::None => {
                                self.as_mut().set_init_tls(socket);

                                // Next loop should poll `join_handle`
                                return PollSocket::Continue;
                            },
                            TlsState::Blocked => {
                                let error = errors::SocketError::Disabled(
                                    errors::SocketType::Tls,
                                    errors::SocketStage::Initialization,
                                );

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
            QTlsSocketProj::GetTlsEstablishing { mut receive_tls_socket } => {
                match receive_tls_socket.as_mut().poll(cx) {
                    Poll::Ready(Ok((tls_socket, tls_kill))) => {
                        self.as_mut().set_acquired(tls_socket, tls_kill);

                        // Next loop should poll `kill_tls`
                        return PollSocket::Continue;
                    },
                    Poll::Ready(Err(once_watch::RecvError::Closed)) => {
                        let error = errors::SocketError::Shutdown(
                            errors::SocketType::Tls,
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
            QTlsSocketProj::InitTls { mut join_handle } => {
                match join_handle.as_mut().poll(cx) {
                    Poll::Ready(Ok(Ok((tls_socket, kill_tls_token)))) => {
                        self.as_mut().set_acquired(tls_socket, kill_tls_token);

                        // Next loop should poll `kill_tls`
                        return PollSocket::Continue;
                    },
                    Poll::Ready(Ok(Err(error))) => {
                        let error = errors::SocketError::from(error);

                        self.as_mut().set_closed(error.clone());

                        return PollSocket::Error(error);
                    },
                    Poll::Ready(Err(join_error)) => {
                        let error = errors::SocketError::from(
                            errors::SocketError::from((
                                errors::SocketType::Tls,
                                errors::SocketStage::Initialization,
                                join_error,
                            ))
                        );

                        self.as_mut().set_closed(error.clone());

                        return PollSocket::Error(error);
                    },
                    Poll::Pending => {
                        return PollSocket::Pending;
                    },
                }
            },
            QTlsSocketProj::Acquired { tls_socket: _, mut kill_tls } => {
                match kill_tls.as_mut().poll(cx) {
                    Poll::Ready(()) => {
                        let error = errors::SocketError::Shutdown(
                            errors::SocketType::Tls,
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
            QTlsSocketProj::Closed(error) => {
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
struct InitTls<'a, 'b, 'c, 'd, 'e, 'f, 'k, 'l, S>
where
    'a: 'c + 'd + 'f + 'l,
    S: TlsSocket,
{
    socket: &'a Arc<S>,
    #[pin]
    kill_tls: AwokenToken,
    tls_socket_sender: once_watch::Sender<(Arc<Mutex<TlsWriteHalf>>, AwakeToken)>,
    #[pin]
    timeout: Sleep,
    #[pin]
    inner: InnerInitTls<'b, 'c, 'd, 'e, 'f, 'k, 'l>,
}

#[pin_project(project = InnerInitTlsProj)]
enum InnerInitTls<'b, 'c, 'd, 'e, 'f, 'k, 'l>
where
    'c: 'b,
    'f: 'e,
    'l: 'k,
{
    Fresh,
    WriteEstablishing(BoxFuture<'b, RwLockWriteGuard<'c, TlsState>>),
    ConnectingTcp(BoxFuture<'d, io::Result<TcpStream>>),
    ConnectingTls(#[pin] Connect<TcpStream>),
    WriteNone {
        reason: CleanupReason<errors::SocketError>,
        w_tls_state: BoxFuture<'e, RwLockWriteGuard<'f, TlsState>>,
    },
    WriteManaged {
        w_tls_state: BoxFuture<'k, RwLockWriteGuard<'l, TlsState>>,
        tls_socket: Arc<Mutex<TlsWriteHalf>>,
    },
    GetEstablishing {
        #[pin]
        receive_tls_socket: once_watch::Receiver<(Arc<Mutex<TlsWriteHalf>>, AwakeToken)>,
    },
    Complete,
}

impl<'a, 'b, 'c, 'd, 'e, 'f, 'k, 'l, S> InitTls<'a, 'b, 'c, 'd, 'e, 'f, 'k, 'l, S>
where
    S: TlsSocket,
{
    #[inline]
    pub fn new(socket: &'a Arc<S>, timeout: Option<Duration>) -> Self {
        let kill_tls_token = AwakeToken::new();
        let tls_socket_sender = once_watch::Sender::new();
        let timeout = timeout.unwrap_or(TCP_INIT_TIMEOUT);

        Self {
            socket,
            kill_tls: kill_tls_token.awoken(),
            tls_socket_sender,
            timeout: tokio::time::sleep(timeout),
            inner: InnerInitTls::Fresh,
        }
    }
}

impl<'a, 'b, 'c, 'd, 'e, 'f, 'k, 'l, S> Future for InitTls<'a, 'b, 'c, 'd, 'e, 'f, 'k, 'l, S>
where
    S: TlsSocket,
{
    type Output = Result<(Arc<Mutex<TlsWriteHalf>>, AwakeToken), errors::SocketError>;

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let mut this = self.as_mut().project();
        match this.inner.as_mut().project() {
            InnerInitTlsProj::Fresh
          | InnerInitTlsProj::WriteEstablishing(_) => {
                if let Poll::Ready(()) = this.kill_tls.as_mut().poll(cx) {
                    this.tls_socket_sender.close();
                    this.kill_tls.awake();
                    let error = errors::SocketError::Shutdown(
                        errors::SocketType::Tls,
                        errors::SocketStage::Initialization,
                    );

                    *this.inner = InnerInitTls::Complete;

                    // Exit loop: query killed.
                    return Poll::Ready(Err(error));
                }

                if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                    this.tls_socket_sender.close();
                    this.kill_tls.awake();
                    let error = errors::SocketError::Timeout(
                        errors::SocketType::Tls,
                        errors::SocketStage::Initialization,
                    );

                    *this.inner = InnerInitTls::Complete;

                    // Exit loop: query timed out.
                    return Poll::Ready(Err(error));
                }
            },
            InnerInitTlsProj::ConnectingTcp(_)
          | InnerInitTlsProj::ConnectingTls(_) => {
                if let Poll::Ready(()) = this.kill_tls.as_mut().poll(cx) {
                    let w_tls_state = this.socket.state().write().boxed();

                    *this.inner = InnerInitTls::WriteNone { reason: CleanupReason::Timeout, w_tls_state };

                    // First loop: poll the write lock.
                } else if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                    let w_tls_state = this.socket.state().write().boxed();

                    *this.inner = InnerInitTls::WriteNone { reason: CleanupReason::Killed, w_tls_state };

                    // First loop: poll the write lock.
                }
            },
            InnerInitTlsProj::GetEstablishing { receive_tls_socket: _ } => {
                // Does not poll `kill_tls` because that gets awoken to kill
                // the listener (if it is set up).
                if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                    this.tls_socket_sender.close();
                    this.kill_tls.awake();
                    let error = errors::SocketError::Timeout(
                        errors::SocketType::Tls,
                        errors::SocketStage::Initialization,
                    );

                    *this.inner = InnerInitTls::Complete;

                    // Exit loop: query timed out.
                    return Poll::Ready(Err(error));
                }
            },
            InnerInitTlsProj::WriteNone { reason: _, w_tls_state: _ }
          | InnerInitTlsProj::WriteManaged { w_tls_state: _, tls_socket: _ }
          | InnerInitTlsProj::Complete => {
                // Not allowed to timeout or be killed. These are cleanup
                // states.
            },
        }

        loop {
            match this.inner.as_mut().project() {
                InnerInitTlsProj::Fresh => {
                    let w_tls_state = this.socket.state().write().boxed();

                    *this.inner = InnerInitTls::WriteEstablishing(w_tls_state);

                    // Next loop: poll the write lock to get the TLS state
                    continue;
                }
                InnerInitTlsProj::WriteEstablishing(w_tls_state) => {
                    match w_tls_state.as_mut().poll(cx) {
                        Poll::Ready(mut tls_state) => {
                            match &*tls_state {
                                TlsState::Managed { socket, kill } => {
                                    let tls_socket = socket.clone();
                                    let kill_tls_token = kill.clone();

                                    let _ = this.tls_socket_sender.send((tls_socket.clone(), kill_tls_token.clone()));
                                    this.kill_tls.awake();

                                    *this.inner = InnerInitTls::Complete;

                                    // Exit loop: connection already setup.
                                    // Nothing to do.
                                    return Poll::Ready(Ok((tls_socket, kill_tls_token)));
                                },
                                TlsState::Establishing { sender: active_sender, kill: _ } => {
                                    let receive_tls_socket = active_sender.subscribe();

                                    *this.inner = InnerInitTls::GetEstablishing { receive_tls_socket };

                                    // Next loop: poll the receiver. Another
                                    // process is setting up the connection.
                                    continue;
                                },
                                TlsState::None => {
                                    let tls_socket_sender = this.tls_socket_sender.clone();
                                    let kill_init_tls = this.kill_tls.get_awake_token();
                                    let init_connection = TcpStream::connect(this.socket.peer_addr()).boxed();

                                    *tls_state = TlsState::Establishing {
                                        sender: tls_socket_sender,
                                        kill: kill_init_tls,
                                    };

                                    *this.inner = InnerInitTls::ConnectingTcp(init_connection);

                                    // Next loop: poll the TLS stream and start
                                    // connecting.
                                    continue;
                                },
                                TlsState::Blocked => {
                                    this.tls_socket_sender.close();
                                    this.kill_tls.awake();
                                    let error = errors::SocketError::Disabled(
                                        errors::SocketType::Tls,
                                        errors::SocketStage::Initialization,
                                    );

                                    *this.inner = InnerInitTls::Complete;

                                    // Exit loop: connection not allowed.
                                    return Poll::Ready(Err(error));
                                },
                            }
                        }
                        Poll::Pending => {
                            // Exit loop. Will be woken up once the TlsState
                            // write lock is available, the timeout condition
                            // occurs, or the connection is killed.
                            return Poll::Pending;
                        },
                    }
                },
                InnerInitTlsProj::ConnectingTcp(init_connection) => {
                    match init_connection.as_mut().poll(cx) {
                        Poll::Ready(Ok(tcp_socket)) => {
                            let mut domain = this.socket.peer_name().to_string();
                            if this.socket.peer_name().is_fully_qualified() {
                                domain.pop();
                            }
                            let domain = match ServerName::try_from(domain) {
                                Ok(domain) => domain,
                                Err(error) => {
                                    let w_tls_state = this.socket.state().write().boxed();
                                    let error = errors::SocketError::InvalidName {
                                        socket_type: errors::SocketType::Tls,
                                        socket_stage: errors::SocketStage::Initialization,
                                        error,
                                    };
        
                                    *this.inner = InnerInitTls::WriteNone { reason: CleanupReason::ConnectionError(error), w_tls_state };
        
                                    // Next loop: poll the write lock.
                                    continue;
                                },
                            };
                            let connector = TlsConnector::from(this.socket.client_config().clone());
                            let connect_tls = connector.connect(domain, tcp_socket);

                            *this.inner = InnerInitTls::ConnectingTls(connect_tls);

                            // Next loop: poll the write lock.
                            continue;
                        },
                        Poll::Ready(Err(error)) => {
                            let w_tls_state = this.socket.state().write().boxed();
                            
                            let io_error = errors::IoError::from(error);
                            let socket_error = errors::SocketError::Io {
                                socket_type: errors::SocketType::Tls,
                                socket_stage: errors::SocketStage::Initialization,
                                error: io_error,
                            };

                            *this.inner = InnerInitTls::WriteNone { reason: CleanupReason::ConnectionError(socket_error), w_tls_state };

                            // Next loop: poll the write lock.
                            continue;
                        },
                        Poll::Pending => {
                            // Exit loop. Will be woken up once TLS is
                            // connected, the timeout condition occurs, or the
                            // connection is killed.
                            return Poll::Pending;
                        },
                    }
                },
                InnerInitTlsProj::ConnectingTls(mut init_connection) => {
                    match init_connection.as_mut().poll(cx) {
                        Poll::Ready(Ok(tls_socket)) => {
                            // FIXME: There is an open issue discussing the safety of splitting the stream.
                            // See https://github.com/tokio-rs/tls/issues/40
                            let (tls_read_socket, tls_write_socket) = tokio::io::split(tls_socket);
                            let tls_write_socket = Arc::new(Mutex::new(tls_write_socket));
                            let w_tls_state = this.socket.state().write().boxed();
                            tokio::spawn(this.socket.clone().listen(tls_read_socket, this.kill_tls.get_awake_token()));

                            *this.inner = InnerInitTls::WriteManaged { w_tls_state, tls_socket: tls_write_socket };

                            // Next loop: poll the write lock.
                            continue;
                        },
                        Poll::Ready(Err(error)) => {
                            let w_tls_state = this.socket.state().write().boxed();
                            let io_error = errors::IoError::from(error);
                            let socket_error = errors::SocketError::Io {
                                socket_type: errors::SocketType::Tls,
                                socket_stage: errors::SocketStage::Initialization,
                                error: io_error,
                            };
                            println!("{socket_error:?}");

                            *this.inner = InnerInitTls::WriteNone { reason: CleanupReason::ConnectionError(socket_error), w_tls_state };

                            // Next loop: poll the write lock.
                            continue;
                        },
                        Poll::Pending => {
                            // Exit loop. Will be woken up once TLS is
                            // connected, the timeout condition occurs, or the
                            // connection is killed.
                            return Poll::Pending;
                        },
                    }
                },
                InnerInitTlsProj::WriteNone { reason: CleanupReason::ConnectionError(error), w_tls_state } => {
                    match w_tls_state.as_mut().poll(cx) {
                        Poll::Ready(mut w_tls_state) => {
                            match &*w_tls_state {
                                TlsState::Managed { socket, kill } => {
                                    let tls_socket = socket.clone();
                                    let kill_tls_token = kill.clone();

                                    let _ = this.tls_socket_sender.send((tls_socket.clone(), kill_tls_token.clone()));
                                    this.kill_tls.awake();

                                    *this.inner = InnerInitTls::Complete;

                                    // Exit loop: connection already setup.
                                    // Nothing to do.
                                    return Poll::Ready(Ok((tls_socket, kill_tls_token)));
                                },
                                TlsState::Establishing { sender, kill: active_kill_tls_token } => {
                                    // If we are the one who set the state to Establishing...
                                    if this.kill_tls.same_awake_token(active_kill_tls_token) {
                                        *w_tls_state = TlsState::None;
                                        drop(w_tls_state);
                                        this.tls_socket_sender.close();
                                        this.kill_tls.awake();
                                        let error = error.clone();

                                        *this.inner = InnerInitTls::Complete;

                                        // Exit loop: we received a connection
                                        // error.
                                        return Poll::Ready(Err(error));
                                    // If some other process set the state to Establishing...
                                    } else {
                                        let receive_tls_socket = sender.subscribe();

                                        *this.inner = InnerInitTls::GetEstablishing { receive_tls_socket };

                                        // Next loop: poll the receiver.
                                        continue;
                                    }
                                },
                                TlsState::None
                              | TlsState::Blocked => {
                                    drop(w_tls_state);
                                    this.tls_socket_sender.close();
                                    this.kill_tls.awake();
                                    let error = error.clone();

                                    *this.inner = InnerInitTls::Complete;

                                    // Exit loop: we received a connection
                                    // error.
                                    return Poll::Ready(Err(error));
                                },
                            }
                        },
                        Poll::Pending => {
                            // Exit loop. Will be woken up once the TlsState
                            // write lock is available. Cannot time out or be
                            // killed.
                            return Poll::Pending;
                        },
                    }
                },
                InnerInitTlsProj::WriteNone { reason: CleanupReason::Timeout, w_tls_state } => {
                    match w_tls_state.as_mut().poll(cx) {
                        Poll::Ready(mut w_tls_state) => {
                            match &*w_tls_state {
                                TlsState::Managed { socket, kill } => {
                                    let tls_socket = socket.clone();
                                    let kill_tls_token = kill.clone();

                                    let _ = this.tls_socket_sender.send((tls_socket.clone(), kill_tls_token.clone()));
                                    this.kill_tls.awake();

                                    *this.inner = InnerInitTls::Complete;

                                    // Exit loop: connection already setup.
                                    // Nothing to do.
                                    return Poll::Ready(Ok((tls_socket, kill_tls_token)));
                                },
                                TlsState::Establishing { sender: _, kill: active_kill_tls_token } => {
                                    // If we are the one who set the state to Establishing...
                                    if this.kill_tls.same_awake_token(active_kill_tls_token) {
                                        *w_tls_state = TlsState::None;
                                    }
                                    drop(w_tls_state);
                                    this.tls_socket_sender.close();
                                    this.kill_tls.awake();

                                    *this.inner = InnerInitTls::Complete;

                                    // Exit loop: connection timed out.
                                    return Poll::Ready(Err(errors::SocketError::Timeout(
                                        errors::SocketType::Tls,
                                        errors::SocketStage::Initialization,
                                    )));
                                },
                                TlsState::None
                              | TlsState::Blocked => {
                                    drop(w_tls_state);
                                    this.tls_socket_sender.close();
                                    this.kill_tls.awake();

                                    *this.inner = InnerInitTls::Complete;

                                    // Exit loop: connection timed out.
                                    return Poll::Ready(Err(errors::SocketError::Timeout(
                                        errors::SocketType::Tls,
                                        errors::SocketStage::Initialization,
                                    )));
                                },
                            }
                        },
                        Poll::Pending => {
                            // Exit loop. Will be woken up once the TlsState
                            // write lock is available. Cannot time out or be
                            // killed.
                            return Poll::Pending;
                        },
                    }
                },
                InnerInitTlsProj::WriteNone { reason: CleanupReason::Killed, w_tls_state } => {
                    match w_tls_state.as_mut().poll(cx) {
                        Poll::Ready(mut w_tls_state) => {
                            match &*w_tls_state {
                                TlsState::Establishing { sender: _, kill: active_kill_tls_token } => {
                                    // If we are the one who set the state to Establishing...
                                    if this.kill_tls.same_awake_token(active_kill_tls_token) {
                                        *w_tls_state = TlsState::None;
                                    }
                                    drop(w_tls_state);
                                    this.tls_socket_sender.close();
                                    this.kill_tls.awake();

                                    *this.inner = InnerInitTls::Complete;

                                    // Exit loop: connection killed.
                                    return Poll::Ready(Err(errors::SocketError::Shutdown(
                                        errors::SocketType::Tls,
                                        errors::SocketStage::Initialization,
                                    )));
                                },
                                TlsState::Managed { socket: _, kill: _ }
                              | TlsState::None
                              | TlsState::Blocked => {
                                    drop(w_tls_state);
                                    this.tls_socket_sender.close();
                                    this.kill_tls.awake();

                                    *this.inner = InnerInitTls::Complete;

                                    // Exit loop: connection killed.
                                    return Poll::Ready(Err(errors::SocketError::Shutdown(
                                        errors::SocketType::Tls,
                                        errors::SocketStage::Initialization,
                                    )));
                                },
                            }
                        },
                        Poll::Pending => {
                            // Exit loop. Will be woken up once the TlsState
                            // write lock is available. Cannot time out or be
                            // killed.
                            return Poll::Pending;
                        },
                    }
                },
                InnerInitTlsProj::WriteManaged { w_tls_state, tls_socket } => {
                    match w_tls_state.as_mut().poll(cx) {
                        Poll::Ready(mut w_tls_state) => {
                            match &*w_tls_state {
                                TlsState::Establishing { sender: active_sender, kill: active_kill_tls_token } => {
                                    // If we are the one who set the state to Establishing...
                                    if this.kill_tls.same_awake_token(active_kill_tls_token) {
                                        *w_tls_state = TlsState::Managed { socket: tls_socket.clone(), kill: this.kill_tls.get_awake_token() };
                                        drop(w_tls_state);

                                        let _ = this.tls_socket_sender.send((tls_socket.clone(), this.kill_tls.get_awake_token()));

                                        let tls_socket = tls_socket.clone();
                                        let kill_tls_token = this.kill_tls.get_awake_token();

                                        *this.inner = InnerInitTls::Complete;

                                        // Exit loop: connection setup
                                        // completed and registered.
                                        return Poll::Ready(Ok((tls_socket, kill_tls_token)));
                                    // If some other process set the state to Establishing...
                                    } else {
                                        let receive_tls_socket = active_sender.subscribe();
                                        drop(w_tls_state);

                                        // Shutdown the listener we started.
                                        this.kill_tls.awake();

                                        *this.inner = InnerInitTls::GetEstablishing { receive_tls_socket };

                                        // Next loop: poll the receiver.
                                        continue;
                                    }
                                },
                                TlsState::Managed { socket, kill } => {
                                    let tls_socket = socket.clone();
                                    let kill_tls_token = kill.clone();
                                    drop(w_tls_state);

                                    let _ = this.tls_socket_sender.send((tls_socket.clone(), kill_tls_token.clone()));
                                    // Shutdown the listener we started.
                                    this.kill_tls.awake();

                                    *this.inner = InnerInitTls::Complete;

                                    // Exit loop: connection already setup.
                                    // Nothing to do.
                                    return Poll::Ready(Ok((tls_socket, kill_tls_token)));
                                },
                                TlsState::None
                              | TlsState::Blocked => {
                                    drop(w_tls_state);

                                    this.tls_socket_sender.close();
                                    // Shutdown the listener we started.
                                    this.kill_tls.awake();

                                    *this.inner = InnerInitTls::Complete;

                                    // Exit loop: state changed after this task
                                    // set it to Establishing. Indicates that
                                    // this task is no longer in charge.
                                    return Poll::Ready(Err(errors::SocketError::Shutdown(
                                        errors::SocketType::Tls,
                                        errors::SocketStage::Initialization,
                                    )));
                                },
                            }
                        },
                        Poll::Pending => {
                            // Exit loop. Will be woken up once the TlsState
                            // write lock is available. Cannot time out or be
                            // killed.
                            return Poll::Pending;
                        },
                    }
                },
                InnerInitTlsProj::GetEstablishing { mut receive_tls_socket } => {
                    match receive_tls_socket.as_mut().poll(cx) {
                        Poll::Ready(Ok((tls_socket, kill_tls_token))) => {
                            let _ = this.tls_socket_sender.send((tls_socket.clone(), kill_tls_token.clone()));
                            this.kill_tls.awake();

                            *this.inner = InnerInitTls::Complete;

                            // Exit loop: connection setup completed and
                            // registered by a different init process.
                            return Poll::Ready(Ok((tls_socket, kill_tls_token)));
                        },
                        Poll::Ready(Err(once_watch::RecvError::Closed)) => {
                            this.tls_socket_sender.close();
                            this.kill_tls.awake();

                            *this.inner = InnerInitTls::Complete;

                            // Exit loop: all senders were dropped so it is not
                            // possible to receive a connection.
                            return Poll::Ready(Err(errors::SocketError::Shutdown(
                                errors::SocketType::Tls,
                                errors::SocketStage::Initialization,
                            )));
                        },
                        Poll::Pending => {
                            // Exit loop. Will be woken up once a TLS write
                            // handle is received or the timeout condition
                            // occurs. Cannot be killed because it may have
                            // already been killed by self.
                            return Poll::Pending;
                        },
                    }
                },
                InnerInitTlsProj::Complete => panic!("InitTls was polled after completion"),
            }
        }
    }
}

#[pinned_drop]
impl<'a, 'b, 'c, 'd, 'e, 'f, 'k, 'l, S> PinnedDrop for InitTls<'a, 'b, 'c, 'd, 'e, 'f, 'k, 'l, S>
where
    S: TlsSocket
{
    fn drop(self: Pin<&mut Self>) {
        match &self.inner {
            InnerInitTls::Fresh
          | InnerInitTls::WriteEstablishing(_)
          | InnerInitTls::GetEstablishing { receive_tls_socket: _ }
          | InnerInitTls::Complete => {
                // Nothing to do.
            },
            InnerInitTls::ConnectingTcp(_)
          | InnerInitTls::ConnectingTls(_)
          | InnerInitTls::WriteNone { reason: _, w_tls_state: _ } => {
                let tls_socket = self.socket.clone();
                let kill_tls_token = self.kill_tls.get_awake_token();
                tokio::spawn(async move {
                    let mut w_tls_state = tls_socket.state().write().await;
                    match &*w_tls_state {
                        TlsState::Establishing { sender: _, kill: active_kill_tls_token } => {
                            // If we are the one who set the state to Establishing...
                            if &kill_tls_token == active_kill_tls_token {
                                *w_tls_state = TlsState::None;
                            }
                            drop(w_tls_state);
                        },
                        TlsState::Managed { socket: _, kill: _ }
                      | TlsState::None
                      | TlsState::Blocked => {
                            drop(w_tls_state);
                        },
                    }
                });
            },
            // If this struct is dropped while it is trying to write the
            // connection to the TlsState, we will spawn a task to complete
            // this operation. This way, those that depend on receiving this
            // the connection don't unexpectedly receive errors and try to
            // re-initialize the connection.
            InnerInitTls::WriteManaged { w_tls_state: _, tls_socket } => {
                let tls_socket = tls_socket.clone();
                let socket = self.socket.clone();
                let tls_socket_sender = self.tls_socket_sender.clone();
                let kill_tls_token = self.kill_tls.get_awake_token();
                tokio::spawn(async move {
                    let mut w_tls_state = socket.state().write().await;
                    match &*w_tls_state {
                        TlsState::Establishing { sender: _, kill: active_kill_tls_token } => {
                            // If we are the one who set the state to Establishing...
                            if &kill_tls_token == active_kill_tls_token {
                                *w_tls_state = TlsState::Managed { socket: tls_socket.clone(), kill: kill_tls_token.clone() };
                                drop(w_tls_state);

                                // Ignore send errors. They just indicate that all receivers have been dropped.
                                let _ = tls_socket_sender.send((tls_socket, kill_tls_token));
                            // If some other process set the state to Establishing...
                            } else {
                                drop(w_tls_state);

                                // Shutdown the listener we started.
                                kill_tls_token.awake();
                            }
                        },
                        TlsState::Managed { socket: _, kill: _ }
                      | TlsState::None
                      | TlsState::Blocked => {
                            drop(w_tls_state);

                            // Shutdown the listener we started.
                            kill_tls_token.awake();
                        },
                    }
                });
            },
        }
    }
}

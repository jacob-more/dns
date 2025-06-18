use std::{future::Future, io, net::SocketAddr, pin::Pin, sync::Arc, task::Poll, time::Duration};

use async_lib::{
    awake_token::{AwakeToken, AwokenToken, SameAwakeToken},
    once_watch::{self, OnceWatchSend, OnceWatchSubscribe},
};
use async_trait::async_trait;
use dns_lib::types::c_domain_name::CDomainName;
use futures::{FutureExt, future::BoxFuture};
use pin_project::{pin_project, pinned_drop};
use rustls::pki_types::ServerName;
use tokio::{net::TcpStream, task::JoinHandle, time::Sleep};
use tokio_rustls::{Connect, TlsConnector, client::TlsStream};

use crate::network::{errors, mixed_tcp_udp::TCP_INIT_TIMEOUT};

use super::{FutureSocket, PollSocket};

pub type TlsWriteHalf = Arc<tokio::sync::Mutex<tokio::io::WriteHalf<TlsStream<TcpStream>>>>;
pub type TlsReadHalf = tokio::io::ReadHalf<TlsStream<TcpStream>>;

pub(crate) enum TlsState {
    Managed {
        socket: TlsWriteHalf,
        kill: AwakeToken,
    },
    Establishing {
        sender: once_watch::Sender<(TlsWriteHalf, AwakeToken)>,
        kill: AwakeToken,
    },
    None,
    Blocked,
}

#[async_trait]
pub(crate) trait TlsSocket
where
    Self: 'static + Sized + Send + Sync,
{
    fn peer_addr(&self) -> SocketAddr;
    fn peer_name(&self) -> &CDomainName;
    fn state(&self) -> &std::sync::RwLock<TlsState>;
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
    async fn init(self: Arc<Self>) -> Result<(TlsWriteHalf, AwakeToken), errors::SocketError> {
        InitTls::new(&self, None).await
    }

    /// Shut down the TLS listener and drive the TLS state to None.
    #[inline]
    async fn shutdown(self: Arc<Self>) {
        let receiver;
        {
            let mut w_state = self.state().write().unwrap();
            match &*w_state {
                TlsState::Managed { socket: _, kill } => {
                    let tls_kill = kill.clone();
                    *w_state = TlsState::None;
                    drop(w_state);

                    tls_kill.awake();

                    // Note: this task is not responsible for actual cleanup. Once the listener closes, it
                    // will kill any active queries and change the TlsState.
                    return;
                }
                TlsState::Establishing { sender, kill } => {
                    let sender = sender.clone();
                    let kill_init_tls = kill.clone();
                    *w_state = TlsState::None;
                    drop(w_state);

                    // Try to prevent the socket from being initialized.
                    kill_init_tls.awake();
                    sender.close();
                    receiver = sender.subscribe();
                }
                TlsState::None | TlsState::Blocked => {
                    // Already shut down
                    drop(w_state);
                    return;
                }
            }
        }

        // If the socket still initialized, shut it down immediately.
        if let Ok((_, kill_tls)) = receiver.await {
            kill_tls.awake();
        } // else, successful cancellation
    }

    /// If the TLS state is Blocked, changes it to None.
    #[inline]
    async fn enable(self: Arc<Self>) {
        let mut w_state = self.state().write().unwrap();
        match &*w_state {
            TlsState::Managed { socket: _, kill: _ } => (), //< Already enabled
            TlsState::Establishing { sender: _, kill: _ } => (), //< Already enabled
            TlsState::None => (),                           //< Already enabled
            TlsState::Blocked => *w_state = TlsState::None,
        }
        drop(w_state);
    }

    /// Sets the TLS state to Blocked, shutting down the socket if needed.
    #[inline]
    async fn disable(self: Arc<Self>) {
        let receiver;
        {
            let mut w_state = self.state().write().unwrap();
            match &*w_state {
                TlsState::Managed { socket: _, kill } => {
                    let kill_tls = kill.clone();
                    *w_state = TlsState::Blocked;
                    drop(w_state);

                    kill_tls.awake();

                    // Note: this task is not responsible for actual cleanup. Once the listener closes, it
                    // will kill any active queries and change the TlsState.
                    return;
                }
                TlsState::Establishing { sender, kill } => {
                    let sender = sender.clone();
                    let kill_init_tls = kill.clone();
                    *w_state = TlsState::Blocked;
                    drop(w_state);

                    // Try to prevent the socket from being initialized.
                    kill_init_tls.awake();
                    sender.close();
                    receiver = sender.subscribe();
                }
                TlsState::None => {
                    *w_state = TlsState::Blocked;
                    drop(w_state);
                    return;
                }
                TlsState::Blocked => {
                    // Already disabled
                    drop(w_state);
                    return;
                }
            }
        }

        // If the socket still initialized, shut it down immediately.
        if let Ok((_, kill_tls)) = receiver.await {
            kill_tls.awake();
        } // else, successful cancellation
    }

    /// Starts a TLS listener to read data from the provided socket. This processes should stop
    /// when the `kill_tls` token is awoken. This function is intended to be run as a
    /// semi-independent background task.
    async fn listen(self: Arc<Self>, mut tls_reader: TlsReadHalf, kill_tls: AwakeToken);
}

#[pin_project(project = QTlsSocketProj)]
pub(crate) enum QTlsSocket {
    Fresh,
    GetTlsEstablishing {
        #[pin]
        receive_tls_socket: once_watch::Receiver<(TlsWriteHalf, AwakeToken)>,
    },
    InitTls {
        #[pin]
        join_handle: JoinHandle<Result<(TlsWriteHalf, AwakeToken), errors::SocketError>>,
    },
    Acquired {
        tls_socket: TlsWriteHalf,
        #[pin]
        kill_tls: AwokenToken,
    },
    Closed(errors::SocketError),
}

impl<'a> QTlsSocket {
    #[inline]
    pub fn set_get_tls_establishing(
        mut self: std::pin::Pin<&mut Self>,
        receiver: once_watch::Receiver<(TlsWriteHalf, AwakeToken)>,
    ) {
        self.set(Self::GetTlsEstablishing {
            receive_tls_socket: receiver,
        });
    }

    #[inline]
    pub fn set_init_tls<S: TlsSocket>(mut self: std::pin::Pin<&mut Self>, socket: &'a Arc<S>) {
        let init_tls = tokio::spawn(socket.clone().init());

        self.set(Self::InitTls {
            join_handle: init_tls,
        });
    }

    #[inline]
    pub fn set_acquired(
        mut self: std::pin::Pin<&mut Self>,
        tls_socket: TlsWriteHalf,
        kill_tls_token: AwakeToken,
    ) {
        self.set(Self::Acquired {
            tls_socket,
            kill_tls: kill_tls_token.awoken(),
        });
    }

    #[inline]
    pub fn set_closed(mut self: std::pin::Pin<&mut Self>, reason: errors::SocketError) {
        self.set(Self::Closed(reason));
    }
}

impl<'a, 'd, S: TlsSocket> FutureSocket<'a, 'd, S, errors::SocketError> for QTlsSocket {
    fn poll(
        self: &mut Pin<&mut Self>,
        socket: &'a Arc<S>,
        cx: &mut std::task::Context<'_>,
    ) -> PollSocket<errors::SocketError>
    where
        'a: 'd,
    {
        match self.as_mut().project() {
            QTlsSocketProj::Fresh => {
                let r_tls_state = socket.state().read().unwrap();
                match &*r_tls_state {
                    TlsState::Managed { socket, kill } => {
                        let quic_socket = socket.clone();
                        let kill_quic = kill.clone();
                        drop(r_tls_state);

                        self.as_mut().set_acquired(quic_socket, kill_quic);

                        // Next loop should poll `kill_tls`
                        PollSocket::Continue
                    }
                    TlsState::Establishing { sender, kill: _ } => {
                        let sender = sender.subscribe();
                        drop(r_tls_state);

                        self.as_mut().set_get_tls_establishing(sender);

                        // Next loop should poll `receive_tls_socket`
                        PollSocket::Continue
                    }
                    TlsState::None => {
                        drop(r_tls_state);

                        self.as_mut().set_init_tls(socket);

                        // Next loop should poll `join_handle`
                        PollSocket::Continue
                    }
                    TlsState::Blocked => {
                        drop(r_tls_state);

                        let error = errors::SocketError::Disabled(
                            errors::SocketType::Tls,
                            errors::SocketStage::Initialization,
                        );

                        self.as_mut().set_closed(error.clone());

                        PollSocket::Error(error)
                    }
                }
            }
            QTlsSocketProj::GetTlsEstablishing {
                mut receive_tls_socket,
            } => {
                match receive_tls_socket.as_mut().poll(cx) {
                    Poll::Ready(Ok((tls_socket, tls_kill))) => {
                        self.as_mut().set_acquired(tls_socket, tls_kill);

                        // Next loop should poll `kill_tls`
                        PollSocket::Continue
                    }
                    Poll::Ready(Err(once_watch::RecvError::Closed)) => {
                        let error = errors::SocketError::Shutdown(
                            errors::SocketType::Tls,
                            errors::SocketStage::Initialization,
                        );

                        self.as_mut().set_closed(error.clone());

                        PollSocket::Error(error)
                    }
                    Poll::Pending => PollSocket::Pending,
                }
            }
            QTlsSocketProj::InitTls { mut join_handle } => {
                match join_handle.as_mut().poll(cx) {
                    Poll::Ready(Ok(Ok((tls_socket, kill_tls_token)))) => {
                        self.as_mut().set_acquired(tls_socket, kill_tls_token);

                        // Next loop should poll `kill_tls`
                        PollSocket::Continue
                    }
                    Poll::Ready(Ok(Err(error))) => {
                        let error = error;

                        self.as_mut().set_closed(error.clone());

                        PollSocket::Error(error)
                    }
                    Poll::Ready(Err(join_error)) => {
                        let error = errors::SocketError::from((
                            errors::SocketType::Tls,
                            errors::SocketStage::Initialization,
                            join_error,
                        ));

                        self.as_mut().set_closed(error.clone());

                        PollSocket::Error(error)
                    }
                    Poll::Pending => PollSocket::Pending,
                }
            }
            QTlsSocketProj::Acquired {
                tls_socket: _,
                mut kill_tls,
            } => match kill_tls.as_mut().poll(cx) {
                Poll::Ready(()) => {
                    let error = errors::SocketError::Shutdown(
                        errors::SocketType::Tls,
                        errors::SocketStage::Connected,
                    );

                    self.as_mut().set_closed(error.clone());

                    PollSocket::Error(error)
                }
                Poll::Pending => PollSocket::Pending,
            },
            QTlsSocketProj::Closed(error) => PollSocket::Error(error.clone()),
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
struct InitTls<'a, 'd, S>
where
    'a: 'd,
    S: TlsSocket,
{
    socket: &'a Arc<S>,
    #[pin]
    kill_tls: AwokenToken,
    tls_socket_sender: once_watch::Sender<(TlsWriteHalf, AwakeToken)>,
    #[pin]
    timeout: Sleep,
    #[pin]
    inner: InnerInitTls<'d>,
}

#[pin_project(project = InnerInitTlsProj)]
#[allow(clippy::large_enum_variant)]
enum InnerInitTls<'d> {
    Fresh,
    WriteEstablishing,
    ConnectingTcp(BoxFuture<'d, io::Result<TcpStream>>),
    ConnectingTls(#[pin] Connect<TcpStream>),
    WriteNone {
        reason: CleanupReason<errors::SocketError>,
    },
    WriteManaged {
        tls_socket: TlsWriteHalf,
    },
    GetEstablishing {
        #[pin]
        receive_tls_socket: once_watch::Receiver<(TlsWriteHalf, AwakeToken)>,
    },
    Complete,
}

impl<'a, 'd, S> InitTls<'a, 'd, S>
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

impl<'a, 'd, S> Future for InitTls<'a, 'd, S>
where
    S: TlsSocket,
{
    type Output = Result<(TlsWriteHalf, AwakeToken), errors::SocketError>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let mut this = self.as_mut().project();
        match this.inner.as_mut().project() {
            InnerInitTlsProj::Fresh | InnerInitTlsProj::WriteEstablishing => {
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
            }
            InnerInitTlsProj::ConnectingTcp(_) | InnerInitTlsProj::ConnectingTls(_) => {
                if let Poll::Ready(()) = this.kill_tls.as_mut().poll(cx) {
                    *this.inner = InnerInitTls::WriteNone {
                        reason: CleanupReason::Timeout,
                    };

                    // First loop: poll the write lock.
                } else if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                    *this.inner = InnerInitTls::WriteNone {
                        reason: CleanupReason::Killed,
                    };

                    // First loop: poll the write lock.
                }
            }
            InnerInitTlsProj::GetEstablishing {
                receive_tls_socket: _,
            } => {
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
            }
            InnerInitTlsProj::WriteNone { reason: _ }
            | InnerInitTlsProj::WriteManaged { tls_socket: _ }
            | InnerInitTlsProj::Complete => {
                // Not allowed to timeout or be killed. These are cleanup
                // states.
            }
        }

        loop {
            match this.inner.as_mut().project() {
                InnerInitTlsProj::Fresh | InnerInitTlsProj::WriteEstablishing => {
                    let mut w_tls_state = this.socket.state().write().unwrap();
                    match &*w_tls_state {
                        TlsState::Managed { socket, kill } => {
                            let tls_socket = socket.clone();
                            let kill_tls_token = kill.clone();
                            drop(w_tls_state);

                            let _ = this
                                .tls_socket_sender
                                .send((tls_socket.clone(), kill_tls_token.clone()));
                            this.kill_tls.awake();

                            *this.inner = InnerInitTls::Complete;

                            // Exit loop: connection already setup.
                            // Nothing to do.
                            return Poll::Ready(Ok((tls_socket, kill_tls_token)));
                        }
                        TlsState::Establishing {
                            sender: active_sender,
                            kill: _,
                        } => {
                            let receive_tls_socket = active_sender.subscribe();
                            drop(w_tls_state);

                            *this.inner = InnerInitTls::GetEstablishing { receive_tls_socket };

                            // Next loop: poll the receiver. Another
                            // process is setting up the connection.
                            continue;
                        }
                        TlsState::None => {
                            let tls_socket_sender = this.tls_socket_sender.clone();
                            let kill_init_tls = this.kill_tls.get_awake_token();
                            let init_connection =
                                TcpStream::connect(this.socket.peer_addr()).boxed();

                            *w_tls_state = TlsState::Establishing {
                                sender: tls_socket_sender,
                                kill: kill_init_tls,
                            };
                            drop(w_tls_state);

                            *this.inner = InnerInitTls::ConnectingTcp(init_connection);

                            // Next loop: poll the TLS stream and start
                            // connecting.
                            continue;
                        }
                        TlsState::Blocked => {
                            drop(w_tls_state);

                            this.tls_socket_sender.close();
                            this.kill_tls.awake();
                            let error = errors::SocketError::Disabled(
                                errors::SocketType::Tls,
                                errors::SocketStage::Initialization,
                            );

                            *this.inner = InnerInitTls::Complete;

                            // Exit loop: connection not allowed.
                            return Poll::Ready(Err(error));
                        }
                    }
                }
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
                                    let error = errors::SocketError::InvalidName {
                                        socket_type: errors::SocketType::Tls,
                                        socket_stage: errors::SocketStage::Initialization,
                                        error,
                                    };

                                    *this.inner = InnerInitTls::WriteNone {
                                        reason: CleanupReason::ConnectionError(error),
                                    };

                                    // Next loop: poll the write lock.
                                    continue;
                                }
                            };
                            let connector = TlsConnector::from(this.socket.client_config().clone());
                            let connect_tls = connector.connect(domain, tcp_socket);

                            *this.inner = InnerInitTls::ConnectingTls(connect_tls);

                            // Next loop: poll the write lock.
                            continue;
                        }
                        Poll::Ready(Err(error)) => {
                            let io_error = errors::IoError::from(error);
                            let socket_error = errors::SocketError::Io {
                                socket_type: errors::SocketType::Tls,
                                socket_stage: errors::SocketStage::Initialization,
                                error: io_error,
                            };

                            *this.inner = InnerInitTls::WriteNone {
                                reason: CleanupReason::ConnectionError(socket_error),
                            };

                            // Next loop: poll the write lock.
                            continue;
                        }
                        Poll::Pending => {
                            // Exit loop. Will be woken up once TLS is
                            // connected, the timeout condition occurs, or the
                            // connection is killed.
                            return Poll::Pending;
                        }
                    }
                }
                InnerInitTlsProj::ConnectingTls(mut init_connection) => {
                    match init_connection.as_mut().poll(cx) {
                        Poll::Ready(Ok(tls_socket)) => {
                            // FIXME: There is an open issue discussing the safety of splitting the stream.
                            // See https://github.com/tokio-rs/tls/issues/40
                            let (tls_read_socket, tls_write_socket) = tokio::io::split(tls_socket);
                            let tls_write_socket =
                                Arc::new(tokio::sync::Mutex::new(tls_write_socket));
                            tokio::spawn(
                                this.socket
                                    .clone()
                                    .listen(tls_read_socket, this.kill_tls.get_awake_token()),
                            );

                            *this.inner = InnerInitTls::WriteManaged {
                                tls_socket: tls_write_socket,
                            };

                            // Next loop: poll the write lock.
                            continue;
                        }
                        Poll::Ready(Err(error)) => {
                            let io_error = errors::IoError::from(error);
                            let socket_error = errors::SocketError::Io {
                                socket_type: errors::SocketType::Tls,
                                socket_stage: errors::SocketStage::Initialization,
                                error: io_error,
                            };
                            println!("{socket_error:?}");

                            *this.inner = InnerInitTls::WriteNone {
                                reason: CleanupReason::ConnectionError(socket_error),
                            };

                            // Next loop: poll the write lock.
                            continue;
                        }
                        Poll::Pending => {
                            // Exit loop. Will be woken up once TLS is
                            // connected, the timeout condition occurs, or the
                            // connection is killed.
                            return Poll::Pending;
                        }
                    }
                }
                InnerInitTlsProj::WriteNone {
                    reason: CleanupReason::ConnectionError(error),
                } => {
                    let mut w_tls_state = this.socket.state().write().unwrap();
                    match &*w_tls_state {
                        TlsState::Managed { socket, kill } => {
                            let tls_socket = socket.clone();
                            let kill_tls_token = kill.clone();
                            drop(w_tls_state);

                            let _ = this
                                .tls_socket_sender
                                .send((tls_socket.clone(), kill_tls_token.clone()));
                            this.kill_tls.awake();

                            *this.inner = InnerInitTls::Complete;

                            // Exit loop: connection already setup.
                            // Nothing to do.
                            return Poll::Ready(Ok((tls_socket, kill_tls_token)));
                        }
                        TlsState::Establishing {
                            sender,
                            kill: active_kill_tls_token,
                        } => {
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
                                drop(w_tls_state);

                                *this.inner = InnerInitTls::GetEstablishing { receive_tls_socket };

                                // Next loop: poll the receiver.
                                continue;
                            }
                        }
                        TlsState::None | TlsState::Blocked => {
                            drop(w_tls_state);

                            this.tls_socket_sender.close();
                            this.kill_tls.awake();
                            let error = error.clone();

                            *this.inner = InnerInitTls::Complete;

                            // Exit loop: we received a connection
                            // error.
                            return Poll::Ready(Err(error));
                        }
                    }
                }
                InnerInitTlsProj::WriteNone {
                    reason: CleanupReason::Timeout,
                } => {
                    let mut w_tls_state = this.socket.state().write().unwrap();
                    match &*w_tls_state {
                        TlsState::Managed { socket, kill } => {
                            let tls_socket = socket.clone();
                            let kill_tls_token = kill.clone();
                            drop(w_tls_state);

                            let _ = this
                                .tls_socket_sender
                                .send((tls_socket.clone(), kill_tls_token.clone()));
                            this.kill_tls.awake();

                            *this.inner = InnerInitTls::Complete;

                            // Exit loop: connection already setup.
                            // Nothing to do.
                            return Poll::Ready(Ok((tls_socket, kill_tls_token)));
                        }
                        TlsState::Establishing {
                            sender: _,
                            kill: active_kill_tls_token,
                        } => {
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
                        }
                        TlsState::None | TlsState::Blocked => {
                            drop(w_tls_state);

                            this.tls_socket_sender.close();
                            this.kill_tls.awake();

                            *this.inner = InnerInitTls::Complete;

                            // Exit loop: connection timed out.
                            return Poll::Ready(Err(errors::SocketError::Timeout(
                                errors::SocketType::Tls,
                                errors::SocketStage::Initialization,
                            )));
                        }
                    }
                }
                InnerInitTlsProj::WriteNone {
                    reason: CleanupReason::Killed,
                } => {
                    let mut w_tls_state = this.socket.state().write().unwrap();
                    match &*w_tls_state {
                        TlsState::Establishing {
                            sender: _,
                            kill: active_kill_tls_token,
                        } => {
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
                        }
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
                        }
                    }
                }
                InnerInitTlsProj::WriteManaged { tls_socket } => {
                    let mut w_tls_state = this.socket.state().write().unwrap();
                    match &*w_tls_state {
                        TlsState::Establishing {
                            sender: active_sender,
                            kill: active_kill_tls_token,
                        } => {
                            // If we are the one who set the state to Establishing...
                            if this.kill_tls.same_awake_token(active_kill_tls_token) {
                                *w_tls_state = TlsState::Managed {
                                    socket: tls_socket.clone(),
                                    kill: this.kill_tls.get_awake_token(),
                                };
                                drop(w_tls_state);

                                let _ = this
                                    .tls_socket_sender
                                    .send((tls_socket.clone(), this.kill_tls.get_awake_token()));

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
                        }
                        TlsState::Managed { socket, kill } => {
                            let tls_socket = socket.clone();
                            let kill_tls_token = kill.clone();
                            drop(w_tls_state);

                            let _ = this
                                .tls_socket_sender
                                .send((tls_socket.clone(), kill_tls_token.clone()));
                            // Shutdown the listener we started.
                            this.kill_tls.awake();

                            *this.inner = InnerInitTls::Complete;

                            // Exit loop: connection already setup.
                            // Nothing to do.
                            return Poll::Ready(Ok((tls_socket, kill_tls_token)));
                        }
                        TlsState::None | TlsState::Blocked => {
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
                        }
                    }
                }
                InnerInitTlsProj::GetEstablishing {
                    mut receive_tls_socket,
                } => {
                    match receive_tls_socket.as_mut().poll(cx) {
                        Poll::Ready(Ok((tls_socket, kill_tls_token))) => {
                            let _ = this
                                .tls_socket_sender
                                .send((tls_socket.clone(), kill_tls_token.clone()));
                            this.kill_tls.awake();

                            *this.inner = InnerInitTls::Complete;

                            // Exit loop: connection setup completed and
                            // registered by a different init process.
                            return Poll::Ready(Ok((tls_socket, kill_tls_token)));
                        }
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
                        }
                        Poll::Pending => {
                            // Exit loop. Will be woken up once a TLS write
                            // handle is received or the timeout condition
                            // occurs. Cannot be killed because it may have
                            // already been killed by self.
                            return Poll::Pending;
                        }
                    }
                }
                InnerInitTlsProj::Complete => panic!("InitTls was polled after completion"),
            }
        }
    }
}

#[pinned_drop]
impl<'a, 'd, S> PinnedDrop for InitTls<'a, 'd, S>
where
    S: TlsSocket,
{
    fn drop(self: Pin<&mut Self>) {
        match &self.inner {
            InnerInitTls::Fresh
            | InnerInitTls::WriteEstablishing
            | InnerInitTls::GetEstablishing {
                receive_tls_socket: _,
            }
            | InnerInitTls::Complete => {
                // Nothing to do.
            }
            InnerInitTls::ConnectingTcp(_)
            | InnerInitTls::ConnectingTls(_)
            | InnerInitTls::WriteNone { reason: _ } => {
                let mut w_tls_state = self.socket.state().write().unwrap();
                match &*w_tls_state {
                    TlsState::Establishing {
                        sender: _,
                        kill: active_kill_tls_token,
                    } => {
                        // If we are the one who set the state to Establishing...
                        if self.kill_tls.same_awake_token(active_kill_tls_token) {
                            *w_tls_state = TlsState::None;
                        }
                        drop(w_tls_state);
                    }
                    TlsState::Managed { socket: _, kill: _ }
                    | TlsState::None
                    | TlsState::Blocked => {
                        drop(w_tls_state);
                    }
                }
            }
            // If this struct is dropped while it is trying to write the
            // connection to the TlsState, we will spawn a task to complete
            // this operation. This way, those that depend on receiving this
            // the connection don't unexpectedly receive errors and try to
            // re-initialize the connection.
            InnerInitTls::WriteManaged { tls_socket } => {
                let mut w_tls_state = self.socket.state().write().unwrap();
                match &*w_tls_state {
                    TlsState::Establishing {
                        sender: _,
                        kill: active_kill_tls_token,
                    } => {
                        // If we are the one who set the state to Establishing...
                        if self.kill_tls.same_awake_token(active_kill_tls_token) {
                            *w_tls_state = TlsState::Managed {
                                socket: tls_socket.clone(),
                                kill: self.kill_tls.get_awake_token(),
                            };
                            drop(w_tls_state);

                            // Ignore send errors. They just indicate that all receivers have been dropped.
                            let _ = self
                                .tls_socket_sender
                                .send((tls_socket.clone(), self.kill_tls.get_awake_token()));
                        // If some other process set the state to Establishing...
                        } else {
                            drop(w_tls_state);

                            // Shutdown the listener we started.
                            self.kill_tls.awake();
                        }
                    }
                    TlsState::Managed { socket: _, kill: _ }
                    | TlsState::None
                    | TlsState::Blocked => {
                        drop(w_tls_state);

                        // Shutdown the listener we started.
                        self.kill_tls.awake();
                    }
                }
            }
        }
    }
}

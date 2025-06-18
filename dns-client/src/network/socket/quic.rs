use std::{
    future::Future,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    pin::Pin,
    sync::Arc,
    task::Poll,
    time::Duration,
};

use async_lib::{
    awake_token::{AwakeToken, AwokenToken, SameAwakeToken},
    once_watch::{self, OnceWatchSend, OnceWatchSubscribe},
};
use async_trait::async_trait;
use dns_lib::types::c_domain_name::CDomainName;
use pin_project::{pin_project, pinned_drop};
use quinn::Connecting;
use tokio::{task::JoinHandle, time::Sleep};

use crate::network::{errors, mixed_tcp_udp::TCP_INIT_TIMEOUT};

use super::{FutureSocket, PollSocket};

pub(crate) enum QuicState {
    Managed {
        socket: Arc<quinn::Connection>,
        kill: AwakeToken,
    },
    Establishing {
        sender: once_watch::Sender<(Arc<quinn::Connection>, AwakeToken)>,
        kill: AwakeToken,
    },
    None,
    Blocked,
}

#[async_trait]
pub(crate) trait QuicSocket
where
    Self: 'static + Sized + Send + Sync,
{
    fn peer_addr(&self) -> SocketAddr;
    fn peer_name(&self) -> &CDomainName;
    fn state(&self) -> &std::sync::RwLock<QuicState>;
    fn client_config(&self) -> &Arc<quinn::ClientConfig>;

    /// Start the QUIC listener and drive the QUIC state to Managed.
    #[inline]
    async fn start(self: Arc<Self>) -> Result<(), errors::SocketError> {
        match self.init().await {
            Ok(_) => Ok(()),
            Err(error) => Err(error),
        }
    }

    /// Start the QUIC listener and drive the QUIC state to Managed.
    /// Returns a reference to the created QUIC stream.
    #[inline]
    async fn init(
        self: Arc<Self>,
    ) -> Result<(Arc<quinn::Connection>, AwakeToken), errors::SocketError> {
        InitQuic::new(&self, None).await
    }

    /// Shut down the QUIC listener and drive the QUIC state to None.
    #[inline]
    async fn shutdown(self: Arc<Self>) {
        let receiver;
        {
            let mut w_state = self.state().write().unwrap();
            match &*w_state {
                QuicState::Managed { socket: _, kill } => {
                    let quic_kill = kill.clone();
                    *w_state = QuicState::None;
                    drop(w_state);

                    quic_kill.awake();

                    // Note: this task is not responsible for actual cleanup. Once the listener closes, it
                    // will kill any active queries and change the QuicState.
                    return;
                }
                QuicState::Establishing { sender, kill } => {
                    let sender = sender.clone();
                    let kill_init_quic = kill.clone();
                    *w_state = QuicState::None;
                    drop(w_state);

                    // Try to prevent the socket from being initialized.
                    kill_init_quic.awake();
                    sender.close();
                    receiver = sender.subscribe();
                }
                QuicState::None | QuicState::Blocked => {
                    // Already shut down
                    drop(w_state);
                    return;
                }
            }
        }

        // If the socket still initialized, shut it down immediately.
        if let Ok((_, kill_quic)) = receiver.await {
            kill_quic.awake();
        } // else, successful cancellation
    }

    /// If the QUIC state is Blocked, changes it to None.
    #[inline]
    async fn enable(self: Arc<Self>) {
        let mut w_state = self.state().write().unwrap();
        match &*w_state {
            QuicState::Managed { socket: _, kill: _ } => (), //< Already enabled
            QuicState::Establishing { sender: _, kill: _ } => (), //< Already enabled
            QuicState::None => (),                           //< Already enabled
            QuicState::Blocked => *w_state = QuicState::None,
        }
        drop(w_state);
    }

    /// Sets the QUIC state to Blocked, shutting down the socket if needed.
    #[inline]
    async fn disable(self: Arc<Self>) {
        let receiver;
        {
            let mut w_state = self.state().write().unwrap();
            match &*w_state {
                QuicState::Managed { socket: _, kill } => {
                    let kill_quic = kill.clone();
                    *w_state = QuicState::Blocked;
                    drop(w_state);

                    kill_quic.awake();

                    // Note: this task is not responsible for actual cleanup. Once the listener closes, it
                    // will kill any active queries and change the QuicState.
                    return;
                }
                QuicState::Establishing { sender, kill } => {
                    let sender = sender.clone();
                    let kill_init_quic = kill.clone();
                    *w_state = QuicState::Blocked;
                    drop(w_state);

                    // Try to prevent the socket from being initialized.
                    kill_init_quic.awake();
                    sender.close();
                    receiver = sender.subscribe();
                }
                QuicState::None => {
                    *w_state = QuicState::Blocked;
                    drop(w_state);
                    return;
                }
                QuicState::Blocked => {
                    // Already disabled
                    drop(w_state);
                    return;
                }
            }
        }

        // If the socket still initialized, shut it down immediately.
        if let Ok((_, kill_quic)) = receiver.await {
            kill_quic.awake();
        } // else, successful cancellation
    }

    /// Starts a QUIC listener to listen for the kill token or for the socket to be closed. This
    /// processes should stop when the `kill_quic` token is awoken. This function is intended to be
    /// run as a semi-independent background task.
    async fn listen(self: Arc<Self>, quic_socket: Arc<quinn::Connection>, kill_quic: AwakeToken);
}

#[pin_project(project = QQuicSocketProj)]
pub(crate) enum QQuicSocket {
    Fresh,
    GetQuicEstablishing {
        #[pin]
        receive_quic_socket: once_watch::Receiver<(Arc<quinn::Connection>, AwakeToken)>,
    },
    InitQuic {
        #[pin]
        join_handle: JoinHandle<Result<(Arc<quinn::Connection>, AwakeToken), errors::SocketError>>,
    },
    Acquired {
        quic_socket: Arc<quinn::Connection>,
        #[pin]
        kill_quic: AwokenToken,
    },
    Closed(errors::SocketError),
}

impl<'a> QQuicSocket {
    #[inline]
    pub fn set_get_quic_establishing(
        mut self: std::pin::Pin<&mut Self>,
        receiver: once_watch::Receiver<(Arc<quinn::Connection>, AwakeToken)>,
    ) {
        self.set(Self::GetQuicEstablishing {
            receive_quic_socket: receiver,
        });
    }

    #[inline]
    pub fn set_init_quic<S: QuicSocket>(mut self: std::pin::Pin<&mut Self>, socket: &'a Arc<S>) {
        let init_quic = tokio::spawn(socket.clone().init());

        self.set(Self::InitQuic {
            join_handle: init_quic,
        });
    }

    #[inline]
    pub fn set_acquired(
        mut self: std::pin::Pin<&mut Self>,
        quic_socket: Arc<quinn::Connection>,
        kill_quic_token: AwakeToken,
    ) {
        self.set(Self::Acquired {
            quic_socket,
            kill_quic: kill_quic_token.awoken(),
        });
    }

    #[inline]
    pub fn set_closed(mut self: std::pin::Pin<&mut Self>, reason: errors::SocketError) {
        self.set(Self::Closed(reason));
    }
}

impl<'a, 'd, S: QuicSocket> FutureSocket<'a, 'd, S, errors::SocketError> for QQuicSocket {
    fn poll(
        self: &mut Pin<&mut Self>,
        socket: &'a Arc<S>,
        cx: &mut std::task::Context<'_>,
    ) -> PollSocket<errors::SocketError>
    where
        'a: 'd,
    {
        match self.as_mut().project() {
            QQuicSocketProj::Fresh => {
                let r_quic_state = socket.state().read().unwrap();
                match &*r_quic_state {
                    QuicState::Managed { socket, kill } => {
                        let udp_socket = socket.clone();
                        let kill_udp = kill.clone();
                        drop(r_quic_state);

                        self.as_mut().set_acquired(udp_socket, kill_udp);

                        // Next loop should poll `kill_quic`
                        PollSocket::Continue
                    }
                    QuicState::Establishing { sender, kill: _ } => {
                        let sender = sender.subscribe();
                        drop(r_quic_state);

                        self.as_mut().set_get_quic_establishing(sender);

                        // Next loop should poll `receive_quic_socket`
                        PollSocket::Continue
                    }
                    QuicState::None => {
                        drop(r_quic_state);

                        self.as_mut().set_init_quic(socket);

                        // Next loop should poll `join_handle`
                        PollSocket::Continue
                    }
                    QuicState::Blocked => {
                        drop(r_quic_state);

                        let error = errors::SocketError::Disabled(
                            errors::SocketType::Quic,
                            errors::SocketStage::Initialization,
                        );

                        self.as_mut().set_closed(error.clone());

                        PollSocket::Error(error)
                    }
                }
            }
            QQuicSocketProj::GetQuicEstablishing {
                mut receive_quic_socket,
            } => {
                match receive_quic_socket.as_mut().poll(cx) {
                    Poll::Ready(Ok((quic_socket, quic_kill))) => {
                        self.as_mut().set_acquired(quic_socket, quic_kill);

                        // Next loop should poll `kill_quic`
                        PollSocket::Continue
                    }
                    Poll::Ready(Err(once_watch::RecvError::Closed)) => {
                        let error = errors::SocketError::Shutdown(
                            errors::SocketType::Quic,
                            errors::SocketStage::Initialization,
                        );

                        self.as_mut().set_closed(error.clone());

                        PollSocket::Error(error)
                    }
                    Poll::Pending => PollSocket::Pending,
                }
            }
            QQuicSocketProj::InitQuic { mut join_handle } => {
                match join_handle.as_mut().poll(cx) {
                    Poll::Ready(Ok(Ok((quic_socket, kill_quic_token)))) => {
                        self.as_mut().set_acquired(quic_socket, kill_quic_token);

                        // Next loop should poll `kill_quic`
                        PollSocket::Continue
                    }
                    Poll::Ready(Ok(Err(error))) => {
                        self.as_mut().set_closed(error.clone());

                        PollSocket::Error(error)
                    }
                    Poll::Ready(Err(join_error)) => {
                        let error = errors::SocketError::from((
                            errors::SocketType::Quic,
                            errors::SocketStage::Initialization,
                            join_error,
                        ));

                        self.as_mut().set_closed(error.clone());

                        PollSocket::Error(error)
                    }
                    Poll::Pending => PollSocket::Pending,
                }
            }
            QQuicSocketProj::Acquired {
                quic_socket: _,
                mut kill_quic,
            } => match kill_quic.as_mut().poll(cx) {
                Poll::Ready(()) => {
                    let error = errors::SocketError::Shutdown(
                        errors::SocketType::Quic,
                        errors::SocketStage::Connected,
                    );

                    self.as_mut().set_closed(error.clone());

                    PollSocket::Error(error)
                }
                Poll::Pending => PollSocket::Pending,
            },
            QQuicSocketProj::Closed(error) => PollSocket::Error(error.clone()),
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
struct InitQuic<'a, S>
where
    S: QuicSocket,
{
    socket: &'a Arc<S>,
    #[pin]
    kill_quic: AwokenToken,
    quic_socket_sender: once_watch::Sender<(Arc<quinn::Connection>, AwakeToken)>,
    #[pin]
    timeout: Sleep,
    #[pin]
    inner: InnerInitQuic,
}

#[pin_project(project = InnerInitQuicProj)]
enum InnerInitQuic {
    Fresh,
    WriteEstablishing,
    ConnectingQuic(#[pin] Connecting),
    WriteNone {
        reason: CleanupReason<errors::SocketError>,
    },
    WriteManaged {
        quic_socket: Arc<quinn::Connection>,
    },
    GetEstablishing {
        #[pin]
        receive_quic_socket: once_watch::Receiver<(Arc<quinn::Connection>, AwakeToken)>,
    },
    Complete,
}

impl<'a, S> InitQuic<'a, S>
where
    S: QuicSocket,
{
    #[inline]
    pub fn new(socket: &'a Arc<S>, timeout: Option<Duration>) -> Self {
        let kill_quic_token = AwakeToken::new();
        let quic_socket_sender = once_watch::Sender::new();
        let timeout = timeout.unwrap_or(TCP_INIT_TIMEOUT);

        Self {
            socket,
            kill_quic: kill_quic_token.awoken(),
            quic_socket_sender,
            timeout: tokio::time::sleep(timeout),
            inner: InnerInitQuic::Fresh,
        }
    }

    #[inline]
    fn poll_timeouts(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Result<(), errors::SocketError> {
        let mut this = self.as_mut().project();
        match this.inner.as_mut().project() {
            InnerInitQuicProj::Fresh | InnerInitQuicProj::WriteEstablishing => {
                if let Poll::Ready(()) = this.kill_quic.as_mut().poll(cx) {
                    this.quic_socket_sender.close();
                    this.kill_quic.awake();
                    let error = errors::SocketError::Shutdown(
                        errors::SocketType::Quic,
                        errors::SocketStage::Initialization,
                    );

                    *this.inner = InnerInitQuic::Complete;

                    // Query killed.
                    return Err(error);
                }

                if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                    this.quic_socket_sender.close();
                    this.kill_quic.awake();
                    let error = errors::SocketError::Timeout(
                        errors::SocketType::Quic,
                        errors::SocketStage::Initialization,
                    );

                    *this.inner = InnerInitQuic::Complete;

                    // Query timed out.
                    return Err(error);
                }

                Ok(())
            }
            InnerInitQuicProj::ConnectingQuic(_) => {
                if let Poll::Ready(()) = this.kill_quic.as_mut().poll(cx) {
                    *this.inner = InnerInitQuic::WriteNone {
                        reason: CleanupReason::Timeout,
                    };

                    // First loop: poll the write lock.
                    return Ok(());
                } else if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                    *this.inner = InnerInitQuic::WriteNone {
                        reason: CleanupReason::Killed,
                    };

                    // First loop: poll the write lock.
                    return Ok(());
                }

                Ok(())
            }
            InnerInitQuicProj::GetEstablishing {
                receive_quic_socket: _,
            } => {
                // Does not poll `kill_quic` because that gets awoken to kill
                // the listener (if it is set up).
                if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                    this.quic_socket_sender.close();
                    this.kill_quic.awake();
                    let error = errors::SocketError::Timeout(
                        errors::SocketType::Quic,
                        errors::SocketStage::Initialization,
                    );

                    *this.inner = InnerInitQuic::Complete;

                    // Exit loop: query timed out.
                    return Err(error);
                }

                Ok(())
            }
            InnerInitQuicProj::WriteNone { reason: _ }
            | InnerInitQuicProj::WriteManaged { quic_socket: _ }
            | InnerInitQuicProj::Complete => {
                // Not allowed to timeout or be killed. These are cleanup
                // states.
                Ok(())
            }
        }
    }
}

impl<'a, S> Future for InitQuic<'a, S>
where
    S: QuicSocket,
{
    type Output = Result<(Arc<quinn::Connection>, AwakeToken), errors::SocketError>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        if let Err(error) = self.as_mut().poll_timeouts(cx) {
            return Poll::Ready(Err(error));
        }

        loop {
            let mut this = self.as_mut().project();
            match this.inner.as_mut().project() {
                InnerInitQuicProj::Fresh | InnerInitQuicProj::WriteEstablishing => {
                    let mut w_quic_state = this.socket.state().write().unwrap();
                    match &*w_quic_state {
                        QuicState::Managed { socket, kill } => {
                            let quic_socket = socket.clone();
                            let kill_quic_token = kill.clone();
                            drop(w_quic_state);

                            let _ = this
                                .quic_socket_sender
                                .send((quic_socket.clone(), kill_quic_token.clone()));
                            this.kill_quic.awake();

                            *this.inner = InnerInitQuic::Complete;

                            // Exit loop: connection already setup.
                            // Nothing to do.
                            return Poll::Ready(Ok((quic_socket, kill_quic_token)));
                        }
                        QuicState::Establishing {
                            sender: active_sender,
                            kill: _,
                        } => {
                            let receive_quic_socket = active_sender.subscribe();
                            drop(w_quic_state);

                            *this.inner = InnerInitQuic::GetEstablishing {
                                receive_quic_socket,
                            };

                            // Next loop: poll the receiver. Another
                            // process is setting up the connection.
                            continue;
                        }
                        QuicState::None => {
                            let quic_socket_sender = this.quic_socket_sender.clone();
                            let kill_init_quic = this.kill_quic.get_awake_token();

                            *w_quic_state = QuicState::Establishing {
                                sender: quic_socket_sender,
                                kill: kill_init_quic,
                            };
                            drop(w_quic_state);

                            let local_addr = match this.socket.peer_addr() {
                                SocketAddr::V4(_) => {
                                    SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0)
                                }
                                SocketAddr::V6(_) => {
                                    SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0)
                                }
                            };
                            let endpoint = match quinn::Endpoint::client(local_addr) {
                                Ok(endpoint) => endpoint,
                                Err(error) => {
                                    let io_error = errors::IoError::from(error);
                                    let socket_error = errors::SocketError::Io {
                                        socket_type: errors::SocketType::Quic,
                                        socket_stage: errors::SocketStage::Initialization,
                                        error: io_error,
                                    };

                                    *this.inner = InnerInitQuic::Complete;

                                    // Exit loop: connection error.
                                    return Poll::Ready(Err(socket_error));
                                }
                            };
                            let init_connection = match endpoint.connect_with(
                                (**this.socket.client_config()).clone(),
                                this.socket.peer_addr(),
                                &this.socket.peer_name().to_string(),
                            ) {
                                Ok(connection) => connection,
                                Err(error) => {
                                    let socket_error = errors::SocketError::QuicConnect {
                                        socket_stage: errors::SocketStage::Initialization,
                                        error,
                                    };

                                    *this.inner = InnerInitQuic::Complete;

                                    // Exit loop: connection error.
                                    return Poll::Ready(Err(socket_error));
                                }
                            };

                            *this.inner = InnerInitQuic::ConnectingQuic(init_connection);

                            // Next loop: poll the QUIC stream and start
                            // connecting.
                            continue;
                        }
                        QuicState::Blocked => {
                            drop(w_quic_state);

                            this.quic_socket_sender.close();
                            this.kill_quic.awake();
                            let error = errors::SocketError::Disabled(
                                errors::SocketType::Quic,
                                errors::SocketStage::Initialization,
                            );

                            *this.inner = InnerInitQuic::Complete;

                            // Exit loop: connection not allowed.
                            return Poll::Ready(Err(error));
                        }
                    }
                }
                InnerInitQuicProj::ConnectingQuic(mut init_connection) => {
                    match init_connection.as_mut().poll(cx) {
                        Poll::Ready(Ok(quic_socket)) => {
                            let quic_socket = Arc::new(quic_socket);
                            tokio::spawn(
                                this.socket
                                    .clone()
                                    .listen(quic_socket.clone(), this.kill_quic.get_awake_token()),
                            );

                            *this.inner = InnerInitQuic::WriteManaged { quic_socket };

                            // Next loop: poll the write lock.
                            continue;
                        }
                        Poll::Ready(Err(error)) => {
                            let error = errors::SocketError::QuicConnection {
                                socket_stage: errors::SocketStage::Initialization,
                                error,
                            };
                            println!("{error:?}");

                            *this.inner = InnerInitQuic::WriteNone {
                                reason: CleanupReason::ConnectionError(error),
                            };

                            // Next loop: poll the write lock.
                            continue;
                        }
                        Poll::Pending => {
                            // Exit loop. Will be woken up once QUIC is
                            // connected, the timeout condition occurs, or the
                            // connection is killed.
                            return Poll::Pending;
                        }
                    }
                }
                InnerInitQuicProj::WriteNone {
                    reason: CleanupReason::ConnectionError(error),
                } => {
                    let mut w_quic_state = this.socket.state().write().unwrap();
                    match &*w_quic_state {
                        QuicState::Managed { socket, kill } => {
                            let quic_socket = socket.clone();
                            let kill_quic_token = kill.clone();
                            drop(w_quic_state);

                            let _ = this
                                .quic_socket_sender
                                .send((quic_socket.clone(), kill_quic_token.clone()));
                            this.kill_quic.awake();

                            *this.inner = InnerInitQuic::Complete;

                            // Exit loop: connection already setup.
                            // Nothing to do.
                            return Poll::Ready(Ok((quic_socket, kill_quic_token)));
                        }
                        QuicState::Establishing {
                            sender,
                            kill: active_kill_quic_token,
                        } => {
                            // If we are the one who set the state to Establishing...
                            if this.kill_quic.same_awake_token(active_kill_quic_token) {
                                *w_quic_state = QuicState::None;
                                drop(w_quic_state);

                                this.quic_socket_sender.close();
                                this.kill_quic.awake();
                                let error = error.clone();

                                *this.inner = InnerInitQuic::Complete;

                                // Exit loop: we received a connection
                                // error.
                                return Poll::Ready(Err(error));
                            // If some other process set the state to Establishing...
                            } else {
                                let receive_quic_socket = sender.subscribe();
                                drop(w_quic_state);

                                *this.inner = InnerInitQuic::GetEstablishing {
                                    receive_quic_socket,
                                };

                                // Next loop: poll the receiver.
                                continue;
                            }
                        }
                        QuicState::None | QuicState::Blocked => {
                            drop(w_quic_state);

                            this.quic_socket_sender.close();
                            this.kill_quic.awake();
                            let error = error.clone();

                            *this.inner = InnerInitQuic::Complete;

                            // Exit loop: we received a connection
                            // error.
                            return Poll::Ready(Err(error));
                        }
                    }
                }
                InnerInitQuicProj::WriteNone {
                    reason: CleanupReason::Timeout,
                } => {
                    let mut w_quic_state = this.socket.state().write().unwrap();
                    match &*w_quic_state {
                        QuicState::Managed { socket, kill } => {
                            let quic_socket = socket.clone();
                            let kill_quic_token = kill.clone();
                            drop(w_quic_state);

                            let _ = this
                                .quic_socket_sender
                                .send((quic_socket.clone(), kill_quic_token.clone()));
                            this.kill_quic.awake();

                            *this.inner = InnerInitQuic::Complete;

                            // Exit loop: connection already setup.
                            // Nothing to do.
                            return Poll::Ready(Ok((quic_socket, kill_quic_token)));
                        }
                        QuicState::Establishing {
                            sender: _,
                            kill: active_kill_quic_token,
                        } => {
                            // If we are the one who set the state to Establishing...
                            if this.kill_quic.same_awake_token(active_kill_quic_token) {
                                *w_quic_state = QuicState::None;
                            }
                            drop(w_quic_state);

                            this.quic_socket_sender.close();
                            this.kill_quic.awake();

                            *this.inner = InnerInitQuic::Complete;

                            // Exit loop: connection timed out.
                            return Poll::Ready(Err(errors::SocketError::Timeout(
                                errors::SocketType::Quic,
                                errors::SocketStage::Initialization,
                            )));
                        }
                        QuicState::None | QuicState::Blocked => {
                            drop(w_quic_state);

                            this.quic_socket_sender.close();
                            this.kill_quic.awake();

                            *this.inner = InnerInitQuic::Complete;

                            // Exit loop: connection timed out.
                            return Poll::Ready(Err(errors::SocketError::Timeout(
                                errors::SocketType::Quic,
                                errors::SocketStage::Initialization,
                            )));
                        }
                    }
                }
                InnerInitQuicProj::WriteNone {
                    reason: CleanupReason::Killed,
                } => {
                    let mut w_quic_state = this.socket.state().write().unwrap();
                    match &*w_quic_state {
                        QuicState::Establishing {
                            sender: _,
                            kill: active_kill_quic_token,
                        } => {
                            // If we are the one who set the state to Establishing...
                            if this.kill_quic.same_awake_token(active_kill_quic_token) {
                                *w_quic_state = QuicState::None;
                            }
                            drop(w_quic_state);

                            this.quic_socket_sender.close();
                            this.kill_quic.awake();

                            *this.inner = InnerInitQuic::Complete;

                            // Exit loop: connection killed.
                            return Poll::Ready(Err(errors::SocketError::Shutdown(
                                errors::SocketType::Quic,
                                errors::SocketStage::Initialization,
                            )));
                        }
                        QuicState::Managed { socket: _, kill: _ }
                        | QuicState::None
                        | QuicState::Blocked => {
                            drop(w_quic_state);

                            this.quic_socket_sender.close();
                            this.kill_quic.awake();

                            *this.inner = InnerInitQuic::Complete;

                            // Exit loop: connection killed.
                            return Poll::Ready(Err(errors::SocketError::Shutdown(
                                errors::SocketType::Quic,
                                errors::SocketStage::Initialization,
                            )));
                        }
                    }
                }
                InnerInitQuicProj::WriteManaged { quic_socket } => {
                    let mut w_quic_state = this.socket.state().write().unwrap();
                    match &*w_quic_state {
                        QuicState::Establishing {
                            sender: active_sender,
                            kill: active_kill_quic_token,
                        } => {
                            // If we are the one who set the state to Establishing...
                            if this.kill_quic.same_awake_token(active_kill_quic_token) {
                                *w_quic_state = QuicState::Managed {
                                    socket: quic_socket.clone(),
                                    kill: this.kill_quic.get_awake_token(),
                                };
                                drop(w_quic_state);

                                let _ = this
                                    .quic_socket_sender
                                    .send((quic_socket.clone(), this.kill_quic.get_awake_token()));

                                let quic_socket = quic_socket.clone();
                                let kill_quic_token = this.kill_quic.get_awake_token();

                                *this.inner = InnerInitQuic::Complete;

                                // Exit loop: connection setup
                                // completed and registered.
                                return Poll::Ready(Ok((quic_socket, kill_quic_token)));
                            // If some other process set the state to Establishing...
                            } else {
                                let receive_quic_socket = active_sender.subscribe();
                                drop(w_quic_state);

                                // Shutdown the listener we started.
                                this.kill_quic.awake();

                                *this.inner = InnerInitQuic::GetEstablishing {
                                    receive_quic_socket,
                                };

                                // Next loop: poll the receiver.
                                continue;
                            }
                        }
                        QuicState::Managed { socket, kill } => {
                            let quic_socket = socket.clone();
                            let kill_quic_token = kill.clone();
                            drop(w_quic_state);

                            let _ = this
                                .quic_socket_sender
                                .send((quic_socket.clone(), kill_quic_token.clone()));
                            // Shutdown the listener we started.
                            this.kill_quic.awake();

                            *this.inner = InnerInitQuic::Complete;

                            // Exit loop: connection already setup.
                            // Nothing to do.
                            return Poll::Ready(Ok((quic_socket, kill_quic_token)));
                        }
                        QuicState::None | QuicState::Blocked => {
                            drop(w_quic_state);

                            this.quic_socket_sender.close();
                            // Shutdown the listener we started.
                            this.kill_quic.awake();

                            *this.inner = InnerInitQuic::Complete;

                            // Exit loop: state changed after this task
                            // set it to Establishing. Indicates that
                            // this task is no longer in charge.
                            return Poll::Ready(Err(errors::SocketError::Shutdown(
                                errors::SocketType::Quic,
                                errors::SocketStage::Initialization,
                            )));
                        }
                    }
                }
                InnerInitQuicProj::GetEstablishing {
                    mut receive_quic_socket,
                } => {
                    match receive_quic_socket.as_mut().poll(cx) {
                        Poll::Ready(Ok((quic_socket, kill_quic_token))) => {
                            let _ = this
                                .quic_socket_sender
                                .send((quic_socket.clone(), kill_quic_token.clone()));
                            this.kill_quic.awake();

                            *this.inner = InnerInitQuic::Complete;

                            // Exit loop: connection setup completed and
                            // registered by a different init process.
                            return Poll::Ready(Ok((quic_socket, kill_quic_token)));
                        }
                        Poll::Ready(Err(once_watch::RecvError::Closed)) => {
                            this.quic_socket_sender.close();
                            this.kill_quic.awake();

                            *this.inner = InnerInitQuic::Complete;

                            // Exit loop: all senders were dropped so it is not
                            // possible to receive a connection.
                            return Poll::Ready(Err(errors::SocketError::Shutdown(
                                errors::SocketType::Quic,
                                errors::SocketStage::Initialization,
                            )));
                        }
                        Poll::Pending => {
                            // Exit loop. Will be woken up once a QUIC write
                            // handle is received or the timeout condition
                            // occurs. Cannot be killed because it may have
                            // already been killed by self.
                            return Poll::Pending;
                        }
                    }
                }
                InnerInitQuicProj::Complete => panic!("InitQuic was polled after completion"),
            }
        }
    }
}

#[pinned_drop]
impl<'a, S> PinnedDrop for InitQuic<'a, S>
where
    S: QuicSocket,
{
    fn drop(self: Pin<&mut Self>) {
        match &self.inner {
            InnerInitQuic::Fresh
            | InnerInitQuic::WriteEstablishing
            | InnerInitQuic::GetEstablishing {
                receive_quic_socket: _,
            }
            | InnerInitQuic::Complete => {
                // Nothing to do.
            }
            InnerInitQuic::ConnectingQuic(_) | InnerInitQuic::WriteNone { reason: _ } => {
                let mut w_quic_state = self.socket.state().write().unwrap();
                match &*w_quic_state {
                    QuicState::Establishing {
                        sender: _,
                        kill: active_kill_quic_token,
                    } => {
                        // If we are the one who set the state to Establishing...
                        if self.kill_quic.same_awake_token(active_kill_quic_token) {
                            *w_quic_state = QuicState::None;
                        }
                        drop(w_quic_state);
                    }
                    QuicState::Managed { socket: _, kill: _ }
                    | QuicState::None
                    | QuicState::Blocked => {
                        drop(w_quic_state);
                    }
                }
            }
            // If this struct is dropped while it is trying to write the
            // connection to the QuicState, we will spawn a task to complete
            // this operation. This way, those that depend on receiving this
            // the connection don't unexpectedly receive errors and try to
            // re-initialize the connection.
            InnerInitQuic::WriteManaged { quic_socket } => {
                let mut w_quic_state = self.socket.state().write().unwrap();
                match &*w_quic_state {
                    QuicState::Establishing {
                        sender: _,
                        kill: active_kill_quic_token,
                    } => {
                        // If we are the one who set the state to Establishing...
                        if self.kill_quic.same_awake_token(active_kill_quic_token) {
                            *w_quic_state = QuicState::Managed {
                                socket: quic_socket.clone(),
                                kill: self.kill_quic.get_awake_token(),
                            };
                            drop(w_quic_state);

                            // Ignore send errors. They just indicate that all receivers have been dropped.
                            let _ = self
                                .quic_socket_sender
                                .send((quic_socket.clone(), self.kill_quic.get_awake_token()));
                        // If some other process set the state to Establishing...
                        } else {
                            drop(w_quic_state);

                            // Shutdown the listener we started.
                            self.kill_quic.awake();
                        }
                    }
                    QuicState::Managed { socket: _, kill: _ }
                    | QuicState::None
                    | QuicState::Blocked => {
                        drop(w_quic_state);

                        // Shutdown the listener we started.
                        self.kill_quic.awake();
                    }
                }
            }
        }
    }
}

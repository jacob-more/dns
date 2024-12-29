use std::{future::Future, net::SocketAddr, pin::Pin, sync::Arc, task::Poll};

use async_lib::awake_token::{AwakeToken, AwokenToken};
use async_trait::async_trait;
use futures::{future::BoxFuture, FutureExt};
use pin_project::pin_project;
use tokio::{net, sync::{RwLock, RwLockReadGuard, RwLockWriteGuard}};

use crate::errors;

use super::{FutureSocket, PollSocket};


pub(crate) enum UdpState {
    Managed(Arc<net::UdpSocket>, AwakeToken),
    None,
    Blocked,
}

#[async_trait]
pub(crate) trait UdpSocket where Self: 'static + Sized + Send + Sync {
    fn peer(&self) -> &SocketAddr;
    fn state(&self) -> &RwLock<UdpState>;

    /// Start the UDP listener and drive the UDP state to Managed.
    #[inline]
    async fn start(self: Arc<Self>) -> Result<(), errors::UdpInitError> {
        match self.init().await {
            Ok(_) => Ok(()),
            Err(error) => Err(error),
        }
    }

    /// Start the UDP listener and drive the UDP state to Managed.
    /// Returns a reference to the created UDP socket.
    #[inline]
    async fn init(self: Arc<Self>) -> Result<(Arc<net::UdpSocket>, AwakeToken), errors::UdpInitError> {
        // Initially, verify if the connection has already been established.
        let r_state = self.state().read().await;
        match &*r_state {
            UdpState::Managed(udp_socket, kill_udp) => return Ok((udp_socket.clone(), kill_udp.clone())),
            UdpState::None => (),
            UdpState::Blocked => {
                drop(r_state);
                return Err(errors::UdpInitError::SocketDisabled);
            },
        }
        drop(r_state);

        let udp_socket = Arc::new(net::UdpSocket::bind("0.0.0.0:0").await?);
        udp_socket.connect(self.peer()).await?;
        let udp_reader = udp_socket.clone();
        let udp_writer = udp_socket;
        let kill_udp = AwakeToken::new();

        // Since there is no intermediate state while the UDP socket is being
        // set up and the lock is dropped, it is possible that another process
        // was doing the same task.

        let mut w_state = self.state().write().await;
        match &*w_state {
            UdpState::Managed(existing_udp_socket, _) => {
                return Ok((existing_udp_socket.clone(), kill_udp));
            },
            UdpState::None => {
                *w_state = UdpState::Managed(udp_writer.clone(), kill_udp.clone());
                drop(w_state);

                tokio::spawn(self.listen(udp_reader, kill_udp.clone()));

                return Ok((udp_writer, kill_udp));
            },
            UdpState::Blocked => {
                drop(w_state);
                return Err(errors::UdpInitError::SocketDisabled);
            },
        }
    }

    /// Shut down the TCP listener and drive the TCP state to None.
    #[inline]
    async fn shutdown(self: Arc<Self>) {
        let r_state = self.state().read().await;
        if let UdpState::Managed(_, kill_udp) = &*r_state {
            let kill_udp = kill_udp.clone();
            drop(r_state);

            kill_udp.awake();

            // Note: this task is not responsible for actual cleanup. Once the listener closes, it
            // will kill any active queries and change the UdpState.
        } else {
            drop(r_state);
        }
    }

    /// If the TCP state is Blocked, changes it to None.
    #[inline]
    async fn enable(self: Arc<Self>) {
        let mut w_state = self.state().write().await;
        match &*w_state {
            UdpState::Managed(_, _) => (),  //< Already enabled
            UdpState::None => (),           //< Already enabled
            UdpState::Blocked => *w_state = UdpState::None,
        }
        drop(w_state);
    }

    /// Sets the TCP state to Blocked, shutting down the socket if needed.
    #[inline]
    async fn disable(self: Arc<Self>) {
        let mut w_state = self.state().write().await;
        match &*w_state {
            UdpState::Managed(_, kill_udp) => {
                // Since we are removing the reference the kill_udp by setting state to Blocked, we
                // need to kill them now since the listener won't be able to kill them.
                let kill_udp = kill_udp.clone();
                *w_state = UdpState::Blocked;
                drop(w_state);

                kill_udp.awake();
            },
            UdpState::None => {
                *w_state = UdpState::Blocked;
                drop(w_state);
            },
            UdpState::Blocked => { //< Already disabled
                drop(w_state);
            },
        }
    }

    /// Starts a UDP listener to read data from the provided socket. This processes should stop
    /// when the `kill_udp` token is awoken. This function is intended to be run as a
    /// semi-independent background task.
    async fn listen(self: Arc<Self>, udp_reader: Arc<net::UdpSocket>, kill_udp: AwakeToken);
}

#[pin_project(project = QUdpSocketProj)]
pub(crate) enum QUdpSocket<'c, 'd>
where
    'd: 'c,
{
    Fresh,
    GetReadUdpState(BoxFuture<'c, RwLockReadGuard<'d, UdpState>>),
    InitUdp(BoxFuture<'c, Result<(Arc<net::UdpSocket>, AwakeToken), errors::UdpInitError>>),
    GetWriteUdpState(BoxFuture<'c, RwLockWriteGuard<'d, UdpState>>, Arc<net::UdpSocket>, AwakeToken),
    Acquired {
        udp_socket: Arc<net::UdpSocket>,
        #[pin]
        kill_udp: AwokenToken,
    },
    Closed(errors::UdpSocketError),
}

impl<'a, 'c, 'd> QUdpSocket<'c, 'd>
where
    'a: 'd,
    'd: 'c,
{
    #[inline]
    fn set_get_read_udp_state<S: UdpSocket>(mut self: std::pin::Pin<&mut Self>, socket: &'a Arc<S>) {
        let r_udp_state = socket.state().read().boxed();

        self.set(QUdpSocket::GetReadUdpState(r_udp_state));
    }

    #[inline]
    fn set_init_udp<S: UdpSocket>(mut self: std::pin::Pin<&mut Self>, socket: &'a Arc<S>) {
        let upstream_socket = socket.peer();
        let init_udp = async move {
            let udp_socket = Arc::new(net::UdpSocket::bind("0.0.0.0:0").await?);
            udp_socket.connect(upstream_socket).await?;
            return Ok((udp_socket, AwakeToken::new()));
        }.boxed();

        self.set(QUdpSocket::InitUdp(init_udp));
    }

    #[inline]
    fn set_get_write_udp_state<S: UdpSocket>(mut self: std::pin::Pin<&mut Self>, socket: &'a Arc<S>, udp_socket: Arc<net::UdpSocket>, kill_udp: AwakeToken) {
        let w_udp_state = socket.state().write().boxed();

        self.set(QUdpSocket::GetWriteUdpState(w_udp_state, udp_socket, kill_udp));
    }

    #[inline]
    fn set_acquired(mut self: std::pin::Pin<&mut Self>, udp_socket: Arc<net::UdpSocket>, kill_udp_token: AwakeToken) {
        self.set(QUdpSocket::Acquired { udp_socket, kill_udp: kill_udp_token.awoken() });
    }

    #[inline]
    fn set_closed(mut self: std::pin::Pin<&mut Self>, reason: errors::UdpSocketError) {
        self.set(QUdpSocket::Closed(reason));
    }
}

impl<'c, 'd, S: UdpSocket> FutureSocket<'d, S, errors::UdpSocketError> for QUdpSocket<'c, 'd> {
    fn poll<'a>(self: &mut Pin<&mut Self>, socket: &'a Arc<S>, cx: &mut std::task::Context<'_>) -> PollSocket<errors::UdpSocketError> where 'a: 'd {
        match self.as_mut().project() {
            QUdpSocketProj::Fresh => {
                self.as_mut().set_get_read_udp_state(socket);

                // Next loop should poll `r_udp_state`
                return PollSocket::Continue;
            },
            QUdpSocketProj::GetReadUdpState(r_udp_state) => {
                match r_udp_state.as_mut().poll(cx) {
                    Poll::Ready(udp_state) => {
                        match &*udp_state {
                            UdpState::Managed(socket, kill) => {
                                self.as_mut().set_acquired(socket.clone(), kill.clone());

                                // Next loop should poll `kill_udp`
                                return PollSocket::Continue;
                            },
                            UdpState::None => {
                                self.as_mut().set_init_udp(socket);

                                // Next loop should poll `init_udp`
                                return PollSocket::Continue;
                            },
                            UdpState::Blocked => {
                                let error = errors::UdpSocketError::Disabled;

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
            QUdpSocketProj::InitUdp(init_udp) => {
                match init_udp.as_mut().poll(cx) {
                    Poll::Ready(Ok((udp_socket, kill_udp_token))) => {
                        tokio::spawn(socket.clone().listen(udp_socket.clone(), kill_udp_token.clone()));
                        self.as_mut().set_get_write_udp_state(socket, udp_socket, kill_udp_token);

                        // Next loop should poll `kill_udp`
                        return PollSocket::Continue;
                    },
                    Poll::Ready(Err(error)) => {
                        let error = errors::UdpSocketError::from(error);

                        self.as_mut().set_closed(error.clone());

                        return PollSocket::Error(error);
                    },
                    Poll::Pending => {
                        return PollSocket::Pending;
                    },
                }
            },
            QUdpSocketProj::GetWriteUdpState(w_udp_state, udp_socket, kill_udp) => {
                match w_udp_state.as_mut().poll(cx) {
                    Poll::Ready(mut udp_state) => {
                        match &*udp_state {
                            UdpState::Managed(udp_socket, kill) => {
                                // The socket that we created should be destroyed. We'll just use
                                // the one that already exists.
                                kill_udp.awake();

                                self.as_mut().set_acquired(udp_socket.clone(), kill.clone());

                                // Next loop should poll `kill_udp`
                                return PollSocket::Continue;
                            },
                            UdpState::None => {
                                let udp_socket = udp_socket.clone();
                                let kill_udp = kill_udp.clone();

                                self.as_mut().set_acquired(udp_socket.clone(), kill_udp.clone());

                                *udp_state = UdpState::Managed(udp_socket, kill_udp);

                                // Next loop should poll `init_udp`
                                return PollSocket::Continue;
                            },
                            UdpState::Blocked => {
                                kill_udp.awake();
                                let error = errors::UdpSocketError::Disabled;

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
            QUdpSocketProj::Acquired { udp_socket: _, mut kill_udp } => {
                match kill_udp.as_mut().poll(cx) {
                    Poll::Ready(()) => {
                        let error = errors::UdpSocketError::Shutdown;

                        self.as_mut().set_closed(error.clone());

                        return PollSocket::Error(error);
                    },
                    Poll::Pending => {
                        return PollSocket::Pending;
                    },
                }
            },
            QUdpSocketProj::Closed(error) => {
                return PollSocket::Error(error.clone());
            },
        }
    }
}

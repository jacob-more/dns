use std::{
    future::Future,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    pin::Pin,
    sync::Arc,
    task::Poll,
};

use async_lib::awake_token::{AwakeToken, AwokenToken};
use async_trait::async_trait;
use futures::{FutureExt, future::BoxFuture};
use pin_project::pin_project;
use tokio::net;

use crate::network::errors;

use super::{FutureSocket, PollSocket};

const SOCKET_TYPE: errors::SocketType = errors::SocketType::Udp;

pub(crate) enum UdpState {
    Managed(Arc<net::UdpSocket>, AwakeToken),
    None,
    Blocked,
}

#[async_trait]
pub(crate) trait UdpSocket
where
    Self: 'static + Sized + Send + Sync,
{
    fn peer_addr(&self) -> SocketAddr;
    fn state(&self) -> &std::sync::RwLock<UdpState>;

    #[inline]
    fn socket_type(&self) -> errors::SocketType {
        SOCKET_TYPE
    }

    /// Start the UDP listener and drive the UDP state to Managed.
    #[inline]
    async fn start(self: Arc<Self>) -> Result<(), errors::SocketError> {
        match self.init().await {
            Ok(_) => Ok(()),
            Err(error) => Err(error),
        }
    }

    /// Start the UDP listener and drive the UDP state to Managed.
    /// Returns a reference to the created UDP socket.
    #[inline]
    async fn init(
        self: Arc<Self>,
    ) -> Result<(Arc<net::UdpSocket>, AwakeToken), errors::SocketError> {
        // Initially, verify if the connection has already been established.
        match &*self.state().read().unwrap() {
            UdpState::Managed(udp_socket, kill_udp) => {
                return Ok((udp_socket.clone(), kill_udp.clone()));
            }
            UdpState::None => (),
            UdpState::Blocked => {
                return Err(errors::SocketError::Disabled(
                    SOCKET_TYPE,
                    errors::SocketStage::Initialization,
                ));
            }
        }

        let local_addr = match self.peer_addr() {
            SocketAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
            SocketAddr::V6(_) => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
        };
        let udp_socket = match net::UdpSocket::bind(local_addr).await {
            Ok(socket) => Arc::new(socket),
            Err(error) => {
                let io_error = errors::IoError::from(error);
                let socket_error = errors::SocketError::Io {
                    socket_type: SOCKET_TYPE,
                    socket_stage: errors::SocketStage::Initialization,
                    error: io_error,
                };
                return Err(socket_error);
            }
        };
        if let Err(error) = udp_socket.connect(self.peer_addr()).await {
            let io_error = errors::IoError::from(error);
            let socket_error = errors::SocketError::Io {
                socket_type: SOCKET_TYPE,
                socket_stage: errors::SocketStage::Initialization,
                error: io_error,
            };
            return Err(socket_error);
        };
        let udp_reader = udp_socket.clone();
        let udp_writer = udp_socket;
        let kill_udp = AwakeToken::new();

        // Since there is no intermediate state while the UDP socket is being
        // set up and the lock is dropped, it is possible that another process
        // was doing the same task.

        let mut w_state = self.state().write().unwrap();
        match &*w_state {
            UdpState::Managed(existing_udp_socket, _) => {
                return Ok((existing_udp_socket.clone(), kill_udp));
            }
            UdpState::None => {
                *w_state = UdpState::Managed(udp_writer.clone(), kill_udp.clone());
                drop(w_state);

                tokio::spawn(self.listen(udp_reader, kill_udp.clone()));

                return Ok((udp_writer, kill_udp));
            }
            UdpState::Blocked => {
                drop(w_state);
                return Err(errors::SocketError::Disabled(
                    SOCKET_TYPE,
                    errors::SocketStage::Initialization,
                ));
            }
        }
    }

    /// Shut down the TCP listener and drive the TCP state to None.
    #[inline]
    async fn shutdown(self: Arc<Self>) {
        let r_state = self.state().read().unwrap();
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
        let mut w_state = self.state().write().unwrap();
        match &*w_state {
            UdpState::Managed(_, _) => (), //< Already enabled
            UdpState::None => (),          //< Already enabled
            UdpState::Blocked => *w_state = UdpState::None,
        }
        drop(w_state);
    }

    /// Sets the TCP state to Blocked, shutting down the socket if needed.
    #[inline]
    async fn disable(self: Arc<Self>) {
        let mut w_state = self.state().write().unwrap();
        match &*w_state {
            UdpState::Managed(_, kill_udp) => {
                // Since we are removing the reference the kill_udp by setting state to Blocked, we
                // need to kill them now since the listener won't be able to kill them.
                let kill_udp = kill_udp.clone();
                *w_state = UdpState::Blocked;
                drop(w_state);

                kill_udp.awake();
            }
            UdpState::None => {
                *w_state = UdpState::Blocked;
                drop(w_state);
            }
            UdpState::Blocked => {
                // Already disabled
                drop(w_state);
            }
        }
    }

    /// Starts a UDP listener to read data from the provided socket. This processes should stop
    /// when the `kill_udp` token is awoken. This function is intended to be run as a
    /// semi-independent background task.
    async fn listen(self: Arc<Self>, udp_reader: Arc<net::UdpSocket>, kill_udp: AwakeToken);
}

#[pin_project(project = QUdpSocketProj)]
pub(crate) enum QUdpSocket<'c> {
    Fresh,
    InitUdp(BoxFuture<'c, Result<(Arc<net::UdpSocket>, AwakeToken), errors::SocketError>>),
    Acquired {
        udp_socket: Arc<net::UdpSocket>,
        #[pin]
        kill_udp: AwokenToken,
    },
    Closed(errors::SocketError),
}

impl<'a, 'c> QUdpSocket<'c>
where
    'a: 'c,
{
    #[inline]
    fn set_init_udp<S: UdpSocket>(mut self: std::pin::Pin<&mut Self>, socket: &'a Arc<S>) {
        let upstream_socket = socket.peer_addr();
        let init_udp = async move {
            let local_addr = match socket.peer_addr() {
                SocketAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
                SocketAddr::V6(_) => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
            };
            let udp_socket = match net::UdpSocket::bind(local_addr).await {
                Ok(socket) => Arc::new(socket),
                Err(error) => {
                    let io_error = errors::IoError::from(error);
                    let socket_error = errors::SocketError::Io {
                        socket_type: SOCKET_TYPE,
                        socket_stage: errors::SocketStage::Initialization,
                        error: io_error,
                    };
                    return Err(socket_error);
                }
            };
            if let Err(error) = udp_socket.connect(upstream_socket).await {
                let io_error = errors::IoError::from(error);
                let socket_error = errors::SocketError::Io {
                    socket_type: SOCKET_TYPE,
                    socket_stage: errors::SocketStage::Initialization,
                    error: io_error,
                };
                return Err(socket_error);
            };
            return Ok((udp_socket, AwakeToken::new()));
        }
        .boxed();

        self.set(QUdpSocket::InitUdp(init_udp));
    }

    #[inline]
    fn set_acquired(
        mut self: std::pin::Pin<&mut Self>,
        udp_socket: Arc<net::UdpSocket>,
        kill_udp_token: AwakeToken,
    ) {
        self.set(QUdpSocket::Acquired {
            udp_socket,
            kill_udp: kill_udp_token.awoken(),
        });
    }

    #[inline]
    fn set_closed(mut self: std::pin::Pin<&mut Self>, reason: errors::SocketError) {
        self.set(QUdpSocket::Closed(reason));
    }
}

impl<'a, 'c, 'd, S: UdpSocket> FutureSocket<'a, 'd, S, errors::SocketError> for QUdpSocket<'c>
where
    'a: 'c,
{
    fn poll(
        self: &mut Pin<&mut Self>,
        socket: &'a Arc<S>,
        cx: &mut std::task::Context<'_>,
    ) -> PollSocket<errors::SocketError> {
        match self.as_mut().project() {
            QUdpSocketProj::Fresh => {
                let r_state = socket.state().read().unwrap();
                match &*r_state {
                    UdpState::Managed(udp_socket, kill_udp) => {
                        let udp_socket = udp_socket.clone();
                        let kill_udp = kill_udp.clone();
                        drop(r_state);

                        self.as_mut().set_acquired(udp_socket, kill_udp);

                        // Next loop should poll `kill_udp`
                        return PollSocket::Continue;
                    }
                    UdpState::None => {
                        drop(r_state);

                        self.as_mut().set_init_udp(socket);

                        // Next loop should poll `init_udp`
                        return PollSocket::Continue;
                    }
                    UdpState::Blocked => {
                        drop(r_state);

                        let error = errors::SocketError::Disabled(
                            SOCKET_TYPE,
                            errors::SocketStage::Initialization,
                        );
                        self.as_mut().set_closed(error.clone());

                        return PollSocket::Error(error);
                    }
                }
            }
            QUdpSocketProj::InitUdp(init_udp) => {
                match init_udp.as_mut().poll(cx) {
                    Poll::Ready(Ok((udp_socket, kill_udp_token))) => {
                        tokio::spawn(
                            socket
                                .clone()
                                .listen(udp_socket.clone(), kill_udp_token.clone()),
                        );

                        let mut w_udp_state = socket.state().write().unwrap();
                        match &*w_udp_state {
                            UdpState::Managed(udp_socket, kill_udp) => {
                                let existing_udp_socket = udp_socket.clone();
                                let existing_kill_udp = kill_udp.clone();
                                drop(w_udp_state);

                                // The socket that we created should be destroyed. We'll just use
                                // the one that already exists.
                                kill_udp_token.awake();

                                self.as_mut().set_acquired(
                                    existing_udp_socket.clone(),
                                    existing_kill_udp.clone(),
                                );

                                // Next loop should poll `kill_udp`
                                return PollSocket::Continue;
                            }
                            UdpState::None => {
                                *w_udp_state =
                                    UdpState::Managed(udp_socket.clone(), kill_udp_token.clone());
                                drop(w_udp_state);

                                self.as_mut().set_acquired(udp_socket, kill_udp_token);

                                // Next loop should poll `init_udp`
                                return PollSocket::Continue;
                            }
                            UdpState::Blocked => {
                                drop(w_udp_state);

                                kill_udp_token.awake();
                                let error = errors::SocketError::Disabled(
                                    SOCKET_TYPE,
                                    errors::SocketStage::Initialization,
                                );
                                self.as_mut().set_closed(error.clone());

                                return PollSocket::Error(error);
                            }
                        }
                    }
                    Poll::Ready(Err(error)) => {
                        self.as_mut().set_closed(error.clone());

                        return PollSocket::Error(error);
                    }
                    Poll::Pending => {
                        return PollSocket::Pending;
                    }
                }
            }
            QUdpSocketProj::Acquired {
                udp_socket: _,
                mut kill_udp,
            } => match kill_udp.as_mut().poll(cx) {
                Poll::Ready(()) => {
                    let error =
                        errors::SocketError::Shutdown(SOCKET_TYPE, errors::SocketStage::Connected);
                    self.as_mut().set_closed(error.clone());

                    return PollSocket::Error(error);
                }
                Poll::Pending => {
                    return PollSocket::Pending;
                }
            },
            QUdpSocketProj::Closed(error) => {
                return PollSocket::Error(error.clone());
            }
        }
    }
}

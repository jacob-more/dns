use std::{pin::Pin, sync::Arc};

use pin_project::pin_project;

use crate::errors;

use super::{tcp::{QTcpSocket, TcpSocket}, udp::{QUdpSocket, UdpSocket}, FutureSocket, PollSocket};

#[pin_project(project = QUdpTcpSocketProj)]
pub(crate) enum QUdpTcpSocket<'c, 'd> {
    Udp {
        #[pin]
        uq_socket: QUdpSocket<'c, 'd>,
        retransmits: u8,
    },
    Tcp {
        #[pin]
        tq_socket: QTcpSocket<'c, 'd>,
    },
}

impl<'c, 'd, S: UdpSocket + TcpSocket> FutureSocket<'d, S, errors::SocketError> for QUdpTcpSocket<'c, 'd> {
    fn poll<'a>(self: &mut Pin<&mut Self>, socket: &'a Arc<S>, cx: &mut std::task::Context<'_>) -> PollSocket<errors::SocketError> where 'a: 'd {
        match self.as_mut().project() {
            QUdpTcpSocketProj::Udp { mut uq_socket, retransmits: _ } => {
                match uq_socket.poll(socket, cx) {
                    PollSocket::Error(error) => PollSocket::Error(errors::SocketError::from(error)),
                    PollSocket::Continue => PollSocket::Continue,
                    PollSocket::Pending => PollSocket::Pending,
                }
            },
            QUdpTcpSocketProj::Tcp { mut tq_socket } => {
                match tq_socket.poll(socket, cx) {
                    PollSocket::Error(error) => PollSocket::Error(errors::SocketError::from(error)),
                    PollSocket::Continue => PollSocket::Continue,
                    PollSocket::Pending => PollSocket::Pending,
                }
            },
        }
    }
}
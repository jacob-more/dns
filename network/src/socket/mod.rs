use std::{pin::Pin, sync::Arc};

pub mod tcp;
pub mod udp;
pub mod udp_tcp;

pub(crate) enum PollSocket<E> {
    Error(E),
    Continue,
    Pending,
}

pub(crate) trait FutureSocket<'d, S, E> {
    /// Polls the socket to try to get the active the socket if possible. Initializes the socket if
    /// needed. If the connection fails, is not allowed, or is killed, PollSocket::Error will be
    /// returned with the error and the socket should not be polled again. Even after the connection
    /// is Acquired, calling this function to poll the kill token to be notified when the connection
    /// is killed.
    fn poll<'a>(self: &mut Pin<&mut Self>, socket: &'a Arc<S>, cx: &mut std::task::Context<'_>) -> PollSocket<E> where 'a: 'd;
}

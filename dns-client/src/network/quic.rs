use std::{
    cmp::{max, min},
    collections::HashMap,
    future::Future,
    net::{IpAddr, SocketAddr},
    num::NonZeroU8,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    task::Poll,
    time::Duration,
};

use async_lib::{
    awake_token::AwakeToken,
    once_watch::{self, OnceWatchSend, OnceWatchSubscribe},
};
use async_trait::async_trait;
use atomic::Atomic;
use dns_lib::{
    interface::ports::DOQ_UDP_PORT,
    query::{message::Message, question::Question},
    serde::wire::write_wire::WriteWire,
    types::c_domain_name::{CDomainName, CompressionMap},
};
use futures::{FutureExt, future::BoxFuture};
use log::debug;
use pin_project::{pin_project, pinned_drop};
use tinyvec::TinyVec;
use tokio::{
    pin, select,
    task::JoinHandle,
    time::{Instant, Sleep},
};

use crate::network::{
    async_query::{QInitQuery, QInitQueryProj, QueryOpt},
    errors::{self, QueryError},
    receive::read_stream_message,
    rolling_average::{RollingAverage, fetch_update},
    socket::{
        FutureSocket, PollSocket,
        quic::{QQuicSocket, QQuicSocketProj, QuicState},
    },
};

const MAX_MESSAGE_SIZE: u16 = 4092;

const MILLISECONDS_IN_1_SECOND: f64 = 1000.0;

pub(crate) const QUIC_INIT_TIMEOUT: Duration = Duration::from_secs(5);

/// The initial TCP timeout, used when setting up a socket, before anything is known about the
/// average response time.
pub(crate) const INIT_TCP_TIMEOUT: Duration = Duration::from_secs(1);
/// The percentage of the average TCP response time that the timeout should be set to. Currently,
/// this represents 200%. If the average response time were 20 ms, then the retransmission timeout
/// would be 40 ms.
pub(crate) const TCP_TIMEOUT_DURATION_ABOVE_TCP_RESPONSE_TIME: f64 = 2.00;
/// The maximum percentage of the average TCP response time that the timeout should be set to.
/// Currently, this represents 400%. If the average response time were 20 ms, then the
/// retransmission timeout would be 80 ms.
pub(crate) const TCP_TIMEOUT_MAX_DURATION_ABOVE_TCP_RESPONSE_TIME: f64 = 4.00;
/// The step size to use if INCREASE_TCP_TIMEOUT_DROPPED_AVERAGE_THRESHOLD is exceeded.
pub(crate) const TCP_TIMEOUT_STEP_WHEN_DROPPED_THRESHOLD_EXCEEDED: Duration =
    Duration::from_millis(50);
/// When 20% or more of packets are being dropped (for TCP, this just means that the queries are
/// timing out), then it is time to start slowing down the socket.
pub(crate) const INCREASE_TCP_TIMEOUT_DROPPED_AVERAGE_THRESHOLD: f64 = 0.20;
/// When 1% or more of packets are being dropped (for TCP, this just means that the queries are
/// timing out), then we might want to try speeding up the socket again, to reflect the average
/// response time.
pub(crate) const DECREASE_TCP_TIMEOUT_DROPPED_AVERAGE_THRESHOLD: f64 = 0.01;
/// The maximum allowable TCP timeout.
pub(crate) const MAX_TCP_TIMEOUT: Duration = Duration::from_secs(10);
/// The minimum allowable TCP timeout.
pub(crate) const MIN_TCP_TIMEOUT: Duration = Duration::from_millis(50);

// Using the safe checked version of new is not stable. As long as we always use non-zero constants,
// there should not be any problems with this.
pub(crate) const ROLLING_AVERAGE_TCP_MAX_DROPPED: NonZeroU8 =
    unsafe { NonZeroU8::new_unchecked(11) };
pub(crate) const ROLLING_AVERAGE_TCP_MAX_RESPONSE_TIMES: NonZeroU8 =
    unsafe { NonZeroU8::new_unchecked(13) };

fn bound<T>(value: T, lower_bound: T, upper_bound: T) -> T
where
    T: Ord,
{
    debug_assert!(lower_bound <= upper_bound);
    value.clamp(lower_bound, upper_bound)
}

enum QuicResponseTime {
    Dropped,
    Responded(Duration),
    /// `None` is used for cases where the message was never sent (e.g. serialization errors) or the
    /// socket was closed before a response could be received.
    None,
}

#[pin_project(PinnedDrop)]
struct QuicQueryRunner<'a, 'b, 'e, 'h> {
    socket: &'a Arc<QuicSocket>,
    query: &'b mut Message,
    quic_timeout: &'h Duration,
    quic_start_time: Instant,
    #[pin]
    timeout: Sleep,
    #[pin]
    result_sender: once_watch::Sender<Result<Message, errors::QueryError>>,
    #[pin]
    inner: InnerQQ<'e>,
}

impl<'a, 'b, 'e, 'h> QuicQueryRunner<'a, 'b, 'e, 'h> {
    #[inline]
    pub fn new(
        socket: &'a Arc<QuicSocket>,
        query: &'b mut Message,
        result_sender: once_watch::Sender<Result<Message, errors::QueryError>>,
        quic_timeout: &'h Duration,
    ) -> Self {
        Self {
            socket,
            query,
            quic_timeout,
            quic_start_time: Instant::now(),
            timeout: tokio::time::sleep(*quic_timeout),
            result_sender,
            inner: InnerQQ::Fresh,
        }
    }
}

#[pin_project(project = InnerTQProj)]
enum InnerQQ<'e> {
    Fresh,
    Running {
        #[pin]
        qq_socket: QQuicSocket,
        #[pin]
        send_query: QuicSend<'e>,
    },
    Cleanup(QuicResponseTime),
    Complete,
}

#[pin_project(project = QuicSendProj)]
pub(crate) enum QuicSend<'e> {
    Fresh,
    SendAndRecv(BoxFuture<'e, Result<Message, errors::QueryError>>),
}

impl<'e> InnerQQ<'e> {
    #[inline]
    pub fn set_running(mut self: std::pin::Pin<&mut Self>) {
        self.set(Self::Running {
            qq_socket: QQuicSocket::Fresh,
            send_query: QuicSend::Fresh,
        });
    }

    #[inline]
    pub fn set_cleanup(mut self: std::pin::Pin<&mut Self>, execution_time: QuicResponseTime) {
        self.set(Self::Cleanup(execution_time));
    }

    #[inline]
    pub fn set_complete(mut self: std::pin::Pin<&mut Self>) {
        self.set(Self::Complete);
    }
}

impl<'a, 'b, 'e, 'h> Future for QuicQueryRunner<'a, 'b, 'e, 'h> {
    type Output = ();

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let mut this = self.as_mut().project();
        match this.inner.as_mut().project() {
            InnerTQProj::Fresh
            | InnerTQProj::Running {
                qq_socket: _,
                send_query: _,
            } => {
                if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                    let _ = this.result_sender.send(Err(errors::QueryError::Timeout));

                    this.inner.set_cleanup(QuicResponseTime::Dropped);

                    // Exit loop forever: query timed out.
                    // Because the in-flight map was set up before this future was created, we are
                    // still responsible for cleanup.
                }
            }
            InnerTQProj::Cleanup(_) | InnerTQProj::Complete => {
                // Not allowed to timeout. This is a cleanup state.
            }
        }

        loop {
            let mut this = self.as_mut().project();
            match this.inner.as_mut().project() {
                InnerTQProj::Fresh => {
                    this.inner.set_running();

                    // Next loop: poll qq_socket and in_flight to start getting the QUIC socket and
                    // inserting the query ID into the in-flight map.
                    continue;
                }
                InnerTQProj::Running {
                    mut qq_socket,
                    mut send_query,
                } => {
                    match (send_query.as_mut().project(), qq_socket.as_mut().project()) {
                        (_, QQuicSocketProj::Fresh)
                        | (
                            _,
                            QQuicSocketProj::GetQuicEstablishing {
                                receive_quic_socket: _,
                            },
                        )
                        | (_, QQuicSocketProj::InitQuic { join_handle: _ })
                        | (_, QQuicSocketProj::Closed(_)) => {
                            match qq_socket.poll(this.socket, cx) {
                                PollSocket::Error(error) => {
                                    let _ = this
                                        .result_sender
                                        .send(Err(errors::QueryError::from(error)));

                                    this.inner.set_cleanup(QuicResponseTime::None);

                                    // Next loop will poll for the in-flight map lock to remove the
                                    // query ID and record socket statistics.
                                    continue;
                                }
                                PollSocket::Continue => {
                                    // No state change. The socket will be polled again on the next
                                    // loop.
                                    continue;
                                }
                                PollSocket::Pending => {
                                    // We are waiting on the QQuicSocket and the timeout.
                                    // We are already registered with the in-flight map and cannot
                                    // send or receive a query until a socket is established.
                                    return Poll::Pending;
                                }
                            }
                        }
                        (
                            QuicSendProj::Fresh,
                            QQuicSocketProj::Acquired {
                                quic_socket,
                                kill_quic: _,
                            },
                        ) => {
                            // The QQ socket will be polled in a future loop.
                            let mut raw_message = [0_u8; MAX_MESSAGE_SIZE as usize];
                            let mut write_wire = WriteWire::from_bytes(&mut raw_message);
                            if let Err(wire_error) =
                                this.query.to_wire_format_with_two_octet_length(
                                    &mut write_wire,
                                    &mut Some(CompressionMap::new()),
                                )
                            {
                                let _ = this.result_sender.send(Err(errors::QueryError::from(
                                    errors::SendError::from(wire_error),
                                )));

                                this.inner.set_cleanup(QuicResponseTime::None);

                                // Next loop will poll for the in-flight map lock to remove the
                                // query ID and record socket statistics.
                                continue;
                            };
                            let wire_length = write_wire.current_len();
                            let quic_socket = quic_socket.clone();
                            let socket = this.socket.clone();

                            println!(
                                "Sending on QUIC socket {} {{ drop rate {:.2}%, response time {:.2} ms, timeout {} ms }} :: {:?}",
                                this.socket.upstream_address,
                                this.socket.average_dropped_quic_packets() * 100.0,
                                this.socket.average_quic_response_time(),
                                this.quic_timeout.as_millis(),
                                this.query
                            );

                            let send_and_recv = async move {
                                let (mut send_stream, mut recv_stream) = match quic_socket
                                    .open_bi()
                                    .await
                                {
                                    Ok((send_stream, recv_stream)) => (send_stream, recv_stream),
                                    Err(error) => {
                                        let socket_error = errors::SocketError::QuicConnection {
                                            socket_stage: errors::SocketStage::Connected,
                                            error,
                                        };
                                        return Err(errors::QueryError::from(socket_error));
                                    }
                                };

                                socket.recent_messages_sent.store(true, Ordering::Release);
                                // Note: `write_all()` is not cancel safe
                                match send_stream.write_all(&raw_message[..wire_length]).await {
                                    Ok(()) => (),
                                    Err(error) => {
                                        let send_error = errors::SendError::QuicWriteError(error);
                                        return Err(QueryError::from(send_error));
                                    }
                                };

                                if let Err(error) = send_stream.finish() {
                                    debug!("{error} after sending on QUIC socket");
                                }

                                let result = read_stream_message::<{ MAX_MESSAGE_SIZE as usize }>(
                                    &mut recv_stream,
                                    errors::SocketType::Quic,
                                )
                                .await;

                                if let Err(error) = recv_stream.stop(quinn::VarInt::from_u32(0)) {
                                    debug!("{error} after receiving on QUIC socket");
                                }

                                match result {
                                    Ok(message) => return Ok(message),
                                    Err(error) => return Err(QueryError::from(error)),
                                }
                            }
                            .boxed();

                            send_query.set(QuicSend::SendAndRecv(send_and_recv));

                            continue;
                        }
                        (QuicSendProj::SendAndRecv(send_and_recv), _) => {
                            match send_and_recv.as_mut().poll(cx) {
                                Poll::Ready(Ok(message)) => {
                                    let _ = this.result_sender.send(Ok(message));

                                    this.inner.set_cleanup(QuicResponseTime::Responded(
                                        this.quic_start_time.elapsed(),
                                    ));

                                    // Next loop will poll for the in-flight map lock to remove the
                                    // query ID and record socket statistics.
                                    continue;
                                }
                                Poll::Ready(Err(recv_error)) => {
                                    let _ = this
                                        .result_sender
                                        .send(Err(errors::QueryError::from(recv_error)));

                                    this.inner.set_cleanup(QuicResponseTime::None);

                                    // Next loop will poll for the in-flight map lock to remove the
                                    // query ID and record socket statistics.
                                    continue;
                                }
                                Poll::Pending => {
                                    // Before exiting, poll the socket so that it can awake
                                    // this task if the socket is closed.
                                }
                            }

                            match qq_socket.poll(this.socket, cx) {
                                PollSocket::Error(error) => {
                                    let _ = this
                                        .result_sender
                                        .send(Err(errors::QueryError::from(error)));

                                    this.inner.set_cleanup(QuicResponseTime::None);

                                    // Next loop will poll for the in-flight map lock to remove the
                                    // query ID and record socket statistics.
                                    continue;
                                }
                                PollSocket::Continue => {
                                    // No state change. The socket will be polled again on the next
                                    // loop.
                                    continue;
                                }
                                PollSocket::Pending => {
                                    // We are waiting on the QQuicSocket and the timeout.
                                    // We are already registered with the in-flight map and cannot
                                    // send or receive a query until a socket is established.
                                    return Poll::Pending;
                                }
                            }
                        }
                    }
                }
                InnerTQProj::Cleanup(execution_time) => {
                    // Should always transition to the cleanup state before exit. This is
                    // responsible for cleaning up the query ID from the in-flight map (failure to
                    // do so should be considered a memory leak) and for updating the socket
                    // statistics.

                    // We are removing the socket. If a message has not been received, it needs to
                    // be closed so that any processes waiting on this channel wake up and are
                    // cleaned up too.
                    this.result_sender.close();

                    let mut w_active_queries = this.socket.active_queries.write().unwrap();
                    match execution_time {
                        QuicResponseTime::Dropped => {
                            let average_quic_dropped_packets =
                                this.socket.add_dropped_packet_to_quic_average();
                            let average_quic_response_time =
                                this.socket.average_quic_response_time();
                            if average_quic_response_time.is_finite() {
                                if average_quic_dropped_packets.current_average()
                                    >= INCREASE_TCP_TIMEOUT_DROPPED_AVERAGE_THRESHOLD
                                {
                                    w_active_queries.quic_timeout = bound(
                                        min(
                                            w_active_queries.quic_timeout.saturating_add(TCP_TIMEOUT_STEP_WHEN_DROPPED_THRESHOLD_EXCEEDED),
                                            Duration::from_secs_f64(average_quic_response_time * TCP_TIMEOUT_MAX_DURATION_ABOVE_TCP_RESPONSE_TIME / MILLISECONDS_IN_1_SECOND),
                                        ),
                                        MIN_TCP_TIMEOUT,
                                        MAX_TCP_TIMEOUT,
                                    );
                                }
                            } else {
                                if average_quic_dropped_packets.current_average()
                                    >= INCREASE_TCP_TIMEOUT_DROPPED_AVERAGE_THRESHOLD
                                {
                                    w_active_queries.quic_timeout = bound(
                                        w_active_queries.quic_timeout.saturating_add(
                                            TCP_TIMEOUT_STEP_WHEN_DROPPED_THRESHOLD_EXCEEDED,
                                        ),
                                        MIN_TCP_TIMEOUT,
                                        MAX_TCP_TIMEOUT,
                                    );
                                }
                            }
                        }
                        QuicResponseTime::Responded(response_time) => {
                            let (average_quic_response_time, average_quic_dropped_packets) = this
                                .socket
                                .add_response_time_to_quic_average(*response_time);
                            if average_quic_dropped_packets.current_average()
                                <= DECREASE_TCP_TIMEOUT_DROPPED_AVERAGE_THRESHOLD
                            {
                                w_active_queries.quic_timeout = bound(
                                    max(
                                        w_active_queries.quic_timeout.saturating_add(
                                            TCP_TIMEOUT_STEP_WHEN_DROPPED_THRESHOLD_EXCEEDED,
                                        ),
                                        Duration::from_secs_f64(
                                            average_quic_response_time.current_average()
                                                * TCP_TIMEOUT_DURATION_ABOVE_TCP_RESPONSE_TIME
                                                / MILLISECONDS_IN_1_SECOND,
                                        ),
                                    ),
                                    MIN_TCP_TIMEOUT,
                                    MAX_TCP_TIMEOUT,
                                );
                            }
                        }
                        QuicResponseTime::None => (),
                    }

                    // We are responsible for clearing these maps. Otherwise, the memory
                    // will only ever be cleaned up when the socket itself is dropped.
                    w_active_queries.in_flight.remove(&this.query.id);
                    w_active_queries.active.remove(&this.query.question);
                    drop(w_active_queries);

                    this.inner.set_complete();

                    // Socket should not be polled again.
                    return Poll::Ready(());
                }
                InnerTQProj::Complete => {
                    panic!("QUIC only query polled after completion");
                }
            }
        }
    }
}

#[pinned_drop]
impl<'a, 'b, 'e, 'h> PinnedDrop for QuicQueryRunner<'a, 'b, 'e, 'h> {
    fn drop(mut self: Pin<&mut Self>) {
        match self.as_mut().project().inner.as_mut().project() {
            InnerTQProj::Fresh
            | InnerTQProj::Running {
                qq_socket: _,
                send_query: _,
            }
            | InnerTQProj::Cleanup(_) => {
                let mut w_active_queries = self.socket.active_queries.write().unwrap();
                let _ = w_active_queries.in_flight.remove(&self.query.id);
                let _ = w_active_queries.active.remove(&self.query.question);
                drop(w_active_queries);
            }
            InnerTQProj::Complete => {
                // Nothing to do for active queries. Already done cleaning up.
            }
        }
    }
}

#[pin_project]
pub struct QuicQuery<'a, 'b> {
    socket: &'a Arc<QuicSocket>,
    query: &'b mut Message,
    #[pin]
    inner: QInitQuery,
}

impl<'a, 'b> QuicQuery<'a, 'b> {
    #[inline]
    pub fn new(socket: &'a Arc<QuicSocket>, query: &'b mut Message) -> Self {
        Self {
            socket,
            query,
            inner: QInitQuery::Fresh,
        }
    }
}

impl<'a, 'b> Future for QuicQuery<'a, 'b> {
    type Output = Result<Message, errors::QueryError>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        loop {
            let mut this = self.as_mut().project();
            match this.inner.as_mut().project() {
                QInitQueryProj::Fresh => {
                    let r_active_queries = this.socket.active_queries.read().unwrap();
                    match r_active_queries.active.get(&this.query.question) {
                        Some((query_id, result_sender)) => {
                            // A query has already been made for this question and is in
                            // flight. This future can listen for that response instead of
                            // making a duplicate query.
                            this.query.id = *query_id;
                            let result_receiver = result_sender.subscribe();
                            drop(r_active_queries);

                            this.inner.set_following(result_receiver);

                            // The next loop will poll the receiver until a result is
                            // received.
                            continue;
                        }
                        None => {
                            // This is a new query and has not yet been registered. Acquire
                            // the write lock to register it.
                            drop(r_active_queries);
                            this.inner.set_write_active_query();

                            // During the next loop, the write-lock will be polled.
                            continue;
                        }
                    }
                }
                QInitQueryProj::WriteActiveQuery => {
                    // Note that the same checks for the read-lock need to be made again in case
                    // something changed between when the lock was dropped and now.
                    let mut w_active_queries = this.socket.active_queries.write().unwrap();
                    match w_active_queries.active.get(&this.query.question) {
                        Some((query_id, result_sender)) => {
                            // A query has already been made for this question and is in
                            // flight. This future can listen for that response instead of
                            // making a duplicate query.
                            this.query.id = *query_id;
                            let result_receiver = result_sender.subscribe();
                            drop(w_active_queries);

                            this.inner.set_following(result_receiver);

                            // The next loop will poll the receiver until a result is
                            // received.
                            continue;
                        }
                        None => {
                            // This question is not already in flight on this socket. Set
                            // up the channels for the listener to send the answer on once
                            // received and start a query-runner that will send and manage
                            // the query.
                            let (result_sender, result_receiver) = once_watch::channel();

                            // This is the initial query ID. However, it could change if it
                            // is already in use.
                            this.query.id = rand::random();

                            // verify that ID is unique.
                            while w_active_queries.in_flight.contains_key(&this.query.id) {
                                this.query.id = rand::random();
                                // FIXME: should this fail after some number of non-unique
                                // keys? May want to verify that the list isn't full.
                            }

                            // The query-runner is spawned as an independent task since it
                            // must run to completion. It should not be cancelled if this
                            // task is cancelled because there may be others that are
                            // listening for the response too.
                            let join_handle = tokio::spawn({
                                let quic_timeout = w_active_queries.quic_timeout;
                                let result_sender = result_sender.clone();
                                let socket = this.socket.clone();
                                let mut query = this.query.clone();
                                async move {
                                    QuicQueryRunner::new(
                                        &socket,
                                        &mut query,
                                        result_sender,
                                        &quic_timeout,
                                    )
                                    .await;
                                }
                            });

                            w_active_queries
                                .in_flight
                                .insert(this.query.id, (result_sender.clone(), join_handle));
                            w_active_queries.active.insert(
                                this.query.question.clone(),
                                (this.query.id, result_sender),
                            );
                            drop(w_active_queries);

                            this.inner.set_following(result_receiver);

                            // The next loop will poll the receiver until a result is
                            // received from the newly spawned query-runner.
                            continue;
                        }
                    }
                }
                QInitQueryProj::Following(mut result_receiver) => {
                    match result_receiver.as_mut().poll(cx) {
                        Poll::Ready(Ok(response)) => {
                            this.inner.set_complete();

                            // A message was received. The query is complete. It should not be
                            // polled again.
                            return Poll::Ready(response);
                        }
                        Poll::Ready(Err(once_watch::RecvError::Closed)) => {
                            let error = errors::QueryError::from(errors::SocketError::Shutdown(
                                errors::SocketType::Quic,
                                errors::SocketStage::Connected,
                            ));

                            this.inner.set_complete();

                            // The query runner closed the channel for some reason. The query is
                            // complete. It should not be polled again.
                            return Poll::Ready(Err(error));
                        }
                        Poll::Pending => {
                            // Waiting to receive a response. The future will be awoken once a
                            // message is sent over the channel or it is closed.
                            return Poll::Pending;
                        }
                    }
                }
                QInitQueryProj::Complete => {
                    panic!("QuicQuery cannot be polled after completion")
                }
            }
        }
    }
}

// Implement QUIC functions on QuicSocket
#[async_trait]
impl crate::network::socket::quic::QuicSocket for QuicSocket {
    #[inline]
    fn peer_addr(&self) -> SocketAddr {
        SocketAddr::new(self.upstream_address, DOQ_UDP_PORT)
    }

    #[inline]
    fn peer_name(&self) -> &CDomainName {
        &self.ns_name
    }

    #[inline]
    fn state(&self) -> &std::sync::RwLock<QuicState> {
        &self.quic
    }

    #[inline]
    fn client_config(&self) -> &Arc<quinn::ClientConfig> {
        &self.client_config
    }

    #[inline]
    async fn listen(self: Arc<Self>, quic_socket: Arc<quinn::Connection>, kill_quic: AwakeToken) {
        pin!(let kill_quic_awoken = kill_quic.awoken(););
        select! {
            biased;
            () = &mut kill_quic_awoken => {
                println!("QUIC Socket {} Canceled. Shutting down QUIC Listener.", self.upstream_address);
            },
            reason = quic_socket.closed() => {
                println!("{reason}");
            },
        }

        self.listen_quic_cleanup(kill_quic).await;
    }
}

impl QuicSocket {
    #[inline]
    async fn listen_quic_cleanup(self: Arc<Self>, kill_quic: AwakeToken) {
        println!("Cleaning up QUIC socket {}", self.upstream_address);

        let mut w_state = self.quic.write().unwrap();
        match &*w_state {
            QuicState::Managed {
                socket: _,
                kill: managed_kill_quic,
            } => {
                // If the managed socket is the one that we are cleaning up...
                if &kill_quic == managed_kill_quic {
                    // We are responsible for cleanup.
                    *w_state = QuicState::None;
                    drop(w_state);

                    kill_quic.awake();

                // If the managed socket isn't the one that we are cleaning up...
                } else {
                    // This is not our socket to clean up.
                    drop(w_state);
                }
            }
            QuicState::Establishing { sender: _, kill: _ }
            | QuicState::None
            | QuicState::Blocked => {
                // This is not our socket to clean up.
                drop(w_state);
            }
        }
    }
}

struct ActiveQueries {
    quic_timeout: Duration,

    in_flight: HashMap<
        u16,
        (
            once_watch::Sender<Result<Message, errors::QueryError>>,
            JoinHandle<()>,
        ),
    >,
    active: HashMap<
        TinyVec<[Question; 1]>,
        (u16, once_watch::Sender<Result<Message, errors::QueryError>>),
    >,
}

impl ActiveQueries {
    #[inline]
    pub fn new() -> Self {
        Self {
            quic_timeout: INIT_TCP_TIMEOUT,

            in_flight: HashMap::new(),
            active: HashMap::new(),
        }
    }
}

pub struct QuicSocket {
    ns_name: CDomainName,
    upstream_address: IpAddr,
    quic: std::sync::RwLock<QuicState>,
    active_queries: std::sync::RwLock<ActiveQueries>,
    client_config: Arc<quinn::ClientConfig>,

    // Rolling averages
    average_quic_response_time: Atomic<RollingAverage>,
    average_quic_dropped_packets: Atomic<RollingAverage>,

    // Counters used to determine when the socket should be closed.
    recent_messages_sent: AtomicBool,
    recent_messages_received: AtomicBool,
}

impl QuicSocket {
    #[inline]
    pub fn new(
        upstream_address: IpAddr,
        ns_name: CDomainName,
        client_config: Arc<quinn::ClientConfig>,
    ) -> Arc<Self> {
        Arc::new(QuicSocket {
            ns_name,
            upstream_address,
            quic: std::sync::RwLock::new(QuicState::None),
            active_queries: std::sync::RwLock::new(ActiveQueries::new()),
            client_config,

            average_quic_response_time: Atomic::new(RollingAverage::new()),
            average_quic_dropped_packets: Atomic::new(RollingAverage::new()),

            recent_messages_sent: AtomicBool::new(false),
            recent_messages_received: AtomicBool::new(false),
        })
    }

    #[inline]
    pub fn average_quic_response_time(&self) -> f64 {
        self.average_quic_response_time
            .load(Ordering::Acquire)
            .current_average()
    }

    #[inline]
    pub fn average_dropped_quic_packets(&self) -> f64 {
        self.average_quic_dropped_packets
            .load(Ordering::Acquire)
            .current_average()
    }

    #[inline]
    fn add_dropped_packet_to_quic_average(&self) -> RollingAverage {
        // We can use relaxed memory orderings with the rolling average because it is not being used
        // for synchronization nor do we care about the order of atomic operations. We only care
        // that the operation is atomic.
        fetch_update(
            &self.average_quic_dropped_packets,
            Ordering::Relaxed,
            Ordering::Relaxed,
            |average| average.put_next(1, ROLLING_AVERAGE_TCP_MAX_DROPPED),
        )
    }

    #[inline]
    fn add_response_time_to_quic_average(
        &self,
        response_time: Duration,
    ) -> (RollingAverage, RollingAverage) {
        // We can use relaxed memory orderings with the rolling average because it is not being used
        // for synchronization nor do we care about the order of atomic operations. We only care
        // that the operation is atomic.
        (
            fetch_update(
                &self.average_quic_response_time,
                Ordering::Relaxed,
                Ordering::Relaxed,
                |average| {
                    average.put_next(
                        u32::try_from(response_time.as_millis()).unwrap_or(u32::MAX),
                        ROLLING_AVERAGE_TCP_MAX_RESPONSE_TIMES,
                    )
                },
            ),
            fetch_update(
                &self.average_quic_dropped_packets,
                Ordering::Relaxed,
                Ordering::Relaxed,
                |average| average.put_next(0, ROLLING_AVERAGE_TCP_MAX_DROPPED),
            ),
        )
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
            self.recent_messages_received.load(Ordering::Acquire),
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
            self.recent_messages_received.swap(false, Ordering::AcqRel),
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
    pub async fn start(self: Arc<Self>) -> Result<(), errors::SocketError> {
        <Self as crate::network::socket::quic::QuicSocket>::start(self).await
    }

    #[inline]
    pub async fn shutdown(self: Arc<Self>) {
        <Self as crate::network::socket::quic::QuicSocket>::shutdown(self).await;
    }

    #[inline]
    pub async fn enable(self: Arc<Self>) {
        <Self as crate::network::socket::quic::QuicSocket>::enable(self).await;
    }

    #[inline]
    pub async fn disable(self: Arc<Self>) {
        <Self as crate::network::socket::quic::QuicSocket>::disable(self).await;
    }

    pub fn query<'a, 'b>(
        self: &'a Arc<Self>,
        query: &'b mut Message,
        options: QueryOpt,
    ) -> QuicQuery<'a, 'b> {
        // If the UDP socket is unreliable, send most data via QUIC. Some queries should still use
        // UDP to determine if the network conditions are improving. However, if the QUIC connection
        // is also unstable, then we should not rely on it.
        let query_task = match options {
            QueryOpt::UdpTcp => todo!(),
            QueryOpt::Tcp => todo!(),
            QueryOpt::Quic => QuicQuery::new(&self, query),
            QueryOpt::Tls => todo!(),
            QueryOpt::QuicTls => todo!(),
            QueryOpt::Https => todo!(),
        };

        return query_task;
    }
}

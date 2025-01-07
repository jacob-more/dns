use std::{cmp::{max, min}, collections::HashMap, future::Future, net::{IpAddr, SocketAddr}, num::NonZeroU8, pin::Pin, sync::{atomic::{AtomicBool, Ordering}, Arc}, task::Poll, time::Duration};

use async_lib::{awake_token::AwakeToken, once_watch::{self, OnceWatchSend, OnceWatchSubscribe}};
use async_trait::async_trait;
use atomic::Atomic;
use dns_lib::{interface::ports::DOT_TCP_PORT, query::{message::Message, question::Question}, serde::wire::write_wire::WriteWire, types::c_domain_name::{CDomainName, CompressionMap}};
use futures::{future::BoxFuture, FutureExt};
use pin_project::{pin_project, pinned_drop};
use rustls::ClientConfig;
use tinyvec::TinyVec;
use tokio::{io::AsyncWriteExt, pin, select, sync::{RwLock, RwLockWriteGuard}, task::JoinHandle, time::{Instant, Sleep}};

use crate::{async_query::{QInitQuery, QInitQueryProj, QSend, QSendProj, QSendType, QueryOpt}, errors, receive::read_stream_message, rolling_average::{fetch_update, RollingAverage}, socket::{tls::{QTlsSocket, QTlsSocketProj, TlsReadHalf, TlsState}, FutureSocket, PollSocket}};

const MAX_MESSAGE_SIZE: u16 = 4092;

const MILLISECONDS_IN_1_SECOND: f64 = 1000.0;

pub(crate) const TCP_INIT_TIMEOUT: Duration = Duration::from_secs(5);
pub(crate) const TCP_LISTEN_TIMEOUT: Duration = Duration::from_secs(120);

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
pub(crate) const TCP_TIMEOUT_STEP_WHEN_DROPPED_THRESHOLD_EXCEEDED: Duration = Duration::from_millis(50);
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
pub(crate) const ROLLING_AVERAGE_TCP_MAX_DROPPED: NonZeroU8        = unsafe { NonZeroU8::new_unchecked(11) };
pub(crate) const ROLLING_AVERAGE_TCP_MAX_RESPONSE_TIMES: NonZeroU8 = unsafe { NonZeroU8::new_unchecked(13) };

fn bound<T>(value: T, lower_bound: T, upper_bound: T) -> T where T: Ord {
    debug_assert!(lower_bound <= upper_bound);
    value.clamp(lower_bound, upper_bound)
}

enum TlsResponseTime {
    Dropped,
    Responded(Duration),
    /// `None` is used for cases where the message was never sent (e.g. serialization errors) or the
    /// socket was closed before a response could be received.
    None,
}

#[pin_project(PinnedDrop)]
struct TlsQueryRunner<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h>
where
    'a: 'd + 'g
{
    socket: &'a Arc<TlsSocket>,
    query: &'b mut Message,
    tls_timeout: &'h Duration,
    tls_start_time: Instant,
    #[pin]
    timeout: Sleep,
    #[pin]
    result_receiver: once_watch::Receiver<Result<Message, errors::QueryError>>,
    #[pin]
    inner: InnerTQ<'c, 'd, 'e, 'f, 'g>,
}

impl<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h> TlsQueryRunner<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h>
where
    'g: 'f
{
    #[inline]
    pub fn new(socket: &'a Arc<TlsSocket>, query: &'b mut Message, result_receiver: once_watch::Receiver<Result<Message, errors::QueryError>>, tls_timeout: &'h Duration) -> Self {
        Self {
            socket,
            query,
            tls_timeout,
            tls_start_time: Instant::now(),
            timeout: tokio::time::sleep(*tls_timeout),
            result_receiver,
            inner: InnerTQ::Fresh,
        }
    }
}

#[pin_project(project = InnerTQProj)]
enum InnerTQ<'c, 'd, 'e, 'f, 'g>
where
    'g: 'f
{
    Fresh,
    Running {
        #[pin]
        tq_socket: QTlsSocket<'c, 'd>,
        #[pin]
        send_query: QSend<'e, QSendType, errors::SendError>,
    },
    Cleanup(BoxFuture<'f, RwLockWriteGuard<'g, ActiveQueries>>, TlsResponseTime),
    Complete,
}

impl<'a, 'c, 'd, 'e, 'f, 'g> InnerTQ<'c, 'd, 'e, 'f, 'g>
where
    'a: 'd + 'g
{
    #[inline]
    pub fn set_running(mut self: std::pin::Pin<&mut Self>, query_type: QSendType) {
        self.set(Self::Running {
            tq_socket: QTlsSocket::Fresh,
            send_query: QSend::Fresh(query_type),
        });
    }

    #[inline]
    pub fn set_cleanup(mut self: std::pin::Pin<&mut Self>, execution_time: TlsResponseTime, socket: &'a Arc<TlsSocket>) {
        let w_active_queries = socket.active_queries.write().boxed();

        self.set(Self::Cleanup(w_active_queries, execution_time));
    }

    #[inline]
    pub fn set_complete(mut self: std::pin::Pin<&mut Self>) {
        self.set(Self::Complete);
    }
}

impl<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h> Future for TlsQueryRunner<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h> {
    type Output = ();

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let mut this = self.as_mut().project();
        match this.inner.as_mut().project() {
            InnerTQProj::Fresh
          | InnerTQProj::Running { tq_socket: _, send_query: _ } => {
                if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                    let _ = this.result_receiver.get_sender().send(Err(errors::QueryError::Timeout));

                    this.inner.set_cleanup(TlsResponseTime::Dropped, this.socket);

                    // Exit loop forever: query timed out.
                    // Because the in-flight map was set up before this future was created, we are
                    // still responsible for cleanup.
                }
            },
            InnerTQProj::Cleanup(_, _)
          | InnerTQProj::Complete => {
                // Not allowed to timeout. This is a cleanup state.
            },
        }

        loop {
            let mut this = self.as_mut().project();
            match this.inner.as_mut().project() {
                InnerTQProj::Fresh => {
                    this.inner.set_running(QSendType::Initial);

                    // Next loop: poll tq_socket and in_flight to start getting the TLS socket and
                    // inserting the query ID into the in-flight map.
                    continue;
                },
                InnerTQProj::Running { mut tq_socket, mut send_query } => {
                    match (send_query.as_mut().project(), tq_socket.as_mut().project()) {
                        (QSendProj::Fresh(_), QTlsSocketProj::Fresh)
                      | (QSendProj::Fresh(_), QTlsSocketProj::GetTlsState(_))
                      | (QSendProj::Fresh(_), QTlsSocketProj::GetTlsEstablishing { receive_tls_socket: _ })
                      | (QSendProj::Fresh(_), QTlsSocketProj::InitTls { join_handle: _ })
                      | (QSendProj::Fresh(_), QTlsSocketProj::Closed(_)) => {
                            // We don't poll the result_receiver until the QSend state is Complete
                            // since you can't receive a message until a query has been sent. If
                            // there is an error while trying to send the query, we should send a
                            // QError over that channel to make sure any tasks waiting on it can
                            // handle the error appropriately.

                            match tq_socket.poll(this.socket, cx) {
                                PollSocket::Error(error) => {
                                    let _ = this.result_receiver.get_sender().send(Err(errors::QueryError::from(error)));

                                    this.inner.set_cleanup(TlsResponseTime::None, this.socket);

                                    // Next loop will poll for the in-flight map lock to remove the
                                    // query ID and record socket statistics.
                                    continue;
                                },
                                PollSocket::Continue => {
                                    // No state change. The socket will be polled again on the next
                                    // loop.
                                    continue;
                                },
                                PollSocket::Pending => {
                                    // We are waiting on the QTlsSocket and the timeout.
                                    // We are already registered with the in-flight map and cannot
                                    // send or receive a query until a socket is established.
                                    return Poll::Pending;
                                },
                            }
                        },
                        (QSendProj::Fresh(_), QTlsSocketProj::Acquired { tls_socket, kill_tls: _ }) => {
                            // We don't poll the result_receiver until the QSend state is Complete
                            // since you can't receive a message until a query has been sent. If
                            // there is an error while trying to send the query, we should send a
                            // QError over that channel to make sure any tasks waiting on it can
                            // handle the error appropriately.

                            let socket = this.socket.clone();
                            let tls_socket = tls_socket.clone();

                            if let PollSocket::Error(error) = tq_socket.poll(this.socket, cx) {
                                let _ = this.result_receiver.get_sender().send(Err(errors::QueryError::from(error)));

                                this.inner.set_cleanup(TlsResponseTime::None, this.socket);

                                // Next loop will poll for the in-flight map lock to remove the
                                // query ID and record socket statistics.
                                continue;
                            }

                            let mut raw_message = [0_u8; MAX_MESSAGE_SIZE as usize];
                            let mut write_wire = WriteWire::from_bytes(&mut raw_message);
                            if let Err(wire_error) = this.query.to_wire_format_with_two_octet_length(&mut write_wire, &mut Some(CompressionMap::new())) {
                                let _ = this.result_receiver.get_sender().send(Err(errors::QueryError::from(errors::SendError::from(wire_error))));

                                this.inner.set_cleanup(TlsResponseTime::None, this.socket);

                                // Next loop will poll for the in-flight map lock to remove the
                                // query ID and record socket statistics.
                                continue;
                            };
                            let wire_length = write_wire.current_len();

                            println!("Sending on TLS socket {} {{ drop rate {:.2}%, response time {:.2} ms, timeout {} ms }} :: {:?}", this.socket.upstream_address, this.socket.average_dropped_tls_packets() * 100.0, this.socket.average_tls_response_time(), this.tls_timeout.as_millis(), this.query);

                            let send_query_future = async move {
                                let socket = socket;
                                let tls_socket = tls_socket;
                                let wire_length = wire_length;

                                socket.recent_messages_sent.store(true, Ordering::Release);
                                let mut w_tls_stream = tls_socket.lock().await;
                                let bytes_written = match w_tls_stream.write(&raw_message[..wire_length]).await {
                                    Ok(bytes) => bytes,
                                    Err(error) => {
                                        let io_error = errors::IoError::from(error);
                                        let send_error = errors::SendError::Io {
                                            socket_type: errors::SocketType::Tls,
                                            error: io_error,
                                        };
                                        return Err(send_error);
                                    },
                                };
                                drop(w_tls_stream);
                                // Verify that the correct number of bytes were written.
                                if bytes_written != wire_length {
                                    return Err(errors::SendError::IncorrectNumberBytes {
                                        socket_type: errors::SocketType::Tls,
                                        expected: wire_length as u16,
                                        sent: bytes_written
                                    });
                                }

                                return Ok(());
                            }.boxed();

                            send_query.set_send_query(send_query_future);

                            // Next loop will begin to poll QSend. This will get the lock and the
                            // TlsStream and write the bytes out.
                            continue;
                        },
                        (QSendProj::SendQuery(_, send_query_future), _) => {
                            // We don't poll the result_receiver until the QSend state is Complete
                            // since you can't receive a message until a query has been sent. If
                            // there is an error while trying to send the query, we should send a
                            // QError over that channel to make sure any tasks waiting on it can
                            // handle the error appropriately.

                            match (send_query_future.as_mut().poll(cx), tq_socket.poll(this.socket, cx)) {
                                (_, PollSocket::Error(error)) => {
                                    let _ = this.result_receiver.get_sender().send(Err(errors::QueryError::from(error)));

                                    this.inner.set_cleanup(TlsResponseTime::None, this.socket);

                                    // Next loop will poll for the in-flight map lock to remove the
                                    // query ID and record socket statistics.
                                    continue;
                                },
                                (Poll::Ready(Err(error)), _) => {
                                    let _ = this.result_receiver.get_sender().send(Err(errors::QueryError::from(error)));

                                    this.inner.set_cleanup(TlsResponseTime::None, this.socket);

                                    // Next loop will poll for the in-flight map lock to remove the
                                    // query ID and record socket statistics.
                                    continue;
                                },
                                (Poll::Ready(Ok(())), PollSocket::Continue | PollSocket::Pending) => {
                                    send_query.set_complete();

                                    // Now that a message has been sent, we will start polling the
                                    // receiver.
                                    continue;
                                },
                                (Poll::Pending, PollSocket::Continue) => {
                                    // If at least one of our futures needs to loop again, we should
                                    // loop again unless an exit condition is reached.
                                    continue;
                                },
                                (Poll::Pending, PollSocket::Pending) => {
                                    // Will wake up if the QTlsSocket wakes us (most likely because
                                    // the socket was killed), the QSend completes, or the timeout
                                    // occurs.
                                    return Poll::Pending;
                                },
                            }
                        },
                        (QSendProj::Complete(_), _) => {
                            // The QSend state is Complete so the query has been sent successfully.
                            // Polling the receiver will get the result from the listener once it
                            // is received. If it returns a message or an error, we don't need to
                            // send anything over that channel since it can only hold one message.

                            match this.result_receiver.as_mut().poll(cx) {
                                Poll::Ready(Ok(Ok(_))) => {
                                    let execution_time = this.tls_start_time.elapsed();

                                    this.inner.set_cleanup(TlsResponseTime::Responded(execution_time), this.socket);

                                    // Next loop will poll for the in-flight map lock to remove the
                                    // query ID and record socket statistics.
                                    continue;
                                },
                                Poll::Ready(Ok(Err(_)))
                              | Poll::Ready(Err(_)) => {
                                    this.inner.set_cleanup(TlsResponseTime::None, this.socket);

                                    // Next loop will poll for the in-flight map lock to remove the
                                    // query ID and record socket statistics.
                                    continue;
                                },
                                Poll::Pending => {
                                    // Nothing has been received yet. Whether we return Pending or
                                    // Continue will be determined when the TLS socket is polled.
                                },
                            }

                            match tq_socket.poll(this.socket, cx) {
                                PollSocket::Error(error) => {
                                    let _ = this.result_receiver.get_sender().send(Err(errors::QueryError::from(error)));

                                    this.inner.set_cleanup(TlsResponseTime::None, this.socket);

                                    // Next loop will poll for the in-flight map lock to remove the
                                    // query ID and record socket statistics.
                                    continue;
                                },
                                PollSocket::Continue => {
                                    // If at least one of our futures needs to loop again, we should
                                    // loop again unless an exit condition is reached.
                                    continue;
                                },
                                PollSocket::Pending => {
                                    // Will wake up if the QTlsSocket wakes us (most likely because
                                    // the socket was killed), the receiver has a response, the
                                    // receiver is closed, or the timeout occurs.
                                    return Poll::Pending;
                                },
                            }
                        },
                    }
                },
                InnerTQProj::Cleanup(w_active_queries, execution_time) => {
                    // Should always transition to the cleanup state before exit. This is
                    // responsible for cleaning up the query ID from the in-flight map (failure to
                    // do so should be considered a memory leak) and for updating the socket
                    // statistics.

                    // We are removing the socket. If a message has not been received, it needs to
                    // be closed so that any processes waiting on this channel wake up and are
                    // cleaned up too.
                    this.result_receiver.close();

                    match w_active_queries.as_mut().poll(cx) {
                        Poll::Ready(mut w_active_queries) => {
                            match execution_time {
                                TlsResponseTime::Dropped => {
                                    let average_tls_dropped_packets = this.socket.add_dropped_packet_to_tls_average();
                                    let average_tls_response_time = this.socket.average_tls_response_time();
                                    if average_tls_response_time.is_finite() {
                                        if average_tls_dropped_packets.current_average() >= INCREASE_TCP_TIMEOUT_DROPPED_AVERAGE_THRESHOLD {
                                            w_active_queries.tls_timeout = bound(
                                                min(
                                                    w_active_queries.tls_timeout.saturating_add(TCP_TIMEOUT_STEP_WHEN_DROPPED_THRESHOLD_EXCEEDED),
                                                    Duration::from_secs_f64(average_tls_response_time * TCP_TIMEOUT_MAX_DURATION_ABOVE_TCP_RESPONSE_TIME / MILLISECONDS_IN_1_SECOND),
                                                ),
                                                MIN_TCP_TIMEOUT,
                                                MAX_TCP_TIMEOUT,
                                            );
                                        }
                                    } else {
                                        if average_tls_dropped_packets.current_average() >= INCREASE_TCP_TIMEOUT_DROPPED_AVERAGE_THRESHOLD {
                                            w_active_queries.tls_timeout = bound(
                                                w_active_queries.tls_timeout.saturating_add(TCP_TIMEOUT_STEP_WHEN_DROPPED_THRESHOLD_EXCEEDED),
                                                MIN_TCP_TIMEOUT,
                                                MAX_TCP_TIMEOUT,
                                            );
                                        }
                                    }
                                },
                                TlsResponseTime::Responded(response_time) => {
                                    let (average_tls_response_time, average_tls_dropped_packets) = this.socket.add_response_time_to_tls_average(*response_time);
                                    if average_tls_dropped_packets.current_average() <= DECREASE_TCP_TIMEOUT_DROPPED_AVERAGE_THRESHOLD {
                                        w_active_queries.tls_timeout = bound(
                                            max(
                                                w_active_queries.tls_timeout.saturating_add(TCP_TIMEOUT_STEP_WHEN_DROPPED_THRESHOLD_EXCEEDED),
                                                Duration::from_secs_f64(average_tls_response_time.current_average() * TCP_TIMEOUT_DURATION_ABOVE_TCP_RESPONSE_TIME / MILLISECONDS_IN_1_SECOND),
                                            ),
                                            MIN_TCP_TIMEOUT,
                                            MAX_TCP_TIMEOUT,
                                        );
                                    }
                                },
                                TlsResponseTime::None => (),
                            }

                            // We are responsible for clearing these maps. Otherwise, the memory
                            // will only ever be cleaned up when the socket itself is dropped.
                            w_active_queries.in_flight.remove(&this.query.id);
                            w_active_queries.active.remove(&this.query.question);
                            drop(w_active_queries);

                            this.inner.set_complete();

                            // Socket should not be polled again.
                            return Poll::Ready(());
                        },
                        Poll::Pending => {
                            // Only waiting on the write lock to clear the in-flight maps. Once
                            // acquired, cleanup can be done.
                            return Poll::Pending;
                        },
                    }
                },
                InnerTQProj::Complete => {
                    panic!("TLS only query polled after completion");
                },
            }
        }
    }
}

#[pinned_drop]
impl<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h> PinnedDrop for TlsQueryRunner<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h> {
    fn drop(mut self: Pin<&mut Self>) {
        async fn cleanup(socket: Arc<TlsSocket>, query: Message) {
            let mut w_active_queries = socket.active_queries.write().await;
            let _ = w_active_queries.in_flight.remove(&query.id);
            let _ = w_active_queries.active.remove(&query.question);
            drop(w_active_queries);
        }

        match self.as_mut().project().inner.as_mut().project() {
            InnerTQProj::Fresh
          | InnerTQProj::Running { tq_socket: _, send_query: _ }
          | InnerTQProj::Cleanup(_, _) => {
                // Unfortunately, cannot re-use the existing futures because this struct is pinned.
                // Spawning the cleanup in a separate task will ensure eventual cleanup.
                let socket = self.socket.clone();
                let query = self.query.clone();
                tokio::spawn(cleanup(socket, query));
            },
            InnerTQProj::Complete => {
                // Nothing to do for active queries. Already done cleaning up.
            }
        }
    }
}

#[pin_project]
pub struct TlsQuery<'a, 'b, 'c, 'd>
where
    'a: 'd
{
    socket: &'a Arc<TlsSocket>,
    query: &'b mut Message,
    #[pin]
    inner: QInitQuery<'c, 'd, ActiveQueries>,
}

impl<'a, 'b, 'c, 'd> TlsQuery<'a, 'b, 'c, 'd> {
    #[inline]
    pub fn new(socket: &'a Arc<TlsSocket>, query: &'b mut Message) -> Self {
        Self {
            socket,
            query,
            inner: QInitQuery::Fresh,
        }
    }
}

impl<'a, 'b, 'c, 'd> Future for TlsQuery<'a, 'b, 'c, 'd> {
    type Output = Result<Message, errors::QueryError>;

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        loop {
            let mut this = self.as_mut().project();
            match this.inner.as_mut().project() {
                QInitQueryProj::Fresh => {
                    this.inner.set_read_active_query(&this.socket.active_queries);

                    // The future for the read-lock will be polled during the next loop.
                    continue;
                },
                QInitQueryProj::ReadActiveQuery(r_active_queries) => {
                    match r_active_queries.as_mut().poll(cx) {
                        Poll::Ready(r_active_queries) => {
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
                                },
                                None => {
                                    // This is a new query and has not yet been registered. Acquire
                                    // the write lock to register it.
                                    drop(r_active_queries);
                                    this.inner.set_write_active_query(&this.socket.active_queries);

                                    // During the next loop, the write-lock will be polled.
                                    continue;
                                },
                            }
                        },
                        Poll::Pending => {
                            // Waiting on the read-lock only. Will be awoken once it is available.
                            return Poll::Pending;
                        },
                    }
                },
                QInitQueryProj::WriteActiveQuery(w_active_queries) => {
                    // Note that the same checks for the read-lock need to be made again in case
                    // something changed between when the lock was dropped and now.
                    match w_active_queries.as_mut().poll(cx) {
                        Poll::Ready(mut w_active_queries) => {
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
                                },
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
                                        let tls_timeout = w_active_queries.tls_timeout;
                                        let result_receiver = result_sender.subscribe();
                                        let socket = this.socket.clone();
                                        let mut query = this.query.clone();
                                        async move {
                                            TlsQueryRunner::new(&socket, &mut query, result_receiver, &tls_timeout).await;
                                        }
                                    });

                                    w_active_queries.in_flight.insert(this.query.id, (result_sender.clone(), join_handle));
                                    w_active_queries.active.insert(this.query.question.clone(), (this.query.id, result_sender));
                                    drop(w_active_queries);

                                    this.inner.set_following(result_receiver);

                                    // The next loop will poll the receiver until a result is
                                    // received from the newly spawned query-runner.
                                    continue;
                                },
                            }
                        },
                        Poll::Pending => {
                            // Waiting on the write-lock only. Will be awoken once it is available.
                            return Poll::Pending;
                        },
                    }
                },
                QInitQueryProj::Following(mut result_receiver) => {
                    match result_receiver.as_mut().poll(cx) {
                        Poll::Ready(Ok(response)) => {
                            this.inner.set_complete();

                            // A message was received. The query is complete. It should not be
                            // polled again.
                            return Poll::Ready(response);
                        },
                        Poll::Ready(Err(once_watch::RecvError::Closed)) => {
                            let error = errors::QueryError::from(errors::SocketError::Shutdown(
                                errors::SocketType::Tls,
                                errors::SocketStage::Connected,
                            ));

                            this.inner.set_complete();

                            // The query runner closed the channel for some reason. The query is
                            // complete. It should not be polled again.
                            return Poll::Ready(Err(error));
                        },
                        Poll::Pending => {
                            // Waiting to receive a response. The future will be awoken once a
                            // message is sent over the channel or it is closed.
                            return Poll::Pending;
                        },
                    }
                },
                QInitQueryProj::Complete => {
                    panic!("TlsQuery cannot be polled after completion")
                },
            }
        }
    }
}

// Implement TLS functions on TlsSocket
#[async_trait]
impl crate::socket::tls::TlsSocket for TlsSocket {
    #[inline]
    fn peer_addr(&self) ->  SocketAddr {
        SocketAddr::new(self.upstream_address, DOT_TCP_PORT)
    }

    #[inline]
    fn peer_name(&self) ->  &CDomainName {
        &self.ns_name
    }

    #[inline]
    fn state(&self) ->  &RwLock<TlsState>  {
        &self.tls
    }

    #[inline]
    fn client_config(&self) -> &Arc<ClientConfig> {
        &self.client_config
    }

    #[inline]
    async fn listen(self: Arc<Self>, mut tls_reader: TlsReadHalf, kill_tls: AwakeToken) {
        pin!(let kill_tls_awoken = kill_tls.awoken(););
        loop {
            select! {
                biased;
                () = &mut kill_tls_awoken => {
                    println!("TLS Socket {} Canceled. Shutting down TLS Listener.", self.upstream_address);
                    break;
                },
                () = tokio::time::sleep(TCP_LISTEN_TIMEOUT) => {
                    println!("TLS Socket {} Timed Out. Shutting down TLS Listener.", self.upstream_address);
                    break;
                },
                response = read_stream_message::<{ MAX_MESSAGE_SIZE as usize }>(&mut tls_reader, errors::SocketType::Tls) => {
                    match response {
                        Ok(response) => {
                            self.recent_messages_received.store(true, Ordering::Release);
                            let response_id = response.id;
                            println!("Received TLS Response: {response:?}");
                            let r_active_queries = self.active_queries.read().await;
                            if let Some((sender, _)) = r_active_queries.in_flight.get(&response_id) {
                                let _ = sender.send(Ok(response));
                            };
                            drop(r_active_queries);
                            // Cleanup is handled by the management processes. This
                            // process is free to move on.
                        },
                        Err(error) => {
                            println!("{error}");
                            break;
                        },
                    }
                },
            }
        }

        self.listen_tls_cleanup(kill_tls).await;
    }
}

impl TlsSocket {
    #[inline]
    async fn listen_tls_cleanup(self: Arc<Self>, kill_tls: AwakeToken) {
        println!("Cleaning up TLS socket {}", self.upstream_address);

        let mut w_state = self.tls.write().await;
        match &*w_state {
            TlsState::Managed { socket: _, kill: managed_kill_tls } => {
                // If the managed socket is the one that we are cleaning up...
                if &kill_tls == managed_kill_tls {
                    // We are responsible for cleanup.
                    *w_state = TlsState::None;
                    drop(w_state);

                    kill_tls.awake();

                // If the managed socket isn't the one that we are cleaning up...
                } else {
                    // This is not our socket to clean up.
                    drop(w_state);
                }
            },
            TlsState::Establishing { sender: _, kill: _ }
          | TlsState::None
          | TlsState::Blocked => {
                // This is not our socket to clean up.
                drop(w_state);
            }
        }
    }
}

struct ActiveQueries {
    tls_timeout: Duration,

    in_flight: HashMap<u16, (once_watch::Sender<Result<Message, errors::QueryError>>, JoinHandle<()>)>,
    active: HashMap<TinyVec<[Question; 1]>, (u16, once_watch::Sender<Result<Message, errors::QueryError>>)>,
}

impl ActiveQueries {
    #[inline]
    pub fn new() -> Self {
        Self {
            tls_timeout: INIT_TCP_TIMEOUT,

            in_flight: HashMap::new(),
            active: HashMap::new(),
        }
    }
}

pub struct TlsSocket {
    ns_name: CDomainName,
    upstream_address: IpAddr,
    tls: RwLock<TlsState>,
    active_queries: RwLock<ActiveQueries>,
    client_config: Arc<ClientConfig>,

    // Rolling averages
    average_tls_response_time: Atomic<RollingAverage>,
    average_tls_dropped_packets: Atomic<RollingAverage>,

    // Counters used to determine when the socket should be closed.
    recent_messages_sent: AtomicBool,
    recent_messages_received: AtomicBool,
}

impl TlsSocket {
    #[inline]
    pub fn new(upstream_address: IpAddr, ns_name: CDomainName, client_config: Arc<ClientConfig>) -> Arc<Self> {
        Arc::new(TlsSocket {
            ns_name,
            upstream_address,
            tls: RwLock::new(TlsState::None),
            active_queries: RwLock::new(ActiveQueries::new()),
            client_config,

            average_tls_response_time: Atomic::new(RollingAverage::new()),
            average_tls_dropped_packets: Atomic::new(RollingAverage::new()),

            recent_messages_sent: AtomicBool::new(false),
            recent_messages_received: AtomicBool::new(false),
        })
    }

    #[inline]
    pub fn average_tls_response_time(&self) -> f64 {
        self.average_tls_response_time.load(Ordering::Acquire).current_average()
    }

    #[inline]
    pub fn average_dropped_tls_packets(&self) -> f64 {
        self.average_tls_dropped_packets.load(Ordering::Acquire).current_average()
    }

    #[inline]
    fn add_dropped_packet_to_tls_average(&self) -> RollingAverage {
        // We can use relaxed memory orderings with the rolling average because it is not being used
        // for synchronization nor do we care about the order of atomic operations. We only care
        // that the operation is atomic.
        fetch_update(
            &self.average_tls_dropped_packets,
            Ordering::Relaxed,
            Ordering::Relaxed,
            |average| average.put_next(1, ROLLING_AVERAGE_TCP_MAX_DROPPED)
        )
    }

    #[inline]
    fn add_response_time_to_tls_average(&self, response_time: Duration) -> (RollingAverage, RollingAverage) {
        // We can use relaxed memory orderings with the rolling average because it is not being used
        // for synchronization nor do we care about the order of atomic operations. We only care
        // that the operation is atomic.
        (
            fetch_update(
                &self.average_tls_response_time,
                Ordering::Relaxed,
                Ordering::Relaxed,
                |average| average.put_next(u32::try_from(response_time.as_millis()).unwrap_or(u32::MAX), ROLLING_AVERAGE_TCP_MAX_RESPONSE_TIMES)
            ),
            fetch_update(
                &self.average_tls_dropped_packets,
                Ordering::Relaxed,
                Ordering::Relaxed,
                |average| average.put_next(0, ROLLING_AVERAGE_TCP_MAX_DROPPED)
            )
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
            self.recent_messages_received.load(Ordering::Acquire)
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
            self.recent_messages_received.swap(false, Ordering::AcqRel)
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
        <Self as crate::socket::tls::TlsSocket>::start(self).await
    }

    #[inline]
    pub async fn shutdown(self: Arc<Self>) {
        <Self as crate::socket::tls::TlsSocket>::shutdown(self).await;
    }

    #[inline]
    pub async fn enable(self: Arc<Self>) {
        <Self as crate::socket::tls::TlsSocket>::enable(self).await;
    }

    #[inline]
    pub async fn disable(self: Arc<Self>) {
        <Self as crate::socket::tls::TlsSocket>::disable(self).await;
    }

    pub fn query<'a, 'b, 'c, 'd>(self: &'a Arc<Self>, query: &'b mut Message, options: QueryOpt) -> TlsQuery<'a, 'b, 'c, 'd> {
        // If the UDP socket is unreliable, send most data via TLS. Some queries should still use
        // UDP to determine if the network conditions are improving. However, if the TLS connection
        // is also unstable, then we should not rely on it.
        let query_task = match options {
            QueryOpt::UdpTcp => todo!(),
            QueryOpt::Tcp => todo!(),
            QueryOpt::Quic => todo!(),
            QueryOpt::Tls => TlsQuery::new(&self, query),
            QueryOpt::QuicTls => todo!(),
            QueryOpt::Https => todo!(),
        };

        return query_task;
    }
}

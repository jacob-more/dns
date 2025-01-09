use std::{cmp::{max, min}, collections::HashMap, future::Future, net::{IpAddr, SocketAddr}, num::NonZeroU8, pin::Pin, sync::{atomic::{AtomicBool, Ordering}, Arc}, task::Poll, time::Duration};

use async_lib::{awake_token::AwakeToken, once_watch::{self, OnceWatchSend, OnceWatchSubscribe}};
use async_trait::async_trait;
use atomic::Atomic;
use dns_lib::{interface::ports::{DNS_TCP_PORT, DNS_UDP_PORT}, query::{message::Message, question::Question}, serde::wire::{to_wire::ToWire, write_wire::WriteWire}, types::c_domain_name::CompressionMap};
use futures::FutureExt;
use pin_project::{pin_project, pinned_drop};
use tinyvec::TinyVec;
use tokio::{io::AsyncWriteExt, join, net::{self, tcp::OwnedReadHalf}, pin, select, task::JoinHandle, time::{Instant, Sleep}};

use crate::{async_query::{QInitQuery, QInitQueryProj, QSend, QSendProj, QSendType, QueryOpt}, errors, receive::{read_stream_message, read_udp_message}, rolling_average::{fetch_update, RollingAverage}, socket::{tcp::{QTcpSocket, QTcpSocketProj, TcpSocket, TcpState}, udp::{QUdpSocket, QUdpSocketProj, UdpSocket, UdpState}, udp_tcp::{QUdpTcpSocket, QUdpTcpSocketProj}, FutureSocket, PollSocket}};

const MAX_MESSAGE_SIZE: u16 = 4092;

const MILLISECONDS_IN_1_SECOND: f64 = 1000.0;

pub(crate) const TCP_INIT_TIMEOUT: Duration = Duration::from_secs(5);
pub(crate) const TCP_LISTEN_TIMEOUT: Duration = Duration::from_secs(120);
pub(crate) const UDP_LISTEN_TIMEOUT: Duration = Duration::from_secs(120);

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

/// The initial UDP retransmission timeout, used when setting up a socket, before anything is known
/// about the average response time.
pub(crate) const INIT_UDP_RETRANSMISSION_TIMEOUT: Duration = Duration::from_millis(500);
/// The percentage of the average UDP response time that the timeout should be set to. Currently,
/// this represents 150%. If the average response time were 20 ms, then the retransmission timeout
/// would be 30 ms.
pub(crate) const UDP_RETRANSMISSION_TIMEOUT_DURATION_ABOVE_UDP_RESPONSE_TIME: f64 = 1.50;
/// The maximum percentage of the average UDP response time that the timeout should be set to.
/// Currently, this represents 250%. If the average response time were 20 ms, then the
/// retransmission timeout would be 60 ms.
pub(crate) const UDP_RETRANSMISSION_TIMEOUT_MAX_DURATION_ABOVE_UDP_RESPONSE_TIME: f64 = 3.00;
/// The step size to use if INCREASE_UDP_RETRANSMISSION_TIMEOUT_DROPPED_AVERAGE_THRESHOLD is
/// exceeded.
pub(crate) const UDP_RETRANSMISSION_TIMEOUT_STEP_WHEN_DROPPED_THRESHOLD_EXCEEDED: Duration = Duration::from_millis(50);
/// When 20% or more of packets are being dropped, then it is time to start slowing down the socket.
pub(crate) const INCREASE_UDP_RETRANSMISSION_TIMEOUT_DROPPED_AVERAGE_THRESHOLD: f64 = 0.20;
/// When 1% or more of packets are being dropped, then we might want to try speeding up the socket
/// again, to reflect the average response time.
pub(crate) const DECREASE_UDP_RETRANSMISSION_TIMEOUT_DROPPED_AVERAGE_THRESHOLD: f64 = 0.01;
/// The maximum allowable UDP retransmission timeout.
pub(crate) const MAX_UDP_RETRANSMISSION_TIMEOUT: Duration = Duration::from_secs(10);
/// The minimum allowable UDP retransmission timeout.
pub(crate) const MIN_UDP_RETRANSMISSION_TIMEOUT: Duration = Duration::from_millis(50);

/// The initial UDP timeout, used when setting up a socket, before anything is known about the
/// average response time.
pub(crate) const INIT_UDP_TIMEOUT: Duration = Duration::from_millis(500);
/// The number of UDP retransmission that are allowed for a mixed UDP-TCP query.
pub(crate) const UDP_RETRANSMISSIONS: u8 = 1;
/// The percentage of the average UDP response time that the timeout should be set to. Currently,
/// this represents 200%. If the average response time were 20 ms, then the timeout would be 40 ms.
pub(crate) const UDP_TIMEOUT_DURATION_ABOVE_UDP_RESPONSE_TIME: f64 = 2.00;
/// The maximum percentage of the average UDP response time that the timeout should be set to.
/// Currently, this represents 400%. If the average response time were 20 ms, then the
/// retransmission timeout would be 80 ms.
pub(crate) const UDP_TIMEOUT_MAX_DURATION_ABOVE_UDP_RESPONSE_TIME: f64 = 4.00;
/// The step size to use if INCREASE_UDP_TIMEOUT_DROPPED_AVERAGE_THRESHOLD is
/// exceeded.
pub(crate) const UDP_TIMEOUT_STEP_WHEN_DROPPED_THRESHOLD_EXCEEDED: Duration = Duration::from_millis(50);
/// When 20% or more of packets are being dropped, then it is time to start slowing down the socket.
pub(crate) const INCREASE_UDP_TIMEOUT_DROPPED_AVERAGE_THRESHOLD: f64 = 0.20;
/// When 1% or more of packets are being dropped, then we might want to try speeding up the socket
/// again, to reflect the average response time.
pub(crate) const DECREASE_UDP_TIMEOUT_DROPPED_AVERAGE_THRESHOLD: f64 = 0.01;
/// The maximum allowable UDP timeout.
pub(crate) const MAX_UDP_TIMEOUT: Duration = Duration::from_secs(10);
/// The minimum allowable UDP timeout.
pub(crate) const MIN_UDP_TIMEOUT: Duration = Duration::from_millis(50);

// Using the safe checked version of new is not stable. As long as we always use non-zero constants,
// there should not be any problems with this.
pub(crate) const ROLLING_AVERAGE_TCP_MAX_DROPPED: NonZeroU8        = unsafe { NonZeroU8::new_unchecked(11) };
pub(crate) const ROLLING_AVERAGE_TCP_MAX_RESPONSE_TIMES: NonZeroU8 = unsafe { NonZeroU8::new_unchecked(13) };
pub(crate) const ROLLING_AVERAGE_UDP_MAX_DROPPED: NonZeroU8        = unsafe { NonZeroU8::new_unchecked(11) };
pub(crate) const ROLLING_AVERAGE_UDP_MAX_RESPONSE_TIMES: NonZeroU8 = unsafe { NonZeroU8::new_unchecked(13) };
pub(crate) const ROLLING_AVERAGE_UDP_MAX_TRUNCATED: NonZeroU8      = unsafe { NonZeroU8::new_unchecked(50) };

fn bound<T>(value: T, lower_bound: T, upper_bound: T) -> T where T: Ord {
    debug_assert!(lower_bound <= upper_bound);
    value.clamp(lower_bound, upper_bound)
}

#[pin_project(project = MixedQueryProj)]
pub enum MixedQuery<'a, 'b> {
    Tcp(#[pin] TcpQuery<'a, 'b>),
    Udp(#[pin] UdpQuery<'a, 'b>),
}

impl<'a, 'b> Future for MixedQuery<'a, 'b> {
    type Output = Result<Message, errors::QueryError>;

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        match self.project() {
            MixedQueryProj::Tcp(tcp_query) => tcp_query.poll(cx),
            MixedQueryProj::Udp(udp_query) => udp_query.poll(cx),
        }
    }
}

enum TcpResponseTime {
    Dropped,
    Responded(Duration),
    /// `None` is used for cases where the message was never sent (e.g. serialization errors) or the
    /// socket was closed before a response could be received.
    None,
}

enum UdpResponseTime {
    Dropped,
    UdpDroppedTcpResponded(Duration),
    Responded {
        execution_time: Duration,
        truncated: bool,
    },
    /// `None` is used for cases where the message was never sent (e.g. serialization errors) or the
    /// socket was closed before a response could be received.
    None,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
enum LoopPoll {
    Continue,
    Pending,
}

#[pin_project(PinnedDrop)]
struct TcpQueryRunner<'a, 'b, 'e, 'h> {
    socket: &'a Arc<MixedSocket>,
    query: &'b mut Message,
    tcp_timeout: &'h Duration,
    tcp_start_time: Instant,
    #[pin]
    timeout: Sleep,
    #[pin]
    result_receiver: once_watch::Receiver<Result<Message, errors::QueryError>>,
    #[pin]
    inner: InnerTQ<'e>,
}

impl<'a, 'b, 'e, 'f, 'h> TcpQueryRunner<'a, 'b, 'e, 'h> {
    #[inline]
    pub fn new(socket: &'a Arc<MixedSocket>, query: &'b mut Message, result_receiver: once_watch::Receiver<Result<Message, errors::QueryError>>, tcp_timeout: &'h Duration) -> Self {
        Self {
            socket,
            query,
            tcp_timeout,
            tcp_start_time: Instant::now(),
            timeout: tokio::time::sleep(*tcp_timeout),
            result_receiver,
            inner: InnerTQ::Fresh,
        }
    }
}

#[pin_project(project = InnerTQProj)]
enum InnerTQ<'e> {
    Fresh,
    Running {
        #[pin]
        tq_socket: QTcpSocket,
        #[pin]
        send_query: QSend<'e, QSendType, errors::SendError>,
    },
    Cleanup(TcpResponseTime),
    Complete,
}

impl<'e> InnerTQ<'e> {
    #[inline]
    pub fn set_running(mut self: std::pin::Pin<&mut Self>, query_type: QSendType) {
        self.set(Self::Running {
            tq_socket: QTcpSocket::Fresh,
            send_query: QSend::Fresh(query_type),
        });
    }

    #[inline]
    pub fn set_cleanup(mut self: std::pin::Pin<&mut Self>, execution_time: TcpResponseTime) {
        self.set(Self::Cleanup(execution_time));
    }

    #[inline]
    pub fn set_complete(mut self: std::pin::Pin<&mut Self>) {
        self.set(Self::Complete);
    }
}

impl<'a, 'b, 'e, 'h> Future for TcpQueryRunner<'a, 'b, 'e, 'h> {
    type Output = ();

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let mut this = self.as_mut().project();
        match this.inner.as_mut().project() {
            InnerTQProj::Fresh
          | InnerTQProj::Running { tq_socket: _, send_query: _ } => {
                if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                    let _ = this.result_receiver.get_sender().send(Err(errors::QueryError::Timeout));

                    this.inner.set_cleanup(TcpResponseTime::Dropped);

                    // Exit loop forever: query timed out.
                    // Because the in-flight map was set up before this future was created, we are
                    // still responsible for cleanup.
                }
            },
            InnerTQProj::Cleanup(_)
          | InnerTQProj::Complete => {
                // Not allowed to timeout. This is a cleanup state.
            },
        }

        loop {
            let mut this = self.as_mut().project();
            match this.inner.as_mut().project() {
                InnerTQProj::Fresh => {
                    this.inner.set_running(QSendType::Initial);

                    // Next loop: poll tq_socket and in_flight to start getting the TCP socket and
                    // inserting the query ID into the in-flight map.
                    continue;
                },
                InnerTQProj::Running { mut tq_socket, mut send_query } => {
                    match (send_query.as_mut().project(), tq_socket.as_mut().project()) {
                        (QSendProj::Fresh(_), QTcpSocketProj::Fresh)
                      | (QSendProj::Fresh(_), QTcpSocketProj::GetTcpEstablishing { receive_tcp_socket: _ })
                      | (QSendProj::Fresh(_), QTcpSocketProj::InitTcp { join_handle: _ })
                      | (QSendProj::Fresh(_), QTcpSocketProj::Closed(_)) => {
                            // We don't poll the result_receiver until the QSend state is Complete
                            // since you can't receive a message until a query has been sent. If
                            // there is an error while trying to send the query, we should send a
                            // QError over that channel to make sure any tasks waiting on it can
                            // handle the error appropriately.

                            match tq_socket.poll(this.socket, cx) {
                                PollSocket::Error(error) => {
                                    let _ = this.result_receiver.get_sender().send(Err(errors::QueryError::from(error)));

                                    this.inner.set_cleanup(TcpResponseTime::None);

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
                                    // We are waiting on the QTcpSocket and the timeout.
                                    // We are already registered with the in-flight map and cannot
                                    // send or receive a query until a socket is established.
                                    return Poll::Pending;
                                },
                            }
                        },
                        (QSendProj::Fresh(_), QTcpSocketProj::Acquired { tcp_socket, kill_tcp: _ }) => {
                            // We don't poll the result_receiver until the QSend state is Complete
                            // since you can't receive a message until a query has been sent. If
                            // there is an error while trying to send the query, we should send a
                            // QError over that channel to make sure any tasks waiting on it can
                            // handle the error appropriately.

                            let socket = this.socket.clone();
                            let tcp_socket = tcp_socket.clone();

                            if let PollSocket::Error(error) = tq_socket.poll(this.socket, cx) {
                                let _ = this.result_receiver.get_sender().send(Err(errors::QueryError::from(error)));

                                this.inner.set_cleanup(TcpResponseTime::None);

                                // Next loop will poll for the in-flight map lock to remove the
                                // query ID and record socket statistics.
                                continue;
                            }

                            let mut raw_message = [0_u8; MAX_MESSAGE_SIZE as usize];
                            let mut write_wire = WriteWire::from_bytes(&mut raw_message);
                            if let Err(wire_error) = this.query.to_wire_format_with_two_octet_length(&mut write_wire, &mut Some(CompressionMap::new())) {
                                let _ = this.result_receiver.get_sender().send(Err(errors::QueryError::from(errors::SendError::from(wire_error))));

                                this.inner.set_cleanup(TcpResponseTime::None);

                                // Next loop will poll for the in-flight map lock to remove the
                                // query ID and record socket statistics.
                                continue;
                            };
                            let wire_length = write_wire.current_len();

                            println!("Sending on TCP socket {} {{ drop rate {:.2}%, truncation rate {:.2}%, response time {:.2} ms, timeout {} ms }} :: {:?}", this.socket.upstream_address, this.socket.average_dropped_tcp_packets() * 100.0, this.socket.average_truncated_udp_packets() * 100.0, this.socket.average_tcp_response_time(), this.tcp_timeout.as_millis(), this.query);

                            let send_query_future = async move {
                                let socket = socket;
                                let tcp_socket = tcp_socket;
                                let wire_length = wire_length;

                                socket.recent_messages_sent.store(true, Ordering::Release);

                                let mut w_tcp_stream = tcp_socket.lock().await;
                                let bytes_written = match w_tcp_stream.write(&raw_message[..wire_length]).await {
                                    Ok(bytes) => bytes,
                                    Err(error) => {
                                        let io_error = errors::IoError::from(error);
                                        let send_error = errors::SendError::Io {
                                            socket_type: errors::SocketType::Tcp,
                                            error: io_error,
                                        };
                                        return Err(send_error);
                                    },
                                };
                                drop(w_tcp_stream);

                                // Verify that the correct number of bytes were written.
                                if bytes_written != wire_length {
                                    return Err(errors::SendError::IncorrectNumberBytes {
                                        socket_type: errors::SocketType::Tcp,
                                        expected: wire_length as u16,
                                        sent: bytes_written
                                    });
                                }

                                return Ok(());
                            }.boxed();

                            send_query.set_send_query(send_query_future);

                            // Next loop will begin to poll QSend. This will get the lock and the
                            // TcpStream and write the bytes out.
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

                                    this.inner.set_cleanup(TcpResponseTime::None);

                                    // Next loop will poll for the in-flight map lock to remove the
                                    // query ID and record socket statistics.
                                    continue;
                                },
                                (Poll::Ready(Err(error)), _) => {
                                    let _ = this.result_receiver.get_sender().send(Err(errors::QueryError::from(error)));

                                    this.inner.set_cleanup(TcpResponseTime::None);

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
                                    // Will wake up if the QTcpSocket wakes us (most likely because
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
                                    let execution_time = this.tcp_start_time.elapsed();

                                    this.inner.set_cleanup(TcpResponseTime::Responded(execution_time));

                                    // Next loop will poll for the in-flight map lock to remove the
                                    // query ID and record socket statistics.
                                    continue;
                                },
                                Poll::Ready(Ok(Err(_)))
                              | Poll::Ready(Err(_)) => {
                                    this.inner.set_cleanup(TcpResponseTime::None);

                                    // Next loop will poll for the in-flight map lock to remove the
                                    // query ID and record socket statistics.
                                    continue;
                                },
                                Poll::Pending => {
                                    // Nothing has been received yet. Whether we return Pending or
                                    // Continue will be determined when the TCP socket is polled.
                                },
                            }

                            match tq_socket.poll(this.socket, cx) {
                                PollSocket::Error(error) => {
                                    let _ = this.result_receiver.get_sender().send(Err(errors::QueryError::from(error)));

                                    this.inner.set_cleanup(TcpResponseTime::None);

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
                                    // Will wake up if the QTcpSocket wakes us (most likely because
                                    // the socket was killed), the receiver has a response, the
                                    // receiver is closed, or the timeout occurs.
                                    return Poll::Pending;
                                },
                            }
                        },
                    }
                },
                InnerTQProj::Cleanup(execution_time) => {
                    // Should always transition to the cleanup state before exit. This is
                    // responsible for cleaning up the query ID from the in-flight map (failure to
                    // do so should be considered a memory leak) and for updating the socket
                    // statistics.

                    // We are removing the socket. If a message has not been received, it needs to
                    // be closed so that any processes waiting on this channel wake up and are
                    // cleaned up too.
                    this.result_receiver.close();

                    let mut w_active_queries = this.socket.active_queries.write().unwrap();
                    match execution_time {
                        TcpResponseTime::Dropped => {
                            let average_tcp_dropped_packets = this.socket.add_dropped_packet_to_tcp_average();
                            let average_tcp_response_time = this.socket.average_tcp_response_time();
                            if average_tcp_response_time.is_finite() {
                                if average_tcp_dropped_packets.current_average() >= INCREASE_TCP_TIMEOUT_DROPPED_AVERAGE_THRESHOLD {
                                    w_active_queries.tcp_timeout = bound(
                                        min(
                                            w_active_queries.tcp_timeout.saturating_add(TCP_TIMEOUT_STEP_WHEN_DROPPED_THRESHOLD_EXCEEDED),
                                            Duration::from_secs_f64(average_tcp_response_time * TCP_TIMEOUT_MAX_DURATION_ABOVE_TCP_RESPONSE_TIME / MILLISECONDS_IN_1_SECOND),
                                        ),
                                        MIN_TCP_TIMEOUT,
                                        MAX_TCP_TIMEOUT,
                                    );
                                }
                            } else {
                                if average_tcp_dropped_packets.current_average() >= INCREASE_TCP_TIMEOUT_DROPPED_AVERAGE_THRESHOLD {
                                    w_active_queries.tcp_timeout = bound(
                                        w_active_queries.tcp_timeout.saturating_add(TCP_TIMEOUT_STEP_WHEN_DROPPED_THRESHOLD_EXCEEDED),
                                        MIN_TCP_TIMEOUT,
                                        MAX_TCP_TIMEOUT,
                                    );
                                }
                            }
                        },
                        TcpResponseTime::Responded(response_time) => {
                            let (average_tcp_response_time, average_tcp_dropped_packets) = this.socket.add_response_time_to_tcp_average(*response_time);
                            if average_tcp_dropped_packets.current_average() <= DECREASE_TCP_TIMEOUT_DROPPED_AVERAGE_THRESHOLD {
                                w_active_queries.tcp_timeout = bound(
                                    max(
                                        w_active_queries.tcp_timeout.saturating_add(TCP_TIMEOUT_STEP_WHEN_DROPPED_THRESHOLD_EXCEEDED),
                                        Duration::from_secs_f64(average_tcp_response_time.current_average() * TCP_TIMEOUT_DURATION_ABOVE_TCP_RESPONSE_TIME / MILLISECONDS_IN_1_SECOND),
                                    ),
                                    MIN_TCP_TIMEOUT,
                                    MAX_TCP_TIMEOUT,
                                );
                            }
                        },
                        TcpResponseTime::None => (),
                    }

                    // We are responsible for clearing these maps. Otherwise, the memory
                    // will only ever be cleaned up when the socket itself is dropped.
                    w_active_queries.in_flight.remove(&this.query.id);
                    w_active_queries.tcp_only.remove(&this.query.question);
                    drop(w_active_queries);

                    this.inner.set_complete();

                    // Socket should not be polled again.
                    return Poll::Ready(());
                },
                InnerTQProj::Complete => {
                    panic!("TCP only query polled after completion");
                },
            }
        }
    }
}

#[pinned_drop]
impl<'a, 'b, 'e, 'h> PinnedDrop for TcpQueryRunner<'a, 'b, 'e, 'h> {
    fn drop(mut self: Pin<&mut Self>) {
        match self.as_mut().project().inner.as_mut().project() {
            InnerTQProj::Fresh
          | InnerTQProj::Running { tq_socket: _, send_query: _ }
          | InnerTQProj::Cleanup(_) => {
                let mut w_active_queries = self.socket.active_queries.write().unwrap();
                let _ = w_active_queries.in_flight.remove(&self.query.id);
                let _ = w_active_queries.tcp_only.remove(&self.query.question);
                drop(w_active_queries);
            },
            InnerTQProj::Complete => {
                // Nothing to do for active queries. Already done cleaning up.
            }
        }
    }
}

#[pin_project]
struct TcpQuery<'a, 'b> {
    socket: &'a Arc<MixedSocket>,
    query: &'b mut Message,
    #[pin]
    inner: QInitQuery,
}

impl<'a, 'b> TcpQuery<'a, 'b> {
    #[inline]
    pub fn new(socket: &'a Arc<MixedSocket>, query: &'b mut Message) -> Self {
        Self {
            socket,
            query,
            inner: QInitQuery::Fresh,
        }
    }
}

impl<'a, 'b> Future for TcpQuery<'a, 'b> {
    type Output = Result<Message, errors::QueryError>;

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        loop {
            let mut this = self.as_mut().project();
            match this.inner.as_mut().project() {
                QInitQueryProj::Fresh => {
                    let r_active_queries = this.socket.active_queries.read().unwrap();
                    match r_active_queries.tcp_only.get(&this.query.question) {
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

                            this.inner.set_write_active_query();

                            // During the next loop, the write-lock will be polled.
                            continue;
                        },
                    }
                },
                QInitQueryProj::WriteActiveQuery => {
                    // Note that the same checks for the read-lock need to be made again in case
                    // something changed between when the lock was dropped and now.
                    let mut w_active_queries = this.socket.active_queries.write().unwrap();
                    match w_active_queries.tcp_only.get(&this.query.question) {
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
                                let tcp_timeout = w_active_queries.tcp_timeout;
                                let result_receiver = result_sender.subscribe();
                                let socket = this.socket.clone();
                                let mut query = this.query.clone();
                                async move {
                                    TcpQueryRunner::new(&socket, &mut query, result_receiver, &tcp_timeout).await;
                                }
                            });

                            w_active_queries.in_flight.insert(this.query.id, (result_sender.clone(), join_handle));
                            w_active_queries.tcp_only.insert(this.query.question.clone(), (this.query.id, result_sender));
                            drop(w_active_queries);

                            this.inner.set_following(result_receiver);

                            // The next loop will poll the receiver until a result is
                            // received from the newly spawned query-runner.
                            continue;
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
                                errors::SocketType::Tcp,
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
                    panic!("TcpQuery cannot be polled after completion")
                },
            }
        }
    }
}

// Implement TCP functions on MixedSocket
#[async_trait]
impl TcpSocket for MixedSocket {
    #[inline]
    fn peer_addr(&self) -> SocketAddr {
        SocketAddr::new(self.upstream_address, DNS_TCP_PORT)
    }

    #[inline]
    fn state(&self) ->  &std::sync::RwLock<TcpState>  {
        &self.tcp
    }

    #[inline]
    async fn listen(self: Arc<Self>, mut tcp_reader: OwnedReadHalf, kill_tcp: AwakeToken) {
        pin!(let kill_tcp_awoken = kill_tcp.awoken(););
        loop {
            select! {
                biased;
                () = &mut kill_tcp_awoken => {
                    println!("TCP Socket {} Canceled. Shutting down TCP Listener.", self.upstream_address);
                    break;
                },
                () = tokio::time::sleep(TCP_LISTEN_TIMEOUT) => {
                    println!("TCP Socket {} Timed Out. Shutting down TCP Listener.", self.upstream_address);
                    break;
                },
                response = read_stream_message::<{ MAX_MESSAGE_SIZE as usize }>(&mut tcp_reader, errors::SocketType::Tcp) => {
                    match response {
                        Ok(response) => {
                            self.recent_messages_received.store(true, Ordering::Release);
                            let response_id = response.id;
                            println!("Received TCP Response: {response:?}");
                            let r_active_queries = self.active_queries.read().unwrap();
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

        self.listen_tcp_cleanup(kill_tcp).await;
    }
}

impl MixedSocket {
    #[inline]
    async fn listen_tcp_cleanup(self: Arc<Self>, kill_tcp: AwakeToken) {
        println!("Cleaning up TCP socket {}", self.upstream_address);

        let mut w_state = self.tcp.write().unwrap();
        match &*w_state {
            TcpState::Managed { socket: _, kill: managed_kill_tcp } => {
                // If the managed socket is the one that we are cleaning up...
                if &kill_tcp == managed_kill_tcp {
                    // We are responsible for cleanup.
                    *w_state = TcpState::None;
                    drop(w_state);

                    kill_tcp.awake();

                // If the managed socket isn't the one that we are cleaning up...
                } else {
                    // This is not our socket to clean up.
                    drop(w_state);
                }
            },
            TcpState::Establishing { sender: _, kill: _ }
          | TcpState::None
          | TcpState::Blocked => {
                // This is not our socket to clean up.
                drop(w_state);
            }
        }
    }
}

#[pin_project(PinnedDrop)]
struct UdpQueryRunner<'a, 'b, 'c, 'f, 'i>
where
    'a: 'c,
{
    socket: &'a Arc<MixedSocket>,
    query: &'b mut Message,
    udp_retransmission_timeout: &'i Duration,
    udp_timeout: &'i Duration,
    tcp_start_time: Instant,
    udp_start_time: Instant,
    #[pin]
    timeout: Sleep,
    #[pin]
    result_receiver: once_watch::Receiver<Result<Message, errors::QueryError>>,
    #[pin]
    inner: InnerUQ<'c, 'f>,
}

impl<'a, 'b, 'c, 'f, 'i> UdpQueryRunner<'a, 'b, 'c, 'f, 'i> {
    #[inline]
    pub fn new(socket: &'a Arc<MixedSocket>, query: &'b mut Message, result_receiver: once_watch::Receiver<Result<Message, errors::QueryError>>, udp_retransmission_timeout: &'i Duration, udp_timeout: &'i Duration) -> Self {
        Self {
            socket,
            query,
            udp_retransmission_timeout,
            udp_timeout,
            timeout: tokio::time::sleep(*udp_retransmission_timeout),
            result_receiver,
            tcp_start_time: Instant::now(),
            udp_start_time: Instant::now(),
            inner: InnerUQ::Fresh { udp_retransmissions: UDP_RETRANSMISSIONS },
        }
    }

    #[inline]
    fn reset_timeout(self: std::pin::Pin<&mut Self>, next_timeout: Duration) {
        let now = Instant::now();
        match now.checked_add(next_timeout) {
            Some(new_deadline) => self.project().timeout.reset(new_deadline),
            None => self.project().timeout.reset(now),
        }
    }
}

#[pin_project(project = InnerUQProj)]
enum InnerUQ<'c, 'f> {
    Fresh { udp_retransmissions: u8 },
    Running {
        #[pin]
        socket: QUdpTcpSocket<'c>,
        #[pin]
        send_query: QSend<'f, QSendType, errors::SendError>,
    },
    Cleanup(UdpResponseTime),
    Complete,
}

impl<'c, 'f> InnerUQ<'c, 'f> {
    #[inline]
    pub fn set_running_udp(mut self: std::pin::Pin<&mut Self>, udp_retransmissions: u8, query_type: QSendType) {
        self.set(Self::Running {
            socket: QUdpTcpSocket::Udp { uq_socket: QUdpSocket::Fresh, retransmits: udp_retransmissions },
            send_query: QSend::Fresh(query_type),
        });
    }

    #[inline]
    fn set_running_tcp(mut self: std::pin::Pin<&mut Self>, query_type: QSendType) {
        self.set(InnerUQ::Running {
            socket: QUdpTcpSocket::Tcp { tq_socket: QTcpSocket::Fresh },
            send_query: QSend::Fresh(query_type),
        });
    }

    #[inline]
    pub fn set_cleanup(mut self: std::pin::Pin<&mut Self>, execution_time: UdpResponseTime) {
        self.set(Self::Cleanup(execution_time));
    }

    #[inline]
    pub fn set_complete(mut self: std::pin::Pin<&mut Self>) {
        self.set(Self::Complete);
    }
}

impl<'a, 'b, 'c, 'f, 'i> Future for UdpQueryRunner<'a, 'b, 'c, 'f, 'i> {
    type Output = ();

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let mut this = self.as_mut().project();
        match this.inner.as_mut().project() {
            InnerUQProj::Fresh { udp_retransmissions: 0 } => {
                if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                    let new_timeout = **this.udp_timeout;
                    // If we run out of UDP retransmission attempts before the query has even
                    // begun, then it is time to transmit via TCP.
                    // Setting the socket state to QTcpSocket::Fresh will cause the socket to be
                    // initialized (if needed) and then a message sent over that socket.
                    this.inner.set_running_tcp(QSendType::Initial);
                    self.as_mut().reset_timeout(new_timeout);
                }
            },
            InnerUQProj::Fresh { udp_retransmissions: udp_retransmissions @ 1.. } => {
                if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                    let new_timeout = **this.udp_retransmission_timeout;
                    // If we time out before the first query has begin, burn a retransmission. If
                    // this happens too many times, the query will transition to TCP.
                    *udp_retransmissions = udp_retransmissions.saturating_sub(1);
                    self.as_mut().reset_timeout(new_timeout);
                }
            },
            InnerUQProj::Running { mut socket, mut send_query } => {
                match (send_query.as_mut().project(), socket.as_mut().project()) {
                    (QSendProj::Fresh(_), QUdpTcpSocketProj::Udp { uq_socket: _, retransmits: 0 })
                  | (QSendProj::SendQuery(_, _), QUdpTcpSocketProj::Udp { uq_socket: _, retransmits: 0 }) => {
                        if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                            let new_timeout = **this.udp_timeout;
                            // Setting the socket state to QTcpSocket::Fresh will cause the socket
                            // to be initialized (if needed) and then a message sent over that
                            // socket. In other words, this has transitioned into a TCP query.
                            this.inner.set_running_tcp(QSendType::Retransmit);
                            self.as_mut().reset_timeout(new_timeout);
                        }
                    },
                    (QSendProj::Complete(_), QUdpTcpSocketProj::Udp { uq_socket: _, retransmits: 0 }) => {
                        if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                            let new_timeout = **this.udp_timeout;
                            this.socket.add_dropped_packet_to_udp_average();
                            // Setting the socket state to QTcpSocket::Fresh will cause the socket
                            // to be initialized (if needed) and then a message sent over that
                            // socket. In other words, this has transitioned into a TCP query.
                            this.inner.set_running_tcp(QSendType::Retransmit);
                            self.as_mut().reset_timeout(new_timeout);
                        }
                    },
                    (QSendProj::Fresh(_), QUdpTcpSocketProj::Udp { uq_socket: _, retransmits: udp_retransmissions @ 1.. })
                  | (QSendProj::SendQuery(_, _), QUdpTcpSocketProj::Udp { uq_socket: _, retransmits: udp_retransmissions @ 1.. }) => {
                        if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                            let new_timeout = **this.udp_retransmission_timeout;
                            // If we are currently sending a query or have not sent one yet, burn
                            // a retransmission. If this happens too many times, the query will
                            // transition to TCP.
                            *udp_retransmissions = udp_retransmissions.saturating_sub(1);
                            self.as_mut().reset_timeout(new_timeout);
                        }
                    },
                    (QSendProj::Complete(_), QUdpTcpSocketProj::Udp { uq_socket: _, retransmits: udp_retransmissions @ 1.. }) => {
                        if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                            let new_timeout = **this.udp_retransmission_timeout;
                            this.socket.add_dropped_packet_to_udp_average();
                            // A previous query has succeeded. Setting the state to Fresh will
                            // cause the state machine to send another query and drive it to
                            // Complete.
                            send_query.set_fresh(QSendType::Retransmit);
                            *udp_retransmissions = udp_retransmissions.saturating_sub(1);
                            self.as_mut().reset_timeout(new_timeout);
                        }
                    },
                    (_, QUdpTcpSocketProj::Tcp { tq_socket: _ }) => {
                        if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                            // Timeout during TCP occurs when all the UDP queries timed out and the
                            // TCP query timed out. There are no more retransmissions. Setting the
                            // state to Cleanup will cause the query to be removed from the
                            // in-flight map.
                            let _ = this.result_receiver.get_sender().send(Err(errors::QueryError::Timeout));

                            this.inner.set_cleanup(UdpResponseTime::Dropped);
                        }
                    },
                }
            },
            InnerUQProj::Cleanup(_)
          | InnerUQProj::Complete => {
                // Not allowed to timeout. This is a cleanup state.
            },
        }

        loop {
            let mut this = self.as_mut().project();
            match this.inner.as_mut().project() {
                InnerUQProj::Fresh { udp_retransmissions } => {
                    let retransmissions = *udp_retransmissions;

                    this.inner.set_running_udp(retransmissions, QSendType::Initial);

                    // Next loop will start establishing the UDP socket if it is not already set
                    // up. This socket will be used to send a query eventually via UDP.
                    continue;
                },
                InnerUQProj::Running { socket: mut q_socket, mut send_query } => {
                    match (send_query.as_mut().project(), q_socket.as_mut().project()) {
                        // Since we don't know how many retransmissions have been made, we always
                        // poll the result receiver, even if it is set to Fresh or SendQuery.
                        (QSendProj::Fresh(query_type), QUdpTcpSocketProj::Udp { mut uq_socket, retransmits: _ }) => {
                            if let QSendType::Retransmit = query_type {
                                match this.result_receiver.as_mut().poll(cx) {
                                    Poll::Ready(Ok(Ok(Message { id: _, qr: _, opcode: _, authoritative_answer: _, truncation: truncated, recursion_desired: _, recursion_available: _, z: _, rcode: _, question: _, answer: _, authority: _, additional: _ }))) => {
                                        let execution_time = this.udp_start_time.elapsed();

                                        this.inner.set_cleanup(UdpResponseTime::Responded { execution_time, truncated });

                                        // Next loop will poll for the in-flight map lock to remove
                                        // the query ID and record socket statistics.
                                        continue;
                                    },
                                    Poll::Ready(Ok(Err(_)))
                                  | Poll::Ready(Err(_)) => {
                                        this.inner.set_cleanup(UdpResponseTime::Dropped);

                                        // Next loop will poll for the in-flight map lock to remove
                                        // the query ID and record socket statistics.
                                        continue;
                                    },
                                    Poll::Pending => (),
                                }
                            }

                            let uq_socket_result = match uq_socket.poll(this.socket, cx) {
                                PollSocket::Error(error) => {
                                    let _ = this.result_receiver.get_sender().send(Err(errors::QueryError::from(error)));

                                    this.inner.set_cleanup(UdpResponseTime::None);

                                    // Next loop will poll for the in-flight map lock to remove the
                                    // query ID and record socket statistics.
                                    continue;
                                },
                                PollSocket::Continue => LoopPoll::Continue,
                                PollSocket::Pending => LoopPoll::Pending,
                            };

                            if let QUdpSocketProj::Acquired { udp_socket, kill_udp: _ } = uq_socket.as_mut().project() {
                                let mut raw_message = [0_u8; MAX_MESSAGE_SIZE as usize];
                                let mut write_wire = WriteWire::from_bytes(&mut raw_message);
                                if let Err(wire_error) = this.query.to_wire_format(&mut write_wire, &mut Some(CompressionMap::new())) {
                                    let _ = this.result_receiver.get_sender().send(Err(errors::QueryError::from(errors::SendError::from(wire_error))));

                                    this.inner.set_cleanup(UdpResponseTime::None);

                                    // Next loop will poll for the in-flight map lock to remove the
                                    // query ID and record socket statistics.
                                    continue;
                                };
                                let wire_length = write_wire.current_len();

                                let socket: Arc<MixedSocket> = this.socket.clone();
                                let udp_socket = udp_socket.clone();

                                println!("Sending on UDP socket {} {{ drop rate {:.2}%, truncation rate {:.2}%, response time {:.2} ms, timeout {} ms }} :: {:?}", this.socket.upstream_address, this.socket.average_dropped_udp_packets() * 100.0, this.socket.average_truncated_udp_packets() * 100.0, this.socket.average_udp_response_time(), this.udp_retransmission_timeout.as_millis(), this.query);

                                let send_query_future = async move {
                                    let socket = socket;
                                    let udp_socket = udp_socket;
                                    let wire_length = wire_length;

                                    socket.recent_messages_sent.store(true, Ordering::Release);
                                    let bytes_written = match udp_socket.send(&raw_message[..wire_length]).await {
                                        Ok(bytes_written) => bytes_written,
                                        Err(error) => {
                                            let io_error = errors::IoError::from(error);
                                            let send_error = errors::SendError::Io {
                                                socket_type: errors::SocketType::Udp,
                                                error: io_error,
                                            };
                                            return Err(send_error);
                                        },
                                    };
                                    // Verify that the correct number of bytes were written.
                                    if bytes_written != wire_length {
                                        return Err(errors::SendError::IncorrectNumberBytes {
                                            socket_type: errors::SocketType::Udp,
                                            expected: wire_length as u16,
                                            sent: bytes_written
                                        });
                                    }

                                    return Ok(());
                                }.boxed();

                                *this.udp_start_time = Instant::now();
                                send_query.set_send_query(send_query_future);

                                // Next loop will begin to poll SendQuery. This will write the bytes
                                // out.
                                continue;
                            }

                            match uq_socket_result {
                                LoopPoll::Continue => {
                                    // Next loop will poll the UDP socket again and try to drive it
                                    // to the next state (hopefully Acquired!).
                                    continue
                                },
                                LoopPoll::Pending => {
                                    // The UDP socket returned pending and was not in the Acquired
                                    // state. So a message has not yet been sent. Will be awoken by
                                    // timeout or the UDP socket when it is ready.
                                    return Poll::Pending
                                },
                            }
                        },
                        (QSendProj::Fresh(query_type), QUdpTcpSocketProj::Tcp { mut tq_socket }) => {
                            match query_type {
                                QSendType::Initial => (),
                                QSendType::Retransmit => match this.result_receiver.as_mut().poll(cx) {
                                    Poll::Ready(Ok(Ok(Message { id: _, qr: _, opcode: _, authoritative_answer: _, truncation: truncated, recursion_desired: _, recursion_available: _, z: _, rcode: _, question: _, answer: _, authority: _, additional: _ }))) => {
                                        let execution_time = this.udp_start_time.elapsed();

                                        this.inner.set_cleanup(UdpResponseTime::Responded { execution_time, truncated });

                                        // Next loop will poll for the in-flight map lock to remove
                                        // the query ID and record socket statistics.
                                        continue;
                                    },
                                    Poll::Ready(Ok(Err(_)))
                                  | Poll::Ready(Err(_)) => {
                                        this.inner.set_cleanup(UdpResponseTime::Dropped);

                                        // Next loop will poll for the in-flight map lock to remove
                                        // the query ID and record socket statistics.
                                        continue;
                                    },
                                    Poll::Pending => (),
                                },
                            };

                            let tq_socket_result = match tq_socket.poll(this.socket, cx) {
                                PollSocket::Error(error) => {
                                    let _ = this.result_receiver.get_sender().send(Err(errors::QueryError::from(error)));

                                    this.inner.set_cleanup(UdpResponseTime::None);

                                    // Next loop will poll for the in-flight map lock to remove the
                                    // query ID and record socket statistics.
                                    continue;
                                },
                                PollSocket::Continue => LoopPoll::Continue,
                                PollSocket::Pending => LoopPoll::Pending,
                            };

                            if let QTcpSocketProj::Acquired { tcp_socket, kill_tcp: _ } = tq_socket.as_mut().project() {
                                let mut raw_message = [0_u8; MAX_MESSAGE_SIZE as usize];
                                let mut write_wire = WriteWire::from_bytes(&mut raw_message);
                                if let Err(wire_error) = this.query.to_wire_format_with_two_octet_length(&mut write_wire, &mut Some(CompressionMap::new())) {
                                    let _ = this.result_receiver.get_sender().send(Err(errors::QueryError::from(errors::SendError::from(wire_error))));

                                    this.inner.set_cleanup(UdpResponseTime::None);

                                    // Next loop will poll for the in-flight map lock to remove the
                                    // query ID and record socket statistics.
                                    continue;
                                };
                                let wire_length = write_wire.current_len();

                                let socket = this.socket.clone();
                                let tcp_socket = tcp_socket.clone();

                                println!("Sending on TCP socket {} {{ drop rate {:.2}%, truncation rate {:.2}%, response time {:.2} ms, timeout {} ms }} :: {:?}", this.socket.upstream_address, this.socket.average_dropped_tcp_packets() * 100.0, this.socket.average_truncated_udp_packets() * 100.0, this.socket.average_tcp_response_time(), this.udp_timeout.as_millis(), this.query);

                                let send_query_future = async move {
                                    let socket = socket;
                                    let tcp_socket = tcp_socket;
                                    let wire_length = wire_length;

                                    socket.recent_messages_sent.store(true, Ordering::Release);
                                    let mut w_tcp_stream = tcp_socket.lock().await;
                                    let bytes_written = match w_tcp_stream.write(&raw_message[..wire_length]).await {
                                        Ok(bytes_written) => bytes_written,
                                        Err(error) => {
                                            let io_error = errors::IoError::from(error);
                                            let send_error = errors::SendError::Io {
                                                socket_type: errors::SocketType::Tcp,
                                                error: io_error,
                                            };
                                            return Err(send_error);
                                        },
                                    };
                                    drop(w_tcp_stream);
                                    // Verify that the correct number of bytes were written.
                                    if bytes_written != wire_length {
                                        return Err(errors::SendError::IncorrectNumberBytes {
                                            socket_type: errors::SocketType::Tcp,
                                            expected: wire_length as u16,
                                            sent: bytes_written,
                                        });
                                    }

                                    return Ok(());
                                }.boxed();

                                *this.tcp_start_time = Instant::now();
                                send_query.set_send_query(send_query_future);

                                // Next loop will begin to poll SendQuery. This will get the lock and
                                // the TcpStream and write the bytes out.
                                continue;
                            }

                            match tq_socket_result {
                                LoopPoll::Continue => continue,
                                LoopPoll::Pending => return Poll::Pending,
                            }
                        },
                        (QSendProj::SendQuery(query_type, send_query_future), _) => {
                            match query_type {
                                QSendType::Initial => (),
                                QSendType::Retransmit => match this.result_receiver.as_mut().poll(cx) {
                                    Poll::Ready(Ok(Ok(Message { id: _, qr: _, opcode: _, authoritative_answer: _, truncation: truncated, recursion_desired: _, recursion_available: _, z: _, rcode: _, question: _, answer: _, authority: _, additional: _ }))) => {
                                        let execution_time = this.udp_start_time.elapsed();

                                        this.inner.set_cleanup(UdpResponseTime::Responded { execution_time, truncated });

                                        // Next loop will poll for the in-flight map lock to remove
                                        // the query ID and record socket statistics.
                                        continue;
                                    },
                                    Poll::Ready(Ok(Err(_)))
                                  | Poll::Ready(Err(_)) => {
                                        this.inner.set_cleanup(UdpResponseTime::Dropped);

                                        // Next loop will poll for the in-flight map lock to remove
                                        // the query ID and record socket statistics.
                                        continue;
                                    },
                                    Poll::Pending => (),
                                },
                            };

                            let q_socket_result = match q_socket.poll(this.socket, cx) {
                                PollSocket::Error(error) => {
                                    let _ = this.result_receiver.get_sender().send(Err(errors::QueryError::from(error)));

                                    this.inner.set_cleanup(UdpResponseTime::None);

                                    // Next loop will poll for the in-flight map lock to remove the
                                    // query ID and record socket statistics.
                                    continue;
                                },
                                PollSocket::Continue => LoopPoll::Continue,
                                PollSocket::Pending => LoopPoll::Pending,
                            };

                            match send_query_future.as_mut().poll(cx) {
                                Poll::Ready(Err(error)) => {
                                    let _ = this.result_receiver.get_sender().send(Err(errors::QueryError::from(error)));

                                    this.inner.set_cleanup(UdpResponseTime::None);

                                    // Next loop will poll for the in-flight map lock to remove the
                                    // query ID and record socket statistics.
                                    continue;
                                },
                                Poll::Ready(Ok(())) => {
                                    send_query.set_complete();

                                    // Next loop will poll the receiver, now that a message has been
                                    // sent out.
                                    continue;
                                },
                                Poll::Pending => (),
                            }

                            match q_socket_result {
                                LoopPoll::Continue => continue,
                                LoopPoll::Pending => return Poll::Pending,
                            }
                        },
                        (QSendProj::Complete(_), _) => {
                            match this.result_receiver.as_mut().poll(cx) {
                                Poll::Ready(Ok(Ok(Message { id: _, qr: _, opcode: _, authoritative_answer: _, truncation: truncated, recursion_desired: _, recursion_available: _, z: _, rcode: _, question: _, answer: _, authority: _, additional: _ }))) => {
                                    let execution_time = this.udp_start_time.elapsed();

                                    this.inner.set_cleanup(UdpResponseTime::Responded { execution_time, truncated });

                                    // Next loop will poll for the in-flight map lock to remove the
                                    // query ID and record socket statistics.
                                    continue;
                                },
                                Poll::Ready(Ok(Err(_)))
                              | Poll::Ready(Err(_)) => {
                                    this.inner.set_cleanup(UdpResponseTime::Dropped);

                                    // Next loop will poll for the in-flight map lock to remove the
                                    // query ID and record socket statistics.
                                    continue;
                                },
                                Poll::Pending => (),
                            }

                            match q_socket.poll(this.socket, cx) {
                                PollSocket::Error(error) => {
                                    let _ = this.result_receiver.get_sender().send(Err(errors::QueryError::from(error)));

                                    this.inner.set_cleanup(UdpResponseTime::None);

                                    // Next loop will poll for the in-flight map lock to remove the
                                    // query ID and record socket statistics.
                                    continue;
                                },
                                PollSocket::Continue => continue,
                                PollSocket::Pending => return Poll::Pending,
                            };
                        },
                    }
                },
                InnerUQProj::Cleanup(execution_time) => {
                    this.result_receiver.close();

                    let mut w_active_queries = this.socket.active_queries.write().unwrap();
                    match execution_time {
                        UdpResponseTime::Dropped => {
                            let average_udp_dropped_packets = this.socket.add_dropped_packet_to_udp_average();
                            let average_udp_response_time = this.socket.average_udp_response_time();
                            if average_udp_response_time.is_finite() {
                                if average_udp_dropped_packets.current_average() >= INCREASE_UDP_TIMEOUT_DROPPED_AVERAGE_THRESHOLD {
                                    w_active_queries.udp_timeout = bound(
                                        min(
                                            w_active_queries.udp_timeout.saturating_add(UDP_TIMEOUT_STEP_WHEN_DROPPED_THRESHOLD_EXCEEDED),
                                            Duration::from_secs_f64(average_udp_response_time * UDP_TIMEOUT_MAX_DURATION_ABOVE_UDP_RESPONSE_TIME / MILLISECONDS_IN_1_SECOND),
                                        ),
                                        MIN_UDP_TIMEOUT,
                                        MAX_UDP_TIMEOUT,
                                    );
                                }
                                if average_udp_dropped_packets.current_average() >= INCREASE_UDP_RETRANSMISSION_TIMEOUT_DROPPED_AVERAGE_THRESHOLD {
                                    w_active_queries.udp_retransmit_timeout = bound(
                                        min(
                                            w_active_queries.udp_timeout.saturating_add(UDP_RETRANSMISSION_TIMEOUT_STEP_WHEN_DROPPED_THRESHOLD_EXCEEDED),
                                            Duration::from_secs_f64(average_udp_response_time * UDP_RETRANSMISSION_TIMEOUT_MAX_DURATION_ABOVE_UDP_RESPONSE_TIME / MILLISECONDS_IN_1_SECOND),
                                        ),
                                        MIN_UDP_RETRANSMISSION_TIMEOUT,
                                        MAX_UDP_RETRANSMISSION_TIMEOUT,
                                    );
                                }
                            } else {
                                if average_udp_dropped_packets.current_average() >= INCREASE_UDP_TIMEOUT_DROPPED_AVERAGE_THRESHOLD {
                                    w_active_queries.udp_timeout = bound(
                                        w_active_queries.udp_timeout.saturating_add(UDP_TIMEOUT_STEP_WHEN_DROPPED_THRESHOLD_EXCEEDED),
                                        MIN_UDP_TIMEOUT,
                                        MAX_UDP_TIMEOUT,
                                    );
                                }
                                if average_udp_dropped_packets.current_average() >= INCREASE_UDP_RETRANSMISSION_TIMEOUT_DROPPED_AVERAGE_THRESHOLD {
                                    w_active_queries.udp_retransmit_timeout = bound(
                                        w_active_queries.udp_retransmit_timeout.saturating_add(UDP_RETRANSMISSION_TIMEOUT_STEP_WHEN_DROPPED_THRESHOLD_EXCEEDED),
                                        MIN_UDP_RETRANSMISSION_TIMEOUT,
                                        MAX_UDP_RETRANSMISSION_TIMEOUT,
                                    );
                                }
                            }
                        },
                        UdpResponseTime::UdpDroppedTcpResponded(response_time) => {
                            let average_udp_dropped_packets = this.socket.add_dropped_packet_to_udp_average();
                            let (average_tcp_response_time, average_tcp_dropped_packets) = this.socket.add_response_time_to_tcp_average(*response_time);
                            if average_udp_dropped_packets.current_average() >= INCREASE_UDP_TIMEOUT_DROPPED_AVERAGE_THRESHOLD {
                                w_active_queries.udp_timeout = bound(
                                    w_active_queries.udp_timeout.saturating_add(UDP_TIMEOUT_STEP_WHEN_DROPPED_THRESHOLD_EXCEEDED),
                                    MIN_UDP_TIMEOUT,
                                    MAX_UDP_TIMEOUT,
                                );
                            }
                            if average_udp_dropped_packets.current_average() >= INCREASE_UDP_RETRANSMISSION_TIMEOUT_DROPPED_AVERAGE_THRESHOLD {
                                w_active_queries.udp_retransmit_timeout = bound(
                                    w_active_queries.udp_retransmit_timeout.saturating_add(UDP_RETRANSMISSION_TIMEOUT_STEP_WHEN_DROPPED_THRESHOLD_EXCEEDED),
                                    MIN_UDP_RETRANSMISSION_TIMEOUT,
                                    MAX_UDP_RETRANSMISSION_TIMEOUT,
                                );
                            }
                        },
                        UdpResponseTime::Responded { execution_time: response_time, truncated } => {
                            let (average_udp_response_time, average_udp_dropped_packets) = this.socket.add_response_time_to_udp_average(*response_time);
                            if average_udp_dropped_packets.current_average() <= DECREASE_UDP_TIMEOUT_DROPPED_AVERAGE_THRESHOLD {
                                w_active_queries.udp_timeout = bound(
                                    bound(
                                        w_active_queries.udp_timeout.saturating_sub(UDP_TIMEOUT_STEP_WHEN_DROPPED_THRESHOLD_EXCEEDED),
                                        Duration::from_secs_f64(average_udp_response_time.current_average() * UDP_TIMEOUT_DURATION_ABOVE_UDP_RESPONSE_TIME / MILLISECONDS_IN_1_SECOND),
                                        Duration::from_secs_f64(average_udp_response_time.current_average() * UDP_TIMEOUT_MAX_DURATION_ABOVE_UDP_RESPONSE_TIME / MILLISECONDS_IN_1_SECOND),
                                    ),
                                    MIN_UDP_TIMEOUT,
                                    MAX_UDP_TIMEOUT,
                                );
                            }
                            if average_udp_dropped_packets.current_average() <= DECREASE_UDP_RETRANSMISSION_TIMEOUT_DROPPED_AVERAGE_THRESHOLD {
                                w_active_queries.udp_retransmit_timeout = bound(
                                    bound(
                                        w_active_queries.udp_retransmit_timeout.saturating_sub(UDP_RETRANSMISSION_TIMEOUT_STEP_WHEN_DROPPED_THRESHOLD_EXCEEDED),
                                        Duration::from_secs_f64(average_udp_response_time.current_average() * UDP_RETRANSMISSION_TIMEOUT_DURATION_ABOVE_UDP_RESPONSE_TIME / MILLISECONDS_IN_1_SECOND),
                                        Duration::from_secs_f64(average_udp_response_time.current_average() * UDP_RETRANSMISSION_TIMEOUT_MAX_DURATION_ABOVE_UDP_RESPONSE_TIME / MILLISECONDS_IN_1_SECOND),
                                    ),
                                    MIN_UDP_RETRANSMISSION_TIMEOUT,
                                    MAX_UDP_RETRANSMISSION_TIMEOUT,
                                );
                            }
                            this.socket.add_truncated_packet_to_udp_average(*truncated);
                        },
                        UdpResponseTime::None => (),
                    }

                    w_active_queries.in_flight.remove(&this.query.id);
                    w_active_queries.tcp_or_udp.remove(&this.query.question);
                    drop(w_active_queries);

                    this.inner.set_complete();

                    return Poll::Ready(());
                },
                InnerUQProj::Complete => {
                    panic!("UDP query polled after completion");
                },
            }
        }
    }
}

#[pinned_drop]
impl<'a, 'b, 'c, 'f, 'i> PinnedDrop for UdpQueryRunner<'a, 'b, 'c, 'f, 'i> {
    fn drop(mut self: Pin<&mut Self>) {
        match self.as_mut().project().inner.as_mut().project() {
            InnerUQProj::Fresh { udp_retransmissions: _ }
          | InnerUQProj::Running { socket: _, send_query: _ }
          | InnerUQProj::Cleanup(_) => {
                let mut w_active_queries = self.socket.active_queries.write().unwrap();
                let _ = w_active_queries.in_flight.remove(&self.query.id);
                let _ = w_active_queries.tcp_or_udp.remove(&self.query.question);
                drop(w_active_queries);
            },
            InnerUQProj::Complete => {
                // Nothing to do for active queries.
            }
        }
    }
}

#[pin_project]
struct UdpQuery<'a, 'b> {
    socket: &'a Arc<MixedSocket>,
    query: &'b mut Message,
    #[pin]
    inner: QInitQuery,
}

impl<'a, 'b> UdpQuery<'a, 'b> {
    #[inline]
    pub fn new(socket: &'a Arc<MixedSocket>, query: &'b mut Message) -> Self {
        Self {
            socket,
            query,
            inner: QInitQuery::Fresh,
        }
    }
}

impl<'a, 'b> Future for UdpQuery<'a, 'b> {
    type Output = Result<Message, errors::QueryError>;

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        loop {
            let mut this = self.as_mut().project();
            match this.inner.as_mut().project() {
                QInitQueryProj::Fresh => {
                    let r_active_queries = this.socket.active_queries.read().unwrap();
                    match (
                        r_active_queries.tcp_or_udp.get(&this.query.question),
                        r_active_queries.tcp_only.get(&this.query.question)
                    ) {
                        (Some((query_id, result_sender)), _)
                      | (_, Some((query_id, result_sender))) => {
                            this.query.id = *query_id;
                            let result_receiver = result_sender.subscribe();
                            drop(r_active_queries);

                            this.inner.set_following(result_receiver);

                            // TODO
                            continue;
                        },
                        (None, None) => {
                            drop(r_active_queries);

                            this.inner.set_write_active_query();

                            // TODO
                            continue;
                        },
                    }
                },
                QInitQueryProj::WriteActiveQuery => {
                    let mut w_active_queries = this.socket.active_queries.write().unwrap();
                    match (
                        w_active_queries.tcp_or_udp.get(&this.query.question),
                        w_active_queries.tcp_only.get(&this.query.question)
                    ) {
                        (Some((query_id, result_sender)), _)
                      | (_, Some((query_id, result_sender))) => {
                            this.query.id = *query_id;
                            let result_receiver = result_sender.subscribe();
                            drop(w_active_queries);

                            this.inner.set_following(result_receiver);

                            // TODO
                            continue;
                        },
                        (None, None) => {
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

                            let join_handle = tokio::spawn({
                                let udp_retransmit_timeout = w_active_queries.udp_retransmit_timeout;
                                let udp_timeout = w_active_queries.udp_timeout;
                                let result_receiver = result_sender.subscribe();
                                let socket = this.socket.clone();
                                let mut query = this.query.clone();
                                async move {
                                    UdpQueryRunner::new(&socket, &mut query, result_receiver, &udp_retransmit_timeout, &udp_timeout).await;
                                }
                            });

                            w_active_queries.in_flight.insert(this.query.id, (result_sender.clone(), join_handle));
                            w_active_queries.tcp_or_udp.insert(this.query.question.clone(), (this.query.id, result_sender));
                            drop(w_active_queries);

                            this.inner.set_following(result_receiver);

                            // TODO
                            continue;
                        },
                    }

                },
                QInitQueryProj::Following(mut result_receiver) => {
                    match result_receiver.as_mut().poll(cx) {
                        Poll::Ready(Ok(Ok(response))) => {
                            this.inner.set_complete();

                            // TODO
                            return Poll::Ready(Ok(response));
                        },
                        Poll::Ready(Ok(Err(error))) => {
                            this.inner.set_complete();

                            // TODO
                            return Poll::Ready(Err(error));
                        },
                        Poll::Ready(Err(once_watch::RecvError::Closed)) => {
                            let error = errors::QueryError::from(errors::SocketError::Shutdown(
                                errors::SocketType::Udp,
                                errors::SocketStage::Connected,
                            ));

                            this.inner.set_complete();

                            // TODO
                            return Poll::Ready(Err(error));
                        },
                        Poll::Pending => {
                            // TODO
                            return Poll::Pending;
                        },
                    }
                },
                QInitQueryProj::Complete => panic!("UdpQuery cannot be polled after completion"),
            }
        }
    }
}

// Implement UDP functions on MixedSocket
#[async_trait]
impl super::socket::udp::UdpSocket for MixedSocket {
    #[inline]
    fn peer_addr(&self) -> SocketAddr {
        SocketAddr::new(self.upstream_address, DNS_UDP_PORT)
    }

    #[inline]
    fn state(&self) ->  &std::sync::RwLock<UdpState>  {
        &self.udp
    }

    #[inline]
    async fn listen(self: Arc<Self>, udp_reader: Arc<net::UdpSocket>, kill_udp: AwakeToken) {
        pin!(let kill_udp_awoken = kill_udp.awoken(););
        loop {
            select! {
                biased;
                () = &mut kill_udp_awoken => {
                    println!("UDP Socket {} Canceled. Shutting down UDP Listener.", self.upstream_address);
                    break;
                },
                () = tokio::time::sleep(UDP_LISTEN_TIMEOUT) => {
                    println!("UDP Socket {} Timed Out. Shutting down UDP Listener.", self.upstream_address);
                    break;
                },
                response = read_udp_message::<{ MAX_MESSAGE_SIZE as usize }>(&udp_reader) => {
                    match response {
                        Ok(response) => {
                            // Note: if truncation flag is set, that will be dealt with by the caller.
                            self.recent_messages_received.store(true, Ordering::Release);
                            let response_id = response.id;
                            println!("Received UDP Response: {response:?}");
                            let r_active_queries = self.active_queries.read().unwrap();
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

        self.listen_udp_cleanup(kill_udp).await;
    }
}

impl MixedSocket {
    #[inline]
    async fn listen_udp_cleanup(self: Arc<Self>,  kill_udp: AwakeToken) {
        println!("Cleaning up UDP socket {}", self.upstream_address);

        let mut w_state = self.udp.write().unwrap();
        match &*w_state {
            UdpState::Managed(_, managed_kill_udp) => {
                // If the managed socket is the one that we are cleaning up...
                if &kill_udp == managed_kill_udp {
                    // We are responsible for cleanup.
                    *w_state = UdpState::None;
                    drop(w_state);

                    kill_udp.awake();

                // If the managed socket isn't the one that we are cleaning up...
                } else {
                    // This is not our socket to clean up.
                    drop(w_state);
                }
            },
            UdpState::None
          | UdpState::Blocked => {
                drop(w_state);
            },
        }
    }
}

struct ActiveQueries {
    udp_retransmit_timeout: Duration,
    udp_timeout: Duration,
    tcp_timeout: Duration,

    in_flight: HashMap<u16, (once_watch::Sender<Result<Message, errors::QueryError>>, JoinHandle<()>)>,
    tcp_only: HashMap<TinyVec<[Question; 1]>, (u16, once_watch::Sender<Result<Message, errors::QueryError>>)>,
    tcp_or_udp: HashMap<TinyVec<[Question; 1]>, (u16, once_watch::Sender<Result<Message, errors::QueryError>>)>,
}

impl ActiveQueries {
    #[inline]
    pub fn new() -> Self {
        Self {
            udp_retransmit_timeout: INIT_UDP_RETRANSMISSION_TIMEOUT,
            udp_timeout: INIT_UDP_TIMEOUT,
            tcp_timeout: INIT_TCP_TIMEOUT,

            in_flight: HashMap::new(),
            tcp_only: HashMap::new(),
            tcp_or_udp: HashMap::new(),
        }
    }
}

pub struct MixedSocket {
    upstream_address: IpAddr,
    tcp: std::sync::RwLock<TcpState>,
    udp: std::sync::RwLock<UdpState>,
    active_queries: std::sync::RwLock<ActiveQueries>,

    // Rolling averages
    average_tcp_response_time: Atomic<RollingAverage>,
    average_tcp_dropped_packets: Atomic<RollingAverage>,
    average_udp_response_time: Atomic<RollingAverage>,
    average_udp_dropped_packets: Atomic<RollingAverage>,
    average_udp_truncated_packets: Atomic<RollingAverage>,

    // Counters used to determine when the socket should be closed.
    recent_messages_sent: AtomicBool,
    recent_messages_received: AtomicBool,
}

impl MixedSocket {
    #[inline]
    pub fn new(upstream_address: IpAddr) -> Arc<Self> {
        Arc::new(MixedSocket {
            upstream_address,
            tcp: std::sync::RwLock::new(TcpState::None),
            udp: std::sync::RwLock::new(UdpState::None),
            active_queries: std::sync::RwLock::new(ActiveQueries::new()),

            average_tcp_response_time: Atomic::new(RollingAverage::new()),
            average_tcp_dropped_packets: Atomic::new(RollingAverage::new()),
            average_udp_response_time: Atomic::new(RollingAverage::new()),
            average_udp_dropped_packets: Atomic::new(RollingAverage::new()),
            average_udp_truncated_packets: Atomic::new(RollingAverage::new()),

            recent_messages_sent: AtomicBool::new(false),
            recent_messages_received: AtomicBool::new(false),
        })
    }

    #[inline]
    pub fn peer_addr(&self) -> IpAddr {
        self.upstream_address
    }

    #[inline]
    pub fn average_tcp_response_time(&self) -> f64 {
        self.average_tcp_response_time.load(Ordering::Acquire).current_average()
    }

    #[inline]
    pub fn average_dropped_tcp_packets(&self) -> f64 {
        self.average_tcp_dropped_packets.load(Ordering::Acquire).current_average()
    }

    #[inline]
    pub fn average_udp_response_time(&self) -> f64 {
        self.average_udp_response_time.load(Ordering::Acquire).current_average()
    }

    #[inline]
    pub fn average_dropped_udp_packets(&self) -> f64 {
        self.average_udp_dropped_packets.load(Ordering::Acquire).current_average()
    }

    #[inline]
    pub fn average_truncated_udp_packets(&self) -> f64 {
        self.average_udp_truncated_packets.load(Ordering::Acquire).current_average()
    }

    #[inline]
    fn add_dropped_packet_to_tcp_average(&self) -> RollingAverage {
        // We can use relaxed memory orderings with the rolling average because it is not being used
        // for synchronization nor do we care about the order of atomic operations. We only care
        // that the operation is atomic.
        fetch_update(
            &self.average_tcp_dropped_packets,
            Ordering::Relaxed,
            Ordering::Relaxed,
            |average| average.put_next(1, ROLLING_AVERAGE_TCP_MAX_DROPPED)
        )
    }

    #[inline]
    fn add_response_time_to_tcp_average(&self, response_time: Duration) -> (RollingAverage, RollingAverage) {
        // We can use relaxed memory orderings with the rolling average because it is not being used
        // for synchronization nor do we care about the order of atomic operations. We only care
        // that the operation is atomic.
        (
            fetch_update(
                &self.average_tcp_response_time,
                Ordering::Relaxed,
                Ordering::Relaxed,
                |average| average.put_next(u32::try_from(response_time.as_millis()).unwrap_or(u32::MAX), ROLLING_AVERAGE_TCP_MAX_RESPONSE_TIMES)
            ),
            fetch_update(
                &self.average_tcp_dropped_packets,
                Ordering::Relaxed,
                Ordering::Relaxed,
                |average| average.put_next(0, ROLLING_AVERAGE_TCP_MAX_DROPPED)
            )
        )
    }

    #[inline]
    fn add_dropped_packet_to_udp_average(&self) -> RollingAverage {
        // We can use relaxed memory orderings with the rolling average because it is not being used
        // for synchronization nor do we care about the order of atomic operations. We only care
        // that the operation is atomic.
        fetch_update(
            &self.average_udp_dropped_packets,
            Ordering::Relaxed,
            Ordering::Relaxed,
            |average| average.put_next(1, ROLLING_AVERAGE_UDP_MAX_DROPPED)
        )
    }

    #[inline]
    fn add_response_time_to_udp_average(&self, response_time: Duration) -> (RollingAverage, RollingAverage) {
        // We can use relaxed memory orderings with the rolling average because it is not being used
        // for synchronization nor do we care about the order of atomic operations. We only care
        // that the operation is atomic.
        (
            fetch_update(
                &self.average_udp_response_time,
                Ordering::Relaxed,
                Ordering::Relaxed,
                |average| average.put_next(u32::try_from(response_time.as_millis()).unwrap_or(u32::MAX), ROLLING_AVERAGE_UDP_MAX_RESPONSE_TIMES)
            ),
            fetch_update(
                &self.average_udp_dropped_packets,
                Ordering::Relaxed,
                Ordering::Relaxed,
                |average| average.put_next(0, ROLLING_AVERAGE_UDP_MAX_DROPPED)
            )
        )
    }

    #[inline]
    fn add_truncated_packet_to_udp_average(&self, truncated: bool) -> RollingAverage {
        // We can use relaxed memory orderings with the rolling average because it is not being used
        // for synchronization nor do we care about the order of atomic operations. We only care
        // that the operation is atomic.
        fetch_update(
            &self.average_udp_truncated_packets,
            Ordering::Relaxed,
            Ordering::Relaxed,
            |average| average.put_next(truncated.into(), ROLLING_AVERAGE_UDP_MAX_TRUNCATED)
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
        match join!(
            <Self as UdpSocket>::start(self.clone()),
            <Self as TcpSocket>::start(self),
        ) {
            (Ok(()), Ok(())) => Ok(()),
            (Ok(()), Err(tcp_error)) => Err(errors::SocketError::from(tcp_error)),
            (Err(udp_error), Ok(())) => Err(errors::SocketError::from(udp_error)),
            (Err(udp_error), Err(tcp_error)) => Err(errors::SocketError::Multiple(vec![udp_error, tcp_error])),
        }
    }

    #[inline]
    pub async fn shutdown(self: Arc<Self>) {
        join!(
            <Self as UdpSocket>::shutdown(self.clone()),
            <Self as TcpSocket>::shutdown(self),
        );
    }

    #[inline]
    pub async fn enable(self: Arc<Self>) {
        join!(
            <Self as UdpSocket>::enable(self.clone()),
            <Self as TcpSocket>::enable(self),
        );
    }

    #[inline]
    pub async fn disable(self: Arc<Self>) {
        join!(
            <Self as UdpSocket>::disable(self.clone()),
            <Self as TcpSocket>::disable(self),
        );
    }

    pub fn query<'a, 'b>(self: &'a Arc<Self>, query: &'b mut Message, options: QueryOpt) -> MixedQuery<'a, 'b> {
        // If the UDP socket is unreliable, send most data via TCP. Some queries should still use
        // UDP to determine if the network conditions are improving. However, if the TCP connection
        // is also unstable, then we should not rely on it.
        let query_task = match options {
            QueryOpt::UdpTcp => {
                let average_dropped_udp_packets = self.average_dropped_udp_packets();
                let average_truncated_udp_packets = self.average_truncated_udp_packets();
                let average_dropped_tcp_packets = self.average_dropped_tcp_packets();
                if ((average_dropped_udp_packets.is_finite() && (average_dropped_udp_packets >= 0.40))
                 || (average_truncated_udp_packets.is_finite() && (average_truncated_udp_packets >= 0.50)))
                && (average_dropped_tcp_packets.is_nan() || (average_dropped_tcp_packets <= 0.25))
                && (rand::random::<f32>() >= 0.20)
                {
                    MixedQuery::Tcp(TcpQuery::new(&self, query))
                } else {
                    MixedQuery::Udp(UdpQuery::new(&self, query))
                }
            },
            QueryOpt::Tcp => {
                MixedQuery::Tcp(TcpQuery::new(&self, query))
            },
            QueryOpt::Quic => todo!(),
            QueryOpt::Tls => todo!(),
            QueryOpt::QuicTls => todo!(),
            QueryOpt::Https => todo!(),
        };

        return query_task;
    }
}

#[cfg(test)]
mod mixed_udp_tcp_tests {
    use std::{net::{IpAddr, Ipv4Addr, SocketAddr}, time::Duration};

    use dns_lib::{query::{message::Message, qr::QR, question::Question}, resource_record::{opcode::OpCode, rclass::RClass, rcode::RCode, resource_record::ResourceRecord, rtype::RType, time::Time, types::a::A}, serde::wire::{from_wire::FromWire, read_wire::ReadWire, to_wire::ToWire}, types::c_domain_name::CDomainName};
    use tinyvec::TinyVec;
    use tokio::{io::AsyncReadExt, select};
    use ux::u3;

    use crate::mixed_tcp_udp::{MixedSocket, QueryOpt};

    const LISTEN_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 53);
    const SEND_ADDR: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);

    #[tokio::test(flavor = "multi_thread")]
    async fn udp_manager_no_responses() {
        // Setup
        let listen_udp_socket = tokio::net::UdpSocket::bind(LISTEN_ADDR).await.unwrap();
        let listen_tcp_socket = tokio::net::TcpListener::bind(LISTEN_ADDR).await.unwrap();

        let example_domain = CDomainName::from_utf8("example.org.").unwrap();
        let example_class = RClass::Internet;

        let question = Question::new(
            example_domain.clone(),
            RType::A,
            RClass::Internet
        );
        let answer = ResourceRecord::new(
            example_domain,
            example_class,
            Time::from_secs(3600),
            A::new(Ipv4Addr::LOCALHOST),
        );
        let query = Message {
            id: 42,
            qr: QR::Query,
            opcode: OpCode::Query,
            authoritative_answer: false,
            truncation: false,
            recursion_desired: false,
            recursion_available: false,
            z: u3::new(0),
            rcode: RCode::NoError,
            question: TinyVec::from([question.clone()]),
            answer: vec![],
            authority: vec![],
            additional: vec![],
        };
        let response = Message {
            id: 42,
            qr: QR::Response,
            opcode: OpCode::Query,
            authoritative_answer: false,
            truncation: false,
            recursion_desired: false,
            recursion_available: false,
            z: u3::new(0),
            rcode: RCode::NoError,
            question: TinyVec::from([question.clone()]),
            answer: vec![answer.into()],
            authority: vec![],
            additional: vec![],
        };

        let mixed_socket = MixedSocket::new(SEND_ADDR);

        // Test: Start Query
        let query_task = tokio::spawn({
            let mixed_socket = mixed_socket.clone();
            let mut query = query.clone();
            async move { mixed_socket.query(&mut query, QueryOpt::UdpTcp).await }
        });

        // Test: Receiver first query (no response + no TCP)
        let mut buffer = [0_u8; 512];
        let bytes_read = select! {
            bytes_read = listen_udp_socket.recv(&mut buffer) => bytes_read,
            () = tokio::time::sleep(Duration::from_secs(1)) => {
                panic!("Did not receive first message in time.")
            },
        };
        assert!(bytes_read.is_ok());
        let bytes_read = bytes_read.unwrap();
        assert!(bytes_read <= query.serial_length() as usize);

        let mut wire = ReadWire::from_bytes(&buffer[..(bytes_read as usize)]);
        let actual_query = Message::from_wire_format(&mut wire);
        assert!(actual_query.is_ok());
        let actual_query = actual_query.unwrap();
        let mut expected_query = query.clone();
        expected_query.id = actual_query.id;
        assert_eq!(actual_query, expected_query);

        tokio::time::sleep(Duration::from_millis(125)).await;

        // Test: Receiver second query (no response)
        let mut buffer = [0_u8; 512];
        let bytes_read = select! {
            bytes_read = listen_udp_socket.recv(&mut buffer) => bytes_read,
            () = tokio::time::sleep(Duration::from_secs(1)) => {
                panic!("Did not receive second message in time.")
            },
        };
        assert!(bytes_read.is_ok());
        let bytes_read = bytes_read.unwrap();
        assert!(bytes_read <= query.serial_length() as usize);

        let mut wire = ReadWire::from_bytes(&buffer[..(bytes_read as usize)]);
        let actual_query = Message::from_wire_format(&mut wire);
        assert!(actual_query.is_ok());
        let actual_query = actual_query.unwrap();
        assert_eq!(actual_query, expected_query);   //< no ID change allowed for same query

        // Test: TCP Connection is requested
        let tcp_receiver = select! {
            tcp_receiver = listen_tcp_socket.accept() => tcp_receiver,
            () = tokio::time::sleep(Duration::from_secs(1)) => {
                panic!("Did not receive TCP connection request in time.")
            },
        };
        assert!(tcp_receiver.is_ok());
        let mut tcp_receiver = tcp_receiver.unwrap().0;

        // Test: TCP request is not yet made
        let mut buffer = [0_u8; 512];
        let bytes_read = tcp_receiver.try_read(&mut buffer);
        assert!(bytes_read.is_err());

        tokio::time::sleep(Duration::from_millis(125)).await;

        // Test: TCP request
        let mut buffer = [0_u8; 2];
        let bytes_read = select! {
            bytes_read = tcp_receiver.read_exact(&mut buffer) => bytes_read,
            () = tokio::time::sleep(Duration::from_secs(1)) => {
                panic!("Did not receive third message in time (size bytes).")
            },
        };
        assert!(bytes_read.is_ok());
        let bytes_read = bytes_read.unwrap();
        assert_eq!(bytes_read, 2);
        let expected_bytes = u16::from_be_bytes(buffer);
        assert!(expected_bytes <= query.serial_length());

        let mut buffer = [0_u8; 512];
        let bytes_read = select! {
            bytes_read = tcp_receiver.read_exact(&mut buffer[..(expected_bytes as usize)]) => bytes_read,
            () = tokio::time::sleep(Duration::from_secs(1)) => {
                panic!("Did not receive third message in time (message bytes).")
            },
        };
        assert!(bytes_read.is_ok());
        let bytes_read = bytes_read.unwrap();
        assert_eq!(bytes_read, expected_bytes as usize);

        let mut wire = ReadWire::from_bytes(&buffer[..(expected_bytes as usize)]);
        let actual_query = Message::from_wire_format(&mut wire);
        assert!(actual_query.is_ok());
        let actual_query = actual_query.unwrap();
        assert_eq!(actual_query, expected_query);   //< no ID change allowed for same query

        // Test: Client connection failed
        let query_task_response = query_task.await;
        assert!(query_task_response.is_err());   //< io error

        // Cleanup
        mixed_socket.disable().await;
    }
}

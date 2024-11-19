use std::{cmp::{max, min}, collections::HashMap, fmt::Display, future::Future, net::SocketAddr, num::NonZeroU8, ops::{Add, Div}, pin::Pin, sync::{atomic::{AtomicBool, Ordering}, Arc}, task::Poll, time::Duration};

use async_lib::{awake_token::{AwakeToken, AwokenToken, SameAwakeToken}, once_watch::{self, OnceWatchSend, OnceWatchSubscribe}};
use atomic::Atomic;
use bytemuck::{NoUninit, Pod, Zeroable};
use dns_lib::{query::{message::Message, question::Question}, serde::wire::{from_wire::FromWire, read_wire::ReadWire, to_wire::ToWire, write_wire::WriteWire}, types::c_domain_name::CompressionMap};
use futures::{future::BoxFuture, FutureExt};
use log::trace;
use pin_project::{pin_project, pinned_drop};
use tinyvec::TinyVec;
use tokio::{io::{self, AsyncReadExt, AsyncWriteExt}, join, net::{tcp::{OwnedReadHalf, OwnedWriteHalf}, TcpStream, UdpSocket}, pin, select, sync::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard}, task::{self, JoinHandle}, time::{Instant, Sleep}};

const MAX_MESSAGE_SIZE: u16 = 8192;

const MILLISECONDS_IN_1_SECOND: f64 = 1000.0;

const TCP_INIT_TIMEOUT: Duration = Duration::from_secs(5);
const TCP_LISTEN_TIMEOUT: Duration = Duration::from_secs(120);
const UDP_LISTEN_TIMEOUT: Duration = Duration::from_secs(120);

/// The initial TCP timeout, used when setting up a socket, before anything is known about the
/// average response time.
const INIT_TCP_TIMEOUT: Duration = Duration::from_secs(1);
/// The percentage of the average TCP response time that the timeout should be set to. Currently,
/// this represents 200%. If the average response time were 20 ms, then the retransmission timeout
/// would be 40 ms.
const TCP_TIMEOUT_DURATION_ABOVE_TCP_RESPONSE_TIME: f64 = 2.00;
/// The maximum percentage of the average TCP response time that the timeout should be set to.
/// Currently, this represents 400%. If the average response time were 20 ms, then the
/// retransmission timeout would be 80 ms.
const TCP_TIMEOUT_MAX_DURATION_ABOVE_TCP_RESPONSE_TIME: f64 = 4.00;
/// The step size to use if INCREASE_TCP_TIMEOUT_DROPPED_AVERAGE_THRESHOLD is exceeded.
const TCP_TIMEOUT_STEP_WHEN_DROPPED_THRESHOLD_EXCEEDED: Duration = Duration::from_millis(50);
/// When 20% or more of packets are being dropped (for TCP, this just means that the queries are
/// timing out), then it is time to start slowing down the socket.
const INCREASE_TCP_TIMEOUT_DROPPED_AVERAGE_THRESHOLD: f64 = 0.20;
/// When 1% or more of packets are being dropped (for TCP, this just means that the queries are
/// timing out), then we might want to try speeding up the socket again, to reflect the average
/// response time.
const DECREASE_TCP_TIMEOUT_DROPPED_AVERAGE_THRESHOLD: f64 = 0.01;
/// The maximum allowable TCP timeout.
const MAX_TCP_TIMEOUT: Duration = Duration::from_secs(10);
/// The minimum allowable TCP timeout.
const MIN_TCP_TIMEOUT: Duration = Duration::from_millis(50);

/// The initial UDP retransmission timeout, used when setting up a socket, before anything is known
/// about the average response time.
const INIT_UDP_RETRANSMISSION_TIMEOUT: Duration = Duration::from_millis(500);
/// The percentage of the average UDP response time that the timeout should be set to. Currently,
/// this represents 150%. If the average response time were 20 ms, then the retransmission timeout
/// would be 30 ms.
const UDP_RETRANSMISSION_TIMEOUT_DURATION_ABOVE_UDP_RESPONSE_TIME: f64 = 1.50;
/// The maximum percentage of the average UDP response time that the timeout should be set to.
/// Currently, this represents 250%. If the average response time were 20 ms, then the
/// retransmission timeout would be 60 ms.
const UDP_RETRANSMISSION_TIMEOUT_MAX_DURATION_ABOVE_UDP_RESPONSE_TIME: f64 = 3.00;
/// The step size to use if INCREASE_UDP_RETRANSMISSION_TIMEOUT_DROPPED_AVERAGE_THRESHOLD is
/// exceeded.
const UDP_RETRANSMISSION_TIMEOUT_STEP_WHEN_DROPPED_THRESHOLD_EXCEEDED: Duration = Duration::from_millis(50);
/// When 20% or more of packets are being dropped, then it is time to start slowing down the socket.
const INCREASE_UDP_RETRANSMISSION_TIMEOUT_DROPPED_AVERAGE_THRESHOLD: f64 = 0.20;
/// When 1% or more of packets are being dropped, then we might want to try speeding up the socket
/// again, to reflect the average response time.
const DECREASE_UDP_RETRANSMISSION_TIMEOUT_DROPPED_AVERAGE_THRESHOLD: f64 = 0.01;
/// The maximum allowable UDP retransmission timeout.
const MAX_UDP_RETRANSMISSION_TIMEOUT: Duration = Duration::from_secs(10);
/// The minimum allowable UDP retransmission timeout.
const MIN_UDP_RETRANSMISSION_TIMEOUT: Duration = Duration::from_millis(50);

/// The initial UDP timeout, used when setting up a socket, before anything is known about the
/// average response time.
const INIT_UDP_TIMEOUT: Duration = Duration::from_millis(500);
/// The number of UDP retransmission that are allowed for a mixed UDP-TCP query.
const UDP_RETRANSMISSIONS: u8 = 1;
/// The percentage of the average UDP response time that the timeout should be set to. Currently,
/// this represents 200%. If the average response time were 20 ms, then the timeout would be 40 ms.
const UDP_TIMEOUT_DURATION_ABOVE_UDP_RESPONSE_TIME: f64 = 2.00;
/// The maximum percentage of the average UDP response time that the timeout should be set to.
/// Currently, this represents 400%. If the average response time were 20 ms, then the
/// retransmission timeout would be 80 ms.
const UDP_TIMEOUT_MAX_DURATION_ABOVE_UDP_RESPONSE_TIME: f64 = 4.00;
/// The step size to use if INCREASE_UDP_TIMEOUT_DROPPED_AVERAGE_THRESHOLD is
/// exceeded.
const UDP_TIMEOUT_STEP_WHEN_DROPPED_THRESHOLD_EXCEEDED: Duration = Duration::from_millis(50);
/// When 20% or more of packets are being dropped, then it is time to start slowing down the socket.
const INCREASE_UDP_TIMEOUT_DROPPED_AVERAGE_THRESHOLD: f64 = 0.20;
/// When 1% or more of packets are being dropped, then we might want to try speeding up the socket
/// again, to reflect the average response time.
const DECREASE_UDP_TIMEOUT_DROPPED_AVERAGE_THRESHOLD: f64 = 0.01;
/// The maximum allowable UDP timeout.
const MAX_UDP_TIMEOUT: Duration = Duration::from_secs(10);
/// The minimum allowable UDP timeout.
const MIN_UDP_TIMEOUT: Duration = Duration::from_millis(50);

// Using the safe checked version of new is not stable. As long as we always use non-zero constants,
// there should not be any problems with this.
const ROLLING_AVERAGE_TCP_MAX_DROPPED: NonZeroU8        = unsafe { NonZeroU8::new_unchecked(11) };
const ROLLING_AVERAGE_TCP_MAX_RESPONSE_TIMES: NonZeroU8 = unsafe { NonZeroU8::new_unchecked(13) };
const ROLLING_AVERAGE_UDP_MAX_DROPPED: NonZeroU8        = unsafe { NonZeroU8::new_unchecked(11) };
const ROLLING_AVERAGE_UDP_MAX_RESPONSE_TIMES: NonZeroU8 = unsafe { NonZeroU8::new_unchecked(13) };
const ROLLING_AVERAGE_UDP_MAX_TRUNCATED: NonZeroU8      = unsafe { NonZeroU8::new_unchecked(50) };

fn bound<T>(value: T, lower_bound: T, upper_bound: T) -> T where T: Ord {
    debug_assert!(lower_bound <= upper_bound);
    value.clamp(lower_bound, upper_bound)
}

pub mod errors {
    use std::{error::Error, fmt::Display, io};

    use dns_lib::serde::wire::{read_wire::ReadWireError, write_wire::WriteWireError};
    use tokio::task::JoinError;

    #[derive(Debug, Clone)]
    pub enum QueryError {
        TcpSocket(TcpSocketError),
        TcpSend(TcpSendError),
        UdpSocket(UdpSocketError),
        UdpSend(UdpSendError),
        Timeout,
    }
    impl Display for QueryError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match &self {
                Self::TcpSocket(tcp_error) => write!(f, "{tcp_error}"),
                Self::TcpSend(tcp_error) => write!(f, "{tcp_error}"),
                Self::UdpSocket(udp_error) => write!(f, "{udp_error}"),
                Self::UdpSend(udp_error) => write!(f, "{udp_error}"),
                Self::Timeout => write!(f, "timeout during query"),
            }
        }
    }
    impl Error for QueryError {}
    impl From<TcpSocketError> for QueryError {
        fn from(error: TcpSocketError) -> Self {
            Self::TcpSocket(error)
        }
    }
    impl From<TcpSendError> for QueryError {
        fn from(error: TcpSendError) -> Self {
            Self::TcpSend(error)
        }
    }
    impl From<UdpSocketError> for QueryError {
        fn from(error: UdpSocketError) -> Self {
            Self::UdpSocket(error)
        }
    }
    impl From<UdpSendError> for QueryError {
        fn from(error: UdpSendError) -> Self {
            Self::UdpSend(error)
        }
    }
    impl From<SocketSendError> for QueryError {
        fn from(error: SocketSendError) -> Self {
            match error {
                SocketSendError::Tcp(tcp_send_error) => Self::from(tcp_send_error),
                SocketSendError::Udp(udp_send_error) => Self::from(udp_send_error),
            }
        }
    }
    impl From<SocketError> for QueryError {
        fn from(error: SocketError) -> Self {
            match error {
                SocketError::Tcp(tcp_error) => Self::from(tcp_error),
                SocketError::Udp(udp_error) => Self::from(udp_error),
            }
        }
    }
    
    #[derive(Debug, Clone)]
    pub enum SocketSendError {
        Tcp(TcpSendError),
        Udp(UdpSendError),
    }
    impl Display for SocketSendError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match &self {
                Self::Tcp(send_error) => write!(f, "{send_error}"),
                Self::Udp(send_error) => write!(f, "{send_error}"),
            }
        }
    }
    impl Error for SocketSendError {}
    impl From<TcpSendError> for SocketSendError {
        fn from(error: TcpSendError) -> Self {
            Self::Tcp(error)
        }
    }
    impl From<UdpSendError> for SocketSendError {
        fn from(error: UdpSendError) -> Self {
            Self::Udp(error)
        }
    }
    
    #[derive(Debug, Clone)]
    pub enum TcpSendError {
        Serialization(WriteWireError),
        IncorrectNumberBytes {
            expected: u16,
            sent: usize,
        },
        Io(IoError),
    }
    impl Display for TcpSendError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match &self {
                Self::Serialization(write_wire_error) => write!(f, "{write_wire_error} before sending on TCP socket"),
                Self::IncorrectNumberBytes { expected, sent } => write!(f, "expected to send {expected} bytes but sent {sent} on TCP socket"),
                Self::Io(error) => write!(f, "{error} when sending on TCP socket"),
            }
        }
    }
    impl Error for TcpSendError {}
    impl From<WriteWireError> for TcpSendError {
        fn from(error: WriteWireError) -> Self {
            Self::Serialization(error)
        }
    }
    impl From<IoError> for TcpSendError {
        fn from(error: IoError) -> Self {
            Self::Io(error)
        }
    }
    impl From<io::Error> for TcpSendError {
        fn from(error: io::Error) -> Self {
            Self::Io(IoError::from(error))
        }
    }
    
    #[derive(Debug, Clone)]
    pub enum UdpSendError {
        Serialization(WriteWireError),
        IncorrectNumberBytes {
            expected: u16,
            sent: usize,
        },
        Io(IoError),
    }
    impl Display for UdpSendError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match &self {
                Self::Serialization(write_wire_error) => write!(f, "{write_wire_error} before sending on UDP socket"),
                Self::IncorrectNumberBytes { expected, sent } => write!(f, "expected to send {expected} bytes but sent {sent} on UDP socket"),
                Self::Io(error) => write!(f, "{error} when sending on UDP socket"),
            }
        }
    }
    impl Error for UdpSendError {}
    impl From<WriteWireError> for UdpSendError {
        fn from(error: WriteWireError) -> Self {
            Self::Serialization(error)
        }
    }
    impl From<IoError> for UdpSendError {
        fn from(error: IoError) -> Self {
            Self::Io(error)
        }
    }
    impl From<io::Error> for UdpSendError {
        fn from(error: io::Error) -> Self {
            Self::Io(IoError::from(error))
        }
    }
    
    #[derive(Debug, Clone)]
    pub enum TcpReceiveError {
        IncorrectNumberBytes {
            expected: u16,
            received: usize,
        },
        IncorrectLengthByte {
            limit: u16,
            received: u16,
        },
        Deserialization(ReadWireError),
        Io(IoError),
    }
    impl Display for TcpReceiveError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match &self {
                Self::IncorrectNumberBytes { expected, received } => write!(f, "expected to receive {expected} bytes but received {received} on TCP socket"),
                Self::IncorrectLengthByte { limit, received } => write!(f, "expected to receive at most {limit} bytes but the length byte is {received} on TCP socket"),
                Self::Deserialization(error) => write!(f, "{error} when receiving on TCP socket"),
                Self::Io(error) => write!(f, "{error} when receiving on TCP socket"),
            }
        }
    }
    impl Error for TcpReceiveError {}
    impl From<ReadWireError> for TcpReceiveError {
        fn from(error: ReadWireError) -> Self {
            Self::Deserialization(error)
        }
    }
    impl From<IoError> for TcpReceiveError {
        fn from(error: IoError) -> Self {
            Self::Io(error)
        }
    }
    impl From<io::Error> for TcpReceiveError {
        fn from(error: io::Error) -> Self {
            Self::Io(IoError::from(error))
        }
    }
    
    #[derive(Debug, Clone)]
    pub enum UdpReceiveError {
        IncorrectNumberBytes {
            expected: u16,
            received: usize,
        },
        Deserialization(ReadWireError),
        Io(IoError),
    }
    impl Display for UdpReceiveError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match &self {
                Self::IncorrectNumberBytes { expected, received } => write!(f, "expected to receive {expected} bytes but received {received} on TCP socket"),
                Self::Deserialization(error) => write!(f, "{error} when receiving on UDP socket"),
                Self::Io(error) => write!(f, "{error} when receiving on UDP socket"),
            }
        }
    }
    impl Error for UdpReceiveError {}
    impl From<ReadWireError> for UdpReceiveError {
        fn from(error: ReadWireError) -> Self {
            Self::Deserialization(error)
        }
    }
    impl From<IoError> for UdpReceiveError {
        fn from(error: IoError) -> Self {
            Self::Io(error)
        }
    }
    impl From<io::Error> for UdpReceiveError {
        fn from(error: io::Error) -> Self {
            Self::Io(IoError::from(error))
        }
    }
    
    #[derive(Debug, Clone)]
    pub enum SocketError {
        Udp(UdpSocketError),
        Tcp(TcpSocketError),
    }
    impl Display for SocketError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match &self {
                Self::Udp(udp_error) => write!(f, "{udp_error}"),
                Self::Tcp(tcp_error) => write!(f, "{tcp_error}"),
            }
        }
    }
    impl Error for SocketError {}
    impl From<UdpSocketError> for SocketError {
        fn from(error: UdpSocketError) -> Self {
            Self::Udp(error)
        }
    }
    impl From<TcpSocketError> for SocketError {
        fn from(error: TcpSocketError) -> Self {
            Self::Tcp(error)
        }
    }
    
    #[derive(Debug, Clone)]
    pub enum TcpSocketError {
        Disabled,
        Shutdown,
        Init(TcpInitError),
    }
    impl Display for TcpSocketError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match &self {
                Self::Disabled => write!(f, "TCP socket is disabled"),
                Self::Shutdown => write!(f, "TCP socket was shutdown"),
                Self::Init(init_error) => write!(f, "{init_error}"),
            }
        }
    }
    impl Error for TcpSocketError {}
    impl From<TcpInitError> for TcpSocketError {
        fn from(error: TcpInitError) -> Self {
            Self::Init(error)
        }
    }
    
    #[derive(Debug, Clone)]
    pub enum UdpSocketError {
        Disabled,
        Shutdown,
        Init(UdpInitError),
    }
    impl Display for UdpSocketError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match &self {
                Self::Disabled => write!(f, "UDP socket is disabled"),
                Self::Shutdown => write!(f, "UDP socket was shutdown"),
                Self::Init(init_error) => write!(f, "{init_error}"),
            }
        }
    }
    impl Error for UdpSocketError {}
    impl From<UdpInitError> for UdpSocketError {
        fn from(error: UdpInitError) -> Self {
            Self::Init(error)
        }
    }

    #[derive(Debug, Clone)]
    pub enum SocketInitError {
        Udp(UdpInitError),
        Tcp(TcpInitError),
        Both(UdpInitError, TcpInitError),
    }
    impl Display for SocketInitError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match &self {
                Self::Udp(udp_init_error) => write!(f, "{udp_init_error}"),
                Self::Tcp(tcp_init_error) => write!(f, "{tcp_init_error}"),
                Self::Both(udp_init_error, tcp_init_error) => write!(f, "{udp_init_error} and {tcp_init_error}"),
            }
        }
    }
    impl Error for SocketInitError {}
    impl From<UdpInitError> for SocketInitError {
        fn from(error: UdpInitError) -> Self {
            Self::Udp(error)
        }
    }
    impl From<TcpInitError> for SocketInitError {
        fn from(error: TcpInitError) -> Self {
            Self::Tcp(error)
        }
    }
    impl From<(UdpInitError, TcpInitError)> for SocketInitError {
        fn from(error: (UdpInitError, TcpInitError)) -> Self {
            Self::Both(error.0, error.1)
        }
    }
    
    #[derive(Debug, Clone)]
    pub enum TcpInitError {
        SocketDisabled,
        SocketShutdown,
        Timeout,
        JoinErrorPanic,
        JoinErrorCancelled,
        Io(IoError),
    }
    impl Display for TcpInitError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match &self {
                Self::SocketDisabled => write!(f, "socket disabled during TCP initialization"),
                Self::SocketShutdown => write!(f, "socket shutdown during TCP initialization"),
                Self::Timeout => write!(f, "timeout during TCP initialization"),
                Self::JoinErrorPanic => write!(f, "panic in TCP initialization task"),
                Self::JoinErrorCancelled => write!(f, "TCP initialization task cancelled"),
                Self::Io(io_error) => write!(f, "{io_error} during TCP initialization"),
            }
        }
    }
    impl Error for TcpInitError {}
    impl From<JoinError> for TcpInitError {
        fn from(error: JoinError) -> Self {
            if error.is_cancelled() {
                Self::JoinErrorCancelled
            } else {
                Self::JoinErrorPanic
            }
        }
    }
    impl From<IoError> for TcpInitError {
        fn from(error: IoError) -> Self {
            Self::Io(error)
        }
    }
    impl From<io::Error> for TcpInitError {
        fn from(error: io::Error) -> Self {
            Self::Io(IoError::from(error))
        }
    }
    
    #[derive(Debug, Clone)]
    pub enum UdpInitError {
        SocketDisabled,
        SocketShutdown,
        Timeout,
        Io(IoError),
    }
    impl Display for UdpInitError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match &self {
                Self::SocketDisabled => write!(f, "socket disabled during UDP initialization"),
                Self::SocketShutdown => write!(f, "socket shutdown during UDP initialization"),
                Self::Timeout => write!(f, "timeout during UDP initialization"),
                Self::Io(io_error) => write!(f, "{io_error} during UDP initialization"),
            }
        }
    }
    impl Error for UdpInitError {}
    impl From<IoError> for UdpInitError {
        fn from(error: IoError) -> Self {
            Self::Io(error)
        }
    }
    impl From<io::Error> for UdpInitError {
        fn from(error: io::Error) -> Self {
            Self::Io(IoError::from(error))
        }
    }
    
    #[derive(Debug, Clone)]
    pub enum IoError {
        OsError(io::ErrorKind),
        Message(io::ErrorKind),
    }
    impl Display for IoError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match &self {
                Self::OsError(error_kind) => write!(f, "OS error: {error_kind}"),
                Self::Message(error_kind) => write!(f, "IO error: {error_kind}"),
            }
        }
    }
    impl Error for IoError {}
    impl From<io::Error> for IoError {
        fn from(error: io::Error) -> Self {
            if let Some(_) = error.raw_os_error() {
                return Self::OsError(error.kind());
            } else {
                return Self::Message(error.kind());
            }
        }
    }    
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum QueryOptions {
    TcpOnly,
    Both,
}

#[pin_project(project = QInitQueryProj)]
enum QInitQuery<'w, 'x> where 'x: 'w {
    Fresh,
    ReadActiveQuery(BoxFuture<'w, RwLockReadGuard<'x, ActiveQueries>>),
    WriteActiveQuery(BoxFuture<'w, RwLockWriteGuard<'x, ActiveQueries>>),
    Following(#[pin] once_watch::Receiver<Result<Message, errors::QueryError>>),
    Complete,
}

impl<'a, 'w, 'x> QInitQuery<'w, 'x> where 'a: 'x {
    fn set_read_active_query(mut self: std::pin::Pin<&mut Self>, socket: &'a Arc<MixedSocket>) {
        let r_active_queries = socket.active_queries.read().boxed();

        self.set(QInitQuery::ReadActiveQuery(r_active_queries));
    }

    fn set_write_active_query(mut self: std::pin::Pin<&mut Self>, socket: &'a Arc<MixedSocket>) {
        let w_active_queries = socket.active_queries.write().boxed();

        self.set(QInitQuery::WriteActiveQuery(w_active_queries));
    }

    fn set_following(mut self: std::pin::Pin<&mut Self>, receiver: once_watch::Receiver<Result<Message, errors::QueryError>>) {
        self.set(QInitQuery::Following(receiver));
    }

    fn set_complete(mut self: std::pin::Pin<&mut Self>) {
        self.set(QInitQuery::Complete);
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
enum Query {
    Initial,
    Retransmit,
}

impl Display for Query {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Initial => write!(f, "Initial"),
            Self::Retransmit => write!(f, "Retransmit"),
        }
    }
}

#[pin_project(project = QSendQueryProj)]
enum QSendQuery<'t, E> {
    Fresh(Query),
    SendQuery(Query, BoxFuture<'t, Result<(), E>>),
    Complete(Query),
}

impl<'t, E> QSendQuery<'t, E> {
    pub fn query_type(&self) -> &Query {
        match self {
            Self::Fresh(query) => query,
            Self::SendQuery(query, _) => query,
            Self::Complete(query) => query,
        }
    }

    pub fn set_fresh(mut self: std::pin::Pin<&mut Self>, query_type: Query) {
        self.set(Self::Fresh(query_type));
    }

    pub fn set_send_query(mut self: std::pin::Pin<&mut Self>, send_query: BoxFuture<'t, Result<(), E>>) {
        let query_type = self.query_type();

        self.set(Self::SendQuery(*query_type, send_query));
    }

    pub fn set_complete(mut self: std::pin::Pin<&mut Self>) {
        let query_type = self.query_type();

        self.set(QSendQuery::Complete(*query_type));
    }
}

#[pin_project(project = MixedQueryProj)]
pub enum MixedQuery<'a, 'b, 'c, 'd> {
    Tcp(#[pin] TcpQuery<'a, 'b, 'c, 'd>),
    Udp(#[pin] UdpQuery<'a, 'b, 'c, 'd>),
}

impl<'a, 'b, 'c, 'd> Future for MixedQuery<'a, 'b, 'c, 'd> {
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

enum PollSocket<E> {
    Error(E),
    Continue,
    Pending,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
enum LoopPoll {
    Continue,
    Pending,
}

trait FutureSocket<'d, E> {
    /// Polls the socket to try to get the active the socket if possible. Initializes the socket if
    /// needed. If the connection fails, is not allowed, or is killed, PollSocket::Error will be
    /// returned with the error and the socket should not be polled again. Even after the connection
    /// is Acquired, calling this function to poll the kill token to be notified when the connection
    /// is killed.
    fn poll<'a>(self: &mut Pin<&mut Self>, socket: &'a Arc<MixedSocket>, cx: &mut std::task::Context<'_>) -> PollSocket<E> where 'a: 'd;
}

#[pin_project(project = TQSocketProj)]
enum TQSocket<'c, 'd>
where
    'd: 'c,
{
    Fresh,
    GetTcpState(BoxFuture<'c, RwLockReadGuard<'d, TcpState>>),
    GetTcpEstablishing {
        #[pin]
        receive_tcp_socket: once_watch::Receiver<(Arc<Mutex<OwnedWriteHalf>>, AwakeToken)>,
    },
    InitTcp {
        #[pin]
        join_handle: JoinHandle<Result<(Arc<Mutex<OwnedWriteHalf>>, AwakeToken), errors::TcpInitError>>,
    },
    Acquired {
        tcp_socket: Arc<Mutex<OwnedWriteHalf>>,
        #[pin]
        kill_tcp: AwokenToken,
    },
    Closed(errors::TcpSocketError),
}

impl<'a, 'c, 'd, 'e> TQSocket<'c, 'd>
where
    'a: 'd,
    'd: 'c,
{
    fn set_get_tcp_state(mut self: std::pin::Pin<&mut Self>, socket: &'a Arc<MixedSocket>) {
        let r_tcp_state = socket.tcp.read().boxed();

        self.set(TQSocket::GetTcpState(r_tcp_state));
    }

    fn set_get_tcp_establishing(mut self: std::pin::Pin<&mut Self>, receiver: once_watch::Receiver<(Arc<Mutex<OwnedWriteHalf>>, AwakeToken)>) {
        self.set(TQSocket::GetTcpEstablishing { receive_tcp_socket: receiver });
    }

    fn set_init_tcp(mut self: std::pin::Pin<&mut Self>, socket: &'a Arc<MixedSocket>) {
        let init_tcp = tokio::spawn(socket.clone().init_tcp());

        self.set(TQSocket::InitTcp { join_handle: init_tcp });
    }

    fn set_acquired(mut self: std::pin::Pin<&mut Self>, tcp_socket: Arc<Mutex<OwnedWriteHalf>>, kill_tcp_token: AwakeToken) {
        self.set(TQSocket::Acquired { tcp_socket, kill_tcp: kill_tcp_token.awoken() });
    }

    fn set_closed(mut self: std::pin::Pin<&mut Self>, reason: errors::TcpSocketError) {
        self.set(TQSocket::Closed(reason));
    }
}

impl<'c, 'd> FutureSocket<'d, errors::TcpSocketError> for TQSocket<'c, 'd> {
    fn poll<'a>(self: &mut Pin<&mut Self>, socket: &'a Arc<MixedSocket>, cx: &mut std::task::Context<'_>) -> PollSocket<errors::TcpSocketError> where 'a: 'd {
        match self.as_mut().project() {
            TQSocketProj::Fresh => {
                self.as_mut().set_get_tcp_state(socket);

                // Next loop should poll `r_tcp_state`
                return PollSocket::Continue;
            },
            TQSocketProj::GetTcpState(r_tcp_state) => {
                match r_tcp_state.as_mut().poll(cx) {
                    Poll::Ready(tcp_state) => {
                        match &*tcp_state {
                            TcpState::Managed { socket, kill } => {
                                self.as_mut().set_acquired(socket.clone(), kill.clone());

                                // Next loop should poll `kill_tcp`
                                return PollSocket::Continue;
                            },
                            TcpState::Establishing { sender, kill: _ } => {
                                self.as_mut().set_get_tcp_establishing(sender.subscribe());

                                // Next loop should poll `receive_tcp_socket`
                                return PollSocket::Continue;
                            },
                            TcpState::None => {
                                self.as_mut().set_init_tcp(socket);

                                // Next loop should poll `join_handle`
                                return PollSocket::Continue;
                            },
                            TcpState::Blocked => {
                                let error = errors::TcpSocketError::Disabled;

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
            TQSocketProj::GetTcpEstablishing { mut receive_tcp_socket } => {
                match receive_tcp_socket.as_mut().poll(cx) {
                    Poll::Ready(Ok((tcp_socket, tcp_kill))) => {
                        self.as_mut().set_acquired(tcp_socket, tcp_kill);

                        // Next loop should poll `kill_tcp`
                        return PollSocket::Continue;
                    },
                    Poll::Ready(Err(once_watch::RecvError::Closed)) => {
                        let error = errors::TcpSocketError::Shutdown;

                        self.as_mut().set_closed(error.clone());

                        return PollSocket::Error(error);
                    },
                    Poll::Pending => {
                        return PollSocket::Pending;
                    },
                }
            },
            TQSocketProj::InitTcp { mut join_handle } => {
                match join_handle.as_mut().poll(cx) {
                    Poll::Ready(Ok(Ok((tcp_socket, kill_tcp_token)))) => {
                        self.as_mut().set_acquired(tcp_socket, kill_tcp_token);

                        // Next loop should poll `kill_tcp`
                        return PollSocket::Continue;
                    },
                    Poll::Ready(Ok(Err(error))) => {
                        let error = errors::TcpSocketError::from(error);

                        self.as_mut().set_closed(error.clone());

                        return PollSocket::Error(error);
                    },
                    Poll::Ready(Err(join_error)) => {
                        let error = errors::TcpSocketError::from(
                            errors::TcpInitError::from(join_error)
                        );

                        self.as_mut().set_closed(error.clone());

                        return PollSocket::Error(error);
                    },
                    Poll::Pending => {
                        return PollSocket::Pending;
                    },
                }
            },
            TQSocketProj::Acquired { tcp_socket: _, mut kill_tcp } => {
                match kill_tcp.as_mut().poll(cx) {
                    Poll::Ready(()) => {
                        let error = errors::TcpSocketError::Shutdown;

                        self.as_mut().set_closed(error.clone());

                        return PollSocket::Error(error);
                    },
                    Poll::Pending => {
                        return PollSocket::Pending;
                    },
                }
            },
            TQSocketProj::Closed(error) => {
                return PollSocket::Error(error.clone());
            },
        }
    }
}

#[pin_project(project = UQSocketProj)]
enum UQSocket<'c, 'd, 'e>
where
    'd: 'c,
{
    Fresh,
    GetReadUdpState(BoxFuture<'c, RwLockReadGuard<'d, UdpState>>),
    InitUdp(BoxFuture<'e, Result<(Arc<UdpSocket>, AwakeToken), errors::UdpInitError>>),
    GetWriteUdpState(BoxFuture<'c, RwLockWriteGuard<'d, UdpState>>, Arc<UdpSocket>, AwakeToken),
    Acquired {
        udp_socket: Arc<UdpSocket>,
        #[pin]
        kill_udp: AwokenToken,
    },
    Closed(errors::UdpSocketError),
}

impl<'a, 'c, 'd, 'e> UQSocket<'c, 'd, 'e>
where
    'a: 'd,
    'd: 'c,
{
    fn set_get_read_udp_state(mut self: std::pin::Pin<&mut Self>, socket: &'a Arc<MixedSocket>) {
        let r_udp_state = socket.udp.read().boxed();

        self.set(UQSocket::GetReadUdpState(r_udp_state));
    }

    fn set_init_udp(mut self: std::pin::Pin<&mut Self>, socket: &'a Arc<MixedSocket>) {
        let upstream_socket = socket.upstream_socket;
        let init_udp = async move {
            let udp_socket = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);
            udp_socket.connect(upstream_socket).await?;
            return Ok((udp_socket, AwakeToken::new()));
        }.boxed();

        self.set(UQSocket::InitUdp(init_udp));
    }
    fn set_get_write_udp_state(mut self: std::pin::Pin<&mut Self>, socket: &'a Arc<MixedSocket>, udp_socket: Arc<UdpSocket>, kill_udp: AwakeToken) {
        let w_udp_state = socket.udp.write().boxed();

        self.set(UQSocket::GetWriteUdpState(w_udp_state, udp_socket, kill_udp));
    }

    fn set_acquired(mut self: std::pin::Pin<&mut Self>, udp_socket: Arc<UdpSocket>, kill_udp_token: AwakeToken) {
        self.set(UQSocket::Acquired { udp_socket, kill_udp: kill_udp_token.awoken() });
    }

    fn set_closed(mut self: std::pin::Pin<&mut Self>, reason: errors::UdpSocketError) {
        self.set(UQSocket::Closed(reason));
    }
}

impl<'c, 'd, 'e> FutureSocket<'d, errors::UdpSocketError> for UQSocket<'c, 'd, 'e> {
    fn poll<'a>(self: &mut Pin<&mut Self>, socket: &'a Arc<MixedSocket>, cx: &mut std::task::Context<'_>) -> PollSocket<errors::UdpSocketError> where 'a: 'd {
        match self.as_mut().project() {
            UQSocketProj::Fresh => {
                self.as_mut().set_get_read_udp_state(socket);

                // Next loop should poll `r_udp_state`
                return PollSocket::Continue;
            },
            UQSocketProj::GetReadUdpState(r_udp_state) => {
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
            UQSocketProj::InitUdp(init_udp) => {
                match init_udp.as_mut().poll(cx) {
                    Poll::Ready(Ok((udp_socket, kill_udp_token))) => {
                        task::spawn(socket.clone().listen_udp(udp_socket.clone(), kill_udp_token.clone()));
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
            UQSocketProj::GetWriteUdpState(w_udp_state, udp_socket, kill_udp) => {
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
            UQSocketProj::Acquired { udp_socket: _, mut kill_udp } => {
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
            UQSocketProj::Closed(error) => {
                return PollSocket::Error(error.clone());
            },
        }
    }
}

#[pin_project(project = EitherSocketProj)]
enum EitherSocket<'c, 'd, 'e> {
    Udp {
        #[pin]
        uq_socket: UQSocket<'c, 'd, 'e>,
        retransmits: u8,
    },
    Tcp {
        #[pin]
        tq_socket: TQSocket<'c, 'd>,
    },
}

impl<'c, 'd, 'e> FutureSocket<'d, errors::SocketError> for EitherSocket<'c, 'd, 'e> {
    fn poll<'a>(self: &mut Pin<&mut Self>, socket: &'a Arc<MixedSocket>, cx: &mut std::task::Context<'_>) -> PollSocket<errors::SocketError> where 'a: 'd {
        match self.as_mut().project() {
            EitherSocketProj::Udp { mut uq_socket, retransmits: _ } => {
                match uq_socket.poll(socket, cx) {
                    PollSocket::Error(error) => PollSocket::Error(errors::SocketError::from(error)),
                    PollSocket::Continue => PollSocket::Continue,
                    PollSocket::Pending => PollSocket::Pending,
                }
            },
            EitherSocketProj::Tcp { mut tq_socket } => {
                match tq_socket.poll(socket, cx) {
                    PollSocket::Error(error) => PollSocket::Error(errors::SocketError::from(error)),
                    PollSocket::Continue => PollSocket::Continue,
                    PollSocket::Pending => PollSocket::Pending,
                }
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

enum TcpState {
    Managed {
        socket: Arc<Mutex<OwnedWriteHalf>>,
        kill: AwakeToken
    },
    Establishing {
        sender: once_watch::Sender<(Arc<Mutex<OwnedWriteHalf>>, AwakeToken)>,
        kill: AwakeToken
    },
    None,
    Blocked,
}

#[pin_project(PinnedDrop)]
struct InitTcp<'a, 'b, 'c, 'd, 'e, 'f, 'k, 'l>
where
    'a: 'c + 'f + 'l
{
    socket: &'a Arc<MixedSocket>,
    #[pin]
    kill_tcp: AwokenToken,
    tcp_socket_sender: once_watch::Sender<(Arc<Mutex<OwnedWriteHalf>>, AwakeToken)>,
    #[pin]
    timeout: Sleep,
    #[pin]
    inner: InnerInitTcp<'b, 'c, 'd, 'e, 'f, 'k, 'l>,
}

#[pin_project(project = InnerInitTcpProj)]
enum InnerInitTcp<'b, 'c, 'd, 'e, 'f, 'k, 'l>
where
    'c: 'b,
    'f: 'e,
    'l: 'k,
{
    Fresh,
    WriteEstablishing(BoxFuture<'b, RwLockWriteGuard<'c, TcpState>>),
    Connecting(BoxFuture<'d, io::Result<TcpStream>>),
    WriteNone {
        reason: CleanupReason<errors::TcpInitError>,
        w_tcp_state: BoxFuture<'e, RwLockWriteGuard<'f, TcpState>>,
    },
    WriteManaged {
        w_tcp_state: BoxFuture<'k, RwLockWriteGuard<'l, TcpState>>,
        tcp_socket: Arc<Mutex<OwnedWriteHalf>>,
    },
    GetEstablishing {
        #[pin]
        receive_tcp_socket: once_watch::Receiver<(Arc<Mutex<OwnedWriteHalf>>, AwakeToken)>,
    },
    Complete,
}

impl<'a, 'b, 'c, 'd, 'e, 'f, 'k, 'l> InitTcp<'a, 'b, 'c, 'd, 'e, 'f, 'k, 'l> {
    pub fn new(socket: &'a Arc<MixedSocket>, timeout: Option<Duration>) -> Self {
        let kill_tcp_token = AwakeToken::new();
        let tcp_socket_sender = once_watch::Sender::new();
        let timeout = timeout.unwrap_or(TCP_INIT_TIMEOUT);

        Self {
            socket,
            kill_tcp: kill_tcp_token.awoken(),
            tcp_socket_sender,
            timeout: tokio::time::sleep(timeout),
            inner: InnerInitTcp::Fresh,
        }
    }
}

impl<'a, 'b, 'c, 'd, 'e, 'f, 'k, 'l> Future for InitTcp<'a, 'b, 'c, 'd, 'e, 'f, 'k, 'l> {
    type Output = Result<(Arc<Mutex<OwnedWriteHalf>>, AwakeToken), errors::TcpInitError>;

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let mut this = self.as_mut().project();
        match this.inner.as_mut().project() {
            InnerInitTcpProj::Fresh
          | InnerInitTcpProj::WriteEstablishing(_) => {
                if let Poll::Ready(()) = this.kill_tcp.as_mut().poll(cx) {
                    this.tcp_socket_sender.close();
                    this.kill_tcp.awake();
                    let error = errors::TcpInitError::SocketShutdown;
                    
                    *this.inner = InnerInitTcp::Complete;

                    // Exit loop: query killed.
                    return Poll::Ready(Err(error));
                }

                if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                    this.tcp_socket_sender.close();
                    this.kill_tcp.awake();
                    let error = errors::TcpInitError::Timeout;

                    *this.inner = InnerInitTcp::Complete;

                    // Exit loop: query timed out.
                    return Poll::Ready(Err(error));
                }
            },
            InnerInitTcpProj::Connecting(_) => {
                if let Poll::Ready(()) = this.kill_tcp.as_mut().poll(cx) {
                    let w_tcp_state = this.socket.tcp.write().boxed();

                    *this.inner = InnerInitTcp::WriteNone { reason: CleanupReason::Timeout, w_tcp_state };

                    // First loop: poll the write lock.
                } else if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                    let w_tcp_state = this.socket.tcp.write().boxed();

                    *this.inner = InnerInitTcp::WriteNone { reason: CleanupReason::Killed, w_tcp_state };

                    // First loop: poll the write lock.
                }
            },
            InnerInitTcpProj::GetEstablishing { receive_tcp_socket: _ } => {
                // Does not poll `kill_tcp` because that gets awoken to kill
                // the listener (if it is set up).
                if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                    this.tcp_socket_sender.close();
                    this.kill_tcp.awake();
                    let error = errors::TcpInitError::Timeout;

                    *this.inner = InnerInitTcp::Complete;

                    // Exit loop: query timed out.
                    return Poll::Ready(Err(error));
                }
            },
            InnerInitTcpProj::WriteNone { reason: _, w_tcp_state: _ }
          | InnerInitTcpProj::WriteManaged { w_tcp_state: _, tcp_socket: _ }
          | InnerInitTcpProj::Complete => {
                // Not allowed to timeout or be killed. These are cleanup
                // states.
            },
        }

        loop {
            match this.inner.as_mut().project() {
                InnerInitTcpProj::Fresh => {
                    let w_tcp_state = this.socket.tcp.write().boxed();

                    *this.inner = InnerInitTcp::WriteEstablishing(w_tcp_state);

                    // Next loop: poll the write lock to get the TCP state
                    continue;
                }
                InnerInitTcpProj::WriteEstablishing(w_tcp_state) => {
                    match w_tcp_state.as_mut().poll(cx) {
                        Poll::Ready(mut tcp_state) => {
                            match &*tcp_state {
                                TcpState::Managed { socket, kill } => {
                                    let tcp_socket = socket.clone();
                                    let kill_tcp_token = kill.clone();

                                    let _ = this.tcp_socket_sender.send((tcp_socket.clone(), kill_tcp_token.clone()));
                                    this.kill_tcp.awake();

                                    *this.inner = InnerInitTcp::Complete;

                                    // Exit loop: connection already setup.
                                    // Nothing to do.
                                    return Poll::Ready(Ok((tcp_socket, kill_tcp_token)));
                                },
                                TcpState::Establishing { sender: active_sender, kill: _ } => {
                                    let receive_tcp_socket = active_sender.subscribe();

                                    *this.inner = InnerInitTcp::GetEstablishing { receive_tcp_socket };

                                    // Next loop: poll the receiver. Another
                                    // process is setting up the connection.
                                    continue;
                                },
                                TcpState::None => {
                                    let tcp_socket_sender = this.tcp_socket_sender.clone();
                                    let kill_init_tcp = this.kill_tcp.get_awake_token();
                                    let init_connection = TcpStream::connect(this.socket.upstream_socket).boxed();

                                    *tcp_state = TcpState::Establishing {
                                        sender: tcp_socket_sender,
                                        kill: kill_init_tcp,
                                    };

                                    *this.inner = InnerInitTcp::Connecting(init_connection);

                                    // Next loop: poll the TCP stream and start
                                    // connecting.
                                    continue;
                                },
                                TcpState::Blocked => {
                                    this.tcp_socket_sender.close();
                                    this.kill_tcp.awake();
                                    let error = errors::TcpInitError::SocketDisabled;

                                    *this.inner = InnerInitTcp::Complete;

                                    // Exit loop: connection not allowed.
                                    return Poll::Ready(Err(error));
                                },
                            }
                        }
                        Poll::Pending => {
                            // Exit loop. Will be woken up once the TcpState
                            // write lock is available, the timeout condition
                            // occurs, or the connection is killed.
                            return Poll::Pending;
                        },
                    }
                },
                InnerInitTcpProj::Connecting(init_connection) => {
                    match init_connection.as_mut().poll(cx) {
                        Poll::Ready(Ok(socket)) => {
                            let (tcp_reader, tcp_writer) = socket.into_split();
                            let tcp_socket = Arc::new(Mutex::new(tcp_writer));
                            let w_tcp_state = this.socket.tcp.write().boxed();
                            task::spawn(this.socket.clone().listen_tcp(tcp_reader, this.kill_tcp.get_awake_token()));

                            *this.inner = InnerInitTcp::WriteManaged { w_tcp_state, tcp_socket };

                            // Next loop: poll the write lock.
                            continue;
                        },
                        Poll::Ready(Err(error)) => {
                            let w_tcp_state = this.socket.tcp.write().boxed();
                            let error = errors::TcpInitError::from(error);

                            *this.inner = InnerInitTcp::WriteNone { reason: CleanupReason::ConnectionError(error), w_tcp_state };

                            // Next loop: poll the write lock.
                            continue;
                        },
                        Poll::Pending => {
                            // Exit loop. Will be woken up once TCP is
                            // connected, the timeout condition occurs, or the
                            // connection is killed.
                            return Poll::Pending;
                        },
                    }
                },
                InnerInitTcpProj::WriteNone { reason: CleanupReason::ConnectionError(error), w_tcp_state } => {
                    match w_tcp_state.as_mut().poll(cx) {
                        Poll::Ready(mut w_tcp_state) => {
                            match &*w_tcp_state {
                                TcpState::Managed { socket, kill } => {
                                    let tcp_socket = socket.clone();
                                    let kill_tcp_token = kill.clone();

                                    let _ = this.tcp_socket_sender.send((tcp_socket.clone(), kill_tcp_token.clone()));
                                    this.kill_tcp.awake();

                                    *this.inner = InnerInitTcp::Complete;

                                    // Exit loop: connection already setup.
                                    // Nothing to do.
                                    return Poll::Ready(Ok((tcp_socket, kill_tcp_token)));
                                },
                                TcpState::Establishing { sender, kill: active_kill_tcp_token } => {
                                    // If we are the one who set the state to Establishing...
                                    if this.kill_tcp.same_awake_token(active_kill_tcp_token) {
                                        *w_tcp_state = TcpState::None;
                                        drop(w_tcp_state);
                                        this.tcp_socket_sender.close();
                                        this.kill_tcp.awake();
                                        let error = error.clone();

                                        *this.inner = InnerInitTcp::Complete;

                                        // Exit loop: we received a connection
                                        // error.
                                        return Poll::Ready(Err(error));
                                    // If some other process set the state to Establishing...
                                    } else {
                                        let receive_tcp_socket = sender.subscribe();

                                        *this.inner = InnerInitTcp::GetEstablishing { receive_tcp_socket };

                                        // Next loop: poll the receiver.
                                        continue;
                                    }
                                },
                                TcpState::None
                              | TcpState::Blocked => {
                                    drop(w_tcp_state);
                                    this.tcp_socket_sender.close();
                                    this.kill_tcp.awake();
                                    let error = error.clone();

                                    *this.inner = InnerInitTcp::Complete;

                                    // Exit loop: we received a connection
                                    // error.
                                    return Poll::Ready(Err(error));
                                },
                            }
                        },
                        Poll::Pending => {
                            // Exit loop. Will be woken up once the TcpState
                            // write lock is available. Cannot time out or be
                            // killed.
                            return Poll::Pending;
                        },
                    }
                },
                InnerInitTcpProj::WriteNone { reason: CleanupReason::Timeout, w_tcp_state } => {
                    match w_tcp_state.as_mut().poll(cx) {
                        Poll::Ready(mut w_tcp_state) => {
                            match &*w_tcp_state {
                                TcpState::Managed { socket, kill } => {
                                    let tcp_socket = socket.clone();
                                    let kill_tcp_token = kill.clone();

                                    let _ = this.tcp_socket_sender.send((tcp_socket.clone(), kill_tcp_token.clone()));
                                    this.kill_tcp.awake();

                                    *this.inner = InnerInitTcp::Complete;

                                    // Exit loop: connection already setup.
                                    // Nothing to do.
                                    return Poll::Ready(Ok((tcp_socket, kill_tcp_token)));
                                },
                                TcpState::Establishing { sender: _, kill: active_kill_tcp_token } => {
                                    // If we are the one who set the state to Establishing...
                                    if this.kill_tcp.same_awake_token(active_kill_tcp_token) {
                                        *w_tcp_state = TcpState::None;
                                    }
                                    drop(w_tcp_state);
                                    this.tcp_socket_sender.close();
                                    this.kill_tcp.awake();

                                    *this.inner = InnerInitTcp::Complete;

                                    // Exit loop: connection timed out.
                                    return Poll::Ready(Err(errors::TcpInitError::Timeout));
                                },
                                TcpState::None
                              | TcpState::Blocked => {
                                    drop(w_tcp_state);
                                    this.tcp_socket_sender.close();
                                    this.kill_tcp.awake();

                                    *this.inner = InnerInitTcp::Complete;

                                    // Exit loop: connection timed out.
                                    return Poll::Ready(Err(errors::TcpInitError::Timeout));
                                },
                            }
                        },
                        Poll::Pending => {
                            // Exit loop. Will be woken up once the TcpState
                            // write lock is available. Cannot time out or be
                            // killed.
                            return Poll::Pending;
                        },
                    }
                },
                InnerInitTcpProj::WriteNone { reason: CleanupReason::Killed, w_tcp_state } => {
                    match w_tcp_state.as_mut().poll(cx) {
                        Poll::Ready(mut w_tcp_state) => {
                            match &*w_tcp_state {
                                TcpState::Establishing { sender: _, kill: active_kill_tcp_token } => {
                                    // If we are the one who set the state to Establishing...
                                    if this.kill_tcp.same_awake_token(active_kill_tcp_token) {
                                        *w_tcp_state = TcpState::None;
                                    }
                                    drop(w_tcp_state);
                                    this.tcp_socket_sender.close();
                                    this.kill_tcp.awake();

                                    *this.inner = InnerInitTcp::Complete;

                                    // Exit loop: connection killed.
                                    return Poll::Ready(Err(errors::TcpInitError::SocketShutdown));
                                },
                                TcpState::Managed { socket: _, kill: _ }
                              | TcpState::None
                              | TcpState::Blocked => {
                                    drop(w_tcp_state);
                                    this.tcp_socket_sender.close();
                                    this.kill_tcp.awake();

                                    *this.inner = InnerInitTcp::Complete;

                                    // Exit loop: connection killed.
                                    return Poll::Ready(Err(errors::TcpInitError::SocketShutdown));
                                },
                            }
                        },
                        Poll::Pending => {
                            // Exit loop. Will be woken up once the TcpState
                            // write lock is available. Cannot time out or be
                            // killed.
                            return Poll::Pending;
                        },
                    }
                },
                InnerInitTcpProj::WriteManaged { w_tcp_state, tcp_socket } => {
                    match w_tcp_state.as_mut().poll(cx) {
                        Poll::Ready(mut w_tcp_state) => {
                            match &*w_tcp_state {
                                TcpState::Establishing { sender: active_sender, kill: active_kill_tcp_token } => {
                                    // If we are the one who set the state to Establishing...
                                    if this.kill_tcp.same_awake_token(active_kill_tcp_token) {
                                        *w_tcp_state = TcpState::Managed { socket: tcp_socket.clone(), kill: this.kill_tcp.get_awake_token() };
                                        drop(w_tcp_state);

                                        let _ = this.tcp_socket_sender.send((tcp_socket.clone(), this.kill_tcp.get_awake_token()));

                                        let tcp_socket = tcp_socket.clone();
                                        let kill_tcp_token = this.kill_tcp.get_awake_token();

                                        *this.inner = InnerInitTcp::Complete;

                                        // Exit loop: connection setup
                                        // completed and registered.
                                        return Poll::Ready(Ok((tcp_socket, kill_tcp_token)));
                                    // If some other process set the state to Establishing...
                                    } else {
                                        let receive_tcp_socket = active_sender.subscribe();
                                        drop(w_tcp_state);

                                        // Shutdown the listener we started.
                                        this.kill_tcp.awake();

                                        *this.inner = InnerInitTcp::GetEstablishing { receive_tcp_socket };

                                        // Next loop: poll the receiver.
                                        continue;
                                    }
                                },
                                TcpState::Managed { socket, kill } => {
                                    let tcp_socket = socket.clone();
                                    let kill_tcp_token = kill.clone();
                                    drop(w_tcp_state);

                                    let _ = this.tcp_socket_sender.send((tcp_socket.clone(), kill_tcp_token.clone()));
                                    // Shutdown the listener we started.
                                    this.kill_tcp.awake();

                                    *this.inner = InnerInitTcp::Complete;

                                    // Exit loop: connection already setup.
                                    // Nothing to do.
                                    return Poll::Ready(Ok((tcp_socket, kill_tcp_token)));
                                },
                                TcpState::None
                              | TcpState::Blocked => {
                                    drop(w_tcp_state);

                                    this.tcp_socket_sender.close();
                                    // Shutdown the listener we started.
                                    this.kill_tcp.awake();

                                    *this.inner = InnerInitTcp::Complete;

                                    // Exit loop: state changed after this task
                                    // set it to Establishing. Indicates that
                                    // this task is no longer in charge.
                                    return Poll::Ready(Err(errors::TcpInitError::SocketShutdown));
                                },
                            }
                        },
                        Poll::Pending => {
                            // Exit loop. Will be woken up once the TcpState
                            // write lock is available. Cannot time out or be
                            // killed.
                            return Poll::Pending;
                        },
                    }
                },
                InnerInitTcpProj::GetEstablishing { mut receive_tcp_socket } => {
                    match receive_tcp_socket.as_mut().poll(cx) {
                        Poll::Ready(Ok((tcp_socket, kill_tcp_token))) => {
                            let _ = this.tcp_socket_sender.send((tcp_socket.clone(), kill_tcp_token.clone()));
                            this.kill_tcp.awake();

                            *this.inner = InnerInitTcp::Complete;

                            // Exit loop: connection setup completed and
                            // registered by a different init process.
                            return Poll::Ready(Ok((tcp_socket, kill_tcp_token)));
                        },
                        Poll::Ready(Err(once_watch::RecvError::Closed)) => {
                            this.tcp_socket_sender.close();
                            this.kill_tcp.awake();

                            *this.inner = InnerInitTcp::Complete;

                            // Exit loop: all senders were dropped so it is not
                            // possible to receive a connection.
                            return Poll::Ready(Err(errors::TcpInitError::SocketShutdown));
                        },
                        Poll::Pending => {
                            // Exit loop. Will be woken up once a TCP write
                            // handle is received or the timeout condition
                            // occurs. Cannot be killed because it may have
                            // already been killed by self.
                            return Poll::Pending;
                        },
                    }
                },
                InnerInitTcpProj::Complete => panic!("InitTcp was polled after completion"),
            }
        }
    }
}

#[pinned_drop]
impl<'a, 'b, 'c, 'd, 'e, 'f, 'k, 'l> PinnedDrop for InitTcp<'a, 'b, 'c, 'd, 'e, 'f, 'k, 'l> {
    fn drop(self: Pin<&mut Self>) {
        match &self.inner {
            InnerInitTcp::Fresh
          | InnerInitTcp::WriteEstablishing(_)
          | InnerInitTcp::GetEstablishing { receive_tcp_socket: _ }
          | InnerInitTcp::Complete => {
                // Nothing to do.
            },
            InnerInitTcp::Connecting(_)
          | InnerInitTcp::WriteNone { reason: _, w_tcp_state: _ } => {
                let tcp_socket = self.socket.clone();
                let kill_tcp_token = self.kill_tcp.get_awake_token();
                tokio::spawn(async move {
                    let mut w_tcp_state = tcp_socket.tcp.write().await;
                    match &*w_tcp_state {
                        TcpState::Establishing { sender: _, kill: active_kill_tcp_token } => {
                            // If we are the one who set the state to Establishing...
                            if &kill_tcp_token == active_kill_tcp_token {
                                *w_tcp_state = TcpState::None;
                            }
                            drop(w_tcp_state);
                        },
                        TcpState::Managed { socket: _, kill: _ }
                      | TcpState::None
                      | TcpState::Blocked => {
                            drop(w_tcp_state);
                        },
                    }
                });
            },
            // If this struct is dropped while it is trying to write the
            // connection to the TcpState, we will spawn a task to complete
            // this operation. This way, those that depend on receiving this
            // the connection don't unexpectedly receive errors and try to
            // re-initialize the connection.
            InnerInitTcp::WriteManaged { w_tcp_state: _, tcp_socket } => {
                let tcp_socket = tcp_socket.clone();
                let socket = self.socket.clone();
                let tcp_socket_sender = self.tcp_socket_sender.clone();
                let kill_tcp_token = self.kill_tcp.get_awake_token();
                tokio::spawn(async move {
                    let mut w_tcp_state = socket.tcp.write().await;
                    match &*w_tcp_state {
                        TcpState::Establishing { sender: _, kill: active_kill_tcp_token } => {
                            // If we are the one who set the state to Establishing...
                            if &kill_tcp_token == active_kill_tcp_token {
                                *w_tcp_state = TcpState::Managed { socket: tcp_socket.clone(), kill: kill_tcp_token.clone() };
                                drop(w_tcp_state);

                                // Ignore send errors. They just indicate that all receivers have been dropped.
                                let _ = tcp_socket_sender.send((tcp_socket, kill_tcp_token));
                            // If some other process set the state to Establishing...
                            } else {
                                drop(w_tcp_state);

                                // Shutdown the listener we started.
                                kill_tcp_token.awake();
                            }
                        },
                        TcpState::Managed { socket: _, kill: _ }
                      | TcpState::None
                      | TcpState::Blocked => {
                            drop(w_tcp_state);

                            // Shutdown the listener we started.
                            kill_tcp_token.awake();
                        },
                    }
                });
            },
        }
    }
}

#[pin_project(PinnedDrop)]
struct TcpQueryRunner<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h>
where
    'a: 'd + 'g
{
    socket: &'a Arc<MixedSocket>,
    query: &'b mut Message,
    tcp_timeout: &'h Duration,
    tcp_start_time: Instant,
    #[pin]
    timeout: Sleep,
    #[pin]
    result_receiver: once_watch::Receiver<Result<Message, errors::QueryError>>,
    #[pin]
    inner: InnerTQ<'c, 'd, 'e, 'f, 'g>,
}

impl<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h> TcpQueryRunner<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h>
where
    'g: 'f
{
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
enum InnerTQ<'c, 'd, 'e, 'f, 'g>
where
    'g: 'f
{
    Fresh,
    Running {
        #[pin]
        tq_socket: TQSocket<'c, 'd>,
        #[pin]
        send_query: QSendQuery<'e, errors::TcpSendError>,
    },
    Cleanup(BoxFuture<'f, RwLockWriteGuard<'g, ActiveQueries>>, TcpResponseTime),
    Complete,
}

impl<'a, 'c, 'd, 'e, 'f, 'g> InnerTQ<'c, 'd, 'e, 'f, 'g>
where
    'a: 'd + 'g
{
    pub fn set_running(mut self: std::pin::Pin<&mut Self>, query_type: Query) {
        self.set(Self::Running {
            tq_socket: TQSocket::Fresh,
            send_query: QSendQuery::Fresh(query_type),
        });
    }

    pub fn set_cleanup(mut self: std::pin::Pin<&mut Self>, execution_time: TcpResponseTime, socket: &'a Arc<MixedSocket>) {
        let w_active_queries = socket.active_queries.write().boxed();

        self.set(Self::Cleanup(w_active_queries, execution_time));
    }

    pub fn set_complete(mut self: std::pin::Pin<&mut Self>) {
        self.set(Self::Complete);
    }
}

impl<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h> Future for TcpQueryRunner<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h> {
    type Output = ();

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let mut this = self.as_mut().project();
        match this.inner.as_mut().project() {
            InnerTQProj::Fresh
          | InnerTQProj::Running { tq_socket: _, send_query: _ } => {
                if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                    let _ = this.result_receiver.get_sender().send(Err(errors::QueryError::Timeout));

                    this.inner.set_cleanup(TcpResponseTime::Dropped, this.socket);

                    // Exit loop forever: query timed out.
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
                    this.inner.set_running(Query::Initial);
            
                    // Next loop: poll tq_socket and in_flight to start getting the TCP socket and
                    // inserting the query ID into the in-flight map.
                    continue;
                },
                InnerTQProj::Running { mut tq_socket, mut send_query } => {
                    match (send_query.as_mut().project(), tq_socket.as_mut().project()) {
                        (QSendQueryProj::Fresh(_), TQSocketProj::Fresh)
                      | (QSendQueryProj::Fresh(_), TQSocketProj::GetTcpState(_))
                      | (QSendQueryProj::Fresh(_), TQSocketProj::GetTcpEstablishing { receive_tcp_socket: _ })
                      | (QSendQueryProj::Fresh(_), TQSocketProj::InitTcp { join_handle: _ })
                      | (QSendQueryProj::Fresh(_), TQSocketProj::Closed(_)) => {
                            // We don't poll the receiver until the QSendQuery state is Complete.
            
                            match tq_socket.poll(this.socket, cx) {
                                PollSocket::Error(error) => {
                                    let _ = this.result_receiver.get_sender().send(Err(errors::QueryError::from(error)));

                                    this.inner.set_cleanup(TcpResponseTime::None, this.socket);
            
                                    // Next loop will poll for the in-flight map lock to clean up
                                    // the query ID before returning the response.
                                    continue;
                                },
                                PollSocket::Continue => {
                                    continue;
                                },
                                PollSocket::Pending => {
                                    // The TQSocket is the only future that we are waiting on,
                                    // besides the timeout. We are already registered with the
                                    // in-flight map and cannot send or receive a query until a
                                    // socket is established.
                                    return Poll::Pending;
                                },
                            }
                        },
                        (QSendQueryProj::Fresh(_), TQSocketProj::Acquired { tcp_socket, kill_tcp: _ }) => {
                            // We don't poll the receiver until the QSendQuery state is Complete.
            
                            let socket = this.socket.clone();
                            let tcp_socket = tcp_socket.clone();
            
                            if let PollSocket::Error(error) = tq_socket.poll(this.socket, cx) {
                                let _ = this.result_receiver.get_sender().send(Err(errors::QueryError::from(error)));

                                this.inner.set_cleanup(TcpResponseTime::None, this.socket);
            
                                // Next loop will poll for the in-flight map lock to clean up the
                                // query ID before returning the response.
                                continue;
                            }
            
                            let mut raw_message = [0_u8; MAX_MESSAGE_SIZE as usize];
                            let mut write_wire = WriteWire::from_bytes(&mut raw_message);
                            if let Err(wire_error) = this.query.to_wire_format_with_two_octet_length(&mut write_wire, &mut Some(CompressionMap::new())) {
                                let _ = this.result_receiver.get_sender().send(Err(errors::QueryError::from(errors::TcpSendError::from(wire_error))));

                                this.inner.set_cleanup(TcpResponseTime::None, this.socket);
            
                                // Next loop will poll for the in-flight map lock to clean up the
                                // query ID before returning the response.
                                continue;
                            };
                            let wire_length = write_wire.current_len();
            
                            println!("Sending on TCP socket {} {{ drop rate {:.2}%, truncation rate {:.2}%, response time {:.2} ms, timeout {} ms }} :: {:?}", this.socket.upstream_socket, this.socket.average_dropped_tcp_packets() * 100.0, this.socket.average_truncated_udp_packets() * 100.0, this.socket.average_tcp_response_time(), this.tcp_timeout.as_millis(), this.query);
            
                            let send_query_future = async move {
                                let socket = socket;
                                let tcp_socket = tcp_socket;
                                let wire_length = wire_length;
            
                                socket.recent_messages_sent.store(true, Ordering::Release);
                                let mut w_tcp_stream = tcp_socket.lock().await;
                                let bytes_written = w_tcp_stream.write(&raw_message[..wire_length]).await?;
                                drop(w_tcp_stream);
                                // Verify that the correct number of bytes were written.
                                if bytes_written != wire_length {
                                    return Err(errors::TcpSendError::IncorrectNumberBytes { expected: wire_length as u16, sent: bytes_written });
                                }
            
                                return Ok(());
                            }.boxed();
            
                            send_query.set_send_query(send_query_future);
            
                            // Next loop will begin to poll SendQuery. This will get the lock and
                            // the TcpStream and write the bytes out.
                            continue;
                        },
                        (QSendQueryProj::SendQuery(_, send_query_future), _) => {
                            // We don't poll the receiver until the QSendQuery state is Complete.
                            match (send_query_future.as_mut().poll(cx), tq_socket.poll(this.socket, cx)) {
                                (_, PollSocket::Error(error)) => {
                                    let _ = this.result_receiver.get_sender().send(Err(errors::QueryError::from(error)));

                                    this.inner.set_cleanup(TcpResponseTime::None, this.socket);
            
                                    // Next loop will poll for the in-flight map lock to clean up
                                    // the query ID before returning the response.
                                    continue;
                                },
                                (Poll::Ready(Err(error)), _) => {
                                    let _ = this.result_receiver.get_sender().send(Err(errors::QueryError::from(error)));

                                    this.inner.set_cleanup(TcpResponseTime::None, this.socket);
            
                                    // Next loop will poll for the in-flight map lock to clean up
                                    // the query ID before returning the response.
                                    continue;
                                },
                                (Poll::Ready(Ok(())), PollSocket::Continue | PollSocket::Pending) => {
                                    send_query.set_complete();
            
                                    // Next loop will poll the receiver, now that a message has been
                                    // sent out.
                                    continue;
                                },
                                (Poll::Pending, PollSocket::Continue) => {
                                    // If at least one of our futures needs to loop again, we should
                                    // loop again unless an exit condition is reached.
                                    continue;
                                },
                                (Poll::Pending, PollSocket::Pending) => {
                                    // All tokens are pending. Will wake up if the TQSocket wakes
                                    // us, the in-flight map lock becomes available, or the timeout
                                    // occurs.
                                    return Poll::Pending;
                                },
                            }
                        },
                        (QSendQueryProj::Complete(_), _) => {
                            match this.result_receiver.as_mut().poll(cx) {
                                Poll::Ready(Ok(Ok(_))) => {
                                    let execution_time = this.tcp_start_time.elapsed();

                                    this.inner.set_cleanup(TcpResponseTime::Responded(execution_time), this.socket);
            
                                    // TODO
                                    continue;
                                },
                                Poll::Ready(Ok(Err(_)))
                              | Poll::Ready(Err(_)) => {
                                    this.inner.set_cleanup(TcpResponseTime::None, this.socket);
            
                                    // TODO
                                    continue;
                                },
                                Poll::Pending => (),
                            }
            
                            match tq_socket.poll(this.socket, cx) {
                                PollSocket::Error(error) => {
                                    let _ = this.result_receiver.get_sender().send(Err(errors::QueryError::from(error)));

                                    this.inner.set_cleanup(TcpResponseTime::None, this.socket);
            
                                    // Next loop will poll for the in-flight map lock to clean up
                                    // the query ID before returning the response.
                                    continue;
                                },
                                PollSocket::Continue => {
                                    // If at least one of our futures needs to loop again, we should
                                    // loop again unless an exit condition is reached.
                                    continue;
                                },
                                PollSocket::Pending => {
                                    // All tokens are pending. Will wake up if the TQSocket wakes
                                    // us, the receiver has a response, or the timeout occurs.
                                    return Poll::Pending;
                                },
                            }
                        },
                    }
                },
                InnerTQProj::Cleanup(w_active_queries, execution_time) => {
                    this.result_receiver.close();

                    match w_active_queries.as_mut().poll(cx) {
                        Poll::Ready(mut w_active_queries) => {
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

                            w_active_queries.in_flight.remove(&this.query.id);
                            w_active_queries.tcp_only.remove(&this.query.question);
                            drop(w_active_queries);

                            this.inner.set_complete();

                            // TODO
                            return Poll::Ready(());
                        },
                        Poll::Pending => {
                            // TODO
                            return Poll::Pending;
                        },
                    }
                },
                InnerTQProj::Complete => {
                    panic!("TCP only query polled after completion");
                },
            }
        }
    }
}

#[pinned_drop]
impl<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h> PinnedDrop for TcpQueryRunner<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h> {
    fn drop(mut self: Pin<&mut Self>) {
        async fn cleanup(socket: Arc<MixedSocket>, query: Message) {
            let mut w_active_queries = socket.active_queries.write().await;
            let _ = w_active_queries.in_flight.remove(&query.id);
            let _ = w_active_queries.tcp_only.remove(&query.question);
            drop(w_active_queries);
        }

        match self.as_mut().project().inner.as_mut().project() {
            InnerTQProj::Fresh
          | InnerTQProj::Running { tq_socket: _, send_query: _ }
          | InnerTQProj::Cleanup(_, _) => {
                let socket = self.socket.clone();
                let query = self.query.clone();
                tokio::spawn(cleanup(socket, query));
            },
            InnerTQProj::Complete => {
                // Nothing to do for active queries.
            }
        }
    }
}

#[pin_project]
struct TcpQuery<'a, 'b, 'c, 'd>
where
    'a: 'd
{
    socket: &'a Arc<MixedSocket>,
    query: &'b mut Message,
    #[pin]
    inner: QInitQuery<'c, 'd>,
}

impl<'a, 'b, 'c, 'd> TcpQuery<'a, 'b, 'c, 'd> {
    pub fn new(socket: &'a Arc<MixedSocket>, query: &'b mut Message) -> Self {
        Self {
            socket,
            query,
            inner: QInitQuery::Fresh,
        }
    }
}

impl<'a, 'b, 'c, 'd> Future for TcpQuery<'a, 'b, 'c, 'd> {
    type Output = Result<Message, errors::QueryError>;

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        loop {
            let mut this = self.as_mut().project();
            match this.inner.as_mut().project() {
                QInitQueryProj::Fresh => {
                    this.inner.set_read_active_query(this.socket);

                    // TODO
                    continue;
                },
                QInitQueryProj::ReadActiveQuery(r_active_queries) => {
                    match r_active_queries.as_mut().poll(cx) {
                        Poll::Ready(r_active_queries) => {
                            match r_active_queries.tcp_only.get(&this.query.question) {
                                Some((query_id, result_sender)) => {
                                    this.query.id = *query_id;
                                    let result_receiver = result_sender.subscribe();
                                    drop(r_active_queries);

                                    this.inner.set_following(result_receiver);
                                    // println!("{} Following(1) active query '{}'", this.socket.upstream_socket, this.query.question);

                                    // TODO
                                    continue;
                                },
                                None => {
                                    drop(r_active_queries);
                                    this.inner.set_write_active_query(this.socket);

                                    // TODO
                                    continue;                
                                },
                            }
                        },
                        Poll::Pending => {
                            // TODO
                            return Poll::Pending;
                        },
                    }
                },
                QInitQueryProj::WriteActiveQuery(w_active_queries) => {
                    match w_active_queries.as_mut().poll(cx) {
                        Poll::Ready(mut w_active_queries) => {
                            match w_active_queries.tcp_only.get(&this.query.question) {
                                Some((query_id, result_sender)) => {
                                    this.query.id = *query_id;
                                    let result_receiver = result_sender.subscribe();
                                    drop(w_active_queries);

                                    this.inner.set_following(result_receiver);
                                    // println!("{} Following(2) active query '{}'", this.socket.upstream_socket, this.query.question);

                                    // TODO
                                    continue;
                                },
                                None => {
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
                                    // println!("{} Following(3) active query '{}'", this.socket.upstream_socket, this.query.question);

                                    // TODO
                                    continue;                
                                },
                            }
                        },
                        Poll::Pending => {
                            // TODO
                            return Poll::Pending;
                        },
                    }
                },
                QInitQueryProj::Following(mut result_receiver) => {
                    match result_receiver.as_mut().poll(cx) {
                        Poll::Ready(Ok(response)) => {
                            this.inner.set_complete();

                            // TODO
                            return Poll::Ready(response);
                        },
                        Poll::Ready(Err(once_watch::RecvError::Closed)) => {
                            let error = errors::QueryError::from(errors::TcpSocketError::Shutdown);

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
                QInitQueryProj::Complete => panic!("TcpQuery cannot be polled after completion"),
            }    
        }
    }
}

// Implement TCP functions on MixedSocket
impl MixedSocket {
    #[inline]
    async fn init_tcp(self: Arc<Self>) -> Result<(Arc<Mutex<OwnedWriteHalf>>, AwakeToken), errors::TcpInitError> {
        InitTcp::new(&self, None).await
    }

    #[inline]
    pub async fn start_tcp(self: Arc<Self>) -> Result<(), errors::TcpInitError> {
        match self.init_tcp().await {
            Ok(_) => Ok(()),
            Err(error) => Err(error),
        }
    }

    #[inline]
    pub async fn shutdown_tcp(self: Arc<Self>) {
        println!("Shutting down TCP socket {}", self.upstream_socket);

        let mut w_state = self.tcp.write().await;
        match &*w_state {
            TcpState::Managed { socket: _, kill } => {
                let tcp_kill = kill.clone();
                *w_state = TcpState::None;
                drop(w_state);

                tcp_kill.awake();

                // Note: this task is not responsible for actual cleanup. Once the listener closes, it
                // will kill any active queries and change the TcpState.
            },
            TcpState::Establishing { sender, kill } => {
                let sender = sender.clone();
                let kill_init_tcp = kill.clone();
                *w_state = TcpState::None;
                drop(w_state);

                // Try to prevent the socket from being initialized.
                kill_init_tcp.awake();
                sender.close();
                let receiver = sender.subscribe();

                // If the socket still initialized, shut it down immediately.
                match receiver.await {
                    Ok((_, tcp_kill)) => {
                        tcp_kill.awake();
                    },
                    Err(_) => (), //< Successful cancellation
                }
            },
            TcpState::None => drop(w_state),    //< Already shut down
            TcpState::Blocked => drop(w_state), //< Already shut down
        }
    }

    #[inline]
    pub async fn enable_tcp(self: Arc<Self>) {
        println!("Enabling TCP socket {}", self.upstream_socket);

        let mut w_state = self.tcp.write().await;
        match &*w_state {
            TcpState::Managed { socket: _, kill: _ } => (),      //< Already enabled
            TcpState::Establishing { sender: _, kill: _ } => (), //< Already enabled
            TcpState::None => (),                                //< Already enabled
            TcpState::Blocked => *w_state = TcpState::None,
        }
        drop(w_state);
    }

    #[inline]
    pub async fn disable_tcp(self: Arc<Self>) {
        println!("Disabling TCP socket {}", self.upstream_socket);

        let mut w_state = self.tcp.write().await;
        match &*w_state {
            TcpState::Managed { socket: _, kill } => {
                let kill_tcp = kill.clone();
                *w_state = TcpState::Blocked;
                drop(w_state);

                kill_tcp.awake();

                // Note: this task is not responsible for actual cleanup. Once the listener closes, it
                // will kill any active queries and change the TcpState.
            },
            TcpState::Establishing { sender, kill }=> {
                let sender = sender.clone();
                let kill_init_tcp = kill.clone();
                *w_state = TcpState::Blocked;
                drop(w_state);

                // Try to prevent the socket from being initialized.
                kill_init_tcp.awake();
                sender.close();
                let receiver = sender.subscribe();

                // If the socket still initialized, shut it down immediately.
                match receiver.await {
                    Ok((_, kill_tcp)) => {
                        kill_tcp.awake();
                    },
                    Err(_) => (), //< Successful cancellation
                }
            },
            TcpState::None => {
                *w_state = TcpState::Blocked;
                drop(w_state)
            },
            TcpState::Blocked => drop(w_state), //< Already disabled
        }
    }

    #[inline]
    async fn listen_tcp(self: Arc<Self>, mut tcp_reader: OwnedReadHalf, kill_tcp: AwakeToken) {
        pin!(let kill_tcp_awoken = kill_tcp.awoken(););
        loop {
            select! {
                biased;
                () = &mut kill_tcp_awoken => {
                    println!("TCP Socket {} Canceled. Shutting down TCP Listener.", self.upstream_socket);
                    break;
                },
                () = tokio::time::sleep(TCP_LISTEN_TIMEOUT) => {
                    println!("TCP Socket {} Timed Out. Shutting down TCP Listener.", self.upstream_socket);
                    break;
                },
                response = read_tcp_message(&mut tcp_reader) => {
                    match response {
                        Ok(response) => {
                            self.recent_messages_received.store(true, Ordering::Release);
                            let response_id = response.id;
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

        self.listen_tcp_cleanup(kill_tcp).await;
    }

    #[inline]
    async fn listen_tcp_cleanup(self: Arc<Self>, kill_tcp: AwakeToken) {
        println!("Cleaning up TCP socket {}", self.upstream_socket);

        let mut w_state = self.tcp.write().await;
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
            TcpState::Establishing { sender: _, kill: _ } => drop(w_state), //< Not our socket to clean up
            TcpState::None => drop(w_state),               //< Not our socket to clean up
            TcpState::Blocked => drop(w_state),            //< Not our socket to clean up
        }
    }
}

enum UdpState {
    Managed(Arc<UdpSocket>, AwakeToken),
    None,
    Blocked,
}

#[pin_project(PinnedDrop)]
struct UdpQueryRunner<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h, 'i>
where
    'a: 'd + 'h
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
    inner: InnerUQ<'c, 'd, 'e, 'f, 'g, 'h>,
}

impl<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h, 'i> UdpQueryRunner<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h, 'i>
where
    'h: 'g
{
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

    fn reset_timeout(self: std::pin::Pin<&mut Self>, next_timeout: Duration) {
        let now = Instant::now();
        match now.checked_add(next_timeout) {
            Some(new_deadline) => self.project().timeout.reset(new_deadline),
            None => self.project().timeout.reset(now),
        }
    }
}

#[pin_project(project = InnerUQProj)]
enum InnerUQ<'c, 'd, 'e, 'f, 'g, 'h>
where
    'h: 'g
{
    Fresh { udp_retransmissions: u8 },
    Running {
        #[pin]
        socket: EitherSocket<'c, 'd, 'e>,
        #[pin]
        send_query: QSendQuery<'f, errors::SocketSendError>,
    },
    Cleanup(BoxFuture<'g, RwLockWriteGuard<'h, ActiveQueries>>, UdpResponseTime),
    Complete,
}

impl<'a, 'c, 'd, 'e, 'f, 'g, 'h> InnerUQ<'c, 'd, 'e, 'f, 'g, 'h>
where
    'a: 'd + 'h
{
    pub fn set_running_udp(mut self: std::pin::Pin<&mut Self>, udp_retransmissions: u8, query_type: Query) {
        self.set(Self::Running {
            socket: EitherSocket::Udp { uq_socket: UQSocket::Fresh, retransmits: udp_retransmissions },
            send_query: QSendQuery::Fresh(query_type),
        });
    }

    fn set_running_tcp(mut self: std::pin::Pin<&mut Self>, query_type: Query) {
        self.set(InnerUQ::Running {
            socket: EitherSocket::Tcp { tq_socket: TQSocket::Fresh },
            send_query: QSendQuery::Fresh(query_type),
        });
    }

    pub fn set_cleanup(mut self: std::pin::Pin<&mut Self>, execution_time: UdpResponseTime, socket: &'a Arc<MixedSocket>) {
        let w_active_queries = socket.active_queries.write().boxed();

        self.set(Self::Cleanup(w_active_queries, execution_time));
    }

    pub fn set_complete(mut self: std::pin::Pin<&mut Self>) {
        self.set(Self::Complete);
    }
}

impl<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h, 'i> Future for UdpQueryRunner<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h, 'i> {
    type Output = ();

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let mut this = self.as_mut().project();
        match this.inner.as_mut().project() {
            InnerUQProj::Fresh { udp_retransmissions: 0 } => {
                if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                    let new_timeout = **this.udp_timeout;
                    // If we run out of UDP retransmissions before the query has even begun,
                    // then it is time to transmit via TCP.
                    // Setting the socket state to TQSocket::Fresh will cause the socket to be
                    // initialized (if needed) and then a message sent over that socket.
                    this.inner.set_running_tcp(Query::Initial);
                    self.as_mut().reset_timeout(new_timeout);

                    // TODO
                }
            },
            InnerUQProj::Fresh { udp_retransmissions: udp_retransmissions @ 1.. } => {
                if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                    let new_timeout = **this.udp_retransmission_timeout;
                    // If we time out before the first query has begin, burn a retransmission.
                    *udp_retransmissions = udp_retransmissions.saturating_sub(1);
                    self.as_mut().reset_timeout(new_timeout);

                    // TODO
                }
            },
            InnerUQProj::Running { mut socket, mut send_query } => {
                match (send_query.as_mut().project(), socket.as_mut().project()) {
                    (QSendQueryProj::Fresh(_), EitherSocketProj::Udp { uq_socket: _, retransmits: 0 })
                  | (QSendQueryProj::SendQuery(_, _), EitherSocketProj::Udp { uq_socket: _, retransmits: 0 }) => {
                        if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                            let new_timeout = **this.udp_timeout;
                            // Setting the socket state to TQSocket::Fresh will cause the socket
                            // to be initialized (if needed) and then a message sent over that
                            // socket.
                            this.inner.set_running_tcp(Query::Retransmit);
                            self.as_mut().reset_timeout(new_timeout);

                            // TODO
                        }
                    },
                    (QSendQueryProj::Complete(_), EitherSocketProj::Udp { uq_socket: _, retransmits: 0 }) => {
                        if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                            let new_timeout = **this.udp_timeout;
                            // Setting the socket state to TQSocket::Fresh will cause the socket
                            // to be initialized (if needed) and then a message sent over that
                            // socket.
                            this.socket.add_dropped_packet_to_udp_average();
                            this.inner.set_running_tcp(Query::Retransmit);
                            self.as_mut().reset_timeout(new_timeout);

                            // TODO
                        }
                    },
                    (QSendQueryProj::Fresh(_), EitherSocketProj::Udp { uq_socket: _, retransmits: udp_retransmissions @ 1.. })
                  | (QSendQueryProj::SendQuery(_, _), EitherSocketProj::Udp { uq_socket: _, retransmits: udp_retransmissions @ 1.. }) => {
                        if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                            let new_timeout = **this.udp_retransmission_timeout;
                            // If we are currently sending a query or have not sent one yet,
                            // burn the retransmission.
                            *udp_retransmissions = udp_retransmissions.saturating_sub(1);
                            self.as_mut().reset_timeout(new_timeout);

                            // TODO
                        }
                    },
                    (QSendQueryProj::Complete(_), EitherSocketProj::Udp { uq_socket: _, retransmits: udp_retransmissions @ 1.. }) => {
                        if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                            let new_timeout = **this.udp_retransmission_timeout;
                            // A previous query has succeeded. Setting the state to Fresh will
                            // cause the state machine to send another query and drive it to
                            // Complete.
                            this.socket.add_dropped_packet_to_udp_average();
                            send_query.set_fresh(Query::Retransmit);
                            *udp_retransmissions = udp_retransmissions.saturating_sub(1);
                            self.as_mut().reset_timeout(new_timeout);
                            // TODO
                        }
                    },
                    (_, EitherSocketProj::Tcp { tq_socket: _ }) => {
                        if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
                            let _ = this.result_receiver.get_sender().send(Err(errors::QueryError::Timeout));

                            this.inner.set_cleanup(UdpResponseTime::Dropped, this.socket);

                            // TODO
                        }
                    },
                }
            },
            InnerUQProj::Cleanup(_, _)
          | InnerUQProj::Complete => {
                // Not allowed to timeout. This is a cleanup state.
            },
        }

        loop {
            let mut this = self.as_mut().project();
            match this.inner.as_mut().project() {
                InnerUQProj::Fresh { udp_retransmissions } => {
                    let retransmissions = *udp_retransmissions;

                    this.inner.set_running_udp(retransmissions, Query::Initial);

                    // Next loop: poll tq_socket and in_flight to start getting the TCP socket and
                    // inserting the query ID into the in-flight map.
                    continue;
                },
                InnerUQProj::Running { socket: mut q_socket, mut send_query } => {
                    match (send_query.as_mut().project(), q_socket.as_mut().project()) {
                        (QSendQueryProj::Fresh(query_type), EitherSocketProj::Udp { mut uq_socket, retransmits: _ }) => {
                            match query_type {
                                Query::Initial => (),
                                Query::Retransmit => match this.result_receiver.as_mut().poll(cx) {
                                    Poll::Ready(Ok(Ok(Message { id: _, qr: _, opcode: _, authoritative_answer: _, truncation: truncated, recursion_desired: _, recursion_available: _, z: _, rcode: _, question: _, answer: _, authority: _, additional: _ }))) => {
                                        let execution_time = this.udp_start_time.elapsed();

                                        this.inner.set_cleanup(UdpResponseTime::Responded { execution_time, truncated }, &this.socket);

                                        // Next loop will poll for the in-flight map lock to clean
                                        // up the query ID before returning the response.
                                        continue;
                                    },
                                    Poll::Ready(Ok(Err(_)))
                                  | Poll::Ready(Err(_)) => {
                                        this.inner.set_cleanup(UdpResponseTime::Dropped, &this.socket);

                                        // Next loop will poll for the in-flight map lock to clean
                                        // up the query ID before returning the response.
                                        continue;
                                    },
                                    Poll::Pending => (),
                                },
                            };

                            let uq_socket_result = match uq_socket.poll(this.socket, cx) {
                                PollSocket::Error(error) => {
                                    let _ = this.result_receiver.get_sender().send(Err(errors::QueryError::from(error)));

                                    this.inner.set_cleanup(UdpResponseTime::None, &this.socket);

                                    // Next loop will poll for the in-flight map lock to clean up
                                    // the query ID before returning the response.
                                    continue;
                                },
                                PollSocket::Continue => LoopPoll::Continue,
                                PollSocket::Pending => LoopPoll::Pending,
                            };

                            if let UQSocketProj::Acquired { udp_socket, kill_udp: _ } = uq_socket.as_mut().project() {
                                let mut raw_message = [0_u8; MAX_MESSAGE_SIZE as usize];
                                let mut write_wire = WriteWire::from_bytes(&mut raw_message);
                                if let Err(wire_error) = this.query.to_wire_format(&mut write_wire, &mut Some(CompressionMap::new())) {
                                    let _ = this.result_receiver.get_sender().send(Err(errors::QueryError::from(errors::UdpSendError::from(wire_error))));

                                    this.inner.set_cleanup(UdpResponseTime::None, &this.socket);

                                    // Next loop will poll for the in-flight map lock to clean up
                                    // the query ID before returning the response.
                                    continue;
                                };
                                let wire_length = write_wire.current_len();

                                let socket: Arc<MixedSocket> = this.socket.clone();
                                let udp_socket = udp_socket.clone();

                                println!("Sending on UDP socket {} {{ drop rate {:.2}%, truncation rate {:.2}%, response time {:.2} ms, timeout {} ms }} :: {:?}", this.socket.upstream_socket, this.socket.average_dropped_udp_packets() * 100.0, this.socket.average_truncated_udp_packets() * 100.0, this.socket.average_udp_response_time(), this.udp_retransmission_timeout.as_millis(), this.query);

                                let send_query_future = async move {
                                    let socket = socket;
                                    let udp_socket = udp_socket;
                                    let wire_length = wire_length;

                                    socket.recent_messages_sent.store(true, Ordering::Release);
                                    let bytes_written = match udp_socket.send(&raw_message[..wire_length]).await {
                                        Ok(bytes_written) => bytes_written,
                                        Err(error) => {
                                            return Err(errors::SocketSendError::from(errors::UdpSendError::from(error)));
                                        },
                                    };
                                    // Verify that the correct number of bytes were written.
                                    if bytes_written != wire_length {
                                        return Err(errors::SocketSendError::from(errors::UdpSendError::IncorrectNumberBytes { expected: wire_length as u16, sent: bytes_written }));
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
                                LoopPoll::Continue => continue,
                                LoopPoll::Pending => return Poll::Pending,
                            }
                        },
                        (QSendQueryProj::Fresh(query_type), EitherSocketProj::Tcp { mut tq_socket }) => {
                            match query_type {
                                Query::Initial => (),
                                Query::Retransmit => match this.result_receiver.as_mut().poll(cx) {
                                    Poll::Ready(Ok(Ok(Message { id: _, qr: _, opcode: _, authoritative_answer: _, truncation: truncated, recursion_desired: _, recursion_available: _, z: _, rcode: _, question: _, answer: _, authority: _, additional: _ }))) => {
                                        let execution_time = this.udp_start_time.elapsed();

                                        this.inner.set_cleanup(UdpResponseTime::Responded { execution_time, truncated }, &this.socket);

                                        // Next loop will poll for the in-flight map lock to clean
                                        // up the query ID before returning the response.
                                        continue;
                                    },
                                    Poll::Ready(Ok(Err(_)))
                                  | Poll::Ready(Err(_)) => {
                                        this.inner.set_cleanup(UdpResponseTime::Dropped, &this.socket);

                                        // Next loop will poll for the in-flight map lock to clean
                                        // up the query ID before returning the response.
                                        continue;
                                    },
                                    Poll::Pending => (),
                                },
                            };

                            let tq_socket_result = match tq_socket.poll(this.socket, cx) {
                                PollSocket::Error(error) => {
                                    let _ = this.result_receiver.get_sender().send(Err(errors::QueryError::from(error)));

                                    this.inner.set_cleanup(UdpResponseTime::None, &this.socket);

                                    // Next loop will poll for the in-flight map lock to clean up
                                    // the query ID before returning the response.
                                    continue;
                                },
                                PollSocket::Continue => LoopPoll::Continue,
                                PollSocket::Pending => LoopPoll::Pending,
                            };

                            if let TQSocketProj::Acquired { tcp_socket, kill_tcp: _ } = tq_socket.as_mut().project() {
                                let mut raw_message = [0_u8; MAX_MESSAGE_SIZE as usize];
                                let mut write_wire = WriteWire::from_bytes(&mut raw_message);
                                if let Err(wire_error) = this.query.to_wire_format_with_two_octet_length(&mut write_wire, &mut Some(CompressionMap::new())) {
                                    let _ = this.result_receiver.get_sender().send(Err(errors::QueryError::from(errors::TcpSendError::from(wire_error))));

                                    this.inner.set_cleanup(UdpResponseTime::None, &this.socket);

                                    // Next loop will poll for the in-flight map lock to clean up the
                                    // query ID before returning the response.
                                    continue;
                                };
                                let wire_length = write_wire.current_len();

                                let socket = this.socket.clone();
                                let tcp_socket = tcp_socket.clone();

                                println!("Sending on TCP socket {} {{ drop rate {:.2}%, truncation rate {:.2}%, response time {:.2} ms, timeout {} ms }} :: {:?}", this.socket.upstream_socket, this.socket.average_dropped_tcp_packets() * 100.0, this.socket.average_truncated_udp_packets() * 100.0, this.socket.average_tcp_response_time(), this.udp_timeout.as_millis(), this.query);

                                let send_query_future = async move {
                                    let socket = socket;
                                    let tcp_socket = tcp_socket;
                                    let wire_length = wire_length;

                                    socket.recent_messages_sent.store(true, Ordering::Release);
                                    let mut w_tcp_stream = tcp_socket.lock().await;
                                    let bytes_written = match w_tcp_stream.write(&raw_message[..wire_length]).await {
                                        Ok(bytes_written) => bytes_written,
                                        Err(error) => {
                                            return Err(errors::SocketSendError::from(errors::TcpSendError::from(error)));
                                        },
                                    };
                                    drop(w_tcp_stream);
                                    // Verify that the correct number of bytes were written.
                                    if bytes_written != wire_length {
                                        return Err(errors::SocketSendError::from(errors::TcpSendError::IncorrectNumberBytes { expected: wire_length as u16, sent: bytes_written }));
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
                        (QSendQueryProj::SendQuery(query_type, send_query_future), _) => {
                            match query_type {
                                Query::Initial => (),
                                Query::Retransmit => match this.result_receiver.as_mut().poll(cx) {
                                    Poll::Ready(Ok(Ok(Message { id: _, qr: _, opcode: _, authoritative_answer: _, truncation: truncated, recursion_desired: _, recursion_available: _, z: _, rcode: _, question: _, answer: _, authority: _, additional: _ }))) => {
                                        let execution_time = this.udp_start_time.elapsed();

                                        this.inner.set_cleanup(UdpResponseTime::Responded { execution_time, truncated }, &this.socket);

                                        // Next loop will poll for the in-flight map lock to clean
                                        // up the query ID before returning the response.
                                        continue;
                                    },
                                    Poll::Ready(Ok(Err(_)))
                                  | Poll::Ready(Err(_)) => {
                                        this.inner.set_cleanup(UdpResponseTime::Dropped, &this.socket);

                                        // Next loop will poll for the in-flight map lock to clean
                                        // up the query ID before returning the response.
                                        continue;
                                    },
                                    Poll::Pending => (),
                                },
                            };

                            let q_socket_result = match q_socket.poll(this.socket, cx) {
                                PollSocket::Error(error) => {
                                    let _ = this.result_receiver.get_sender().send(Err(errors::QueryError::from(error)));

                                    this.inner.set_cleanup(UdpResponseTime::None, &this.socket);

                                    // Next loop will poll for the in-flight map lock to clean up
                                    // the query ID before returning the response.
                                    continue;
                                },
                                PollSocket::Continue => LoopPoll::Continue,
                                PollSocket::Pending => LoopPoll::Pending,
                            };

                            match send_query_future.as_mut().poll(cx) {
                                Poll::Ready(Err(error)) => {
                                    let _ = this.result_receiver.get_sender().send(Err(errors::QueryError::from(error)));

                                    this.inner.set_cleanup(UdpResponseTime::None, &this.socket);

                                    // Next loop will poll for the in-flight map lock to clean up
                                    // the query ID before returning the response.
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
                        (QSendQueryProj::Complete(_), _) => {
                            match this.result_receiver.as_mut().poll(cx) {
                                Poll::Ready(Ok(Ok(Message { id: _, qr: _, opcode: _, authoritative_answer: _, truncation: truncated, recursion_desired: _, recursion_available: _, z: _, rcode: _, question: _, answer: _, authority: _, additional: _ }))) => {
                                    let execution_time = this.udp_start_time.elapsed();

                                    this.inner.set_cleanup(UdpResponseTime::Responded { execution_time, truncated }, &this.socket);

                                    // TODO
                                    continue;
                                },
                                Poll::Ready(Ok(Err(_))) => {
                                    this.inner.set_cleanup(UdpResponseTime::Dropped, &this.socket);

                                    // TODO
                                    continue;
                                },
                                Poll::Ready(Err(_)) => {
                                    this.inner.set_cleanup(UdpResponseTime::Dropped, &this.socket);

                                    // TODO
                                    continue;
                                },
                                Poll::Pending => (),
                            }

                            match q_socket.poll(this.socket, cx) {
                                PollSocket::Error(error) => {
                                    let _ = this.result_receiver.get_sender().send(Err(errors::QueryError::from(error)));

                                    this.inner.set_cleanup(UdpResponseTime::None, &this.socket);

                                    // Next loop will poll for the in-flight map lock to clean up
                                    // the query ID before returning the response.
                                    continue;
                                },
                                PollSocket::Continue => continue,
                                PollSocket::Pending => return Poll::Pending,
                            };
                        },
                    }
                },
                InnerUQProj::Cleanup(w_active_queries, execution_time) => {
                    this.result_receiver.close();

                    match w_active_queries.as_mut().poll(cx) {
                        Poll::Ready(mut w_active_queries) => {
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
                        Poll::Pending => {
                            // TODO
                            return Poll::Pending;
                        },
                    }
                },
                InnerUQProj::Complete => {
                    panic!("UDP query polled after completion");
                },
            }
        }
    }
}

#[pinned_drop]
impl<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h, 'i> PinnedDrop for UdpQueryRunner<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h, 'i> {
    fn drop(mut self: Pin<&mut Self>) {
        async fn cleanup(socket: Arc<MixedSocket>, query: Message) {
            let mut w_active_queries = socket.active_queries.write().await;
            let _ = w_active_queries.in_flight.remove(&query.id);
            let _ = w_active_queries.tcp_or_udp.remove(&query.question);
            drop(w_active_queries);
        }

        match self.as_mut().project().inner.as_mut().project() {
            InnerUQProj::Fresh { udp_retransmissions: _ }
          | InnerUQProj::Running { socket: _, send_query: _ }
          | InnerUQProj::Cleanup(_, _) => {
                let socket = self.socket.clone();
                let query = self.query.clone();
                tokio::spawn(cleanup(socket, query));
            },
            InnerUQProj::Complete => {
                // Nothing to do for active queries.
            }
        }
    }
}

#[pin_project]
struct UdpQuery<'a, 'b, 'c, 'd>
where
    'a: 'd
{
    socket: &'a Arc<MixedSocket>,
    query: &'b mut Message,
    #[pin]
    inner: QInitQuery<'c, 'd>,
}

impl<'a, 'b, 'c, 'd> UdpQuery<'a, 'b, 'c, 'd> {
    pub fn new(socket: &'a Arc<MixedSocket>, query: &'b mut Message) -> Self {
        Self {
            socket,
            query,
            inner: QInitQuery::Fresh,
        }
    }
}

impl<'a, 'b, 'c, 'd> Future for UdpQuery<'a, 'b, 'c, 'd> {
    type Output = Result<Message, errors::QueryError>;

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        loop {
            let mut this = self.as_mut().project();
            match this.inner.as_mut().project() {
                QInitQueryProj::Fresh => {
                    this.inner.set_read_active_query(this.socket);

                    // TODO
                    continue;
                },
                QInitQueryProj::ReadActiveQuery(r_active_queries) => {
                    match r_active_queries.as_mut().poll(cx) {
                        Poll::Ready(r_active_queries) => {
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
                                    // println!("{} Following(1) active query '{}'", this.socket.upstream_socket, this.query.question);

                                    // TODO
                                    continue;
                                },
                                (None, None) => {
                                    drop(r_active_queries);
                                    this.inner.set_write_active_query(this.socket);

                                    // TODO
                                    continue;                
                                },
                            }
                        },
                        Poll::Pending => {
                            // TODO
                            return Poll::Pending;
                        },
                    }
                },
                QInitQueryProj::WriteActiveQuery(w_active_queries) => {
                    match w_active_queries.as_mut().poll(cx) {
                        Poll::Ready(mut w_active_queries) => {
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
                                    // println!("{} Following(2) active query '{}'", this.socket.upstream_socket, this.query.question);

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
                                    // println!("{} Following(3) active query '{}'", this.socket.upstream_socket, this.query.question);

                                    // TODO
                                    continue;                
                                },
                            }
                        },
                        Poll::Pending => {
                            // TODO
                            return Poll::Pending;
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
                            let error = errors::QueryError::from(errors::UdpSocketError::Shutdown);

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
impl MixedSocket {
    #[inline]
    async fn init_udp(self: Arc<Self>) -> Result<(Arc<UdpSocket>, AwakeToken), errors::UdpInitError> {
        // Initially, verify if the connection has already been established.
        let r_state = self.udp.read().await;
        match &*r_state {
            UdpState::Managed(udp_socket, kill_udp) => return Ok((udp_socket.clone(), kill_udp.clone())),
            UdpState::None => (),
            UdpState::Blocked => {
                drop(r_state);
                return Err(errors::UdpInitError::SocketDisabled);
            },
        }
        drop(r_state);

        let udp_socket = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);
        udp_socket.connect(self.upstream_socket).await?;
        let udp_reader = udp_socket.clone();
        let udp_writer = udp_socket;
        let kill_udp = AwakeToken::new();

        // Since there is no intermediate state while the UDP socket is being
        // set up and the lock is dropped, it is possible that another process
        // was doing the same task.

        let mut w_state = self.udp.write().await;
        match &*w_state {
            UdpState::Managed(existing_udp_socket, _) => {
                return Ok((existing_udp_socket.clone(), kill_udp));
            },
            UdpState::None => {
                *w_state = UdpState::Managed(udp_writer.clone(), kill_udp.clone());
                drop(w_state);

                task::spawn(self.listen_udp(udp_reader, kill_udp.clone()));

                return Ok((udp_writer, kill_udp));
            },
            UdpState::Blocked => {
                drop(w_state);
                return Err(errors::UdpInitError::SocketDisabled);
            },
        }
    }

    #[inline]
    pub async fn start_udp(self: Arc<Self>) -> Result<(), errors::UdpInitError> {
        match self.init_udp().await {
            Ok(_) => Ok(()),
            Err(error) => Err(error),
        }
    }

    #[inline]
    pub async fn shutdown_udp(self: Arc<Self>) {
        println!("Shutting down UDP socket {}", self.upstream_socket);

        let r_state = self.udp.read().await;
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

    #[inline]
    pub async fn enable_udp(self: Arc<Self>) {
        println!("Enabling UDP socket {}", self.upstream_socket);

        let mut w_state = self.udp.write().await;
        match &*w_state {
            UdpState::Managed(_, _) => (),  //< Already enabled
            UdpState::None => (),           //< Already enabled
            UdpState::Blocked => *w_state = UdpState::None,
        }
        drop(w_state);
    }

    #[inline]
    pub async fn disable_udp(self: Arc<Self>) {
        println!("Disabling UDP socket {}", self.upstream_socket);

        let mut w_state = self.udp.write().await;
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

    #[inline]
    async fn listen_udp(self: Arc<Self>, udp_reader: Arc<UdpSocket>, kill_udp: AwakeToken) {
        pin!(let kill_udp_awoken = kill_udp.awoken(););
        loop {
            select! {
                biased;
                () = &mut kill_udp_awoken => {
                    println!("UDP Socket {} Canceled. Shutting down UDP Listener.", self.upstream_socket);
                    break;
                },
                () = tokio::time::sleep(UDP_LISTEN_TIMEOUT) => {
                    println!("UDP Socket {} Timed Out. Shutting down UDP Listener.", self.upstream_socket);
                    break;
                },
                response = read_udp_message(&udp_reader) => {
                    match response {
                        Ok(response) => {
                            // Note: if truncation flag is set, that will be dealt with by the caller.
                            self.recent_messages_received.store(true, Ordering::Release);
                            let response_id = response.id;
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

        self.listen_udp_cleanup(kill_udp).await;
    }

    #[inline]
    async fn listen_udp_cleanup(self: Arc<Self>,  kill_udp: AwakeToken) {
        println!("Cleaning up UDP socket {}", self.upstream_socket);

        let mut w_state = self.udp.write().await;
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
            UdpState::None => (),
            UdpState::Blocked => (),
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

#[repr(C)]
#[derive(Debug, Clone, Copy, Zeroable, Pod)]
struct RollingAverage {
    total: u32,
    count: u8,
    // Padding out to 8 bytes is required for bytemuck Zeroable and Pod.
    pad1: u8,
    pad2: u16,
}

impl RollingAverage {
    pub fn new() -> Self {
        Self {
            total: 0,
            count: 0,
            pad1: 0,
            pad2: 0,
        }
    }

    pub fn put_next(mut self, value: u32, max_count: NonZeroU8) -> Self {
        if self.count < max_count.into() {
            if let Some(total) = self.total.checked_add(value) {
                self.total = total;
                self.count += 1;
                return self;
            }
        }

        // Don't need to use a check_div since max_count is guaranteed to be non-zero, so if the
        // count were zero, the first case would have run instead.
        let average = u64::from(self.total.div(u32::from(self.count)));
        let value = u64::from(value);
        // Since we up-casted from u32, there is no way for the addition to fail. No need to use a
        // checked add.
        let mut total = u64::from(self.total).add(value);

        // If we have overshot the maximum count, then subtract the average one an extra time.
        if self.count > max_count.into() {
            total = total.saturating_sub(average);
            // self.count will never drop below 1 because max_count is non-zero. If max_count were 0
            // or 1, this conditional wouldn't be run.
            self.count -= 1;
        }

        match u32::try_from(total.saturating_sub(average)) {
            Ok(total) => {
                self.total = total;
                return self;
            },
            Err(_) => {
                self.total = u32::MAX;
                return self;
            },
        }
    }

    pub fn current_average(&self) -> f64 {
        f64::from(self.total).div(f64::from(self.count))
    }
}

/// Similar to `Atomic::fetch_update()` except...
/// 1. it returns the updated value, not the previous value.
/// 2. the input function returns `T`, not `Option<T>`.
/// 3. the return value is never an `Err(T)`.
/// This allows it to work better when updating `RollingAverage`.
pub fn fetch_update<T, F>(atomic: &Atomic<T>, success: Ordering, failure: Ordering, f: F) -> T
where
    T: NoUninit + Clone,
    F: Fn(T) -> T
{
    let mut prev = atomic.load(Ordering::Relaxed);
    loop {
        let next = f(prev);
        match atomic.compare_exchange_weak(prev, next.clone(), success, failure) {
            Ok(_) => return next,
            Err(next_prev) => prev = next_prev,
        }
    }
}

pub struct MixedSocket {
    upstream_socket: SocketAddr,
    tcp: RwLock<TcpState>,
    udp: RwLock<UdpState>,
    active_queries: RwLock<ActiveQueries>,

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
    pub fn new(upstream_socket: SocketAddr) -> Arc<Self> {
        Arc::new(MixedSocket {
            upstream_socket,
            tcp: RwLock::new(TcpState::None),
            udp: RwLock::new(UdpState::None),
            active_queries: RwLock::new(ActiveQueries::new()),

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
    pub fn socket_address(&self) -> &SocketAddr {
        &self.upstream_socket
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
    pub async fn start_both(self: Arc<Self>) -> Result<(), errors::SocketInitError> {
        match join!(
            self.clone().start_udp(),
            self.start_tcp()
        ) {
            (Ok(()), Ok(())) => Ok(()),
            (Ok(()), Err(tcp_error)) => Err(errors::SocketInitError::from(tcp_error)),
            (Err(udp_error), Ok(())) => Err(errors::SocketInitError::from(udp_error)),
            (Err(udp_error), Err(tcp_error)) => Err(errors::SocketInitError::from((udp_error, tcp_error))),
        }
    }

    #[inline]
    pub async fn shutdown_both(self: Arc<Self>) {
        join!(
            self.clone().shutdown_udp(),
            self.shutdown_tcp()
        );
    }

    #[inline]
    pub async fn enable_both(self: Arc<Self>) {
        join!(
            self.clone().enable_udp(),
            self.enable_tcp()
        );
    }

    #[inline]
    pub async fn disable_both(self: Arc<Self>) {
        join!(
            self.clone().disable_udp(),
            self.disable_tcp()
        );
    }

    pub fn query<'a, 'b, 'c, 'd>(self: &'a Arc<Self>, query: &'b mut Message, options: QueryOptions) -> MixedQuery<'a, 'b, 'c, 'd> {
        // If the UDP socket is unreliable, send most data via TCP. Some queries should still use
        // UDP to determine if the network conditions are improving. However, if the TCP connection
        // is also unstable, then we should not rely on it.
        let query_task = match options {
            QueryOptions::Both => {
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
            QueryOptions::TcpOnly => {
                MixedQuery::Tcp(TcpQuery::new(&self, query))
            },
        };

        return query_task;
    }
}

#[inline]
async fn read_udp_message(udp_socket: &Arc<UdpSocket>) -> Result<Message, errors::UdpReceiveError> {
    // Step 1: Setup buffer. Make sure it is within the configured size.
    let mut buffer = [0; MAX_MESSAGE_SIZE as usize];
    let mut buffer = &mut buffer[..MAX_MESSAGE_SIZE as usize];

    // Step 2: Get the bytes from the UDP socket.
    let received_byte_count = udp_socket.recv(&mut buffer).await?;

    // Step 3: Deserialize the Message received on UDP socket.
    let mut wire = ReadWire::from_bytes(&buffer[..received_byte_count]);
    let message = Message::from_wire_format(&mut wire)?;

    // eprintln!("(\"{}\", \"{}\", \"{}\"),", message.question[0].qtype(), message.question[0].qname(), Base64::from_bytes(&buffer[..received_byte_count]));
    println!("Received UDP Response: {message:?}");

    return Ok(message);
}

#[inline]
async fn read_tcp_message(tcp_stream: &mut OwnedReadHalf) -> Result<Message, errors::TcpReceiveError> {
    // Step 1: Deserialize the u16 representing the size of the rest of the data. This is the first
    //         2 bytes of data.
    let mut wire_size = [0, 0];
    let bytes_read = tcp_stream.read_exact(&mut wire_size).await?;
    if bytes_read != 2 {
        return Err(errors::TcpReceiveError::IncorrectNumberBytes { expected: 2, received: bytes_read });
    }
    let expected_message_size = u16::from_be_bytes(wire_size);
    if expected_message_size > MAX_MESSAGE_SIZE {
        return Err(errors::TcpReceiveError::IncorrectLengthByte { limit: MAX_MESSAGE_SIZE, received: expected_message_size });
    }

    // Step 2: Read the rest of the packet.
    // Note: It MUST be the size of the previous u16 (expected_message_size).
    let mut tcp_buffer = [0; MAX_MESSAGE_SIZE as usize];
    let tcp_buffer = &mut tcp_buffer[..MAX_MESSAGE_SIZE as usize];
    let bytes_read = tcp_stream.read_exact(&mut tcp_buffer[..expected_message_size as usize]).await?;
    if bytes_read != (expected_message_size as usize) {
        return Err(errors::TcpReceiveError::IncorrectNumberBytes { expected: expected_message_size, received: bytes_read });
    }

    // Step 3: Deserialize the Message from the buffer.
    let mut wire = ReadWire::from_bytes(&mut tcp_buffer[..expected_message_size as usize]);
    let message = Message::from_wire_format(&mut wire)?;

    // if let Some(question) = message.question.first() {
    //     eprintln!("(\"{}\", \"{}\", \"{}\"),", question.qtype(), question.qname(), Base64::from_bytes(&tcp_buffer[..bytes_read]));
    // }
    println!("Received TCP Response: {message:?}");

    return Ok(message);
}

#[cfg(test)]
mod mixed_udp_tcp_tests {
    use std::{net::{IpAddr, Ipv4Addr, SocketAddr}, time::Duration};

    use dns_lib::{query::{message::Message, qr::QR, question::Question}, resource_record::{opcode::OpCode, rclass::RClass, rcode::RCode, resource_record::{RRHeader, ResourceRecord}, rtype::RType, time::Time, types::a::A}, serde::wire::{from_wire::FromWire, read_wire::ReadWire, to_wire::ToWire}, types::c_domain_name::CDomainName};
    use tinyvec::TinyVec;
    use tokio::{io::AsyncReadExt, select};
    use ux::u3;

    use crate::mixed_tcp_udp::{MixedSocket, QueryOptions};

    const LISTEN_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 65000);
    const SEND_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 65000);

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
        let answer = ResourceRecord::A(
            RRHeader::new(example_domain, example_class, Time::from_secs(3600)),
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
            answer: vec![answer],
            authority: vec![],
            additional: vec![],
        };

        let mixed_socket = MixedSocket::new(SEND_ADDR);

        // Test: Start Query
        let query_task = tokio::spawn({
            let mixed_socket = mixed_socket.clone();
            let mut query = query.clone();
            async move { mixed_socket.query(&mut query, QueryOptions::Both).await }
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
        mixed_socket.disable_both().await;
    }
}

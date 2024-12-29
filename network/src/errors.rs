use std::{error::Error, fmt::Display, io};

use dns_lib::serde::wire::{read_wire::ReadWireError, write_wire::WriteWireError};
use tokio::task::JoinError;


#[derive(Debug, Clone, Hash, PartialEq, Eq)]
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

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
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

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
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

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
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

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum StreamReceiveError {
    IncorrectNumberBytes {
        stream_protocol: &'static str,
        expected: u16,
        received: usize,
    },
    IncorrectLengthByte {
        stream_protocol: &'static str,
        limit: u16,
        received: u16,
    },
    Deserialization {
        stream_protocol: &'static str,
        error: ReadWireError
    },
    Io {
        stream_protocol: &'static str,
        error: IoError
    },
}
impl Display for StreamReceiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::IncorrectNumberBytes { stream_protocol, expected, received } => write!(f, "expected to receive {expected} bytes but received {received} on {stream_protocol} socket"),
            Self::IncorrectLengthByte { stream_protocol, limit, received } => write!(f, "expected to receive at most {limit} bytes but the length byte is {received} on {stream_protocol} socket"),
            Self::Deserialization{ stream_protocol, error } => write!(f, "{error} when receiving on {stream_protocol} socket"),
            Self::Io{ stream_protocol, error } => write!(f, "{error} when receiving on {stream_protocol} socket"),
        }
    }
}
impl Error for StreamReceiveError {}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
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

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
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

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
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

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
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

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
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

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
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

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
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

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
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

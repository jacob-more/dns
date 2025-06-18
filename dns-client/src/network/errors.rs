use std::{error::Error, fmt::Display, io};

use dns_lib::serde::wire::{read_wire::ReadWireError, write_wire::WriteWireError};
use rustls::pki_types::InvalidDnsNameError;
use tokio::task::JoinError;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SocketType {
    Udp,
    Tcp,
    Tls,
    Quic,
}
impl Display for SocketType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Udp => write!(f, "UDP"),
            Self::Tcp => write!(f, "TCP"),
            Self::Tls => write!(f, "TLS"),
            Self::Quic => write!(f, "QUIC"),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SocketStage {
    Initialization,
    Connected,
}
impl Display for SocketStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Initialization => write!(f, "Initialization"),
            Self::Connected => write!(f, "Connected"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryError {
    Socket(SocketError),
    Send(SendError),
    Receive(ReceiveError),
    Timeout,
}
impl Display for QueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::Socket(error) => write!(f, "{error}"),
            Self::Send(error) => write!(f, "{error}"),
            Self::Receive(error) => write!(f, "{error}"),
            Self::Timeout => write!(f, "timeout during query"),
        }
    }
}
impl Error for QueryError {}
impl From<SocketError> for QueryError {
    fn from(error: SocketError) -> Self {
        Self::Socket(error)
    }
}
impl From<SendError> for QueryError {
    fn from(error: SendError) -> Self {
        Self::Send(error)
    }
}
impl From<ReceiveError> for QueryError {
    fn from(error: ReceiveError) -> Self {
        Self::Receive(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendError {
    Serialization(WriteWireError),
    IncorrectNumberBytes {
        socket_type: SocketType,
        expected: u16,
        sent: usize,
    },
    Io {
        socket_type: SocketType,
        error: IoError,
    },
    QuicWriteError(quinn::WriteError),
}
impl Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::Serialization(write_wire_error) => write!(f, "{write_wire_error}"),
            Self::IncorrectNumberBytes {
                socket_type,
                expected,
                sent,
            } => write!(
                f,
                "expected to send {expected} bytes but sent {sent} on {socket_type} socket"
            ),
            Self::Io { socket_type, error } => {
                write!(f, "{error} when sending on {socket_type} socket")
            }
            Self::QuicWriteError(error) => write!(f, "{error} when sending on QUIC socket"),
        }
    }
}
impl Error for SendError {}
impl From<WriteWireError> for SendError {
    fn from(error: WriteWireError) -> Self {
        Self::Serialization(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiveError {
    IncorrectNumberBytes {
        protocol: SocketType,
        expected: u16,
        received: usize,
    },
    IncorrectLengthByte {
        protocol: SocketType,
        limit: u16,
        received: u16,
    },
    Deserialization {
        protocol: SocketType,
        error: ReadWireError,
    },
    Io {
        protocol: SocketType,
        error: IoError,
    },
}
impl Display for ReceiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::IncorrectNumberBytes {
                protocol,
                expected,
                received,
            } => write!(
                f,
                "expected to receive {expected} bytes but received {received} on {protocol} socket"
            ),
            Self::IncorrectLengthByte {
                protocol,
                limit,
                received,
            } => write!(
                f,
                "expected to receive at most {limit} bytes but the length byte is {received} on {protocol} socket"
            ),
            Self::Deserialization { protocol, error } => {
                write!(f, "{error} when receiving on {protocol} socket")
            }
            Self::Io { protocol, error } => {
                write!(f, "{error} when receiving on {protocol} socket")
            }
        }
    }
}
impl Error for ReceiveError {}

#[derive(Debug)]
pub enum SocketError {
    Disabled(SocketType, SocketStage),
    Shutdown(SocketType, SocketStage),
    Timeout(SocketType, SocketStage),
    JoinErrorPanic(SocketType, SocketStage),
    JoinErrorCancelled(SocketType, SocketStage),
    InvalidName {
        socket_type: SocketType,
        socket_stage: SocketStage,
        error: InvalidDnsNameError,
    },
    Io {
        socket_type: SocketType,
        socket_stage: SocketStage,
        error: IoError,
    },
    QuicConnection {
        socket_stage: SocketStage,
        error: quinn::ConnectionError,
    },
    QuicConnect {
        socket_stage: SocketStage,
        error: quinn::ConnectError,
    },
    Multiple(Vec<SocketError>),
}
impl Clone for SocketError {
    fn clone(&self) -> Self {
        match self {
            Self::Disabled(socket_type, socket_stage) => {
                Self::Disabled(*socket_type, *socket_stage)
            }
            Self::Shutdown(socket_type, socket_stage) => {
                Self::Shutdown(*socket_type, *socket_stage)
            }
            Self::Timeout(socket_type, socket_stage) => Self::Timeout(*socket_type, *socket_stage),
            Self::JoinErrorPanic(socket_type, socket_stage) => {
                Self::JoinErrorPanic(*socket_type, *socket_stage)
            }
            Self::JoinErrorCancelled(socket_type, socket_stage) => {
                Self::JoinErrorCancelled(*socket_type, *socket_stage)
            }
            Self::InvalidName {
                socket_type,
                socket_stage,
                error: _,
            } => Self::InvalidName {
                socket_type: *socket_type,
                socket_stage: *socket_stage,
                error: InvalidDnsNameError,
            },
            Self::Io {
                socket_type,
                socket_stage,
                error,
            } => Self::Io {
                socket_type: *socket_type,
                socket_stage: *socket_stage,
                error: error.clone(),
            },
            Self::QuicConnection {
                socket_stage,
                error,
            } => Self::QuicConnection {
                socket_stage: *socket_stage,
                error: error.clone(),
            },
            Self::QuicConnect {
                socket_stage,
                error,
            } => Self::QuicConnect {
                socket_stage: *socket_stage,
                error: error.clone(),
            },
            Self::Multiple(errors) => Self::Multiple(errors.clone()),
        }
    }
}
impl PartialEq for SocketError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Disabled(self_socket_type, self_socket_stage),
                Self::Disabled(other_socket_type, other_socket_stage),
            )
            | (
                Self::Shutdown(self_socket_type, self_socket_stage),
                Self::Shutdown(other_socket_type, other_socket_stage),
            )
            | (
                Self::Timeout(self_socket_type, self_socket_stage),
                Self::Timeout(other_socket_type, other_socket_stage),
            )
            | (
                Self::JoinErrorPanic(self_socket_type, self_socket_stage),
                Self::JoinErrorPanic(other_socket_type, other_socket_stage),
            )
            | (
                Self::JoinErrorCancelled(self_socket_type, self_socket_stage),
                Self::JoinErrorCancelled(other_socket_type, other_socket_stage),
            )
            | (
                Self::InvalidName {
                    socket_type: self_socket_type,
                    socket_stage: self_socket_stage,
                    error: _,
                },
                Self::InvalidName {
                    socket_type: other_socket_type,
                    socket_stage: other_socket_stage,
                    error: _,
                },
            ) => {
                (self_socket_type == other_socket_type) && (self_socket_stage == other_socket_stage)
            }
            (
                Self::Io {
                    socket_type: self_socket_type,
                    socket_stage: self_socket_stage,
                    error: self_error,
                },
                Self::Io {
                    socket_type: other_socket_type,
                    socket_stage: other_socket_stage,
                    error: other_error,
                },
            ) => {
                (self_socket_type == other_socket_type)
                    && (self_socket_stage == other_socket_stage)
                    && (self_error == other_error)
            }
            (
                Self::QuicConnection {
                    socket_stage: self_socket_stage,
                    error: self_error,
                },
                Self::QuicConnection {
                    socket_stage: other_socket_stage,
                    error: other_error,
                },
            ) => (self_socket_stage == other_socket_stage) && (self_error == other_error),
            (
                Self::QuicConnect {
                    socket_stage: self_socket_stage,
                    error: self_error,
                },
                Self::QuicConnect {
                    socket_stage: other_socket_stage,
                    error: other_error,
                },
            ) => (self_socket_stage == other_socket_stage) && (self_error == other_error),
            (Self::Multiple(self_errors), Self::Multiple(other_errors)) => {
                self_errors == other_errors
            }
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
    }
}
impl Eq for SocketError {}
impl Display for SocketError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::Disabled(socket_type, socket_stage) => {
                write!(f, "{socket_type} socket is disabled during {socket_stage}")
            }
            Self::Shutdown(socket_type, socket_stage) => {
                write!(f, "{socket_type} socket is disabled during {socket_stage}")
            }
            Self::Timeout(socket_type, socket_stage) => {
                write!(f, "{socket_type} socket timed out during {socket_stage}")
            }
            Self::JoinErrorPanic(socket_type, socket_stage) => write!(
                f,
                "{socket_type} socket task panicked during {socket_stage}"
            ),
            Self::JoinErrorCancelled(socket_type, socket_stage) => write!(
                f,
                "{socket_type} socket task cancelled during {socket_stage}"
            ),
            Self::InvalidName {
                socket_type,
                socket_stage,
                error,
            } => write!(f, "{error} during {socket_type} {socket_stage}"),
            Self::Io {
                socket_type,
                socket_stage,
                error,
            } => write!(f, "{error} during {socket_type} {socket_stage}"),
            Self::QuicConnection {
                socket_stage,
                error,
            } => write!(f, "{error} during QUIC {socket_stage}"),
            Self::QuicConnect {
                socket_stage,
                error,
            } => write!(f, "{error} during QUIC {socket_stage}"),
            Self::Multiple(errors) => {
                let mut errors_iter = errors.iter();
                if let Some(error) = errors_iter.next() {
                    write!(f, "{error}")?;
                }
                for error in errors_iter {
                    write!(f, "and {error}")?;
                }
                Ok(())
            }
        }
    }
}
impl Error for SocketError {}
impl From<(SocketType, SocketStage, JoinError)> for SocketError {
    fn from(error: (SocketType, SocketStage, JoinError)) -> Self {
        if error.2.is_cancelled() {
            Self::JoinErrorCancelled(error.0, error.1)
        } else {
            Self::JoinErrorPanic(error.0, error.1)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
        if error.raw_os_error().is_some() {
            Self::OsError(error.kind())
        } else {
            Self::Message(error.kind())
        }
    }
}

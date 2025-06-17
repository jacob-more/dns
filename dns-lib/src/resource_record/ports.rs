use std::{error::Error, fmt::Display};

use super::protocol::Protocol;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum PortError {
    UnknownMnemonic(String, Protocol),
}
impl Error for PortError {}
impl Display for PortError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownMnemonic(service, protocol) => write!(
                f,
                "Unknown Service Mnemonic '{service}' for protocol '{protocol}'"
            ),
        }
    }
}

use std::{error::Error, fmt::Display};

use crate::types::{
    ascii::AsciiError, base16::Base16Error, base32::Base32Error, base64::Base64Error,
    c_domain_name::CDomainNameError, domain_name::DomainNameError,
    extended_base32::ExtendedBase32Error,
};

use super::read_wire::ReadWire;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum WriteWireError {
    FormatError(String),
    OverflowError(String),
    UnderflowError(String),
    OutOfBoundsError(String),
    ValueError(String),
    VersionError(String),
    CDomainNameError(CDomainNameError),
    DomainNameError(DomainNameError),
    AsciiError(AsciiError),
    Base16Error(Base16Error),
    Base32Error(Base32Error),
    ExtendedBase32Error(ExtendedBase32Error),
    Base64Error(Base64Error),
}
impl Error for WriteWireError {}
impl Display for WriteWireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FormatError(error) => write!(f, "Write Wire Format Error: {error}"),
            Self::OverflowError(error) => write!(f, "Write Wire Overflow Error: {error}"),
            Self::UnderflowError(error) => write!(f, "Write Wire Underflow Error: {error}"),
            Self::OutOfBoundsError(error) => write!(f, "Write Wire Out Of Bounds Error: {error}"),
            Self::ValueError(error) => write!(f, "Write Wire Value Error: {error}"),
            Self::VersionError(error) => write!(f, "Write Wire Version Error: {error}"),

            Self::CDomainNameError(error) => {
                write!(f, "Write Wire Compressible Domain Name Error: {error}")
            }
            Self::DomainNameError(error) => {
                write!(f, "Write Wire Incompressible Domain Name Error: {error}")
            }
            Self::AsciiError(error) => write!(f, "Write Wire Ascii Error: {error}"),

            Self::Base16Error(error) => write!(f, "Write Wire Base16 Error: {error}"),
            Self::Base32Error(error) => write!(f, "Write Wire Bsse32 Error: {error}"),
            Self::ExtendedBase32Error(error) => {
                write!(f, "Write Wire Extended Base32 Error: {error}")
            }
            Self::Base64Error(error) => write!(f, "Write Wire Base64 Error: {error}"),
        }
    }
}
impl From<CDomainNameError> for WriteWireError {
    fn from(value: CDomainNameError) -> Self {
        WriteWireError::CDomainNameError(value)
    }
}
impl From<DomainNameError> for WriteWireError {
    fn from(value: DomainNameError) -> Self {
        WriteWireError::DomainNameError(value)
    }
}
impl From<AsciiError> for WriteWireError {
    fn from(value: AsciiError) -> Self {
        WriteWireError::AsciiError(value)
    }
}
impl From<Base16Error> for WriteWireError {
    fn from(value: Base16Error) -> Self {
        WriteWireError::Base16Error(value)
    }
}
impl From<Base32Error> for WriteWireError {
    fn from(value: Base32Error) -> Self {
        WriteWireError::Base32Error(value)
    }
}
impl From<ExtendedBase32Error> for WriteWireError {
    fn from(value: ExtendedBase32Error) -> Self {
        WriteWireError::ExtendedBase32Error(value)
    }
}
impl From<Base64Error> for WriteWireError {
    fn from(value: Base64Error) -> Self {
        WriteWireError::Base64Error(value)
    }
}

#[derive(PartialEq, Eq, Hash, Debug)]
pub struct WriteWire<'a> {
    wire: &'a mut [u8],
    offset: usize,
}

impl<'a> WriteWire<'a> {
    #[inline]
    pub fn from_bytes(wire: &'a mut [u8]) -> Self {
        Self { wire, offset: 0 }
    }

    #[inline]
    pub fn current_len(&self) -> usize {
        self.offset
    }

    #[inline]
    pub fn remaining_len(&self) -> usize {
        self.wire.len() - self.current_len()
    }

    #[inline]
    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), WriteWireError> {
        if bytes.len() > self.remaining_len() {
            return Err(WriteWireError::OverflowError(
                "tried to write bytes past the end of the WriteWire buffer".to_string(),
            ));
        }

        self.wire[self.offset..(self.offset + bytes.len())].copy_from_slice(bytes);
        self.offset += bytes.len();

        return Ok(());
    }

    #[inline]
    pub fn write_byte(&mut self, byte: u8) -> Result<(), WriteWireError> {
        if 1 > self.remaining_len() {
            return Err(WriteWireError::OverflowError(
                "tried to write a byte past the end of the WriteWire buffer".to_string(),
            ));
        }

        self.wire[self.offset] = byte;
        self.offset += 1;

        return Ok(());
    }

    #[inline]
    pub fn write_bytes_at(&mut self, bytes: &[u8], index: usize) -> Result<(), WriteWireError> {
        let new_len = (index + bytes.len()).max(self.offset);
        if new_len > self.wire.len() {
            return Err(WriteWireError::OverflowError(
                "tried to write bytes past the end of the WriteWire buffer".to_string(),
            ));
        }

        self.wire[index..(index + bytes.len())].copy_from_slice(bytes);
        self.offset = new_len;

        return Ok(());
    }

    #[inline]
    pub fn write_byte_at(&mut self, byte: u8, index: usize) -> Result<(), WriteWireError> {
        let new_len = (index + 1).max(self.offset);
        if new_len > self.wire.len() {
            return Err(WriteWireError::OverflowError(
                "tried to write a byte past the end of the WriteWire buffer".to_string(),
            ));
        }

        self.wire[index] = byte;
        self.offset = new_len;

        return Ok(());
    }

    #[inline]
    pub fn current(&self) -> &[u8] {
        &self.wire[..self.offset]
    }

    #[inline]
    pub fn as_read_wire(&self) -> ReadWire {
        ReadWire::from_bytes(&self.wire[..self.offset])
    }
}

use std::{error::Error, fmt::Display};

use crate::types::{c_domain_name::CDomainNameError, ascii::AsciiError, base16::Base16Error, base32::Base32Error, extended_base32::ExtendedBase32Error, base64::Base64Error, domain_name::DomainNameError};

#[derive(Debug)]
pub enum ReadWireError {
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
impl Error for ReadWireError {}
impl Display for ReadWireError {
     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FormatError(error) => write!(f, "Read Wire Format Error: {error}"),
            Self::OverflowError(error) => write!(f, "Read Wire Overflow Error: {error}"),
            Self::UnderflowError(error) => write!(f, "Read Wire Underflow Error: {error}"),
            Self::OutOfBoundsError(error) => write!(f, "Read Wire Out Of Bounds Error: {error}"),
            Self::ValueError(error) => write!(f, "Read Wire Value Error: {error}"),
            Self::VersionError(error) => write!(f, "Read Wire Version Error: {error}"),

            Self::CDomainNameError(error) => write!(f, "Read Wire Compressible Domain Name Error: {error}"),
            Self::DomainNameError(error) => write!(f, "Read Wire Incompressible Domain Name Error: {error}"),
            Self::AsciiError(error) => write!(f, "Read Wire Ascii Error: {error}"),

            Self::Base16Error(error) => write!(f, "Read Wire Base16 Error: {error}"),
            Self::Base32Error(error) => write!(f, "Read Wire Bsse32 Error: {error}"),
            Self::ExtendedBase32Error(error) => write!(f, "Read Wire Extended Base32 Error: {error}"),
            Self::Base64Error(error) => write!(f, "Read Wire Base64 Error: {error}"),
        }
    }
}
impl From<CDomainNameError> for ReadWireError {
    fn from(value: CDomainNameError) -> Self {
        ReadWireError::CDomainNameError(value)
    }
}
impl From<DomainNameError> for ReadWireError {
    fn from(value: DomainNameError) -> Self {
        ReadWireError::DomainNameError(value)
    }
}
impl From<AsciiError> for ReadWireError {
    fn from(value: AsciiError) -> Self {
        ReadWireError::AsciiError(value)
    }
}
impl From<Base16Error> for ReadWireError {
    fn from(value: Base16Error) -> Self {
        ReadWireError::Base16Error(value)
    }
}
impl From<Base32Error> for ReadWireError {
    fn from(value: Base32Error) -> Self {
        ReadWireError::Base32Error(value)
    }
}
impl From<ExtendedBase32Error> for ReadWireError {
    fn from(value: ExtendedBase32Error) -> Self {
        ReadWireError::ExtendedBase32Error(value)
    }
}
impl From<Base64Error> for ReadWireError {
    fn from(value: Base64Error) -> Self {
        ReadWireError::Base64Error(value)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ReadWire<'a> {
    wire: &'a [u8],
    offset: usize,
}

impl<'a> ReadWire<'a> {
    #[inline]
    pub fn from_bytes(wire: &'a [u8]) -> Self {
        Self {
            wire: wire,
            offset: 0,
        }
    }

    #[inline]
    pub fn current_state(&'a self) -> &'a [u8] {
        &self.wire[self.offset..]
    }

    #[inline]
    pub fn current_state_len(&self) -> usize {
        self.wire[self.offset..].len()
    }

    #[inline]
    pub fn current_state_offset(&self) -> usize {
        self.offset
    }

    #[inline]
    pub fn is_end_reached(&self) -> bool {
        self.offset >= self.wire.len()
    }

    #[inline]
    pub fn set_offset(&mut self, offset: usize) -> Result<(), ReadWireError> {
        if self.wire.len() < offset {
            return Err(ReadWireError::OverflowError(
                String::from("offset went past the end of the wire")
            ));
        } else {
            self.offset = offset;
            return Ok(());
        }
    }

    #[inline]
    pub fn full_state(&'a self) -> &'a [u8] {
        self.wire
    }

    #[inline]
    pub fn full_state_len(&self) -> usize {
        self.wire.len()
    }

    #[inline]
    pub fn state_from_offset(&'a self, offset: usize) -> Result<&'a [u8], ReadWireError> {
        if self.wire.len() < offset {
            return Err(ReadWireError::OverflowError(
                String::from("offset went past the end of the wire")
            ));
        } else {
            return Ok(&self.wire[offset..]);
        }
    }

    #[inline]
    pub fn shift(&mut self, shift: usize) -> Result<(), ReadWireError> {
        let new_offset = self.offset + shift;
        if self.wire.len() < new_offset {
            return Err(ReadWireError::OverflowError(
                String::from("attempted to shift past the end of the wire")
            ));
        } else {
            self.offset = new_offset;
            return Ok(());
        }
    }

    #[inline]
    pub fn section_from_current_state(&self, lower_bound: Option<usize>, upper_bound: Option<usize>) -> Result<Self, ReadWireError> {
        let (lower_bound, new_offset) = match lower_bound {
            None => (0, self.offset),
            Some(lower_bound) => (lower_bound + self.offset, 0),
        };
        let upper_bound = match upper_bound {
            None => 0,
            Some(upper_bound) => upper_bound + self.offset,
        };

        if lower_bound > upper_bound {
            return Err(ReadWireError::ValueError(
                String::from("lower bound cannot be greater than the upper bound")
            ));
        } else if upper_bound > self.wire.len() {
            return Err(ReadWireError::OverflowError(
                String::from("upper bound cannot be greater than the end of the wire")
            ));
        } else {
            return Ok(Self {
                wire: &self.wire[lower_bound..upper_bound],
                offset: new_offset,
            });
        }
    }

    #[inline]
    pub fn section(&self, lower_bound: usize, upper_bound: usize) -> Result<Self, ReadWireError> {
        if lower_bound > upper_bound {
            return Err(ReadWireError::ValueError(
                String::from("lower bound cannot be greater than the upper bound")
            ));
        } else if upper_bound > self.wire.len() {
            return Err(ReadWireError::OverflowError(
                String::from("upper bound cannot be greater than the end of the wire")
            ));
        } else {
            return Ok(Self {
                wire: &self.wire[lower_bound..upper_bound],
                offset: 0,
            });
        }
    }
}


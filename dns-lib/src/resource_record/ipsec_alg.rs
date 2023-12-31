use std::fmt::Display;

use crate::serde::{wire::{to_wire::ToWire, from_wire::FromWire}, presentation::from_presentation::FromPresentation};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum IpSecAlgorithm {
    Unknown(u8),

    /// https://datatracker.ietf.org/doc/html/rfc2536
    Dsa,

    /// https://datatracker.ietf.org/doc/html/rfc3110
    Rsa,
}

impl IpSecAlgorithm {
    pub const MIN: u8 = u8::MIN;
    pub const MAX: u8 = u8::MAX;

    #[inline]
    pub const fn code(&self) -> u8 {
        return match self {
            Self::Unknown(x) => *x,

            Self::Dsa => 1,
            Self::Rsa => 2,
        };
    }

    #[inline]
    pub const fn mnemonic(&self) -> &str {
        return match self {
            Self::Unknown(_) => "Unknown",

            Self::Dsa => "DSA",
            Self::Rsa => "RSA",
        };
    }

    #[inline]
    pub const fn from_code(value: u8) -> Self {
        return match value {
            1 => Self::Dsa,
            2 => Self::Rsa,

            _ => Self::Unknown(value),
        };
    }
}

impl Display for IpSecAlgorithm {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.mnemonic())
    }
}

impl ToWire for IpSecAlgorithm {
    #[inline]
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>, compression: &mut Option<crate::serde::wire::compression_map::CompressionMap>) -> Result<(), crate::serde::wire::write_wire::WriteWireError> where 'a: 'b {
        self.code().to_wire_format(wire, compression)
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        self.code().serial_length()
    }
}

impl FromWire for IpSecAlgorithm {
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>) -> Result<Self, crate::serde::wire::read_wire::ReadWireError> where Self: Sized, 'a: 'b {
        Ok(Self::from_code(
            u8::from_wire_format(wire)?
        ))
    }
}

impl FromPresentation for IpSecAlgorithm {
    #[inline]
    fn from_token_format<'a, 'b>(token: &'a str) -> Result<Self, crate::serde::presentation::errors::TokenError<'b>> where Self: Sized, 'a: 'b {
        Ok(Self::from_code(
            u8::from_token_format(token)?
        ))
    }
}

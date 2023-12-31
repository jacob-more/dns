use std::fmt::Display;

use crate::serde::{wire::{to_wire::ToWire, from_wire::FromWire}, presentation::from_presentation::FromPresentation};

/// https://datatracker.ietf.org/doc/html/rfc2535#section-3.1.3
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum KeyProtocol {
    Unknown(u8),

    None,
    Tls,
    Email,
    DnsSec,
    IpSec,

    All,
}

impl KeyProtocol {
    pub const MIN: u8 = u8::MIN;
    pub const MAX: u8 = u8::MAX;

    #[inline]
    pub const fn code(&self) -> u8 {
        return match self {
            Self::Unknown(x) => *x,

            Self::None   => 0,
            Self::Tls    => 1,
            Self::Email  => 2,
            Self::DnsSec => 3,
            Self::IpSec  => 4,

            Self::All => 255,
        };
    }

    #[inline]
    pub const fn mnemonic(&self) -> &str {
        return match self {
            Self::Unknown(_) => "Unknown",

            Self::None   => "NONE",
            Self::Tls    => "TLS",
            Self::Email  => "EMAIL",
            Self::DnsSec => "DNSSEC",
            Self::IpSec  => "IPSEC",

            Self::All => "ALL",
        };
    }

    #[inline]
    pub const fn from_code(value: u8) -> Self {
        return match value {
            0 => Self::None,
            1 => Self::Tls,
            2 => Self::Email,
            3 => Self::DnsSec,
            4 => Self::IpSec,

            255 => Self::All,

            _ => Self::Unknown(value),
        };
    }
}

impl Display for KeyProtocol {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.mnemonic())
    }
}

impl ToWire for KeyProtocol {
    #[inline]
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>, compression: &mut Option<crate::serde::wire::compression_map::CompressionMap>) -> Result<(), crate::serde::wire::write_wire::WriteWireError> where 'a: 'b {
        self.code().to_wire_format(wire, compression)
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        self.code().serial_length()
    }
}

impl FromWire for KeyProtocol {
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>) -> Result<Self, crate::serde::wire::read_wire::ReadWireError> where Self: Sized, 'a: 'b {
        Ok(Self::from_code(
            u8::from_wire_format(wire)?
        ))
    }
}

impl FromPresentation for KeyProtocol {
    #[inline]
    fn from_token_format<'a, 'b>(token: &'a str) -> Result<Self, crate::serde::presentation::errors::TokenError<'b>> where Self: Sized, 'a: 'b {
        Ok(Self::from_code(
            u8::from_token_format(token)?
        ))
    }
}

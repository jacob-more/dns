use std::fmt::Display;

use crate::serde::{presentation::{errors::TokenError, from_presentation::FromPresentation, to_presentation::ToPresentation}, wire::{from_wire::FromWire, to_wire::ToWire}};

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
    pub fn mnemonic(&self) -> String {
        return match self {
            Self::Unknown(code) => code.to_string(),

            Self::None   => "NONE".to_string(),
            Self::Tls    => "TLS".to_string(),
            Self::Email  => "EMAIL".to_string(),
            Self::DnsSec => "DNSSEC".to_string(),
            Self::IpSec  => "IPSEC".to_string(),

            Self::All => "ALL".to_string(),
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
    fn from_token_format<'a, 'b, 'c, 'd>(tokens: &'c [&'a str]) -> Result<(Self, &'d [&'a str]), TokenError<'b>> where Self: Sized, 'a: 'b, 'c: 'd, 'c: 'd {
        let (code, tokens) = u8::from_token_format(tokens)?;
        Ok((Self::from_code(code), tokens))
    }
}

impl ToPresentation for KeyProtocol {
    #[inline]
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        out_buffer.push(self.code().to_string())
    }
}

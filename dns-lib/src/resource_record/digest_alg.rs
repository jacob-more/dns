use std::fmt::Display;

use crate::serde::{presentation::{errors::TokenError, from_presentation::FromPresentation, to_presentation::ToPresentation}, wire::{from_wire::FromWire, to_wire::ToWire}};

/// https://www.iana.org/assignments/ds-rr-types/ds-rr-types.xhtml#ds-rr-types-1
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum DigestAlgorithm {
    Unknown(u8),

    Sha1,
    Sha256,
    Gostr341194,
    Sha384,
}

impl DigestAlgorithm {
    pub const MIN: u8 = u8::MIN;
    pub const MAX: u8 = u8::MAX;

    #[inline]
    pub const fn code(&self) -> u8 {
        return match self {
            Self::Unknown(x) => *x,
            
            Self::Sha1        => 1,
            Self::Sha256      => 2,
            Self::Gostr341194 => 3,
            Self::Sha384      => 4,
        };
    }

    #[inline]
    pub const fn mnemonic(&self) -> &str {
        return match self {
            Self::Unknown(_) => "Unknown",

            Self::Sha1        => "SHA-1",
            Self::Sha256      => "SHA-256",
            Self::Gostr341194 => "GOST R 34.11-94",
            Self::Sha384      => "SHA-384",
        };
    }

    #[inline]
    pub const fn from_code(value: u8) -> Self {
        return match value {
            1 => Self::Sha1,
            2 => Self::Sha256,
            3 => Self::Gostr341194,
            4 => Self::Sha384,

            _ => Self::Unknown(value),
        };
    }
}

impl Display for DigestAlgorithm {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.mnemonic())
    }
}

impl ToWire for DigestAlgorithm {
    #[inline]
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>, compression: &mut Option<crate::serde::wire::compression_map::CompressionMap>) -> Result<(), crate::serde::wire::write_wire::WriteWireError> where 'a: 'b {
        self.code().to_wire_format(wire, compression)
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        self.code().serial_length()
    }
}

impl FromWire for DigestAlgorithm {
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>) -> Result<Self, crate::serde::wire::read_wire::ReadWireError> where Self: Sized, 'a: 'b {
        Ok(Self::from_code(
            u8::from_wire_format(wire)?
        ))
    }
}

impl FromPresentation for DigestAlgorithm {
    fn from_token_format<'a, 'b, 'c, 'd>(tokens: &'c [&'a str]) -> Result<(Self, &'d [&'a str]), TokenError<'b>> where Self: Sized, 'a: 'b, 'c: 'd, 'c: 'd {
        let (code, tokens) = u8::from_token_format(tokens)?;
        Ok((Self::from_code(code), tokens))
    }
}

impl ToPresentation for DigestAlgorithm {
    #[inline]
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        out_buffer.push(self.code().to_string())
    }
}

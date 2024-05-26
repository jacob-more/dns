use std::{error::Error, fmt::Display};

use dns_macros::{FromTokenizedRData, FromWire, RTypeCode, ToPresentation, ToWire};

use crate::{resource_record::dnssec_alg::DnsSecAlgorithm, serde::{presentation::{from_presentation::FromPresentation, to_presentation::ToPresentation}, wire::{from_wire::FromWire, to_wire::ToWire}}, types::base64::Base64};

/// (Original) https://datatracker.ietf.org/doc/html/rfc4398#section-2
/// (Updated) https://datatracker.ietf.org/doc/html/rfc6944
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, ToPresentation, FromTokenizedRData, RTypeCode)]
pub struct CERT {
    cert_type: CertificateType,
    key_tag: u16,
    algorithm: DnsSecAlgorithm,
    // FIXME: Base64 needs to be able to decode from multiple whitespace separated tokens, not just one. This is not currently supported because I was running into issues with lifetimes.
    certificate: Base64,
}

#[derive(Debug)]
pub enum CertificateTypeError<'a> {
    UnknownMnemonic(&'a str),
}
impl<'a> Error for CertificateTypeError<'a> {}
impl<'a> Display for CertificateTypeError<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownMnemonic(mnemonic) => write!(f, "unknown certificate type mnemonic '{mnemonic}'"),
        }
    }
}

/// https://datatracker.ietf.org/doc/html/rfc4398#section-2.1
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum CertificateType {
    Unknown(u16),

    Pkix,
    Spki,
    Pgp,
    Ipkix,
    Ispki,
    Ipgp,
    Acpkix,
    Iacpkix,

    Uri,
    Oid,
}

impl CertificateType {
    pub const MIN: u16 = u16::MIN;
    pub const MAX: u16 = u16::MAX;

    #[inline]
    pub const fn code(&self) -> u16 {
        return match self {
            Self::Unknown(x) => *x,

            Self::Pkix =>    1,
            Self::Spki =>    2,
            Self::Pgp =>     3,
            Self::Ipkix =>   4,
            Self::Ispki =>   5,
            Self::Ipgp =>    6,
            Self::Acpkix =>  7,
            Self::Iacpkix => 8,

            Self::Uri => 253,
            Self::Oid => 254,
        };
    }

    #[inline]
    pub const fn mnemonic(&self) -> &str {
        return match self {
            Self::Unknown(_value) => "Unknown",

            Self::Pkix    => "PKIX",
            Self::Spki    => "SPKI",
            Self::Pgp     => "PGP",
            Self::Ipkix   => "IPKIX",
            Self::Ispki   => "ISPKI",
            Self::Ipgp    => "IPGP",
            Self::Acpkix  => "ACPKIX",
            Self::Iacpkix => "IACPKIX",

            Self::Uri => "URI",
            Self::Oid => "OID",
        };
    }

    #[inline]
    pub const fn from_code(value: u16) -> Self {
        return match value {
            1 => Self::Pkix,
            2 => Self::Spki,
            3 => Self::Pgp,
            4 => Self::Ipkix,
            5 => Self::Ispki,
            6 => Self::Ipgp,
            7 => Self::Acpkix,
            8 => Self::Iacpkix,

            253 => Self::Uri,
            254 => Self::Oid,

            _ => Self::Unknown(value),
        };
    }

    #[inline]
    pub fn from_str(cert_type: &str) -> Result<Self, CertificateTypeError> {
        match cert_type {
            "PKIX"    => Ok(Self::Pkix),
            "SPKI"    => Ok(Self::Spki),
            "PGP"     => Ok(Self::Pgp),
            "IPKIX"   => Ok(Self::Ipkix),
            "ISPKI"   => Ok(Self::Ispki),
            "IPGP"    => Ok(Self::Ipgp),
            "ACPKIX"  => Ok(Self::Acpkix),
            "IACPKIX" => Ok(Self::Iacpkix),

            "URI" => Ok(Self::Uri),
            "OID" => Ok(Self::Oid),

            _ => {
                let cert_type = match u16::from_str_radix(cert_type, 10) {
                    Ok(cert_type) => cert_type,
                    Err(_) => return Err(CertificateTypeError::UnknownMnemonic(cert_type)),
                };
                // Note: we don't directly assign it to Unknown since it could be a known code that
                //       just uses the '\d+' syntax.
                Ok(Self::from_code(cert_type))
            },
        }
    }
}

impl Display for CertificateType {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.mnemonic())
    }
}

impl ToWire for CertificateType {
    #[inline]
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>, compression: &mut Option<crate::serde::wire::compression_map::CompressionMap>) -> Result<(), crate::serde::wire::write_wire::WriteWireError> where 'a: 'b {
        self.code().to_wire_format(wire, compression)
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        self.code().serial_length()
    }
}

impl FromWire for CertificateType {
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>) -> Result<Self, crate::serde::wire::read_wire::ReadWireError> where Self: Sized, 'a: 'b {
        Ok(Self::from_code(
            u16::from_wire_format(wire)?
        ))
    }
}

impl FromPresentation for CertificateType {
    #[inline]
    fn from_token_format<'a, 'b>(token: &'a str) -> Result<Self, crate::serde::presentation::errors::TokenError<'b>> where Self: Sized, 'a: 'b {
        Ok(Self::from_str(token)?)
    }
}

impl ToPresentation for CertificateType {
    #[inline]
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        out_buffer.push(self.code().to_string())
    }
}

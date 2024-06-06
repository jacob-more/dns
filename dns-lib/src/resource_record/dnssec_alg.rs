use std::{fmt::Display, error::Error};

use lazy_static::lazy_static;
use regex::Regex;

use crate::serde::{presentation::{errors::TokenError, from_presentation::FromPresentation, to_presentation::ToPresentation}, wire::{from_wire::FromWire, to_wire::ToWire}};

#[derive(Debug)]
pub enum DnsSecAlgorithmError<'a> {
    UnknownMnemonic(&'a str),
}
impl<'a> Error for DnsSecAlgorithmError<'a> {}
impl<'a> Display for DnsSecAlgorithmError<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownMnemonic(mnemonic) => write!(f, "unknown dns security algorithm mnemonic '{mnemonic}'"),
        }
    }
}

/// https://www.iana.org/assignments/dns-sec-alg-numbers/dns-sec-alg-numbers.xhtml#dns-sec-alg-numbers-1
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum DnsSecAlgorithm {
    Unknown(u8),

    Delete,
    RsaMd5,
    Dh,
    Dsa,
    Ecc,
    RsaSha1,
    DsaNsec3Sha1,
    RsaSha1Nsec3Sha1,
    RsaSha256,

    RsaSha512,

    EccGhost,
    EcdsaP256Sha256,
    EcdsaP384Sha384,
    Ed25519,
    Ed448,

    Indirect,
    PrivateDns,
    PrivateOid,
}

impl DnsSecAlgorithm {
    pub const MIN: u8 = u8::MIN;
    pub const MAX: u8 = u8::MAX;

    #[inline]
    pub const fn code(&self) -> u8 {
        return match self {
            Self::Unknown(x) => *x,

            Self::Delete           => 0,
            Self::RsaMd5           => 1,
            Self::Dh               => 2,
            Self::Dsa              => 3,
            Self::Ecc              => 4,
            Self::RsaSha1          => 5,
            Self::DsaNsec3Sha1     => 6,
            Self::RsaSha1Nsec3Sha1 => 7,
            Self::RsaSha256        => 8,

            Self::RsaSha512 => 10,

            Self::EccGhost        => 12,
            Self::EcdsaP256Sha256 => 13,
            Self::EcdsaP384Sha384 => 14,
            Self::Ed25519         => 15,
            Self::Ed448           => 16,

            Self::Indirect    => 252,
            Self::PrivateDns  => 253,
            Self::PrivateOid  => 254,
        };
    }

    #[inline]
    pub const fn name(&self) -> &str {
        return match self {
            Self::Unknown(_) => "Unknown",

            Self::Delete           => "Delete DS",
            Self::RsaMd5           => "RSA/MD5",
            Self::Dh               => "Diffie-Hellman",
            Self::Dsa              => "DSA/SHA-1",
            Self::Ecc              => "Elliptic Curve",
            Self::RsaSha1          => "RSA/SHA-1",
            Self::DsaNsec3Sha1     => "DSA-NSEC3-SHA1",
            Self::RsaSha1Nsec3Sha1 => "RSASHA1-NSEC3-SHA1",
            Self::RsaSha256        => "RSA/SHA-256",

            Self::RsaSha512 => "RSA/SHA-512",

            Self::EccGhost        => "GOST R 34.10-2001",
            Self::EcdsaP256Sha256 => "ECDSA Curve P-256 with SHA-256",
            Self::EcdsaP384Sha384 => "ECDSA Curve P-384 with SHA-384",
            Self::Ed25519         => "Ed25519",
            Self::Ed448           => "Ed448",

            Self::Indirect    => "Indirect",
            Self::PrivateDns  => "Private DNS",
            Self::PrivateOid  => "Private OID",
        };
    }

    #[inline]
    pub fn mnemonic(&self) -> String {
        match self {
            Self::Unknown(code) => code.to_string(),

            Self::Delete  => "DELETE".to_string(),
            Self::RsaMd5  => "RSAMD5".to_string(),
            Self::Dh      => "DH".to_string(),
            Self::Dsa     => "DSA".to_string(),
            Self::Ecc     => "ECC".to_string(),
            Self::RsaSha1 => "RSASHA1".to_string(),
            Self::DsaNsec3Sha1     => "DSA-NSEC3-SHA1".to_string(),
            Self::RsaSha1Nsec3Sha1 => "RSASHA1-NSEC3-SHA1".to_string(),
            Self::RsaSha256        => "RSASHA256".to_string(),

            Self::RsaSha512 => "RSASHA512".to_string(),

            Self::EccGhost        => "ECC-GOST".to_string(),
            Self::EcdsaP256Sha256 => "ECDSAP256SHA256".to_string(),
            Self::EcdsaP384Sha384 => "ECDSAP384SHA384".to_string(),
            Self::Ed25519         => "ED25519".to_string(),
            Self::Ed448           => "ED448".to_string(),

            Self::Indirect    => "INDIRECT".to_string(),
            Self::PrivateDns  => "PRIVATEDNS".to_string(),
            Self::PrivateOid  => "PRIVATEOID".to_string(),
        }
    }

    #[inline]
    pub const fn from_code(value: u8) -> Self {
        return match value {
            0 => Self::Delete,
            1 => Self::RsaMd5,
            2 => Self::Dh,
            3 => Self::Dsa,
            4 => Self::Ecc,
            5 => Self::RsaSha1,
            6 => Self::DsaNsec3Sha1,
            7 => Self::RsaSha1Nsec3Sha1,
            8 => Self::RsaSha256,

            10 => Self::RsaSha512,

            12 => Self::EccGhost,
            13 => Self::EcdsaP256Sha256,
            14 => Self::EcdsaP384Sha384,
            15 => Self::Ed25519,
            16 => Self::Ed448,

            252 => Self::Indirect,
            253 => Self::PrivateDns,
            254 => Self::PrivateOid,

            _ => Self::Unknown(value),
        };
    }

    #[inline]
    pub fn from_str<'a>(mnemonic: &'a str) -> Result<Self, DnsSecAlgorithmError<'a>> {
        return match mnemonic {
            "DELETE"             => Ok(Self::Delete),
            "RSAMD5"             => Ok(Self::RsaMd5),
            "DH"                 => Ok(Self::Dh),
            "DSA"                => Ok(Self::Dsa),
            "ECC"                => Ok(Self::Ecc),
            "RSASHA1"            => Ok(Self::RsaSha1),
            "DSA-NSEC3-SHA1"     => Ok(Self::DsaNsec3Sha1),
            "RSASHA1-NSEC3-SHA1" => Ok(Self::RsaSha1Nsec3Sha1),
            "RSASHA256"          => Ok(Self::RsaSha256),
            "RSASHA512"          => Ok(Self::RsaSha512),
            "ECC-GOST"           => Ok(Self::EccGhost),
            "ECDSAP256SHA256"    => Ok(Self::EcdsaP256Sha256),
            "ECDSAP384SHA384"    => Ok(Self::EcdsaP384Sha384),
            "ED25519"            => Ok(Self::Ed25519),
            "ED448"              => Ok(Self::Ed448),
            "INDIRECT"           => Ok(Self::Indirect),
            "PRIVATEDNS"         => Ok(Self::PrivateDns),
            "PRIVATEOID"         => Ok(Self::PrivateOid),
            _ => Err(DnsSecAlgorithmError::UnknownMnemonic(mnemonic)),
        };
    }
}

impl Display for DnsSecAlgorithm {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.mnemonic())
    }
}

impl ToWire for DnsSecAlgorithm {
    #[inline]
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>, compression: &mut Option<crate::serde::wire::compression_map::CompressionMap>) -> Result<(), crate::serde::wire::write_wire::WriteWireError> where 'a: 'b {
        self.code().to_wire_format(wire, compression)
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        self.code().serial_length()
    }
}

impl FromWire for DnsSecAlgorithm {
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>) -> Result<Self, crate::serde::wire::read_wire::ReadWireError> where Self: Sized, 'a: 'b {
        Ok(Self::from_code(
            u8::from_wire_format(wire)?
        ))
    }
}

impl FromPresentation for DnsSecAlgorithm {
    #[inline]
    fn from_token_format<'a, 'b, 'c, 'd>(tokens: &'c [&'a str]) -> Result<(Self, &'d [&'a str]), TokenError<'b>> where Self: Sized, 'a: 'b, 'c: 'd, 'c: 'd {
        lazy_static!{
            static ref REGEX_UNSIGNED_INT: Regex = Regex::new(r"\A((\d)+)\z").unwrap();
        }

        match tokens {
            &[] => Err(TokenError::OutOfTokens),
            &[token, ..] => {
                if REGEX_UNSIGNED_INT.is_match(token) {
                    Ok((Self::from_code(u8::from_str_radix(token, 10)?), &tokens[1..]))
                } else {
                    Ok((Self::from_str(token)?, &tokens[1..]))
                }
            }
        }
    }
}

impl ToPresentation for DnsSecAlgorithm {
    #[inline]
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        out_buffer.push(self.mnemonic())
    }
}

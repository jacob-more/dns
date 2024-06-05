use std::fmt::Display;

use dns_macros::{FromTokenizedRData, FromWire, RTypeCode, ToPresentation, ToWire};

use crate::{serde::{presentation::{from_presentation::FromPresentation, to_presentation::ToPresentation}, wire::{from_wire::FromWire, to_wire::ToWire}}, types::base16::Base16};

/// (Original) https://datatracker.ietf.org/doc/html/rfc6698#section-2
/// (Updated) https://datatracker.ietf.org/doc/html/rfc8749#name-moving-dlv-to-historic-stat
/// (Updated) https://datatracker.ietf.org/doc/html/rfc7218
/// (Updated) https://datatracker.ietf.org/doc/html/rfc7671
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, ToPresentation, FromTokenizedRData, RTypeCode)]
pub struct TLSA {
    certificate_usage: CertificateUsage,
    selector: Selector,
    matching_type: MatchingType,
    // FIXME: Base16 needs to be able to decode from multiple whitespace separated tokens, not just one. This is not currently supported because I was running into issues with lifetimes.
    certificate: Base16,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum CertificateUsage {
    Unknown(u8),
    
    PkixTa,
    PkixEe,
    DaneTa,
    DaneEe,

    PrivCert,
}

impl CertificateUsage {
    pub const MIN: u8 = u8::MIN;
    pub const MAX: u8 = u8::MAX;

    #[inline]
    pub fn code(&self) -> u8 {
        match self {
            Self::Unknown(x) => *x,

            Self::PkixTa => 0,
            Self::PkixEe => 1,
            Self::DaneTa => 2,
            Self::DaneEe => 3,

            Self::PrivCert => 255,
        }
    }

    #[inline]
    pub fn mnemonic(&self) -> String {
        match self {
            Self::Unknown(value) => value.to_string(),

            Self::PkixTa => "PKIX-TA".to_string(),
            Self::PkixEe => "PKIX-EE".to_string(),
            Self::DaneTa => "DANE-TA".to_string(),
            Self::DaneEe => "DANE-EE".to_string(),

            Self::PrivCert => "PrivCert".to_string(),
        }
    }

    #[inline]
    pub fn from_code(value: u8) -> Self {
        match value {
            0 => Self::PkixTa,
            1 => Self::PkixEe,
            2 => Self::DaneTa,
            3 => Self::DaneEe,

            255 => Self::PrivCert,

            _ => Self::Unknown(value),
        }
    }
}

impl Display for CertificateUsage {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.mnemonic())
    }
}

impl ToWire for CertificateUsage {
    #[inline]
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>, compression: &mut Option<crate::serde::wire::compression_map::CompressionMap>) -> Result<(), crate::serde::wire::write_wire::WriteWireError> where 'a: 'b {
        self.code().to_wire_format(wire, compression)
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        self.code().serial_length()
    }
}

impl FromWire for CertificateUsage {
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>) -> Result<Self, crate::serde::wire::read_wire::ReadWireError> where Self: Sized, 'a: 'b {
        Ok(Self::from_code(
            u8::from_wire_format(wire)?
        ))
    }
}

impl FromPresentation for CertificateUsage {
    fn from_token_format<'a, 'b>(token: &'a str) -> Result<Self, crate::serde::presentation::errors::TokenError<'b>> where Self: Sized, 'a: 'b {
        Ok(Self::from_code(
            u8::from_token_format(token)?
        ))
    }
}

impl ToPresentation for CertificateUsage {
    #[inline]
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        out_buffer.push(self.code().to_string())
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Selector {
    Unknown(u8),
    
    Cert,
    Spki,

    PrivSel,
}

impl Selector {
    pub const MIN: u8 = u8::MIN;
    pub const MAX: u8 = u8::MAX;

    #[inline]
    pub fn code(&self) -> u8 {
        match self {
            Self::Unknown(x) => *x,

            Self::Cert => 0,
            Self::Spki => 1,

            Self::PrivSel => 255,
        }
    }

    #[inline]
    pub fn mnemonic(&self) -> String {
        match self {
            Self::Unknown(value) => value.to_string(),

            Self::Cert => "Cert".to_string(),
            Self::Spki => "SPKI".to_string(),

            Self::PrivSel => "PrivSel".to_string(),
        }
    }

    #[inline]
    pub fn from_code(value: u8) -> Self {
        match value {
            0 => Self::Cert,
            1 => Self::Spki,

            255 => Self::PrivSel,

            _ => Self::Unknown(value),
        }
    }
}

impl Display for Selector {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.mnemonic())
    }
}

impl ToWire for Selector {
    #[inline]
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>, compression: &mut Option<crate::serde::wire::compression_map::CompressionMap>) -> Result<(), crate::serde::wire::write_wire::WriteWireError> where 'a: 'b {
        self.code().to_wire_format(wire, compression)
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        self.code().serial_length()
    }
}

impl FromWire for Selector {
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>) -> Result<Self, crate::serde::wire::read_wire::ReadWireError> where Self: Sized, 'a: 'b {
        Ok(Self::from_code(
            u8::from_wire_format(wire)?
        ))
    }
}

impl FromPresentation for Selector {
    fn from_token_format<'a, 'b>(token: &'a str) -> Result<Self, crate::serde::presentation::errors::TokenError<'b>> where Self: Sized, 'a: 'b {
        Ok(Self::from_code(
            u8::from_token_format(token)?
        ))
    }
}

impl ToPresentation for Selector {
    #[inline]
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        out_buffer.push(self.code().to_string())
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum MatchingType {
    Unknown(u8),
    
    Full,
    Sha2_256,
    Sha2_512,

    PrivMatch,
}

impl MatchingType {
    pub const MIN: u8 = u8::MIN;
    pub const MAX: u8 = u8::MAX;

    #[inline]
    pub fn code(&self) -> u8 {
        match self {
            Self::Unknown(x) => *x,

            Self::Full     => 0,
            Self::Sha2_256 => 1,
            Self::Sha2_512 => 2,

            Self::PrivMatch => 255,
        }
    }

    #[inline]
    pub fn mnemonic(&self) -> String {
        match self {
            Self::Unknown(value) => value.to_string(),

            Self::Full     => "Full".to_string(),
            Self::Sha2_256 => "SHA2-256".to_string(),
            Self::Sha2_512 => "SHA2-512".to_string(),

            Self::PrivMatch => "PrivMatch".to_string(),
        }
    }

    #[inline]
    pub fn from_code(value: u8) -> Self {
        match value {
            0 => Self::Full,
            1 => Self::Sha2_256,
            2 => Self::Sha2_512,

            255 => Self::PrivMatch,

            _ => Self::Unknown(value),
        }
    }
}

impl Display for MatchingType {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.mnemonic())
    }
}

impl ToWire for MatchingType {
    #[inline]
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>, compression: &mut Option<crate::serde::wire::compression_map::CompressionMap>) -> Result<(), crate::serde::wire::write_wire::WriteWireError> where 'a: 'b {
        self.code().to_wire_format(wire, compression)
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        self.code().serial_length()
    }
}

impl FromWire for MatchingType {
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>) -> Result<Self, crate::serde::wire::read_wire::ReadWireError> where Self: Sized, 'a: 'b {
        Ok(Self::from_code(
            u8::from_wire_format(wire)?
        ))
    }
}

impl FromPresentation for MatchingType {
    fn from_token_format<'a, 'b>(token: &'a str) -> Result<Self, crate::serde::presentation::errors::TokenError<'b>> where Self: Sized, 'a: 'b {
        Ok(Self::from_code(
            u8::from_token_format(token)?
        ))
    }
}

impl ToPresentation for MatchingType {
    #[inline]
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        out_buffer.push(self.code().to_string())
    }
}

#[cfg(test)]
mod tlsa_circular_serde_sanity_test {
    use crate::{serde::wire::circular_test::gen_test_circular_serde_sanity_test, types::base16::Base16};
    use super::{CertificateUsage, MatchingType, Selector, TLSA};

    gen_test_circular_serde_sanity_test!(
        rfc_6698_example_1_record_circular_serde_sanity_test,
        TLSA {
            certificate_usage: CertificateUsage::from_code(0),
            selector: Selector::from_code(0),
            matching_type: MatchingType::from_code(1),
            certificate: Base16::from_case_insensitive_utf8("d2abde240d7cd3ee6b4b28c54df034b97983a1d16e8a410e4561cb106618e971").unwrap()
        }
    );
    gen_test_circular_serde_sanity_test!(
        rfc_6698_example_2_record_circular_serde_sanity_test,
        TLSA {
            certificate_usage: CertificateUsage::from_code(1),
            selector: Selector::from_code(1),
            matching_type: MatchingType::from_code(2),
            certificate: Base16::from_case_insensitive_utf8("92003ba34942dc74152e2f2c408d29eca5a520e7f2e06bb944f4dca346baf63c1b177615d466f6c4b71c216a50292bd58c9ebdd2f74e38fe51ffd48c43326cbc").unwrap()
        }
    );
}

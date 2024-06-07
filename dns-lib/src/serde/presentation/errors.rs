use std::{error::Error, fmt::Display, num::{ParseIntError, TryFromIntError}, net::AddrParseError};

use mac_address::MacParseError;

use crate::{resource_record::{dnssec_alg::DnsSecAlgorithmError, ports::PortError, protocol::ProtocolError, rclass::RClassError, rtype::{RType, RTypeError}, time::{DateTimeError, TimeError}, types::cert::CertificateTypeError}, types::{ascii::AsciiError, base16::Base16Error, base32::Base32Error, base64::Base64Error, c_domain_name::CDomainNameError, character_string::CharacterStringError, domain_name::DomainNameError, extended_base32::ExtendedBase32Error}};

use super::tokenizer::errors::TokenizerError;

#[derive(Debug)]
pub enum TokenizedRecordError<'a> {
    TokenizerError(TokenizerError<'a>),
    TokenError(TokenError<'a>),
    TooManyRDataTokensError{ expected: usize, received: usize },
    TooFewRDataTokensError{ expected: usize, received: usize },
    UnsupportedRType(RType),
    RTypeNotAllowed(RType),
    OutOfBoundsError(String),
    ValueError(String),
}
impl<'a> Error for TokenizedRecordError<'a> {}
impl<'a> Display for TokenizedRecordError<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TokenizerError(error) => write!(f, "{error}"),
            Self::TokenError(error) => write!(f, "{error}"),
            Self::TooManyRDataTokensError{expected, received} => write!(f, "too many tokens; expected {expected} but received {received}"),
            Self::TooFewRDataTokensError{expected, received} => write!(f, "too few tokens; expected {expected} but received {received}"),
            Self::UnsupportedRType(rtype) => write!(f, "Resource Record Type {rtype} is not supported"),
            Self::RTypeNotAllowed(rtype) => write!(f, "Resource Record Type {rtype} is not allowed in files"),
            Self::OutOfBoundsError(error) => write!(f, "Out Of Bounds: {error}"),
            Self::ValueError(error) => write!(f, "Value Error: {error}"),
        }
    }
}
impl<'a> From<TokenizerError<'a>> for TokenizedRecordError<'a> {
    fn from(value: TokenizerError<'a>) -> Self {
        Self::TokenizerError(value)
    }
}
impl<'a> From<TokenError<'a>> for TokenizedRecordError<'a> {
    fn from(value: TokenError<'a>) -> Self {
        Self::TokenError(value)
    }
}

#[derive(Debug)]
pub enum TokenError<'a> {
    OutOfTokens,
    ParseIntError(ParseIntError),
    TryFromIntError(TryFromIntError),
    UxTryFromIntError,
    AddressParseError(AddrParseError),
    MacParseError(MacParseError),
    RClassError(RClassError<'a>),
    RTypeError(RTypeError<'a>),
    DnsSecAlgorithmError(DnsSecAlgorithmError<'a>),
    AsciiError(AsciiError),
    CharacterStringError(CharacterStringError),
    CDomainNameError(CDomainNameError),
    DomainNameError(DomainNameError),
    Base16Error(Base16Error),
    Base32Error(Base32Error),
    ExtendedBase32Error(ExtendedBase32Error),
    Base64Error(Base64Error),
    TimeError(TimeError),
    ProtocolError(ProtocolError<'a>),
    PortError(PortError),
    CertificateTypeError(CertificateTypeError<'a>),
}
impl<'a> Error for TokenError<'a> {}
impl<'a> Display for TokenError<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OutOfTokens => write!(f, "Token Error: no tokens to parse. At least 1 was expected"),
            Self::ParseIntError(error) => write!(f, "{error}"),
            Self::TryFromIntError(error) => write!(f, "{error}"),
            Self::UxTryFromIntError => write!(f, "out of range integral type conversion attempted"),
            Self::AddressParseError(error) => write!(f, "{error}"),
            Self::MacParseError(error) => write!(f, "{error}"),
            Self::RClassError(error) => write!(f, "{error}"),
            Self::RTypeError(error) => write!(f, "{error}"),
            Self::DnsSecAlgorithmError(error) => write!(f, "{error}"),
            Self::AsciiError(error) => write!(f, "{error}"),
            Self::CharacterStringError(error) => write!(f, "{error}"),
            Self::CDomainNameError(error) => write!(f, "{error}"),
            Self::DomainNameError(error) => write!(f, "{error}"),
            Self::Base16Error(error) => write!(f, "{error}"),
            Self::Base32Error(error) => write!(f, "{error}"),
            Self::ExtendedBase32Error(error) => write!(f, "{error}"),
            Self::Base64Error(error) => write!(f, "{error}"),
            Self::TimeError(error) => write!(f, "{error}"),
            Self::ProtocolError(error) => write!(f, "{error}"),
            Self::PortError(error) => write!(f, "{error}"),
            Self::CertificateTypeError(error) => write!(f, "{error}"),
        }
    }
}
impl<'a> From<ParseIntError> for TokenError<'a> {
    fn from(value: ParseIntError) -> Self {
        Self::ParseIntError(value)
    }
}
impl<'a> From<AddrParseError> for TokenError<'a> {
    fn from(value: AddrParseError) -> Self {
        Self::AddressParseError(value)
    }
}
impl<'a> From<MacParseError> for TokenError<'a> {
    fn from(value: MacParseError) -> Self {
        Self::MacParseError(value)
    }
}
impl<'a> From<RClassError<'a>> for TokenError<'a> {
    fn from(value: RClassError<'a>) -> Self {
        Self::RClassError(value)
    }
}
impl<'a> From<RTypeError<'a>> for TokenError<'a> {
    fn from(value: RTypeError<'a>) -> Self {
        Self::RTypeError(value)
    }
}
impl<'a> From<DnsSecAlgorithmError<'a>> for TokenError<'a> {
    fn from(value: DnsSecAlgorithmError<'a>) -> Self {
        Self::DnsSecAlgorithmError(value)
    }
}
impl<'a> From<AsciiError> for TokenError<'a> {
    fn from(value: AsciiError) -> Self {
        Self::AsciiError(value)
    }
}
impl<'a> From<CharacterStringError> for TokenError<'a> {
    fn from(value: CharacterStringError) -> Self {
        Self::CharacterStringError(value)
    }
}
impl<'a> From<CDomainNameError> for TokenError<'a> {
    fn from(value: CDomainNameError) -> Self {
        Self::CDomainNameError(value)
    }
}
impl<'a> From<DomainNameError> for TokenError<'a> {
    fn from(value: DomainNameError) -> Self {
        Self::DomainNameError(value)
    }
}
impl<'a> From<Base16Error> for TokenError<'a> {
    fn from(value: Base16Error) -> Self {
        Self::Base16Error(value)
    }
}
impl<'a> From<Base32Error> for TokenError<'a> {
    fn from(value: Base32Error) -> Self {
        Self::Base32Error(value)
    }
}
impl<'a> From<ExtendedBase32Error> for TokenError<'a> {
    fn from(value: ExtendedBase32Error) -> Self {
        Self::ExtendedBase32Error(value)
    }
}
impl<'a> From<Base64Error> for TokenError<'a> {
    fn from(value: Base64Error) -> Self {
        Self::Base64Error(value)
    }
}
impl<'a> From<TimeError> for TokenError<'a> {
    fn from(value: TimeError) -> Self {
        Self::TimeError(value)
    }
}
impl<'a> From<DateTimeError> for TokenError<'a> {
    fn from(value: DateTimeError) -> Self {
        Self::TimeError(TimeError::DateTimeError(value))
    }
}
impl<'a> From<ProtocolError<'a>> for TokenError<'a> {
    fn from(value: ProtocolError<'a>) -> Self {
        Self::ProtocolError(value)
    }
}
impl<'a> From<PortError> for TokenError<'a> {
    fn from(value: PortError) -> Self {
        Self::PortError(value)
    }
}
impl<'a> From<CertificateTypeError<'a>> for TokenError<'a> {
    fn from(value: CertificateTypeError<'a>) -> Self {
        Self::CertificateTypeError(value)
    }
}

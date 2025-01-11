use std::{error::Error, fmt::Display, num::{ParseIntError, TryFromIntError}, net::AddrParseError};

use mac_address::MacParseError;

use crate::{resource_record::{dnssec_alg::DnsSecAlgorithmError, ports::PortError, protocol::ProtocolError, rclass::RClassError, rtype::{RType, RTypeError}, time::{DateTimeError, TimeError}, types::cert::CertificateTypeError}, types::{ascii::AsciiError, base16::Base16Error, base32::Base32Error, base64::Base64Error, c_domain_name::CDomainNameError, character_string::CharacterStringError, domain_name::DomainNameError, extended_base32::ExtendedBase32Error}};

use super::tokenizer::errors::TokenizerError;

#[derive(Debug)]
pub enum TokenizedRecordError {
    TokenizerError(TokenizerError),
    TokenError(TokenError),
    TooManyRDataTokensError{ expected: usize, received: usize },
    TooFewRDataTokensError{ expected: usize, received: usize },
    UnsupportedRType(RType),
    RTypeNotAllowed(RType),
    OutOfBoundsError(String),
    ValueError(String),
}
impl Error for TokenizedRecordError {}
impl Display for TokenizedRecordError {
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
impl From<TokenizerError> for TokenizedRecordError {
    fn from(value: TokenizerError) -> Self {
        Self::TokenizerError(value)
    }
}
impl From<TokenError> for TokenizedRecordError {
    fn from(value: TokenError) -> Self {
        Self::TokenError(value)
    }
}

#[derive(Debug)]
pub enum TokenError {
    OutOfTokens,
    ParseIntError(ParseIntError),
    TryFromIntError(TryFromIntError),
    UxTryFromIntError,
    AddressParseError(AddrParseError),
    MacParseError(MacParseError),
    RClassError(RClassError),
    RTypeError(RTypeError),
    DnsSecAlgorithmError(DnsSecAlgorithmError),
    AsciiError(AsciiError),
    CharacterStringError(CharacterStringError),
    CDomainNameError(CDomainNameError),
    DomainNameError(DomainNameError),
    Base16Error(Base16Error),
    Base32Error(Base32Error),
    ExtendedBase32Error(ExtendedBase32Error),
    Base64Error(Base64Error),
    TimeError(TimeError),
    ProtocolError(ProtocolError),
    PortError(PortError),
    CertificateTypeError(CertificateTypeError),
}
impl Error for TokenError {}
impl Display for TokenError {
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
impl From<ParseIntError> for TokenError {
    fn from(value: ParseIntError) -> Self {
        Self::ParseIntError(value)
    }
}
impl From<AddrParseError> for TokenError {
    fn from(value: AddrParseError) -> Self {
        Self::AddressParseError(value)
    }
}
impl From<MacParseError> for TokenError {
    fn from(value: MacParseError) -> Self {
        Self::MacParseError(value)
    }
}
impl From<RClassError> for TokenError {
    fn from(value: RClassError) -> Self {
        Self::RClassError(value)
    }
}
impl From<RTypeError> for TokenError {
    fn from(value: RTypeError) -> Self {
        Self::RTypeError(value)
    }
}
impl From<DnsSecAlgorithmError> for TokenError {
    fn from(value: DnsSecAlgorithmError) -> Self {
        Self::DnsSecAlgorithmError(value)
    }
}
impl From<AsciiError> for TokenError {
    fn from(value: AsciiError) -> Self {
        Self::AsciiError(value)
    }
}
impl From<CharacterStringError> for TokenError {
    fn from(value: CharacterStringError) -> Self {
        Self::CharacterStringError(value)
    }
}
impl From<CDomainNameError> for TokenError {
    fn from(value: CDomainNameError) -> Self {
        Self::CDomainNameError(value)
    }
}
impl From<DomainNameError> for TokenError {
    fn from(value: DomainNameError) -> Self {
        Self::DomainNameError(value)
    }
}
impl From<Base16Error> for TokenError {
    fn from(value: Base16Error) -> Self {
        Self::Base16Error(value)
    }
}
impl From<Base32Error> for TokenError {
    fn from(value: Base32Error) -> Self {
        Self::Base32Error(value)
    }
}
impl From<ExtendedBase32Error> for TokenError {
    fn from(value: ExtendedBase32Error) -> Self {
        Self::ExtendedBase32Error(value)
    }
}
impl From<Base64Error> for TokenError {
    fn from(value: Base64Error) -> Self {
        Self::Base64Error(value)
    }
}
impl From<TimeError> for TokenError {
    fn from(value: TimeError) -> Self {
        Self::TimeError(value)
    }
}
impl From<DateTimeError> for TokenError {
    fn from(value: DateTimeError) -> Self {
        Self::TimeError(TimeError::DateTimeError(value))
    }
}
impl From<ProtocolError> for TokenError {
    fn from(value: ProtocolError) -> Self {
        Self::ProtocolError(value)
    }
}
impl From<PortError> for TokenError {
    fn from(value: PortError) -> Self {
        Self::PortError(value)
    }
}
impl From<CertificateTypeError> for TokenError {
    fn from(value: CertificateTypeError) -> Self {
        Self::CertificateTypeError(value)
    }
}

use std::{error::Error, fmt::Display, num::{ParseIntError, TryFromIntError}, net};

use mac_address::MacParseError;

use crate::types::{ascii::AsciiError, character_string::CharacterStringError, c_domain_name::CDomainNameError, base16::Base16Error, base32::Base32Error, extended_base32::ExtendedBase32Error, domain_name::DomainNameError, base64::Base64Error};

use super::tokenizer::errors::TokenizerError;

#[derive(Debug)]
pub enum TokenizedRecordError<'a> {
    TokenizerError(TokenizerError<'a>),
    TokenError(TokenError<'a>),
}
impl<'a> Error for TokenizedRecordError<'a> {}
impl<'a> Display for TokenizedRecordError<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TokenizerError(error) => write!(f, "{error}"),
            Self::TokenError(error) => write!(f, "{error}"),
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
    ParseIntError(ParseIntError),
    TryFromIntError(TryFromIntError),
    UxTryFromIntError,
    AddressParseError(net::AddrParseError),
    MacParseError(MacParseError),
    AsciiError(AsciiError),
    CharacterStringError(CharacterStringError),
    CDomainNameError(CDomainNameError),
    DomainNameError(DomainNameError),
    Base16Error(Base16Error),
    Base32Error(Base32Error),
    ExtendedBase32Error(ExtendedBase32Error),
    Base64Error(Base64Error),
}
impl<'a> Error for TokenError<'a> {}
impl<'a> Display for TokenError<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ParseIntError(error) => write!(f, "{error}"),
            Self::TryFromIntError(error) => write!(f, "{error}"),
            Self::UxTryFromIntError => write!(f, "out of range integral type conversion attempted"),
            Self::AddressParseError(error) => write!(f, "{error}"),
            Self::MacParseError(error) => write!(f, "{error}"),
            Self::AsciiError(error) => write!(f, "{error}"),
            Self::CharacterStringError(error) => write!(f, "{error}"),
            Self::CDomainNameError(error) => write!(f, "{error}"),
            Self::DomainNameError(error) => write!(f, "{error}"),
            Self::Base16Error(error) => write!(f, "{error}"),
            Self::Base32Error(error) => write!(f, "{error}"),
            Self::ExtendedBase32Error(error) => write!(f, "{error}"),
            Self::Base64Error(error) => write!(f, "{error}"),
        }
    }
}
impl<'a> From<ParseIntError> for TokenError<'a> {
    fn from(value: ParseIntError) -> Self {
        Self::ParseIntError(value)
    }
}
impl<'a> From<net::AddrParseError> for TokenError<'a> {
    fn from(value: net::AddrParseError) -> Self {
        Self::AddressParseError(value)
    }
}
impl<'a> From<MacParseError> for TokenError<'a> {
    fn from(value: MacParseError) -> Self {
        Self::MacParseError(value)
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
}impl<'a> From<CDomainNameError> for TokenError<'a> {
    fn from(value: CDomainNameError) -> Self {
        Self::CDomainNameError(value)
    }
}impl<'a> From<DomainNameError> for TokenError<'a> {
    fn from(value: DomainNameError) -> Self {
        Self::DomainNameError(value)
    }
}impl<'a> From<Base16Error> for TokenError<'a> {
    fn from(value: Base16Error) -> Self {
        Self::Base16Error(value)
    }
}impl<'a> From<Base32Error> for TokenError<'a> {
    fn from(value: Base32Error) -> Self {
        Self::Base32Error(value)
    }
}impl<'a> From<ExtendedBase32Error> for TokenError<'a> {
    fn from(value: ExtendedBase32Error) -> Self {
        Self::ExtendedBase32Error(value)
    }
}impl<'a> From<Base64Error> for TokenError<'a> {
    fn from(value: Base64Error) -> Self {
        Self::Base64Error(value)
    }
}

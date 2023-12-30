use std::{error::Error, fmt::Display, num::{ParseIntError, TryFromIntError}, net};

use mac_address::MacParseError;

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

use std::{error::Error, fmt::Display, num::{ParseIntError, TryFromIntError}, net};

use mac_address::MacParseError;

use super::tokenizer::errors::TokenizerError;

#[derive(Debug)]
pub enum TokenizedRDataError<'a> {
    TokenizerError(TokenizerError<'a>),
    TokenError(TokenError),
}
impl<'a> Error for TokenizedRDataError<'a> {}
impl<'a> Display for TokenizedRDataError<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TokenizerError(error) => write!(f, "{error}"),
            Self::TokenError(error) => write!(f, "{error}"),
        }
    }
}
impl<'a> From<TokenizerError<'a>> for TokenizedRDataError<'a> {
    fn from(value: TokenizerError<'a>) -> Self {
        Self::TokenizerError(value)
    }
}
impl<'a> From<TokenError> for TokenizedRDataError<'a> {
    fn from(value: TokenError) -> Self {
        Self::TokenError(value)
    }
}

#[derive(Debug)]
pub enum TokenError {
    ParseIntError(ParseIntError),
    TryFromIntError(TryFromIntError),
    UxTryFromIntError,
    AddressParseError(net::AddrParseError),
    MacParseError(MacParseError),
}
impl<'a> Error for TokenError {}
impl<'a> Display for TokenError {
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
impl From<ParseIntError> for TokenError {
    fn from(value: ParseIntError) -> Self {
        Self::ParseIntError(value)
    }
}
impl From<net::AddrParseError> for TokenError {
    fn from(value: net::AddrParseError) -> Self {
        Self::AddressParseError(value)
    }
}
impl From<MacParseError> for TokenError {
    fn from(value: MacParseError) -> Self {
        Self::MacParseError(value)
    }
}

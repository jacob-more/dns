use std::{error::Error, fmt::Display, io};

#[derive(Debug)]
pub enum TokenizerError<'a> {
    IOError(io::Error),
    NestedOpenParenthesis,
    UnopenedClosingParenthesis,
    NoClosingParenthesis,
    UnknownTokens,
    UnknownToken(&'a str),
    TwoUnknownTokens(&'a str, &'a str),
    ThreeUnknownTokens(&'a str, &'a str, &'a str),
}
impl<'a> Error for TokenizerError<'a> {}
impl<'a> Display for TokenizerError<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IOError(error) => write!(f, "{error}"),
            Self::NestedOpenParenthesis => write!(f, "an open parenthesis was used within a block of parenthesis. Only 1 set of parenthesis may be used at a time"),
            Self::UnopenedClosingParenthesis => write!(f, "a closing parenthesis was used without a matching opening parenthesis"),
            Self::NoClosingParenthesis => write!(f, "an opening parenthesis was used without a closing parenthesis"),
            Self::UnknownTokens => write!(f, "unknown tokens"),
            Self::UnknownToken(token) => write!(f, "unknown token '{token}'"),
            Self::TwoUnknownTokens(token1, token2) => write!(f, "unknown tokens '{token1}' and '{token2}'"),
            Self::ThreeUnknownTokens(token1, token2, token3) => write!(f, "unknown tokens '{token1}', '{token2}' and '{token3}'"),
        }
    }
}

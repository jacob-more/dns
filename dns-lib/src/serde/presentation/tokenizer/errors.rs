use std::{error::Error, fmt::Display, io};

#[derive(Debug)]
pub enum TokenizerError {
    IOError(io::Error),
    NestedOpenParenthesis,
    UnopenedClosingParenthesis,
    NoClosingParenthesis,
    OriginUsedBeforeDefined,
    BlankDomainUsedBeforeDefined,
    BlankClassUsedBeforeDefined,
    BlankTTLUsedBeforeDefined,
    UnknownTokens,
    UnknownToken(String),
    TwoUnknownTokens(String, String),
    ThreeUnknownTokens(String, String, String),
}
impl Error for TokenizerError {}
impl Display for TokenizerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IOError(error) => write!(f, "{error}"),
            Self::NestedOpenParenthesis => write!(
                f,
                "an open parenthesis was used within a block of parenthesis. Only 1 set of parenthesis may be used at a time"
            ),
            Self::UnopenedClosingParenthesis => write!(
                f,
                "a closing parenthesis was used without a matching opening parenthesis"
            ),
            Self::NoClosingParenthesis => write!(
                f,
                "an opening parenthesis was used without a closing parenthesis"
            ),
            Self::OriginUsedBeforeDefined => write!(
                f,
                "the origin '@' was used before it was defined with '$ORIGIN'"
            ),
            Self::BlankDomainUsedBeforeDefined => write!(
                f,
                "a resource record that refers to the last domain name was used without a previous domain name being defined"
            ),
            Self::BlankClassUsedBeforeDefined => write!(
                f,
                "a resource record that refers to the last rclass was used without a previous rclass being defined"
            ),
            Self::BlankTTLUsedBeforeDefined => write!(
                f,
                "a resource record that refers to the last ttl was used without a previous ttl being defined"
            ),
            Self::UnknownTokens => write!(f, "unknown tokens"),
            Self::UnknownToken(token) => write!(f, "unknown token '{token}'"),
            Self::TwoUnknownTokens(token1, token2) => {
                write!(f, "unknown tokens '{token1}' and '{token2}'")
            }
            Self::ThreeUnknownTokens(token1, token2, token3) => {
                write!(f, "unknown tokens '{token1}', '{token2}' and '{token3}'")
            }
        }
    }
}

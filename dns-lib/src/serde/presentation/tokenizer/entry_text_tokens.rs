use std::fmt::Display;

use super::{text_tokens::{TextToken, TextTokenIter}, errors::TokenizerError};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum EntryTextToken<'a> {
    TextLiteral(&'a str),
    Separator(&'a str),
}

impl<'a> Display for EntryTextToken<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TextLiteral(text) => write!(f, "TextLiteral: '{text}'"),
            Self::Separator(text) => write!(f, "Separator: '{text}'"),
        }
    }
}

impl<'a> Into<&'a str> for EntryTextToken<'a> {
    #[inline]
    fn into(self) -> &'a str {
        match &self {
            Self::TextLiteral(string) => string,
            Self::Separator(string) => string,
        }
    }
}

impl<'a> Into<&'a str> for &EntryTextToken<'a> {
    #[inline]
    fn into(self) -> &'a str {
        match &self {
            EntryTextToken::TextLiteral(string) => string,
            EntryTextToken::Separator(string) => string,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct EntryTokens<'a> {
    pub text_tokens: Vec<EntryTextToken<'a>>
}

impl<'a> Display for EntryTokens<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "ENTRY TOKENS")?;
        for entry in &self.text_tokens {
            writeln!(f, "\t{}", entry)?;
        }
        Ok(())
    }
}

pub struct EntryTokenIter<'a> {
    token_iter: TextTokenIter<'a>
}

impl<'a> EntryTokenIter<'a> {
    #[inline]
    pub fn new(feed: &'a str) -> Self {
        EntryTokenIter { token_iter: TextTokenIter::new(feed) }
    }
}

impl<'a> Iterator for EntryTokenIter<'a> {
    type Item = Result<EntryTokens<'a>, TokenizerError<'a>>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let mut tokens = Vec::new();
        let mut ignore_new_line = false;
        loop {
            match (self.token_iter.next(), ignore_new_line) {
                // Open parenthesis is used to indicate that newlines should be ignored
                (Some(Ok(TextToken::TextLiteral("("))), true) => return Some(Err(TokenizerError::NestedOpenParenthesis)),
                (Some(Ok(TextToken::TextLiteral("("))), false) => ignore_new_line = true,

                // Closing parenthesis is used to end the indication that newlines should be ignored
                (Some(Ok(TextToken::TextLiteral(")"))), true) => ignore_new_line = false,
                (Some(Ok(TextToken::TextLiteral(")"))), false) => return Some(Err(TokenizerError::UnopenedClosingParenthesis)),

                // Any text literals that are not a part of a comment should be included as part
                // of the entry
                (Some(Ok(TextToken::TextLiteral(token_str))), _) => tokens.push(EntryTextToken::TextLiteral(token_str)),
                (Some(Ok(TextToken::QuotedTextLiteral(token_str))), _) => tokens.push(EntryTextToken::TextLiteral(token_str)),

                // Comments have no meaning
                (Some(Ok(TextToken::Comment(_))), _) => (),

                // Separators are removed at this step. We should only care about the text
                // literals from this point onwards. The only time they matter is if they are
                // the first token.
                (Some(Ok(TextToken::Separator(token_str))), _) if tokens.is_empty() => tokens.push(EntryTextToken::Separator(token_str)),
                (Some(Ok(TextToken::Separator(_))), _) => (),

                (Some(Ok(TextToken::NewLine(_))), true) => (),
                (Some(Ok(TextToken::NewLine(_))), false) => break,

                (None, true) => return Some(Err(TokenizerError::NoClosingParenthesis)),
                (None, false) => return None,

                (Some(Err(error)), _) => return Some(Err(error)),
            }
        }
        
        return Some(Ok(EntryTokens { text_tokens: tokens }));
    }
}

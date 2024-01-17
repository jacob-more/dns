use std::fmt::Display;

use super::{raw_literals::{RawLiteral, RawLiteralIter}, errors::TokenizerError};

/// A version of [RawLiteral] that has fewer variants. This way, impossible states cannot be
/// represented. This version only has [EntryRawLiteral::Text], [EntryRawLiteral::Origin], &
/// [EntryRawLiteral::Separator].
/// - [EntryRawLiteral::Text] is used to represent any string literal, quoted or unquoted. If
///   quoted, the quotation marks are removed.
/// - [EntryRawLiteral::Origin] is used to represent a token that needs to be replaced with the
///   origin.
/// - [EntryRawLiteral::Separator] is used to represent a non-empty sequence of tab and/or space
///   characters.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum EntryRawLiteral<'a> {
    /// A set of ascii characters, including escaped characters, and octal sequences.
    Text(&'a str),
    /// An @ sign from a non-quoted string, which will get replaced by the origin value
    Origin,
    /// Either a single newline character or a return carriage followed by a newline character.
    Separator(&'a str),
}

impl<'a> Display for EntryRawLiteral<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text(text) => write!(f, "Text: '{text}'"),
            Self::Origin => write!(f, "Origin: '@'"),
            Self::Separator(text) => write!(f, "Separator: '{text}'"),
        }
    }
}

impl<'a> Into<&'a str> for EntryRawLiteral<'a> {
    #[inline]
    fn into(self) -> &'a str {
        match &self {
            Self::Text(string) => string,
            Self::Origin => "@",
            Self::Separator(string) => string,
        }
    }
}

impl<'a> Into<&'a str> for &EntryRawLiteral<'a> {
    #[inline]
    fn into(self) -> &'a str {
        match &self {
            EntryRawLiteral::Text(string) => string,
            EntryRawLiteral::Origin => "@",
            EntryRawLiteral::Separator(string) => string,
        }
    }
}

/// A sequence of raw literals that are grouped together into a single entry. At this stage, certain
/// tokens are filtered out so that only meaningful tokens are included. Comments, newlines, and the
/// majority of separators are filtered. If a set of raw literals are enclosed by parenthesis, the
/// tokens in a single entry could span multiple lines.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct RawLiteralEntry<'a> {
    pub entry_raw_literals: Vec<EntryRawLiteral<'a>>
}

impl<'a> Display for RawLiteralEntry<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Raw Literal Entry")?;
        for entry in &self.entry_raw_literals {
            writeln!(f, "\t{entry}")?;
        }
        Ok(())
    }
}

/// Reads in raw literal tokens ([RawLiteral]) that are parsed out of the feed and parses them into
/// basic entries. The output is an iterator of [RawLiteralEntry], where each [RawLiteralEntry]
/// represents something like a raw resource record.
pub struct RawLiteralEntriesIter<'a> {
    raw_literal_iter: RawLiteralIter<'a>
}

impl<'a> RawLiteralEntriesIter<'a> {
    #[inline]
    pub fn new(feed: &'a str) -> Self {
        RawLiteralEntriesIter { raw_literal_iter: RawLiteralIter::new(feed) }
    }
}

impl<'a> Iterator for RawLiteralEntriesIter<'a> {
    type Item = Result<RawLiteralEntry<'a>, TokenizerError<'a>>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let mut tokens = Vec::new();
        let mut ignore_new_line = false;
        loop {
            match (self.raw_literal_iter.next(), ignore_new_line) {
                // Open parenthesis is used to indicate that newlines should be ignored
                (Some(Ok(RawLiteral::Text("("))), true) => return Some(Err(TokenizerError::NestedOpenParenthesis)),
                (Some(Ok(RawLiteral::Text("("))), false) => ignore_new_line = true,

                // Closing parenthesis is used to end the indication that newlines should be ignored
                (Some(Ok(RawLiteral::Text(")"))), true) => ignore_new_line = false,
                (Some(Ok(RawLiteral::Text(")"))), false) => return Some(Err(TokenizerError::UnopenedClosingParenthesis)),

                // Loose, unquoted @, is an origin token
                (Some(Ok(RawLiteral::Text("@"))), _) => tokens.push(EntryRawLiteral::Origin),

                // Any text literals that are not a part of a comment should be included as part
                // of the entry
                (Some(Ok(RawLiteral::Text(token_str))), _) => tokens.push(EntryRawLiteral::Text(token_str)),
                (Some(Ok(RawLiteral::QuotedText(token_str))), _) => tokens.push(EntryRawLiteral::Text(token_str)),

                // Comments have no meaning
                (Some(Ok(RawLiteral::Comment(_))), _) => (),

                // Separators are removed at this step. We should only care about the text
                // literals from this point onwards. The only time they matter is if they are
                // the first token.
                (Some(Ok(RawLiteral::Separator(token_str))), _) if tokens.is_empty() => tokens.push(EntryRawLiteral::Separator(token_str)),
                (Some(Ok(RawLiteral::Separator(_))), _) => (),

                (Some(Ok(RawLiteral::NewLine(_))), true) => (),
                (Some(Ok(RawLiteral::NewLine(_))), false) => break,

                (None, true) => return Some(Err(TokenizerError::NoClosingParenthesis)),
                (None, false) => return None,

                (Some(Err(error)), _) => return Some(Err(error)),
            }
        }
        
        return Some(Ok(RawLiteralEntry { entry_raw_literals: tokens }));
    }
}

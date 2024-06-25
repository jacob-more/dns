use std::fmt::Display;

use super::{raw_literals::{RawLiteral, RawLiteralIter}, errors::TokenizerError};

/// A version of [RawLiteral] that has fewer variants. This way, impossible states cannot be
/// represented. This version only has [RawItem::Text], [RawItem::QuotedText], &
/// [RawItem::Separator].
/// - [RawItem::Text] is used to represent any string literal that is unquoted.
/// - [RawItem::QuotedText] to represent any string literal that is quoted (quotation marks are
///     removed).
/// - [RawItem::Separator] is used to represent a non-empty sequence of tab and/or space characters.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum RawItem<'a> {
    /// A sequence of ascii characters, including escaped characters, and octal sequences.
    Text(&'a str),
    /// A sequence of ascii characters, including escaped characters, and octal sequences. The
    /// sequence does not have special meaning (e.g. '@' does not get read as the origin)
    QuotedText(&'a str),
    /// Either a single newline character or a return carriage followed by a newline character.
    Separator(&'a str),
}

impl<'a> Display for RawItem<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text(text) => write!(f, "Text: '{text}'"),
            Self::QuotedText(text) => write!(f, "QuotedText: '{text}'"),
            Self::Separator(text) => write!(f, "Separator: '{text}'"),
        }
    }
}

impl<'a> Into<&'a str> for RawItem<'a> {
    #[inline]
    fn into(self) -> &'a str {
        match &self {
            Self::Text(string) => string,
            Self::QuotedText(string) => string,
            Self::Separator(string) => string,
        }
    }
}

impl<'a> Into<&'a str> for &RawItem<'a> {
    #[inline]
    fn into(self) -> &'a str {
        match &self {
            RawItem::Text(string) => string,
            RawItem::QuotedText(string) => string,
            RawItem::Separator(string) => string,
        }
    }
}

/// A sequence of raw literals that are grouped together into a single entry. At this stage, certain
/// tokens are filtered out so that only meaningful tokens are included. Comments, newlines, and the
/// majority of separators are filtered. If a set of raw literals are enclosed by parenthesis, the
/// tokens in a single entry could span multiple lines.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct RawEntry<'a> {
    raw_items: Vec<RawItem<'a>>
}

impl<'a> RawEntry<'a> {
    pub fn as_slice(&self) -> &[RawItem<'a>] { self.raw_items.as_slice() }
}

impl<'a> Display for RawEntry<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Raw Entry")?;
        for entry in &self.raw_items {
            writeln!(f, "\t{entry}")?;
        }
        Ok(())
    }
}

/// Reads in raw literal tokens ([RawLiteral]) that are parsed out of the feed and parses them into
/// basic entries. The output is an iterator of [RawLiteralEntry], where each [RawLiteralEntry]
/// represents something like a raw resource record.
pub struct RawEntryIter<'a> {
    raw_literal_iter: RawLiteralIter<'a>
}

impl<'a> RawEntryIter<'a> {
    #[inline]
    pub fn new(feed: &'a str) -> Self {
        Self { raw_literal_iter: RawLiteralIter::new(feed) }
    }
}

impl<'a> Iterator for RawEntryIter<'a> {
    type Item = Result<RawEntry<'a>, TokenizerError<'a>>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let mut entry_items = Vec::new();
        let mut ignore_new_line = false;
        loop {
            match (self.raw_literal_iter.next(), ignore_new_line) {
                // Open parenthesis is used to indicate that newlines should be ignored
                (Some(Ok(RawLiteral::Text("("))), true) => return Some(Err(TokenizerError::NestedOpenParenthesis)),
                (Some(Ok(RawLiteral::Text("("))), false) => ignore_new_line = true,

                // Closing parenthesis is used to end the indication that newlines should be ignored
                (Some(Ok(RawLiteral::Text(")"))), true) => ignore_new_line = false,
                (Some(Ok(RawLiteral::Text(")"))), false) => return Some(Err(TokenizerError::UnopenedClosingParenthesis)),

                // Any text literals that are not a part of a comment should be included as part
                // of the entry
                (Some(Ok(RawLiteral::Text(token_str))), _) => entry_items.push(RawItem::Text(token_str)),
                (Some(Ok(RawLiteral::QuotedText(token_str))), _) => entry_items.push(RawItem::QuotedText(token_str)),

                // Comments have no meaning
                (Some(Ok(RawLiteral::Comment(_))), _) => (),

                // Separators are removed at this step. We should only care about the text
                // literals from this point onwards. The only time they matter is if they are
                // the first token.
                (Some(Ok(RawLiteral::Separator(token_str))), _) if entry_items.is_empty() => entry_items.push(RawItem::Separator(token_str)),
                (Some(Ok(RawLiteral::Separator(_))), _) => (),

                (Some(Ok(RawLiteral::NewLine(_))), true) => (),
                (Some(Ok(RawLiteral::NewLine(_))), false) => break,

                (None, true) => return Some(Err(TokenizerError::NoClosingParenthesis)),
                (None, false) => return None,

                (Some(Err(error)), _) => return Some(Err(error)),
            }
        }
        
        return Some(Ok(RawEntry { raw_items: entry_items }));
    }
}

use std::fmt::Display;

use super::{errors::TokenizerError, regex::{REGEX_CHARACTER_STR_UNQUOTED, REGEX_CHARACTER_STR_QUOTED, REGEX_SEPARATOR, REGEX_NEW_LINE, REGEX_COMMENT}};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TextToken<'a> {
    TextLiteral(&'a str),
    Separator(&'a str),
    NewLine(&'a str),
    Comment(&'a str),
}

impl<'a> Display for TextToken<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TextToken::TextLiteral(text) => write!(f, "TextLiteral: '{text}'"),
            TextToken::Separator(text) => write!(f, "Separator: '{text}'"),
            TextToken::NewLine(text) => write!(f, "NewLine: '{text}'"),
            TextToken::Comment(text) => write!(f, "Comment: '{text}'"),
        }
    }
}

impl<'a> Into<&'a str> for TextToken<'a> {
    fn into(self) -> &'a str {
        match &self {
            TextToken::TextLiteral(string) => string,
            TextToken::Separator(string) => string,
            TextToken::NewLine(string) => string,
            TextToken::Comment(string) => string,
        }
    }
}

impl<'a> Into<&'a str> for &TextToken<'a> {
    fn into(self) -> &'a str {
        match &self {
            TextToken::TextLiteral(string) => string,
            TextToken::Separator(string) => string,
            TextToken::NewLine(string) => string,
            TextToken::Comment(string) => string,
        }
    }
}

pub struct TextTokenIter<'a> {
    feed: &'a str
}

impl<'a> TextTokenIter<'a> {
    pub fn new(feed: &'a str) -> Self {
        TextTokenIter { feed }
    }
}

impl<'a> Iterator for TextTokenIter<'a> {
    type Item = Result<TextToken<'a>, TokenizerError<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        // Once the input feed is empty, there are no more characters to consume.
        if self.feed.is_empty() {
            return None;
        }

        // Case 1: The next token is a string literal
        if let Some(next_literal) = REGEX_CHARACTER_STR_UNQUOTED.find(self.feed) {
            let result = TextToken::TextLiteral(&self.feed[..next_literal.end()]);
            self.feed = &self.feed[next_literal.end()..];
            return Some(Ok(result));
        }
        if let Some(next_literal) = REGEX_CHARACTER_STR_QUOTED.find(self.feed) {
            // Exclude quotation marks from actual token string
            let result = TextToken::TextLiteral(&self.feed[1..(next_literal.end()-1)]);
            self.feed = &self.feed[next_literal.end()..];
            return Some(Ok(result));
        }

        // Case 2: The next token is a separator
        if let Some(next_separator) = REGEX_SEPARATOR.find(self.feed) {
            let result = TextToken::Separator(&self.feed[..(next_separator.end())]);
            self.feed = &self.feed[next_separator.end()..];
            return Some(Ok(result));
        }

        // Case 3: The next token is a new line
        if let Some(next_new_line) = REGEX_NEW_LINE.find(self.feed) {
            let result = TextToken::NewLine(&self.feed[..next_new_line.end()]);
            self.feed = &self.feed[next_new_line.end()..];
            return Some(Ok(result));
        }

        // Case 4: The next token is a comment
        if let Some(next_comment) = REGEX_COMMENT.find(self.feed) {
            let result = TextToken::Comment(&self.feed[..next_comment.end()]);
            self.feed = &self.feed[next_comment.end()..];
            return Some(Ok(result));
        }

        return Some(Err(TokenizerError::UnknownToken(self.feed)));
    }
}

use std::fmt::Display;

use super::{errors::TokenizerError, regex::{REGEX_CHARACTER_STR_UNQUOTED, REGEX_CHARACTER_STR_QUOTED, REGEX_SEPARATOR, REGEX_NEW_LINE, REGEX_COMMENT}};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TextToken<'a> {
    TextLiteral(&'a str),
    Separator(&'a str),
    NewLine(&'a str),
    Comment(&'a str),
    End,
}

impl<'a> Display for TextToken<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TextToken::TextLiteral(text) => write!(f, "TextLiteral: '{text}'"),
            TextToken::Separator(text) => write!(f, "Separator: '{text}'"),
            TextToken::NewLine(text) => write!(f, "NewLine: '{text}'"),
            TextToken::Comment(text) => write!(f, "Comment: '{text}'"),
            TextToken::End => write!(f, "End"),
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
            TextToken::End => "",
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
            TextToken::End => "",
        }
    }
}

impl<'a> TextToken<'a> {
    #[inline]
    pub fn parse_next(feed: &'a str) -> Result<(Self, &'a str), TokenizerError> {
        // Once the input feed is empty, there are no more characters to consume.
        if feed.is_empty() {
            return Ok((TextToken::End, feed));
        }

        // Case 1: The next token is a string literal
        if let Some(next_literal) = REGEX_CHARACTER_STR_UNQUOTED.find(feed) {
            return Ok((TextToken::TextLiteral(&feed[..next_literal.end()]), &feed[next_literal.end()..]));
        }
        if let Some(next_literal) = REGEX_CHARACTER_STR_QUOTED.find(feed) {
            // Exclude quotation marks from actual token string
            return Ok((TextToken::TextLiteral(&feed[1..(next_literal.end()-1)]), &feed[next_literal.end()..]));
        }

        // Case 2: The next token is a separator
        if let Some(next_separator) = REGEX_SEPARATOR.find(feed) {
            return Ok((TextToken::Separator(&feed[..next_separator.end()]), &feed[next_separator.end()..]));
        }

        // Case 3: The next token is a new line
        if let Some(next_new_line) = REGEX_NEW_LINE.find(feed) {
            return Ok((TextToken::NewLine(&feed[..next_new_line.end()]), &feed[next_new_line.end()..]));
        }

        // Case 4: The next token is a comment
        if let Some(next_comment) = REGEX_COMMENT.find(feed) {
            return Ok((TextToken::Comment(&feed[..next_comment.end()]), &feed[next_comment.end()..]));
        }

        return Err(TokenizerError::UnknownToken(feed));
    }
}

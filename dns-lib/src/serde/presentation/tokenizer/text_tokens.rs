use std::fmt::Display;

use super::{errors::TokenizerError, regex::{REGEX_CHARACTER_STR_UNQUOTED, REGEX_CHARACTER_STR_QUOTED, REGEX_SEPARATOR, REGEX_NEW_LINE, REGEX_COMMENT}};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TextToken<'a> {
    TextLiteral(&'a str),
    QuotedTextLiteral(&'a str),
    Separator(&'a str),
    NewLine(&'a str),
    Comment(&'a str),
}

impl<'a> Display for TextToken<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TextLiteral(text) => write!(f, "TextLiteral: '{text}'"),
            Self::QuotedTextLiteral(text) => write!(f, "QuotedTextLiteral: '{text}'"),
            Self::Separator(text) => write!(f, "Separator: '{text}'"),
            Self::NewLine(text) => write!(f, "NewLine: '{text}'"),
            Self::Comment(text) => write!(f, "Comment: '{text}'"),
        }
    }
}

impl<'a> Into<&'a str> for TextToken<'a> {
    #[inline]
    fn into(self) -> &'a str {
        match &self {
            Self::TextLiteral(string) => string,
            Self::QuotedTextLiteral(string) => string,
            Self::Separator(string) => string,
            Self::NewLine(string) => string,
            Self::Comment(string) => string,
        }
    }
}

impl<'a> Into<&'a str> for &TextToken<'a> {
    #[inline]
    fn into(self) -> &'a str {
        match &self {
            TextToken::TextLiteral(string) => string,
            TextToken::QuotedTextLiteral(string) => string,
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
    #[inline]
    pub fn new(feed: &'a str) -> Self {
        TextTokenIter { feed }
    }
}

impl<'a> Iterator for TextTokenIter<'a> {
    type Item = Result<TextToken<'a>, TokenizerError<'a>>;

    #[inline]
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
            let result = TextToken::QuotedTextLiteral(&self.feed[1..(next_literal.end()-1)]);
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

#[cfg(test)]
mod test_text_tokens {
    use crate::serde::presentation::tokenizer::text_tokens::{TextToken, TextTokenIter};

    macro_rules! gen_ok_test {
        ($test_name:ident, $input:expr, $expected:expr) => {
            #[test]
            fn $test_name() {
                let expected = $expected;
                let input = $input;

                let mut iterator = TextTokenIter::new(&input);
                let first_output = iterator.next();
                let second_output = iterator.next();

                assert!(first_output.is_some());
                let first_token_result = first_output.unwrap();
                assert!(first_token_result.is_ok());
                let first_token = first_token_result.unwrap();
                assert_eq!(first_token, expected);

                assert!(second_output.is_none());
            }
        };
        ($test_name:ident, $input:expr, $expected1:expr, $expected2:expr) => {
            #[test]
            fn $test_name() {
                let expected1 = $expected1;
                let expected2 = $expected2;
                let input = $input;

                let mut iterator = TextTokenIter::new(&input);
                let first_output = iterator.next();
                let second_output = iterator.next();
                let third_output = iterator.next();

                assert!(first_output.is_some());
                let first_token_result = first_output.unwrap();
                assert!(first_token_result.is_ok());
                let first_token = first_token_result.unwrap();
                assert_eq!(first_token, expected1);

                assert!(second_output.is_some());
                let second_token_result = second_output.unwrap();
                assert!(second_token_result.is_ok());
                let second_token = second_token_result.unwrap();
                assert_eq!(second_token, expected2);

                assert!(third_output.is_none());
            }
        };
        ($test_name:ident, $input:expr, $expected1:expr, $expected2:expr, $expected3:expr) => {
            #[test]
            fn $test_name() {
                let expected1 = $expected1;
                let expected2 = $expected2;
                let expected3 = $expected3;
                let input = $input;

                let mut iterator = TextTokenIter::new(&input);
                let first_output = iterator.next();
                let second_output = iterator.next();
                let third_output = iterator.next();
                let fourth_output = iterator.next();

                assert!(first_output.is_some());
                let first_token_result = first_output.unwrap();
                assert!(first_token_result.is_ok());
                let first_token = first_token_result.unwrap();
                assert_eq!(first_token, expected1);

                assert!(second_output.is_some());
                let second_token_result = second_output.unwrap();
                assert!(second_token_result.is_ok());
                let second_token = second_token_result.unwrap();
                assert_eq!(second_token, expected2);

                assert!(third_output.is_some());
                let third_token_result = third_output.unwrap();
                assert!(third_token_result.is_ok());
                let third_token = third_token_result.unwrap();
                assert_eq!(third_token, expected3);

                assert!(fourth_output.is_none());
            }
        };
    }

    // Check the simplest cases
    gen_ok_test!(test_text_literal, "abcdefghijklmnopqrstuvwxyz", TextToken::TextLiteral("abcdefghijklmnopqrstuvwxyz"));
    gen_ok_test!(test_text_quoted_literal, "\"abcdefghijklmnopqrstuvwxyz\"", TextToken::QuotedTextLiteral("abcdefghijklmnopqrstuvwxyz"));
    gen_ok_test!(test_text_space, " ", TextToken::Separator(" "));
    gen_ok_test!(test_text_tab, "\t", TextToken::Separator("\t"));
    gen_ok_test!(test_text_new_line, "\n", TextToken::NewLine("\n"));
    gen_ok_test!(test_text_comment, ";this is a comment", TextToken::Comment(";this is a comment"));

    gen_ok_test!(test_text_space_and_comment, " ;this is a comment", TextToken::Separator(" "), TextToken::Comment(";this is a comment"));
    gen_ok_test!(test_text_tab_and_comment, "\t;this is a comment", TextToken::Separator("\t"), TextToken::Comment(";this is a comment"));
    gen_ok_test!(test_text_comment_and_newline, ";this is a comment\n", TextToken::Comment(";this is a comment"), TextToken::NewLine("\n"));

    // Check that parenthesis are parsed as expected
    gen_ok_test!(test_text_open_parenthesis, "(", TextToken::TextLiteral("("));
    gen_ok_test!(test_text_close_parenthesis, ")", TextToken::TextLiteral(")"));
    gen_ok_test!(test_text_quoted_open_parenthesis, "\"(\"", TextToken::QuotedTextLiteral("("));
    gen_ok_test!(test_text_quoted_close_parenthesis, "\")\"", TextToken::QuotedTextLiteral(")"));

    gen_ok_test!(test_text_before_open_parenthesis, "preamble(", TextToken::TextLiteral("preamble"), TextToken::TextLiteral("("));
    gen_ok_test!(test_text_before_close_parenthesis, "preamble)", TextToken::TextLiteral("preamble"), TextToken::TextLiteral(")"));
    gen_ok_test!(test_text_before_quoted_open_parenthesis, "\"preamble(\"", TextToken::QuotedTextLiteral("preamble("));
    gen_ok_test!(test_text_before_quoted_close_parenthesis, "\"preamble)\"", TextToken::QuotedTextLiteral("preamble)"));

    gen_ok_test!(test_text_after_open_parenthesis, "(post-amble", TextToken::TextLiteral("("), TextToken::TextLiteral("post-amble"));
    gen_ok_test!(test_text_after_close_parenthesis, ")post-amble", TextToken::TextLiteral(")"), TextToken::TextLiteral("post-amble"));
    gen_ok_test!(test_text_after_quoted_open_parenthesis, "\"(post-amble\"", TextToken::QuotedTextLiteral("(post-amble"));
    gen_ok_test!(test_text_after_quoted_close_parenthesis, "\")post-amble\"", TextToken::QuotedTextLiteral(")post-amble"));

    gen_ok_test!(test_text_around_open_parenthesis, "preamble(post-amble", TextToken::TextLiteral("preamble"), TextToken::TextLiteral("("), TextToken::TextLiteral("post-amble"));
    gen_ok_test!(test_text_around_close_parenthesis, "preamble)post-amble", TextToken::TextLiteral("preamble"), TextToken::TextLiteral(")"), TextToken::TextLiteral("post-amble"));
    gen_ok_test!(test_text_around_quoted_open_parenthesis, "\"preamble(post-amble\"", TextToken::QuotedTextLiteral("preamble(post-amble"));
    gen_ok_test!(test_text_around_quoted_close_parenthesis, "\"preamble)post-amble\"", TextToken::QuotedTextLiteral("preamble)post-amble"));
}

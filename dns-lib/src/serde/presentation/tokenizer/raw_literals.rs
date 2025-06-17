use std::fmt::Display;

use super::{
    errors::TokenizerError,
    regex::{
        REGEX_CHARACTER_STR_QUOTED, REGEX_CHARACTER_STR_UNQUOTED, REGEX_COMMENT, REGEX_NEW_LINE,
        REGEX_SEPARATOR,
    },
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum RawLiteral<'a> {
    /// A non-empty contiguous set of ascii characters, including escaped characters, and octal
    /// sequences. Certain characters cannot be included unless escaped, such as spaces & tabs
    /// (indicate token separators), semicolons (indicate comments), newline & carriage (indicate
    /// line separators), double-quotes (indicate quoted strings), open & close parenthesis
    /// (indicate multi-line records).
    Text(&'a str),
    /// A contiguous set of ascii characters, including escaped characters, and octal sequences, all
    /// enclosed by a pair of double-quotes. Inside the quotations, all characters are legal except
    /// for non-escaped double-quotations and backslashes. This sequence CAN be empty.
    QuotedText(&'a str),
    /// A non-empty sequence of spaces and/or tabs.
    Separator(&'a str),
    /// Either a single newline character or a return carriage followed by a newline character.
    NewLine(&'a str),
    /// A semi-colon followed by any sequence of characters. The only character that cannot occur
    /// after the semicolon is the newline.
    Comment(&'a str),
}

impl<'a> Display for RawLiteral<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text(text) => write!(f, "Text: '{text}'"),
            Self::QuotedText(text) => write!(f, "QuotedText: '{text}'"),
            Self::Separator(text) => write!(f, "Separator: '{text}'"),
            Self::NewLine(text) => write!(f, "NewLine: '{text}'"),
            Self::Comment(text) => write!(f, "Comment: '{text}'"),
        }
    }
}

impl<'a> Into<&'a str> for RawLiteral<'a> {
    #[inline]
    fn into(self) -> &'a str {
        match &self {
            Self::Text(string) => string,
            Self::QuotedText(string) => string,
            Self::Separator(string) => string,
            Self::NewLine(string) => string,
            Self::Comment(string) => string,
        }
    }
}

impl<'a> Into<&'a str> for &RawLiteral<'a> {
    #[inline]
    fn into(self) -> &'a str {
        match &self {
            RawLiteral::Text(string) => string,
            RawLiteral::QuotedText(string) => string,
            RawLiteral::Separator(string) => string,
            RawLiteral::NewLine(string) => string,
            RawLiteral::Comment(string) => string,
        }
    }
}

/// Reads in the raw string feed and parses it into basic raw literals based on basic rules, each
/// explained in the descriptions of the tokens: [RawLiteral::Text], [RawLiteral::QuotedText],
/// [RawLiteral::Separator], [RawLiteral::NewLine], & [RawLiteral::Comment].
pub struct RawLiteralIter<'a> {
    feed: &'a str,
}

impl<'a> RawLiteralIter<'a> {
    #[inline]
    pub fn new(feed: &'a str) -> Self {
        RawLiteralIter { feed }
    }
}

impl<'a> Iterator for RawLiteralIter<'a> {
    type Item = Result<RawLiteral<'a>, TokenizerError>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        // Once the input feed is empty, there are no more characters to consume.
        if self.feed.is_empty() {
            return None;
        }

        // Case 1: The next token is a string literal
        if let Some(next_literal) = REGEX_CHARACTER_STR_UNQUOTED.find(self.feed) {
            let result = RawLiteral::Text(&self.feed[..next_literal.end()]);
            self.feed = &self.feed[next_literal.end()..];
            return Some(Ok(result));
        }
        if let Some(next_literal) = REGEX_CHARACTER_STR_QUOTED.find(self.feed) {
            // Exclude quotation marks from actual token string
            let result = RawLiteral::QuotedText(&self.feed[1..(next_literal.end() - 1)]);
            self.feed = &self.feed[next_literal.end()..];
            return Some(Ok(result));
        }

        // Case 2: The next token is a separator
        if let Some(next_separator) = REGEX_SEPARATOR.find(self.feed) {
            let result = RawLiteral::Separator(&self.feed[..(next_separator.end())]);
            self.feed = &self.feed[next_separator.end()..];
            return Some(Ok(result));
        }

        // Case 3: The next token is a new line
        if let Some(next_new_line) = REGEX_NEW_LINE.find(self.feed) {
            let result = RawLiteral::NewLine(&self.feed[..next_new_line.end()]);
            self.feed = &self.feed[next_new_line.end()..];
            return Some(Ok(result));
        }

        // Case 4: The next token is a comment
        if let Some(next_comment) = REGEX_COMMENT.find(self.feed) {
            let result = RawLiteral::Comment(&self.feed[..next_comment.end()]);
            self.feed = &self.feed[next_comment.end()..];
            return Some(Ok(result));
        }

        return Some(Err(TokenizerError::UnknownToken(self.feed.to_string())));
    }
}

#[cfg(test)]
mod test_text_tokens {
    use crate::serde::presentation::tokenizer::raw_literals::{RawLiteral, RawLiteralIter};

    macro_rules! gen_ok_test {
        ($test_name:ident, $input:expr, [$($expected:expr),*]) => {
            #[test]
            fn $test_name() {
                let expected = [$($expected),*];
                let input = $input;

                let mut iterator = RawLiteralIter::new(&input);
                for i in 0..expected.len() {
                    let next_output = iterator.next();

                    assert!(next_output.is_some());
                    let token_result = next_output.unwrap();
                    assert!(token_result.is_ok());
                    let token = token_result.unwrap();
                    assert_eq!(token, expected[i]);
                }

                assert!(iterator.next().is_none());
            }
        };
    }

    // Check the simplest cases
    gen_ok_test!(
        test_text_literal,
        "abcdefghijklmnopqrstuvwxyz",
        [RawLiteral::Text("abcdefghijklmnopqrstuvwxyz")]
    );
    gen_ok_test!(
        test_text_quoted_literal,
        "\"abcdefghijklmnopqrstuvwxyz\"",
        [RawLiteral::QuotedText("abcdefghijklmnopqrstuvwxyz")]
    );
    gen_ok_test!(test_text_space, " ", [RawLiteral::Separator(" ")]);
    gen_ok_test!(test_text_tab, "\t", [RawLiteral::Separator("\t")]);
    gen_ok_test!(test_text_new_line, "\n", [RawLiteral::NewLine("\n")]);
    gen_ok_test!(
        test_text_comment,
        ";this is a comment",
        [RawLiteral::Comment(";this is a comment")]
    );

    gen_ok_test!(
        test_text_space_and_comment,
        " ;this is a comment",
        [
            RawLiteral::Separator(" "),
            RawLiteral::Comment(";this is a comment")
        ]
    );
    gen_ok_test!(
        test_text_tab_and_comment,
        "\t;this is a comment",
        [
            RawLiteral::Separator("\t"),
            RawLiteral::Comment(";this is a comment")
        ]
    );
    gen_ok_test!(
        test_text_comment_and_newline,
        ";this is a comment\n",
        [
            RawLiteral::Comment(";this is a comment"),
            RawLiteral::NewLine("\n")
        ]
    );

    gen_ok_test!(
        test_text_newline_separated_tabs,
        "\t\n\t",
        [
            RawLiteral::Separator("\t"),
            RawLiteral::NewLine("\n"),
            RawLiteral::Separator("\t")
        ]
    );
    gen_ok_test!(
        test_text_newline_separated_spaces,
        " \n ",
        [
            RawLiteral::Separator(" "),
            RawLiteral::NewLine("\n"),
            RawLiteral::Separator(" ")
        ]
    );

    gen_ok_test!(
        test_text_literal_escaped_chars,
        r"\a\b\c\d\e\f\g\h\i\j\k\l\m\n\o\p\q\r\s\t\u\v\w\x\y\z\ \!\@\#\$\%\^\&\*\(\)\{\}\[\]\|\\\:\;\'\~\`",
        [RawLiteral::Text(
            r"\a\b\c\d\e\f\g\h\i\j\k\l\m\n\o\p\q\r\s\t\u\v\w\x\y\z\ \!\@\#\$\%\^\&\*\(\)\{\}\[\]\|\\\:\;\'\~\`"
        )]
    );
    gen_ok_test!(
        test_text_literal_octal_chars,
        r"\000\001\002\003\004\005\006\007\010\011\376\377",
        [RawLiteral::Text(
            r"\000\001\002\003\004\005\006\007\010\011\376\377"
        )]
    );

    gen_ok_test!(
        test_text_quoted_literal_escaped_chars,
        "\"\\a\\b\\c\\d\\e\\f\\g\\h\\i\\j\\k\\l\\m\\n\\o\\p\\q\\r\\s\\t\\u\\v\\w\\x\\y\\z\\ \\!\\@\\#\\$\\%\\^\\&\\*\\(\\)\\{\\}\\[\\]\\|\\\\\\:\\;\\'\\~\\`\"",
        [RawLiteral::QuotedText(
            r"\a\b\c\d\e\f\g\h\i\j\k\l\m\n\o\p\q\r\s\t\u\v\w\x\y\z\ \!\@\#\$\%\^\&\*\(\)\{\}\[\]\|\\\:\;\'\~\`"
        )]
    );
    gen_ok_test!(
        test_text_quoted_literal_octal_chars,
        "\"\\000\\001\\002\\003\\004\\005\\006\\007\\010\\011\\376\\377\"",
        [RawLiteral::QuotedText(
            r"\000\001\002\003\004\005\006\007\010\011\376\377"
        )]
    );

    // Check that parenthesis are parsed as expected
    gen_ok_test!(test_text_open_parenthesis, "(", [RawLiteral::Text("(")]);
    gen_ok_test!(test_text_close_parenthesis, ")", [RawLiteral::Text(")")]);
    gen_ok_test!(
        test_text_quoted_open_parenthesis,
        "\"(\"",
        [RawLiteral::QuotedText("(")]
    );
    gen_ok_test!(
        test_text_quoted_close_parenthesis,
        "\")\"",
        [RawLiteral::QuotedText(")")]
    );

    gen_ok_test!(
        test_text_before_open_parenthesis,
        "preamble(",
        [RawLiteral::Text("preamble"), RawLiteral::Text("(")]
    );
    gen_ok_test!(
        test_text_before_close_parenthesis,
        "preamble)",
        [RawLiteral::Text("preamble"), RawLiteral::Text(")")]
    );
    gen_ok_test!(
        test_text_before_quoted_open_parenthesis,
        "\"preamble(\"",
        [RawLiteral::QuotedText("preamble(")]
    );
    gen_ok_test!(
        test_text_before_quoted_close_parenthesis,
        "\"preamble)\"",
        [RawLiteral::QuotedText("preamble)")]
    );

    gen_ok_test!(
        test_text_after_open_parenthesis,
        "(post-amble",
        [RawLiteral::Text("("), RawLiteral::Text("post-amble")]
    );
    gen_ok_test!(
        test_text_after_close_parenthesis,
        ")post-amble",
        [RawLiteral::Text(")"), RawLiteral::Text("post-amble")]
    );
    gen_ok_test!(
        test_text_after_quoted_open_parenthesis,
        "\"(post-amble\"",
        [RawLiteral::QuotedText("(post-amble")]
    );
    gen_ok_test!(
        test_text_after_quoted_close_parenthesis,
        "\")post-amble\"",
        [RawLiteral::QuotedText(")post-amble")]
    );

    gen_ok_test!(
        test_text_around_open_parenthesis,
        "preamble(post-amble",
        [
            RawLiteral::Text("preamble"),
            RawLiteral::Text("("),
            RawLiteral::Text("post-amble")
        ]
    );
    gen_ok_test!(
        test_text_around_close_parenthesis,
        "preamble)post-amble",
        [
            RawLiteral::Text("preamble"),
            RawLiteral::Text(")"),
            RawLiteral::Text("post-amble")
        ]
    );
    gen_ok_test!(
        test_text_around_quoted_open_parenthesis,
        "\"preamble(post-amble\"",
        [RawLiteral::QuotedText("preamble(post-amble")]
    );
    gen_ok_test!(
        test_text_around_quoted_close_parenthesis,
        "\"preamble)post-amble\"",
        [RawLiteral::QuotedText("preamble)post-amble")]
    );
}

use std::{fmt::Display, io, error::Error};

use lazy_static::lazy_static;
use regex::Regex;

#[derive(Debug)]
pub enum TokenizerError<'a> {
    IOError(io::Error),
    NestedOpenParenthesis,
    UnopenedClosingParenthesis,
    NoClosingParenthesis,
    UnknownToken(&'a str),
    UnknownTokens(&'a str, &'a str),
}
impl<'a> Error for TokenizerError<'a> {}
impl<'a> Display for TokenizerError<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IOError(error) => write!(f, "{error}"),
            Self::NestedOpenParenthesis => write!(f, "an open parenthesis was used within a block of parenthesis. Only 1 set of parenthesis may be used at a time"),
            Self::UnopenedClosingParenthesis => write!(f, "a closing parenthesis was used without a matching opening parenthesis"),
            Self::NoClosingParenthesis => write!(f, "an opening parenthesis was used without a closing parenthesis"),
            Self::UnknownToken(token) => write!(f, "unknown token '{token}'"),
            Self::UnknownTokens(token1, token2) => write!(f, "unknown tokens '{token1}' and '{token2}'"),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Origin<'a> {
    pub origin: &'a str,
}

impl<'a> Display for Origin<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "ORIGIN")?;
        writeln!(f, "\tDomain Name: {}", self.origin)
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Include<'a> {
    pub file_name: &'a str,
    pub domain_name: Option<&'a str>,
}

impl<'a> Display for Include<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "INCLUDE")?;
        writeln!(f, "\tFile Name: {}", self.file_name)?;
        if let Some(domain_name) = &self.domain_name {
            writeln!(f, "\tDomain Name: {}", domain_name)?;
        }
        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ResourceRecord<'a> {
    pub domain_name: Option<&'a str>,
    pub ttl: Option<&'a str>,
    pub rclass: Option<&'a str>,
    pub rtype: &'a str,
    pub rdata: Vec<&'a str>,
}

impl<'a> Display for ResourceRecord<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Resource Record")?;
        if let Some(domain_name) = &self.domain_name {
            writeln!(f, "\tDomain Name: {}", domain_name)?;
        }
        if let Some(ttl) = &self.ttl {
            writeln!(f, "\tTTL: {}", ttl)?;
        }
        if let Some(rclass) = &self.rclass {
            writeln!(f, "\tClass: {}", rclass)?;
        }
        writeln!(f, "\tType: {}", self.rtype)?;
        for rdata in &self.rdata {
            writeln!(f, "\tRData: {}", rdata)?;
        }
        Ok(())
    }
}

const RCLASS_STR: &str = r"\A((IN)|(CS)|(CH)|(HS)|(NONE)|(ANY)|(CLASS[[:digit:]]+))\z";
const RTYPE_STR: &str = r"\A(([A-Z]+)|(TYPE[[:digit:]]+))\z";
const TTL_STR: &str = r"\A([[:digit:]]+)\z";

lazy_static! {
    static ref REGEX_RCLASS: Regex = Regex::new(RCLASS_STR).unwrap();
    static ref REGEX_RTYPE: Regex = Regex::new(RTYPE_STR).unwrap();
    static ref REGEX_TTL: Regex = Regex::new(TTL_STR).unwrap();
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Entry<'a> {
    Origin(Origin<'a>),
    Include(Include<'a>),
    ResourceRecord(ResourceRecord<'a>),
    Empty,
}

impl<'a> Display for Entry<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Entry::Origin(token) => write!(f, "{token}"),
            Entry::Include(token) => write!(f, "{token}"),
            Entry::ResourceRecord(token) => write!(f, "{token}"),
            Entry::Empty => write!(f, "Empty Entry"),
        }
    }
}

impl<'a> Entry<'a> {
    #[inline]
    pub fn parse(mut feed: &'a str) -> Result<Vec<Self>, TokenizerError> {
        let mut next_entry: Self;
        let mut all_entries = Vec::new();
        loop {
            (next_entry, feed) = Self::parse_next(feed)?;
            all_entries.push(next_entry);
            if feed.is_empty() {
                break;
            }
        }
        return Ok(all_entries);
    }

    fn new_rr<'b>(domain_name: Option<&'a str>, ttl: Option<&'a str>, rclass: Option<&'a str>, rtype: &'a str, rdata: impl Iterator<Item = &'b TextToken<'a>>) -> Entry<'a> where 'a: 'b {
        Entry::ResourceRecord(ResourceRecord {
            domain_name,
            ttl,
            rclass,
            rtype,
            rdata: rdata.map(|token| token.into()).collect(),
        })
    }

    #[inline]
    pub fn parse_next(mut feed: &'a str) -> Result<(Self, &'a str), TokenizerError<'a>> {
        let entry_tokens;
        (entry_tokens, feed) = EntryTokens::parse_next(feed)?;

        match entry_tokens.text_tokens.as_slice() {
            // <blank>[<comment>]
            &[] => Ok((Self::Empty, feed)),

            // $ORIGIN <domain-name> [<comment>]
            &[TextToken::TextLiteral("$ORIGIN"), TextToken::TextLiteral(domain_name)] => Ok((
                Entry::Origin(Origin {
                    origin: domain_name
                }),
                feed
            )),

            // $INCLUDE <file-name> [<domain-name>] [<comment>]
            &[TextToken::TextLiteral("$INCLUDE"), TextToken::TextLiteral(file_name)] => Ok((
                Entry::Include(Include {
                    file_name: file_name,
                    domain_name: None,
                }),
                feed
            )),
            &[TextToken::TextLiteral("$INCLUDE"), TextToken::TextLiteral(file_name), TextToken::TextLiteral(domain_name)] => Ok((
                Entry::Include(Include {
                    file_name: file_name,
                    domain_name: Some(domain_name),
                }),
                feed
            )),

            // <domain-name> [<TTL>] [<class>] <type> <RDATA> [<comment>]
            &[TextToken::TextLiteral(domain_name), ..] => Self::parse_rr(Some(domain_name), &entry_tokens.text_tokens[1..], feed),
            // <blank> [<TTL>] [<class>] <type> <RDATA> [<comment>]
            &[TextToken::Separator(_), ..] => Self::parse_rr(None, &entry_tokens.text_tokens[1..], feed),

            _ => return Err(TokenizerError::UnknownToken(feed)),
        }
    }

    fn parse_rr(domain_name: Option<&'a str>, other_tokens: &[TextToken<'a>], feed: &'a str) -> Result<(Self, &'a str), TokenizerError<'a>> {
        match other_tokens {
            &[TextToken::TextLiteral(token_1), TextToken::TextLiteral(token_2), ..] => {
                // If the first token is an rtype, then the rest is the rdata and we should not read it
                if REGEX_RTYPE.is_match(token_1) {
                    return Self::parse_rr_rtype_first(domain_name, token_1, &other_tokens[1..], feed);
                }

                if REGEX_RTYPE.is_match(token_2) {
                    return Self::parse_rr_rtype_second(domain_name, token_1, token_2, &other_tokens[2..], feed);
                }

                // The match case only covers a minimum of 2 tokens. This case can only happen if
                // there are at least 3.
                if other_tokens.len() >= 3 {
                    let token_3 = other_tokens[3].into();
                    if REGEX_RTYPE.is_match(token_3) {
                        return Self::parse_rr_rtype_third(domain_name, token_1, token_2, token_3, &other_tokens[3..], feed);
                    } else {
                        return Err(TokenizerError::UnknownToken(token_3));
                    }
                }

                return Err(TokenizerError::UnknownToken(feed));
            },
            _ => return Err(TokenizerError::UnknownToken(feed)),
        }
    }

    fn parse_rr_rtype_first(domain_name: Option<&'a str>, rtype: &'a str, other_tokens: &[TextToken<'a>], feed: &'a str) -> Result<(Self, &'a str), TokenizerError<'a>> {
        Ok((
            Entry::new_rr(
                domain_name,
                None,
                None,
                rtype,
                other_tokens.iter()
            ),
            feed,
        ))
    }

    fn parse_rr_rtype_second(domain_name: Option<&'a str>, token_1: &'a str, rtype: &'a str, other_tokens: &[TextToken<'a>], feed: &'a str) -> Result<(Self, &'a str), TokenizerError<'a>> {
        if REGEX_RCLASS.is_match(token_1) {
            Ok((
                Entry::new_rr(
                    domain_name,
                    None,
                    Some(token_1),
                    rtype,
                    other_tokens.iter()
                ),
                feed,
            ))
        } else if REGEX_TTL.is_match(token_1) {
            Ok((
                Entry::new_rr(
                    None,
                    Some(token_1),
                    None,
                    rtype,
                    other_tokens.iter()
                ),
                feed,
            ))
        } else {
            Err(TokenizerError::UnknownToken(token_1))
        }
    }

    fn parse_rr_rtype_third(domain_name: Option<&'a str>, token_1: &'a str, token_2: &'a str, rtype: &'a str, other_tokens: &[TextToken<'a>], feed: &'a str) -> Result<(Self, &'a str), TokenizerError<'a>> {
        if REGEX_RCLASS.is_match(token_1) && REGEX_TTL.is_match(token_2) {
            Ok((
                Entry::new_rr(
                    domain_name,
                    Some(token_2),
                    Some(token_1),
                    rtype,
                    other_tokens.iter()
                ),
                feed,
            ))
        } else if REGEX_TTL.is_match(token_1) && REGEX_RCLASS.is_match(token_2) {
            Ok((
                Entry::new_rr(
                    None,
                    Some(token_1),
                    Some(token_2),
                    rtype,
                    other_tokens.iter()
                ),
                feed,
            ))
        } else {
            Err(TokenizerError::UnknownTokens(token_1, token_2))
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct EntryTokens<'a> {
    text_tokens: Vec<TextToken<'a>>
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

impl<'a> EntryTokens<'a> {
    #[inline]
    pub fn parse(mut feed: &'a str) -> Result<Vec<Self>, TokenizerError> {
        let mut next_entry: Self;
        let mut all_entries = Vec::new();
        loop {
            (next_entry, feed) = Self::parse_next(feed)?;
            all_entries.push(next_entry);
            if feed.is_empty() {
                break;
            }
        }
        return Ok(all_entries);
    }

    #[inline]
    pub fn parse_next(mut feed: &'a str) -> Result<(Self, &'a str), TokenizerError> {
        let mut tokens = Vec::new();
        'non_empty_entry: loop {
            let mut ignore_new_line = false;
            let mut next_token;
            'read_single_entry: loop {
                (next_token, feed) = TextToken::parse_next(feed)?;
                match (&next_token, ignore_new_line) {
                    // Open parenthesis is used to indicate that newlines should be ignored
                    (TextToken::TextLiteral("("), true) => return Err(TokenizerError::NestedOpenParenthesis),
                    (TextToken::TextLiteral("("), false) => ignore_new_line = true,

                    // Closing parenthesis is used to end the indication that newlines should be ignored
                    (TextToken::TextLiteral(")"), true) => ignore_new_line = false,
                    (TextToken::TextLiteral(")"), false) => return Err(TokenizerError::UnopenedClosingParenthesis),

                    // Any text literals that are not a part of a comment should be included as part
                    // of the entry
                    (TextToken::TextLiteral(_), _) => tokens.push(next_token),

                    // Comments have no meaning
                    (TextToken::Comment(_), _) => (),

                    // Separators are removed at this step. We should only care about the text
                    // literals from this point onwards. The only time they matter is if they are
                    // the first token.
                    (TextToken::Separator(_), _) if (tokens.len() == 0) => tokens.push(next_token),
                    (TextToken::Separator(_), _) => (),

                    (TextToken::NewLine(_), true) => (),
                    (TextToken::NewLine(_), false) => break 'read_single_entry,

                    (TextToken::End, true) => return Err(TokenizerError::NoClosingParenthesis),
                    (TextToken::End, false) => break 'non_empty_entry,
                }
            }
            // If an entry would have had zero tokens in it, keep parsing. This way, we minimize the
            // number of empty entries we generate.
            // If the line only has spaces/tabs, then we clear it and try again.
            if !tokens.is_empty() {
                break 'non_empty_entry;
            } else if let Some(TextToken::Separator(_)) = tokens.first() {
                if tokens.len() == 1 {
                    tokens.clear();
                }
            }
        }
        
        Ok((Self { text_tokens: tokens }, feed))
    }
}

const CHARACTER_STR_UNQUOTED: &str = "\\A(([[:ascii:]&&[^ ;\\r\\n\"\\(\\)]])+)";
const CHARACTER_STR_QUOTED: &str = "\\A(\"(([[:ascii:]&&[^\"]]|(\\\"))*)\")";

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
    pub fn parse(mut feed: &'a str) -> Result<Vec<Self>, TokenizerError> {
        let mut next_token: Self;
        let mut all_tokens = Vec::new();
        loop {
            (next_token, feed) = Self::parse_next(feed)?;
            if let Self::End = next_token {
                all_tokens.push(next_token);
                return Ok(all_tokens);
            }
            all_tokens.push(next_token);
        }
    }

    #[inline]
    pub fn parse_next(feed: &'a str) -> Result<(Self, &'a str), TokenizerError> {
        lazy_static! {
            static ref REGEX_CHARACTER_STR_UNQUOTED: Regex = Regex::new(CHARACTER_STR_UNQUOTED).unwrap();
            static ref REGEX_CHARACTER_STR_QUOTED: Regex = Regex::new(CHARACTER_STR_QUOTED).unwrap();
            static ref REGEX_SEPARATOR: Regex = Regex::new("\\A([ \t]+)").unwrap();
            static ref REGEX_NEW_LINE: Regex = Regex::new("\\A(\r?\n)").unwrap();
            static ref REGEX_COMMENT: Regex = Regex::new("\\A((;)([^\n]*))").unwrap();
        }

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

#[cfg(test)]
mod character_str_regex {
    use regex::Regex;

    use super::{CHARACTER_STR_UNQUOTED, CHARACTER_STR_QUOTED};

    const QUOTED_TESTS: [[&str; 2]; 3] = [
        ["\"This\" is a character string", "\"This\""],
        ["\"This is a\n character\" string", "\"This is a\n character\""],
        ["\"\" string", "\"\""],
    ];
    const UNQUOTED_TESTS: [[&str; 2]; 3] = [
        ["This is a character string", "This"],
        [" This is a character string", "This"],
        ["\nThis is a character string", "This"],
    ];
    const FAIL_TESTS: [&str; 4] = [
        "    ",
        " \n ",
        "  \r\n",
        "\"",
    ];

    #[test]
    fn test_read_quoted_character_string_found() {
        let tests = QUOTED_TESTS;

        let quoted_char_str_regex = Regex::new(CHARACTER_STR_QUOTED).unwrap();
        for [input, expected_output] in tests {
            let actual_output = quoted_char_str_regex.find(input);
            assert!(actual_output.is_some());
            let actual_output = actual_output.unwrap();
            assert_eq!(actual_output.as_str(), expected_output, "Index start: {0}  Index end: {1}", actual_output.start(), actual_output.end());
        }
    }

    #[test]
    fn test_read_unquoted_character_string_found() {
        let tests = UNQUOTED_TESTS;

        let unquoted_char_str_regex = Regex::new(CHARACTER_STR_UNQUOTED).unwrap();
        for [input, expected_output] in tests {
            let actual_output = unquoted_char_str_regex.find(input);
            assert!(actual_output.is_some());
            let actual_output = actual_output.unwrap();
            assert_eq!(actual_output.as_str(), expected_output, "Index start: {0}  Index end: {1}", actual_output.start(), actual_output.end());
        }
    }

    #[test]
    fn test_read_quoted_and_unquoted_character_string_found() {
        let tests = QUOTED_TESTS.iter().chain(UNQUOTED_TESTS.iter());

        let char_str_regex = Regex::new(&format!("(({CHARACTER_STR_UNQUOTED})|({CHARACTER_STR_QUOTED}))")).unwrap();
        for [input, expected_output] in tests {
            let actual_output = char_str_regex.find(input);
            assert!(actual_output.is_some());
            let actual_output = actual_output.unwrap();
            assert_eq!(actual_output.as_str(), *expected_output, "Index start: {0}  Index end: {1}", actual_output.start(), actual_output.end());
        }
    }

    #[test]
    fn test_read_quoted_and_unquoted_character_string_not_found() {
        let tests = FAIL_TESTS;

        let char_str_regex = Regex::new(&format!("(({CHARACTER_STR_UNQUOTED})|({CHARACTER_STR_QUOTED}))")).unwrap();
        for input in tests {
            let actual_output = char_str_regex.find(input);
            assert!(actual_output.is_none(), "Found: {0}", actual_output.unwrap().as_str());
        }
    }
}

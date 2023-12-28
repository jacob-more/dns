use std::fmt::Display;

use super::{text_tokens::TextToken, errors::TokenizerError};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct EntryTokens<'a> {
    pub text_tokens: Vec<TextToken<'a>>
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

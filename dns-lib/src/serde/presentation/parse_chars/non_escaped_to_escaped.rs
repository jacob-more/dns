use crate::types::ascii::{AsciiChar, constants::ASCII_BACKSLASH, is_control_char};

use super::char_token::EscapableChar;

/// Converts from an internal sequence of raw ascii characters, with no escape sequences, and
/// converts it into printable ascii, with escaped backslashes and octal sequences for non-printable
/// characters.
pub struct NonEscapedIntoEscapedIter<T> where T: Iterator<Item = AsciiChar> {
    chars: T
}

impl<T> NonEscapedIntoEscapedIter<T> where T: Iterator<Item = AsciiChar> {
    #[inline]
    pub fn new(iterator: T) -> Self {
        NonEscapedIntoEscapedIter { chars: iterator }
    }
}

impl<T> From<T> for NonEscapedIntoEscapedIter<T> where T: Iterator<Item = AsciiChar> {
    #[inline]
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T> Iterator for NonEscapedIntoEscapedIter<T> where T: Iterator<Item = AsciiChar> {
    type Item = EscapableChar;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self.chars.next() {
            None => None,
            Some(ASCII_BACKSLASH) => Some(EscapableChar::EscapedAscii(ASCII_BACKSLASH)),
            Some(character) if is_control_char(character) => Some(EscapableChar::EscapedOctal(character)),
            Some(character) => Some(EscapableChar::Ascii(character)),
        }
    }
}

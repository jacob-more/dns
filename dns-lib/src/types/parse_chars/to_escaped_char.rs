use std::fmt::Display;

use crate::types::ascii::{AsciiChar, constants::{ASCII_BACKSLASH, ASCII_ZERO}, is_control_char};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum EscapableChar {
    Regular(AsciiChar),
    Escaped(EscapedChar),
}

impl Display for EscapableChar {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EscapableChar::Regular(character) => write!(f, "{}", *character as char),
            EscapableChar::Escaped(escaped_character) => write!(f, "{}", *escaped_character),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum EscapedChar {
    EscapedOctal(AsciiChar),
    EscapedAscii(AsciiChar)
}

impl Display for EscapedChar {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EscapedChar::EscapedAscii(character) => write!(f, "\\{}", *character as char),
            EscapedChar::EscapedOctal(character) => {
                let (char1, char2, char3) = ascii_to_octal(*character);
                write!(f, "\\{}{}{}", char1 as char, char2 as char, char3 as char)
            },
        }
    }
}

#[inline]
const fn ascii_to_octal(character: AsciiChar) -> (AsciiChar, AsciiChar, AsciiChar) {
    (
        (character / 100) + ASCII_ZERO,
        (character / 10) + ASCII_ZERO,
        character + ASCII_ZERO
    )
}

pub struct EscapedCharsIter<T> where T: Iterator<Item = AsciiChar> {
    chars: T
}

impl<T> EscapedCharsIter<T> where T: Iterator<Item = AsciiChar> {
    #[inline]
    pub fn new(iterator: T) -> Self {
        EscapedCharsIter { chars: iterator }
    }
}

impl<T> From<T> for EscapedCharsIter<T> where T: Iterator<Item = AsciiChar> {
    #[inline]
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T> Iterator for EscapedCharsIter<T> where T: Iterator<Item = AsciiChar> {
    type Item = EscapableChar;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self.chars.next() {
            None => None,
            Some(ASCII_BACKSLASH) => Some(EscapableChar::Escaped(EscapedChar::EscapedAscii(ASCII_BACKSLASH))),
            Some(character) if is_control_char(character) => Some(EscapableChar::Escaped(EscapedChar::EscapedOctal(character))),
            Some(character) => Some(EscapableChar::Regular(character)),
        }
    }
}

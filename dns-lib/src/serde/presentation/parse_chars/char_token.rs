use std::fmt::Display;

use crate::types::ascii::{AsciiChar, constants::ASCII_ZERO};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum EscapableChar {
    Ascii(AsciiChar),
    EscapedAscii(AsciiChar),
    EscapedOctal(AsciiChar)
}

impl Display for EscapableChar {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ascii(character) => write!(f, "{}", *character as char),
            Self::EscapedAscii(escaped_character) => write!(f, "\\{}", *escaped_character as char),
            Self::EscapedOctal(escaped_octal) => {
                let (char1, char2, char3) = ascii_to_octal(*escaped_octal);
                write!(f, "\\{}{}{}", char1 as char, char2 as char, char3 as char)
            },
        }
    }
}

#[inline]
pub const fn ascii_to_octal(character: AsciiChar) -> (AsciiChar, AsciiChar, AsciiChar) {
    (
        (character / 100) + ASCII_ZERO,
        (character / 10) + ASCII_ZERO,
        character + ASCII_ZERO
    )
}

#[inline]
pub const fn octal_to_ascii(char1: AsciiChar, char2: AsciiChar, char3: AsciiChar) -> AsciiChar {
    ((char1 - ASCII_ZERO) * 100)
    + ((char2 - ASCII_ZERO) * 10)
    + (char3 - ASCII_ZERO)
}

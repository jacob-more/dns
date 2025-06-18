use std::{error::Error, fmt::Display};

use crate::types::ascii::{
    AsciiChar,
    constants::{ASCII_BACKSLASH, ASCII_NINE, ASCII_ZERO},
};

use super::char_token::{EscapableChar, octal_to_ascii};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum ParseError {
    TrailingEscapeCharacter,

    FirstEscapedCharacterNotADigit {
        char1: AsciiChar,
        char2: AsciiChar,
        char3: AsciiChar,
    },
    SecondEscapedCharacterNotADigit {
        char1: AsciiChar,
        char2: AsciiChar,
        char3: AsciiChar,
    },
    ThirdEscapedCharacterNotADigit {
        char1: AsciiChar,
        char2: AsciiChar,
        char3: AsciiChar,
    },

    FirstAndSecondEscapedCharactersNotDigits {
        char1: AsciiChar,
        char2: AsciiChar,
        char3: AsciiChar,
    },
    FirstAndThirdEscapedCharactersNotDigits {
        char1: AsciiChar,
        char2: AsciiChar,
        char3: AsciiChar,
    },
    SecondAndThirdEscapedCharactersNotDigits {
        char1: AsciiChar,
        char2: AsciiChar,
        char3: AsciiChar,
    },
    NoEscapedCharactersAreDigits {
        char1: AsciiChar,
        char2: AsciiChar,
        char3: AsciiChar,
    },

    OneEscapedCharacterButThreeEscapedDigitsExpected {
        char1: AsciiChar,
    },
    TwoEscapedCharactersButThreeEscapedDigitsExpected {
        char1: AsciiChar,
        char2: AsciiChar,
    },
}
impl Error for ParseError {}
impl Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TrailingEscapeCharacter => write!(
                f,
                "Trailing escape character '\\' not followed by any other characters"
            ),

            Self::FirstEscapedCharacterNotADigit {
                char1,
                char2,
                char3,
            } => write!(
                f,
                "The first escaped character is not a digit ('\\{char1}{char2}{char3}')"
            ),
            Self::SecondEscapedCharacterNotADigit {
                char1,
                char2,
                char3,
            } => write!(
                f,
                "The second escaped character is not a digit ('\\{char1}{char2}{char3}')"
            ),
            Self::ThirdEscapedCharacterNotADigit {
                char1,
                char2,
                char3,
            } => write!(
                f,
                "The third escaped character is not a digit ('\\{char1}{char2}{char3}')"
            ),

            Self::FirstAndSecondEscapedCharactersNotDigits {
                char1,
                char2,
                char3,
            } => write!(
                f,
                "The first & second escaped characters are not digits ('\\{char1}{char2}{char3}')"
            ),
            Self::FirstAndThirdEscapedCharactersNotDigits {
                char1,
                char2,
                char3,
            } => write!(
                f,
                "The first & third escaped characters are not digits ('\\{char1}{char2}{char3}')"
            ),
            Self::SecondAndThirdEscapedCharactersNotDigits {
                char1,
                char2,
                char3,
            } => write!(
                f,
                "The second & third escaped characters are not digits ('\\{char1}{char2}{char3}')"
            ),
            Self::NoEscapedCharactersAreDigits {
                char1,
                char2,
                char3,
            } => write!(
                f,
                "None of the escaped characters are digits ('\\{char1}{char2}{char3}')"
            ),

            Self::OneEscapedCharacterButThreeEscapedDigitsExpected { char1 } => write!(
                f,
                "There was only one escaped character but three escaped digits were expected ('\\{char1}')"
            ),
            Self::TwoEscapedCharactersButThreeEscapedDigitsExpected { char1, char2 } => write!(
                f,
                "There were only two escaped characters but three escaped digits were expected ('\\{char1}{char2}')"
            ),
        }
    }
}

/// Given a sequence of characters, finds escape sequences and turns them into raw ascii. Characters
/// that were previously escaped will output as [`EscapableChar::EscapedAscii`] while octal
/// sequences that were previously escaped will output as [`EscapableChar::EscapedOctal`]. All other
/// characters are output as raw ascii, represented by [`EscapableChar::Ascii`].
pub struct EscapedToEscapableIter<T>
where
    T: Iterator<Item = AsciiChar>,
{
    chars: T,
}

impl<T> EscapedToEscapableIter<T>
where
    T: Iterator<Item = AsciiChar>,
{
    #[inline]
    pub fn new(iterator: T) -> Self {
        EscapedToEscapableIter { chars: iterator }
    }
}

impl<T> From<T> for EscapedToEscapableIter<T>
where
    T: Iterator<Item = AsciiChar>,
{
    #[inline]
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T> Iterator for EscapedToEscapableIter<T>
where
    T: Iterator<Item = AsciiChar>,
{
    type Item = Result<EscapableChar, ParseError>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self.chars.next() {
            Some(ASCII_BACKSLASH) => Self::next_after_escape(self),
            Some(character) => Some(Ok(EscapableChar::Ascii(character))),
            None => None,
        }
    }
}

impl<T> EscapedToEscapableIter<T>
where
    T: Iterator<Item = AsciiChar>,
{
    #[inline]
    fn next_after_escape(&mut self) -> Option<Result<EscapableChar, ParseError>> {
        match self.chars.next() {
            Some(digit @ ASCII_ZERO..=ASCII_NINE) => Self::next_after_escaped_digit(self, digit),
            Some(character) => Some(Ok(EscapableChar::Ascii(character))),
            None => Some(Err(ParseError::TrailingEscapeCharacter)),
        }
    }

    #[inline]
    fn next_after_escaped_digit(
        &mut self,
        first_char: AsciiChar,
    ) -> Option<Result<EscapableChar, ParseError>> {
        match (first_char, self.chars.next(), self.chars.next()) {
            (
                ASCII_ZERO..=ASCII_NINE,
                Some(second_char @ ASCII_ZERO..=ASCII_NINE),
                Some(third_char @ ASCII_ZERO..=ASCII_NINE),
            ) => Some(Ok(EscapableChar::EscapedOctal(octal_to_ascii(
                first_char,
                second_char,
                third_char,
            )))),

            (
                char1,
                Some(char2 @ ASCII_ZERO..=ASCII_NINE),
                Some(char3 @ ASCII_ZERO..=ASCII_NINE),
            ) => Some(Err(ParseError::FirstEscapedCharacterNotADigit {
                char1,
                char2,
                char3,
            })),
            (
                char1 @ ASCII_ZERO..=ASCII_NINE,
                Some(char2),
                Some(char3 @ ASCII_ZERO..=ASCII_NINE),
            ) => Some(Err(ParseError::SecondEscapedCharacterNotADigit {
                char1,
                char2,
                char3,
            })),
            (
                char1 @ ASCII_ZERO..=ASCII_NINE,
                Some(char2 @ ASCII_ZERO..=ASCII_NINE),
                Some(char3),
            ) => Some(Err(ParseError::ThirdEscapedCharacterNotADigit {
                char1,
                char2,
                char3,
            })),

            (char1, Some(char2), Some(char3 @ ASCII_ZERO..=ASCII_NINE)) => {
                Some(Err(ParseError::FirstAndSecondEscapedCharactersNotDigits {
                    char1,
                    char2,
                    char3,
                }))
            }
            (char1, Some(char2 @ ASCII_ZERO..=ASCII_NINE), Some(char3)) => {
                Some(Err(ParseError::FirstAndThirdEscapedCharactersNotDigits {
                    char1,
                    char2,
                    char3,
                }))
            }
            (char1 @ ASCII_ZERO..=ASCII_NINE, Some(char2), Some(char3)) => {
                Some(Err(ParseError::SecondAndThirdEscapedCharactersNotDigits {
                    char1,
                    char2,
                    char3,
                }))
            }
            (char1, Some(char2), Some(char3)) => {
                Some(Err(ParseError::NoEscapedCharactersAreDigits {
                    char1,
                    char2,
                    char3,
                }))
            }

            (char1, Some(char2), None) => Some(Err(
                ParseError::TwoEscapedCharactersButThreeEscapedDigitsExpected { char1, char2 },
            )),
            (char1, None, _) => Some(Err(
                ParseError::OneEscapedCharacterButThreeEscapedDigitsExpected { char1 },
            )),
        }
    }
}

pub struct EscapedCharsEnumerateIter<T>
where
    T: Iterator<Item = (usize, AsciiChar)>,
{
    chars: T,
}

impl<T> EscapedCharsEnumerateIter<T>
where
    T: Iterator<Item = (usize, AsciiChar)>,
{
    #[inline]
    pub fn new(iterator: T) -> Self {
        EscapedCharsEnumerateIter { chars: iterator }
    }
}

impl<T> From<T> for EscapedCharsEnumerateIter<T>
where
    T: Iterator<Item = (usize, AsciiChar)>,
{
    #[inline]
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T> Iterator for EscapedCharsEnumerateIter<T>
where
    T: Iterator<Item = (usize, AsciiChar)>,
{
    type Item = Result<(usize, EscapableChar), ParseError>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self.chars.next() {
            Some((_, ASCII_BACKSLASH)) => Self::next_after_escape(self),
            Some((index, character)) => Some(Ok((index, EscapableChar::Ascii(character)))),
            None => None,
        }
    }
}

impl<T> EscapedCharsEnumerateIter<T>
where
    T: Iterator<Item = (usize, AsciiChar)>,
{
    #[inline]
    fn next_after_escape(&mut self) -> Option<Result<(usize, EscapableChar), ParseError>> {
        let character = self.chars.next();
        match character {
            Some((_, digit @ ASCII_ZERO..=ASCII_NINE)) => {
                Self::next_after_escaped_digit(self, digit)
            }
            Some((index, character)) => Some(Ok((index, EscapableChar::EscapedAscii(character)))),
            None => Some(Err(ParseError::TrailingEscapeCharacter)),
        }
    }

    #[inline]
    fn next_after_escaped_digit(
        &mut self,
        first_char: AsciiChar,
    ) -> Option<Result<(usize, EscapableChar), ParseError>> {
        match (first_char, self.chars.next(), self.chars.next()) {
            (
                ASCII_ZERO..=ASCII_NINE,
                Some((_, second_char @ ASCII_ZERO..=ASCII_NINE)),
                Some((third_index, third_char @ ASCII_ZERO..=ASCII_NINE)),
            ) => Some(Ok((
                third_index,
                EscapableChar::EscapedOctal(octal_to_ascii(first_char, second_char, third_char)),
            ))),

            (
                char1,
                Some((_, char2 @ ASCII_ZERO..=ASCII_NINE)),
                Some((_, char3 @ ASCII_ZERO..=ASCII_NINE)),
            ) => Some(Err(ParseError::FirstEscapedCharacterNotADigit {
                char1,
                char2,
                char3,
            })),
            (
                char1 @ ASCII_ZERO..=ASCII_NINE,
                Some((_, char2)),
                Some((_, char3 @ ASCII_ZERO..=ASCII_NINE)),
            ) => Some(Err(ParseError::SecondEscapedCharacterNotADigit {
                char1,
                char2,
                char3,
            })),
            (
                char1 @ ASCII_ZERO..=ASCII_NINE,
                Some((_, char2 @ ASCII_ZERO..=ASCII_NINE)),
                Some((_, char3)),
            ) => Some(Err(ParseError::ThirdEscapedCharacterNotADigit {
                char1,
                char2,
                char3,
            })),

            (char1, Some((_, char2)), Some((_, char3 @ ASCII_ZERO..=ASCII_NINE))) => {
                Some(Err(ParseError::FirstAndSecondEscapedCharactersNotDigits {
                    char1,
                    char2,
                    char3,
                }))
            }
            (char1, Some((_, char2 @ ASCII_ZERO..=ASCII_NINE)), Some((_, char3))) => {
                Some(Err(ParseError::FirstAndThirdEscapedCharactersNotDigits {
                    char1,
                    char2,
                    char3,
                }))
            }
            (char1 @ ASCII_ZERO..=ASCII_NINE, Some((_, char2)), Some((_, char3))) => {
                Some(Err(ParseError::SecondAndThirdEscapedCharactersNotDigits {
                    char1,
                    char2,
                    char3,
                }))
            }
            (char1, Some((_, char2)), Some((_, char3))) => {
                Some(Err(ParseError::NoEscapedCharactersAreDigits {
                    char1,
                    char2,
                    char3,
                }))
            }

            (char1, Some((_, char2)), None) => Some(Err(
                ParseError::TwoEscapedCharactersButThreeEscapedDigitsExpected { char1, char2 },
            )),
            (char1, None, _) => Some(Err(
                ParseError::OneEscapedCharacterButThreeEscapedDigitsExpected { char1 },
            )),
        }
    }
}

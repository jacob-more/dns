use crate::types::ascii::{AsciiChar, constants::{ASCII_BACKSLASH, ASCII_ZERO, ASCII_NINE}};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum EscapableChar {
    Regular(AsciiChar),
    Escaped(AsciiChar),
}

#[inline]
const fn octal_to_ascii(char1: AsciiChar, char2: AsciiChar, char3: AsciiChar) -> AsciiChar {
    ((char1 - ASCII_ZERO) * 100)
    + ((char2 - ASCII_ZERO) * 10)
    + (char3 - ASCII_ZERO)
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
            Some(ASCII_BACKSLASH) => Self::next_after_escape(self),
            Some(character) => Some(EscapableChar::Regular(character)),
            None => None,
        }
    }
}

impl<T> EscapedCharsIter<T> where T: Iterator<Item = AsciiChar> {
    #[inline]
    fn next_after_escape(&mut self) -> Option<EscapableChar> {
        match self.chars.next() {
            Some(digit @ ASCII_ZERO..=ASCII_NINE) => Self::next_after_escaped_digit(self, digit),
            Some(character) => Some(EscapableChar::Escaped(character)),
            None => panic!("Trailing escape character '\\' not followed by any characters"),
        }
    }

    #[inline]
    fn next_after_escaped_digit(&mut self, first_char: AsciiChar) -> Option<EscapableChar> {
        match (first_char, self.chars.next(), self.chars.next()) {
            (ASCII_ZERO..=ASCII_NINE, Some(second_char @ ASCII_ZERO..=ASCII_NINE), Some(third_char @ ASCII_ZERO..=ASCII_NINE)) => Some(EscapableChar::Escaped(octal_to_ascii(first_char, second_char, third_char))),

            (_, Some(ASCII_ZERO..=ASCII_NINE), Some(ASCII_ZERO..=ASCII_NINE)) => panic!("The first escaped character is not a digit"),
            (ASCII_ZERO..=ASCII_NINE, Some(_), Some(ASCII_ZERO..=ASCII_NINE)) => panic!("The second escaped character is not a digit"),
            (ASCII_ZERO..=ASCII_NINE, Some(ASCII_ZERO..=ASCII_NINE), Some(_)) => panic!("The third escaped character is not a digit"),

            (_, Some(_), Some(ASCII_ZERO..=ASCII_NINE)) => panic!("The first & second escaped characters are not digits"),
            (_, Some(ASCII_ZERO..=ASCII_NINE), Some(_)) => panic!("The first & third escaped characters are not digits"),
            (ASCII_ZERO..=ASCII_NINE, Some(_), Some(_)) => panic!("The second & third escaped characters are not digits"),

            (_, Some(_), Some(_)) => panic!("None of the escaped characters are digits"),

            (_, None, _) => panic!("There was only one escaped character but three escaped digits were expected"),
            (_, _, None) => panic!("There were only two escaped characters but three escaped digits were expected"),
        }
    }
}

pub struct EscapedCharsEnumerateIter<T> where T: Iterator<Item = (usize, AsciiChar)> {
    chars: T
}

impl<T> EscapedCharsEnumerateIter<T> where T: Iterator<Item = (usize, AsciiChar)> {
    #[inline]
    pub fn new(iterator: T) -> Self {
        EscapedCharsEnumerateIter { chars: iterator }
    }
}

impl<T> From<T> for EscapedCharsEnumerateIter<T> where T: Iterator<Item = (usize, AsciiChar)> {
    #[inline]
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T> Iterator for EscapedCharsEnumerateIter<T> where T: Iterator<Item = (usize, AsciiChar)> {
    type Item = (usize, EscapableChar);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self.chars.next() {
            Some((_, ASCII_BACKSLASH)) => Self::next_after_escape(self),
            Some((index, character)) => Some((index, EscapableChar::Regular(character))),
            None => None,
        }
    }
}

impl<T> EscapedCharsEnumerateIter<T> where T: Iterator<Item = (usize, AsciiChar)> {
    #[inline]
    fn next_after_escape(&mut self) -> Option<(usize, EscapableChar)> {
        let character = self.chars.next();
        match character {
            Some((_, digit @ ASCII_ZERO..=ASCII_NINE)) => Self::next_after_escaped_digit(self, digit),
            Some((index, character)) => Some((index, EscapableChar::Escaped(character))),
            None => panic!("Trailing escape character '\\' not followed by any characters"),
        }
    }

    #[inline]
    fn next_after_escaped_digit(&mut self, first_char: AsciiChar) -> Option<(usize, EscapableChar)> {
        match (first_char, self.chars.next(), self.chars.next()) {
            (ASCII_ZERO..=ASCII_NINE, Some((_, second_char @ ASCII_ZERO..=ASCII_NINE)), Some((third_index, third_char @ ASCII_ZERO..=ASCII_NINE))) => Some((third_index, EscapableChar::Escaped(octal_to_ascii(first_char, second_char, third_char)))),

            (_, Some((_, ASCII_ZERO..=ASCII_NINE)), Some((_, ASCII_ZERO..=ASCII_NINE))) => panic!("The first escaped character is not a digit"),
            (ASCII_ZERO..=ASCII_NINE, Some(_), Some((_, ASCII_ZERO..=ASCII_NINE))) => panic!("The second escaped character is not a digit"),
            (ASCII_ZERO..=ASCII_NINE, Some((_, ASCII_ZERO..=ASCII_NINE)), Some(_)) => panic!("The third escaped character is not a digit"),

            (_, Some(_), Some((_, ASCII_ZERO..=ASCII_NINE))) => panic!("The first & second escaped characters are not digits"),
            (_, Some((_, ASCII_ZERO..=ASCII_NINE)), Some(_)) => panic!("The first & third escaped characters are not digits"),
            (ASCII_ZERO..=ASCII_NINE, Some(_), Some(_)) => panic!("The second & third escaped characters are not digits"),

            (_, Some(_), Some(_)) => panic!("None of the escaped characters are digits"),

            (_, None, _) => panic!("There was only one escaped character but three escaped digits were expected"),
            (_, _, None) => panic!("There were only two escaped characters but three escaped digits were expected"),
        }
    }
}

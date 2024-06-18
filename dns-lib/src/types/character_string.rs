use std::{error::Error, fmt::Display, iter::Rev, slice::{Iter, IterMut}};

use lazy_static::lazy_static;

use crate::{serde::{presentation::{errors::TokenError, from_presentation::FromPresentation, to_presentation::ToPresentation}, wire::{from_wire::FromWire, read_wire::SliceWireVisibility, to_wire::ToWire}}, types::ascii::{constants::{ASCII_BACKSLASH, ASCII_CLOSE_PARENTHESIS, ASCII_OPEN_PARENTHESIS, ASCII_SEMICOLON, ASCII_SPACE, EMPTY_ASCII_STRING}, AsciiChar, AsciiError, AsciiString}};

use super::ascii::constants::ASCII_HORIZONTAL_TAB;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum CharacterStringError {
    AsciiError(AsciiError),
    ExceededMaxString,
}

impl Error for CharacterStringError {}
impl Display for CharacterStringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AsciiError(error) => write!(f, "{error}"),
            Self::ExceededMaxString => write!(f, "String Exceeded 255 Bytes in Txt"),
        }
    }
}
impl From<AsciiError> for CharacterStringError {
    fn from(value: AsciiError) -> Self {
        Self::AsciiError(value)
    }
}

/// Implemented as a wrapper around AsciiString, but follows the rules of a DNS
/// character string so this is preferred when those rules need to be followed.
/// 
/// https://datatracker.ietf.org/doc/html/rfc1035#section-3.3
/// 
/// <character-string> is a single length octet followed by that number of characters.
/// <character-string> is treated as binary information, and can be up to 256
/// characters in length (including the length octet).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct CharacterString {
    ascii: AsciiString,
}

impl CharacterString {
    pub const EMPTY: Self = Self { ascii: EMPTY_ASCII_STRING };

    /// The maximum serial length of a character string, excluding the length octet.
    pub const MAX_OCTETS: usize = 255;


    pub fn new(string: AsciiString) -> Result<Self, CharacterStringError> {
        // Bound checking needs to be done to make sure it will be
        // a legal character string.
        if string.len() > Self::MAX_OCTETS {
            return Err(CharacterStringError::ExceededMaxString);
        }

        Ok(Self { ascii: string, })
    }

    #[inline]
    pub fn from_utf8(string: &str) -> Result<Self, CharacterStringError> {
        Self::new(
            AsciiString::from_utf8(string)?
        )
    }
    
    #[inline]
    pub fn len(&self) -> usize {
        self.ascii.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.ascii.is_empty()
    }

    #[inline]
    pub fn get(&self, index: usize) -> Option<&AsciiChar> {
        self.ascii.get(index)
    }

    #[inline]
    pub fn last(&self) -> Option<&AsciiChar> {
        self.ascii.last()
    }

    #[inline]
    pub fn last_mut(&mut self) -> Option<&mut AsciiChar> {
        self.ascii.last_mut()
    }

    #[inline]
    pub fn first(&self) -> Option<&AsciiChar> {
        self.ascii.first()
    }

    #[inline]
    pub fn first_mut(&mut self) -> Option<&mut AsciiChar> {
        self.ascii.first_mut()
    }

    #[inline]
    pub fn push(&mut self, character: AsciiChar) -> Result<(), CharacterStringError> {
        // Bound checking needs to be done to make sure it will still be
        // a legal character string.
        if self.ascii.len() + 1 > Self::MAX_OCTETS {
            return Err(CharacterStringError::ExceededMaxString);
        }

        self.ascii.push(character);
        return Ok(());
    }

    #[inline]
    pub fn pop(&mut self) -> Option<AsciiChar> {
        self.ascii.pop()
    }

    #[inline]
    pub fn insert(&mut self, index: usize, character: AsciiChar) -> Result<(), CharacterStringError> {
        // Bound checking needs to be done to make sure it will still be
        // a legal character string.
        if self.ascii.len() + 1 > Self::MAX_OCTETS {
            return Err(CharacterStringError::ExceededMaxString);
        }

        self.ascii.insert(index, character);
        return Ok(());
    }

    #[inline]
    pub fn remove(&mut self, index: usize) -> AsciiChar {
        self.ascii.remove(index)
    }

    #[inline]
    pub fn clear(&mut self) {
        self.ascii.clear()
    }

    #[inline]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut AsciiChar> {
        self.ascii.get_mut(index)
    }

    #[inline]
    pub fn swap(&mut self, index1: usize, index2: usize) {
        self.ascii.swap(index1, index2)
    }

    #[inline]
    pub fn reverse(&mut self) {
        self.ascii.reverse()
    }

    #[inline]
    pub fn as_reversed(&self) -> Rev<Iter<'_, AsciiChar>> {
        self.ascii.as_reversed()
    }

    #[inline]
    pub fn iter(&self) -> Iter<'_, AsciiChar> {
        self.ascii.iter()
    }

    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<'_, AsciiChar> {
        self.ascii.iter_mut()
    }

    #[inline]
    pub fn contains(&self, character: &AsciiChar) -> bool {
        self.ascii.contains(character)
    }

    #[inline]
    pub fn as_lower(&self) -> Self {
        Self { ascii: self.ascii.as_lower(), }
    }

    #[inline]
    pub fn lower(&mut self) {
        self.ascii.lower()
    }

    #[inline]
    pub fn as_upper(&self) -> Self {
        Self { ascii: self.ascii.as_upper(), }
    }

    #[inline]
    pub fn upper(&mut self) {
        self.ascii.upper()
    }

    #[inline]
    pub fn is_numeric(&self) -> bool {
        self.ascii.is_numeric()
    }
}

impl Display for CharacterString {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.ascii)
    }
}

impl ToWire for CharacterString {
    #[inline]
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>, compression: &mut Option<crate::serde::wire::compression_map::CompressionMap>) -> Result<(), crate::serde::wire::write_wire::WriteWireError> where 'a: 'b {
        (self.ascii.len() as u8).to_wire_format(wire, compression)?;
        self.ascii.to_wire_format(wire, compression)
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        1 //< For the length octet, not included in a plain AsciiString.
        + self.ascii.serial_length()
    }
}

impl FromWire for CharacterString {
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>) -> Result<Self, crate::serde::wire::read_wire::ReadWireError> where Self: Sized, 'a: 'b {
        let length = u8::from_wire_format(wire)?;

        if (length as usize) > Self::MAX_OCTETS {
            return Err(crate::serde::wire::read_wire::ReadWireError::OutOfBoundsError(
                format!("character strings must be at most {} bytes (including the length byte)", Self::MAX_OCTETS + 1)
            ));
        }

        if wire.current_len() < (length as usize) {
            return Err(crate::serde::wire::read_wire::ReadWireError::OverflowError(
                String::from("wire length runs out before the full string is finished reading")
            ));
        }

        // Since the AsciiString deserializer will consume the entire buffer,
        // we only feed it the section we want it to read.
        let mut ascii_wire = wire.slice_from_current(..(length as usize), SliceWireVisibility::Slice)?;
        let string = AsciiString::from_wire_format(&mut ascii_wire)?;
        wire.shift(length as usize)?;

        Ok(Self { ascii: string })
    }
}

impl FromPresentation for CharacterString {
    #[inline]
    fn from_token_format<'a, 'b, 'c, 'd>(tokens: &'c [&'a str]) -> Result<(Self, &'d [&'a str]), TokenError<'b>> where Self: Sized, 'a: 'b, 'c: 'd, 'c: 'd {
        match tokens {
            &[] => Err(TokenError::OutOfTokens),
            &[token, ..] => Ok((Self::from_utf8(token)?, &tokens[1..])),
        }
    }
}

impl ToPresentation for CharacterString {
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        lazy_static!{
            static ref AT_SIGN: AsciiString = AsciiString::from_utf8(r"@").unwrap();
        }

        if self.ascii == *AT_SIGN {
            out_buffer.push(r"\@".to_string());
            return;
        }

        let out_string = self.ascii.iter()
            .flat_map(|character| match *character {
                ASCII_SPACE | ASCII_HORIZONTAL_TAB | ASCII_SEMICOLON | ASCII_OPEN_PARENTHESIS | ASCII_CLOSE_PARENTHESIS => vec![ASCII_BACKSLASH as char, *character as char],
                _ => vec![*character as char]
            }).collect::<String>();
        
        out_buffer.push(out_string)
    }
}

#[cfg(test)]
mod circular_serde_sanity_test {
    use crate::serde::wire::circular_test::gen_test_circular_serde_sanity_test;
    use super::CharacterString;

    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test,
        CharacterString::from_utf8("This is a character string").unwrap()
    );
    gen_test_circular_serde_sanity_test!(
        one_char_zone_record_circular_serde_sanity_test,
        CharacterString::from_utf8("a").unwrap()
    );
    gen_test_circular_serde_sanity_test!(
        empty_record_circular_serde_sanity_test,
        CharacterString::from_utf8("").unwrap()
    );
}

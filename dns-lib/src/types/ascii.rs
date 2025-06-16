use std::{fmt::{Display, Debug}, slice::{Iter, IterMut}, iter::Rev, error::Error, ops::Add};

use tinyvec::{tiny_vec, TinyVec};

use crate::serde::{presentation::{errors::TokenError, from_presentation::FromPresentation, parse_chars::non_escaped_to_escaped::NonEscapedIntoEscapedIter, to_presentation::ToPresentation}, wire::{from_wire::FromWire, to_wire::ToWire}};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum AsciiError {
    BadChar,
    Buffer,
    Overflow,
}

impl Error for AsciiError {}
impl Display for AsciiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadChar =>  write!(f, "character is not a valid ascii character"),
            Self::Buffer =>   write!(f, "Buffer size too small"),
            Self::Overflow => write!(f, "Overflow Unpacking Txt"),
        }
    }
}

pub type AsciiChar = u8;

pub mod constants {
    use super::AsciiChar;

    pub const ASCII_NUL: AsciiChar                       = 000;
    pub const ASCII_START_OF_HEADING: AsciiChar          = 001;
    pub const ASCII_START_OF_TEXT: AsciiChar             = 002;
    pub const ASCII_END_OF_TEXT: AsciiChar               = 003;
    pub const ASCII_END_OF_TRANSMISSION: AsciiChar       = 004;
    pub const ASCII_ENQUIRY: AsciiChar                   = 005;
    pub const ASCII_ACKNOWLEDGE: AsciiChar               = 006;
    pub const ASCII_BELL: AsciiChar                      = 007;
    pub const ASCII_BACKSPACE: AsciiChar                 = 008;
    pub const ASCII_HORIZONTAL_TAB: AsciiChar            = 009;
    pub const ASCII_LINE_FEED: AsciiChar                 = 010;
    pub const ASCII_VERTICAL_TABULATION: AsciiChar       = 011;
    pub const ASCII_FORM_FEED: AsciiChar                 = 012;
    pub const ASCII_CARRIAGE_RETURN: AsciiChar           = 013;
    pub const ASCII_SHIFT_OUT: AsciiChar                 = 014;
    pub const ASCII_SHIFT_IN: AsciiChar                  = 015;
    pub const ASCII_DATA_LINK_ESCAPE: AsciiChar          = 016;
    pub const ASCII_DEVICE_CONTROL_ONE: AsciiChar        = 017;
    pub const ASCII_DEVICE_CONTROL_TWO: AsciiChar        = 018;
    pub const ASCII_DEVICE_CONTROL_THREE: AsciiChar      = 019;
    pub const ASCII_DEVICE_CONTROL_FOUR: AsciiChar       = 020;
    pub const ASCII_NEGATIVE_ACKNOWLEDGE: AsciiChar      = 021;
    pub const ASCII_SYNCHRONOUS_IDLE: AsciiChar          = 022;
    pub const ASCII_END_OF_TRANSMISSION_BLOCK: AsciiChar = 023;
    pub const ASCII_CANCEL: AsciiChar                    = 024;
    pub const ASCII_END_OF_MEDIUM: AsciiChar             = 025;
    pub const ASCII_SUBSTITUTE: AsciiChar                = 026;
    pub const ASCII_ESCAPE: AsciiChar                    = 027;
    pub const ASCII_FILE_SEPARATOR: AsciiChar            = 028;
    pub const ASCII_GROUP_SEPARATOR: AsciiChar           = 029;
    pub const ASCII_RECORD_SEPARATOR: AsciiChar          = 030;
    pub const ASCII_UNIT_SEPARATOR: AsciiChar            = 031;
    pub const ASCII_SPACE: AsciiChar                     = 032;
    pub const ASCII_EXCLAMATION_MARK: AsciiChar          = 033;
    pub const ASCII_DOUBLE_QUOTES: AsciiChar             = 034;
    pub const ASCII_NUMBER_SIGN: AsciiChar               = 035;
    pub const ASCII_DOLLAR_SIGN: AsciiChar               = 036;
    pub const ASCII_PERCENT_SIGN: AsciiChar              = 037;
    pub const ASCII_AMPERSAND: AsciiChar                 = 038;
    pub const ASCII_SINGLE_QUOTE: AsciiChar              = 039;
    pub const ASCII_OPEN_PARENTHESIS: AsciiChar          = 040;
    pub const ASCII_CLOSE_PARENTHESIS: AsciiChar         = 041;
    pub const ASCII_ASTERISK: AsciiChar                  = 042;
    pub const ASCII_PLUS: AsciiChar                      = 043;
    pub const ASCII_COMMA: AsciiChar                     = 044;
    pub const ASCII_HYPHEN_MINUS: AsciiChar              = 045;
    pub const ASCII_PERIOD: AsciiChar                    = 046;
    pub const ASCII_SLASH: AsciiChar                     = 047;
    pub const ASCII_ZERO: AsciiChar                      = 048;
    pub const ASCII_ONE: AsciiChar                       = 049;
    pub const ASCII_TWO: AsciiChar                       = 050;
    pub const ASCII_THREE: AsciiChar                     = 051;
    pub const ASCII_FOUR: AsciiChar                      = 052;
    pub const ASCII_FIVE: AsciiChar                      = 053;
    pub const ASCII_SIX: AsciiChar                       = 054;
    pub const ASCII_SEVEN: AsciiChar                     = 055;
    pub const ASCII_EIGHT: AsciiChar                     = 056;
    pub const ASCII_NINE: AsciiChar                      = 057;
    pub const ASCII_COLON: AsciiChar                     = 058;
    pub const ASCII_SEMICOLON: AsciiChar                 = 059;
    pub const ASCII_OPEN_ANGLED_BRACKET: AsciiChar       = 060;
    pub const ASCII_EQUALS: AsciiChar                    = 061;
    pub const ASCII_CLOSE_ANGLED_BRACKET: AsciiChar      = 062;
    pub const ASCII_QUESTION_MARK: AsciiChar             = 063;
    pub const ASCII_AT_SIGN: AsciiChar                   = 064;
    pub const ASCII_UPPERCASE_A: AsciiChar               = 065;
    pub const ASCII_UPPERCASE_B: AsciiChar               = 066;
    pub const ASCII_UPPERCASE_C: AsciiChar               = 067;
    pub const ASCII_UPPERCASE_D: AsciiChar               = 068;
    pub const ASCII_UPPERCASE_E: AsciiChar               = 069;
    pub const ASCII_UPPERCASE_F: AsciiChar               = 070;
    pub const ASCII_UPPERCASE_G: AsciiChar               = 071;
    pub const ASCII_UPPERCASE_H: AsciiChar               = 072;
    pub const ASCII_UPPERCASE_I: AsciiChar               = 073;
    pub const ASCII_UPPERCASE_J: AsciiChar               = 074;
    pub const ASCII_UPPERCASE_K: AsciiChar               = 075;
    pub const ASCII_UPPERCASE_L: AsciiChar               = 076;
    pub const ASCII_UPPERCASE_M: AsciiChar               = 077;
    pub const ASCII_UPPERCASE_N: AsciiChar               = 078;
    pub const ASCII_UPPERCASE_O: AsciiChar               = 079;
    pub const ASCII_UPPERCASE_P: AsciiChar               = 080;
    pub const ASCII_UPPERCASE_Q: AsciiChar               = 081;
    pub const ASCII_UPPERCASE_R: AsciiChar               = 082;
    pub const ASCII_UPPERCASE_S: AsciiChar               = 083;
    pub const ASCII_UPPERCASE_T: AsciiChar               = 084;
    pub const ASCII_UPPERCASE_U: AsciiChar               = 085;
    pub const ASCII_UPPERCASE_V: AsciiChar               = 086;
    pub const ASCII_UPPERCASE_W: AsciiChar               = 087;
    pub const ASCII_UPPERCASE_X: AsciiChar               = 088;
    pub const ASCII_UPPERCASE_Y: AsciiChar               = 089;
    pub const ASCII_UPPERCASE_Z: AsciiChar               = 090;
    pub const ASCII_OPENING_BRACKET: AsciiChar           = 091;
    pub const ASCII_BACKSLASH: AsciiChar                 = 092;
    pub const ASCII_CLOSING_BRACKET: AsciiChar           = 093;
    pub const ASCII_CARET: AsciiChar                     = 094;
    pub const ASCII_UNDERSCORE: AsciiChar                = 095;
    pub const ASCII_GRAVE_ACCENT: AsciiChar              = 096;
    pub const ASCII_LOWERCASE_A: AsciiChar               = 097;
    pub const ASCII_LOWERCASE_B: AsciiChar               = 098;
    pub const ASCII_LOWERCASE_C: AsciiChar               = 099;
    pub const ASCII_LOWERCASE_D: AsciiChar               = 100;
    pub const ASCII_LOWERCASE_E: AsciiChar               = 101;
    pub const ASCII_LOWERCASE_F: AsciiChar               = 102;
    pub const ASCII_LOWERCASE_G: AsciiChar               = 103;
    pub const ASCII_LOWERCASE_H: AsciiChar               = 104;
    pub const ASCII_LOWERCASE_I: AsciiChar               = 105;
    pub const ASCII_LOWERCASE_J: AsciiChar               = 106;
    pub const ASCII_LOWERCASE_K: AsciiChar               = 107;
    pub const ASCII_LOWERCASE_L: AsciiChar               = 108;
    pub const ASCII_LOWERCASE_M: AsciiChar               = 109;
    pub const ASCII_LOWERCASE_N: AsciiChar               = 110;
    pub const ASCII_LOWERCASE_O: AsciiChar               = 111;
    pub const ASCII_LOWERCASE_P: AsciiChar               = 112;
    pub const ASCII_LOWERCASE_Q: AsciiChar               = 113;
    pub const ASCII_LOWERCASE_R: AsciiChar               = 114;
    pub const ASCII_LOWERCASE_S: AsciiChar               = 115;
    pub const ASCII_LOWERCASE_T: AsciiChar               = 116;
    pub const ASCII_LOWERCASE_U: AsciiChar               = 117;
    pub const ASCII_LOWERCASE_V: AsciiChar               = 118;
    pub const ASCII_LOWERCASE_W: AsciiChar               = 119;
    pub const ASCII_LOWERCASE_X: AsciiChar               = 120;
    pub const ASCII_LOWERCASE_Y: AsciiChar               = 121;
    pub const ASCII_LOWERCASE_Z: AsciiChar               = 122;
    pub const ASCII_OPENING_BRACE: AsciiChar             = 123;
    pub const ASCII_VERTICAL_BAR: AsciiChar              = 124;
    pub const ASCII_CLOSING_BRACE: AsciiChar             = 125;
    pub const ASCII_TILDE: AsciiChar                     = 126;
    pub const ASCII_DELETE: AsciiChar                    = 127;
}

// TODO: I am not sure whether it is better to do large match statements, like I have now that match
//       each letter to their uppercase/lowercase counterparts or use ranges A..=Z and add/subtract
//       32. Both solutions have merits but I don't know which is better.

#[inline]
pub const fn to_ascii_lowercase(character: &AsciiChar) -> AsciiChar {
    character.to_ascii_lowercase()
}

#[inline]
pub const fn to_ascii_uppercase(character: &AsciiChar) -> AsciiChar {
    character.to_ascii_uppercase()
}

#[inline]
pub const fn is_ascii_uppercase(character: &AsciiChar) -> bool {
    character.is_ascii_uppercase()
}

#[inline]
pub const fn is_ascii_lowercase(character: &AsciiChar) -> bool {
    character.is_ascii_lowercase()
}

#[inline]
pub const fn is_ascii_digit(character: &AsciiChar) -> bool {
    character.is_ascii_digit()
}

#[inline]
pub const fn is_ascii_alphabetic(character: &AsciiChar) -> bool {
    character.is_ascii_alphabetic()
}

#[inline]
pub const fn is_ascii_alphanumeric(character: &AsciiChar) -> bool {
    character.is_ascii_alphanumeric()
}

#[inline]
pub const fn is_ascii_lowercase_alphanumeric(character: &AsciiChar) -> bool {
    character.is_ascii_lowercase() || character.is_ascii_digit()
}

#[inline]
pub const fn is_ascii_uppercase_alphanumeric(character: &AsciiChar) -> bool {
    character.is_ascii_uppercase() || character.is_ascii_digit()
}

#[inline]
pub const fn is_ascii_control(character: &AsciiChar) -> bool {
    character.is_ascii_control()
}

#[inline]
pub const fn is_ascii_printable(character: &AsciiChar) -> bool {
    character.is_ascii_graphic()
}

#[derive(Clone, Default, PartialEq, Eq, Hash)]
pub struct AsciiString {
    string: TinyVec<[AsciiChar; 14]>,
}

impl Display for AsciiString {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for character in NonEscapedIntoEscapedIter::from(self.string.iter().map(|character| *character)) {
            write!(f, "{character}")?;
        }
        Ok(())
    }
}

impl Debug for AsciiString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AsciiString(\"{self}\")")
    }
}

impl AsciiString {
    #[inline]
    pub fn new_empty() -> Self {
        Self { string: tiny_vec![] }
    }

    #[inline]
    pub fn from(string: &[u8]) -> Self {
        Self { string: string.into() }
    }

    #[inline]
    pub fn from_utf8(string: &str) -> Result<Self, AsciiError> {
        let mut ascii_characters = TinyVec::with_capacity(string.len());
        for character in string.chars() {
            let u32_character = character as u32;
            if ((u8::MIN as u32) > u32_character) || ((u8::MAX as u32) < u32_character) {
                return Err(AsciiError::BadChar);
            } else {
                ascii_characters.push(character as u8);
            }
        }
        return Ok(Self {string: ascii_characters});
    }

    #[inline]
    pub fn from_range(&self, start: usize, end: usize) -> Self {
        Self { string: self.string[start..end].into() }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.string.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.string.is_empty()
    }

    #[inline]
    pub fn get(&self, index: usize) -> Option<&AsciiChar> {
        self.string.get(index)
    }

    #[inline]
    pub fn last(&self) -> Option<&AsciiChar> {
        self.string.last()
    }

    #[inline]
    pub fn last_mut(&mut self) -> Option<&mut AsciiChar> {
        self.string.last_mut()
    }

    #[inline]
    pub fn first(&self) -> Option<&AsciiChar> {
        self.string.first()
    }

    #[inline]
    pub fn first_mut(&mut self) -> Option<&mut AsciiChar> {
        self.string.first_mut()
    }

    #[inline]
    pub fn push(&mut self, character: AsciiChar) {
        self.string.push(character);
    }

    #[inline]
    pub fn pop(&mut self) -> Option<AsciiChar> {
        self.string.pop()
    }

    #[inline]
    pub fn insert(&mut self, index: usize, character: AsciiChar) {
        self.string.insert(index, character);
    }

    #[inline]
    pub fn remove(&mut self, index: usize) -> AsciiChar {
        self.string.remove(index)
    }

    #[inline]
    pub fn clear(&mut self) {
        self.string.clear();
    }

    #[inline]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut AsciiChar> {
        self.string.get_mut(index)
    }

    #[inline]
    pub fn swap(&mut self, index1: usize, index2: usize) {
        self.string.swap(index1, index2)
    }

    #[inline]
    pub fn reverse(&mut self) {
        self.string.reverse();
    }

    #[inline]
    pub fn as_reversed(&self) -> Rev<Iter<'_, AsciiChar>> {
        self.iter().rev()
    }

    #[inline]
    pub fn iter(&self) -> Iter<'_, AsciiChar> {
        self.string.iter()
    }

    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<'_, AsciiChar> {
        self.string.iter_mut()
    }

    #[inline]
    pub fn strip_suffix(&self, suffix: &Self) -> Option<Self> {
        let suffix = suffix.string.as_slice();
        let (len, n) = (self.len(), suffix.len());
        if n <= len {
            let (head, tail) = self.string.split_at(len - n);
            if tail == suffix {
                return Some(Self { string: head.into() });
            }
        }
        None
    }

    #[inline]
    pub fn strip_prefix(&self, prefix: &Self) -> Option<Self> {
        let prefix = prefix.string.as_slice();
        let n = prefix.len();
        if n <= self.len() {
            let (head, tail) = self.string.split_at(n);
            if head == prefix {
                return Some(Self { string: tail.into() });
            }
        }
        None
    }

    #[inline]
    pub fn contains(&self, character: &AsciiChar) -> bool {
        self.string.contains(character)
    }

    #[inline]
    pub fn as_slice(&self) -> &[AsciiChar] {
        self.string.as_slice()
    }

    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [AsciiChar] {
        self.string.as_mut_slice()
    }

    #[inline]
    pub fn to_vec(&self) -> Vec<AsciiChar> {
        self.string.to_vec()
    }

    #[inline]
    pub fn into_vec(self) -> Vec<AsciiChar> {
        self.string.into_vec()
    }

    #[inline]
    pub fn as_lowercase(&self) -> Self {
        let mut string = self.string.clone();
        string.make_ascii_lowercase();
        Self { string }
    }

    #[inline]
    pub fn make_lowercase(&mut self) {
        self.string.make_ascii_lowercase();
    }

    #[inline]
    pub fn as_uppercase(&self) -> Self {
        let mut string = self.string.clone();
        string.make_ascii_uppercase();
        Self { string }
    }

    #[inline]
    pub fn make_uppercase(&mut self) {
        self.string.make_ascii_uppercase();
    }

    #[inline]
    pub fn is_alphabetic_or_empty(&self) -> bool {
        self.string.iter().all(|character| is_ascii_digit(character))
    }

    #[inline]
    pub fn is_numeric_or_empty(&self) -> bool {
        self.string.iter().all(|character| is_ascii_digit(character))
    }

    #[inline]
    pub fn is_alphanumeric_or_empty(&self) -> bool {
        self.string.iter().all(|character| is_ascii_alphanumeric(character))
    }

    #[inline]
    pub fn is_lowercase_alphanumeric_or_empty(&self) -> bool {
        self.string.iter().all(|character| is_ascii_lowercase_alphanumeric(character))
    }

    #[inline]
    pub fn is_uppercase_alphanumeric_or_empty(&self) -> bool {
        self.string.iter().all(|character| is_ascii_uppercase_alphanumeric(character))
    }

    #[inline]
    pub fn is_alphabetic(&self) -> bool {
        (!self.string.is_empty()) && self.is_alphabetic_or_empty()
    }

    #[inline]
    pub fn is_numeric(&self) -> bool {
        (!self.string.is_empty()) && self.is_numeric_or_empty()
    }

    #[inline]
    pub fn is_alphanumeric(&self) -> bool {
        (!self.string.is_empty()) && self.is_alphanumeric_or_empty()
    }

    #[inline]
    pub fn is_lowercase_alphanumeric(&self) -> bool {
        (!self.string.is_empty()) && self.is_lowercase_alphanumeric_or_empty()
    }

    #[inline]
    pub fn is_uppercase_alphanumeric(&self) -> bool {
        (!self.string.is_empty()) && self.is_uppercase_alphanumeric_or_empty()
    }
}

impl Add for AsciiString {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        let mut string = self.string.clone();
        string.extend(rhs.string);

        Self { string: string, }
    }
}

impl ToWire for AsciiString {
    #[inline]
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>, _compression: &mut Option<crate::types::c_domain_name::CompressionMap>) -> Result<(), crate::serde::wire::write_wire::WriteWireError> where 'a: 'b {
        wire.write_bytes(&self.string)
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        self.string.len() as u16
    }
}

impl FromWire for AsciiString {
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>) -> Result<Self, crate::serde::wire::read_wire::ReadWireError> where Self: Sized, 'a: 'b {
        let string = Self::from(wire.take_all());
        return Ok(string);
    }
}

impl FromPresentation for AsciiString {
    #[inline]
    fn from_token_format<'a, 'b, 'c, 'd>(tokens: &'c [&'a str]) -> Result<(Self, &'d [&'a str]), TokenError> where Self: Sized, 'a: 'b, 'c: 'd, 'c: 'd {
        match tokens {
            &[] => Err(TokenError::OutOfTokens),
            &[token, ..] => Ok((Self::from_utf8(token)?, &tokens[1..])),
        }
    }
}

impl ToPresentation for AsciiString {
    #[inline]
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        out_buffer.push(
            self.string.iter()
                .map(|character| *character as char)
                .collect::<String>()
        )
    }
}

#[cfg(test)]
mod circular_serde_sanity_test {
    use crate::serde::wire::circular_test::gen_test_circular_serde_sanity_test;
    use super::AsciiString;

    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test,
        AsciiString::from_utf8("This is a character string").unwrap()
    );
    gen_test_circular_serde_sanity_test!(
        one_char_zone_record_circular_serde_sanity_test,
        AsciiString::from_utf8("a").unwrap()
    );
    gen_test_circular_serde_sanity_test!(
        empty_record_circular_serde_sanity_test,
        AsciiString::from_utf8("").unwrap()
    );
}

#[cfg(test)]
mod upper_lower_case_tests {
    use super::AsciiString;

    const MIXED_CASE_STRING: &str = "This Is A Test aAbBcCdDeEfFgGhHiIjJkKlLmMnNoOpPqQrRsStTuUvVwWxXyYzZ 0123456789 !@#$%^&*()";
    const UPPER_STRING: &str = "THIS IS A TEST AABBCCDDEEFFGGHHIIJJKKLLMMNNOOPPQQRRSSTTUUVVWWXXYYZZ 0123456789 !@#$%^&*()";
    const LOWER_STRING: &str = "this is a test aabbccddeeffgghhiijjkkllmmnnooppqqrrssttuuvvwwxxyyzz 0123456789 !@#$%^&*()";

    #[test]
    fn test_as_upper_success() {
        // Setup
        let ascii_mixed_case = AsciiString::from_utf8(MIXED_CASE_STRING).expect("The mixed case string could not convert from utf8 to ascii");
        let ascii_upper_case = AsciiString::from_utf8(UPPER_STRING).expect("The upper case string could not convert from utf8 to ascii");
        let ascii_lower_case = AsciiString::from_utf8(LOWER_STRING).expect("The lower case string could not convert from utf8 to ascii");

        let expected_string = AsciiString::from_utf8(UPPER_STRING).expect("The upper case string could not convert from utf8 to ascii");

        // Test & Check
        assert_eq!(expected_string, ascii_mixed_case.as_uppercase());
        assert_eq!(expected_string, ascii_upper_case.as_uppercase());
        assert_eq!(expected_string, ascii_lower_case.as_uppercase());
    }

    #[test]
    fn test_upper_success() {
        // Setup
        let mut ascii_mixed_case = AsciiString::from_utf8(MIXED_CASE_STRING).expect("The mixed case string could not convert from utf8 to ascii");
        let mut ascii_upper_case = AsciiString::from_utf8(UPPER_STRING).expect("The upper case string could not convert from utf8 to ascii");
        let mut ascii_lower_case = AsciiString::from_utf8(LOWER_STRING).expect("The lower case string could not convert from utf8 to ascii");

        let expected_string = AsciiString::from_utf8(UPPER_STRING).expect("The upper case string could not convert from utf8 to ascii");

        // Test & Check
        ascii_mixed_case.make_uppercase();
        ascii_upper_case.make_uppercase();
        ascii_lower_case.make_uppercase();

        assert_eq!(expected_string, ascii_mixed_case);
        assert_eq!(expected_string, ascii_upper_case);
        assert_eq!(expected_string, ascii_lower_case);
    }

    #[test]
    fn test_as_lower_success() {
        // Setup
        let ascii_mixed_case = AsciiString::from_utf8(MIXED_CASE_STRING).expect("The mixed case string could not convert from utf8 to ascii");
        let ascii_upper_case = AsciiString::from_utf8(UPPER_STRING).expect("The upper case string could not convert from utf8 to ascii");
        let ascii_lower_case = AsciiString::from_utf8(LOWER_STRING).expect("The lower case string could not convert from utf8 to ascii");

        let expected_string = AsciiString::from_utf8(LOWER_STRING).expect("The upper case string could not convert from utf8 to ascii");

        // Test & Check
        assert_eq!(expected_string, ascii_mixed_case.as_lowercase());
        assert_eq!(expected_string, ascii_upper_case.as_lowercase());
        assert_eq!(expected_string, ascii_lower_case.as_lowercase());
    }

    #[test]
    fn test_lower_success() {
        // Setup
        let mut ascii_mixed_case = AsciiString::from_utf8(MIXED_CASE_STRING).expect("The mixed case string could not convert from utf8 to ascii");
        let mut ascii_upper_case = AsciiString::from_utf8(UPPER_STRING).expect("The upper case string could not convert from utf8 to ascii");
        let mut ascii_lower_case = AsciiString::from_utf8(LOWER_STRING).expect("The lower case string could not convert from utf8 to ascii");

        let expected_string = AsciiString::from_utf8(LOWER_STRING).expect("The upper case string could not convert from utf8 to ascii");

        // Test & Check
        ascii_mixed_case.make_lowercase();
        ascii_upper_case.make_lowercase();
        ascii_lower_case.make_lowercase();

        assert_eq!(expected_string, ascii_mixed_case);
        assert_eq!(expected_string, ascii_upper_case);
        assert_eq!(expected_string, ascii_lower_case);
    }

    const MIXED_CASE_ALPHA: &str = "aAbBcCdDeEfFgGhHiIjJkKlLmMnNoOpPqQrRsStTuUvVwWxXyYzZ";
    const UPPER_STRING_ALPHA: &str = "AABBCCDDEEFFGGHHIIJJKKLLMMNNOOPPQQRRSSTTUUVVWWXXYYZZ";
    const LOWER_STRING_ALPHA: &str = "aabbccddeeffgghhiijjkkllmmnnooppqqrrssttuuvvwwxxyyzz";

    const NUMERIC: &str = "0123456789";

    const MIXED_CASE_ALPHA_NUMERIC: &str = "aAbBcCdDeEfFgGhHiIjJkKlLmMnNoOpPqQrRsStTuUvVwWxXyYzZ0123456789";
    const UPPER_STRING_ALPHA_NUMERIC: &str = "AABBCCDDEEFFGGHHIIJJKKLLMMNNOOPPQQRRSSTTUUVVWWXXYYZZ0123456789";
    const LOWER_STRING_ALPHA_NUMERIC: &str = "aabbccddeeffgghhiijjkkllmmnnooppqqrrssttuuvvwwxxyyzz0123456789";

    const MIXED_CASE_NON_ALPHA_NUMERIC: &str = "aAbBcCdDeEfFgGhHiIjJkKlLmMnNoOpPqQrRsStTuUvVwWxXyYzZ0123456789!@#$%^&*()";
    const UPPER_STRING_NON_ALPHA_NUMERIC: &str = "AABBCCDDEEFFGGHHIIJJKKLLMMNNOOPPQQRRSSTTUUVVWWXXYYZZ0123456789!@#$%^&*()";
    const LOWER_STRING_NON_ALPHA_NUMERIC: &str = "aabbccddeeffgghhiijjkkllmmnnooppqqrrssttuuvvwwxxyyzz0123456789!@#$%^&*()";

    #[test]
    fn test_is_alpha_numeric_lower() {
        // Setup
        let ascii_mixed_case_alpha = AsciiString::from_utf8(MIXED_CASE_ALPHA).expect("The mixed case string could not convert from utf8 to ascii");
        let ascii_upper_case_alpha = AsciiString::from_utf8(UPPER_STRING_ALPHA).expect("The upper case string could not convert from utf8 to ascii");
        let ascii_lower_case_alpha = AsciiString::from_utf8(LOWER_STRING_ALPHA).expect("The lower case string could not convert from utf8 to ascii");

        let ascii_numeric = AsciiString::from_utf8(NUMERIC).expect("The numeric string could not convert from utf8 to ascii");

        let ascii_mixed_case_alphanumeric = AsciiString::from_utf8(MIXED_CASE_ALPHA_NUMERIC).expect("The mixed case string could not convert from utf8 to ascii");
        let ascii_upper_case_alphanumeric = AsciiString::from_utf8(UPPER_STRING_ALPHA_NUMERIC).expect("The upper case string could not convert from utf8 to ascii");
        let ascii_lower_case_alphanumeric = AsciiString::from_utf8(LOWER_STRING_ALPHA_NUMERIC).expect("The lower case string could not convert from utf8 to ascii");

        let ascii_mixed_case_non_alphanumeric = AsciiString::from_utf8(MIXED_CASE_NON_ALPHA_NUMERIC).expect("The mixed case string could not convert from utf8 to ascii");
        let ascii_upper_case_non_alphanumeric = AsciiString::from_utf8(UPPER_STRING_NON_ALPHA_NUMERIC).expect("The upper case string could not convert from utf8 to ascii");
        let ascii_lower_case_non_alphanumeric = AsciiString::from_utf8(LOWER_STRING_NON_ALPHA_NUMERIC).expect("The lower case string could not convert from utf8 to ascii");

        // Test & Check
        assert!(!ascii_mixed_case_alpha.is_lowercase_alphanumeric());
        assert!(!ascii_upper_case_alpha.is_lowercase_alphanumeric());
        assert!(ascii_lower_case_alpha.is_lowercase_alphanumeric());

        assert!(ascii_numeric.is_lowercase_alphanumeric());

        assert!(!ascii_mixed_case_alphanumeric.is_lowercase_alphanumeric());
        assert!(!ascii_upper_case_alphanumeric.is_lowercase_alphanumeric());
        assert!(ascii_lower_case_alphanumeric.is_lowercase_alphanumeric());

        assert!(!ascii_mixed_case_non_alphanumeric.is_lowercase_alphanumeric());
        assert!(!ascii_upper_case_non_alphanumeric.is_lowercase_alphanumeric());
        assert!(!ascii_lower_case_non_alphanumeric.is_lowercase_alphanumeric());
    }

    #[test]
    fn test_is_alpha_numeric_upper() {
        // Setup
        let ascii_mixed_case_alpha = AsciiString::from_utf8(MIXED_CASE_ALPHA).expect("The mixed case string could not convert from utf8 to ascii");
        let ascii_upper_case_alpha = AsciiString::from_utf8(UPPER_STRING_ALPHA).expect("The upper case string could not convert from utf8 to ascii");
        let ascii_lower_case_alpha = AsciiString::from_utf8(LOWER_STRING_ALPHA).expect("The lower case string could not convert from utf8 to ascii");

        let ascii_numeric = AsciiString::from_utf8(NUMERIC).expect("The numeric string could not convert from utf8 to ascii");

        let ascii_mixed_case_alphanumeric = AsciiString::from_utf8(MIXED_CASE_ALPHA_NUMERIC).expect("The mixed case string could not convert from utf8 to ascii");
        let ascii_upper_case_alphanumeric = AsciiString::from_utf8(UPPER_STRING_ALPHA_NUMERIC).expect("The upper case string could not convert from utf8 to ascii");
        let ascii_lower_case_alphanumeric = AsciiString::from_utf8(LOWER_STRING_ALPHA_NUMERIC).expect("The lower case string could not convert from utf8 to ascii");

        let ascii_mixed_case_non_alphanumeric = AsciiString::from_utf8(MIXED_CASE_NON_ALPHA_NUMERIC).expect("The mixed case string could not convert from utf8 to ascii");
        let ascii_upper_case_non_alphanumeric = AsciiString::from_utf8(UPPER_STRING_NON_ALPHA_NUMERIC).expect("The upper case string could not convert from utf8 to ascii");
        let ascii_lower_case_non_alphanumeric = AsciiString::from_utf8(LOWER_STRING_NON_ALPHA_NUMERIC).expect("The lower case string could not convert from utf8 to ascii");

        // Test & Check
        assert!(!ascii_mixed_case_alpha.is_uppercase_alphanumeric());
        assert!(ascii_upper_case_alpha.is_uppercase_alphanumeric());
        assert!(!ascii_lower_case_alpha.is_uppercase_alphanumeric());

        assert!(ascii_numeric.is_uppercase_alphanumeric());

        assert!(!ascii_mixed_case_alphanumeric.is_uppercase_alphanumeric());
        assert!(ascii_upper_case_alphanumeric.is_uppercase_alphanumeric());
        assert!(!ascii_lower_case_alphanumeric.is_uppercase_alphanumeric());

        assert!(!ascii_mixed_case_non_alphanumeric.is_uppercase_alphanumeric());
        assert!(!ascii_upper_case_non_alphanumeric.is_uppercase_alphanumeric());
        assert!(!ascii_lower_case_non_alphanumeric.is_uppercase_alphanumeric());
    }

    #[test]
    fn test_is_alpha_numeric() {
        // Setup
        let ascii_mixed_case_alpha = AsciiString::from_utf8(MIXED_CASE_ALPHA).expect("The mixed case string could not convert from utf8 to ascii");
        let ascii_upper_case_alpha = AsciiString::from_utf8(UPPER_STRING_ALPHA).expect("The upper case string could not convert from utf8 to ascii");
        let ascii_lower_case_alpha = AsciiString::from_utf8(LOWER_STRING_ALPHA).expect("The lower case string could not convert from utf8 to ascii");

        let ascii_numeric = AsciiString::from_utf8(NUMERIC).expect("The numeric string could not convert from utf8 to ascii");

        let ascii_mixed_case_alphanumeric = AsciiString::from_utf8(MIXED_CASE_ALPHA_NUMERIC).expect("The mixed case string could not convert from utf8 to ascii");
        let ascii_upper_case_alphanumeric = AsciiString::from_utf8(UPPER_STRING_ALPHA_NUMERIC).expect("The upper case string could not convert from utf8 to ascii");
        let ascii_lower_case_alphanumeric = AsciiString::from_utf8(LOWER_STRING_ALPHA_NUMERIC).expect("The lower case string could not convert from utf8 to ascii");

        let ascii_mixed_case_non_alphanumeric = AsciiString::from_utf8(MIXED_CASE_NON_ALPHA_NUMERIC).expect("The mixed case string could not convert from utf8 to ascii");
        let ascii_upper_case_non_alphanumeric = AsciiString::from_utf8(UPPER_STRING_NON_ALPHA_NUMERIC).expect("The upper case string could not convert from utf8 to ascii");
        let ascii_lower_case_non_alphanumeric = AsciiString::from_utf8(LOWER_STRING_NON_ALPHA_NUMERIC).expect("The lower case string could not convert from utf8 to ascii");

        // Test & Check
        assert!(ascii_mixed_case_alpha.is_alphanumeric());
        assert!(ascii_upper_case_alpha.is_alphanumeric());
        assert!(ascii_lower_case_alpha.is_alphanumeric());

        assert!(ascii_numeric.is_alphanumeric());

        assert!(ascii_mixed_case_alphanumeric.is_alphanumeric());
        assert!(ascii_upper_case_alphanumeric.is_alphanumeric());
        assert!(ascii_lower_case_alphanumeric.is_alphanumeric());

        assert!(!ascii_mixed_case_non_alphanumeric.is_alphanumeric());
        assert!(!ascii_upper_case_non_alphanumeric.is_alphanumeric());
        assert!(!ascii_lower_case_non_alphanumeric.is_alphanumeric());
    }

    #[test]
    fn test_is_numeric() {
        // Setup
        let ascii_mixed_case_alpha = AsciiString::from_utf8(MIXED_CASE_ALPHA).expect("The mixed case string could not convert from utf8 to ascii");
        let ascii_upper_case_alpha = AsciiString::from_utf8(UPPER_STRING_ALPHA).expect("The upper case string could not convert from utf8 to ascii");
        let ascii_lower_case_alpha = AsciiString::from_utf8(LOWER_STRING_ALPHA).expect("The lower case string could not convert from utf8 to ascii");

        let ascii_numeric = AsciiString::from_utf8(NUMERIC).expect("The numeric string could not convert from utf8 to ascii");

        let ascii_mixed_case_alphanumeric = AsciiString::from_utf8(MIXED_CASE_ALPHA_NUMERIC).expect("The mixed case string could not convert from utf8 to ascii");
        let ascii_upper_case_alphanumeric = AsciiString::from_utf8(UPPER_STRING_ALPHA_NUMERIC).expect("The upper case string could not convert from utf8 to ascii");
        let ascii_lower_case_alphanumeric = AsciiString::from_utf8(LOWER_STRING_ALPHA_NUMERIC).expect("The lower case string could not convert from utf8 to ascii");

        let ascii_mixed_case_non_alphanumeric = AsciiString::from_utf8(MIXED_CASE_NON_ALPHA_NUMERIC).expect("The mixed case string could not convert from utf8 to ascii");
        let ascii_upper_case_non_alphanumeric = AsciiString::from_utf8(UPPER_STRING_NON_ALPHA_NUMERIC).expect("The upper case string could not convert from utf8 to ascii");
        let ascii_lower_case_non_alphanumeric = AsciiString::from_utf8(LOWER_STRING_NON_ALPHA_NUMERIC).expect("The lower case string could not convert from utf8 to ascii");

        // Test & Check
        assert!(!ascii_mixed_case_alpha.is_numeric());
        assert!(!ascii_upper_case_alpha.is_numeric());
        assert!(!ascii_lower_case_alpha.is_numeric());

        assert!(ascii_numeric.is_numeric());

        assert!(!ascii_mixed_case_alphanumeric.is_numeric());
        assert!(!ascii_upper_case_alphanumeric.is_numeric());
        assert!(!ascii_lower_case_alphanumeric.is_numeric());

        assert!(!ascii_mixed_case_non_alphanumeric.is_numeric());
        assert!(!ascii_upper_case_non_alphanumeric.is_numeric());
        assert!(!ascii_lower_case_non_alphanumeric.is_numeric());
    }
}

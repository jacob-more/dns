use std::{error::Error, fmt::{Display, Debug}};

use ux::u4;

use crate::{types::{ascii::{AsciiChar, AsciiError, constants::*, AsciiString}, base_conversions::BaseConversions}, serde::presentation::from_presentation::FromPresentation};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Base16Error {
    BadChar(AsciiChar),
    IncorrectBufferSize,
    Overflow,
    Underflow,
    BadPadding,
    AsciiError(AsciiError),
}

impl Error for Base16Error {}
impl Display for Base16Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // TODO: error messages could be improved
            Self::BadChar(character) => write!(f, "Character '{character}' is not a valid base 16 character"),
            Self::IncorrectBufferSize => write!(f, "Buffer size is incorrect"),
            Self::Overflow => write!(f, "Overflow"),
            Self::Underflow => write!(f, "Underflow"),
            Self::BadPadding => write!(f, "Padding can only occur at the end of the encoding and must be exact"),
            Self::AsciiError(ascii_err) => write!(f, "{ascii_err}")
        }
    }
}
impl From<AsciiError> for Base16Error {
    fn from(value: AsciiError) -> Self {
        Self::AsciiError(value)
    }
}

#[inline]
fn u4_to_base16(bits: u4) -> AsciiChar {
    match u8::from(bits) {
        0b0000_0000 => ASCII_ZERO,
        0b0000_0001 => ASCII_ONE,
        0b0000_0010 => ASCII_TWO,
        0b0000_0011 => ASCII_THREE,
        0b0000_0100 => ASCII_FOUR,
        0b0000_0101 => ASCII_FIVE,
        0b0000_0110 => ASCII_SIX,
        0b0000_0111 => ASCII_SEVEN,
        0b0000_1000 => ASCII_EIGHT,
        0b0000_1001 => ASCII_NINE,
        0b0000_1010 => ASCII_UPPERCASE_A,
        0b0000_1011 => ASCII_UPPERCASE_B,
        0b0000_1100 => ASCII_UPPERCASE_C,
        0b0000_1101 => ASCII_UPPERCASE_D,
        0b0000_1110 => ASCII_UPPERCASE_E,
        0b0000_1111 => ASCII_UPPERCASE_F,

        _ => panic!("Illegal State Reached: This should not be possible. The value should convert to a u4 and exhaustively match those values.")
    }
}

#[inline]
const fn base16_to_u4(character: AsciiChar) -> Result<u8, Base16Error> {
    match character {
        ASCII_ZERO        => Ok(0b0000_0000),
        ASCII_ONE         => Ok(0b0000_0001),
        ASCII_TWO         => Ok(0b0000_0010),
        ASCII_THREE       => Ok(0b0000_0011),
        ASCII_FOUR        => Ok(0b0000_0100),
        ASCII_FIVE        => Ok(0b0000_0101),
        ASCII_SIX         => Ok(0b0000_0110),
        ASCII_SEVEN       => Ok(0b0000_0111),
        ASCII_EIGHT       => Ok(0b0000_1000),
        ASCII_NINE        => Ok(0b0000_1001),
        ASCII_UPPERCASE_A => Ok(0b0000_1010),
        ASCII_UPPERCASE_B => Ok(0b0000_1011),
        ASCII_UPPERCASE_C => Ok(0b0000_1100),
        ASCII_UPPERCASE_D => Ok(0b0000_1101),
        ASCII_UPPERCASE_E => Ok(0b0000_1110),
        ASCII_UPPERCASE_F => Ok(0b0000_1111),

        _ => Err(Base16Error::BadChar(character)),
    }
}

#[inline]
const fn is_base16_char(encoded: AsciiChar) -> bool {
    match encoded {
        ASCII_ZERO..=ASCII_NINE => true,
        ASCII_UPPERCASE_A..=ASCII_UPPERCASE_F => true,
        _ => false,
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Base16 {
    bytes: Vec<u8>,
}

impl Display for Base16 {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.decode())
    }
}

impl Debug for Base16 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Base16(\"{}\")", self.decode())
    }
}

impl Base16 {
    #[inline]
    pub fn from_ascii(string: &AsciiString) -> Result<Self, Base16Error> {
        Self::encode(string)
    }

    #[inline]
    pub fn from_case_insensitive_ascii(string: &AsciiString) -> Result<Self, Base16Error> {
        Self::from_ascii(
            &string.as_upper()
        )
    }

    #[inline]
    pub fn from_utf8(string: &str) -> Result<Self, Base16Error> {
        Base16::from_ascii(
            &AsciiString::from_utf8(string)?
        )
    }

    #[inline]
    pub fn from_case_insensitive_utf8(string: &str) -> Result<Self, Base16Error> {
        Base16::from_case_insensitive_ascii(
            &AsciiString::from_utf8(string)?
        )
    }

    #[inline]
    pub fn encode(string: &AsciiString) -> Result<Self, Base16Error> {
        let mut encoded_bytes: Vec<u8> = Vec::with_capacity(string.len());
        let string_chunks = string.as_slice().chunks_exact(2);

        let remainder = string_chunks.remainder();
        if remainder.len() != 0 {
            return Err(Base16Error::Overflow);
        }

        for chunk in string_chunks {
            match chunk {
                &[char1, char2] => {
                    let bits0_3 = (u8::from(base16_to_u4(char1)?) << 4) & 0b1111_0000;
                    let bits4_7 = (u8::from(base16_to_u4(char2)?) << 0) & 0b0000_1111;
                    let merged_bytes = bits0_3 | bits4_7;

                    let [byte1] = u8::to_be_bytes(merged_bytes);

                    encoded_bytes.push(byte1);
                }
                _ => panic!("The pattern was supposed to chunk exactly 8 bytes. However, the chunk contained {} bytes", chunk.len()),
            }
        }

        return Ok(Self { bytes: encoded_bytes });
    }

    #[inline]
    pub fn decode(&self) -> AsciiString {
        let mut decoded_bytes: Vec<u8> = Vec::with_capacity((self.bytes.len() * 16) / 8);

        self.bytes.iter().for_each(|chunk| {
            let merged_bytes = *chunk;

            let bits0_3 = ((merged_bytes & 0b1111_0000) >> 4) as u8;
            let bits4_7 = ((merged_bytes & 0b0000_1111) >> 0) as u8;

            decoded_bytes.extend([
                u4_to_base16(u4::new(bits0_3)),
                u4_to_base16(u4::new(bits4_7)),
            ]);
        });

        AsciiString::from(&decoded_bytes)
    }

    #[inline]
    pub fn verify_base16_string(string: &AsciiString) -> Result<(), Base16Error> {
        // Verify that all of the characters are valid for this alphabet.
        for character in string.iter() {
            if !is_base16_char(*character) {
                return Err(Base16Error::BadChar(*character));
            }
        }

        // Verify that the format does not contain padding characters where they are not allowed.
        let remainder = string.as_vec().chunks_exact(2).remainder();
        if remainder.len() != 0 {
            return Err(Base16Error::Overflow);
        }

        return Ok(());
    }
}

impl BaseConversions for Base16 {
    #[inline]
    fn from_vec(bytes: Vec<u8>) -> Self {
        Self { bytes: bytes }
    }

    #[inline]
    fn to_bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[inline]
    fn to_ascii(&self) -> AsciiString {
        self.decode()
    }

    #[inline]
    fn string_len(&self) -> usize {
        // Base
        let base = (self.bytes.len() * 16).div_ceil(8);
        // + Padding
        base.next_multiple_of(2)
    }
}

impl FromPresentation for Base16 {
    #[inline]
    fn from_token_format<'a, 'b>(token: &'a str) -> Result<Self, crate::serde::presentation::errors::TokenError<'b>> where Self: Sized, 'a: 'b {
        Ok(Self::encode(
            &AsciiString::from_token_format(token)?
        )?)
    }
}

#[cfg(test)]
mod circular_sanity_tests {
    use crate::{types::base_conversions::BaseConversions, serde::wire::circular_test::gen_test_circular_serde_sanity_test};
    use super::Base16;

    macro_rules! circular_sanity_test {
        ($test_encoding:ident, $test_wire:ident, $input:expr) => {
            #[test]
            fn $test_encoding() {
                let init_bytes = $input;
                let input = Base16 { bytes: init_bytes.clone() };
                let guessed_string_len = input.string_len();
                assert_eq!(init_bytes.len(), input.byte_len());

                let decoded = input.decode();
                let verified = Base16::verify_base16_string(&decoded);
                assert!(verified.is_ok());
                assert_eq!(decoded.len(), guessed_string_len);

                let encoded = Base16::encode(&decoded);
                assert!(encoded.is_ok());
                let output = encoded.unwrap();
                assert_eq!(input, output);
                assert_eq!(init_bytes.len(), output.byte_len());
            }

            gen_test_circular_serde_sanity_test!($test_wire, Base16::from_bytes($input));
        }
    }

    circular_sanity_test!(test_0_bytes_encoding_decoding, test_0_bytes_wire, &vec![]);
    circular_sanity_test!(test_1_byte_encoding_decoding, test_1_byte_wire, &vec![1]);
    circular_sanity_test!(test_2_bytes_encoding_decoding, test_2_bytes_wire, &vec![1, 2]);
    circular_sanity_test!(test_3_bytes_encoding_decoding, test_3_bytes_wire, &vec![1, 2, 3]);
    circular_sanity_test!(test_4_bytes_encoding_decoding, test_4_bytes_wire, &vec![1, 2, 3, 4]);
    circular_sanity_test!(test_5_bytes_encoding_decoding, test_5_bytes_wire, &vec![1, 2, 3, 4, 5]);
    circular_sanity_test!(test_6_bytes_encoding_decoding, test_6_bytes_wire, &vec![1, 2, 3, 4, 5, 6]);
    circular_sanity_test!(test_7_bytes_encoding_decoding, test_7_bytes_wire, &vec![1, 2, 3, 4, 5, 6, 7]);
    circular_sanity_test!(test_8_bytes_encoding_decoding, test_8_bytes_wire, &vec![1, 2, 3, 4, 5, 6, 7, 8]);
    circular_sanity_test!(test_9_bytes_encoding_decoding, test_9_bytes_wire, &vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    circular_sanity_test!(test_10_bytes_encoding_decoding, test_10_bytes_wire, &vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    circular_sanity_test!(test_11_bytes_encoding_decoding, test_11_bytes_wire, &vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
    circular_sanity_test!(test_12_bytes_encoding_decoding, test_12_bytes_wire, &vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
    circular_sanity_test!(test_13_bytes_encoding_decoding, test_13_bytes_wire, &vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]);
    circular_sanity_test!(test_14_bytes_encoding_decoding, test_14_bytes_wire, &vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]);
    circular_sanity_test!(test_15_bytes_encoding_decoding, test_15_bytes_wire, &vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);
    circular_sanity_test!(test_16_bytes_encoding_decoding, test_16_bytes_wire, &vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
}

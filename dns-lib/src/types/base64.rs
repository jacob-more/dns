use std::{fmt::{Display, Debug}, error::Error};

use ux::u6;

use crate::{types::{ascii::{AsciiChar, AsciiError, constants::*, AsciiString}, base_conversions::BaseConversions}, serde::presentation::from_presentation::FromPresentation};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Base64Error {
    BadChar(AsciiChar),
    IncorrectBufferSize,
    Overflow,
    Underflow,
    BadPadding,
    AsciiError(AsciiError),
}
impl Error for Base64Error {}
impl Display for Base64Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // TODO: error messages could be improved
            Self::BadChar(character) => write!(f, "Character '{character}' is not a valid base 64 character"),
            Self::IncorrectBufferSize => write!(f, "Buffer size is incorrect"),
            Self::Overflow => write!(f, "Overflow"),
            Self::Underflow => write!(f, "Underflow"),
            Self::BadPadding => write!(f, "Padding can only occur at the end of the encoding and must be exact"),
            Self::AsciiError(ascii_err) => write!(f, "{ascii_err}")
        }
    }
}
impl From<AsciiError> for Base64Error {
    fn from(value: AsciiError) -> Self {
        Self::AsciiError(value)
    }
}

const PADDING_CHAR: AsciiChar = ASCII_EQUALS;

#[inline]
fn u6_to_base64(bits: u6) -> AsciiChar {
    match u8::from(bits) {
        0b00_000000 => ASCII_UPPERCASE_A,
        0b00_000001 => ASCII_UPPERCASE_B,
        0b00_000010 => ASCII_UPPERCASE_C,
        0b00_000011 => ASCII_UPPERCASE_D,
        0b00_000100 => ASCII_UPPERCASE_E,
        0b00_000101 => ASCII_UPPERCASE_F,
        0b00_000110 => ASCII_UPPERCASE_G,
        0b00_000111 => ASCII_UPPERCASE_H,
        0b00_001000 => ASCII_UPPERCASE_I,
        0b00_001001 => ASCII_UPPERCASE_J,
        0b00_001010 => ASCII_UPPERCASE_K,
        0b00_001011 => ASCII_UPPERCASE_L,
        0b00_001100 => ASCII_UPPERCASE_M,
        0b00_001101 => ASCII_UPPERCASE_N,
        0b00_001110 => ASCII_UPPERCASE_O,
        0b00_001111 => ASCII_UPPERCASE_P,
        0b00_010000 => ASCII_UPPERCASE_Q,
        0b00_010001 => ASCII_UPPERCASE_R,
        0b00_010010 => ASCII_UPPERCASE_S,
        0b00_010011 => ASCII_UPPERCASE_T,
        0b00_010100 => ASCII_UPPERCASE_U,
        0b00_010101 => ASCII_UPPERCASE_V,
        0b00_010110 => ASCII_UPPERCASE_W,
        0b00_010111 => ASCII_UPPERCASE_X,
        0b00_011000 => ASCII_UPPERCASE_Y,
        0b00_011001 => ASCII_UPPERCASE_Z,
        0b00_011010 => ASCII_LOWERCASE_A,
        0b00_011011 => ASCII_LOWERCASE_B,
        0b00_011100 => ASCII_LOWERCASE_C,
        0b00_011101 => ASCII_LOWERCASE_D,
        0b00_011110 => ASCII_LOWERCASE_E,
        0b00_011111 => ASCII_LOWERCASE_F,
        0b00_100000 => ASCII_LOWERCASE_G,
        0b00_100001 => ASCII_LOWERCASE_H,
        0b00_100010 => ASCII_LOWERCASE_I,
        0b00_100011 => ASCII_LOWERCASE_J,
        0b00_100100 => ASCII_LOWERCASE_K,
        0b00_100101 => ASCII_LOWERCASE_L,
        0b00_100110 => ASCII_LOWERCASE_M,
        0b00_100111 => ASCII_LOWERCASE_N,
        0b00_101000 => ASCII_LOWERCASE_O,
        0b00_101001 => ASCII_LOWERCASE_P,
        0b00_101010 => ASCII_LOWERCASE_Q,
        0b00_101011 => ASCII_LOWERCASE_R,
        0b00_101100 => ASCII_LOWERCASE_S,
        0b00_101101 => ASCII_LOWERCASE_T,
        0b00_101110 => ASCII_LOWERCASE_U,
        0b00_101111 => ASCII_LOWERCASE_V,
        0b00_110000 => ASCII_LOWERCASE_W,
        0b00_110001 => ASCII_LOWERCASE_X,
        0b00_110010 => ASCII_LOWERCASE_Y,
        0b00_110011 => ASCII_LOWERCASE_Z,
        0b00_110100 => ASCII_ZERO,
        0b00_110101 => ASCII_ONE,
        0b00_110110 => ASCII_TWO,
        0b00_110111 => ASCII_THREE,
        0b00_111000 => ASCII_FOUR,
        0b00_111001 => ASCII_FIVE,
        0b00_111010 => ASCII_SIX,
        0b00_111011 => ASCII_SEVEN,
        0b00_111100 => ASCII_EIGHT,
        0b00_111101 => ASCII_NINE,
        0b00_111110 => ASCII_PLUS,
        0b00_111111 => ASCII_SLASH,

        _ => panic!("Illegal State Reached: This should not be possible. The value should convert to a u6 and exhaustively match those values.")
    }
}

#[inline]
const fn base64_to_u6(character: AsciiChar) -> u6 {
    match character {
        ASCII_UPPERCASE_A =>  u6::new(0b00_000000),
        ASCII_UPPERCASE_B =>  u6::new(0b00_000001),
        ASCII_UPPERCASE_C =>  u6::new(0b00_000010),
        ASCII_UPPERCASE_D =>  u6::new(0b00_000011),
        ASCII_UPPERCASE_E =>  u6::new(0b00_000100),
        ASCII_UPPERCASE_F =>  u6::new(0b00_000101),
        ASCII_UPPERCASE_G =>  u6::new(0b00_000110),
        ASCII_UPPERCASE_H =>  u6::new(0b00_000111),
        ASCII_UPPERCASE_I =>  u6::new(0b00_001000),
        ASCII_UPPERCASE_J =>  u6::new(0b00_001001),
        ASCII_UPPERCASE_K =>  u6::new(0b00_001010),
        ASCII_UPPERCASE_L =>  u6::new(0b00_001011),
        ASCII_UPPERCASE_M =>  u6::new(0b00_001100),
        ASCII_UPPERCASE_N =>  u6::new(0b00_001101),
        ASCII_UPPERCASE_O =>  u6::new(0b00_001110),
        ASCII_UPPERCASE_P =>  u6::new(0b00_001111),
        ASCII_UPPERCASE_Q =>  u6::new(0b00_010000),
        ASCII_UPPERCASE_R =>  u6::new(0b00_010001),
        ASCII_UPPERCASE_S =>  u6::new(0b00_010010),
        ASCII_UPPERCASE_T =>  u6::new(0b00_010011),
        ASCII_UPPERCASE_U =>  u6::new(0b00_010100),
        ASCII_UPPERCASE_V =>  u6::new(0b00_010101),
        ASCII_UPPERCASE_W =>  u6::new(0b00_010110),
        ASCII_UPPERCASE_X =>  u6::new(0b00_010111),
        ASCII_UPPERCASE_Y =>  u6::new(0b00_011000),
        ASCII_UPPERCASE_Z =>  u6::new(0b00_011001),
        ASCII_LOWERCASE_A =>  u6::new(0b00_011010),
        ASCII_LOWERCASE_B =>  u6::new(0b00_011011),
        ASCII_LOWERCASE_C =>  u6::new(0b00_011100),
        ASCII_LOWERCASE_D =>  u6::new(0b00_011101),
        ASCII_LOWERCASE_E =>  u6::new(0b00_011110),
        ASCII_LOWERCASE_F =>  u6::new(0b00_011111),
        ASCII_LOWERCASE_G =>  u6::new(0b00_100000),
        ASCII_LOWERCASE_H =>  u6::new(0b00_100001),
        ASCII_LOWERCASE_I =>  u6::new(0b00_100010),
        ASCII_LOWERCASE_J =>  u6::new(0b00_100011),
        ASCII_LOWERCASE_K =>  u6::new(0b00_100100),
        ASCII_LOWERCASE_L =>  u6::new(0b00_100101),
        ASCII_LOWERCASE_M =>  u6::new(0b00_100110),
        ASCII_LOWERCASE_N =>  u6::new(0b00_100111),
        ASCII_LOWERCASE_O =>  u6::new(0b00_101000),
        ASCII_LOWERCASE_P =>  u6::new(0b00_101001),
        ASCII_LOWERCASE_Q =>  u6::new(0b00_101010),
        ASCII_LOWERCASE_R =>  u6::new(0b00_101011),
        ASCII_LOWERCASE_S =>  u6::new(0b00_101100),
        ASCII_LOWERCASE_T =>  u6::new(0b00_101101),
        ASCII_LOWERCASE_U =>  u6::new(0b00_101110),
        ASCII_LOWERCASE_V =>  u6::new(0b00_101111),
        ASCII_LOWERCASE_W =>  u6::new(0b00_110000),
        ASCII_LOWERCASE_X =>  u6::new(0b00_110001),
        ASCII_LOWERCASE_Y =>  u6::new(0b00_110010),
        ASCII_LOWERCASE_Z =>  u6::new(0b00_110011),
        ASCII_ZERO =>         u6::new(0b00_110100),
        ASCII_ONE =>          u6::new(0b00_110101),
        ASCII_TWO =>          u6::new(0b00_110110),
        ASCII_THREE =>        u6::new(0b00_110111),
        ASCII_FOUR =>         u6::new(0b00_111000),
        ASCII_FIVE =>         u6::new(0b00_111001),
        ASCII_SIX =>          u6::new(0b00_111010),
        ASCII_SEVEN =>        u6::new(0b00_111011),
        ASCII_EIGHT =>        u6::new(0b00_111100),
        ASCII_NINE =>         u6::new(0b00_111101),
        ASCII_PLUS =>         u6::new(0b00_111110),
        ASCII_SLASH =>        u6::new(0b00_111111),

        _ => panic!("Illegal Character: base64 encoding does not include this character.")
    }
}

#[inline]
const fn is_base64_char(encoded: AsciiChar) -> bool {
    match encoded {
        ASCII_UPPERCASE_A..=ASCII_UPPERCASE_Z => true,
        ASCII_LOWERCASE_A..=ASCII_LOWERCASE_Z => true,
        ASCII_ZERO..=ASCII_NINE => true,
        ASCII_PLUS => true,
        ASCII_SLASH => true,
        PADDING_CHAR => true,
        _ => false,
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Base64 {
    bytes: Vec<u8>,
}

impl Display for Base64 {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.decode())
    }
}

impl Debug for Base64 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Base64(\"{}\")", self.decode())
    }
}

impl Base64 {
    #[inline]
    pub fn from_ascii(string: &AsciiString) -> Result<Self, Base64Error> {
        Self::encode(string)
    }

    #[inline]
    pub fn from_case_insensitive_ascii(string: &AsciiString) -> Result<Self, Base64Error> {
        Self::from_ascii(
            &string.as_upper()
        )
    }

    #[inline]
    pub fn from_utf8(string: &str) -> Result<Self, Base64Error> {
        Self::from_ascii(
            &AsciiString::from_utf8(string)?
        )
    }

    #[inline]
    pub fn from_case_insensitive_utf8(string: &str) -> Result<Self, Base64Error> {
        Self::from_case_insensitive_ascii(
            &AsciiString::from_utf8(string)?
        )
    }

    #[inline]
    pub fn encode(string: &AsciiString) -> Result<Self, Base64Error> {
        let mut encoded_bytes: Vec<u8> = Vec::with_capacity((string.len() * 6) / 8);
        let string_chunks = string.as_slice().chunks_exact(4);

        let remainder = string_chunks.remainder();
        if remainder.len() != 0 {
            return Err(Base64Error::Overflow);
        }

        for chunk in string_chunks {
            match chunk {
                // 1st & 2nd character can never be a padding character.
                &[PADDING_CHAR, _, _, _] => return Err(Base64Error::Overflow),
                &[_, PADDING_CHAR, _, _] => return Err(Base64Error::Overflow),
                // 3rd character can only be padding if 4th is as well
                &[char1, char2, PADDING_CHAR, PADDING_CHAR] => {
                    let bits0_5   = (u32::from(base64_to_u6(char1)) << 18) & 0b00000000_111111_000000_000000_000000;
                    let bits6_11  = (u32::from(base64_to_u6(char2)) << 12) & 0b00000000_000000_110000_000000_000000;
                    let merged_bytes = bits0_5 | bits6_11;

                    let [_, byte1, _, _] = u32::to_be_bytes(merged_bytes);

                    encoded_bytes.push(byte1);
                    break;
                },
                &[_, _, PADDING_CHAR, _] => return Err(Base64Error::Overflow),
                // 4th character can be padding
                &[char1, char2, char3, PADDING_CHAR] => {
                    let bits0_5   = (u32::from(base64_to_u6(char1)) << 18) & 0b00000000_111111_000000_000000_000000;
                    let bits6_11  = (u32::from(base64_to_u6(char2)) << 12) & 0b00000000_000000_111111_000000_000000;
                    let bits12_15 = (u32::from(base64_to_u6(char3)) << 6)  & 0b00000000_000000_000000_111100_000000;
                    let merged_bytes = bits0_5 | bits6_11 | bits12_15;

                    let [_, byte1, byte2, _] = u32::to_be_bytes(merged_bytes);
                    encoded_bytes.extend([byte1, byte2]);
                    break;
                },
                &[char1, char2, char3, char4] => {
                    let bits0_5   = (u32::from(base64_to_u6(char1)) << 18) & 0b00000000_111111_000000_000000_000000;
                    let bits6_11  = (u32::from(base64_to_u6(char2)) << 12) & 0b00000000_000000_111111_000000_000000;
                    let bits12_17 = (u32::from(base64_to_u6(char3)) << 6)  & 0b00000000_000000_000000_111111_000000;
                    let bits18_23 = (u32::from(base64_to_u6(char4)) << 0)  & 0b00000000_000000_000000_000000_111111;
                    let merged_bytes = bits0_5 | bits6_11 | bits12_17 | bits18_23;

                    let [_, byte1, byte2, byte3] = u32::to_be_bytes(merged_bytes);
                    encoded_bytes.extend([byte1, byte2, byte3]);
                },
                _ => panic!("The pattern was supposed to chunk exactly 4 bytes. However, the chunk contained {} bytes", chunk.len()),
            }
        }

        return Ok(Self { bytes: encoded_bytes });
    }

    #[inline]
    pub fn decode(&self) -> AsciiString {
        let mut decoded_chars: Vec<u8> = Vec::with_capacity((self.bytes.len() * 8) / 6);
        let byte_chunks = self.bytes.chunks_exact(3);
        let remainder = byte_chunks.remainder();

        byte_chunks.for_each(|chunk| {
            let merged_bytes = u32::from_be_bytes([0, chunk[0], chunk[1], chunk[2]]);

            let bits0_5   = ((merged_bytes & 0b00000000_111111_000000_000000_000000) >> 18) as u8;
            let bits6_11  = ((merged_bytes & 0b00000000_000000_111111_000000_000000) >> 12) as u8;
            let bits12_17 = ((merged_bytes & 0b00000000_000000_000000_111111_000000) >> 6) as u8;
            let bits18_23 = ((merged_bytes & 0b00000000_000000_000000_000000_111111) >> 0) as u8;

            decoded_chars.extend([
                u6_to_base64(u6::new(bits0_5)),
                u6_to_base64(u6::new(bits6_11)),
                u6_to_base64(u6::new(bits12_17)),
                u6_to_base64(u6::new(bits18_23)),
            ]);
        });

        match remainder.len() {
            0 => (),
            1 => {
                let merged_bytes = u32::from_be_bytes([0, remainder[0], 0, 0]);

                let bits0_5 = ((merged_bytes & 0b00000000_111111_000000_000000_000000) >> 18) as u8;
                let bits6_7 = ((merged_bytes & 0b00000000_000000_110000_000000_000000) >> 12) as u8;

                decoded_chars.extend([
                    u6_to_base64(u6::new(bits0_5)),
                    u6_to_base64(u6::new(bits6_7)),
                    PADDING_CHAR,
                    PADDING_CHAR,
                ]);
            },
            2 => {
                let merged_bytes = u32::from_be_bytes([0, remainder[0], remainder[1], 0]);

                let bits0_5   = ((merged_bytes & 0b00000000_111111_000000_000000_000000) >> 18) as u8;
                let bits6_11  = ((merged_bytes & 0b00000000_000000_111111_000000_000000) >> 12) as u8;
                let bits12_15 = ((merged_bytes & 0b00000000_000000_000000_111100_000000) >> 6) as u8;

                decoded_chars.extend([
                    u6_to_base64(u6::new(bits0_5)),
                    u6_to_base64(u6::new(bits6_11)),
                    u6_to_base64(u6::new(bits12_15)),
                    PADDING_CHAR,
                ]);
            },
            _ => panic!("An impossible remainder length was reached. The length of a remainder can only be 0, 1, or 2.")
        }

        AsciiString::from(&decoded_chars)
    }

    #[inline]
    pub fn verify_base64_string(string: &AsciiString) -> Result<(), Base64Error> {
        // Verify that all of the characters are valid for this alphabet.
        for character in string.iter() {
            if !is_base64_char(*character) {
                return Err(Base64Error::BadChar(*character));
            }
        }

        let string_chunks = string.as_slice().chunks_exact(4);

        // Verify that the format has the correct number of characters and won't overflow when decoding.
        if !string_chunks.remainder().is_empty() {
            return Err(Base64Error::Overflow);
        }

        // Verify that the format does not contain padding characters where they are not allowed.
        let mut padding_reached = false;
        for chunk in string_chunks {
            if padding_reached {
                return Err(Base64Error::Overflow);
            }

            match chunk {
                // 1st & 2nd character can never be a padding character.
                &[PADDING_CHAR, _, _, _] => return Err(Base64Error::Overflow),
                &[_, PADDING_CHAR, _, _] => return Err(Base64Error::Overflow),
                // 3rd character can only be padding if 4th is as well
                &[_, _, PADDING_CHAR, PADDING_CHAR] => padding_reached = true,
                &[_, _, PADDING_CHAR, _] => return Err(Base64Error::Overflow),
                // 4th character can be padding
                &[_, _, _, PADDING_CHAR] => padding_reached = true,
                &[_, _, _, _] => (),
                _ => panic!("The pattern was supposed to chunk exactly 4 bytes. However, the chunk contained {} bytes", chunk.len()),
            }
        }

        return Ok(());
    }
}

impl BaseConversions for Base64 {
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
        let base = (self.bytes.len() * 4).div_ceil(3);
        // + Padding
        base.next_multiple_of(4)
    }
}

impl FromPresentation for Base64 {
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
    use super::Base64;

    macro_rules! circular_sanity_test {
        ($test_encoding:ident, $test_wire:ident, $input:expr) => {
            #[test]
            fn $test_encoding() {
                let init_bytes = $input;
                let input = Base64 { bytes: init_bytes.clone() };
                let guessed_string_len = input.string_len();
                assert_eq!(init_bytes.len(), input.byte_len());

                let decoded = input.decode();
                let verified = Base64::verify_base64_string(&decoded);
                assert!(verified.is_ok());
                assert_eq!(decoded.len(), guessed_string_len);

                let encoded = Base64::encode(&decoded);
                assert!(encoded.is_ok());
                let output = encoded.unwrap();
                assert_eq!(input, output);
                assert_eq!(init_bytes.len(), output.byte_len());
            }

            gen_test_circular_serde_sanity_test!($test_wire, Base64::from_bytes($input));
        }
    }

    circular_sanity_test!(test_0_bytes_encode_decode, test_0_bytes_wire, &vec![]);
    circular_sanity_test!(test_1_byte_encode_decode, test_1_byte_wire, &vec![1]);
    circular_sanity_test!(test_2_bytes_encode_decode, test_2_bytes_wire, &vec![1, 2]);
    circular_sanity_test!(test_3_bytes_encode_decode, test_3_bytes_wire, &vec![1, 2, 3]);
    circular_sanity_test!(test_4_bytes_encode_decode, test_4_bytes_wire, &vec![1, 2, 3, 4]);
    circular_sanity_test!(test_5_bytes_encode_decode, test_5_bytes_wire, &vec![1, 2, 3, 4, 5]);
    circular_sanity_test!(test_6_bytes_encode_decode, test_6_bytes_wire, &vec![1, 2, 3, 4, 5, 6]);
    circular_sanity_test!(test_7_bytes_encode_decode, test_7_bytes_wire, &vec![1, 2, 3, 4, 5, 6, 7]);
    circular_sanity_test!(test_8_bytes_encode_decode, test_8_bytes_wire, &vec![1, 2, 3, 4, 5, 6, 7, 8]);
    circular_sanity_test!(test_9_bytes_encode_decode, test_9_bytes_wire, &vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    circular_sanity_test!(test_10_bytes_encode_decode, test_10_bytes_wire, &vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    circular_sanity_test!(test_11_bytes_encode_decode, test_11_bytes_wire, &vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
    circular_sanity_test!(test_12_bytes_encode_decode, test_12_bytes_wire, &vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
    circular_sanity_test!(test_13_bytes_encode_decode, test_13_bytes_wire, &vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]);
    circular_sanity_test!(test_14_bytes_encode_decode, test_14_bytes_wire, &vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]);
    circular_sanity_test!(test_15_bytes_encode_decode, test_15_bytes_wire, &vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);
    circular_sanity_test!(test_16_bytes_encode_decode, test_16_bytes_wire, &vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
}

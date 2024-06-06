use std::{error::Error, fmt::{Display, Debug}};

use ux::u5;

use crate::{serde::presentation::{errors::TokenError, from_presentation::FromPresentation}, types::{ascii::{constants::*, AsciiChar, AsciiError, AsciiString}, base_conversions::BaseConversions}};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Base32Error {
    BadChar(AsciiChar),
    IncorrectBufferSize,
    Overflow,
    Underflow,
    BadPadding,
    AsciiError(AsciiError),
}
impl Error for Base32Error {}
impl Display for Base32Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // TODO: error messages could be improved
            Self::BadChar(character) => write!(f, "Character '{character}' is not a valid base 32 character"),
            Self::IncorrectBufferSize => write!(f, "Buffer size is incorrect"),
            Self::Overflow => write!(f, "Overflow"),
            Self::Underflow => write!(f, "Underflow"),
            Self::BadPadding => write!(f, "Padding can only occur at the end of the encoding and must be exact"),
            Self::AsciiError(ascii_err) => write!(f, "{ascii_err}")
        }
    }
}
impl From<AsciiError> for Base32Error {
    fn from(value: AsciiError) -> Self {
        Self::AsciiError(value)
    }
}

const PADDING_CHAR: AsciiChar = ASCII_EQUALS;

#[inline]
fn u5_to_base32(bits: u5) -> AsciiChar {
    match u8::from(bits) {
        0b000_00000 => ASCII_UPPERCASE_A,
        0b000_00001 => ASCII_UPPERCASE_B,
        0b000_00010 => ASCII_UPPERCASE_C,
        0b000_00011 => ASCII_UPPERCASE_D,
        0b000_00100 => ASCII_UPPERCASE_E,
        0b000_00101 => ASCII_UPPERCASE_F,
        0b000_00110 => ASCII_UPPERCASE_G,
        0b000_00111 => ASCII_UPPERCASE_H,
        0b000_01000 => ASCII_UPPERCASE_I,
        0b000_01001 => ASCII_UPPERCASE_J,
        0b000_01010 => ASCII_UPPERCASE_K,
        0b000_01011 => ASCII_UPPERCASE_L,
        0b000_01100 => ASCII_UPPERCASE_M,
        0b000_01101 => ASCII_UPPERCASE_N,
        0b000_01110 => ASCII_UPPERCASE_O,
        0b000_01111 => ASCII_UPPERCASE_P,
        0b000_10000 => ASCII_UPPERCASE_Q,
        0b000_10001 => ASCII_UPPERCASE_R,
        0b000_10010 => ASCII_UPPERCASE_S,
        0b000_10011 => ASCII_UPPERCASE_T,
        0b000_10100 => ASCII_UPPERCASE_U,
        0b000_10101 => ASCII_UPPERCASE_V,
        0b000_10110 => ASCII_UPPERCASE_W,
        0b000_10111 => ASCII_UPPERCASE_X,
        0b000_11000 => ASCII_UPPERCASE_Y,
        0b000_11001 => ASCII_UPPERCASE_Z,
        0b000_11010 => ASCII_TWO,
        0b000_11011 => ASCII_THREE,
        0b000_11100 => ASCII_FOUR,
        0b000_11101 => ASCII_FIVE,
        0b000_11110 => ASCII_SIX,
        0b000_11111 => ASCII_SEVEN,

        _ => unreachable!("Illegal State Reached: This should not be possible. The value should convert to a u5 and exhaustively match those values.")
    }
}

#[inline]
const fn base32_to_u5(character: AsciiChar) -> Result<u5, Base32Error> {
    match character {
        ASCII_UPPERCASE_A => Ok(u5::new(0b000_00000)),
        ASCII_UPPERCASE_B => Ok(u5::new(0b000_00001)),
        ASCII_UPPERCASE_C => Ok(u5::new(0b000_00010)),
        ASCII_UPPERCASE_D => Ok(u5::new(0b000_00011)),
        ASCII_UPPERCASE_E => Ok(u5::new(0b000_00100)),
        ASCII_UPPERCASE_F => Ok(u5::new(0b000_00101)),
        ASCII_UPPERCASE_G => Ok(u5::new(0b000_00110)),
        ASCII_UPPERCASE_H => Ok(u5::new(0b000_00111)),
        ASCII_UPPERCASE_I => Ok(u5::new(0b000_01000)),
        ASCII_UPPERCASE_J => Ok(u5::new(0b000_01001)),
        ASCII_UPPERCASE_K => Ok(u5::new(0b000_01010)),
        ASCII_UPPERCASE_L => Ok(u5::new(0b000_01011)),
        ASCII_UPPERCASE_M => Ok(u5::new(0b000_01100)),
        ASCII_UPPERCASE_N => Ok(u5::new(0b000_01101)),
        ASCII_UPPERCASE_O => Ok(u5::new(0b000_01110)),
        ASCII_UPPERCASE_P => Ok(u5::new(0b000_01111)),
        ASCII_UPPERCASE_Q => Ok(u5::new(0b000_10000)),
        ASCII_UPPERCASE_R => Ok(u5::new(0b000_10001)),
        ASCII_UPPERCASE_S => Ok(u5::new(0b000_10010)),
        ASCII_UPPERCASE_T => Ok(u5::new(0b000_10011)),
        ASCII_UPPERCASE_U => Ok(u5::new(0b000_10100)),
        ASCII_UPPERCASE_V => Ok(u5::new(0b000_10101)),
        ASCII_UPPERCASE_W => Ok(u5::new(0b000_10110)),
        ASCII_UPPERCASE_X => Ok(u5::new(0b000_10111)),
        ASCII_UPPERCASE_Y => Ok(u5::new(0b000_11000)),
        ASCII_UPPERCASE_Z => Ok(u5::new(0b000_11001)),
        ASCII_TWO         => Ok(u5::new(0b000_11010)),
        ASCII_THREE       => Ok(u5::new(0b000_11011)),
        ASCII_FOUR        => Ok(u5::new(0b000_11100)),
        ASCII_FIVE        => Ok(u5::new(0b000_11101)),
        ASCII_SIX         => Ok(u5::new(0b000_11110)),
        ASCII_SEVEN       => Ok(u5::new(0b000_11111)),

        _ => Err(Base32Error::BadChar(character)),
    }
}

#[inline]
const fn is_base32_char(encoded: AsciiChar) -> bool {
    match encoded {
        ASCII_UPPERCASE_A..=ASCII_UPPERCASE_Z => true,
        ASCII_TWO..=ASCII_SEVEN => true,
        PADDING_CHAR => true,
        _ => false,
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Base32 {
    bytes: Vec<u8>,
}

impl Display for Base32 {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.decode())
    }
}

impl Debug for Base32 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Base32(\"{}\")", self.decode())
    }
}

impl Base32 {
    #[inline]
    pub fn from_ascii(string: &AsciiString) -> Result<Self, Base32Error> {
        Self::encode(string)
    }

    #[inline]
    pub fn from_case_insensitive_ascii(string: &AsciiString) -> Result<Self, Base32Error> {
        Self::from_ascii(
            &string.as_upper()
        )
    }

    #[inline]
    pub fn from_utf8(string: &str) -> Result<Self, Base32Error> {
        Base32::from_ascii(
            &AsciiString::from_utf8(string)?
        )
    }

    #[inline]
    pub fn from_case_insensitive_utf8(string: &str) -> Result<Self, Base32Error> {
        Self::from_case_insensitive_ascii(
            &AsciiString::from_utf8(string)?
        )
    }

    #[inline]
    pub fn encode(string: &AsciiString) -> Result<Self, Base32Error> {
        let mut encoded_bytes: Vec<u8> = Vec::with_capacity((string.len() * 5) / 8);
        let string_chunks = string.as_slice().chunks_exact(8);

        let remainder = string_chunks.remainder();
        if remainder.len() != 0 {
            return Err(Base32Error::Overflow);
        }

        for chunk in string_chunks {
            match chunk {
                // 1st character can never be a padding character.
                &[PADDING_CHAR, _, _, _, _, _, _, _] => return Err(Base32Error::Overflow),

                // Characters 1, 2, & 3 can only be a padding character if every byte that follows is also a padding character.
                &[char1, PADDING_CHAR, PADDING_CHAR, PADDING_CHAR, PADDING_CHAR, PADDING_CHAR, PADDING_CHAR, PADDING_CHAR] => {
                    let bits0_4   = (u64::from(base32_to_u5(char1)?) << 35) & 0b11111_00000_00000_00000_00000_00000_00000_00000;
                    let merged_bytes = bits0_4;

                    let [_, _, _, byte1, _, _, _, _] = u64::to_be_bytes(merged_bytes);

                    encoded_bytes.push(byte1);
                    break;
                },
                &[_, PADDING_CHAR, _, _, _, _, _, _] => return Err(Base32Error::BadPadding),

                &[char1, char2, PADDING_CHAR, PADDING_CHAR, PADDING_CHAR, PADDING_CHAR, PADDING_CHAR, PADDING_CHAR] => {
                    let bits0_4   = (u64::from(base32_to_u5(char1)?) << 35) & 0b11111_00000_00000_00000_00000_00000_00000_00000;
                    let bits5_7   = (u64::from(base32_to_u5(char2)?) << 30) & 0b00000_11100_00000_00000_00000_00000_00000_00000;
                    let merged_bytes = bits0_4 | bits5_7;

                    let [_, _, _, byte1, _, _, _, _] = u64::to_be_bytes(merged_bytes);

                    encoded_bytes.push(byte1);
                    break;
                },
                &[_, _, PADDING_CHAR, _, _, _, _, _] => return Err(Base32Error::BadPadding),

                // 4th character can never be a padding character.
                &[_, _, _, PADDING_CHAR, _, _, _, _] => return Err(Base32Error::BadPadding),

                // Characters 5 & 6 can only be a padding character if every byte that follows is also a padding character.
                &[char1, char2, char3, char4, PADDING_CHAR, PADDING_CHAR, PADDING_CHAR, PADDING_CHAR] => {
                    let bits0_4   = (u64::from(base32_to_u5(char1)?) << 35) & 0b11111_00000_00000_00000_00000_00000_00000_00000;
                    let bits5_9   = (u64::from(base32_to_u5(char2)?) << 30) & 0b00000_11111_00000_00000_00000_00000_00000_00000;
                    let bits10_14 = (u64::from(base32_to_u5(char3)?) << 25) & 0b00000_00000_11111_00000_00000_00000_00000_00000;
                    let bits15_15 = (u64::from(base32_to_u5(char4)?) << 20) & 0b00000_00000_00000_10000_00000_00000_00000_00000;
                    let merged_bytes = bits0_4 | bits5_9 | bits10_14 | bits15_15;

                    let [_, _, _, byte1, byte2, _, _, _] = u64::to_be_bytes(merged_bytes);
                    encoded_bytes.extend([byte1, byte2]);
                    break;
                },
                &[_, _, _, _, PADDING_CHAR, _, _, _] => return Err(Base32Error::BadPadding),
                &[char1, char2, char3, char4, char5, PADDING_CHAR, PADDING_CHAR, PADDING_CHAR] => {
                    let bits0_4   = (u64::from(base32_to_u5(char1)?) << 35) & 0b11111_00000_00000_00000_00000_00000_00000_00000;
                    let bits5_9   = (u64::from(base32_to_u5(char2)?) << 30) & 0b00000_11111_00000_00000_00000_00000_00000_00000;
                    let bits10_14 = (u64::from(base32_to_u5(char3)?) << 25) & 0b00000_00000_11111_00000_00000_00000_00000_00000;
                    let bits15_19 = (u64::from(base32_to_u5(char4)?) << 20) & 0b00000_00000_00000_11111_00000_00000_00000_00000;
                    let bits20_23 = (u64::from(base32_to_u5(char5)?) << 15) & 0b00000_00000_00000_00000_11110_00000_00000_00000;
                    let merged_bytes = bits0_4 | bits5_9 | bits10_14 | bits15_19 | bits20_23;

                    let [_, _, _, byte1, byte2, byte3, _, _] = u64::to_be_bytes(merged_bytes);
                    encoded_bytes.extend([byte1, byte2, byte3]);
                    break;
                },
                &[_, _, _, _, _, PADDING_CHAR, _, _] => return Err(Base32Error::BadPadding),

                // 7th character can never be a padding character.
                &[_, _, _, _, _, _, PADDING_CHAR, _] => return Err(Base32Error::BadPadding),

                // 8th character can be padding but does not have to be.
                &[char1, char2, char3, char4, char5, char6, char7, PADDING_CHAR] => {
                    let bits0_4   = (u64::from(base32_to_u5(char1)?) << 35) & 0b11111_00000_00000_00000_00000_00000_00000_00000;
                    let bits5_9   = (u64::from(base32_to_u5(char2)?) << 30) & 0b00000_11111_00000_00000_00000_00000_00000_00000;
                    let bits10_14 = (u64::from(base32_to_u5(char3)?) << 25) & 0b00000_00000_11111_00000_00000_00000_00000_00000;
                    let bits15_19 = (u64::from(base32_to_u5(char4)?) << 20) & 0b00000_00000_00000_11111_00000_00000_00000_00000;
                    let bits20_24 = (u64::from(base32_to_u5(char5)?) << 15) & 0b00000_00000_00000_00000_11111_00000_00000_00000;
                    let bits25_29 = (u64::from(base32_to_u5(char6)?) << 10) & 0b00000_00000_00000_00000_00000_11111_00000_00000;
                    let bits30_31 = (u64::from(base32_to_u5(char7)?) << 5)  & 0b00000_00000_00000_00000_00000_00000_11000_00000;
                    let merged_bytes = bits0_4 | bits5_9 | bits10_14 | bits15_19 | bits20_24 | bits25_29 | bits30_31;

                    let [_, _, _, byte1, byte2, byte3, byte4, _] = u64::to_be_bytes(merged_bytes);
                    encoded_bytes.extend([byte1, byte2, byte3, byte4]);
                    break;
                },

                &[char1, char2, char3, char4, char5, char6, char7, char8] => {
                    let bits0_4   = (u64::from(base32_to_u5(char1)?) << 35) & 0b11111_00000_00000_00000_00000_00000_00000_00000;
                    let bits5_9   = (u64::from(base32_to_u5(char2)?) << 30) & 0b00000_11111_00000_00000_00000_00000_00000_00000;
                    let bits10_14 = (u64::from(base32_to_u5(char3)?) << 25) & 0b00000_00000_11111_00000_00000_00000_00000_00000;
                    let bits15_19 = (u64::from(base32_to_u5(char4)?) << 20) & 0b00000_00000_00000_11111_00000_00000_00000_00000;
                    let bits20_24 = (u64::from(base32_to_u5(char5)?) << 15) & 0b00000_00000_00000_00000_11111_00000_00000_00000;
                    let bits25_29 = (u64::from(base32_to_u5(char6)?) << 10) & 0b00000_00000_00000_00000_00000_11111_00000_00000;
                    let bits30_34 = (u64::from(base32_to_u5(char7)?) << 5)  & 0b00000_00000_00000_00000_00000_00000_11111_00000;
                    let bits35_39 = (u64::from(base32_to_u5(char8)?) << 0)  & 0b00000_00000_00000_00000_00000_00000_00000_11111;
                    let merged_bytes = bits0_4 | bits5_9 | bits10_14 | bits15_19 | bits20_24 | bits25_29 | bits30_34 | bits35_39;

                    let [_, _, _, byte1, byte2, byte3, byte4, byte5] = u64::to_be_bytes(merged_bytes);
                    encoded_bytes.extend([byte1, byte2, byte3, byte4, byte5]);
                },
                _ => panic!("The pattern was supposed to chunk exactly 8 bytes. However, the chunk contained {} bytes", chunk.len()),
            }
        }

        return Ok(Self { bytes: encoded_bytes });
    }

    #[inline]
    pub fn decode(&self) -> AsciiString {
        let mut decoded_bytes: Vec<u8> = Vec::with_capacity(self.string_len());
        let byte_chunks = self.bytes.chunks_exact(5);
        let remainder = byte_chunks.remainder();

        byte_chunks.for_each(|chunk| {
            let merged_bytes = u64::from_be_bytes([0, 0, 0, chunk[0], chunk[1], chunk[2], chunk[3], chunk[4]]);

            let bits0_4   = ((merged_bytes & 0b11111_00000_00000_00000_00000_00000_00000_00000) >> 35) as u8;
            let bits5_9   = ((merged_bytes & 0b00000_11111_00000_00000_00000_00000_00000_00000) >> 30) as u8;
            let bits10_14 = ((merged_bytes & 0b00000_00000_11111_00000_00000_00000_00000_00000) >> 25) as u8;
            let bits15_19 = ((merged_bytes & 0b00000_00000_00000_11111_00000_00000_00000_00000) >> 20) as u8;
            let bits20_24 = ((merged_bytes & 0b00000_00000_00000_00000_11111_00000_00000_00000) >> 15) as u8;
            let bits25_29 = ((merged_bytes & 0b00000_00000_00000_00000_00000_11111_00000_00000) >> 10) as u8;
            let bits30_34 = ((merged_bytes & 0b00000_00000_00000_00000_00000_00000_11111_00000) >> 5) as u8;
            let bits35_39 = ((merged_bytes & 0b00000_00000_00000_00000_00000_00000_00000_11111) >> 0) as u8;

            decoded_bytes.extend([
                u5_to_base32(u5::new(bits0_4)),
                u5_to_base32(u5::new(bits5_9)),
                u5_to_base32(u5::new(bits10_14)),
                u5_to_base32(u5::new(bits15_19)),
                u5_to_base32(u5::new(bits20_24)),
                u5_to_base32(u5::new(bits25_29)),
                u5_to_base32(u5::new(bits30_34)),
                u5_to_base32(u5::new(bits35_39)),
            ]);
        });

        match remainder {
            &[] => (),
            &[byte0] => {
                let merged_bytes = u64::from_be_bytes([0, 0, 0, byte0, 0, 0, 0, 0]);

                let bits0_4   = ((merged_bytes & 0b11111_00000_00000_00000_00000_00000_00000_00000) >> 35) as u8;
                let bits5_7   = ((merged_bytes & 0b00000_11100_00000_00000_00000_00000_00000_00000) >> 30) as u8;

                decoded_bytes.extend([
                    u5_to_base32(u5::new(bits0_4)),
                    u5_to_base32(u5::new(bits5_7)),
                    PADDING_CHAR,
                    PADDING_CHAR,
                    PADDING_CHAR,
                    PADDING_CHAR,
                    PADDING_CHAR,
                    PADDING_CHAR,
                ]);
            },
            &[byte0, byte1] => {
                let merged_bytes = u64::from_be_bytes([0, 0, 0, byte0, byte1, 0, 0, 0]);

                let bits0_4   = ((merged_bytes & 0b11111_00000_00000_00000_00000_00000_00000_00000) >> 35) as u8;
                let bits5_9   = ((merged_bytes & 0b00000_11111_00000_00000_00000_00000_00000_00000) >> 30) as u8;
                let bits10_14 = ((merged_bytes & 0b00000_00000_11111_00000_00000_00000_00000_00000) >> 25) as u8;
                let bits15_15 = ((merged_bytes & 0b00000_00000_00000_10000_00000_00000_00000_00000) >> 20) as u8;

                decoded_bytes.extend([
                    u5_to_base32(u5::new(bits0_4)),
                    u5_to_base32(u5::new(bits5_9)),
                    u5_to_base32(u5::new(bits10_14)),
                    u5_to_base32(u5::new(bits15_15)),
                    PADDING_CHAR,
                    PADDING_CHAR,
                    PADDING_CHAR,
                    PADDING_CHAR,
                ]);
            },
            &[byte0, byte1, byte2] => {
                let merged_bytes = u64::from_be_bytes([0, 0, 0, byte0, byte1, byte2, 0, 0]);

                let bits0_4   = ((merged_bytes & 0b11111_00000_00000_00000_00000_00000_00000_00000) >> 35) as u8;
                let bits5_9   = ((merged_bytes & 0b00000_11111_00000_00000_00000_00000_00000_00000) >> 30) as u8;
                let bits10_14 = ((merged_bytes & 0b00000_00000_11111_00000_00000_00000_00000_00000) >> 25) as u8;
                let bits15_19 = ((merged_bytes & 0b00000_00000_00000_11111_00000_00000_00000_00000) >> 20) as u8;
                let bits20_23 = ((merged_bytes & 0b00000_00000_00000_00000_11110_00000_00000_00000) >> 15) as u8;

                decoded_bytes.extend([
                    u5_to_base32(u5::new(bits0_4)),
                    u5_to_base32(u5::new(bits5_9)),
                    u5_to_base32(u5::new(bits10_14)),
                    u5_to_base32(u5::new(bits15_19)),
                    u5_to_base32(u5::new(bits20_23)),
                    PADDING_CHAR,
                    PADDING_CHAR,
                    PADDING_CHAR,
                ]);
            },
            &[byte0, byte1, byte2, byte3] => {
                let merged_bytes = u64::from_be_bytes([0, 0, 0, byte0, byte1, byte2, byte3, 0]);

                let bits0_4   = ((merged_bytes & 0b11111_00000_00000_00000_00000_00000_00000_00000) >> 35) as u8;
                let bits5_9   = ((merged_bytes & 0b00000_11111_00000_00000_00000_00000_00000_00000) >> 30) as u8;
                let bits10_14 = ((merged_bytes & 0b00000_00000_11111_00000_00000_00000_00000_00000) >> 25) as u8;
                let bits15_19 = ((merged_bytes & 0b00000_00000_00000_11111_00000_00000_00000_00000) >> 20) as u8;
                let bits20_24 = ((merged_bytes & 0b00000_00000_00000_00000_11111_00000_00000_00000) >> 15) as u8;
                let bits25_29 = ((merged_bytes & 0b00000_00000_00000_00000_00000_11111_00000_00000) >> 10) as u8;
                let bits30_31 = ((merged_bytes & 0b00000_00000_00000_00000_00000_00000_11000_00000) >> 5) as u8;

                decoded_bytes.extend([
                    u5_to_base32(u5::new(bits0_4)),
                    u5_to_base32(u5::new(bits5_9)),
                    u5_to_base32(u5::new(bits10_14)),
                    u5_to_base32(u5::new(bits15_19)),
                    u5_to_base32(u5::new(bits20_24)),
                    u5_to_base32(u5::new(bits25_29)),
                    u5_to_base32(u5::new(bits30_31)),
                    PADDING_CHAR,
                ]);
            },
            _ => panic!("An impossible remainder length was reached. The length of a remainder can only be 0, 1, 2, 3, or 4.")
        }

        AsciiString::from(&decoded_bytes)
    }

    #[inline]
    pub fn verify_base32_string(string: &AsciiString) -> Result<(), Base32Error> {
        // Verify that all of the characters are valid for this alphabet.
        for character in string.iter() {
            if !is_base32_char(*character) {
                return Err(Base32Error::BadChar(*character));
            }
        }

        let string_chunks = string.as_slice().chunks_exact(8);

        // Verify that the format has the correct number of characters and won't overflow when decoding.
        if !string_chunks.remainder().is_empty() {
            return Err(Base32Error::Overflow);
        }

        // Verify that the format does not contain padding characters where they are not allowed.
        let mut padding_reached = false;
        for chunk in string_chunks {
            if padding_reached {
                return Err(Base32Error::Overflow);
            }

            match chunk {
                // 1st character can never be a padding character.
                &[PADDING_CHAR, _, _, _, _, _, _, _] => return Err(Base32Error::Overflow),
                // Characters 1, 2, & 3 can only be a padding character if every byte that follows is also a padding character.
                &[_, PADDING_CHAR, PADDING_CHAR, PADDING_CHAR, PADDING_CHAR, PADDING_CHAR, PADDING_CHAR, PADDING_CHAR] => padding_reached = true,
                &[_, PADDING_CHAR, _, _, _, _, _, _] => return Err(Base32Error::BadPadding),
                &[_, _, PADDING_CHAR, PADDING_CHAR, PADDING_CHAR, PADDING_CHAR, PADDING_CHAR, PADDING_CHAR] => padding_reached = true,
                &[_, _, PADDING_CHAR, _, _, _, _, _] => return Err(Base32Error::BadPadding),
                // 4th character can never be a padding character.
                &[_, _, _, PADDING_CHAR, _, _, _, _] => return Err(Base32Error::BadPadding),
                // Characters 5 & 6 can only be a padding character if every byte that follows is also a padding character.
                &[_, _, _, _, PADDING_CHAR, PADDING_CHAR, PADDING_CHAR, PADDING_CHAR] => padding_reached = true,
                &[_, _, _, _, PADDING_CHAR, _, _, _] => return Err(Base32Error::BadPadding),
                &[_, _, _, _, _, PADDING_CHAR, PADDING_CHAR, PADDING_CHAR] => padding_reached = true,
                &[_, _, _, _, _, PADDING_CHAR, _, _] => return Err(Base32Error::BadPadding),
                // 7th character can never be a padding character.
                &[_, _, _, _, _, _, PADDING_CHAR, _] => return Err(Base32Error::BadPadding),
                // 8th character can be padding but does not have to be.
                &[_, _, _, _, _, _, _, PADDING_CHAR] => padding_reached = true,
                &[_, _, _, _, _, _, _, _] => (),
                _ => panic!("The pattern was supposed to chunk exactly 8 bytes. However, the chunk contained {} bytes", chunk.len()),
            }
        }

        return Ok(());
    }
}

impl BaseConversions for Base32 {
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
        let base = (self.bytes.len() * 8).div_ceil(5);
        // + Padding
        base.next_multiple_of(8)
    }
}

impl FromPresentation for Base32 {
    #[inline]
    fn from_token_format<'a, 'b, 'c, 'd>(tokens: &'c [&'a str]) -> Result<(Self, &'d [&'a str]), TokenError<'b>> where Self: Sized, 'a: 'b, 'c: 'd, 'c: 'd {
        let (encoded, tokens) = AsciiString::from_token_format(tokens)?;
        Ok((Self::encode(&encoded)?, tokens))
    }
}

#[cfg(test)]
mod circular_sanity_tests {
    use crate::{types::base_conversions::BaseConversions, serde::wire::circular_test::gen_test_circular_serde_sanity_test};
    use super::Base32;

    macro_rules! circular_sanity_test {
        ($test_encoding:ident, $test_wire:ident, $input:expr) => {
            #[test]
            fn $test_encoding() {
                let init_bytes = $input;
                let input = Base32 { bytes: init_bytes.clone() };
                let guessed_string_len = input.string_len();
                assert_eq!(init_bytes.len(), input.byte_len());

                let decoded = input.decode();
                let verified = Base32::verify_base32_string(&decoded);
                assert!(verified.is_ok());
                assert_eq!(decoded.len(), guessed_string_len);

                let encoded = Base32::encode(&decoded);
                assert!(encoded.is_ok());
                let output = encoded.unwrap();
                assert_eq!(input, output);
                assert_eq!(init_bytes.len(), output.byte_len());
            }

            gen_test_circular_serde_sanity_test!($test_wire, Base32::from_bytes($input));
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

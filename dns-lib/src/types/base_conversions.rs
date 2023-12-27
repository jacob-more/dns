use crate::types::{base32::Base32, extended_base32::ExtendedBase32, base64::Base64, base16::Base16};

use super::ascii::AsciiString;

pub trait BaseConversions {
    fn from_vec(bytes: Vec<u8>) -> Self;

    fn to_bytes(&self) -> &[u8];
    fn to_ascii(&self) -> AsciiString;

    fn string_len(&self) -> usize;

    #[inline]
    fn from_bytes(bytes: &[u8]) -> Self where Self: Sized {
        Self::from_vec(Vec::from(bytes))
    }

    #[inline]
    fn to_base16(&self) -> Base16 {
        Base16::from_bytes(self.to_bytes())
    }

    #[inline]
    fn to_base32(&self) -> Base32 {
        Base32::from_bytes(self.to_bytes())
    }

    #[inline]
    fn to_extended_base32(&self) -> ExtendedBase32 {
        ExtendedBase32::from_bytes(self.to_bytes())
    }

    #[inline]
    fn to_base64(&self) -> Base64 {
        Base64::from_bytes(self.to_bytes())
    }

    #[inline]
    fn byte_len(&self) -> usize {
        self.to_bytes().len()
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.to_bytes().is_empty()
    }
}

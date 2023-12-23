use crate::types::{base32::Base32, extended_base32::ExtendedBase32, base64::Base64, base16::Base16};

pub trait BaseConversions {
    fn from_vec(bytes: Vec<u8>) -> Self;
    fn from_bytes(bytes: &[u8]) -> Self;

    fn to_bytes(&self) -> &Vec<u8>;

    #[inline]
    fn to_base16(&self) -> Base16 {
        Base16::from_vec(self.to_bytes().clone())
    }

    #[inline]
    fn to_base32(&self) -> Base32 {
        Base32::from_vec(self.to_bytes().clone())
    }

    #[inline]
    fn to_extended_base32(&self) -> ExtendedBase32 {
        ExtendedBase32::from_vec(self.to_bytes().clone())
    }

    #[inline]
    fn to_base64(&self) -> Base64 {
        Base64::from_vec(self.to_bytes().clone())
    }
}

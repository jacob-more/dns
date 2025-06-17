use crate::{
    serde::{
        presentation::to_presentation::ToPresentation,
        wire::{from_wire::FromWire, to_wire::ToWire},
    },
    types::{base16::Base16, base32::Base32, base64::Base64, extended_base32::ExtendedBase32},
};

use super::ascii::AsciiString;

pub trait BaseConversions {
    fn from_vec(bytes: Vec<u8>) -> Self;

    fn to_bytes(&self) -> &[u8];
    fn to_ascii(&self) -> AsciiString;

    fn string_len(&self) -> usize;

    #[inline]
    fn from_bytes(bytes: &[u8]) -> Self
    where
        Self: Sized,
    {
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

impl<T: BaseConversions> ToWire for T {
    #[inline]
    fn to_wire_format<'a, 'b>(
        &self,
        wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>,
        _compression: &mut Option<crate::types::c_domain_name::CompressionMap>,
    ) -> Result<(), crate::serde::wire::write_wire::WriteWireError>
    where
        'a: 'b,
    {
        wire.write_bytes(self.to_bytes())
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        self.byte_len() as u16
    }
}

impl<T: BaseConversions> FromWire for T {
    #[inline]
    fn from_wire_format<'a, 'b>(
        wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>,
    ) -> Result<Self, crate::serde::wire::read_wire::ReadWireError>
    where
        Self: Sized,
        'a: 'b,
    {
        let base = Self::from_bytes(wire.take_all());
        return Ok(base);
    }
}

impl<T: BaseConversions> ToPresentation for T {
    #[inline]
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        self.to_ascii().to_presentation_format(out_buffer)
    }
}

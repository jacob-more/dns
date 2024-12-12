// https://www.rfc-editor.org/rfc/rfc1700
//
// When serializing and deserializing, recall that network order is defined to be Big Endian.
// Therefore, all data output by serialization must be Big Endian.
// All data input to a deserializer must be Big Endian.

use std::net::{Ipv4Addr, Ipv6Addr};

use mac_address::MacAddress;
use tinyvec::{ArrayVec, TinyVec};
use ux::{u24, u40, u48, u56, u72, u80, u88, u96, u104, u112, u120, i24, i40, i48, i56, i72, i80, i88, i96, i104, i112, i120, u1, u4, u3, u7};

use crate::serde::const_byte_counts::*;

use super::read_wire::{ReadWire, ReadWireError};

pub trait FromWire {
    fn from_wire_format<'a, 'b>(wire: &'b mut ReadWire<'a>) -> Result<Self, ReadWireError> where Self: Sized, 'a: 'b;
}

// #################### BUILT-IN PRIMITIVE TYPES ####################

macro_rules! int_from_wire_impl {
    ($int_name:literal, $int_type:ty, $byte_count:ident) => {
        impl FromWire for $int_type {
            #[inline]
            fn from_wire_format<'a, 'b>(wire: &'b mut ReadWire<'a>) -> Result<Self, ReadWireError> where Self: Sized, 'a: 'b {
                // `unwrap()` is safe in this case because it will always have a fixed size.
                let bytes = wire.take_or_err($byte_count as usize, || format!("could not read {}; wire length less than {} bytes", $int_name, $byte_count))?
                    .try_into()
                    .unwrap();

                Ok(Self::from_be_bytes(bytes))
            }
        }
    }
}

int_from_wire_impl!("u8",   u8,   U8_BYTE_COUNT);
int_from_wire_impl!("u16",  u16,  U16_BYTE_COUNT);
int_from_wire_impl!("u32",  u32,  U32_BYTE_COUNT);
int_from_wire_impl!("u64",  u64,  U64_BYTE_COUNT);
int_from_wire_impl!("u128", u128, U128_BYTE_COUNT);

int_from_wire_impl!("i8",   i8,   I8_BYTE_COUNT);
int_from_wire_impl!("i16",  i16,  I16_BYTE_COUNT);
int_from_wire_impl!("i32",  i32,  I32_BYTE_COUNT);
int_from_wire_impl!("i64",  i64,  I64_BYTE_COUNT);
int_from_wire_impl!("i128", i128, I128_BYTE_COUNT);

// #################### UX PRIMITIVE TYPES ####################

macro_rules! ux_from_wire_impl {
    ($int_name:literal, $int_type:ty, $int_byte_count:ident, $super_type:ty, $super_byte_count:ident) => {
        impl FromWire for $int_type {
            #[inline]
            fn from_wire_format<'a, 'b>(wire: &'b mut ReadWire<'a>) -> Result<Self, ReadWireError> where Self: Sized, 'a: 'b {
                let bytes = wire.take_or_err($int_byte_count as usize, || format!("could not read {}; wire length less than {} bytes", $int_name, $int_byte_count))?;
                let bytes: Vec<u8> = [0_u8].iter().cycle().take(($super_byte_count as usize) - ($int_byte_count as usize))
                    .chain(bytes.iter())
                    .map(|x| *x)
                    .collect();
                // `unwrap()` is safe in this case because it will always have a fixed size.
                let bytes: [u8; $super_byte_count as usize] = bytes.try_into().unwrap();

                Ok(Self::new(<$super_type>::from_be_bytes(bytes)))
            }
        }
    }
}

macro_rules! ix_from_wire_impl {
    ($int_name:literal, $int_type:ty, $int_byte_count:ident, $super_type:ty, $super_byte_count:ident) => {
        impl FromWire for $int_type {
            #[inline]
            fn from_wire_format<'a, 'b>(wire: &'b mut ReadWire<'a>) -> Result<Self, ReadWireError> where Self: Sized, 'a: 'b {
                // The match statement is used for sign extension.
                let fill_remaining = match wire.get_byte()? >> 7 {
                    0 => 0_u8,
                    1 => u8::MAX,
                    _ => unreachable!("When a u8 is shifted to the right 7 times, only 1 bit should remain. However, a case with more than 1 bit has been reached."),
                };
                let bytes = wire.take_or_err($int_byte_count as usize, || format!("could not read {}; wire length less than {} bytes", $int_name, $int_byte_count))?;
                let bytes: Vec<u8> = [fill_remaining].iter().cycle().take(($super_byte_count as usize) - ($int_byte_count as usize))
                    .chain(bytes.iter())
                    .map(|x| *x)
                    .collect();
                // `unwrap()` is safe in this case because it will always have a fixed size.
                let bytes: [u8; $super_byte_count as usize] = bytes.try_into().unwrap();

                Ok(Self::new(<$super_type>::from_be_bytes(bytes)))
            }
        }
    }
}

ux_from_wire_impl!( "u24",  u24,  U24_BYTE_COUNT,  u32,  U32_BYTE_COUNT);
ux_from_wire_impl!( "u40",  u40,  U40_BYTE_COUNT,  u64,  U64_BYTE_COUNT);
ux_from_wire_impl!( "u48",  u48,  U48_BYTE_COUNT,  u64,  U64_BYTE_COUNT);
ux_from_wire_impl!( "u56",  u56,  U56_BYTE_COUNT,  u64,  U64_BYTE_COUNT);
ux_from_wire_impl!( "u72",  u72,  U72_BYTE_COUNT, u128, U128_BYTE_COUNT);
ux_from_wire_impl!( "u80",  u80,  U80_BYTE_COUNT, u128, U128_BYTE_COUNT);
ux_from_wire_impl!( "u88",  u88,  U88_BYTE_COUNT, u128, U128_BYTE_COUNT);
ux_from_wire_impl!( "u96",  u96,  U96_BYTE_COUNT, u128, U128_BYTE_COUNT);
ux_from_wire_impl!("u104", u104, U104_BYTE_COUNT, u128, U128_BYTE_COUNT);
ux_from_wire_impl!("u112", u112, U112_BYTE_COUNT, u128, U128_BYTE_COUNT);
ux_from_wire_impl!("u120", u120, U120_BYTE_COUNT, u128, U128_BYTE_COUNT);

ix_from_wire_impl!( "i24",  i24,  I24_BYTE_COUNT,  i32,  I32_BYTE_COUNT);
ix_from_wire_impl!( "i40",  i40,  I40_BYTE_COUNT,  i64,  I64_BYTE_COUNT);
ix_from_wire_impl!( "i48",  i48,  I48_BYTE_COUNT,  i64,  I64_BYTE_COUNT);
ix_from_wire_impl!( "i56",  i56,  I56_BYTE_COUNT,  i64,  I64_BYTE_COUNT);
ix_from_wire_impl!( "i72",  i72,  I72_BYTE_COUNT, i128, I128_BYTE_COUNT);
ix_from_wire_impl!( "i80",  i80,  I80_BYTE_COUNT, i128, I128_BYTE_COUNT);
ix_from_wire_impl!( "i88",  i88,  I88_BYTE_COUNT, i128, I128_BYTE_COUNT);
ix_from_wire_impl!( "i96",  i96,  I96_BYTE_COUNT, i128, I128_BYTE_COUNT);
ix_from_wire_impl!("i104", i104, I104_BYTE_COUNT, i128, I128_BYTE_COUNT);
ix_from_wire_impl!("i112", i112, I112_BYTE_COUNT, i128, I128_BYTE_COUNT);
ix_from_wire_impl!("i120", i120, I120_BYTE_COUNT, i128, I128_BYTE_COUNT);

// #################### OTHER COMMON TYPES ####################

impl<T: FromWire> FromWire for Option<T> {
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut ReadWire<'a>) -> Result<Self, ReadWireError> where Self: Sized, 'a: 'b {
        match wire.current_len() {
            0   => Ok(None),
            1.. => Ok(Some(T::from_wire_format(wire)?)),
        }
    }
}

impl<T: FromWire> FromWire for Vec<T> {
    /// Consumes the entire wire buffer.
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut ReadWire<'a>) -> Result<Self, ReadWireError> where Self: Sized, 'a: 'b {
        let mut vector = Vec::new();

        while wire.current_len() > 0 {
            vector.push(T::from_wire_format(wire)?);
        }

        Ok(vector)
    }
}

impl<T: FromWire + Default, const SIZE: usize> FromWire for TinyVec<[T; SIZE]> where [T; SIZE]: tinyvec::Array<Item = T> {
    /// Consumes the entire wire buffer.
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut ReadWire<'a>) -> Result<Self, ReadWireError> where Self: Sized, 'a: 'b {
        let mut vector = TinyVec::new();

        while wire.current_len() > 0 {
            vector.push(T::from_wire_format(wire)?);
        }

        Ok(vector)
    }
}

impl<T: FromWire + Default, const SIZE: usize> FromWire for ArrayVec<[T; SIZE]> where [T; SIZE]: tinyvec::Array<Item = T> {
    /// Consumes the entire wire buffer.
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut ReadWire<'a>) -> Result<Self, ReadWireError> where Self: Sized, 'a: 'b {
        let mut vector = ArrayVec::new();

        while wire.current_len() > 0 {
            vector.push(T::from_wire_format(wire)?);
        }

        Ok(vector)
    }
}

impl FromWire for Ipv4Addr {
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut ReadWire<'a>) -> Result<Self, ReadWireError> where Self: Sized, 'a: 'b {
        // `unwrap()` is safe in this case because it will always have a fixed size.
        let bytes: [u8; IPV4_BYTE_COUNT as usize] = wire.take_or_err(IPV4_BYTE_COUNT as usize, || format!("IPv4 Addresses must be {IPV4_BYTE_COUNT} bytes in length"))?
            .try_into()
            .unwrap();

        Ok(Ipv4Addr::from(bytes))
    }
}

impl FromWire for Ipv6Addr {
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut ReadWire<'a>) -> Result<Self, ReadWireError> where Self: Sized, 'a: 'b {
        // `unwrap()` is safe in this case because it will always have a fixed size.
        let bytes: [u8; IPV6_BYTE_COUNT as usize] = wire.take_or_err(IPV6_BYTE_COUNT as usize, || format!("IPv6 Addresses must be {IPV6_BYTE_COUNT} bytes in length"))?
            .try_into()
            .unwrap();

        Ok(Ipv6Addr::from(bytes))
    }
}

impl FromWire for MacAddress {
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut ReadWire<'a>) -> Result<Self, ReadWireError> where Self: Sized, 'a: 'b {
        // `unwrap()` is safe in this case because it will always have a fixed size.
        let bytes: [u8; MAC_ADDRESS_BYTE_COUNT as usize] = wire.take_or_err(MAC_ADDRESS_BYTE_COUNT as usize, || format!("Mac Addresses must be {MAC_ADDRESS_BYTE_COUNT} bytes in length"))?
            .try_into()
            .unwrap();

        Ok(MacAddress::from(bytes))
    }
}

impl FromWire for (u1, u7) {
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut ReadWire<'a>) -> Result<Self, ReadWireError> where Self: Sized, 'a: 'b {
        let bit_7to0 = u8::from_wire_format(wire)?;

        // | 0  | 0 0 0 0 0 0 0 |
        // | u1 | u7            |
        let bit_7    = u1::new( (bit_7to0 >> 7) & 0b00000001 );
        let bit_6to0 = u7::new( (bit_7to0 >> 0) & 0b01111111 );

        Ok((bit_7, bit_6to0))
    }
}

impl FromWire for (u1, u3, u4) {
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut ReadWire<'a>) -> Result<Self, ReadWireError> where Self: Sized, 'a: 'b {
        let input = u8::from_wire_format(wire)?;

        // | 0  | 0 0 0 | 0 0 0 0 |
        // | u1 | u3    | u4      |
        let bit_7    = u1::new( (input >> 7) & 0b00000001 );
        let bit_6to4 = u3::new( (input >> 4) & 0b00000111 );
        let bit_3to0 = u4::new( (input >> 0) & 0b00001111 );

        Ok((bit_7, bit_6to4, bit_3to0))
    }
}

impl FromWire for (u1, u4, u1, u1, u1) {
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut ReadWire<'a>) -> Result<Self, ReadWireError> where Self: Sized, 'a: 'b {
        let input = u8::from_wire_format(wire)?;

        // | 0  | 0 0 0 0 | 0  | 0  | 0  |
        // | u1 | u4      | u1 | u1 | u1 |
        let bit_7    = u1::new( (input >> 7) & 0b00000001 );
        let bit_6to3 = u4::new( (input >> 3) & 0b00001111 );
        let bit_2    = u1::new( (input >> 2) & 0b00000001 );
        let bit_1    = u1::new( (input >> 1) & 0b00000001 );
        let bit_0    = u1::new( (input >> 0) & 0b00000001 );

        Ok((bit_7, bit_6to3, bit_2, bit_1, bit_0))
    }
}

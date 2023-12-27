// https://www.rfc-editor.org/rfc/rfc1700
// 
// When serializing and deserializing, recall that network order is defined to be Big Endian.
// Therefore, all data output by serialization must be Big Endian.
// All data input to a deserializer must be Big Endian.

use std::net::{Ipv4Addr, Ipv6Addr};

use mac_address::MacAddress;
use ux::{u24, u40, u48, u56, u72, u80, u88, u96, u104, u112, u120, i24, i40, i48, i56, i72, i80, i88, i96, i104, i112, i120};

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
                if wire.current_state_len() < ($byte_count as usize) {
                    return Err(ReadWireError::UnderflowError(format!("could not read {}; wire length less than {} bytes", $int_name, $byte_count)));
                }
        
                // `unwrap()` is safe in this case because it will always have a fixed size.
                let bytes = wire.current_state()[0..($byte_count as usize)].try_into().unwrap();
                wire.shift($byte_count as usize)?;
        
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
                if wire.current_state_len() < ($int_byte_count as usize) {
                    return Err(ReadWireError::UnderflowError(format!("could not read {}; wire length less than {} bytes", $int_name, $int_byte_count)));
                }

                let bytes: Vec<u8> = [0_u8].iter().cycle().take(($super_byte_count as usize) - ($int_byte_count as usize))
                    .chain(wire.current_state()[0..($int_byte_count as usize)].iter())
                    .map(|x| *x)
                    .collect();
                // `unwrap()` is safe in this case because it will always have a fixed size.
                let bytes: [u8; $super_byte_count as usize] = bytes.try_into().unwrap();
                wire.shift($int_byte_count as usize)?;
        
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
                if wire.current_state_len() < ($int_byte_count as usize) {
                    return Err(ReadWireError::UnderflowError(format!("could not read {}; wire length less than {} bytes", $int_name, $int_byte_count)));
                }

                // The match statement is done for sign extension.
                let fill_remaining = match wire.current_state()[0] >> 7 {
                    0 => 0_u8,
                    1 => u8::MAX,
                    _ => panic!("When a u8 is shifted to the right 7 times, only 1 bit should remain. However, a case with more than 1 bit has been reached."),
                };
                let bytes: Vec<u8> = [fill_remaining].iter().cycle().take(($super_byte_count as usize) - ($int_byte_count as usize))
                    .chain(wire.current_state()[0..($int_byte_count as usize)].iter())
                    .map(|x| *x)
                    .collect();
                // `unwrap()` is safe in this case because it will always have a fixed size.
                let bytes: [u8; $super_byte_count as usize] = bytes.try_into().unwrap();
                wire.shift($int_byte_count as usize)?;
        
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
        match wire.current_state_len() {
            0 => Ok(None),
            _ => Ok(Some(T::from_wire_format(wire)?)),
        }
    }
}

impl<T: FromWire> FromWire for Vec<T> {
    /// Consumes the entire wire buffer.
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut ReadWire<'a>) -> Result<Self, ReadWireError> where Self: Sized, 'a: 'b {
        let mut vector = Vec::new();
        
        while wire.current_state_len() > 0 {
            vector.push(T::from_wire_format(wire)?);
        }

        Ok(vector)
    }
}

impl FromWire for Ipv4Addr {
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut ReadWire<'a>) -> Result<Self, ReadWireError> where Self: Sized, 'a: 'b {
        if wire.current_state_len() < (IPV4_BYTE_COUNT as usize) {
            return Err(ReadWireError::UnderflowError(format!("IPv4 Addresses must be {} bytes in length", IPV4_BYTE_COUNT)));
        }

        // `unwrap()` is safe in this case because it will always have a fixed size.
        let bytes: [u8; IPV4_BYTE_COUNT as usize] = wire.current_state()[0..(IPV4_BYTE_COUNT as usize)].try_into().unwrap();
        wire.shift(IPV4_BYTE_COUNT as usize)?;

        Ok(Ipv4Addr::from(bytes))
    }
}

impl FromWire for Ipv6Addr {
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut ReadWire<'a>) -> Result<Self, ReadWireError> where Self: Sized, 'a: 'b {
        if wire.current_state_len() < (IPV6_BYTE_COUNT as usize) {
            return Err(ReadWireError::UnderflowError(format!("IPv6 Addresses must be {} bytes in length", IPV6_BYTE_COUNT)));
        }

        // `unwrap()` is safe in this case because it will always have a fixed size.
        let bytes: [u8; IPV6_BYTE_COUNT as usize] = wire.current_state()[0..(IPV6_BYTE_COUNT as usize)].try_into().unwrap();
        wire.shift(IPV6_BYTE_COUNT as usize)?;

        Ok(Ipv6Addr::from(bytes))
    }
}

impl FromWire for MacAddress {
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut ReadWire<'a>) -> Result<Self, ReadWireError> where Self: Sized, 'a: 'b {
        if wire.current_state_len() < (MAC_ADDRESS_BYTE_COUNT as usize) {
            return Err(ReadWireError::UnderflowError(format!("Mac Addresses must be {} bytes in length", MAC_ADDRESS_BYTE_COUNT)));
        }
        
        // `unwrap()` is safe in this case because it will always have a fixed size.
        let bytes: [u8; MAC_ADDRESS_BYTE_COUNT as usize] = wire.current_state()[0..(MAC_ADDRESS_BYTE_COUNT as usize)].try_into().unwrap();
        wire.shift(MAC_ADDRESS_BYTE_COUNT as usize)?;

        Ok(MacAddress::from(bytes))
    }
}

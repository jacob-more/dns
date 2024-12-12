// https://www.rfc-editor.org/rfc/rfc1700
//
// When serializing and deserializing, recall that network order is defined to be Big Endian.
// Therefore, all data output by serialization must be Big Endian.
// All data input to a deserializer must be Big Endian.

use std::net::{Ipv4Addr, Ipv6Addr, IpAddr};

use mac_address::MacAddress;
use ux::{u24, u40, u48, u56, i24, i40, i48, i56, u1, u4, u3, u7};

use crate::{serde::const_byte_counts::*, types::c_domain_name::CompressionMap};

use super::write_wire::{WriteWire, WriteWireError};

pub trait ToWire {
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut WriteWire<'a>, compression: &mut Option<CompressionMap>) -> Result<(), WriteWireError> where 'a: 'b;
    fn serial_length(&self) -> u16;
}

// #################### BUILT-IN PRIMITIVE TYPES ####################

macro_rules! int_to_wire_impl {
    ($int_name:literal, $int_type:ty, $byte_count:ident) => {
        impl ToWire for $int_type {
            #[inline]
            fn to_wire_format<'a, 'b>(&self, wire: &'b mut WriteWire<'a>, _compression: &mut Option<CompressionMap>) -> Result<(), WriteWireError> where 'a: 'b {
                wire.write_bytes(&self.to_be_bytes())
            }

            #[inline]
            fn serial_length(&self) -> u16 {
                $byte_count
            }
        }
    }
}

int_to_wire_impl!("u8",   u8,   U8_BYTE_COUNT);
int_to_wire_impl!("u16",  u16,  U16_BYTE_COUNT);
int_to_wire_impl!("u32",  u32,  U32_BYTE_COUNT);
int_to_wire_impl!("u64",  u64,  U64_BYTE_COUNT);
int_to_wire_impl!("u128", u128, U128_BYTE_COUNT);

int_to_wire_impl!("i8",   i8,   I8_BYTE_COUNT);
int_to_wire_impl!("i16",  i16,  I16_BYTE_COUNT);
int_to_wire_impl!("i32",  i32,  I32_BYTE_COUNT);
int_to_wire_impl!("i64",  i64,  I64_BYTE_COUNT);
int_to_wire_impl!("i128", i128, I128_BYTE_COUNT);

// #################### UX PRIMITIVE TYPES ####################

macro_rules! ux_to_wire_impl {
    ($int_name:literal, $int_type:ty, $byte_count:ident, $super_type:ty, $super_byte_count:ident) => {
        impl ToWire for $int_type {
            #[inline]
            fn to_wire_format<'a, 'b>(&self, wire: &'b mut WriteWire<'a>, _compression: &mut Option<CompressionMap>) -> Result<(), WriteWireError> where 'a: 'b {
                wire.write_bytes(&(<$super_type>::from(*self)).to_be_bytes()[(($super_byte_count as usize) - ($byte_count as usize))..])
            }

            #[inline]
            fn serial_length(&self) -> u16 {
                $byte_count
            }
        }
    }
}

ux_to_wire_impl!("u24",  u24,  U24_BYTE_COUNT,  u32,  U32_BYTE_COUNT);
ux_to_wire_impl!("u40",  u40,  U40_BYTE_COUNT,  u64,  U64_BYTE_COUNT);
ux_to_wire_impl!("u48",  u48,  U48_BYTE_COUNT,  u64,  U64_BYTE_COUNT);
ux_to_wire_impl!("u56",  u56,  U56_BYTE_COUNT,  u64,  U64_BYTE_COUNT);
// FIXME: There is no From<u128> implementation for `ux` types. Until then, cannot implement ToWire for them.
// ux_to_wire_impl!("u72",  u72,  U72_BYTE_COUNT,  u128, U128_BYTE_COUNT);
// ux_to_wire_impl!("u80",  u80,  U80_BYTE_COUNT,  u128, U128_BYTE_COUNT);
// ux_to_wire_impl!("u88",  u88,  U88_BYTE_COUNT,  u128, U128_BYTE_COUNT);
// ux_to_wire_impl!("u96",  u96,  U96_BYTE_COUNT,  u128, U128_BYTE_COUNT);
// ux_to_wire_impl!("u104", u104, U104_BYTE_COUNT, u128, U128_BYTE_COUNT);
// ux_to_wire_impl!("u112", u112, U112_BYTE_COUNT, u128, U128_BYTE_COUNT);
// ux_to_wire_impl!("u120", u120, U120_BYTE_COUNT, u128, U128_BYTE_COUNT);

ux_to_wire_impl!("i24",  i24,  I24_BYTE_COUNT,  i32,  I32_BYTE_COUNT);
ux_to_wire_impl!("i40",  i40,  I40_BYTE_COUNT,  i64,  I64_BYTE_COUNT);
ux_to_wire_impl!("i48",  i48,  I48_BYTE_COUNT,  i64,  I64_BYTE_COUNT);
ux_to_wire_impl!("i56",  i56,  I56_BYTE_COUNT,  i64,  I64_BYTE_COUNT);
// FIXME: There is no From<i128> implementation for `ux` types. Until then, cannot implement ToWire for them.
// ux_to_wire_impl!("i72",  i72,  I72_BYTE_COUNT,  i128, I128_BYTE_COUNT);
// ux_to_wire_impl!("i80",  i80,  I80_BYTE_COUNT,  i128, I128_BYTE_COUNT);
// ux_to_wire_impl!("i88",  i88,  I88_BYTE_COUNT,  i128, I128_BYTE_COUNT);
// ux_to_wire_impl!("i96",  i96,  I96_BYTE_COUNT,  i128, I128_BYTE_COUNT);
// ux_to_wire_impl!("i104", i104, I104_BYTE_COUNT, i128, I128_BYTE_COUNT);
// ux_to_wire_impl!("i112", i112, I112_BYTE_COUNT, i128, I128_BYTE_COUNT);
// ux_to_wire_impl!("i120", i120, I120_BYTE_COUNT, i128, I128_BYTE_COUNT);

// #################### OTHER COMMON TYPES ####################

impl<T: ToWire> ToWire for Option<T> {
    #[inline]
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut WriteWire<'a>, compression: &mut Option<CompressionMap>) -> Result<(), WriteWireError> where 'a: 'b {
        match self {
            None => Ok(()),
            Some(x) => x.to_wire_format(wire, compression),
        }
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        match self {
            None => 0,
            Some(x) => x.serial_length(),
        }
    }
}

impl<T: ToWire> ToWire for Vec<T> {
    #[inline]
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut WriteWire<'a>, compression: &mut Option<CompressionMap>) -> Result<(), WriteWireError> where 'a: 'b {
        for x in self {
            x.to_wire_format(wire, compression)?;
        }

        Ok(())
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        self.iter()
            .map(|x| x.serial_length())
            .sum()
    }
}

impl<T: ToWire, const N: usize> ToWire for [T; N] {
    #[inline]
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut WriteWire<'a>, compression: &mut Option<CompressionMap>) -> Result<(), WriteWireError> where 'a: 'b {
        for x in self {
            x.to_wire_format(wire, compression)?;
        }

        Ok(())
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        let mut length = 0;
        for x in self {
            length += x.serial_length();
        }

        return length;
    }
}

impl ToWire for Ipv4Addr {
    #[inline]
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut WriteWire<'a>, _compression: &mut Option<CompressionMap>) -> Result<(), WriteWireError> where 'a: 'b {
        wire.write_bytes(&self.octets())
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        IPV4_BYTE_COUNT
    }
}

impl ToWire for Ipv6Addr {
    #[inline]
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut WriteWire<'a>, _compression: &mut Option<CompressionMap>) -> Result<(), WriteWireError> where 'a: 'b {
        wire.write_bytes(&self.octets())
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        IPV6_BYTE_COUNT
    }
}

impl ToWire for IpAddr {
    #[inline]
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut WriteWire<'a>, compression: &mut Option<CompressionMap>) -> Result<(), WriteWireError> where 'a: 'b {
        match self {
            IpAddr::V4(address) => address.to_wire_format(wire, compression),
            IpAddr::V6(address) => address.to_wire_format(wire, compression),
        }
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        match self {
            IpAddr::V4(address) => address.serial_length(),
            IpAddr::V6(address) => address.serial_length(),
        }
    }
}

impl ToWire for MacAddress {
    #[inline]
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut WriteWire<'a>, _compression: &mut Option<CompressionMap>) -> Result<(), WriteWireError> where 'a: 'b {
        wire.write_bytes(&self.bytes())
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        MAC_ADDRESS_BYTE_COUNT
    }
}

impl ToWire for (u1, u7) {
    #[inline]
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut WriteWire<'a>, compression: &mut Option<CompressionMap>) -> Result<(), WriteWireError> where 'a: 'b {
        let bit_7 = u16::from(self.0) as u8;    //< for some reason, there is no default conversion?
        let bit_6to0: u8 = self.1.into();

        // | 0  | 0 0 0 0 0 0 0 |
        // | u1 | u7            |
        let bit_7: u8    = (bit_7    << 7) & 0b10000000;
        let bit_6to0: u8 = (bit_6to0 << 0) & 0b01111111;
        let result = bit_7 | bit_6to0;

        result.to_wire_format(wire, compression)
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        return U8_BYTE_COUNT;
    }
}

impl ToWire for (u1, u3, u4) {
    #[inline]
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut WriteWire<'a>, compression: &mut Option<CompressionMap>) -> Result<(), WriteWireError> where 'a: 'b {
        let bit_7 = u16::from(self.0) as u8;
        let bit_6to4: u8 = self.1.into();
        let bit_3to0: u8 = self.2.into();

        // | 0  | 0 0 0 | 0 0 0 0 |
        // | u1 | u3    | u4      |
        let bit_7    = (bit_7    << 7) & 0b10000000;
        let bit_6to4 = (bit_6to4 << 4) & 0b01110000;
        let bit_3to0 = (bit_3to0 << 0) & 0b00001111;
        let result = bit_7 | bit_6to4 | bit_3to0;

        result.to_wire_format(wire, compression)
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        return U8_BYTE_COUNT;
    }
}

impl ToWire for (u1, u4, u1, u1, u1) {
    #[inline]
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut WriteWire<'a>, compression: &mut Option<CompressionMap>) -> Result<(), WriteWireError> where 'a: 'b {
        let bit_7 = u16::from(self.0) as u8;
        let bit_6to3: u8 = self.1.into();
        let bit_2 = u16::from(self.2) as u8;
        let bit_1 = u16::from(self.3) as u8;
        let bit_0 = u16::from(self.4) as u8;

        // | 0  | 0 0 0 0 | 0  | 0  | 0  |
        // | u1 | u4      | u1 | u1 | u1 |
        let bit_7    = (bit_7    << 7) & 0b10000000;
        let bit_6to3 = (bit_6to3 << 3) & 0b01111000;
        let bit_2    = (bit_2    << 2) & 0b00000100;
        let bit_1    = (bit_1    << 1) & 0b00000010;
        let bit_0    = (bit_0    << 0) & 0b00000001;
        let result = bit_7 | bit_6to3 | bit_2 | bit_1 | bit_0;

        result.to_wire_format(wire, compression)
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        return U8_BYTE_COUNT;
    }
}

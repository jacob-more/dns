use std::net::{Ipv6Addr, Ipv4Addr};

// #################### BUILT-IN PRIMITIVE UNSIGNED ####################

pub const   U8_BYTE_COUNT: u16 = (  u8::BITS / 8) as u16;
pub const  U16_BYTE_COUNT: u16 = ( u16::BITS / 8) as u16;
pub const  U32_BYTE_COUNT: u16 = ( u32::BITS / 8) as u16;
pub const  U64_BYTE_COUNT: u16 = ( u64::BITS / 8) as u16;
pub const U128_BYTE_COUNT: u16 = (u128::BITS / 8) as u16;

// #################### BUILT-IN PRIMITIVE SIGNED ####################

pub const   I8_BYTE_COUNT: u16 = (  i8::BITS / 8) as u16;
pub const  I16_BYTE_COUNT: u16 = ( i16::BITS / 8) as u16;
pub const  I32_BYTE_COUNT: u16 = ( i32::BITS / 8) as u16;
pub const  I64_BYTE_COUNT: u16 = ( i64::BITS / 8) as u16;
pub const I128_BYTE_COUNT: u16 = (i128::BITS / 8) as u16;

// #################### ADDRESS TYPES ####################

pub const IPV4_BYTE_COUNT: u16 = (Ipv4Addr::BITS / 8) as u16;
pub const IPV6_BYTE_COUNT: u16 = (Ipv6Addr::BITS / 8) as u16;
pub const MAC_ADDRESS_BYTE_COUNT: u16 = 6;

// #################### UX PRIMITIVE UNSIGNED ####################

pub const  U24_BYTE_COUNT: u16 = (  24 / 8) as u16;
pub const  U40_BYTE_COUNT: u16 = (  40 / 8) as u16;
pub const  U48_BYTE_COUNT: u16 = (  48 / 8) as u16;
pub const  U56_BYTE_COUNT: u16 = (  56 / 8) as u16;
pub const  U72_BYTE_COUNT: u16 = (  72 / 8) as u16;
pub const  U80_BYTE_COUNT: u16 = (  80 / 8) as u16;
pub const  U88_BYTE_COUNT: u16 = (  88 / 8) as u16;
pub const  U96_BYTE_COUNT: u16 = (  96 / 8) as u16;
pub const U104_BYTE_COUNT: u16 = ( 104 / 8) as u16;
pub const U112_BYTE_COUNT: u16 = ( 112 / 8) as u16;
pub const U120_BYTE_COUNT: u16 = ( 120 / 8) as u16;

// #################### UX PRIMITIVE SIGNED ####################

pub const  I24_BYTE_COUNT: u16 = (  24 / 8) as u16;
pub const  I40_BYTE_COUNT: u16 = (  40 / 8) as u16;
pub const  I48_BYTE_COUNT: u16 = (  48 / 8) as u16;
pub const  I56_BYTE_COUNT: u16 = (  56 / 8) as u16;
pub const  I72_BYTE_COUNT: u16 = (  72 / 8) as u16;
pub const  I80_BYTE_COUNT: u16 = (  80 / 8) as u16;
pub const  I88_BYTE_COUNT: u16 = (  88 / 8) as u16;
pub const  I96_BYTE_COUNT: u16 = (  96 / 8) as u16;
pub const I104_BYTE_COUNT: u16 = ( 104 / 8) as u16;
pub const I112_BYTE_COUNT: u16 = ( 112 / 8) as u16;
pub const I120_BYTE_COUNT: u16 = ( 120 / 8) as u16;

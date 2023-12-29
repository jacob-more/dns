use std::{net::{Ipv4Addr, Ipv6Addr}, str::FromStr};

use mac_address::MacAddress;

use super::errors::TokenError;

/// https://datatracker.ietf.org/doc/html/rfc1035#section-5
pub trait FromPresentation {
    fn from_token_format<'a>(token: &'a str) -> Result<Self, TokenError> where Self: Sized;
}

// #################### BUILT-IN PRIMITIVE TYPES ####################

macro_rules! int_from_token_impl {
    ($int_type:ty) => {
        impl FromPresentation for $int_type {
            #[inline]
            fn from_token_format<'a>(token: &'a str) -> Result<Self, TokenError> where Self: Sized {
                Ok(<$int_type>::from_str_radix(token, 10)?)
            }
        }
    }
}

int_from_token_impl!(u8);
int_from_token_impl!(u16);
int_from_token_impl!(u32);
int_from_token_impl!(u64);
int_from_token_impl!(u128);

int_from_token_impl!(i8);
int_from_token_impl!(i16);
int_from_token_impl!(i32);
int_from_token_impl!(i64);
int_from_token_impl!(i128);

// #################### UX PRIMITIVE TYPES ####################

macro_rules! ux_from_token_impl {
    ($int_type:ident, $super_type:ty) => {
        use ux::$int_type;

        impl FromPresentation for $int_type {
            #[inline]
            fn from_token_format<'a>(token: &'a str) -> Result<Self, TokenError> where Self: Sized {
                match <$int_type>::try_from(<$super_type>::from_str_radix(token, 10)?) {
                    Ok(integer) => Ok(integer),
                    Err(_) => Err(TokenError::UxTryFromIntError),
                }
            }
        }        
    }
}

ux_from_token_impl!(u24, u32);
ux_from_token_impl!(u40, u64);
ux_from_token_impl!(u48, u64);
ux_from_token_impl!(u56, u64);
// FIXME: There is no TryFrom<u128> implementation for `ux` types. Until then, cannot implement FromPresentation for them.
// ux_from_token_impl!(u72, u128);
// ux_from_token_impl!(u80, u128);
// ux_from_token_impl!(u88, u128);
// ux_from_token_impl!(u96, u128);
// ux_from_token_impl!(u104, u128);
// ux_from_token_impl!(u112, u128);
// ux_from_token_impl!(u120, u128);

ux_from_token_impl!(i24, i32);
ux_from_token_impl!(i40, i64);
ux_from_token_impl!(i48, i64);
ux_from_token_impl!(i56, i64);
// FIXME: There is no TryFrom<i128> implementation for `ux` types. Until then, cannot implement FromPresentation for them.
// ux_from_token_impl!(i72, i128);
// ux_from_token_impl!(i80, i128);
// ux_from_token_impl!(i88, i128);
// ux_from_token_impl!(i96, i128);
// ux_from_token_impl!(i104, i128);
// ux_from_token_impl!(i112, i128);
// ux_from_token_impl!(i120, i128);

// #################### OTHER COMMON TYPES ####################

macro_rules! address_from_token_impl {
    ($addr_type:ty) => {
        impl FromPresentation for $addr_type {
            #[inline]
            fn from_token_format<'a>(token: &'a str) -> Result<Self, TokenError> where Self: Sized {
                Ok(<$addr_type>::from_str(token)?)
            }
        }        
    }
}

address_from_token_impl!(Ipv4Addr);
address_from_token_impl!(Ipv6Addr);
address_from_token_impl!(MacAddress);

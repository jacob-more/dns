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

// u1 and i1 don't have TryFrom<u8> implementations. Since their implementation is trivial, this can
// be done easily.

use ux::{u1, i1};

impl FromPresentation for u1 {
    #[inline]
    fn from_token_format<'a>(token: &'a str) -> Result<Self, TokenError> where Self: Sized {
        match token {
            "0" => Ok(u1::new(0)),
            "1" => Ok(u1::new(1)),
            _ => Err(TokenError::UxTryFromIntError),
        }
    }
}

impl FromPresentation for i1 {
    #[inline]
    fn from_token_format<'a>(token: &'a str) -> Result<Self, TokenError> where Self: Sized {
        match token {
            "0" => Ok(i1::new(0)),
            "-1" => Ok(i1::new(-1)),
            _ => Err(TokenError::UxTryFromIntError),
        }
    }
}

ux_from_token_impl!(u2, u8);
ux_from_token_impl!(u3, u8);
ux_from_token_impl!(u4, u8);
ux_from_token_impl!(u5, u8);
ux_from_token_impl!(u6, u8);
ux_from_token_impl!(u7, u8);
ux_from_token_impl!(u9, u16);
ux_from_token_impl!(u10, u16);
ux_from_token_impl!(u11, u16);
ux_from_token_impl!(u12, u16);
ux_from_token_impl!(u13, u16);
ux_from_token_impl!(u14, u16);
ux_from_token_impl!(u15, u16);
ux_from_token_impl!(u17, u32);
ux_from_token_impl!(u18, u32);
ux_from_token_impl!(u19, u32);
ux_from_token_impl!(u20, u32);
ux_from_token_impl!(u21, u32);
ux_from_token_impl!(u22, u32);
ux_from_token_impl!(u23, u32);
ux_from_token_impl!(u24, u32);
ux_from_token_impl!(u25, u32);
ux_from_token_impl!(u26, u32);
ux_from_token_impl!(u27, u32);
ux_from_token_impl!(u28, u32);
ux_from_token_impl!(u29, u32);
ux_from_token_impl!(u30, u32);
ux_from_token_impl!(u31, u32);
ux_from_token_impl!(u33, u64);
ux_from_token_impl!(u34, u64);
ux_from_token_impl!(u35, u64);
ux_from_token_impl!(u36, u64);
ux_from_token_impl!(u37, u64);
ux_from_token_impl!(u38, u64);
ux_from_token_impl!(u39, u64);
ux_from_token_impl!(u40, u64);
ux_from_token_impl!(u41, u64);
ux_from_token_impl!(u42, u64);
ux_from_token_impl!(u43, u64);
ux_from_token_impl!(u44, u64);
ux_from_token_impl!(u45, u64);
ux_from_token_impl!(u46, u64);
ux_from_token_impl!(u47, u64);
ux_from_token_impl!(u48, u64);
ux_from_token_impl!(u49, u64);
ux_from_token_impl!(u50, u64);
ux_from_token_impl!(u51, u64);
ux_from_token_impl!(u52, u64);
ux_from_token_impl!(u53, u64);
ux_from_token_impl!(u54, u64);
ux_from_token_impl!(u55, u64);
ux_from_token_impl!(u56, u64);
ux_from_token_impl!(u57, u64);
ux_from_token_impl!(u58, u64);
ux_from_token_impl!(u59, u64);
ux_from_token_impl!(u60, u64);
ux_from_token_impl!(u61, u64);
ux_from_token_impl!(u62, u64);
ux_from_token_impl!(u63, u64);
// FIXME: There is no TryFrom<u128> implementation for `ux` types. Until then, cannot implement FromPresentation for them.
// ux_from_token_impl!(u65, u128);
// ux_from_token_impl!(u66, u128);
// ux_from_token_impl!(u67, u128);
// ux_from_token_impl!(u68, u128);
// ux_from_token_impl!(u69, u128);
// ux_from_token_impl!(u70, u128);
// ux_from_token_impl!(u71, u128);
// ux_from_token_impl!(u72, u128);
// ux_from_token_impl!(u73, u128);
// ux_from_token_impl!(u74, u128);
// ux_from_token_impl!(u75, u128);
// ux_from_token_impl!(u76, u128);
// ux_from_token_impl!(u77, u128);
// ux_from_token_impl!(u78, u128);
// ux_from_token_impl!(u79, u128);
// ux_from_token_impl!(u80, u128);
// ux_from_token_impl!(u81, u128);
// ux_from_token_impl!(u82, u128);
// ux_from_token_impl!(u83, u128);
// ux_from_token_impl!(u84, u128);
// ux_from_token_impl!(u85, u128);
// ux_from_token_impl!(u86, u128);
// ux_from_token_impl!(u87, u128);
// ux_from_token_impl!(u88, u128);
// ux_from_token_impl!(u89, u128);
// ux_from_token_impl!(u90, u128);
// ux_from_token_impl!(u91, u128);
// ux_from_token_impl!(u92, u128);
// ux_from_token_impl!(u93, u128);
// ux_from_token_impl!(u94, u128);
// ux_from_token_impl!(u95, u128);
// ux_from_token_impl!(u96, u128);
// ux_from_token_impl!(u97, u128);
// ux_from_token_impl!(u98, u128);
// ux_from_token_impl!(u99, u128);
// ux_from_token_impl!(u100, u128);
// ux_from_token_impl!(u101, u128);
// ux_from_token_impl!(u102, u128);
// ux_from_token_impl!(u103, u128);
// ux_from_token_impl!(u104, u128);
// ux_from_token_impl!(u105, u128);
// ux_from_token_impl!(u106, u128);
// ux_from_token_impl!(u107, u128);
// ux_from_token_impl!(u108, u128);
// ux_from_token_impl!(u109, u128);
// ux_from_token_impl!(u110, u128);
// ux_from_token_impl!(u111, u128);
// ux_from_token_impl!(u112, u128);
// ux_from_token_impl!(u113, u128);
// ux_from_token_impl!(u114, u128);
// ux_from_token_impl!(u115, u128);
// ux_from_token_impl!(u116, u128);
// ux_from_token_impl!(u117, u128);
// ux_from_token_impl!(u118, u128);
// ux_from_token_impl!(u119, u128);
// ux_from_token_impl!(u120, u128);
// ux_from_token_impl!(u121, u128);
// ux_from_token_impl!(u122, u128);
// ux_from_token_impl!(u123, u128);
// ux_from_token_impl!(u124, u128);
// ux_from_token_impl!(u125, u128);
// ux_from_token_impl!(u126, u128);
// ux_from_token_impl!(u127, u128);

ux_from_token_impl!(i2, i8);
ux_from_token_impl!(i3, i8);
ux_from_token_impl!(i4, i8);
ux_from_token_impl!(i5, i8);
ux_from_token_impl!(i6, i8);
ux_from_token_impl!(i7, i8);
ux_from_token_impl!(i9, i16);
ux_from_token_impl!(i10, i16);
ux_from_token_impl!(i11, i16);
ux_from_token_impl!(i12, i16);
ux_from_token_impl!(i13, i16);
ux_from_token_impl!(i14, i16);
ux_from_token_impl!(i15, i16);
ux_from_token_impl!(i17, i32);
ux_from_token_impl!(i18, i32);
ux_from_token_impl!(i19, i32);
ux_from_token_impl!(i20, i32);
ux_from_token_impl!(i21, i32);
ux_from_token_impl!(i22, i32);
ux_from_token_impl!(i23, i32);
ux_from_token_impl!(i24, i32);
ux_from_token_impl!(i25, i32);
ux_from_token_impl!(i26, i32);
ux_from_token_impl!(i27, i32);
ux_from_token_impl!(i28, i32);
ux_from_token_impl!(i29, i32);
ux_from_token_impl!(i30, i32);
ux_from_token_impl!(i31, i32);
ux_from_token_impl!(i33, i64);
ux_from_token_impl!(i34, i64);
ux_from_token_impl!(i35, i64);
ux_from_token_impl!(i36, i64);
ux_from_token_impl!(i37, i64);
ux_from_token_impl!(i38, i64);
ux_from_token_impl!(i39, i64);
ux_from_token_impl!(i40, i64);
ux_from_token_impl!(i41, i64);
ux_from_token_impl!(i42, i64);
ux_from_token_impl!(i43, i64);
ux_from_token_impl!(i44, i64);
ux_from_token_impl!(i45, i64);
ux_from_token_impl!(i46, i64);
ux_from_token_impl!(i47, i64);
ux_from_token_impl!(i48, i64);
ux_from_token_impl!(i49, i64);
ux_from_token_impl!(i50, i64);
ux_from_token_impl!(i51, i64);
ux_from_token_impl!(i52, i64);
ux_from_token_impl!(i53, i64);
ux_from_token_impl!(i54, i64);
ux_from_token_impl!(i55, i64);
ux_from_token_impl!(i56, i64);
ux_from_token_impl!(i57, i64);
ux_from_token_impl!(i58, i64);
ux_from_token_impl!(i59, i64);
ux_from_token_impl!(i60, i64);
ux_from_token_impl!(i61, i64);
ux_from_token_impl!(i62, i64);
ux_from_token_impl!(i63, i64);
// FIXME: There is no TryFrom<i128> implementation for `ux` types. Until then, cannot implement FromPresentation for them.
// ux_from_token_impl!(i65, i128);
// ux_from_token_impl!(i66, i128);
// ux_from_token_impl!(i67, i128);
// ux_from_token_impl!(i68, i128);
// ux_from_token_impl!(i69, i128);
// ux_from_token_impl!(i70, i128);
// ux_from_token_impl!(i71, i128);
// ux_from_token_impl!(i72, i128);
// ux_from_token_impl!(i73, i128);
// ux_from_token_impl!(i74, i128);
// ux_from_token_impl!(i75, i128);
// ux_from_token_impl!(i76, i128);
// ux_from_token_impl!(i77, i128);
// ux_from_token_impl!(i78, i128);
// ux_from_token_impl!(i79, i128);
// ux_from_token_impl!(i80, i128);
// ux_from_token_impl!(i81, i128);
// ux_from_token_impl!(i82, i128);
// ux_from_token_impl!(i83, i128);
// ux_from_token_impl!(i84, i128);
// ux_from_token_impl!(i85, i128);
// ux_from_token_impl!(i86, i128);
// ux_from_token_impl!(i87, i128);
// ux_from_token_impl!(i88, i128);
// ux_from_token_impl!(i89, i128);
// ux_from_token_impl!(i90, i128);
// ux_from_token_impl!(i91, i128);
// ux_from_token_impl!(i92, i128);
// ux_from_token_impl!(i93, i128);
// ux_from_token_impl!(i94, i128);
// ux_from_token_impl!(i95, i128);
// ux_from_token_impl!(i96, i128);
// ux_from_token_impl!(i97, i128);
// ux_from_token_impl!(i98, i128);
// ux_from_token_impl!(i99, i128);
// ux_from_token_impl!(i100, i128);
// ux_from_token_impl!(i101, i128);
// ux_from_token_impl!(i102, i128);
// ux_from_token_impl!(i103, i128);
// ux_from_token_impl!(i104, i128);
// ux_from_token_impl!(i105, i128);
// ux_from_token_impl!(i106, i128);
// ux_from_token_impl!(i107, i128);
// ux_from_token_impl!(i108, i128);
// ux_from_token_impl!(i109, i128);
// ux_from_token_impl!(i110, i128);
// ux_from_token_impl!(i111, i128);
// ux_from_token_impl!(i112, i128);
// ux_from_token_impl!(i113, i128);
// ux_from_token_impl!(i114, i128);
// ux_from_token_impl!(i115, i128);
// ux_from_token_impl!(i116, i128);
// ux_from_token_impl!(i117, i128);
// ux_from_token_impl!(i118, i128);
// ux_from_token_impl!(i119, i128);
// ux_from_token_impl!(i120, i128);
// ux_from_token_impl!(i121, i128);
// ux_from_token_impl!(i122, i128);
// ux_from_token_impl!(i123, i128);
// ux_from_token_impl!(i124, i128);
// ux_from_token_impl!(i125, i128);
// ux_from_token_impl!(i126, i128);
// ux_from_token_impl!(i127, i128);

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

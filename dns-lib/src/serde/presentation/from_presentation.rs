use std::{
    net::{Ipv4Addr, Ipv6Addr},
    str::FromStr,
};

use mac_address::MacAddress;

use super::errors::TokenError;

/// https://datatracker.ietf.org/doc/html/rfc1035#section-5
pub trait FromPresentation {
    fn from_token_format<'a, 'b, 'c, 'd>(
        tokens: &'c [&'a str],
    ) -> Result<(Self, &'d [&'a str]), TokenError>
    where
        Self: Sized,
        'a: 'b,
        'c: 'd,
        'c: 'd;
}

// #################### BUILT-IN PRIMITIVE TYPES ####################

macro_rules! int_from_token_impl {
    ($int_type:ty) => {
        impl FromPresentation for $int_type {
            #[inline]
            fn from_token_format<'a, 'b, 'c, 'd>(
                tokens: &'c [&'a str],
            ) -> Result<(Self, &'d [&'a str]), TokenError>
            where
                Self: Sized,
                'a: 'b,
                'c: 'd,
                'c: 'd,
            {
                match tokens {
                    [] => Err(TokenError::OutOfTokens),
                    [token, ..] => Ok((str::parse::<$int_type>(token)?, &tokens[1..])),
                }
            }
        }
    };
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

#[cfg(test)]
mod builtin_primitive_int_test {
    macro_rules! gen_unsigned_integer_test {
        ($test_name:ident, $type:ty) => {
            #[cfg(test)]
            mod $test_name {
                use crate::serde::presentation::test_from_presentation::{
                    gen_fail_token_test, gen_ok_token_test,
                };
                use lazy_static::lazy_static;
                use num_bigint::ToBigInt;

                lazy_static! {
                    pub static ref MAX_STR: String = <$type>::MAX.to_string();
                    pub static ref MIN_STR: String = <$type>::MIN.to_string();
                    pub static ref TOO_BIG_STR: String =
                        (<$type>::MAX.to_bigint().unwrap() + 1.to_bigint().unwrap()).to_string();
                }

                gen_ok_token_test!(test_ok_1, $type, 1, &["1"]);

                gen_ok_token_test!(test_ok_max, $type, <$type>::MAX, &[MAX_STR.as_str()]);
                gen_ok_token_test!(test_ok_min, $type, <$type>::MIN, &[MIN_STR.as_str()]);

                gen_fail_token_test!(test_fail_too_large, $type, &[TOO_BIG_STR.as_str()]);
                gen_fail_token_test!(test_fail_too_small, $type, &["-1"]);

                gen_fail_token_test!(test_fail_not_an_integer, $type, &["not_an_integer"]);
                gen_fail_token_test!(test_fail_empty_str, $type, &[""]);
            }
        };
    }

    gen_unsigned_integer_test!(test_u8, u8);
    gen_unsigned_integer_test!(test_u16, u16);
    gen_unsigned_integer_test!(test_u32, u32);
    gen_unsigned_integer_test!(test_u64, u64);
    gen_unsigned_integer_test!(test_u128, u128);

    macro_rules! gen_signed_integer_test {
        ($test_name:ident, $type:ty) => {
            #[cfg(test)]
            mod $test_name {
                use crate::serde::presentation::test_from_presentation::{
                    gen_fail_token_test, gen_ok_token_test,
                };
                use lazy_static::lazy_static;
                use num_bigint::ToBigInt;

                lazy_static! {
                    pub static ref MAX_STR: String = <$type>::MAX.to_string();
                    pub static ref MIN_STR: String = <$type>::MIN.to_string();
                    pub static ref TOO_SMALL_STR: String =
                        (<$type>::MAX.to_bigint().unwrap() + 1.to_bigint().unwrap()).to_string();
                    pub static ref TOO_BIG_STR: String =
                        (<$type>::MIN.to_bigint().unwrap() - 1.to_bigint().unwrap()).to_string();
                }

                gen_ok_token_test!(test_ok_neg_1, $type, -1, &["-1"]);
                gen_ok_token_test!(test_ok_0, $type, 0, &["0"]);
                gen_ok_token_test!(test_ok_1, $type, 1, &["1"]);

                gen_ok_token_test!(test_ok_max, $type, <$type>::MAX, &[MAX_STR.as_str()]);
                gen_ok_token_test!(test_ok_min, $type, <$type>::MIN, &[MIN_STR.as_str()]);

                gen_fail_token_test!(test_fail_too_large, $type, &[TOO_SMALL_STR.as_str()]);
                gen_fail_token_test!(test_fail_too_small, $type, &[TOO_BIG_STR.as_str()]);

                gen_fail_token_test!(test_fail_not_an_integer, $type, &["not_an_integer"]);
                gen_fail_token_test!(test_fail_empty_str, $type, &[""]);
            }
        };
    }

    gen_signed_integer_test!(test_i8, i8);
    gen_signed_integer_test!(test_i16, i16);
    gen_signed_integer_test!(test_i32, i32);
    gen_signed_integer_test!(test_i64, i64);
    gen_signed_integer_test!(test_i128, i128);
}

// #################### UX PRIMITIVE TYPES ####################

macro_rules! ux_from_token_impl {
    ($int_type:ident, $super_type:ty) => {
        use ux::$int_type;

        impl FromPresentation for $int_type {
            #[inline]
            fn from_token_format<'a, 'b, 'c, 'd>(
                tokens: &'c [&'a str],
            ) -> Result<(Self, &'d [&'a str]), TokenError>
            where
                Self: Sized,
                'a: 'b,
                'c: 'd,
            {
                match tokens {
                    [] => Err(TokenError::OutOfTokens),
                    [token, ..] => match <$int_type>::try_from(str::parse::<$super_type>(token)?) {
                        Ok(integer) => Ok((integer, &tokens[1..])),
                        Err(_) => Err(TokenError::UxTryFromIntError),
                    },
                }
            }
        }
    };
}

// u1 and i1 don't have TryFrom<u8> implementations. Since their implementation is trivial, this can
// be done easily.

use ux::{i1, u1};

impl FromPresentation for u1 {
    #[inline]
    fn from_token_format<'a, 'b, 'c, 'd>(
        tokens: &'c [&'a str],
    ) -> Result<(Self, &'d [&'a str]), TokenError>
    where
        Self: Sized,
        'a: 'b,
        'c: 'd,
    {
        match tokens {
            [] => Err(TokenError::OutOfTokens),
            ["0", ..] => Ok((u1::new(0), &tokens[1..])),
            ["1", ..] => Ok((u1::new(1), &tokens[1..])),
            _ => Err(TokenError::UxTryFromIntError),
        }
    }
}

impl FromPresentation for i1 {
    #[inline]
    fn from_token_format<'a, 'b, 'c, 'd>(
        tokens: &'c [&'a str],
    ) -> Result<(Self, &'d [&'a str]), TokenError>
    where
        Self: Sized,
        'a: 'b,
        'c: 'd,
    {
        match tokens {
            [] => Err(TokenError::OutOfTokens),
            ["0", ..] => Ok((i1::new(0), &tokens[1..])),
            ["-1", ..] => Ok((i1::new(-1), &tokens[1..])),
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

#[cfg(test)]
mod ux_primitive_int_test {
    macro_rules! gen_unsigned_integer_test {
        ($test_name:ident, $ux_type:ident, $super_type:ty) => {
            #[cfg(test)]
            mod $test_name {
                use crate::serde::presentation::test_from_presentation::{
                    gen_fail_token_test, gen_ok_token_test,
                };
                use lazy_static::lazy_static;
                use num_bigint::ToBigInt;
                use ux::$ux_type;

                lazy_static! {
                    pub static ref MAX_STR: String = <$ux_type>::MAX.to_string();
                    pub static ref MIN_STR: String = <$ux_type>::MIN.to_string();
                    pub static ref TOO_BIG_STR: String =
                        (<$super_type>::from(<$ux_type>::MAX).to_bigint().unwrap()
                            + 1.to_bigint().unwrap())
                        .to_string();
                }

                gen_ok_token_test!(test_ok_1, $ux_type, <$ux_type>::new(1), &["1"]);

                gen_ok_token_test!(test_ok_max, $ux_type, <$ux_type>::MAX, &[MAX_STR.as_str()]);
                gen_ok_token_test!(test_ok_min, $ux_type, <$ux_type>::MIN, &[MIN_STR.as_str()]);

                gen_fail_token_test!(test_fail_too_large, $ux_type, &[TOO_BIG_STR.as_str()]);
                gen_fail_token_test!(test_fail_too_small, $ux_type, &["-1"]);

                gen_fail_token_test!(test_fail_not_an_integer, $ux_type, &["not_an_integer"]);
                gen_fail_token_test!(test_fail_empty_str, $ux_type, &[""]);
            }
        };
    }

    #[cfg(test)]
    mod test_u1 {
        use crate::serde::presentation::test_from_presentation::{
            gen_fail_token_test, gen_ok_token_test,
        };
        use lazy_static::lazy_static;
        use ux::u1;

        lazy_static! {
            pub static ref MAX_STR: String = u1::MAX.to_string();
            pub static ref MIN_STR: String = u1::MIN.to_string();
        }

        gen_ok_token_test!(test_ok_max, u1, u1::MAX, &[MAX_STR.as_str()]);
        gen_ok_token_test!(test_ok_min, u1, u1::MIN, &[MIN_STR.as_str()]);

        gen_fail_token_test!(test_fail_too_large, u1, &["2"]);
        gen_fail_token_test!(test_fail_too_small, u1, &["-1"]);

        gen_fail_token_test!(test_fail_not_an_integer, u1, &["not_an_integer"]);
        gen_fail_token_test!(test_fail_empty_str, u1, &[""]);
    }

    gen_unsigned_integer_test!(test_u2, u2, u8);
    gen_unsigned_integer_test!(test_u3, u3, u8);
    gen_unsigned_integer_test!(test_u4, u4, u8);
    gen_unsigned_integer_test!(test_u5, u5, u8);
    gen_unsigned_integer_test!(test_u6, u6, u8);
    gen_unsigned_integer_test!(test_u7, u7, u8);
    gen_unsigned_integer_test!(test_u9, u9, u16);
    gen_unsigned_integer_test!(test_u10, u10, u16);
    gen_unsigned_integer_test!(test_u11, u11, u16);
    gen_unsigned_integer_test!(test_u12, u12, u16);
    gen_unsigned_integer_test!(test_u13, u13, u16);
    gen_unsigned_integer_test!(test_u14, u14, u16);
    gen_unsigned_integer_test!(test_u15, u15, u16);
    gen_unsigned_integer_test!(test_u17, u17, u32);
    gen_unsigned_integer_test!(test_u18, u18, u32);
    gen_unsigned_integer_test!(test_u19, u19, u32);
    gen_unsigned_integer_test!(test_u20, u20, u32);
    gen_unsigned_integer_test!(test_u21, u21, u32);
    gen_unsigned_integer_test!(test_u22, u22, u32);
    gen_unsigned_integer_test!(test_u23, u23, u32);
    gen_unsigned_integer_test!(test_u24, u24, u32);
    gen_unsigned_integer_test!(test_u25, u25, u32);
    gen_unsigned_integer_test!(test_u26, u26, u32);
    gen_unsigned_integer_test!(test_u27, u27, u32);
    gen_unsigned_integer_test!(test_u28, u28, u32);
    gen_unsigned_integer_test!(test_u29, u29, u32);
    gen_unsigned_integer_test!(test_u30, u30, u32);
    gen_unsigned_integer_test!(test_u31, u31, u32);
    gen_unsigned_integer_test!(test_u33, u33, u64);
    gen_unsigned_integer_test!(test_u34, u34, u64);
    gen_unsigned_integer_test!(test_u35, u35, u64);
    gen_unsigned_integer_test!(test_u36, u36, u64);
    gen_unsigned_integer_test!(test_u37, u37, u64);
    gen_unsigned_integer_test!(test_u38, u38, u64);
    gen_unsigned_integer_test!(test_u39, u39, u64);
    gen_unsigned_integer_test!(test_u40, u40, u64);
    gen_unsigned_integer_test!(test_u41, u41, u64);
    gen_unsigned_integer_test!(test_u42, u42, u64);
    gen_unsigned_integer_test!(test_u43, u43, u64);
    gen_unsigned_integer_test!(test_u44, u44, u64);
    gen_unsigned_integer_test!(test_u45, u45, u64);
    gen_unsigned_integer_test!(test_u46, u46, u64);
    gen_unsigned_integer_test!(test_u47, u47, u64);
    gen_unsigned_integer_test!(test_u48, u48, u64);
    gen_unsigned_integer_test!(test_u49, u49, u64);
    gen_unsigned_integer_test!(test_u50, u50, u64);
    gen_unsigned_integer_test!(test_u51, u51, u64);
    gen_unsigned_integer_test!(test_u52, u52, u64);
    gen_unsigned_integer_test!(test_u53, u53, u64);
    gen_unsigned_integer_test!(test_u54, u54, u64);
    gen_unsigned_integer_test!(test_u55, u55, u64);
    gen_unsigned_integer_test!(test_u56, u56, u64);
    gen_unsigned_integer_test!(test_u57, u57, u64);
    gen_unsigned_integer_test!(test_u58, u58, u64);
    gen_unsigned_integer_test!(test_u59, u59, u64);
    gen_unsigned_integer_test!(test_u60, u60, u64);
    gen_unsigned_integer_test!(test_u61, u61, u64);
    gen_unsigned_integer_test!(test_u62, u62, u64);
    gen_unsigned_integer_test!(test_u63, u63, u64);
    // FIXME: There is no TryFrom<i128> implementation for `ux` types. Until then, cannot implement FromPresentation for them.
    // gen_unsigned_integer_test!(test_u65, u65, u128);
    // gen_unsigned_integer_test!(test_u66, u66, u128);
    // gen_unsigned_integer_test!(test_u67, u67, u128);
    // gen_unsigned_integer_test!(test_u68, u68, u128);
    // gen_unsigned_integer_test!(test_u69, u69, u128);
    // gen_unsigned_integer_test!(test_u70, u70, u128);
    // gen_unsigned_integer_test!(test_u71, u71, u128);
    // gen_unsigned_integer_test!(test_u72, u72, u128);
    // gen_unsigned_integer_test!(test_u73, u73, u128);
    // gen_unsigned_integer_test!(test_u74, u74, u128);
    // gen_unsigned_integer_test!(test_u75, u75, u128);
    // gen_unsigned_integer_test!(test_u76, u76, u128);
    // gen_unsigned_integer_test!(test_u77, u77, u128);
    // gen_unsigned_integer_test!(test_u78, u78, u128);
    // gen_unsigned_integer_test!(test_u79, u79, u128);
    // gen_unsigned_integer_test!(test_u80, u80, u128);
    // gen_unsigned_integer_test!(test_u81, u81, u128);
    // gen_unsigned_integer_test!(test_u82, u82, u128);
    // gen_unsigned_integer_test!(test_u83, u83, u128);
    // gen_unsigned_integer_test!(test_u84, u84, u128);
    // gen_unsigned_integer_test!(test_u85, u85, u128);
    // gen_unsigned_integer_test!(test_u86, u86, u128);
    // gen_unsigned_integer_test!(test_u87, u87, u128);
    // gen_unsigned_integer_test!(test_u88, u88, u128);
    // gen_unsigned_integer_test!(test_u89, u89, u128);
    // gen_unsigned_integer_test!(test_u90, u90, u128);
    // gen_unsigned_integer_test!(test_u91, u91, u128);
    // gen_unsigned_integer_test!(test_u92, u92, u128);
    // gen_unsigned_integer_test!(test_u93, u93, u128);
    // gen_unsigned_integer_test!(test_u94, u94, u128);
    // gen_unsigned_integer_test!(test_u95, u95, u128);
    // gen_unsigned_integer_test!(test_u96, u96, u128);
    // gen_unsigned_integer_test!(test_u97, u97, u128);
    // gen_unsigned_integer_test!(test_u98, u98, u128);
    // gen_unsigned_integer_test!(test_u99, u99, u128);
    // gen_unsigned_integer_test!(test_u100, u100, u128);
    // gen_unsigned_integer_test!(test_u101, u101, u128);
    // gen_unsigned_integer_test!(test_u102, u102, u128);
    // gen_unsigned_integer_test!(test_u103, u103, u128);
    // gen_unsigned_integer_test!(test_u104, u104, u128);
    // gen_unsigned_integer_test!(test_u105, u105, u128);
    // gen_unsigned_integer_test!(test_u106, u106, u128);
    // gen_unsigned_integer_test!(test_u107, u107, u128);
    // gen_unsigned_integer_test!(test_u108, u108, u128);
    // gen_unsigned_integer_test!(test_u109, u109, u128);
    // gen_unsigned_integer_test!(test_u110, u110, u128);
    // gen_unsigned_integer_test!(test_u111, u111, u128);
    // gen_unsigned_integer_test!(test_u112, u112, u128);
    // gen_unsigned_integer_test!(test_u113, u113, u128);
    // gen_unsigned_integer_test!(test_u114, u114, u128);
    // gen_unsigned_integer_test!(test_u115, u115, u128);
    // gen_unsigned_integer_test!(test_u116, u116, u128);
    // gen_unsigned_integer_test!(test_u117, u117, u128);
    // gen_unsigned_integer_test!(test_u118, u118, u128);
    // gen_unsigned_integer_test!(test_u119, u119, u128);
    // gen_unsigned_integer_test!(test_u120, u120, u128);
    // gen_unsigned_integer_test!(test_u121, u121, u128);
    // gen_unsigned_integer_test!(test_u122, u122, u128);
    // gen_unsigned_integer_test!(test_u123, u123, u128);
    // gen_unsigned_integer_test!(test_u124, u124, u128);
    // gen_unsigned_integer_test!(test_u125, u125, u128);
    // gen_unsigned_integer_test!(test_u126, u126, u128);
    // gen_unsigned_integer_test!(test_u127, u127, u128);

    macro_rules! gen_signed_integer_test {
        ($test_name:ident, $ux_type:ident, $super_type:ty) => {
            #[cfg(test)]
            mod $test_name {
                use crate::serde::presentation::test_from_presentation::{
                    gen_fail_token_test, gen_ok_token_test,
                };
                use lazy_static::lazy_static;
                use num_bigint::ToBigInt;
                use ux::$ux_type;

                lazy_static! {
                    pub static ref MAX_STR: String = <$ux_type>::MAX.to_string();
                    pub static ref MIN_STR: String = <$ux_type>::MIN.to_string();
                    pub static ref TOO_SMALL_STR: String =
                        (<$super_type>::from(<$ux_type>::MAX).to_bigint().unwrap()
                            + 1.to_bigint().unwrap())
                        .to_string();
                    pub static ref TOO_BIG_STR: String =
                        (<$super_type>::from(<$ux_type>::MIN).to_bigint().unwrap()
                            - 1.to_bigint().unwrap())
                        .to_string();
                }

                gen_ok_token_test!(test_ok_neg_1, $ux_type, <$ux_type>::new(-1), &["-1"]);
                gen_ok_token_test!(test_ok_0, $ux_type, <$ux_type>::new(0), &["0"]);
                gen_ok_token_test!(test_ok_1, $ux_type, <$ux_type>::new(1), &["1"]);

                gen_ok_token_test!(test_ok_max, $ux_type, <$ux_type>::MAX, &[MAX_STR.as_str()]);
                gen_ok_token_test!(test_ok_min, $ux_type, <$ux_type>::MIN, &[MIN_STR.as_str()]);

                gen_fail_token_test!(test_fail_too_large, $ux_type, &[TOO_SMALL_STR.as_str()]);
                gen_fail_token_test!(test_fail_too_small, $ux_type, &[TOO_BIG_STR.as_str()]);

                gen_fail_token_test!(test_fail_not_an_integer, $ux_type, &["not_an_integer"]);
                gen_fail_token_test!(test_fail_empty_str, $ux_type, &[""]);
            }
        };
    }

    #[cfg(test)]
    mod test_i1 {
        use crate::serde::presentation::test_from_presentation::{
            gen_fail_token_test, gen_ok_token_test,
        };
        use lazy_static::lazy_static;
        use ux::i1;

        lazy_static! {
            pub static ref MAX_STR: String = i1::MAX.to_string();
            pub static ref MIN_STR: String = i1::MIN.to_string();
        }

        gen_ok_token_test!(test_ok_max, i1, i1::MAX, &[MAX_STR.as_str()]);
        gen_ok_token_test!(test_ok_min, i1, i1::MIN, &[MIN_STR.as_str()]);

        gen_fail_token_test!(test_fail_too_large, i1, &["1"]);
        gen_fail_token_test!(test_fail_too_small, i1, &["-2"]);

        gen_fail_token_test!(test_fail_not_an_integer, i1, &["not_an_integer"]);
        gen_fail_token_test!(test_fail_empty_str, i1, &[""]);
    }

    gen_signed_integer_test!(testiu2, i2, i8);
    gen_signed_integer_test!(testiu3, i3, i8);
    gen_signed_integer_test!(testiu4, i4, i8);
    gen_signed_integer_test!(testiu5, i5, i8);
    gen_signed_integer_test!(testiu6, i6, i8);
    gen_signed_integer_test!(testiu7, i7, i8);
    gen_signed_integer_test!(testiu9, i9, i16);
    gen_signed_integer_test!(test_i10, i10, i16);
    gen_signed_integer_test!(test_i11, i11, i16);
    gen_signed_integer_test!(test_i12, i12, i16);
    gen_signed_integer_test!(test_i13, i13, i16);
    gen_signed_integer_test!(test_i14, i14, i16);
    gen_signed_integer_test!(test_i15, i15, i16);
    gen_signed_integer_test!(test_i17, i17, i32);
    gen_signed_integer_test!(test_i18, i18, i32);
    gen_signed_integer_test!(test_i19, i19, i32);
    gen_signed_integer_test!(test_i20, i20, i32);
    gen_signed_integer_test!(test_i21, i21, i32);
    gen_signed_integer_test!(test_i22, i22, i32);
    gen_signed_integer_test!(test_i23, i23, i32);
    gen_signed_integer_test!(test_i24, i24, i32);
    gen_signed_integer_test!(test_i25, i25, i32);
    gen_signed_integer_test!(test_i26, i26, i32);
    gen_signed_integer_test!(test_i27, i27, i32);
    gen_signed_integer_test!(test_i28, i28, i32);
    gen_signed_integer_test!(test_i29, i29, i32);
    gen_signed_integer_test!(test_i30, i30, i32);
    gen_signed_integer_test!(test_i31, i31, i32);
    gen_signed_integer_test!(test_i33, i33, i64);
    gen_signed_integer_test!(test_i34, i34, i64);
    gen_signed_integer_test!(test_i35, i35, i64);
    gen_signed_integer_test!(test_i36, i36, i64);
    gen_signed_integer_test!(test_i37, i37, i64);
    gen_signed_integer_test!(test_i38, i38, i64);
    gen_signed_integer_test!(test_i39, i39, i64);
    gen_signed_integer_test!(test_i40, i40, i64);
    gen_signed_integer_test!(test_i41, i41, i64);
    gen_signed_integer_test!(test_i42, i42, i64);
    gen_signed_integer_test!(test_i43, i43, i64);
    gen_signed_integer_test!(test_i44, i44, i64);
    gen_signed_integer_test!(test_i45, i45, i64);
    gen_signed_integer_test!(test_i46, i46, i64);
    gen_signed_integer_test!(test_i47, i47, i64);
    gen_signed_integer_test!(test_i48, i48, i64);
    gen_signed_integer_test!(test_i49, i49, i64);
    gen_signed_integer_test!(test_i50, i50, i64);
    gen_signed_integer_test!(test_i51, i51, i64);
    gen_signed_integer_test!(test_i52, i52, i64);
    gen_signed_integer_test!(test_i53, i53, i64);
    gen_signed_integer_test!(test_i54, i54, i64);
    gen_signed_integer_test!(test_i55, i55, i64);
    gen_signed_integer_test!(test_i56, i56, i64);
    gen_signed_integer_test!(test_i57, i57, i64);
    gen_signed_integer_test!(test_i58, i58, i64);
    gen_signed_integer_test!(test_i59, i59, i64);
    gen_signed_integer_test!(test_i60, i60, i64);
    gen_signed_integer_test!(test_i61, i61, i64);
    gen_signed_integer_test!(test_i62, i62, i64);
    gen_signed_integer_test!(test_i63, i63, i64);
    // FIXME: There is no TryFrom<i128> implementation for `ux` types. Until then, cannot implement FromPresentation for them.
    // gen_signed_integer_test!(test_i65, i65, i128);
    // gen_signed_integer_test!(test_i66, i66, i128);
    // gen_signed_integer_test!(test_i67, i67, i128);
    // gen_signed_integer_test!(test_i68, i68, i128);
    // gen_signed_integer_test!(test_i69, i69, i128);
    // gen_signed_integer_test!(test_i70, i70, i128);
    // gen_signed_integer_test!(test_i71, i71, i128);
    // gen_signed_integer_test!(test_i72, i72, i128);
    // gen_signed_integer_test!(test_i73, i73, i128);
    // gen_signed_integer_test!(test_i74, i74, i128);
    // gen_signed_integer_test!(test_i75, i75, i128);
    // gen_signed_integer_test!(test_i76, i76, i128);
    // gen_signed_integer_test!(test_i77, i77, i128);
    // gen_signed_integer_test!(test_i78, i78, i128);
    // gen_signed_integer_test!(test_i79, i79, i128);
    // gen_signed_integer_test!(test_i80, i80, i128);
    // gen_signed_integer_test!(test_i81, i81, i128);
    // gen_signed_integer_test!(test_i82, i82, i128);
    // gen_signed_integer_test!(test_i83, i83, i128);
    // gen_signed_integer_test!(test_i84, i84, i128);
    // gen_signed_integer_test!(test_i85, i85, i128);
    // gen_signed_integer_test!(test_i86, i86, i128);
    // gen_signed_integer_test!(test_i87, i87, i128);
    // gen_signed_integer_test!(test_i88, i88, i128);
    // gen_signed_integer_test!(test_i89, i89, i128);
    // gen_signed_integer_test!(test_i90, i90, i128);
    // gen_signed_integer_test!(test_i91, i91, i128);
    // gen_signed_integer_test!(test_i92, i92, i128);
    // gen_signed_integer_test!(test_i93, i93, i128);
    // gen_signed_integer_test!(test_i94, i94, i128);
    // gen_signed_integer_test!(test_i95, i95, i128);
    // gen_signed_integer_test!(test_i96, i96, i128);
    // gen_signed_integer_test!(test_i97, i97, i128);
    // gen_signed_integer_test!(test_i98, i98, i128);
    // gen_signed_integer_test!(test_i99, i99, i128);
    // gen_signed_integer_test!(test_i100, i100, i128);
    // gen_signed_integer_test!(test_i101, i101, i128);
    // gen_signed_integer_test!(test_i102, i102, i128);
    // gen_signed_integer_test!(test_i103, i103, i128);
    // gen_signed_integer_test!(test_i104, i104, i128);
    // gen_signed_integer_test!(test_i105, i105, i128);
    // gen_signed_integer_test!(test_i106, i106, i128);
    // gen_signed_integer_test!(test_i107, i107, i128);
    // gen_signed_integer_test!(test_i108, i108, i128);
    // gen_signed_integer_test!(test_i109, i109, i128);
    // gen_signed_integer_test!(test_i110, i110, i128);
    // gen_signed_integer_test!(test_i111, i111, i128);
    // gen_signed_integer_test!(test_i112, i112, i128);
    // gen_signed_integer_test!(test_i113, i113, i128);
    // gen_signed_integer_test!(test_i114, i114, i128);
    // gen_signed_integer_test!(test_i115, i115, i128);
    // gen_signed_integer_test!(test_i116, i116, i128);
    // gen_signed_integer_test!(test_i117, i117, i128);
    // gen_signed_integer_test!(test_i118, i118, i128);
    // gen_signed_integer_test!(test_i119, i119, i128);
    // gen_signed_integer_test!(test_i120, i120, i128);
    // gen_signed_integer_test!(test_i121, i121, i128);
    // gen_signed_integer_test!(test_i122, i122, i128);
    // gen_signed_integer_test!(test_i123, i123, i128);
    // gen_signed_integer_test!(test_i124, i124, i128);
    // gen_signed_integer_test!(test_i125, i125, i128);
    // gen_signed_integer_test!(test_i126, i126, i128);
    // gen_signed_integer_test!(test_i127, i127, i128);
}

// #################### OTHER COMMON TYPES ####################

macro_rules! address_from_token_impl {
    ($addr_type:ty) => {
        impl FromPresentation for $addr_type {
            #[inline]
            fn from_token_format<'a, 'b, 'c, 'd>(
                tokens: &'c [&'a str],
            ) -> Result<(Self, &'d [&'a str]), TokenError>
            where
                Self: Sized,
                'a: 'b,
                'c: 'd,
            {
                match tokens {
                    [] => Err(TokenError::OutOfTokens),
                    [token, ..] => Ok((<$addr_type>::from_str(token)?, &tokens[1..])),
                }
            }
        }
    };
}

address_from_token_impl!(Ipv4Addr);
address_from_token_impl!(Ipv6Addr);
address_from_token_impl!(MacAddress);

#[cfg(test)]
mod test_ipv4_address {
    use std::net::Ipv4Addr;

    use crate::serde::presentation::test_from_presentation::{
        gen_fail_token_test, gen_ok_token_test,
    };

    const GOOD_IPV4: &str = "192.168.86.1";
    const BAD_IPV4: &str = "192.168.86.1.3";
    const EMPTY_STR: &str = "";

    gen_ok_token_test!(
        test_ipv4_ok,
        Ipv4Addr,
        Ipv4Addr::new(192, 168, 86, 1),
        &[GOOD_IPV4]
    );
    gen_fail_token_test!(test_ipv4_fail, Ipv4Addr, &[BAD_IPV4]);
    gen_fail_token_test!(test_ipv4_fail_blank, Ipv4Addr, &[EMPTY_STR]);
}

#[cfg(test)]
mod test_ipv6_address {
    use std::net::Ipv6Addr;

    use crate::serde::presentation::test_from_presentation::{
        gen_fail_token_test, gen_ok_token_test,
    };

    const GOOD_IPV6: &str = "a:9:8:7:6:5:4:3";
    const BAD_IPV6: &str = "a:9:8:7:6:5:4:3:2:1";
    const EMPTY_STR: &str = "";

    gen_ok_token_test!(
        test_ipv6_ok,
        Ipv6Addr,
        Ipv6Addr::new(10, 9, 8, 7, 6, 5, 4, 3),
        &[GOOD_IPV6]
    );
    gen_fail_token_test!(test_ipv6_fail, Ipv6Addr, &[BAD_IPV6]);
    gen_fail_token_test!(test_ipv6_fail_blank, Ipv6Addr, &[EMPTY_STR]);
}

#[cfg(test)]
mod test_mac_address {
    use mac_address::MacAddress;

    use crate::serde::presentation::test_from_presentation::{
        gen_fail_token_test, gen_ok_token_test,
    };

    const GOOD_MAC: &str = "0a:09:08:07:06:05";
    const BAD_MAC: &str = "0a:09:08:07:06:05:04:03";
    const EMPTY_STR: &str = "";

    gen_ok_token_test!(
        test_mac_address_ok,
        MacAddress,
        MacAddress::new([10, 9, 8, 7, 6, 5]),
        &[GOOD_MAC]
    );
    gen_fail_token_test!(test_mac_address_fail, MacAddress, &[BAD_MAC]);
    gen_fail_token_test!(test_mac_address_fail_blank, MacAddress, &[EMPTY_STR]);
}

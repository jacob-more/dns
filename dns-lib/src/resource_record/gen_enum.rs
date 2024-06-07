macro_rules! enum_encoding {
    ($((doc $($doc_str:expr),+),)? $enum_name:ident, $int_ty:ty, ($(($((doc $($item_doc_str:expr),+),)? $item_name:ident, $item_code:literal)),+$(,)?)) => {
        crate::resource_record::gen_enum::gen_enum!($(($($doc_str),+),)? $enum_name, $int_ty, ($(($(($($item_doc_str),+),)? $item_name),)+));

        crate::resource_record::gen_enum::impl_enum_code!($enum_name, $int_ty, ($(($item_name, $item_code),)+));
        crate::resource_record::gen_enum::impl_enum_from_code!($enum_name, $int_ty, ($(($item_name, $item_code),)+));
        crate::resource_record::gen_enum::impl_enum_display!($enum_name, $int_ty, display_code);

        crate::resource_record::gen_enum::impl_enum_to_wire!($enum_name, $int_ty);
        crate::resource_record::gen_enum::impl_enum_from_wire!($enum_name, $int_ty);
        crate::resource_record::gen_enum::impl_enum_to_presentation!($enum_name, $int_ty, code_to_presentation);
        crate::resource_record::gen_enum::impl_enum_from_presentation!($enum_name, $int_ty, code_from_presentation);
    };
    ($((doc $($doc_str:expr),+),)? $enum_name:ident, $int_ty:ty, ($(($((doc $($item_doc_str:expr),+),)? $item_name:ident, $item_mnemonic:literal, $item_code:literal)),+$(,)?), $from_presentation:ident, $to_presentation:ident, $display:ident) => {
        crate::resource_record::gen_enum::gen_enum!($(($($doc_str),+),)? $enum_name, $int_ty, ($(($(($($item_doc_str),+),)? $item_name),)+));

        crate::resource_record::gen_enum::impl_enum_code!($enum_name, $int_ty, ($(($item_name, $item_code),)+));
        crate::resource_record::gen_enum::impl_enum_from_code!($enum_name, $int_ty, ($(($item_name, $item_code),)+));
        crate::resource_record::gen_enum::impl_enum_mnemonic!($enum_name, $int_ty, ($(($item_name, $item_mnemonic),)+));
        crate::resource_record::gen_enum::impl_enum_display!($enum_name, $int_ty, $display);

        crate::resource_record::gen_enum::impl_enum_to_wire!($enum_name, $int_ty);
        crate::resource_record::gen_enum::impl_enum_from_wire!($enum_name, $int_ty);
        crate::resource_record::gen_enum::impl_enum_to_presentation!($enum_name, $int_ty, $to_presentation);
        crate::resource_record::gen_enum::impl_enum_from_presentation!($enum_name, $int_ty, $from_presentation);
    };
    ($((doc $($doc_str:expr),+),)? $enum_name:ident, $int_ty:ty, ($(($((doc $($item_doc_str:expr),+),)? $item_name:ident, $item_mnemonic:literal, $item_code:literal)),+$(,)?), from_str, $error_ty:ty, $from_presentation:ident, $to_presentation:ident, $display:ident) => {
        crate::resource_record::gen_enum::gen_enum!($(($($doc_str),+),)? $enum_name, $int_ty, ($(($(($($item_doc_str),+),)? $item_name),)+));

        crate::resource_record::gen_enum::impl_enum_code!($enum_name, $int_ty, ($(($item_name, $item_code),)+));
        crate::resource_record::gen_enum::impl_enum_from_code!($enum_name, $int_ty, ($(($item_name, $item_code),)+));
        crate::resource_record::gen_enum::impl_enum_mnemonic!($enum_name, $int_ty, ($(($item_name, $item_mnemonic),)+));
        crate::resource_record::gen_enum::impl_enum_from_str!($enum_name, $int_ty, $error_ty, ($(($item_name, $item_mnemonic),)+));
        crate::resource_record::gen_enum::impl_enum_display!($enum_name, $int_ty, $display);

        crate::resource_record::gen_enum::impl_enum_to_wire!($enum_name, $int_ty);
        crate::resource_record::gen_enum::impl_enum_from_wire!($enum_name, $int_ty);
        crate::resource_record::gen_enum::impl_enum_to_presentation!($enum_name, $int_ty, $to_presentation);
        crate::resource_record::gen_enum::impl_enum_from_presentation!($enum_name, $int_ty, $from_presentation);
    };
}

macro_rules! gen_enum {
    ($(($($doc_str:expr),+),)? $enum_name:ident, $int_ty:ty, ($(($(($($item_doc_str:expr),+),)? $item_name:ident)),+$(,)?)) => {
        $($(#[doc = $doc_str])*)?
        #[allow(non_camel_case_types)]
        #[derive(Clone, PartialEq, Eq, Hash, Debug)]
        pub enum $enum_name {
            Unknown($int_ty),
            $(
                $($(#[doc = $item_doc_str])*)?
                $item_name,
            )+
        }
    };
}

macro_rules! impl_enum_code {
    ($enum_name:ident, $int_ty:ty, ($(($item_name:ident, $item_code:literal)),+$(,)?)) => {
        impl $enum_name {
            pub const MIN: $int_ty = <$int_ty>::MIN;
            pub const MAX: $int_ty = <$int_ty>::MAX;

            #[inline]
            pub const fn code(&self) -> $int_ty {
                match self {
                    Self::Unknown(x) => *x,
                    $(Self::$item_name => $item_code,)+
                }
            }
        }
    };
}

macro_rules! impl_enum_from_code {
    ($enum_name:ident, $int_ty:ty, ($(($item_name:ident, $item_code:literal)),+$(,)?)) => {
        impl $enum_name {
            #[inline]
            pub const fn from_code(value: $int_ty) -> Self {
                match value {
                    $($item_code => Self::$item_name,)+
                    _ => Self::Unknown(value),
                }
            }
        }
    };
}

macro_rules! impl_enum_mnemonic {
    ($enum_name:ident, $int_ty:ty, ($(($item_name:ident, $item_mnemonic:literal)),+$(,)?)) => {
        impl $enum_name {
            #[inline]
            pub fn mnemonic(&self) -> String {
                match self {
                    Self::Unknown(code) => code.to_string(),
                    $(Self::$item_name => $item_mnemonic.to_string(),)+
                }
            }
        }
    };
}

macro_rules! impl_enum_display {
    ($enum_name:ident, $int_ty:ty, display_mnemonic) => {
        impl std::fmt::Display for $enum_name {
            #[inline]
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.mnemonic())
            }
        }
    };
    ($enum_name:ident, $int_ty:ty, display_code) => {
        impl std::fmt::Display for $enum_name {
            #[inline]
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.code())
            }
        }
    };
}

macro_rules! impl_enum_from_str {
    ($enum_name:ident, $int_ty:ty, $error_ty:ty, ($(($item_name:ident, $item_mnemonic:literal)),+$(,)?)) => {
        impl $enum_name {
            #[inline]
            pub fn from_str(string: &str) -> Result<Self, $error_ty> {
                match string {
                    $($item_mnemonic => Ok(Self::$item_name),)+
                    _ => {
                        let protocol = match <$int_ty>::from_str_radix(string, 10) {
                            Ok(protocol) => protocol,
                            Err(_) => return Err(<$error_ty>::UnknownMnemonic(string)),
                        };
                        // Note: we don't directly assign it to Unknown since it could be a known
                        //       code that just uses the '(\d)+' syntax.
                        Ok(Self::from_code(protocol))
                    },
                }
            }
        }
    };
}

macro_rules! impl_enum_to_wire {
    ($enum_name:ident, $int_ty:ty) => {
        impl crate::serde::wire::to_wire::ToWire for $enum_name {
            #[inline]
            fn to_wire_format<'a, 'b>(&self, wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>, compression: &mut Option<crate::serde::wire::compression_map::CompressionMap>) -> Result<(), crate::serde::wire::write_wire::WriteWireError> where 'a: 'b {
                self.code().to_wire_format(wire, compression)
            }
        
            #[inline]
            fn serial_length(&self) -> u16 {
                self.code().serial_length()
            }
        }
    };
}

macro_rules! impl_enum_from_wire {
    ($enum_name:ident, $int_ty:ty) => {
        impl crate::serde::wire::from_wire::FromWire for $enum_name {
            #[inline]
            fn from_wire_format<'a, 'b>(wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>) -> Result<Self, crate::serde::wire::read_wire::ReadWireError> where Self: Sized, 'a: 'b {
                Ok(Self::from_code(
                    <$int_ty>::from_wire_format(wire)?
                ))
            }
        }
    };
}

macro_rules! impl_enum_to_presentation {
    ($enum_name:ident, $int_ty:ty, code_to_presentation) => {
        impl crate::serde::presentation::to_presentation::ToPresentation for $enum_name {
            #[inline]
            fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
                out_buffer.push(self.code().to_string())
            }
        }
    };
    ($enum_name:ident, $int_ty:ty, mnemonic_to_presentation) => {
        impl crate::serde::presentation::to_presentation::ToPresentation for $enum_name {
            #[inline]
            fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
                out_buffer.push(self.mnemonic())
            }
        }
    };
}

macro_rules! impl_enum_from_presentation {
    ($enum_name:ident, $int_ty:ty, code_from_presentation) => {
        impl crate::serde::presentation::from_presentation::FromPresentation for $enum_name {
            #[inline]
            fn from_token_format<'a, 'b, 'c, 'd>(tokens: &'c [&'a str]) -> Result<(Self, &'d [&'a str]), crate::serde::presentation::errors::TokenError<'b>> where Self: Sized, 'a: 'b, 'c: 'd, 'c: 'd {
                let (code, tokens) = <$int_ty>::from_token_format(tokens)?;
                Ok((Self::from_code(code), tokens))
            }
        }
    };
    ($enum_name:ident, $int_ty:ty, mnemonic_from_presentation) => {
        impl crate::serde::presentation::from_presentation::FromPresentation for $enum_name {
            #[inline]
            fn from_token_format<'a, 'b, 'c, 'd>(tokens: &'c [&'a str]) -> Result<(Self, &'d [&'a str]), crate::serde::presentation::errors::TokenError<'b>> where Self: Sized, 'a: 'b, 'c: 'd, 'c: 'd {
                match tokens {
                    &[] => Err(crate::serde::presentation::errors::TokenError::OutOfTokens),
                    &[token, ..] => Ok((Self::from_str(token)?, &tokens[1..])),
                }
            }
        }
    };
}

pub(crate) use enum_encoding;
pub(crate) use gen_enum;
pub(crate) use impl_enum_code;
pub(crate) use impl_enum_from_code;
pub(crate) use impl_enum_mnemonic;
pub(crate) use impl_enum_display;
pub(crate) use impl_enum_from_str;
pub(crate) use impl_enum_to_wire;
pub(crate) use impl_enum_from_wire;
pub(crate) use impl_enum_to_presentation;
pub(crate) use impl_enum_from_presentation;

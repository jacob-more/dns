macro_rules! enum_encoding {
    // Generates a enum with only Name-Code matchings. No mnemonics. No from_str.
    ($((doc $($doc_str:expr),+),)? $enum_name:ident, $int_ty:ty, ($(($((doc $($item_doc_str:expr),+),)? $item_name:ident, $item_code:literal)),+$(,)?)) => {
        $crate::gen_enum::gen_enum!($(($($doc_str),+),)? $enum_name, $int_ty, ($(($(($($item_doc_str),+),)? $item_name),)+));

        $crate::gen_enum::impl_enum_code!($enum_name, $int_ty, ($(($item_name, $item_code),)+));
        $crate::gen_enum::impl_enum_from_code!($enum_name, $int_ty, ($(($item_name, $item_code),)+));
        $crate::gen_enum::impl_enum_display!($enum_name, $int_ty, code_display);
        $crate::gen_enum::impl_enum_cmp!($enum_name, $int_ty);
        $crate::gen_enum::impl_enum_hash!($enum_name, $int_ty);

        $crate::gen_enum::impl_enum_to_wire!($enum_name, $int_ty);
        $crate::gen_enum::impl_enum_from_wire!($enum_name, $int_ty);
        $crate::gen_enum::impl_enum_to_presentation!($enum_name, $int_ty, code_presentation);
        $crate::gen_enum::impl_enum_from_presentation!($enum_name, $int_ty, code_presentation);
    };
    // Generates a enum with Name-Code & Name-Mnemonic matchings. No from_str.
    ($((doc $($doc_str:expr),+),)? $enum_name:ident, $int_ty:ty, ($(($((doc $($item_doc_str:expr),+),)? $item_name:ident, $item_mnemonic:literal, $item_code:literal)),+$(,)?), $presentation:ident, $display:ident) => {
        $crate::gen_enum::gen_enum!($(($($doc_str),+),)? $enum_name, $int_ty, ($(($(($($item_doc_str),+),)? $item_name),)+));

        $crate::gen_enum::impl_enum_code!($enum_name, $int_ty, ($(($item_name, $item_code),)+));
        $crate::gen_enum::impl_enum_from_code!($enum_name, $int_ty, ($(($item_name, $item_code),)+));
        $crate::gen_enum::impl_enum_mnemonic!($enum_name, $int_ty, ($(($item_name, $item_mnemonic),)+));
        $crate::gen_enum::impl_enum_display!($enum_name, $int_ty, $display);
        $crate::gen_enum::impl_enum_cmp!($enum_name, $int_ty);
        $crate::gen_enum::impl_enum_hash!($enum_name, $int_ty);

        $crate::gen_enum::impl_enum_to_wire!($enum_name, $int_ty);
        $crate::gen_enum::impl_enum_from_wire!($enum_name, $int_ty);
        $crate::gen_enum::impl_enum_to_presentation!($enum_name, $int_ty, $presentation);
        $crate::gen_enum::impl_enum_from_presentation!($enum_name, $int_ty, $presentation);
    };
    // Generates a enum with Name-Code & Name-Mnemonic matchings.
    // The from_str can translate using the rules for just Mnemonic->Name or both Mnemonic->Name and Code->Name.
    ($((doc $($doc_str:expr),+),)? $enum_name:ident, $int_ty:ty, $error_ty:ty, ($(($((doc $($item_doc_str:expr),+),)? $item_name:ident, $item_mnemonic:literal, $item_code:literal)),+$(,)?), $from_str:ident, $presentation:ident, $display:ident) => {
        $crate::gen_enum::gen_enum!($(($($doc_str),+),)? $enum_name, $int_ty, ($(($(($($item_doc_str),+),)? $item_name),)+));

        $crate::gen_enum::impl_enum_code!($enum_name, $int_ty, ($(($item_name, $item_code),)+));
        $crate::gen_enum::impl_enum_from_code!($enum_name, $int_ty, ($(($item_name, $item_code),)+));
        $crate::gen_enum::impl_enum_mnemonic!($enum_name, $int_ty, ($(($item_name, $item_mnemonic),)+));
        $crate::gen_enum::impl_enum_from_str!($enum_name, $int_ty, $error_ty, ($(($item_name, $item_mnemonic),)+), $from_str);
        $crate::gen_enum::impl_enum_display!($enum_name, $int_ty, $display);
        $crate::gen_enum::impl_enum_cmp!($enum_name, $int_ty);
        $crate::gen_enum::impl_enum_hash!($enum_name, $int_ty);

        $crate::gen_enum::impl_enum_to_wire!($enum_name, $int_ty);
        $crate::gen_enum::impl_enum_from_wire!($enum_name, $int_ty);
        $crate::gen_enum::impl_enum_to_presentation!($enum_name, $int_ty, $presentation);
        $crate::gen_enum::impl_enum_from_presentation!($enum_name, $int_ty, $presentation);
    };
    // Generates a enum with Name-Code & Name-Mnemonic matchings.
    // The from_str can translate {Wildcard}Code->Name or Mnemonic->Name.
    ($((doc $($doc_str:expr),+),)? $enum_name:ident, $int_ty:ty, $error_ty:ty, ($(($((doc $($item_doc_str:expr),+),)? $item_name:ident, $item_mnemonic:literal, $item_code:literal)),+$(,)?), (wildcard_or_mnemonic_from_str, $wildcard:literal), $presentation:ident, $display:ident) => {
        $crate::gen_enum::gen_enum!($(($($doc_str),+),)? $enum_name, $int_ty, ($(($(($($item_doc_str),+),)? $item_name),)+));

        $crate::gen_enum::impl_enum_code!($enum_name, $int_ty, ($(($item_name, $item_code),)+));
        $crate::gen_enum::impl_enum_from_code!($enum_name, $int_ty, ($(($item_name, $item_code),)+));
        $crate::gen_enum::impl_enum_mnemonic!($enum_name, $int_ty, ($(($item_name, $item_mnemonic),)+), $wildcard);
        $crate::gen_enum::impl_enum_from_str!($enum_name, $int_ty, $error_ty, ($(($item_name, $item_mnemonic),)+), (wildcard_or_mnemonic_from_str, $wildcard));
        $crate::gen_enum::impl_enum_display!($enum_name, $int_ty, $display);
        $crate::gen_enum::impl_enum_cmp!($enum_name, $int_ty);
        $crate::gen_enum::impl_enum_hash!($enum_name, $int_ty);

        $crate::gen_enum::impl_enum_to_wire!($enum_name, $int_ty);
        $crate::gen_enum::impl_enum_from_wire!($enum_name, $int_ty);
        $crate::gen_enum::impl_enum_to_presentation!($enum_name, $int_ty, $presentation);
        $crate::gen_enum::impl_enum_from_presentation!($enum_name, $int_ty, $presentation);
    };
}

macro_rules! gen_enum {
    ($(($($doc_str:expr),+),)? $enum_name:ident, $int_ty:ty, ($(($(($($item_doc_str:expr),+),)? $item_name:ident)),+$(,)?)) => {
        $($(#[doc = $doc_str])*)?
        #[allow(non_camel_case_types)]
        #[derive(Clone, Copy, Debug)]
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

        impl std::convert::Into<$int_ty> for $enum_name {
            #[inline]
            fn into(self) -> $int_ty {
                match self {
                    Self::Unknown(x) => x,
                    $(Self::$item_name => $item_code,)+
                }
            }
        }
    };
}

macro_rules! impl_enum_from_code {
    ($enum_name:ident, $int_ty:ty, ($(($item_name:ident, $item_code:literal)),+$(,)?)) => {
        impl $enum_name {
            // There may be multiple names assigned the same code. Only one code will ever be
            // selected through this function, unfortunately.
            #[allow(unreachable_patterns)]
            #[inline]
            pub const fn from_code(value: $int_ty) -> Self {
                match value {
                    $($item_code => Self::$item_name,)+
                    _ => Self::Unknown(value),
                }
            }
        }

        impl std::convert::From<$int_ty> for $enum_name {
            #[inline]
            fn from(value: $int_ty) -> Self {
                Self::from_code(value)
            }
        }

        impl std::convert::From<&$int_ty> for $enum_name {
            #[inline]
            fn from(value: &$int_ty) -> Self {
                Self::from_code(*value)
            }
        }
    };
}

macro_rules! impl_enum_mnemonic {
    ($enum_name:ident, $int_ty:ty, ($(($item_name:ident, $item_mnemonic:literal)),+$(,)?)) => {
        impl $enum_name {
            #[inline]
            pub fn mnemonic(&self) -> std::string::String {
                match self {
                    Self::Unknown(code) => code.to_string(),
                    $(Self::$item_name => $item_mnemonic.to_string(),)+
                }
            }
        }

        $crate::gen_enum::impl_enum_mnemonic_into_string!($enum_name, $int_ty);
    };
    ($enum_name:ident, $int_ty:ty, ($(($item_name:ident, $item_mnemonic:literal)),+$(,)?), $wildcard:literal) => {
        impl $enum_name {
            #[inline]
            pub fn mnemonic(&self) -> std::string::String {
                match self {
                    Self::Unknown(code) => std::format!("{}{code}", $wildcard),
                    $(Self::$item_name => $item_mnemonic.to_string(),)+
                }
            }
        }

        $crate::gen_enum::impl_enum_mnemonic_into_string!($enum_name, $int_ty);
    };
}

macro_rules! impl_enum_mnemonic_into_string {
    ($enum_name:ident, $int_ty:ty$(,)?) => {
        impl std::convert::Into<std::string::String> for $enum_name {
            #[inline]
            fn into(self) -> std::string::String {
                self.mnemonic()
            }
        }
    };
}

macro_rules! impl_enum_display {
    ($enum_name:ident, $int_ty:ty, mnemonic_display) => {
        impl std::fmt::Display for $enum_name {
            #[inline]
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.mnemonic())
            }
        }
    };
    ($enum_name:ident, $int_ty:ty, code_display) => {
        impl std::fmt::Display for $enum_name {
            #[inline]
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.code())
            }
        }
    };
}

macro_rules! impl_enum_from_str {
    ($enum_name:ident, $int_ty:ty, $error_ty:ty, ($(($item_name:ident, $item_mnemonic:literal)),+$(,)?), mnemonic_from_str) => {
        impl $enum_name {
            #[inline]
            pub fn from_str(string: &str) -> std::result::Result<Self, $error_ty> {
                match string {
                    $($item_mnemonic => std::result::Result::Ok(Self::$item_name),)+
                    _ => std::result::Result::Err(<$error_ty>::UnknownMnemonic(string.to_string())),
                }
            }
        }

        $crate::gen_enum::impl_enum_from_string!($enum_name, $int_ty, $error_ty);
    };
    ($enum_name:ident, $int_ty:ty, $error_ty:ty, ($(($item_name:ident, $item_mnemonic:literal)),+$(,)?), code_or_mnemonic_from_str) => {
        impl $enum_name {
            #[inline]
            pub fn from_str(string: &str) -> std::result::Result<Self, $error_ty> {
                match string {
                    $($item_mnemonic => std::result::Result::Ok(Self::$item_name),)+
                    _ => {
                        let protocol = match <$int_ty>::from_str_radix(string, 10) {
                            std::result::Result::Ok(protocol) => protocol,
                            std::result::Result::Err(_) => return std::result::Result::Err(<$error_ty>::UnknownMnemonic(string.to_string())),
                        };
                        // Note: we don't directly assign it to Unknown since it could be a known
                        //       code that just uses the '(\d)+' syntax.
                        std::result::Result::Ok(Self::from_code(protocol))
                    },
                }
            }
        }

        $crate::gen_enum::impl_enum_from_string!($enum_name, $int_ty, $error_ty);
    };
    ($enum_name:ident, $int_ty:ty, $error_ty:ty, ($(($item_name:ident, $item_mnemonic:literal)),+$(,)?), (wildcard_or_mnemonic_from_str, $wildcard:literal)) => {
        impl $enum_name {
            #[inline]
            pub fn from_str(string: &str) -> std::result::Result<Self, $error_ty> {
                match string {
                    $($item_mnemonic => std::result::Result::Ok(Self::$item_name),)+
                    _ => {
                        const WILDCARD: &str = $wildcard;
                        if !string.starts_with(WILDCARD) {
                            return std::result::Result::Err(<$error_ty>::UnknownMnemonic(string.to_string()));
                        }
                        let code_str = match u16::from_str_radix(&string[WILDCARD.len()..], 10) {
                            std::result::Result::Ok(code_str) => code_str,
                            std::result::Result::Err(_) => return std::result::Result::Err(<$error_ty>::UnknownMnemonic(string.to_string())),
                        };
                        // Note: we don't directly assign it to Unknown since it could be a known
                        //       code that just uses the 'WILDCARD(\d)+' syntax.
                        std::result::Result::Ok(Self::from_code(code_str))
                    },
                }
            }
        }

        $crate::gen_enum::impl_enum_from_string!($enum_name, $int_ty, $error_ty);
    };
}

macro_rules! impl_enum_from_string {
    ($enum_name:ident, $int_ty:ty, $error_ty:ty$(,)?) => {
        impl std::convert::TryFrom<&str> for $enum_name {
            type Error = $error_ty;

            #[inline]
            fn try_from(value: &str) -> std::result::Result<Self, $error_ty> {
                Self::from_str(value)
            }
        }
    };
}

macro_rules! impl_enum_cmp {
    ($enum_name:ident, $int_ty:ty) => {
        impl std::cmp::PartialEq for $enum_name {
            #[inline]
            fn eq(&self, other: &Self) -> bool {
                self.code() == other.code()
            }
        }

        impl std::cmp::Eq for $enum_name {}
    };
}

macro_rules! impl_enum_hash {
    ($enum_name:ident, $int_ty:ty) => {
        impl std::hash::Hash for $enum_name {
            #[inline]
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.code().hash(state);
            }
        }
    };
}

macro_rules! impl_enum_to_wire {
    ($enum_name:ident, $int_ty:ty) => {
        impl $crate::serde::wire::to_wire::ToWire for $enum_name {
            #[inline]
            fn to_wire_format<'a, 'b>(
                &self,
                wire: &'b mut $crate::serde::wire::write_wire::WriteWire<'a>,
                compression: &mut Option<$crate::types::c_domain_name::CompressionMap>,
            ) -> std::result::Result<(), $crate::serde::wire::write_wire::WriteWireError>
            where
                'a: 'b,
            {
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
        impl $crate::serde::wire::from_wire::FromWire for $enum_name {
            #[inline]
            fn from_wire_format<'a, 'b>(
                wire: &'b mut $crate::serde::wire::read_wire::ReadWire<'a>,
            ) -> std::result::Result<Self, $crate::serde::wire::read_wire::ReadWireError>
            where
                Self: Sized,
                'a: 'b,
            {
                std::result::Result::Ok(Self::from_code(<$int_ty>::from_wire_format(wire)?))
            }
        }
    };
}

macro_rules! impl_enum_to_presentation {
    ($enum_name:ident, $int_ty:ty, code_presentation) => {
        impl $crate::serde::presentation::to_presentation::ToPresentation for $enum_name {
            #[inline]
            fn to_presentation_format(&self, out_buffer: &mut std::vec::Vec<std::string::String>) {
                out_buffer.push(self.code().to_string())
            }
        }
    };
    ($enum_name:ident, $int_ty:ty, mnemonic_presentation) => {
        impl $crate::serde::presentation::to_presentation::ToPresentation for $enum_name {
            #[inline]
            fn to_presentation_format(&self, out_buffer: &mut std::vec::Vec<std::string::String>) {
                out_buffer.push(self.mnemonic())
            }
        }
    };
}

macro_rules! impl_enum_from_presentation {
    ($enum_name:ident, $int_ty:ty, code_presentation) => {
        impl $crate::serde::presentation::from_presentation::FromPresentation for $enum_name {
            #[inline]
            fn from_token_format<'a, 'b, 'c, 'd>(
                tokens: &'c [&'a str],
            ) -> std::result::Result<
                (Self, &'d [&'a str]),
                $crate::serde::presentation::errors::TokenError,
            >
            where
                Self: Sized,
                'a: 'b,
                'c: 'd,
                'c: 'd,
            {
                let (code, tokens) = <$int_ty>::from_token_format(tokens)?;
                std::result::Result::Ok((Self::from_code(code), tokens))
            }
        }
    };
    ($enum_name:ident, $int_ty:ty, mnemonic_presentation) => {
        impl $crate::serde::presentation::from_presentation::FromPresentation for $enum_name {
            #[inline]
            fn from_token_format<'a, 'b, 'c, 'd>(
                tokens: &'c [&'a str],
            ) -> std::result::Result<
                (Self, &'d [&'a str]),
                $crate::serde::presentation::errors::TokenError,
            >
            where
                Self: Sized,
                'a: 'b,
                'c: 'd,
                'c: 'd,
            {
                match tokens {
                    &[] => std::result::Result::Err(
                        $crate::serde::presentation::errors::TokenError::OutOfTokens,
                    ),
                    &[token, ..] => std::result::Result::Ok((Self::from_str(token)?, &tokens[1..])),
                }
            }
        }
    };
}

pub(crate) use enum_encoding;
pub(crate) use gen_enum;
pub(crate) use impl_enum_cmp;
pub(crate) use impl_enum_code;
pub(crate) use impl_enum_display;
pub(crate) use impl_enum_from_code;
pub(crate) use impl_enum_from_presentation;
pub(crate) use impl_enum_from_str;
pub(crate) use impl_enum_from_string;
pub(crate) use impl_enum_from_wire;
pub(crate) use impl_enum_hash;
pub(crate) use impl_enum_mnemonic;
pub(crate) use impl_enum_mnemonic_into_string;
pub(crate) use impl_enum_to_presentation;
pub(crate) use impl_enum_to_wire;

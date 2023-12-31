use std::{error::Error, fmt::Display};

use crate::serde::{wire::{to_wire::ToWire, from_wire::FromWire}, presentation::from_presentation::FromPresentation};

#[derive(Debug)]
pub enum RClassError<'a> {
    UnknownMnemonic(&'a str),
}
impl<'a> Error for RClassError<'a> {}
impl<'a> Display for RClassError<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownMnemonic(mnemonic) => write!(f, "unknown class mnemonic '{mnemonic}'"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum RClass {
    Unknown(u16),

    Internet,
    CSNet,
    Chaos,
    Hesiod,

    QClassNone,
    QClassAny, // QCLASS *
}

impl RClass {
    pub const MIN: u16 = u16::MIN;
    pub const MAX: u16 = u16::MAX;

    #[inline]
    pub const fn code(&self) -> u16 {
        return match self {
            Self::Unknown(x) => *x,

            Self::Internet => 1,
            Self::CSNet =>    2,
            Self::Chaos =>    3,
            Self::Hesiod =>   4,

            Self::QClassNone =>  254,
            Self::QClassAny =>   255,
        };
    }

    #[inline]
    pub fn mnemonic(&self) -> String {
        return match self {
            Self::Unknown(code) => format!("CLASS{}", code),

            Self::Internet => "IN".to_string(),
            Self::CSNet =>    "CS".to_string(),
            Self::Chaos =>    "CH".to_string(),
            Self::Hesiod =>   "HS".to_string(),

            Self::QClassNone =>  "NONE".to_string(),
            Self::QClassAny =>   "ANY".to_string(),
        };
    }

    #[inline]
    pub const fn from_code(value: u16) -> Self {
        return match value {
            1 =>   Self::Internet,
            2 =>   Self::CSNet,
            3 =>   Self::Chaos,
            4 =>   Self::Hesiod,

            254 => Self::QClassNone,
            255 => Self::QClassAny,

            _ =>   Self::Unknown(value),
        };
    }

    #[inline]
    pub fn from_str(rclass: &str) -> Result<RClass, RClassError> {
        match rclass {
            "IN" => Ok(RClass::Internet),
            "CS" => Ok(RClass::CSNet),
            "CH" => Ok(RClass::Chaos),
            "HS" => Ok(RClass::Hesiod),

            "NONE" =>   Ok(RClass::QClassNone),
            "ANY" =>    Ok(RClass::QClassAny),

            _ => {
                const WILDCARD: &str = "CLASS";
                if !rclass.starts_with(WILDCARD) {
                    return Err(RClassError::UnknownMnemonic(rclass));
                }
                let rclass = match u16::from_str_radix(&rclass[WILDCARD.len()..], 10) {
                    Ok(rclass) => rclass,
                    Err(_) => return Err(RClassError::UnknownMnemonic(rclass)),
                };
                // Note: we don't directly assign it to Unknown since it could be a known code that
                //       just uses the 'CLASS(\d)+' syntax.
                Ok(Self::from_code(rclass))
            },
        }
    }
}

impl Display for RClass {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.mnemonic())
    }
}

pub trait RClassCode {
    fn rclass(&self) -> RClass;
}

impl ToWire for RClass {
    #[inline]
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>, compression: &mut Option<crate::serde::wire::compression_map::CompressionMap>) -> Result<(), crate::serde::wire::write_wire::WriteWireError> where 'a: 'b {
        self.code().to_wire_format(wire, compression)
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        self.code().serial_length()
    }
}

impl FromWire for RClass {
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>) -> Result<Self, crate::serde::wire::read_wire::ReadWireError> where Self: Sized, 'a: 'b {
        Ok(Self::from_code(
            u16::from_wire_format(wire)?
        ))
    }
}

impl FromPresentation for RClass {
    #[inline]
    fn from_token_format<'a, 'b>(token: &'a str) -> Result<Self, crate::serde::presentation::errors::TokenError<'b>> where Self: Sized, 'a: 'b {
        Ok(Self::from_str(token)?)
    }
}

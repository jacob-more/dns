use std::fmt::Display;

use crate::serde::{wire::{to_wire::ToWire, from_wire::FromWire}, presentation::from_presentation::FromPresentation};

/// https://www.iana.org/assignments/dns-parameters/dns-parameters.xhtml#dns-parameters-6
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum RCode {
    Unknown(u16),

    NoError,
    FormErr,
    ServFail,
    NXDomain,
    NotImp,
    Refused,
    YXDomain,
    YXRRSet,
    NXRRSet,
    NotAuth,
    NotZone,
    DsoTypeNI,

    BadVers,
    BadSig,
    BadKey,
    BadTime,
    BadMode,
    BadName,
    BadAlg,
    BadTrunc,
    BadCookie,
}

impl RCode {
    pub const MIN: u16 = u16::MIN;
    pub const MAX: u16 = u16::MAX;

    #[inline]
    pub const fn code(&self) -> u16 {
        return match self {
            Self::Unknown(x) => *x,

            Self::NoError   => 0,
            Self::FormErr   => 1,
            Self::ServFail  => 2,
            Self::NXDomain  => 3,
            Self::NotImp    => 4,
            Self::Refused   => 5,
            Self::YXDomain  => 6,
            Self::YXRRSet   => 7,
            Self::NXRRSet   => 8,
            Self::NotAuth   => 9,
            Self::NotZone   => 10,
            Self::DsoTypeNI => 11,

            Self::BadVers   => 16,
            Self::BadSig    => 16,
            Self::BadKey    => 17,
            Self::BadTime   => 18,
            Self::BadMode   => 19,
            Self::BadName   => 20,
            Self::BadAlg    => 21,
            Self::BadTrunc  => 22,
            Self::BadCookie => 23,
        };
    }

    #[inline]
    pub const fn mnemonic(&self) -> &str {
        return match self {
            Self::Unknown(_) => "Unknown",

            Self::NoError   => "NoError",
            Self::FormErr   => "FormErr",
            Self::ServFail  => "ServFail",
            Self::NXDomain  => "NXDomain",
            Self::NotImp    => "NotImp",
            Self::Refused   => "Refused",
            Self::YXDomain  => "YXDomain",
            Self::YXRRSet   => "YXRRSet",
            Self::NXRRSet   => "NXRRSet",
            Self::NotAuth   => "NotAuth",
            Self::NotZone   => "NotZone",
            Self::DsoTypeNI => "DSOTYPENI",

            Self::BadVers   => "BADVERS",
            Self::BadSig    => "BADSIG",
            Self::BadKey    => "BADKEY",
            Self::BadTime   => "BADTIME",
            Self::BadMode   => "BADMODE",
            Self::BadName   => "BADNAME",
            Self::BadAlg    => "BADALG",
            Self::BadTrunc  => "BADTRUNC",
            Self::BadCookie => "BADCOOKIE",
        };
    }

    #[inline]
    pub const fn from_code(value: u16) -> Self {
        return match value {
            0  => Self::NoError,
            1  => Self::FormErr,
            2  => Self::ServFail,
            3  => Self::NXDomain,
            4  => Self::NotImp,
            5  => Self::Refused,
            6  => Self::YXDomain,
            7  => Self::YXRRSet,
            8  => Self::NXRRSet,
            9  => Self::NotAuth,
            10 => Self::NotZone,
            11 => Self::DsoTypeNI,

            16 => Self::BadVers,
            17 => Self::BadKey,
            18 => Self::BadTime,
            19 => Self::BadMode,
            20 => Self::BadName,
            21 => Self::BadAlg,
            22 => Self::BadTrunc,
            23 => Self::BadCookie,

            _ => Self::Unknown(value),
        };
    }
}

impl Display for RCode {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.mnemonic())
    }
}

impl ToWire for RCode {
    #[inline]
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>, compression: &mut Option<crate::serde::wire::compression_map::CompressionMap>) -> Result<(), crate::serde::wire::write_wire::WriteWireError> where 'a: 'b {
        self.code().to_wire_format(wire, compression)
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        self.code().serial_length()
    }
}

impl FromWire for RCode {
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>) -> Result<Self, crate::serde::wire::read_wire::ReadWireError> where Self: Sized, 'a: 'b {
        Ok(Self::from_code(
            u16::from_wire_format(wire)?
        ))
    }
}

impl FromPresentation for RCode {
    #[inline]
    fn from_token_format<'a, 'b>(token: &'a str) -> Result<Self, crate::serde::presentation::errors::TokenError<'b>> where Self: Sized, 'a: 'b {
        Ok(Self::from_code(
            u16::from_token_format(token)?
        ))
    }
}

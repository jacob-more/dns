use std::fmt::Display;

use ux::u4;

/// https://www.iana.org/assignments/dns-parameters/dns-parameters.xhtml#dns-parameters-5
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum OpCode {
    Unknown(u4),

    Query,
    IQuery,
    Status,

    Notify,
    Update,
    DNSStatefulOperations,
}

impl OpCode {
    pub const MIN: u4 = u4::MIN;
    pub const MAX: u4 = u4::MAX;

    #[inline]
    pub const fn code(&self) -> u4 {
        return match self {
            Self::Unknown(x) => *x,

            Self::Query  => u4::new(0),
            Self::IQuery => u4::new(1),
            Self::Status => u4::new(2),

            Self::Notify                => u4::new(4),
            Self::Update                => u4::new(5),
            Self::DNSStatefulOperations => u4::new(6),
        };
    }

    #[inline]
    pub const fn mnemonic(&self) -> &str {
        return match self {
            Self::Unknown(_) => "Unknown",

            Self::Query  => "Query",
            Self::IQuery => "Inverse Query",
            Self::Status => "Status",

            Self::Notify                => "Notify",
            Self::Update                => "Update",
            Self::DNSStatefulOperations => "DNS Stateful Operations",
        };
    }

    #[inline]
    pub fn from_code(value: u4) -> Self {
        return match u8::from(value) {
            0 =>      Self::Query,
            1 =>      Self::IQuery,
            2 =>      Self::Status,
            3 =>      Self::Unknown(value),
            4 =>      Self::Notify,
            5 =>      Self::Update,
            6 =>      Self::DNSStatefulOperations,
            7..=15 => Self::Unknown(value),

            _ => panic!("codes greater than 15 are invalid options for OpCodes")
        };
    }
}

impl Display for OpCode {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.mnemonic())
    }
}

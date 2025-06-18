use std::{error::Error, fmt::Display};

use crate::gen_enum::enum_encoding;

#[derive(Debug)]
pub enum RClassError {
    UnknownMnemonic(String),
}
impl Error for RClassError {}
impl Display for RClassError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownMnemonic(mnemonic) => write!(f, "unknown class mnemonic '{mnemonic}'"),
        }
    }
}

enum_encoding!(
    (doc "https://www.iana.org/assignments/dns-parameters/dns-parameters.xhtml#dns-parameters-2"),
    RClass,
    u16,
    RClassError,
    (
        (Internet, "IN", 1),
        (CSNet,    "CS", 2),
        (Chaos,    "CH", 3),
        (Hesiod,   "HS", 4),

        (QClassNone, "NONE", 254),
        (QClassAny,  "ANY",  255),
    ),
    (wildcard_or_mnemonic_from_str, "CLASS"),
    mnemonic_presentation,
    mnemonic_display
);

pub trait RClassCode {
    fn rclass(&self) -> RClass;
}

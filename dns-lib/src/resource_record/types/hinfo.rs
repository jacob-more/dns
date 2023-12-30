use dns_macros::{ToWire, FromWire, FromTokenizedRecord, RTypeCode};

use crate::{types::character_string::CharacterString, serde::wire::circular_test::gen_test_circular_serde_sanity_test};

/// (Original) https://datatracker.ietf.org/doc/html/rfc1035#section-3.3.2
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, FromTokenizedRecord, RTypeCode)]
pub struct HINFO {
    cpu: CharacterString,
    os: CharacterString,
}

impl HINFO {
    #[inline]
    pub fn cpu(&self) -> &CharacterString {
        &self.cpu
    }

    #[inline]
    pub fn os(&self) -> &CharacterString {
        &self.os
    }
}

gen_test_circular_serde_sanity_test!(
    record_circular_serde_sanity_test,
    HINFO {
        cpu: CharacterString::from_utf8("PRIME-9650").unwrap(),
        os: CharacterString::from_utf8("PRIMOS").unwrap(),
    }
);

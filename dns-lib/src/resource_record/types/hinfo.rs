use dns_macros::{ToWire, FromWire, FromTokenizedRecord, RTypeCode, ToPresentation};

use crate::types::character_string::CharacterString;

/// (Original) https://datatracker.ietf.org/doc/html/rfc1035#section-3.3.2
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, ToPresentation, FromTokenizedRecord, RTypeCode)]
pub struct HINFO {
    cpu: CharacterString,
    os: CharacterString,
}

impl HINFO {
    #[inline]
    pub fn new(cpu: CharacterString, os: CharacterString) -> Self {
        Self { cpu, os }
    }

    #[inline]
    pub fn cpu(&self) -> &CharacterString {
        &self.cpu
    }

    #[inline]
    pub fn os(&self) -> &CharacterString {
        &self.os
    }
}

#[cfg(test)]
mod circular_serde_sanity_test {
    use crate::{serde::wire::circular_test::gen_test_circular_serde_sanity_test, types::character_string::CharacterString};
    use super::HINFO;

    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test,
        HINFO {
            cpu: CharacterString::from_utf8("PRIME-9650").unwrap(),
            os: CharacterString::from_utf8("PRIMOS").unwrap(),
        }
    );
}

#[cfg(test)]
mod tokenizer_tests {
    use crate::{serde::presentation::test_from_tokenized_record::{gen_ok_record_test, gen_fail_record_test}, types::character_string::CharacterString};
    use super::HINFO;

    const GOOD_CPU: &str = "PRIME-9650";
    const GOOD_OS: &str = "PRIMOS";

    gen_ok_record_test!(test_ok, HINFO, HINFO { cpu: CharacterString::from_utf8(GOOD_CPU).unwrap(), os: CharacterString::from_utf8(GOOD_OS).unwrap() }, [GOOD_CPU, GOOD_OS]);
    gen_fail_record_test!(test_fail_three_tokens, HINFO, [GOOD_CPU, GOOD_OS, GOOD_CPU]);
    gen_fail_record_test!(test_fail_one_token, HINFO, [GOOD_CPU]);
    gen_fail_record_test!(test_fail_no_tokens, HINFO, []);
}

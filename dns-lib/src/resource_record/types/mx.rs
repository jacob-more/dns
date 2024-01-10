use dns_macros::{ToWire, FromWire, FromTokenizedRecord, RTypeCode, ToPresentation};

use crate::types::c_domain_name::CDomainName;

/// (Original) https://datatracker.ietf.org/doc/html/rfc1035#section-3.3.9
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, ToPresentation, FromTokenizedRecord, RTypeCode)]
pub struct MX {
    preference: u16,
    exchange: CDomainName,
}

impl MX {
    #[inline]
    pub fn new(preference: u16, exchange: CDomainName) -> Self {
        Self { preference, exchange }
    }

    #[inline]
    pub fn preference(&self) -> u16 {
        self.preference
    }

    #[inline]
    pub fn exchange(&self) -> &CDomainName {
        &self.exchange
    }
}

#[cfg(test)]
mod circular_serde_sanity_test {
    use crate::{serde::wire::circular_test::gen_test_circular_serde_sanity_test, types::c_domain_name::CDomainName};
    use super::MX;

    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test,
        MX {
            preference: 10,
            exchange: CDomainName::from_utf8("www.example.com.").unwrap(),
        }
    );
}

#[cfg(test)]
mod tokenizer_tests {
    use crate::{serde::presentation::test_from_tokenized_record::{gen_ok_record_test, gen_fail_record_test}, types::c_domain_name::CDomainName};
    use super::MX;

    const GOOD_DOMAIN: &str = "www.example.com.";
    const BAD_DOMAIN: &str = "..www.example.com.";

    const GOOD_PREFERENCE: &str = "10";
    const BAD_PREFERENCE: &str = "-1";

    gen_ok_record_test!(test_ok, MX, MX { preference: 10, exchange: CDomainName::from_utf8(GOOD_DOMAIN).unwrap() }, [GOOD_PREFERENCE, GOOD_DOMAIN]);

    gen_fail_record_test!(test_fail_three_tokens, MX, [GOOD_PREFERENCE, GOOD_DOMAIN, GOOD_DOMAIN]);
    gen_fail_record_test!(test_fail_two_domains, MX, [GOOD_DOMAIN, GOOD_DOMAIN]);
    gen_fail_record_test!(test_fail_one_domain, MX, [GOOD_DOMAIN]);
    gen_fail_record_test!(test_fail_one_preference, MX, [GOOD_PREFERENCE]);
    gen_fail_record_test!(test_fail_no_tokens, MX, []);

    gen_fail_record_test!(test_fail_bad_preference, MX, [BAD_PREFERENCE, GOOD_DOMAIN]);
    gen_fail_record_test!(test_fail_bad_domain, MX, [GOOD_PREFERENCE, BAD_DOMAIN]);
    gen_fail_record_test!(test_fail_bad_domain_and_preference, MX, [BAD_PREFERENCE, BAD_DOMAIN]);
}

use dns_macros::{ToWire, FromWire, FromTokenizedRData, RData, ToPresentation};

use crate::types::domain_name::DomainName;

/// (Original) https://datatracker.ietf.org/doc/html/rfc3596
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, ToPresentation, FromTokenizedRData, RData)]
pub struct AFSDB {
    subtype: u16,
    hostname: DomainName
}

impl AFSDB {
    #[inline]
    pub fn new(subtype: u16, hostname: DomainName) -> Self {
        Self { subtype, hostname }
    }

    #[inline]
    pub fn subtype(&self) -> &u16 {
        &self.subtype
    }

    #[inline]
    pub fn hostname(&self) -> &DomainName {
        &self.hostname
    }

    #[inline]
    pub fn into_hostname(self) -> DomainName {
        self.hostname
    }
}

#[cfg(test)]
mod circular_serde_sanity_test {
    use crate::{serde::wire::circular_test::gen_test_circular_serde_sanity_test, types::domain_name::DomainName};
    use super::AFSDB;

    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test,
        AFSDB { subtype: 1, hostname: DomainName::from_utf8("www.example.org.").unwrap() }
    );
}

#[cfg(test)]
mod tokenizer_tests {
    use crate::{serde::presentation::test_from_tokenized_rdata::{gen_ok_record_test, gen_fail_record_test}, types::domain_name::DomainName};
    use super::AFSDB;

    const GOOD_SUBTYPE: &str = "1";
    const BAD_SUBTYPE: &str = "-1";
    const EMPTY_SUBTYPE: &str = "";

    const GOOD_DOMAIN: &str = "www.example.org.";
    const BAD_DOMAIN: &str = "..www.example.org.";

    gen_ok_record_test!(test_ok, AFSDB, AFSDB { subtype: 1, hostname: DomainName::from_utf8(GOOD_DOMAIN).unwrap() }, [GOOD_SUBTYPE, GOOD_DOMAIN]);
    gen_fail_record_test!(test_fail_bad_subtype, AFSDB, [BAD_SUBTYPE, GOOD_DOMAIN]);
    gen_fail_record_test!(test_fail_empty_subtype, AFSDB, [EMPTY_SUBTYPE, GOOD_DOMAIN]);
    gen_fail_record_test!(test_fail_bad_domain, AFSDB, [GOOD_SUBTYPE, BAD_DOMAIN]);
    gen_fail_record_test!(test_fail_three_tokens, AFSDB, [GOOD_SUBTYPE, GOOD_DOMAIN, GOOD_DOMAIN]);
    gen_fail_record_test!(test_fail_bad_token, AFSDB, [GOOD_SUBTYPE]);
    gen_fail_record_test!(test_fail_no_tokens, AFSDB, []);
}

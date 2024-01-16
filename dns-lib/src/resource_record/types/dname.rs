use dns_macros::{ToWire, FromWire, FromTokenizedRData, RTypeCode, ToPresentation};

use crate::types::domain_name::DomainName;

/// TODO: read RFC 2672
/// 
/// (Original) https://datatracker.ietf.org/doc/html/rfc6672
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, ToPresentation, FromTokenizedRData, RTypeCode)]
pub struct DNAME {
    target: DomainName,
}

impl DNAME {
    #[inline]
    pub fn new(target: DomainName) -> Self {
        Self { target }
    }

    #[inline]
    pub fn target_name(&self) -> &DomainName {
        &self.target
    }
}

#[cfg(test)]
mod circular_serde_sanity_test {
    use crate::{serde::wire::circular_test::gen_test_circular_serde_sanity_test, types::domain_name::DomainName};
    use super::DNAME;

    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test,
        DNAME { target: DomainName::from_utf8("www.example.com.").unwrap() }
    );
}

#[cfg(test)]
mod tokenizer_tests {
    use crate::{serde::presentation::test_from_tokenized_rdata::{gen_ok_record_test, gen_fail_record_test}, types::domain_name::DomainName};
    use super::DNAME;

    const GOOD_DOMAIN: &str = "www.example.com.";
    const BAD_DOMAIN: &str = "..www.example.org.";

    gen_ok_record_test!(test_ok, DNAME, DNAME { target: DomainName::from_utf8(GOOD_DOMAIN).unwrap() }, [GOOD_DOMAIN]);

    gen_fail_record_test!(test_fail_bad_domain, DNAME, [BAD_DOMAIN]);
    gen_fail_record_test!(test_fail_two_tokens, DNAME, [GOOD_DOMAIN, GOOD_DOMAIN]);
    gen_fail_record_test!(test_fail_no_tokens, DNAME, []);
}

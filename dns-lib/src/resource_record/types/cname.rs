use dns_macros::{ToWire, FromWire, FromTokenizedRData, RTypeCode, ToPresentation};

use crate::types::c_domain_name::CDomainName;

#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, ToPresentation, FromTokenizedRData, RTypeCode)]
pub struct CNAME {
    primary_name: CDomainName,
}

impl CNAME {
    #[inline]
    pub fn new(primary_name: CDomainName) -> Self {
        Self { primary_name }
    }

    #[inline]
    pub fn primary_name(&self) -> &CDomainName {
        &self.primary_name
    }
}

#[cfg(test)]
mod circular_serde_sanity_test {
    use crate::{serde::wire::circular_test::gen_test_circular_serde_sanity_test, types::c_domain_name::CDomainName};
    use super::CNAME;

    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test,
        CNAME { primary_name: CDomainName::from_utf8("www.example.com.").unwrap() }
    );
}

#[cfg(test)]
mod tokenizer_tests {
    use crate::{serde::presentation::test_from_tokenized_rdata::{gen_ok_record_test, gen_fail_record_test}, types::c_domain_name::CDomainName};
    use super::CNAME;

    const GOOD_DOMAIN: &str = "www.example.com.";
    const BAD_DOMAIN: &str = "..www.example.org.";

    gen_ok_record_test!(test_ok, CNAME, CNAME { primary_name: CDomainName::from_utf8(GOOD_DOMAIN).unwrap() }, [GOOD_DOMAIN]);

    gen_fail_record_test!(test_fail_bad_domain, CNAME, [BAD_DOMAIN]);
    gen_fail_record_test!(test_fail_two_tokens, CNAME, [GOOD_DOMAIN, GOOD_DOMAIN]);
    gen_fail_record_test!(test_fail_no_tokens, CNAME, []);
}

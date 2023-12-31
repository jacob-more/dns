use dns_macros::{ToWire, FromWire, FromTokenizedRecord, RTypeCode};

use crate::types::c_domain_name::CDomainName;

#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, FromTokenizedRecord, RTypeCode)]
pub struct CNAME {
    primary_name: CDomainName,
}

impl CNAME {
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
    use crate::{serde::presentation::test_from_tokenized_record::{gen_ok_record_test, gen_fail_record_test}, types::c_domain_name::CDomainName};
    use super::CNAME;

    const GOOD_DOMAIN_NAME: &str = "www.example.com.";

    gen_ok_record_test!(test_ok, CNAME, CNAME { primary_name: CDomainName::from_utf8(GOOD_DOMAIN_NAME).unwrap() }, [GOOD_DOMAIN_NAME]);
    gen_fail_record_test!(test_fail_two_tokens, CNAME, [GOOD_DOMAIN_NAME, GOOD_DOMAIN_NAME]);
    gen_fail_record_test!(test_fail_no_tokens, CNAME, []);
}
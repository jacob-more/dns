use dns_macros::{ToWire, FromWire, FromTokenizedRecord, RTypeCode, ToPresentation};

use crate::types::c_domain_name::CDomainName;

/// (Original) https://datatracker.ietf.org/doc/html/rfc1035#section-3.3.8
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, ToPresentation, FromTokenizedRecord, RTypeCode)]
pub struct MR {
    new_domain_name: CDomainName,
}

impl MR {
    #[inline]
    pub fn new(new_domain_name: CDomainName) -> Self {
        Self { new_domain_name }
    }

    #[inline]
    pub fn mailbox_rename_domain_name(&self) -> &CDomainName {
        &self.new_domain_name
    }
}

#[cfg(test)]
mod circular_serde_sanity_test {
    use crate::{serde::wire::circular_test::gen_test_circular_serde_sanity_test, types::c_domain_name::CDomainName};
    use super::MR;

    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test,
        MR { new_domain_name: CDomainName::from_utf8("www.example.com.").unwrap() }
    );
}

#[cfg(test)]
mod tokenizer_tests {
    use crate::{serde::presentation::test_from_tokenized_record::{gen_ok_record_test, gen_fail_record_test}, types::c_domain_name::CDomainName};
    use super::MR;

    const GOOD_DOMAIN: &str = "www.example.com.";
    const BAD_DOMAIN: &str = "..www.example.com.";

    gen_ok_record_test!(test_ok, MR, MR { new_domain_name: CDomainName::from_utf8(GOOD_DOMAIN).unwrap() }, [GOOD_DOMAIN]);

    gen_fail_record_test!(test_fail_bad_domain, MR, [BAD_DOMAIN]);
    gen_fail_record_test!(test_fail_two_tokens, MR, [GOOD_DOMAIN, GOOD_DOMAIN]);
    gen_fail_record_test!(test_fail_no_tokens, MR, []);
}

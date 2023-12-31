use dns_macros::{ToWire, FromWire, FromTokenizedRecord, RTypeCode};

use crate::types::c_domain_name::CDomainName;

/// (Original) https://datatracker.ietf.org/doc/html/rfc1035#section-3.3.5
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, FromTokenizedRecord, RTypeCode)]
pub struct MF {
    ma_domain_name: CDomainName,
}

impl MF {
    #[inline]
    pub fn new(ma_domain_name: CDomainName) -> Self {
        Self { ma_domain_name }
    }

    #[inline]
    pub fn mail_forwarding_agent_domain_name(&self) -> &CDomainName {
        &self.ma_domain_name
    }
}

#[cfg(test)]
mod circular_serde_sanity_test {
    use crate::{serde::wire::circular_test::gen_test_circular_serde_sanity_test, types::c_domain_name::CDomainName};
    use super::MF;

    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test,
        MF { ma_domain_name: CDomainName::from_utf8("www.example.com.").unwrap() }
    );
}

#[cfg(test)]
mod tokenizer_tests {
    use crate::{serde::presentation::test_from_tokenized_record::{gen_ok_record_test, gen_fail_record_test}, types::c_domain_name::CDomainName};
    use super::MF;

    const GOOD_DOMAIN_NAME: &str = "www.example.com.";

    gen_ok_record_test!(test_ok, MF, MF { ma_domain_name: CDomainName::from_utf8(GOOD_DOMAIN_NAME).unwrap() }, [GOOD_DOMAIN_NAME]);
    gen_fail_record_test!(test_fail_two_tokens, MF, [GOOD_DOMAIN_NAME, GOOD_DOMAIN_NAME]);
    gen_fail_record_test!(test_fail_no_tokens, MF, []);
}

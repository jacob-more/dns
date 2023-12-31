use dns_macros::{ToWire, FromWire, FromTokenizedRecord, RTypeCode};

use crate::types::c_domain_name::CDomainName;

/// (Original) https://datatracker.ietf.org/doc/html/rfc1035#section-3.3.7
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, FromTokenizedRecord, RTypeCode)]
pub struct MINFO {
    responsible_mailbox: CDomainName,
    error_mailbox: CDomainName,
}

impl MINFO {
    #[inline]
    pub fn responsible_mailbox(&self) -> &CDomainName {
        &self.responsible_mailbox
    }

    #[inline]
    pub fn error_mailbox(&self) -> &CDomainName {
        &self.error_mailbox
    }
}

#[cfg(test)]
mod circular_serde_sanity_test {
    use crate::{serde::wire::circular_test::gen_test_circular_serde_sanity_test, types::c_domain_name::CDomainName};
    use super::MINFO;

    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test,
        MINFO {
            responsible_mailbox: CDomainName::from_utf8("responsible.example.com.").unwrap(),
            error_mailbox: CDomainName::from_utf8("error.example.com.").unwrap(),
        }
    );
}

#[cfg(test)]
mod tokenizer_tests {
    use crate::{serde::presentation::test_from_tokenized_record::{gen_ok_record_test, gen_fail_record_test}, types::c_domain_name::CDomainName};
    use super::MINFO;

    const GOOD_DOMAIN_NAME: &str = "www.example.com.";

    gen_ok_record_test!(test_ok, MINFO, MINFO { responsible_mailbox: CDomainName::from_utf8(GOOD_DOMAIN_NAME).unwrap(), error_mailbox: CDomainName::from_utf8(GOOD_DOMAIN_NAME).unwrap() }, [GOOD_DOMAIN_NAME, GOOD_DOMAIN_NAME]);
    gen_fail_record_test!(test_fail_three_tokens, MINFO, [GOOD_DOMAIN_NAME, GOOD_DOMAIN_NAME, GOOD_DOMAIN_NAME]);
    gen_fail_record_test!(test_fail_one_tokens, MINFO, [GOOD_DOMAIN_NAME]);
    gen_fail_record_test!(test_fail_no_tokens, MINFO, []);
}

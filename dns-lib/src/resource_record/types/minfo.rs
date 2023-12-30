use dns_macros::{ToWire, FromWire, FromTokenizedRecord, RTypeCode};

use crate::{types::c_domain_name::CDomainName, serde::wire::circular_test::gen_test_circular_serde_sanity_test};

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

gen_test_circular_serde_sanity_test!(
    record_circular_serde_sanity_test,
    MINFO {
        responsible_mailbox: CDomainName::from_utf8("responsible.example.com.").unwrap(),
        error_mailbox: CDomainName::from_utf8("error.example.com.").unwrap(),
    }
);

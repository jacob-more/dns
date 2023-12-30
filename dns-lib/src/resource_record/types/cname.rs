use dns_macros::{ToWire, FromWire, FromTokenizedRecord, RTypeCode};

use crate::{types::c_domain_name::CDomainName, serde::wire::circular_test::gen_test_circular_serde_sanity_test};

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

gen_test_circular_serde_sanity_test!(
    record_circular_serde_sanity_test,
    CNAME { primary_name: CDomainName::from_utf8("www.example.com.").unwrap() }
);

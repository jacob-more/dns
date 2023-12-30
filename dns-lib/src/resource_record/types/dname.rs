use dns_macros::{ToWire, FromWire, FromTokenizedRecord, RTypeCode};

use crate::{types::domain_name::DomainName, serde::wire::circular_test::gen_test_circular_serde_sanity_test};

/// TODO: read RFC 2672
/// 
/// (Original) https://datatracker.ietf.org/doc/html/rfc6672
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, FromTokenizedRecord, RTypeCode)]
pub struct DNAME {
    target: DomainName,
}

impl DNAME {
    #[inline]
    pub fn target_name(&self) -> &DomainName {
        &self.target
    }
}

gen_test_circular_serde_sanity_test!(
    record_circular_serde_sanity_test,
    DNAME { target: DomainName::from_utf8("www.example.com.").unwrap() }
);

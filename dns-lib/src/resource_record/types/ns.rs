use dns_macros::{ToWire, FromWire, FromTokenizedRecord, RTypeCode};

use crate::{types::c_domain_name::CDomainName, serde::wire::circular_test::gen_test_circular_serde_sanity_test};

/// (Original) https://datatracker.ietf.org/doc/html/rfc1035#section-3.3.11
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, FromTokenizedRecord, RTypeCode)]
pub struct NS {
    ns_domain: CDomainName,
}

impl NS {
    #[inline]
    pub fn name_server_domain_name(&self) -> &CDomainName {
        &self.ns_domain
    }
}

gen_test_circular_serde_sanity_test!(
    record_circular_serde_sanity_test,
    NS { ns_domain: CDomainName::from_utf8("www.example.com.").unwrap() }
);

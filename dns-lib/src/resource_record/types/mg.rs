use dns_macros::{ToWire, FromWire, FromTokenizedRecord, RTypeCode};

use crate::{types::c_domain_name::CDomainName, serde::wire::circular_test::gen_test_circular_serde_sanity_test};

/// (Original) https://datatracker.ietf.org/doc/html/rfc1035#section-3.3.5
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, FromTokenizedRecord, RTypeCode)]
pub struct MG {
    mg_domain_name: CDomainName,
}

impl MG {
    #[inline]
    pub fn mailbox_group_domain_name(&self) -> &CDomainName {
        &self.mg_domain_name
    }
}

gen_test_circular_serde_sanity_test!(
    record_circular_serde_sanity_test,
    MG { mg_domain_name: CDomainName::from_utf8("www.example.com.").unwrap() }
);

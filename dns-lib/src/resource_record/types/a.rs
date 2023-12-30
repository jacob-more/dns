use std::net::Ipv4Addr;

use dns_macros::{ToWire, FromWire, FromTokenizedRecord, RTypeCode};

use crate::serde::wire::circular_test::gen_test_circular_serde_sanity_test;

/// (Original) https://datatracker.ietf.org/doc/html/rfc1035#section-3.4.1
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, FromTokenizedRecord, RTypeCode)]
pub struct A {
    ipv4_address: Ipv4Addr,
}

impl A {
    #[inline]
    pub fn ipv4_addr(&self) -> &Ipv4Addr {
        &self.ipv4_address
    }
}

gen_test_circular_serde_sanity_test!(
    record_circular_serde_sanity_test,
    A { ipv4_address: Ipv4Addr::new(192, 168, 86, 1) }
);

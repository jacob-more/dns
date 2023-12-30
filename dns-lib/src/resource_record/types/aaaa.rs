use std::net::Ipv6Addr;

use dns_macros::{ToWire, FromWire, FromTokenizedRecord, RTypeCode};

use crate::serde::wire::circular_test::gen_test_circular_serde_sanity_test;

/// (Original) https://datatracker.ietf.org/doc/html/rfc3596
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, FromTokenizedRecord, RTypeCode)]
pub struct AAAA {
    ipv6_address: Ipv6Addr,
}

impl AAAA {
    #[inline]
    pub fn ipv6_addr(&self) -> &Ipv6Addr {
        &self.ipv6_address
    }
}

gen_test_circular_serde_sanity_test!(
    record_circular_serde_sanity_test,
    AAAA { ipv6_address: Ipv6Addr::new(10, 9, 8, 7, 6, 5, 4, 3) }
);

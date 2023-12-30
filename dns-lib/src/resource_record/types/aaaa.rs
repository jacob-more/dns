use std::net::Ipv6Addr;

use dns_macros::{ToWire, FromWire, FromTokenizedRecord, RTypeCode};

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

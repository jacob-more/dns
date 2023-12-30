use std::net::Ipv4Addr;

use dns_macros::{ToWire, FromWire, FromTokenizedRecord, RTypeCode};

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

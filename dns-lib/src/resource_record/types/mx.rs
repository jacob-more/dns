use dns_macros::{ToWire, FromWire, FromTokenizedRecord, RTypeCode};

use crate::types::c_domain_name::CDomainName;

/// (Original) https://datatracker.ietf.org/doc/html/rfc1035#section-3.3.9
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, FromTokenizedRecord, RTypeCode)]
pub struct MX {
    preference: u16,
    exchange: CDomainName,
}

impl MX {
    #[inline]
    pub fn preference(&self) -> u16 {
        self.preference
    }

    #[inline]
    pub fn exchange(&self) -> &CDomainName {
        &self.exchange
    }
}

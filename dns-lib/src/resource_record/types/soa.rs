use dns_macros::{ToWire, FromWire, FromTokenizedRecord, RTypeCode};

use crate::types::c_domain_name::CDomainName;

/// (Original) https://datatracker.ietf.org/doc/html/rfc1035#section-3.3.13
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, FromTokenizedRecord, RTypeCode)]
pub struct SOA {
    mname: CDomainName,
    rname: CDomainName,
    serial: u32,
    refresh: i32,   // TODO: make DNSTime once that is defined
    retry: i32,     // TODO: make DNSTime once that is defined
    expire: i32,    // TODO: make DNSTime once that is defined
    minimum: u32,
}

impl SOA {
    #[inline]
    pub fn main_domain_name(&self) -> &CDomainName {
        &self.mname
    }

    #[inline]
    pub fn responsible_mailbox_domain_name(&self) -> &CDomainName {
        &self.rname
    }
}

use dns_macros::{ToWire, FromWire, FromTokenizedRecord, RTypeCode};

use crate::{types::c_domain_name::CDomainName, serde::wire::circular_test::gen_test_circular_serde_sanity_test};

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

gen_test_circular_serde_sanity_test!(
    record_circular_serde_sanity_test,
    SOA {
        mname: CDomainName::from_utf8("name_server.example.com.").unwrap(),
        rname: CDomainName::from_utf8("responsible_person.example.com.").unwrap(),
        serial: 12,
        refresh: 60,
        retry: 15,
        expire: 86400,
        minimum: 0,
    }
);

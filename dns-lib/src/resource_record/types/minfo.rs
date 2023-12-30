use dns_macros::{ToWire, FromWire, FromTokenizedRecord, RTypeCode};

use crate::types::c_domain_name::CDomainName;

/// (Original) https://datatracker.ietf.org/doc/html/rfc1035#section-3.3.7
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, FromTokenizedRecord, RTypeCode)]
pub struct MINFO {
    responsible_mailbox: CDomainName,
    error_mailbox: CDomainName,
}

impl MINFO {
    #[inline]
    pub fn responsible_mailbox(&self) -> &CDomainName {
        &self.responsible_mailbox
    }

    #[inline]
    pub fn error_mailbox(&self) -> &CDomainName {
        &self.error_mailbox
    }
}

use dns_macros::{ToWire, FromWire, FromTokenizedRecord, RTypeCode};

use crate::types::c_domain_name::CDomainName;

/// (Original) https://datatracker.ietf.org/doc/html/rfc1035#section-3.3.8
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, FromTokenizedRecord, RTypeCode)]
pub struct MR {
    new_domain_name: CDomainName,
}

impl MR {
    #[inline]
    pub fn mailbox_rename_domain_name(&self) -> &CDomainName {
        &self.new_domain_name
    }
}

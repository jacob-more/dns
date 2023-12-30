use dns_macros::{ToWire, FromWire, FromTokenizedRecord, RTypeCode};

use crate::types::domain_name::DomainName;

/// TODO: read RFC 2672
/// 
/// (Original) https://datatracker.ietf.org/doc/html/rfc6672
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, FromTokenizedRecord, RTypeCode)]
pub struct DNAME {
    target: DomainName,
}

impl DNAME {
    #[inline]
    pub fn target_name(&self) -> &DomainName {
        &self.target
    }
}

use dns_macros::{ToWire, FromWire, FromTokenizedRecord, RTypeCode};

use crate::types::c_domain_name::CDomainName;

#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, FromTokenizedRecord, RTypeCode)]
pub struct CNAME {
    primary_name: CDomainName,
}

impl CNAME {
    #[inline]
    pub fn primary_name(&self) -> &CDomainName {
        &self.primary_name
    }
}

use dns_macros::{ToWire, FromWire, FromTokenizedRecord, RTypeCode};

use crate::types::c_domain_name::CDomainName;

/// (Original) https://datatracker.ietf.org/doc/html/rfc1035#section-3.3.4
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, FromTokenizedRecord, RTypeCode)]
pub struct MD {
    ma_domain_name: CDomainName,
}

impl MD {
    #[inline]
    pub fn mail_agent_domain_name(&self) -> &CDomainName {
        &self.ma_domain_name
    }
}

use dns_macros::{ToWire, FromWire, FromTokenizedRData, RData, ToPresentation};

use crate::types::domain_name::DomainName;

/// (Original) https://datatracker.ietf.org/doc/html/rfc2782
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, ToPresentation, FromTokenizedRData, RData)]
pub struct SRV {
    priority: u16,
    weight: u16,
    port: u16,
    target: DomainName,
}

impl SRV {
    #[inline]
    pub fn new(priority: u16, weight: u16, port: u16, target: DomainName) -> Self {
        Self {
            priority,
            weight,
            port,
            target,
        }
    }

    #[inline]
    pub fn priority(&self) -> u16 { self.priority }

    #[inline]
    pub fn weight(&self) -> u16 { self.weight }

    #[inline]
    pub fn port(&self) -> u16 { self.port }

    #[inline]
    pub fn target(&self) -> &DomainName { &self.target }
}

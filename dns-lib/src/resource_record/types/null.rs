use dns_macros::{ToWire, FromWire, RTypeCode};

/// (Original) https://datatracker.ietf.org/doc/html/rfc1035#section-3.3.11
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, RTypeCode)]
pub struct NULL {
    any: Vec<u8>,
}

impl NULL {
    #[inline]
    pub fn any(&self) -> &[u8] {
        &self.any
    }
}

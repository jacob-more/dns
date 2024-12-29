use dns_macros::{FromTokenizedRData, FromWire, RData, ToPresentation, ToWire};


/// (Original) https://datatracker.ietf.org/doc/html/rfc7043#section-4
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, FromTokenizedRData, ToPresentation, RData)]
pub struct EUI64 {
    address: u64,
}

impl EUI64 {
    #[inline]
    pub const fn new(address: u64) -> Self {
        Self { address }
    }

    #[inline]
    pub const fn address(&self) -> u64 {
        self.address
    }
}

#[cfg(test)]
mod circular_sanity_tests {
    use crate::serde::wire::circular_test::gen_test_circular_serde_sanity_test;

    use super::EUI64;

    gen_test_circular_serde_sanity_test!(
        rfc_7043_example_record_circular_serde_sanity_test,
        EUI64 { address: 0x00_00_5e_ef_10_00_00_2a }
    );
}

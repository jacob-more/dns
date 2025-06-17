use dns_macros::{FromTokenizedRData, FromWire, RData, ToPresentation, ToWire};
use ux::u48;

/// (Original) https://datatracker.ietf.org/doc/html/rfc7043#section-3
#[derive(
    Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, FromTokenizedRData, ToPresentation, RData,
)]
pub struct EUI48 {
    address: u48,
}

impl EUI48 {
    #[inline]
    pub const fn new(address: u48) -> Self {
        Self { address }
    }

    #[inline]
    pub const fn address(&self) -> u48 {
        self.address
    }
}

#[cfg(test)]
mod circular_sanity_tests {
    use ux::u48;

    use crate::serde::wire::circular_test::gen_test_circular_serde_sanity_test;

    use super::EUI48;

    gen_test_circular_serde_sanity_test!(
        rfc_7043_example_record_circular_serde_sanity_test,
        EUI48 {
            address: u48::new(0x00_00_5e_00_53_2a)
        }
    );
}

use dns_macros::{ToWire, FromWire, RData};

/// (Original) https://datatracker.ietf.org/doc/html/rfc1035#section-3.2.3
/// (Updated)  https://datatracker.ietf.org/doc/html/rfc5936#section-2
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, RData)]
pub struct AXFR {}

impl AXFR {
    #[inline]
    pub fn new() -> Self {
        Self {}
    }
}

#[cfg(test)]
mod circular_serde_sanity_test {
    use crate::serde::wire::circular_test::gen_test_circular_serde_sanity_test;
    use super::AXFR;

    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test,
        AXFR {}
    );
}

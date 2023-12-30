use dns_macros::{ToWire, FromWire, RTypeCode};

use crate::serde::wire::circular_test::gen_test_circular_serde_sanity_test;

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

gen_test_circular_serde_sanity_test!(
    record_circular_serde_sanity_test,
    NULL { any: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15] }
);

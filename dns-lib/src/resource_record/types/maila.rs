use dns_macros::{ToWire, FromWire, FromTokenizedRecord, RTypeCode};

use crate::serde::wire::circular_test::gen_test_circular_serde_sanity_test;

/// (Original) https://datatracker.ietf.org/doc/html/rfc1035#section-3.2.3
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, FromTokenizedRecord, RTypeCode)]
pub struct MAILA {}

gen_test_circular_serde_sanity_test!(
    record_circular_serde_sanity_test,
    MAILA {}
);

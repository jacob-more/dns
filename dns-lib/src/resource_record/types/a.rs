use std::net::Ipv4Addr;

use dns_macros::{FromTokenizedRData, FromWire, RData, ToPresentation, ToWire};

/// (Original) https://datatracker.ietf.org/doc/html/rfc1035#section-3.4.1
#[derive(
    Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, ToPresentation, FromTokenizedRData, RData,
)]
pub struct A {
    ipv4_address: Ipv4Addr,
}

impl A {
    #[inline]
    pub fn new(ipv4_address: Ipv4Addr) -> Self {
        Self { ipv4_address }
    }

    #[inline]
    pub fn ipv4_addr(&self) -> &Ipv4Addr {
        &self.ipv4_address
    }

    #[inline]
    pub fn into_ipv4_addr(self) -> Ipv4Addr {
        self.ipv4_address
    }
}

#[cfg(test)]
mod circular_serde_sanity_test {
    use std::net::Ipv4Addr;

    use super::A;
    use crate::serde::wire::circular_test::gen_test_circular_serde_sanity_test;

    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test,
        A {
            ipv4_address: Ipv4Addr::new(192, 168, 86, 1)
        }
    );
}

#[cfg(test)]
mod tokenizer_tests {
    use super::A;
    use crate::serde::presentation::test_from_tokenized_rdata::{
        gen_fail_record_test, gen_ok_record_test,
    };
    use std::net::Ipv4Addr;

    gen_ok_record_test!(
        test_ok,
        A,
        A {
            ipv4_address: Ipv4Addr::new(192, 168, 86, 1)
        },
        ["192.168.86.1"]
    );
    gen_fail_record_test!(test_fail_two_tokens, A, ["192.168.86.1", "192.168.86.1"]);
    gen_fail_record_test!(test_fail_bad_token, A, ["192.168.86.1.0"]);
    gen_fail_record_test!(test_fail_no_tokens, A, []);
}

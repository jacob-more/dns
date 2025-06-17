use std::net::Ipv6Addr;

use dns_macros::{FromTokenizedRData, FromWire, RData, ToPresentation, ToWire};

/// (Original) https://datatracker.ietf.org/doc/html/rfc3596
#[derive(
    Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, ToPresentation, FromTokenizedRData, RData,
)]
pub struct AAAA {
    ipv6_address: Ipv6Addr,
}

impl AAAA {
    #[inline]
    pub fn new(ipv6_address: Ipv6Addr) -> Self {
        Self { ipv6_address }
    }

    #[inline]
    pub fn ipv6_addr(&self) -> &Ipv6Addr {
        &self.ipv6_address
    }

    #[inline]
    pub fn into_ipv6_addr(self) -> Ipv6Addr {
        self.ipv6_address
    }
}

#[cfg(test)]
mod circular_serde_sanity_test {
    use std::net::Ipv6Addr;

    use super::AAAA;
    use crate::serde::wire::circular_test::gen_test_circular_serde_sanity_test;

    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test,
        AAAA {
            ipv6_address: Ipv6Addr::new(10, 9, 8, 7, 6, 5, 4, 3)
        }
    );
}

#[cfg(test)]
mod tokenizer_tests {
    use super::AAAA;
    use crate::serde::presentation::test_from_tokenized_rdata::{
        gen_fail_record_test, gen_ok_record_test,
    };
    use std::net::Ipv6Addr;

    const GOOD_IP: &str = "a:9:8:7:6:5:4:3";
    const BAD_IP: &str = "a:9:8:7:6:5:4:3:2:1";

    gen_ok_record_test!(
        test_ok,
        AAAA,
        AAAA {
            ipv6_address: Ipv6Addr::new(10, 9, 8, 7, 6, 5, 4, 3)
        },
        [GOOD_IP]
    );
    gen_fail_record_test!(test_fail_two_tokens, AAAA, [GOOD_IP, GOOD_IP]);
    gen_fail_record_test!(test_fail_bad_token, AAAA, [BAD_IP]);
    gen_fail_record_test!(test_fail_no_tokens, AAAA, []);
}

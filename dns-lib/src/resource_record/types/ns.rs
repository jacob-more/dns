use dns_macros::{FromTokenizedRData, FromWire, RData, ToPresentation, ToWire};

use crate::types::c_domain_name::CDomainName;

/// (Original) https://datatracker.ietf.org/doc/html/rfc1035#section-3.3.11
#[derive(
    Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, ToPresentation, FromTokenizedRData, RData,
)]
pub struct NS {
    ns_domain: CDomainName,
}

impl NS {
    #[inline]
    pub fn new(ns_domain_name: CDomainName) -> Self {
        Self {
            ns_domain: ns_domain_name,
        }
    }

    #[inline]
    pub fn name_server_domain_name(&self) -> &CDomainName {
        &self.ns_domain
    }

    #[inline]
    pub fn into_name_server_domain_name(self) -> CDomainName {
        self.ns_domain
    }
}

#[cfg(test)]
mod circular_serde_sanity_test {
    use super::NS;
    use crate::{
        serde::wire::circular_test::gen_test_circular_serde_sanity_test,
        types::c_domain_name::CDomainName,
    };

    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test,
        NS {
            ns_domain: CDomainName::from_utf8("www.example.com.").unwrap()
        }
    );
}

#[cfg(test)]
mod tokenizer_tests {
    use super::NS;
    use crate::{
        serde::presentation::test_from_tokenized_rdata::{
            gen_fail_record_test, gen_ok_record_test,
        },
        types::c_domain_name::CDomainName,
    };

    const GOOD_DOMAIN: &str = "www.example.com.";
    const BAD_DOMAIN: &str = "..www.example.com.";

    gen_ok_record_test!(
        test_ok,
        NS,
        NS {
            ns_domain: CDomainName::from_utf8(GOOD_DOMAIN).unwrap()
        },
        [GOOD_DOMAIN]
    );

    gen_fail_record_test!(test_fail_bad_domain, NS, [BAD_DOMAIN]);
    gen_fail_record_test!(test_fail_two_tokens, NS, [GOOD_DOMAIN, GOOD_DOMAIN]);
    gen_fail_record_test!(test_fail_no_tokens, NS, []);
}

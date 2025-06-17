use dns_macros::{FromTokenizedRData, FromWire, RData, ToPresentation, ToWire};

use crate::types::c_domain_name::CDomainName;

/// (Original) https://datatracker.ietf.org/doc/html/rfc1035#section-3.4.1
#[derive(
    Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, ToPresentation, FromTokenizedRData, RData,
)]
pub struct PTR {
    ptr_domain_name: CDomainName,
}

impl PTR {
    #[inline]
    pub fn new(ptr_domain_name: CDomainName) -> Self {
        Self { ptr_domain_name }
    }

    #[inline]
    pub fn ptr_domain_name(&self) -> &CDomainName {
        &self.ptr_domain_name
    }
}

#[cfg(test)]
mod circular_serde_sanity_test {
    use super::PTR;
    use crate::{
        serde::wire::circular_test::gen_test_circular_serde_sanity_test,
        types::c_domain_name::CDomainName,
    };

    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test,
        PTR {
            ptr_domain_name: CDomainName::from_utf8("www.example.org.").unwrap()
        }
    );
}

#[cfg(test)]
mod tokenizer_tests {
    use super::PTR;
    use crate::{
        serde::presentation::test_from_tokenized_rdata::{
            gen_fail_record_test, gen_ok_record_test,
        },
        types::c_domain_name::CDomainName,
    };

    const GOOD_DOMAIN: &str = "www.example.org.";
    const BAD_DOMAIN: &str = "..www.example.org.";

    gen_ok_record_test!(
        test_ok,
        PTR,
        PTR {
            ptr_domain_name: CDomainName::from_utf8(GOOD_DOMAIN).unwrap()
        },
        [GOOD_DOMAIN]
    );

    gen_fail_record_test!(test_fail_bad_domain, PTR, [BAD_DOMAIN]);
    gen_fail_record_test!(test_fail_two_tokens, PTR, [GOOD_DOMAIN, GOOD_DOMAIN]);
    gen_fail_record_test!(test_fail_no_tokens, PTR, []);
}

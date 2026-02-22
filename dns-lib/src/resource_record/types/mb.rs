use dns_macros::{FromTokenizedRData, FromWire, RData, ToPresentation, ToWire};

use crate::types::domain_name::{CompressibleDomainVec, DomainVec};

/// (Original) https://datatracker.ietf.org/doc/html/rfc1035#section-3.3.3
#[derive(
    Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, ToPresentation, FromTokenizedRData, RData,
)]
pub struct MB {
    ma_domain_name: CompressibleDomainVec,
}

impl MB {
    #[inline]
    pub fn new(ma_domain_name: DomainVec) -> Self {
        Self {
            ma_domain_name: CompressibleDomainVec(ma_domain_name),
        }
    }

    #[inline]
    pub fn mailbox_domain_name(&self) -> &DomainVec {
        &self.ma_domain_name
    }
}

#[cfg(test)]
mod circular_serde_sanity_test {
    use super::MB;
    use crate::{
        serde::wire::circular_test::gen_test_circular_serde_sanity_test,
        types::domain_name::{CompressibleDomainVec, DomainVec},
    };

    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test,
        MB {
            ma_domain_name: CompressibleDomainVec(
                DomainVec::from_utf8("www.example.com.").unwrap()
            )
        }
    );
}

#[cfg(test)]
mod tokenizer_tests {
    use super::MB;
    use crate::{
        serde::presentation::test_from_tokenized_rdata::{
            gen_fail_record_test, gen_ok_record_test,
        },
        types::domain_name::{CompressibleDomainVec, DomainVec},
    };

    const GOOD_DOMAIN: &str = "www.example.com.";
    const BAD_DOMAIN: &str = "..www.example.com.";

    gen_ok_record_test!(
        test_ok,
        MB,
        MB {
            ma_domain_name: CompressibleDomainVec(DomainVec::from_utf8(GOOD_DOMAIN).unwrap())
        },
        [GOOD_DOMAIN]
    );

    gen_fail_record_test!(test_fail_bad_domain, MB, [BAD_DOMAIN]);
    gen_fail_record_test!(test_fail_two_tokens, MB, [GOOD_DOMAIN, GOOD_DOMAIN]);
    gen_fail_record_test!(test_fail_no_tokens, MB, []);
}

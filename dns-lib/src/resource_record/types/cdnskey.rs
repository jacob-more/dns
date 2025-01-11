use std::{fmt::Debug, ops::{Deref, DerefMut}};

use dns_macros::{FromWire, RData, ToPresentation, ToWire};

use crate::serde::presentation::from_tokenized_rdata::FromTokenizedRData;

use super::dnskey::DNSKEY;


/// (Original) https://datatracker.ietf.org/doc/html/rfc7344#section-3.2
#[derive(Clone, PartialEq, Eq, Hash, ToWire, FromWire, ToPresentation, RData)]
pub struct CDNSKEY {
    key: DNSKEY
}

impl Debug for CDNSKEY {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CDNSKEY")
            .field("flags", &self.flags())
            .field("protocol", &self.protocol())
            .field("algorithm", &self.algorithm())
            .field("key", self.key())
            .finish()
    }
}

impl Deref for CDNSKEY {
    type Target = DNSKEY;

    fn deref(&self) -> &Self::Target {
        &self.key
    }
}

impl DerefMut for CDNSKEY {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.key
    }
}

impl FromTokenizedRData for CDNSKEY {
    fn from_tokenized_rdata(record: &Vec<&str>) -> Result<Self, crate::serde::presentation::errors::TokenizedRecordError> where Self: Sized {
        Ok(Self { key: DNSKEY::from_tokenized_rdata(record)? })
    }
}

#[cfg(test)]
mod circular_serde_sanity_test {
    use crate::{resource_record::dnssec_alg::DnsSecAlgorithm, serde::wire::circular_test::gen_test_circular_serde_sanity_test, types::base64::Base64};

    use super::{CDNSKEY, DNSKEY};

    gen_test_circular_serde_sanity_test!(
        rfc_4034_example_record_circular_serde_sanity_test,
        CDNSKEY {
            key: DNSKEY::new(
                256,
                DnsSecAlgorithm::from_code(5),
                Base64::from_utf8("AQPSKmynfzW4kyBv015MUG2DeIQ3Cbl+BBZH4b/0PY1kxkmvHjcZc8nokfzj31GajIQKY+5CptLr3buXA10hWqTkF7H6RfoRqXQeogmMHfpftf6zMv1LyBUgia7za6ZEzOJBOztyvhjL742iU/TpPSEDhm2SNKLijfUppn1UaNvv4w==").unwrap(),
            )
        }
    );
}

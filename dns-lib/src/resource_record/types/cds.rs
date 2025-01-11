use std::{fmt::Debug, ops::{Deref, DerefMut}};

use dns_macros::{FromWire, RData, ToPresentation, ToWire};

use crate::serde::presentation::from_tokenized_rdata::FromTokenizedRData;

use super::ds::DS;


/// (Original) https://datatracker.ietf.org/doc/html/rfc4034#section-5
#[derive(Clone, PartialEq, Eq, Hash, ToWire, FromWire, ToPresentation, RData)]
pub struct CDS {
    ds: DS
}

impl Debug for CDS {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CDS")
            .field("key_tag", &self.key_tag())
            .field("algorithm", &self.algorithm())
            .field("digest_type", &self.digest_type())
            .field("digest", self.digest())
            .finish()
    }
}

impl Deref for CDS {
    type Target = DS;

    fn deref(&self) -> &Self::Target {
        &self.ds
    }
}

impl DerefMut for CDS {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ds
    }
}

impl FromTokenizedRData for CDS {
    fn from_tokenized_rdata(record: &Vec<&str>) -> Result<Self, crate::serde::presentation::errors::TokenizedRecordError> where Self: Sized {
        Ok(Self { ds: DS::from_tokenized_rdata(record)? })
    }
}
#[cfg(test)]
mod circular_serde_sanity_test {
    use crate::{resource_record::{digest_alg::DigestAlgorithm, dnssec_alg::DnsSecAlgorithm, types::ds::DS}, serde::wire::circular_test::gen_test_circular_serde_sanity_test, types::base16::Base16};

    use super::CDS;

    gen_test_circular_serde_sanity_test!(
        rfc_4034_example_record_circular_serde_sanity_test,
        CDS {
            ds: DS::new(
                60485,
                DnsSecAlgorithm::from_code(5),
                DigestAlgorithm::from_code(1),
                Base16::from_utf8("2BB183AF5F22588179A53B0A98631FAD1A292118").unwrap(),
            )
        }
    );
}

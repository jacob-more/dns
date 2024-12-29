use dns_macros::{FromTokenizedRData, FromWire, RData, ToPresentation, ToWire};

use crate::{resource_record::{digest_alg::DigestAlgorithm, dnssec_alg::DnsSecAlgorithm}, types::base16::Base16};


/// (Original) https://datatracker.ietf.org/doc/html/rfc4034#section-5
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, FromTokenizedRData, ToPresentation, RData)]
pub struct DS {
    key_tag: u16,
    algorithm: DnsSecAlgorithm,
    digest_type: DigestAlgorithm,
    digest: Base16,
}

impl DS {
    #[inline]
    pub const fn new(key_tag: u16, algorithm: DnsSecAlgorithm, digest_type: DigestAlgorithm, digest: Base16) -> Self {
        Self { key_tag, algorithm, digest_type, digest }
    }

    #[inline]
    pub const fn key_tag(&self) -> u16 {
        self.key_tag
    }
    
    #[inline]
    pub const fn algorithm(&self) -> DnsSecAlgorithm {
        self.algorithm
    }
    
    #[inline]
    pub const fn digest_type(&self) -> DigestAlgorithm {
        self.digest_type
    }
    
    #[inline]
    pub const fn digest(&self) -> &Base16 {
        &self.digest
    }
    
    #[inline]
    pub fn into_digest(self) -> Base16 {
        self.digest
    }
}

#[cfg(test)]
mod circular_serde_sanity_test {
    use crate::{resource_record::{digest_alg::DigestAlgorithm, dnssec_alg::DnsSecAlgorithm}, serde::wire::circular_test::gen_test_circular_serde_sanity_test, types::base16::Base16};

    use super::DS;

    gen_test_circular_serde_sanity_test!(
        rfc_4034_example_record_circular_serde_sanity_test,
        DS {
            key_tag: 60485,
            algorithm: DnsSecAlgorithm::from_code(5),
            digest_type: DigestAlgorithm::from_code(1),
            digest: Base16::from_utf8("2BB183AF5F22588179A53B0A98631FAD1A292118").unwrap(),
        }
    );
}

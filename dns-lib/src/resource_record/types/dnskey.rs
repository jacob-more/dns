use dns_macros::{FromTokenizedRData, FromWire, RTypeCode, ToPresentation, ToWire};

use crate::{resource_record::dnssec_alg::DnsSecAlgorithm, types::base64::Base64};

const DNS_ZONE_KEY_FLAG_MASK: u16       = 0b0000_0001_0000_0000;
const SECURE_ENTRY_POINT_FLAG_MASK: u16 = 0b0000_0000_0000_0001;

/// (Original) https://datatracker.ietf.org/doc/html/rfc4034#section-2
/// (Update) https://datatracker.ietf.org/doc/html/rfc3225
/// (Update) https://datatracker.ietf.org/doc/html/rfc6840
/// (Update) https://datatracker.ietf.org/doc/html/rfc6944
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, ToPresentation, FromTokenizedRData, RTypeCode)]
pub struct DNSKEY {
    ///                     1 1 1 1 1 1
    /// 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5
    ///               │               │
    ///               │               └─ Secure Entry Point flag
    ///               └─ Zone Key flag
    flags: u16,
    /// The Protocol Field MUST have value 3, and the DNSKEY RR MUST be
    /// treated as invalid during signature verification if it is found to be
    /// some value other than 3.
    /// 
    /// Although the Protocol Field always has value 3, it is retained for
    /// backward compatibility with early versions of the KEY record.
    protocol: u8,
    algorithm: DnsSecAlgorithm,
    key: Base64,
}

impl DNSKEY {
    #[inline]
    pub const fn dns_zone_key(&self) -> bool {
        (self.flags & DNS_ZONE_KEY_FLAG_MASK) == DNS_ZONE_KEY_FLAG_MASK
    }

    #[inline]
    pub const fn secure_entry_point(&self) -> bool {
        (self.flags & SECURE_ENTRY_POINT_FLAG_MASK) == SECURE_ENTRY_POINT_FLAG_MASK
    }
}

#[cfg(test)]
mod circular_serde_sanity_test {
    use crate::{resource_record::dnssec_alg::DnsSecAlgorithm, serde::wire::circular_test::gen_test_circular_serde_sanity_test, types::base64::Base64};

    use super::DNSKEY;

    gen_test_circular_serde_sanity_test!(
        rfc_4034_example_record_circular_serde_sanity_test,
        DNSKEY {
            flags: 256,
            protocol: 3,
            algorithm: DnsSecAlgorithm::from_code(5),
            key: Base64::from_utf8("AQPSKmynfzW4kyBv015MUG2DeIQ3Cbl+BBZH4b/0PY1kxkmvHjcZc8nokfzj31GajIQKY+5CptLr3buXA10hWqTkF7H6RfoRqXQeogmMHfpftf6zMv1LyBUgia7za6ZEzOJBOztyvhjL742iU/TpPSEDhm2SNKLijfUppn1UaNvv4w==").unwrap(),
        }
    );
}

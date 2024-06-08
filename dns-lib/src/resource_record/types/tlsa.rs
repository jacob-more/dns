use dns_macros::{FromTokenizedRData, FromWire, RTypeCode, ToPresentation, ToWire};

use crate::{gen_enum::enum_encoding, types::base16::Base16};

/// (Original) https://datatracker.ietf.org/doc/html/rfc6698#section-2
/// (Updated) https://datatracker.ietf.org/doc/html/rfc8749#name-moving-dlv-to-historic-stat
/// (Updated) https://datatracker.ietf.org/doc/html/rfc7218
/// (Updated) https://datatracker.ietf.org/doc/html/rfc7671
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, ToPresentation, FromTokenizedRData, RTypeCode)]
pub struct TLSA {
    certificate_usage: CertificateUsage,
    selector: Selector,
    matching_type: MatchingType,
    // FIXME: Base16 needs to be able to decode from multiple whitespace separated tokens, not just one. This is not currently supported because I was running into issues with lifetimes.
    certificate: Base16,
}

enum_encoding!(
    CertificateUsage,
    u8,
    (
        (PkixTa, "PKIX-TA", 0),
        (PkixEe, "PKIX-EE", 1),
        (DaneTa, "DANE-TA", 2),
        (DaneEe, "DANE-EE", 3),
    
        (PrivCert, "PrivCert", 255),
    ),
    code_presentation,
    mnemonic_display
);

enum_encoding!(
    Selector,
    u8,
    (
        (Cert, "Cert", 0),
        (Spki, "SPKI", 1),
    
        (PrivSel, "PrivSel", 255),
    ),
    code_presentation,
    mnemonic_display
);

enum_encoding!(
    MatchingType,
    u8,
    (
        (Full,     "Full",     0),
        (Sha2_256, "SHA2-256", 1),
        (Sha2_512, "SHA2-512", 2),
    
        (PrivMatch, "PrivMatch", 255),
    ),
    code_presentation,
    mnemonic_display
);

#[cfg(test)]
mod tlsa_circular_serde_sanity_test {
    use crate::{serde::wire::circular_test::gen_test_circular_serde_sanity_test, types::base16::Base16};
    use super::{CertificateUsage, MatchingType, Selector, TLSA};

    gen_test_circular_serde_sanity_test!(
        rfc_6698_example_1_record_circular_serde_sanity_test,
        TLSA {
            certificate_usage: CertificateUsage::from_code(0),
            selector: Selector::from_code(0),
            matching_type: MatchingType::from_code(1),
            certificate: Base16::from_case_insensitive_utf8("d2abde240d7cd3ee6b4b28c54df034b97983a1d16e8a410e4561cb106618e971").unwrap()
        }
    );
    gen_test_circular_serde_sanity_test!(
        rfc_6698_example_2_record_circular_serde_sanity_test,
        TLSA {
            certificate_usage: CertificateUsage::from_code(1),
            selector: Selector::from_code(1),
            matching_type: MatchingType::from_code(2),
            certificate: Base16::from_case_insensitive_utf8("92003ba34942dc74152e2f2c408d29eca5a520e7f2e06bb944f4dca346baf63c1b177615d466f6c4b71c216a50292bd58c9ebdd2f74e38fe51ffd48c43326cbc").unwrap()
        }
    );
}

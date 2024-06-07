use std::{error::Error, fmt::Display};

use dns_macros::{FromTokenizedRData, FromWire, RTypeCode, ToPresentation, ToWire};

use crate::{resource_record::{dnssec_alg::DnsSecAlgorithm, gen_enum::enum_encoding}, types::base64::Base64};

/// (Original) https://datatracker.ietf.org/doc/html/rfc4398#section-2
/// (Updated) https://datatracker.ietf.org/doc/html/rfc6944
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, ToPresentation, FromTokenizedRData, RTypeCode)]
pub struct CERT {
    cert_type: CertificateType,
    key_tag: u16,
    algorithm: DnsSecAlgorithm,
    // FIXME: Base64 needs to be able to decode from multiple whitespace separated tokens, not just one. This is not currently supported because I was running into issues with lifetimes.
    certificate: Base64,
}

#[derive(Debug)]
pub enum CertificateTypeError<'a> {
    UnknownMnemonic(&'a str),
}
impl<'a> Error for CertificateTypeError<'a> {}
impl<'a> Display for CertificateTypeError<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownMnemonic(mnemonic) => write!(f, "unknown certificate type mnemonic '{mnemonic}'"),
        }
    }
}

enum_encoding!(
    (doc "https://datatracker.ietf.org/doc/html/rfc4398#section-2.1"),
    CertificateType,
    u16,
    (
        (Pkix,    "PKIX",    1),
        (Spki,    "SPKI",    2),
        (Pgp,     "PGP",     3),
        (Ipkix,   "IPKIX",   4),
        (Ispki,   "ISPKI",   5),
        (Ipgp,    "IPGP",    6),
        (Acpkix,  "ACPKIX",  7),
        (Iacpkix, "IACPKIX", 8),
    
        (Uri, "URI", 253),
        (Oid, "OID", 254),
    ),
    from_str,
    CertificateTypeError,
    mnemonic_from_presentation,
    mnemonic_to_presentation,
    display_mnemonic
);

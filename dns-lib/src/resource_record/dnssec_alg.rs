use std::{fmt::Display, error::Error};

use super::gen_enum::enum_encoding;

#[derive(Debug)]
pub enum DnsSecAlgorithmError<'a> {
    UnknownMnemonic(&'a str),
}
impl<'a> Error for DnsSecAlgorithmError<'a> {}
impl<'a> Display for DnsSecAlgorithmError<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownMnemonic(mnemonic) => write!(f, "unknown dns security algorithm mnemonic '{mnemonic}'"),
        }
    }
}

// https://www.iana.org/assignments/dns-sec-alg-numbers/dns-sec-alg-numbers.xhtml#dns-sec-alg-numbers-1
enum_encoding!(
    DnsSecAlgorithm,
    u8,
    (
        (Delete,           "DELETE",             0),
        (RsaMd5,           "RSAMD5",             1),
        (Dh,               "DH",                 2),
        (Dsa,              "DSA",                3),
        (Ecc,              "ECC",                4),
        (RsaSha1,          "RSASHA1",            5),
        (DsaNsec3Sha1,     "DSA-NSEC3-SHA1",     6),
        (RsaSha1Nsec3Sha1, "RSASHA1-NSEC3-SHA1", 7),
        (RsaSha256,        "RSASHA256",          8),

        (RsaSha512, "RSASHA512", 10),

        (EccGhost,        "ECC-GOST",        12),
        (EcdsaP256Sha256, "ECDSAP256SHA256", 13),
        (EcdsaP384Sha384, "ECDSAP384SHA384", 14),
        (Ed25519,         "ED25519",         15),
        (Ed448,           "ED448",           16),

        (Indirect,   "INDIRECT",   252),
        (PrivateDns, "PRIVATEDNS", 253),
        (PrivateOid, "PRIVATEOID", 254),
    ),
    from_str,
    DnsSecAlgorithmError,
    mnemonic_from_presentation,
    mnemonic_to_presentation,
    display_mnemonic
);

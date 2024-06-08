use std::{fmt::Display, error::Error};

use crate::gen_enum::enum_encoding;

#[derive(Debug)]
pub enum RTypeError<'a> {
    UnknownMnemonic(&'a str),
}
impl<'a> Error for RTypeError<'a> {}
impl<'a> Display for RTypeError<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownMnemonic(mnemonic) => write!(f, "unknown type mnemonic '{mnemonic}'"),
        }
    }
}

enum_encoding!(
    (doc "https://www.iana.org/assignments/dns-parameters/dns-parameters.xhtml#dns-parameters-4"),
    RType,
    u16,
    RTypeError,
    (
        (A,          "A",          1),
        (NS,         "NS",         2),
        (MD,         "MD",         3),
        (MF,         "MF",         4),
        (CNAME,      "CNAME",      5),
        (SOA,        "SOA",        6),
        (MB,         "MB",         7),
        (MG,         "MG",         8),
        (MR,         "MR",         9),
        (NULL,       "NULL",       10),
        (WKS,        "WKS",        11),
        (PTR,        "PTR",        12),
        (HINFO,      "HINFO",      13),
        (MINFO,      "MINFO",      14),
        (MX,         "MX",         15),
        (TXT,        "TXT",        16),
        (RP,         "RP",         17),
        (AFSDB,      "AFSDB",      18),
        (X25,        "X25",        19),
        (ISDN,       "ISDN",       20),
        (RT,         "RT",         21),
        (NSAP,       "NSAP",       22),
        (NSAP_PTR,   "NSAP-PTR",   23),
        (SIG,        "SIG",        24),
        (KEY,        "KEY",        25),
        (PX,         "PX",         26),
        (GPOS,       "GPOS",       27),
        (AAAA,       "AAAA",       28),
        (LOC,        "LOC",        29),
        (NXT,        "NXT",        30),
        (EID,        "EID",        31),
        (NIMLOC,     "NIMLOC",     32),
        (SRV,        "SRV",        33),
        (ATMA,       "ATMA",       34),
        (NAPTR,      "NAPTR",      35),
        (KX,         "KX",         36),
        (CERT,       "CERT",       37),
        (A6,         "A6",         38),
        (DNAME,      "DNAME",      39),
        (SINK,       "SINK",       40),
        (OPT,        "OPT",        41),
        (APL,        "APL",        42),
        (DS,         "DS",         43),
        (SSHFP,      "SSHFP",      44),
        (IPSECKEY,   "IPSECKEY",   45),
        (RRSIG,      "RRSIG",      46),
        (NSEC,       "NSEC",       47),
        (DNSKEY,     "DNSKEY",     48),
        (DHCID,      "DHCID",      49),
        (NSEC3,      "NSEC3",      50),
        (NSEC3PARAM, "NSEC3PARAM", 51),
        (TLSA,       "TLSA",       52),
        (SMIMEA,     "SMIMEA",     53),
    
        (HIP,        "HIP",        55),
        (NINFO,      "NINFO",      56),
        (RKEY,       "RKEY",       57),
        (TALINK,     "TALINK",     58),
        (CDS,        "CDS",        59),
        (CDNSKEY,    "CDNSKEY",    60),
        (OPENPGPKEY, "OPENPGPKEY", 61),
        (CSYNC,      "CSYNC",      62),
        (ZONEMD,     "ZONEMD",     63),
        (SVCB,       "SVCB",       64),
        (HTTPS,      "HTTPS",      65),
                
        (SPF,    "SPF",    99),
        (UINFO,  "UINFO",  100),
        (UID,    "UID",    101),
        (GID,    "GID",    102),
        (UNSPEC, "UNSPEC", 103),
        (NID,    "NID",    104),
        (L32,    "L32",    105),
        (L64,    "L64",    106),
        (LP,     "LP",     107),
        (EUI48,  "EUI48",  108),
        (EUI64,  "EUI64",  109),
    
        (TKEY,     "TKEY",     249),
        (TSIG,     "TSIG",     250),
        (IXFR,     "IXFR",     251),
        (AXFR,     "AXFR",     252),
        (MAILB,    "MAILB",    253),
        (MAILA,    "MAILA",    254),
        (ANY,      "*",        255), // *
        (URI,      "URI",      256),
        (CAA,      "CAA",      257),
        (AVC,      "AVC",      258),
        (DOA,      "DOA",      259),
        (AMTRELAY, "AMTRELAY", 260),
    
        (TA,  "TA",  32768),
        (DLV, "DLV", 32769),
    ),
    (wildcard_or_mnemonic_from_str, "TYPE"),
    mnemonic_presentation,
    mnemonic_display
);

pub trait RTypeCode {
    fn rtype(&self) -> RType;
}

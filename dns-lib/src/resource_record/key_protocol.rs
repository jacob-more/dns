use super::gen_enum::enum_encoding;

// https://datatracker.ietf.org/doc/html/rfc2535#section-3.1.3
enum_encoding!(
    KeyProtocol,
    u8,
    (
        (None,   "NONE",   0),
        (Tls,    "TLS",    1),
        (Email,  "EMAIL",  2),
        (DnsSec, "DNSSEC", 3),
        (IpSec,  "IPSEC",  4),
    
        (All, "ALL", 255),
    ),
    code_from_presentation,
    code_to_presentation,
    display_mnemonic
);

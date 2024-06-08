use crate::gen_enum::enum_encoding;

enum_encoding!(
    (doc "https://datatracker.ietf.org/doc/html/rfc2535#section-3.1.3"),
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
    code_presentation,
    mnemonic_display
);

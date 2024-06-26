use crate::gen_enum::enum_encoding;

enum_encoding!(
    IpSecAlgorithm,
    u8,
    (
        (
            (doc "https://datatracker.ietf.org/doc/html/rfc2536"),
            Dsa, "DSA", 1
        ),
        (
            (doc "https://datatracker.ietf.org/doc/html/rfc3110"),
            Rsa, "RSA", 2
        ),
    ),
    code_presentation,
    mnemonic_display
);

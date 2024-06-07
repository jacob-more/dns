use super::gen_enum::enum_encoding;

enum_encoding!(
    IpSecAlgorithm,
    u8,
    (
        // https://datatracker.ietf.org/doc/html/rfc2536
        (Dsa, "DSA", 1),
        // https://datatracker.ietf.org/doc/html/rfc3110
        (Rsa, "RSA", 2),
    ),
    code_from_presentation,
    code_to_presentation,
    display_mnemonic
);

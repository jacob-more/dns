use super::gen_enum::enum_encoding;

enum_encoding!(
    (doc "https://www.iana.org/assignments/ds-rr-types/ds-rr-types.xhtml#ds-rr-types-1"),
    DigestAlgorithm,
    u8,
    (
        (Sha1,        "SHA-1",           1),
        (Sha256,      "SHA-256",         2),
        (Gostr341194, "GOST R 34.11-94", 3),
        (Sha384,      "SHA-384",         4),
    ),
    code_from_presentation,
    code_to_presentation,
    display_mnemonic
);

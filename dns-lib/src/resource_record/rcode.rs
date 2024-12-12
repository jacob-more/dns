use crate::gen_enum::enum_encoding;

enum_encoding!(
    (doc "https://www.iana.org/assignments/dns-parameters/dns-parameters.xhtml#dns-parameters-6"),
    RCode,
    u16,
    (
        (NoError,   "NoError",   0),
        (FormErr,   "FormErr",   1),
        (ServFail,  "ServFail",  2),
        (NXDomain,  "NXDomain",  3),
        (NotImp,    "NotImp",    4),
        (Refused,   "Refused",   5),
        (YXDomain,  "YXDomain",  6),
        (YXRRSet,   "YXRRSet",   7),
        (NXRRSet,   "NXRRSet",   8),
        (NotAuth,   "NotAuth",   9),
        (NotZone,   "NotZone",   10),
        (DsoTypeNI, "DSOTYPENI", 11),

        (BadVers,   "BADVERS",   16),
        (BadSig,    "BADSIG",    16),
        (BadKey,    "BADKEY",    17),
        (BadTime,   "BADTIME",   18),
        (BadMode,   "BADMODE",   19),
        (BadName,   "BADNAME",   20),
        (BadAlg,    "BADALG",    21),
        (BadTrunc,  "BADTRUNC",  22),
        (BadCookie, "BADCOOKIE", 23),
    ),
    code_presentation,
    mnemonic_display
);

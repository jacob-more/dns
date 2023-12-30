use dns_macros::{ToWire, FromWire, RTypeCode};

use crate::{types::character_string::CharacterString, serde::{presentation::{from_tokenized_record::FromTokenizedRecord, from_presentation::FromPresentation}, wire::circular_test::gen_test_circular_serde_sanity_test}};

/// (Original) https://datatracker.ietf.org/doc/html/rfc1035#section-3.3.14
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, RTypeCode)]
pub struct TXT {
    strings: Vec<CharacterString>,
}

impl FromTokenizedRecord for TXT {
    #[inline]
    fn from_tokenized_record<'a>(record: &'a crate::serde::presentation::tokenizer::tokenizer::ResourceRecord) -> Result<Self, crate::serde::presentation::errors::TokenizedRecordError<'a>> where Self: Sized {
        match record.rdata.as_slice() {
            &[_, ..] => {
                let mut strings = Vec::with_capacity(record.rdata.len());
                for string_token in &record.rdata {
                    strings.push(CharacterString::from_token_format(&string_token)?);
                }
                Ok(Self { strings })
            },
            _ => Err(crate::serde::presentation::errors::TokenizedRecordError::TooFewRDataTokensError(1, record.rdata.len())),
        }
    }
}

gen_test_circular_serde_sanity_test!(
    record_single_string_circular_serde_sanity_test,
    TXT {
        strings: vec![
            CharacterString::from_utf8("This string is all alone.").unwrap(),
        ]
    }
);
gen_test_circular_serde_sanity_test!(
    record_two_string_circular_serde_sanity_test,
    TXT {
        strings: vec![
            CharacterString::from_utf8("This is a pretty cool string.").unwrap(),
            CharacterString::from_utf8("This string isn't as cool.").unwrap(),
        ]
    }
);

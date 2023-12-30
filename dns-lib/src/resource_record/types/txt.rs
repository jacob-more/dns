use dns_macros::{ToWire, FromWire, RTypeCode};

use crate::{types::character_string::CharacterString, serde::presentation::{from_tokenized_record::FromTokenizedRecord, from_presentation::FromPresentation}};

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

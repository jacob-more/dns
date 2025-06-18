use dns_macros::{FromWire, RData, ToWire};

use crate::{
    serde::presentation::{
        from_presentation::FromPresentation, from_tokenized_rdata::FromTokenizedRData,
        to_presentation::ToPresentation,
    },
    types::character_string::CharacterString,
};

/// (Original) https://datatracker.ietf.org/doc/html/rfc1035#section-3.3.14
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, RData)]
pub struct TXT {
    strings: Vec<CharacterString>,
}

impl TXT {
    #[inline]
    pub fn new(strings: Vec<CharacterString>) -> Self {
        Self { strings }
    }

    #[inline]
    pub fn strings(&self) -> &[CharacterString] {
        &self.strings
    }
}

impl FromTokenizedRData for TXT {
    #[inline]
    fn from_tokenized_rdata(
        rdata: &[&str],
    ) -> Result<Self, crate::serde::presentation::errors::TokenizedRecordError>
    where
        Self: Sized,
    {
        match rdata {
            [_, ..] => {
                let mut strings = Vec::with_capacity(rdata.len());
                for string_token in rdata {
                    strings.push(CharacterString::from_token_format(&[string_token])?.0);
                }
                Ok(Self { strings })
            }
            _ => Err(
                crate::serde::presentation::errors::TokenizedRecordError::TooFewRDataTokensError {
                    expected: 1,
                    received: rdata.len(),
                },
            ),
        }
    }
}

impl ToPresentation for TXT {
    #[inline]
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        for string in &self.strings {
            string.to_presentation_format(out_buffer);
        }
    }
}

#[cfg(test)]
mod circular_serde_sanity_test {
    use super::TXT;
    use crate::{
        serde::wire::circular_test::gen_test_circular_serde_sanity_test,
        types::character_string::CharacterString,
    };

    gen_test_circular_serde_sanity_test!(
        record_single_string_circular_serde_sanity_test,
        TXT {
            strings: vec![CharacterString::from_utf8("This string is all alone.").unwrap(),]
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
}

#[cfg(test)]
mod tokenizer_tests {
    use super::TXT;
    use crate::{
        serde::presentation::test_from_tokenized_rdata::{
            gen_fail_record_test, gen_ok_record_test,
        },
        types::character_string::CharacterString,
    };

    const GOOD_STRING: &str = "This is a string with some characters";

    gen_ok_record_test!(
        test_ok_one_string,
        TXT,
        TXT {
            strings: vec![CharacterString::from_utf8(GOOD_STRING).unwrap(),]
        },
        [GOOD_STRING]
    );
    gen_ok_record_test!(
        test_ok_two_string,
        TXT,
        TXT {
            strings: vec![
                CharacterString::from_utf8(GOOD_STRING).unwrap(),
                CharacterString::from_utf8(GOOD_STRING).unwrap(),
            ]
        },
        [GOOD_STRING, GOOD_STRING]
    );
    gen_ok_record_test!(
        test_ok_three_string,
        TXT,
        TXT {
            strings: vec![
                CharacterString::from_utf8(GOOD_STRING).unwrap(),
                CharacterString::from_utf8(GOOD_STRING).unwrap(),
                CharacterString::from_utf8(GOOD_STRING).unwrap(),
            ]
        },
        [GOOD_STRING, GOOD_STRING, GOOD_STRING]
    );
    gen_ok_record_test!(
        test_ok_four_string,
        TXT,
        TXT {
            strings: vec![
                CharacterString::from_utf8(GOOD_STRING).unwrap(),
                CharacterString::from_utf8(GOOD_STRING).unwrap(),
                CharacterString::from_utf8(GOOD_STRING).unwrap(),
                CharacterString::from_utf8(GOOD_STRING).unwrap(),
            ]
        },
        [GOOD_STRING, GOOD_STRING, GOOD_STRING, GOOD_STRING]
    );
    gen_ok_record_test!(
        test_ok_five_string,
        TXT,
        TXT {
            strings: vec![
                CharacterString::from_utf8(GOOD_STRING).unwrap(),
                CharacterString::from_utf8(GOOD_STRING).unwrap(),
                CharacterString::from_utf8(GOOD_STRING).unwrap(),
                CharacterString::from_utf8(GOOD_STRING).unwrap(),
                CharacterString::from_utf8(GOOD_STRING).unwrap(),
            ]
        },
        [
            GOOD_STRING,
            GOOD_STRING,
            GOOD_STRING,
            GOOD_STRING,
            GOOD_STRING
        ]
    );
    gen_fail_record_test!(test_fail_no_tokens, TXT, []);
}

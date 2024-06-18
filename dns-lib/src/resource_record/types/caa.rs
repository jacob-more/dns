use std::{error::Error, fmt::Display};

use dns_macros::RTypeCode;

use crate::{serde::{presentation::{errors::TokenizedRecordError, from_presentation::FromPresentation, from_tokenized_rdata::FromTokenizedRData, to_presentation::ToPresentation}, wire::{from_wire::FromWire, read_wire::ReadWireError, to_wire::ToWire}}, types::ascii::{AsciiChar, AsciiString}};

#[derive(Debug)]
pub enum CAAError {
    TagLengthTooSmall(usize),
    TagLengthTooLarge(usize),
    IllegalTagValue,
}
impl Error for CAAError {}
impl Display for CAAError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TagLengthTooSmall(length) => write!(f, "Tag Length Too Small: the tag must contain at least 1 byte. Found {length}"),
            Self::TagLengthTooLarge(length) => write!(f, "Tag Length Too Large: the tag must contain at most 255 bytes. Found {length}"),
            Self::IllegalTagValue => write!(f, "Illegal Tag Value: the tag can only contain alphanumeric ascii characters (a-z, A-Z, 0-9)"),
        }
    }
}

/// (Original) https://datatracker.ietf.org/doc/html/rfc1035#section-3.2.3
/// (Updated)  https://datatracker.ietf.org/doc/html/rfc5936#section-2
#[derive(Clone, PartialEq, Eq, Hash, Debug, RTypeCode)]
pub struct CAA {
    flags: u8,
    tag: AsciiString,
    value: Vec<AsciiChar>,
}

impl CAA {
    #[inline]
    pub fn new(flags: u8, tag: AsciiString, value: Vec<u8>) -> Result<Self, CAAError> {
        match tag.len() {
            tag_len @ 0 => return Err(CAAError::TagLengthTooSmall(tag_len)),
            tag_len @ 256.. => return Err(CAAError::TagLengthTooLarge(tag_len)),
            _ => (),
        };
        if !tag.is_alphanumeric() {
            return Err(CAAError::IllegalTagValue);
        }

        Ok(Self { flags, tag, value })
    }

    #[inline]
    pub fn flags(&self) -> u8 {
        self.flags
    }

    #[inline]
    pub fn issuer_critical_flag(&self) -> bool {
        (self.flags & 0b10000000) == 0b10000000
    }

    #[inline]
    pub fn tag_length(&self) -> u8 {
        self.tag.len() as u8
    }

    #[inline]
    pub fn tag(&self) -> &AsciiString {
        &self.tag
    }

    #[inline]
    pub fn value_length(&self) -> u16 {
        self.value.len() as u16
    }

    #[inline]
    pub fn value(&self) -> &Vec<AsciiChar> {
        &self.value
    }
}

impl ToWire for CAA {
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>, compression: &mut Option<crate::serde::wire::compression_map::CompressionMap>) -> Result<(), crate::serde::wire::write_wire::WriteWireError> where 'a: 'b {
        self.flags.to_wire_format(wire, compression)?;
        (self.tag.len() as u8).to_wire_format(wire, compression)?;
        wire.write_bytes(self.tag.as_slice())?;
        wire.write_bytes(&self.value)?;

        Ok(())
    }

    fn serial_length(&self) -> u16 {
        self.flags.serial_length()
        + (self.tag.len() as u8).serial_length()
        + self.tag.serial_length()
        + self.value.serial_length()
    }
}

impl FromWire for CAA {
    fn from_wire_format<'a, 'b>(wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>) -> Result<Self, crate::serde::wire::read_wire::ReadWireError> where Self: Sized, 'a: 'b {
        let flags = u8::from_wire_format(wire)?;
        let tag_length = u8::from_wire_format(wire)?;
        if tag_length < 1 {
            return Err(ReadWireError::ValueError(format!("Expected CAA tag length to be at least 1. it was {tag_length}")));
        }

        let tag = AsciiString::from_wire_format(
            &mut wire.take_as_read_wire(tag_length as usize)?
        )?;
        if !tag.is_alphanumeric() {
            // Note that the characters must only be lowercase if reading from presentation format.
            return Err(ReadWireError::ValueError("Expected CAA tag to contain only ASCII characters a-z, A-Z, and 0-9".to_string()));
        }

        // the rest of the rdata is the value portion.
        let value = wire.current().to_vec();
        wire.shift(value.len())?;

        Ok(Self { flags, tag, value })
    }
}

impl FromTokenizedRData for CAA {
    #[inline]
    fn from_tokenized_rdata<'a, 'b>(rdata: &Vec<&'a str>) -> Result<Self, TokenizedRecordError<'b>> where Self: Sized, 'a: 'b {
        match rdata.as_slice() {
            &[flags, tag, value] => {
                // Flags must be between 0 and 255. This is enforced by the type, u8.
                let (flags, _) = u8::from_token_format(&[flags])?;

                let (tag, _) = AsciiString::from_token_format(&[tag])?;
                match tag.len() {
                    tag_length @ 0 => return Err(TokenizedRecordError::ValueError(format!("Expected CAA tag length to be at least 1. it was {}", tag_length))),
                    tag_length @ 256.. => return Err(TokenizedRecordError::ValueError(format!("Expected CAA tag length to be at most 255. it was {}", tag_length))),
                    _ => (),
                }
                if !tag.is_lower_alphanumeric() {
                    return Err(TokenizedRecordError::ValueError("Expected CAA tag to contain only ASCII characters a-z (lowercase only) and 0-9".to_string()));
                }

                let value = AsciiString::from_token_format(&[value])?.0.as_owned_vec();

                Ok(Self { flags, tag, value })
            },
            &[_, _, _, ..] => Err(TokenizedRecordError::TooManyRDataTokensError{expected: 3, received: rdata.len()}),
            _ => Err(TokenizedRecordError::TooFewRDataTokensError{expected: 3, received: rdata.len()}),
        }
    }
}

impl ToPresentation for CAA {
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        self.flags.to_presentation_format(out_buffer);
        self.tag.to_presentation_format(out_buffer);
        // TODO: verify that the resulting output gets put in quotations if it has spaces.
        //       I am not sure if it is valid for the string to contain spaces if the space is
        //       escaped with an escape character.
        AsciiString::from(&self.value).to_presentation_format(out_buffer);
    }
}

#[cfg(test)]
mod circular_serde_sanity_test {
    use std::iter::repeat;

    use lazy_static::lazy_static;

    use crate::{serde::wire::circular_test::gen_test_circular_serde_sanity_test, types::ascii::AsciiString};
    use super::CAA;

    const NO_FLAGS: u8 = 0b00000000;
    const ISSUER_CRITICAL_FLAG: u8 = 0b10000000;
    const UNKNOWN_FLAG: u8 = 0b01000000;

    lazy_static!(
        static ref OK_TAG: AsciiString = AsciiString::from_utf8("issue123ISSUE").unwrap();
        static ref FAIL_TAG_EMPTY: AsciiString = AsciiString::from_utf8("").unwrap();
        static ref FAIL_TAG_TOO_LONG: AsciiString = AsciiString::from_utf8(&repeat('a').take(256).collect::<String>()).unwrap();
        static ref FAIL_TAG_NON_ALPHANUMERIC: AsciiString = AsciiString::from_utf8("has a space").unwrap();
    );

    lazy_static!(
        static ref OK_VALUE: Vec<u8> = AsciiString::from_utf8("shortvalue123SHORTVALUE").unwrap().as_owned_vec();
        static ref OK_VALUE_EMPTY: Vec<u8> = AsciiString::from_utf8("").unwrap().as_owned_vec();
        static ref OK_VALUE_LONGER_THAN_255: Vec<u8> = AsciiString::from_utf8(&repeat('a').take(256).collect::<String>()).unwrap().as_owned_vec();
        static ref OK_VALUE_NON_ALPHANUMERIC: Vec<u8> = AsciiString::from_utf8("has a space").unwrap().as_owned_vec();
    );

    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test_no_flag_ok_tag_ok_value,
        CAA { flags: NO_FLAGS, tag: OK_TAG.clone(), value: OK_VALUE.clone() }
    );

    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test_no_flag_ok_tag_ok_value_empty,
        CAA { flags: NO_FLAGS, tag: OK_TAG.clone(), value: OK_VALUE_EMPTY.clone() }
    );

    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test_no_flag_ok_tag_ok_value_longer_than_255,
        CAA { flags: NO_FLAGS, tag: OK_TAG.clone(), value: OK_VALUE_LONGER_THAN_255.clone() }
    );

    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test_no_flag_ok_tag_ok_value_non_alpha_numeric,
        CAA { flags: NO_FLAGS, tag: OK_TAG.clone(), value: OK_VALUE_NON_ALPHANUMERIC.clone() }
    );

    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test_ic_flag_ok_tag_ok_value,
        CAA { flags: ISSUER_CRITICAL_FLAG, tag: OK_TAG.clone(), value: OK_VALUE.clone() }
    );

    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test_ic_flag_ok_tag_ok_value_empty,
        CAA { flags: ISSUER_CRITICAL_FLAG, tag: OK_TAG.clone(), value: OK_VALUE_EMPTY.clone() }
    );

    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test_ic_flag_ok_tag_ok_value_longer_than_255,
        CAA { flags: ISSUER_CRITICAL_FLAG, tag: OK_TAG.clone(), value: OK_VALUE_LONGER_THAN_255.clone() }
    );

    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test_ic_flag_ok_tag_ok_value_non_alpha_numeric,
        CAA { flags: ISSUER_CRITICAL_FLAG, tag: OK_TAG.clone(), value: OK_VALUE_NON_ALPHANUMERIC.clone() }
    );

    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test_unknown_flag_ok_tag_ok_value,
        CAA { flags: UNKNOWN_FLAG, tag: OK_TAG.clone(), value: OK_VALUE.clone() }
    );

    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test_unknown_flag_ok_tag_ok_value_empty,
        CAA { flags: UNKNOWN_FLAG, tag: OK_TAG.clone(), value: OK_VALUE_EMPTY.clone() }
    );

    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test_unknown_flag_ok_tag_ok_value_longer_than_255,
        CAA { flags: UNKNOWN_FLAG, tag: OK_TAG.clone(), value: OK_VALUE_LONGER_THAN_255.clone() }
    );

    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test_unknown_flag_ok_tag_ok_value_non_alpha_numeric,
        CAA { flags: UNKNOWN_FLAG, tag: OK_TAG.clone(), value: OK_VALUE_NON_ALPHANUMERIC.clone() }
    );
}

#[cfg(test)]
mod tokenizer_tests {
    use std::iter::repeat;

    use lazy_static::lazy_static;

    use crate::{serde::presentation::test_from_tokenized_rdata::{gen_fail_record_test, gen_ok_record_test}, types::ascii::AsciiString};
    use super::CAA;

    const NO_FLAGS: u8 = 0b00000000;
    const ISSUER_CRITICAL_FLAG: u8 = 0b10000000;
    const UNKNOWN_FLAG: u8 = 0b01000000;

    const STR_NO_FLAGS: &'static str = "0";
    const STR_ISSUER_CRITICAL_FLAG: &'static str = "128";
    const STR_UNKNOWN_FLAG: &'static str = "64";

    lazy_static!(
        static ref STR_OK_TAG: &'static str = "issue123";
        static ref STR_FAIL_TAG_EMPTY: &'static str = "";
        static ref STR_FAIL_TAG_TOO_LONG: String = repeat('a').take(256).collect::<String>();
        static ref STR_FAIL_TAG_NON_ALPHANUMERIC: &'static str = "has a space";

        static ref OK_TAG: AsciiString = AsciiString::from_utf8(&STR_OK_TAG).unwrap();
    );

    lazy_static!(
        static ref STR_OK_VALUE: &'static str = "shortvalue123SHORTVALUE";
        static ref STR_OK_VALUE_EMPTY: &'static str = "";
        static ref STR_OK_VALUE_LONGER_THAN_255: String = repeat('a').take(256).collect::<String>();
        static ref STR_OK_VALUE_NON_ALPHANUMERIC: &'static str = "has a space";

        static ref OK_VALUE: Vec<u8> = AsciiString::from_utf8(&STR_OK_VALUE).unwrap().as_owned_vec();
        static ref OK_VALUE_EMPTY: Vec<u8> = AsciiString::from_utf8(&STR_OK_VALUE_EMPTY).unwrap().as_owned_vec();
        static ref OK_VALUE_LONGER_THAN_255: Vec<u8> = AsciiString::from_utf8(&STR_OK_VALUE_LONGER_THAN_255).unwrap().as_owned_vec();
        static ref OK_VALUE_NON_ALPHANUMERIC: Vec<u8> = AsciiString::from_utf8(&STR_OK_VALUE_NON_ALPHANUMERIC).unwrap().as_owned_vec();
    );

    // VARIOUS FLAGS, VARIOUS TAGS, OK VALUE

    gen_ok_record_test!(test_ok_no_flags_ok_tag_ok_value, CAA, CAA { flags: NO_FLAGS, tag: OK_TAG.clone(), value: OK_VALUE.clone() }, [STR_NO_FLAGS, &STR_OK_TAG, &STR_OK_VALUE]);
    gen_ok_record_test!(test_ok_ic_flag_ok_tag_ok_value, CAA, CAA { flags: ISSUER_CRITICAL_FLAG, tag: OK_TAG.clone(), value: OK_VALUE.clone() }, [STR_ISSUER_CRITICAL_FLAG, &STR_OK_TAG, &STR_OK_VALUE]);
    gen_ok_record_test!(test_ok_unknown_flags_ok_tag_ok_value, CAA, CAA { flags: UNKNOWN_FLAG, tag: OK_TAG.clone(), value: OK_VALUE.clone() }, [STR_UNKNOWN_FLAG, &STR_OK_TAG, &STR_OK_VALUE]);

    gen_fail_record_test!(test_fail_no_flags_fail_tag_empty_ok_value, CAA, [STR_NO_FLAGS, &STR_FAIL_TAG_EMPTY, &STR_OK_VALUE]);
    gen_fail_record_test!(test_fail_ic_flag_fail_tag_empty_ok_value, CAA, [STR_ISSUER_CRITICAL_FLAG, &STR_FAIL_TAG_EMPTY, &STR_OK_VALUE]);
    gen_fail_record_test!(test_fail_unknown_flags_fail_tag_empty_ok_value, CAA, [STR_UNKNOWN_FLAG, &STR_FAIL_TAG_EMPTY, &STR_OK_VALUE]);

    gen_fail_record_test!(test_fail_no_flags_fail_tag_too_long_ok_value, CAA, [STR_NO_FLAGS, &STR_FAIL_TAG_TOO_LONG, &STR_OK_VALUE]);
    gen_fail_record_test!(test_fail_ic_flag_fail_tag_too_long_ok_value, CAA, [STR_ISSUER_CRITICAL_FLAG, &STR_FAIL_TAG_TOO_LONG, &STR_OK_VALUE]);
    gen_fail_record_test!(test_fail_unknown_flags_fail_tag_too_long_ok_value, CAA, [STR_UNKNOWN_FLAG, &STR_FAIL_TAG_TOO_LONG, &STR_OK_VALUE]);

    gen_fail_record_test!(test_fail_no_flags_fail_tag_non_alphanumeric_ok_value, CAA, [STR_NO_FLAGS, &STR_FAIL_TAG_NON_ALPHANUMERIC, &STR_OK_VALUE]);
    gen_fail_record_test!(test_fail_ic_flag_fail_tag_non_alphanumeric_ok_value, CAA, [STR_ISSUER_CRITICAL_FLAG, &STR_FAIL_TAG_NON_ALPHANUMERIC, &STR_OK_VALUE]);
    gen_fail_record_test!(test_fail_unknown_flags_fail_tag_non_alphanumeric_ok_value, CAA, [STR_UNKNOWN_FLAG, &STR_FAIL_TAG_NON_ALPHANUMERIC, &STR_OK_VALUE]);

    // VARIOUS FLAGS, VARIOUS TAGS, OK VALUE EMPTY

    gen_ok_record_test!(test_ok_no_flags_ok_tag_ok_value_empty, CAA, CAA { flags: NO_FLAGS, tag: OK_TAG.clone(), value: OK_VALUE_EMPTY.clone() }, [STR_NO_FLAGS, &STR_OK_TAG, &STR_OK_VALUE_EMPTY]);
    gen_ok_record_test!(test_ok_ic_flag_ok_tag_ok_value_empty, CAA, CAA { flags: ISSUER_CRITICAL_FLAG, tag: OK_TAG.clone(), value: OK_VALUE_EMPTY.clone() }, [STR_ISSUER_CRITICAL_FLAG, &STR_OK_TAG, &STR_OK_VALUE_EMPTY]);
    gen_ok_record_test!(test_ok_unknown_flags_ok_tag_ok_value_empty, CAA, CAA { flags: UNKNOWN_FLAG, tag: OK_TAG.clone(), value: OK_VALUE_EMPTY.clone() }, [STR_UNKNOWN_FLAG, &STR_OK_TAG, &STR_OK_VALUE_EMPTY]);

    gen_fail_record_test!(test_fail_no_flags_fail_tag_empty_ok_value_empty, CAA, [STR_NO_FLAGS, &STR_FAIL_TAG_EMPTY, &STR_OK_VALUE_EMPTY]);
    gen_fail_record_test!(test_fail_ic_flag_fail_tag_empty_ok_value_empty, CAA, [STR_ISSUER_CRITICAL_FLAG, &STR_FAIL_TAG_EMPTY, &STR_OK_VALUE_EMPTY]);
    gen_fail_record_test!(test_fail_unknown_flags_fail_tag_empty_ok_value_empty, CAA, [STR_UNKNOWN_FLAG, &STR_FAIL_TAG_EMPTY, &STR_OK_VALUE_EMPTY]);

    gen_fail_record_test!(test_fail_no_flags_fail_tag_too_long_ok_value_empty, CAA, [STR_NO_FLAGS, &STR_FAIL_TAG_TOO_LONG, &STR_OK_VALUE_EMPTY]);
    gen_fail_record_test!(test_fail_ic_flag_fail_tag_too_long_ok_value_empty, CAA, [STR_ISSUER_CRITICAL_FLAG, &STR_FAIL_TAG_TOO_LONG, &STR_OK_VALUE_EMPTY]);
    gen_fail_record_test!(test_fail_unknown_flags_fail_tag_too_long_ok_value_empty, CAA, [STR_UNKNOWN_FLAG, &STR_FAIL_TAG_TOO_LONG, &STR_OK_VALUE_EMPTY]);

    gen_fail_record_test!(test_fail_no_flags_fail_tag_non_alphanumeric_ok_value_empty, CAA, [STR_NO_FLAGS, &STR_FAIL_TAG_NON_ALPHANUMERIC, &STR_OK_VALUE_EMPTY]);
    gen_fail_record_test!(test_fail_ic_flag_fail_tag_non_alphanumeric_ok_value_empty, CAA, [STR_ISSUER_CRITICAL_FLAG, &STR_FAIL_TAG_NON_ALPHANUMERIC, &STR_OK_VALUE_EMPTY]);
    gen_fail_record_test!(test_fail_unknown_flags_fail_tag_non_alphanumeric_ok_value_empty, CAA, [STR_UNKNOWN_FLAG, &STR_FAIL_TAG_NON_ALPHANUMERIC, &STR_OK_VALUE_EMPTY]);

    // VARIOUS FLAGS, VARIOUS TAGS, OK VALUE LONGER THAN 255

    gen_ok_record_test!(test_ok_no_flags_ok_tag_ok_value_longer_than_255, CAA, CAA { flags: NO_FLAGS, tag: OK_TAG.clone(), value: OK_VALUE_LONGER_THAN_255.clone() }, [STR_NO_FLAGS, &STR_OK_TAG, &STR_OK_VALUE_LONGER_THAN_255]);
    gen_ok_record_test!(test_ok_ic_flag_ok_tag_ok_value_longer_than_255, CAA, CAA { flags: ISSUER_CRITICAL_FLAG, tag: OK_TAG.clone(), value: OK_VALUE_LONGER_THAN_255.clone() }, [STR_ISSUER_CRITICAL_FLAG, &STR_OK_TAG, &STR_OK_VALUE_LONGER_THAN_255]);
    gen_ok_record_test!(test_ok_unknown_flags_ok_tag_ok_value_longer_than_255, CAA, CAA { flags: UNKNOWN_FLAG, tag: OK_TAG.clone(), value: OK_VALUE_LONGER_THAN_255.clone() }, [STR_UNKNOWN_FLAG, &STR_OK_TAG, &STR_OK_VALUE_LONGER_THAN_255]);

    gen_fail_record_test!(test_fail_no_flags_fail_tag_empty_ok_value_longer_than_255, CAA, [STR_NO_FLAGS, &STR_FAIL_TAG_EMPTY, &STR_OK_VALUE_LONGER_THAN_255]);
    gen_fail_record_test!(test_fail_ic_flag_fail_tag_empty_ok_value_longer_than_255, CAA, [STR_ISSUER_CRITICAL_FLAG, &STR_FAIL_TAG_EMPTY, &STR_OK_VALUE_LONGER_THAN_255]);
    gen_fail_record_test!(test_fail_unknown_flags_fail_tag_empty_ok_value_longer_than_255, CAA, [STR_UNKNOWN_FLAG, &STR_FAIL_TAG_EMPTY, &STR_OK_VALUE_LONGER_THAN_255]);

    gen_fail_record_test!(test_fail_no_flags_fail_tag_too_long_ok_value_longer_than_255, CAA, [STR_NO_FLAGS, &STR_FAIL_TAG_TOO_LONG, &STR_OK_VALUE_LONGER_THAN_255]);
    gen_fail_record_test!(test_fail_ic_flag_fail_tag_too_long_ok_value_longer_than_255, CAA, [STR_ISSUER_CRITICAL_FLAG, &STR_FAIL_TAG_TOO_LONG, &STR_OK_VALUE_LONGER_THAN_255]);
    gen_fail_record_test!(test_fail_unknown_flags_fail_tag_too_long_ok_value_longer_than_255, CAA, [STR_UNKNOWN_FLAG, &STR_FAIL_TAG_TOO_LONG, &STR_OK_VALUE_LONGER_THAN_255]);

    gen_fail_record_test!(test_fail_no_flags_fail_tag_non_alphanumeric_ok_value_longer_than_255, CAA, [STR_NO_FLAGS, &STR_FAIL_TAG_NON_ALPHANUMERIC, &STR_OK_VALUE_LONGER_THAN_255]);
    gen_fail_record_test!(test_fail_ic_flag_fail_tag_non_alphanumeric_ok_value_longer_than_255, CAA, [STR_ISSUER_CRITICAL_FLAG, &STR_FAIL_TAG_NON_ALPHANUMERIC, &STR_OK_VALUE_LONGER_THAN_255]);
    gen_fail_record_test!(test_fail_unknown_flags_fail_tag_non_alphanumeric_ok_value_longer_than_255, CAA, [STR_UNKNOWN_FLAG, &STR_FAIL_TAG_NON_ALPHANUMERIC, &STR_OK_VALUE_LONGER_THAN_255]);

    // VARIOUS FLAGS, VARIOUS TAGS, OK VALUE NON ALPHANUMERIC

    gen_ok_record_test!(test_ok_no_flags_ok_tag_ok_value_non_alphanumeric, CAA, CAA { flags: NO_FLAGS, tag: OK_TAG.clone(), value: OK_VALUE_NON_ALPHANUMERIC.clone() }, [STR_NO_FLAGS, &STR_OK_TAG, &STR_OK_VALUE_NON_ALPHANUMERIC]);
    gen_ok_record_test!(test_ok_ic_flag_ok_tag_ok_value_non_alphanumeric, CAA, CAA { flags: ISSUER_CRITICAL_FLAG, tag: OK_TAG.clone(), value: OK_VALUE_NON_ALPHANUMERIC.clone() }, [STR_ISSUER_CRITICAL_FLAG, &STR_OK_TAG, &STR_OK_VALUE_NON_ALPHANUMERIC]);
    gen_ok_record_test!(test_ok_unknown_flags_ok_tag_ok_value_non_alphanumeric, CAA, CAA { flags: UNKNOWN_FLAG, tag: OK_TAG.clone(), value: OK_VALUE_NON_ALPHANUMERIC.clone() }, [STR_UNKNOWN_FLAG, &STR_OK_TAG, &STR_OK_VALUE_NON_ALPHANUMERIC]);

    gen_fail_record_test!(test_fail_no_flags_fail_tag_empty_ok_value_non_alphanumeric, CAA, [STR_NO_FLAGS, &STR_FAIL_TAG_EMPTY, &STR_OK_VALUE_NON_ALPHANUMERIC]);
    gen_fail_record_test!(test_fail_ic_flag_fail_tag_empty_ok_value_non_alphanumeric, CAA, [STR_ISSUER_CRITICAL_FLAG, &STR_FAIL_TAG_EMPTY, &STR_OK_VALUE_NON_ALPHANUMERIC]);
    gen_fail_record_test!(test_fail_unknown_flags_fail_tag_empty_ok_value_non_alphanumeric, CAA, [STR_UNKNOWN_FLAG, &STR_FAIL_TAG_EMPTY, &STR_OK_VALUE_NON_ALPHANUMERIC]);

    gen_fail_record_test!(test_fail_no_flags_fail_tag_too_long_ok_value_non_alphanumeric, CAA, [STR_NO_FLAGS, &STR_FAIL_TAG_TOO_LONG, &STR_OK_VALUE_NON_ALPHANUMERIC]);
    gen_fail_record_test!(test_fail_ic_flag_fail_tag_too_long_ok_value_non_alphanumeric, CAA, [STR_ISSUER_CRITICAL_FLAG, &STR_FAIL_TAG_TOO_LONG, &STR_OK_VALUE_NON_ALPHANUMERIC]);
    gen_fail_record_test!(test_fail_unknown_flags_fail_tag_too_long_ok_value_non_alphanumeric, CAA, [STR_UNKNOWN_FLAG, &STR_FAIL_TAG_TOO_LONG, &STR_OK_VALUE_NON_ALPHANUMERIC]);

    gen_fail_record_test!(test_fail_no_flags_fail_tag_non_alphanumeric_ok_value_non_alphanumeric, CAA, [STR_NO_FLAGS, &STR_FAIL_TAG_NON_ALPHANUMERIC, &STR_OK_VALUE_NON_ALPHANUMERIC]);
    gen_fail_record_test!(test_fail_ic_flag_fail_tag_non_alphanumeric_ok_value_non_alphanumeric, CAA, [STR_ISSUER_CRITICAL_FLAG, &STR_FAIL_TAG_NON_ALPHANUMERIC, &STR_OK_VALUE_NON_ALPHANUMERIC]);
    gen_fail_record_test!(test_fail_unknown_flags_fail_tag_non_alphanumeric_ok_value_non_alphanumeric, CAA, [STR_UNKNOWN_FLAG, &STR_FAIL_TAG_NON_ALPHANUMERIC, &STR_OK_VALUE_NON_ALPHANUMERIC]);
}

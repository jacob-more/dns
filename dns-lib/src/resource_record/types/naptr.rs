use dns_macros::{ToWire, RTypeCode, ToPresentation};

use crate::{serde::{presentation::{errors::{TokenError, TokenizedRecordError}, from_presentation::FromPresentation, from_tokenized_rdata::FromTokenizedRData}, wire::{from_wire::FromWire, read_wire::ReadWireError}}, types::{c_domain_name::{CDomainNameError, Labels}, character_string::CharacterString, domain_name::{DomainName, DomainNameError}}};

/// (Original) https://datatracker.ietf.org/doc/html/rfc3403#section-4
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, ToPresentation, RTypeCode)]
pub struct NAPTR {
    order: u16,
    preference: u16,
    flags: CharacterString,
    service: CharacterString,
    regexp: CharacterString,
    replacement: DomainName,
}

impl NAPTR {
    #[inline]
    pub fn new(order: u16, preference: u16, flags: CharacterString, service: CharacterString, regexp: CharacterString, replacement: DomainName) -> Self {
        Self { order, preference, flags, service, regexp, replacement }
    }

    #[inline]
    pub fn order(&self) -> u16 { self.order }

    #[inline]
    pub fn preference(&self) -> u16 { self.preference }

    #[inline]
    pub fn flags(&self) -> &CharacterString { &self.flags }

    #[inline]
    pub fn service(&self) -> &CharacterString { &self.service }

    #[inline]
    pub fn regexp(&self) -> &CharacterString { &self.regexp }

    #[inline]
    pub fn replacement(&self) -> &DomainName { &self.replacement }

}

impl FromWire for NAPTR {
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>) -> Result<Self, crate::serde::wire::read_wire::ReadWireError> where Self: Sized, 'a: 'b {
        let order = u16::from_wire_format(wire)?;
        let preference = u16::from_wire_format(wire)?;

        let flags = CharacterString::from_wire_format(wire)?;
        if !flags.is_alphanumeric_or_empty() {
            return Err(ReadWireError::FormatError(format!("The 'flags' field is required to be alphanumeric")));
        }

        let service = CharacterString::from_wire_format(wire)?;
        let regexp = CharacterString::from_wire_format(wire)?;

        let replacement = DomainName::from_wire_format(wire)?;
        if !replacement.is_fully_qualified() {
            return Err(ReadWireError::DomainNameError(DomainNameError::CDomainNameError(CDomainNameError::Fqdn)));
        }

        if (regexp.len() > 0) && (!replacement.is_root()) {
            return Err(ReadWireError::FormatError(format!("The 'regexp' and 'replacement' fields are mutually exclusive but both were non-empty")));
        }

        Ok(Self { order, preference, flags, service, regexp, replacement })
    }
}

impl FromTokenizedRData for NAPTR {
    #[inline]
    fn from_tokenized_rdata<'a, 'b>(record: &Vec<&'a str>) -> Result<Self, crate::serde::presentation::errors::TokenizedRecordError<'b>> where Self: Sized, 'a: 'b {
        match record.as_slice() {
            &[order, preference, flags, service, replacement, regexp] => {
                let (order, _) = u16::from_token_format(&[order])?;
                let (preference, _) = u16::from_token_format(&[preference])?;

                let (flags, _) = CharacterString::from_token_format(&[flags])?;
                if !flags.is_alphanumeric_or_empty() {
                    return Err(TokenizedRecordError::ValueError(format!("The 'flags' field is required to be alphanumeric")));
                }

                let (service, _) = CharacterString::from_token_format(&[service])?;
                let (regexp, _) = CharacterString::from_token_format(&[regexp])?;

                let (replacement, _) = DomainName::from_token_format(&[replacement])?;
                if !replacement.is_fully_qualified() {
                    return Err(TokenizedRecordError::TokenError(TokenError::DomainNameError(DomainNameError::CDomainNameError(CDomainNameError::Fqdn))));
                }

                if (regexp.len() > 0) && (!replacement.is_root()) {
                    return Err(TokenizedRecordError::ValueError(format!("The 'regexp' and 'replacement' fields are mutually exclusive but both were non-empty")));
                }

                Ok(Self { order, preference, flags, service, regexp, replacement })
            },
            &[_, _, _, _, _, _, ..] => Err(TokenizedRecordError::TooManyRDataTokensError { expected: 6, received: record.len() }),
            _ => Err(TokenizedRecordError::TooFewRDataTokensError { expected: 6, received: record.len() }),
        }
    }
}

#[cfg(test)]
mod circular_serde_sanity_test {
    use crate::{serde::wire::circular_test::gen_test_circular_serde_sanity_test, types::{character_string::CharacterString, domain_name::DomainName}};

    use super::NAPTR;

    gen_test_circular_serde_sanity_test!(
        rfc_3403_example_1_record_circular_serde_sanity_test,
        NAPTR {
            order: 100,
            preference: 50,
            flags: CharacterString::from_utf8("").unwrap(),
            service: CharacterString::from_utf8("").unwrap(),
            regexp: CharacterString::from_utf8(r"!^urn:cid:.+@([^\.]+\.)(.*)$!\2!i").unwrap(),
            replacement: DomainName::from_utf8(".").unwrap()
        }
    );

    gen_test_circular_serde_sanity_test!(
        rfc_3403_example_2_record_circular_serde_sanity_test,
        NAPTR {
            order: 100,
            preference: 50,
            flags: CharacterString::from_utf8("a").unwrap(),
            service: CharacterString::from_utf8("z3950+N2L+N2C").unwrap(),
            regexp: CharacterString::from_utf8("").unwrap(),
            replacement: DomainName::from_utf8("cidserver.example.com.").unwrap()
        }
    );

    gen_test_circular_serde_sanity_test!(
        rfc_3403_example_3_record_circular_serde_sanity_test,
        NAPTR {
            order: 100,
            preference: 50,
            flags: CharacterString::from_utf8("a").unwrap(),
            service: CharacterString::from_utf8("rcds+N2C").unwrap(),
            regexp: CharacterString::from_utf8("").unwrap(),
            replacement: DomainName::from_utf8("cidserver.example.com.").unwrap()
        }
    );

    gen_test_circular_serde_sanity_test!(
        rfc_3403_example_4_record_circular_serde_sanity_test,
        NAPTR {
            order: 100,
            preference: 50,
            flags: CharacterString::from_utf8("s").unwrap(),
            service: CharacterString::from_utf8("http+N2L+N2C+N2R").unwrap(),
            regexp: CharacterString::from_utf8("").unwrap(),
            replacement: DomainName::from_utf8("www.example.com.").unwrap()
        }
    );

    gen_test_circular_serde_sanity_test!(
        rfc_3403_example_5_record_circular_serde_sanity_test,
        NAPTR {
            order: 100,
            preference: 10,
            flags: CharacterString::from_utf8("u").unwrap(),
            service: CharacterString::from_utf8("sip+E2U").unwrap(),
            regexp: CharacterString::from_utf8(r"!^.*$!sip:information@foo.se!i").unwrap(),
            replacement: DomainName::from_utf8(".").unwrap()
        }
    );

    gen_test_circular_serde_sanity_test!(
        rfc_3403_example_6_record_circular_serde_sanity_test,
        NAPTR {
            order: 102,
            preference: 10,
            flags: CharacterString::from_utf8("u").unwrap(),
            service: CharacterString::from_utf8("smtp+E2U").unwrap(),
            regexp: CharacterString::from_utf8(r"!^.*$!mailto:information@foo.se!i").unwrap(),
            replacement: DomainName::from_utf8(".").unwrap()
        }
    );
}

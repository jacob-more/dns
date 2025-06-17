use dns_macros::{FromWire, RData, ToPresentation, ToWire};

use crate::{
    serde::presentation::{
        errors::TokenizedRecordError, from_presentation::FromPresentation,
        from_tokenized_rdata::FromTokenizedRData,
    },
    types::{domain_name::DomainName, rtype_bitmap::RTypeBitmap},
};

/// (Original) https://datatracker.ietf.org/doc/html/rfc4034#section-3
/// (Update) https://datatracker.ietf.org/doc/html/rfc3225
/// (Update) https://datatracker.ietf.org/doc/html/rfc6840
/// (Update) https://datatracker.ietf.org/doc/html/rfc6944
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, ToPresentation, RData)]
pub struct NSEC {
    next_domain_name: DomainName,
    type_bit_map: RTypeBitmap,
}

impl FromTokenizedRData for NSEC {
    fn from_tokenized_rdata(
        rdata: &Vec<&str>,
    ) -> Result<Self, crate::serde::presentation::errors::TokenizedRecordError>
    where
        Self: Sized,
    {
        match rdata.as_slice() {
            &[] | &[_] => Err(TokenizedRecordError::TooFewRDataTokensError {
                expected: 2,
                received: rdata.len(),
            }),
            &[next_domain_name, ..] => {
                let (next_domain_name, _) = DomainName::from_token_format(&[next_domain_name])?;
                let (type_bit_map, _) = RTypeBitmap::from_token_format(&rdata[1..])?;
                Ok(Self {
                    next_domain_name,
                    type_bit_map,
                })
            }
        }
    }
}

#[cfg(test)]
mod circular_serde_sanity_test {
    use crate::{
        resource_record::rtype::RType,
        serde::wire::circular_test::gen_test_circular_serde_sanity_test,
        types::{domain_name::DomainName, rtype_bitmap::RTypeBitmap},
    };

    use super::NSEC;

    gen_test_circular_serde_sanity_test!(
        rfc_4034_example_record_circular_serde_sanity_test,
        NSEC {
            next_domain_name: DomainName::from_utf8("host.example.com.").unwrap(),
            type_bit_map: RTypeBitmap::from_rtypes(
                [
                    RType::A,
                    RType::MX,
                    RType::RRSIG,
                    RType::NSEC,
                    RType::Unknown(1234)
                ]
                .iter()
            )
        }
    );
}

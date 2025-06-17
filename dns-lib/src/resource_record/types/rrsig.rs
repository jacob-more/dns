use dns_macros::{FromTokenizedRData, FromWire, RData, ToPresentation, ToWire};

use crate::{
    resource_record::{dnssec_alg::DnsSecAlgorithm, rtype::RType, time::Time},
    types::{base64::Base64, domain_name::DomainName},
};

/// (Original) https://datatracker.ietf.org/doc/html/rfc4034#section-3
/// (Update) https://datatracker.ietf.org/doc/html/rfc3225
/// (Update) https://datatracker.ietf.org/doc/html/rfc6840
/// (Update) https://datatracker.ietf.org/doc/html/rfc6944
#[derive(
    Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, ToPresentation, FromTokenizedRData, RData,
)]
pub struct RRSIG {
    type_covered: RType,
    algorithm: DnsSecAlgorithm,
    labels: u8,
    original_ttl: Time,
    signature_expiration: u32,
    signature_inception: u32,
    key_tag: u16,
    signers_name: DomainName,
    signature: Base64,
}

#[cfg(test)]
mod circular_serde_sanity_test {
    use crate::{
        resource_record::{dnssec_alg::DnsSecAlgorithm, rtype::RType, time::Time},
        serde::wire::circular_test::gen_test_circular_serde_sanity_test,
        types::{base64::Base64, domain_name::DomainName},
    };

    use super::RRSIG;

    gen_test_circular_serde_sanity_test!(
        rfc_4034_example_record_circular_serde_sanity_test,
        RRSIG {
            type_covered: RType::A,
            algorithm: DnsSecAlgorithm::from_code(5),
            labels: 3,
            original_ttl: Time::from_secs(86400),
            signature_expiration: 100,  //< TODO: value should be '20030322173103', using form YYYYMMDDHHmmSS
            signature_inception: 50,    //< TODO: value should be '20030220173103', using form YYYYMMDDHHmmSS
            key_tag: 2642,
            signers_name: DomainName::from_utf8("example.com.").unwrap(),
            signature: Base64::from_utf8("oJB1W6WNGv+ldvQ3WDG0MQkg5IEhjRip8WTrPYGv07h108dUKGMeDPKijVCHX3DDKdfb+v6oB9wfuh3DTJXUAfI/M0zmO/zz8bW0Rznl8O3tGNazPwQKkRN20XPXV6nwwfoXmJQbsLNrLfkGJ5D6fwFm8nN+6pBzeDQfsS3Ap3o=").unwrap(),
        }
    );
}

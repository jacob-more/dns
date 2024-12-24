use std::net::{Ipv4Addr, Ipv6Addr};

use dns_macros::RData;
use ux::{u1, u7};

use crate::{serde::{presentation::{from_presentation::FromPresentation, from_tokenized_rdata::FromTokenizedRData, to_presentation::ToPresentation}, wire::{from_wire::FromWire, to_wire::ToWire}}, types::domain_name::DomainName};

/// (Original) https://datatracker.ietf.org/doc/html/rfc8777#name-amtrelay-rdata-format
///
/// AMT = Automatic Multicast Tunneling
#[derive(Clone, PartialEq, Eq, Hash, Debug, RData)]
pub struct AMTRELAY {
    precedence: u8,
    discovery_optional: u1,
    relay: RelayType,
}

impl AMTRELAY {
    #[inline]
    pub fn new(precedence: u8, discovery_optional: bool, relay: RelayType) -> Self {
        Self {
            precedence,
            discovery_optional: u1::from(discovery_optional),
            relay,
        }
    }

    #[inline]
    pub fn precedence(&self) -> u8 { self.precedence }

    #[inline]
    pub fn discovery_optional(&self) -> bool{ bool::from(self.discovery_optional) }

    #[inline]
    pub fn relay_type(&self) -> &RelayType { &self.relay }

    #[inline]
    pub fn into_relay_type(self) -> RelayType { self.relay }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum RelayType {
    Unknown(u7, Vec<u8>),

    Empty,
    Ipv4(Ipv4Addr),
    Ipv6(Ipv6Addr),
    DomainName(DomainName),
}

impl RelayType {
    #[inline]
    pub const fn relay_type(&self) -> u7 {
        match self {
            Self::Unknown(x, _) => *x,

            Self::Empty         => u7::new(0),
            Self::Ipv4(_)       => u7::new(1),
            Self::Ipv6(_)       => u7::new(2),
            Self::DomainName(_) => u7::new(3),
        }
    }
}

impl ToWire for AMTRELAY {
    #[inline]
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>, compression: &mut Option<crate::types::c_domain_name::CompressionMap>) -> Result<(), crate::serde::wire::write_wire::WriteWireError> where 'a: 'b {
        self.precedence.to_wire_format(wire, compression)?;
        (self.discovery_optional, self.relay.relay_type()).to_wire_format(wire, compression)?;
        match &self.relay {
            RelayType::Empty => Ok(()),
            RelayType::Ipv4(address) => address.to_wire_format(wire, compression),
            RelayType::Ipv6(address) => address.to_wire_format(wire, compression),
            RelayType::DomainName(dn) => dn.to_wire_format(wire, compression),
            RelayType::Unknown(_, data) => {
                wire.write_bytes(&data)?;
                Ok(())
            },
        }
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        self.precedence.serial_length()
        + (self.discovery_optional, self.relay.relay_type()).serial_length()
        + match &self.relay {
            RelayType::Empty => 0,
            RelayType::Ipv4(address) => address.serial_length(),
            RelayType::Ipv6(address) => address.serial_length(),
            RelayType::DomainName(dn) => dn.serial_length(),
            RelayType::Unknown(_, data) => data.len() as u16,
        }
    }
}

const U7_MAX: u8 = 2_u8.pow(7) - 1;

impl FromWire for AMTRELAY {
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>) -> Result<Self, crate::serde::wire::read_wire::ReadWireError> where Self: Sized, 'a: 'b {
        let precedence = u8::from_wire_format(wire)?;
        let (discovery_optional, relay_type) = <(u1, u7)>::from_wire_format(wire)?;
        let relay = match u8::from(relay_type) {
            0 => RelayType::Empty,
            1 => RelayType::Ipv4(Ipv4Addr::from_wire_format(wire)?),
            2 => RelayType::Ipv6(Ipv6Addr::from_wire_format(wire)?),
            3 => RelayType::DomainName(DomainName::from_wire_format(wire)?),
            4..=U7_MAX => RelayType::Unknown(relay_type, wire.take_all().to_vec()),
            _ => unreachable!("All numbers, 0-127, are represented by the u7 type. No value outside that range should be possible"),
        };

        Ok(Self { precedence, discovery_optional, relay })
    }
}

impl FromTokenizedRData for AMTRELAY {
    #[inline]
    fn from_tokenized_rdata<'a, 'b>(rdata: &Vec<&'a str>) -> Result<Self, crate::serde::presentation::errors::TokenizedRecordError<'b>> where Self: Sized, 'a: 'b {
        match rdata.as_slice() {
            &[precedence, discovery_optional, relay_type, relay] => {
                let (precedence, _) = u8::from_token_format(&[precedence])?;
                let (discovery_optional, _) = u1::from_token_format(&[discovery_optional])?;
                let (relay_type, _) = u7::from_token_format(&[relay_type])?;
                let relay = match u8::from(relay_type) {
                    0 => {
                        let (root_domain, _) = DomainName::from_token_format(&[relay])?;
                        // According to RFC 8777,
                        // "If the relay type field is 0, the relay field MUST be ".""
                        // In other words, the relay type is equivalent to the root domain encoding.
                        if !root_domain.is_root() {
                            return Err(crate::serde::presentation::errors::TokenizedRecordError::ValueError(
                                format!("The relay type field was 0 but the relay field was not \".\". Instead, it was '{relay}'")
                            ));
                        }
                        RelayType::Empty
                    },
                    1 => RelayType::Ipv4(Ipv4Addr::from_token_format(&[relay])?.0),
                    2 => RelayType::Ipv6(Ipv6Addr::from_token_format(&[relay])?.0),
                    3 => RelayType::DomainName(DomainName::from_token_format(&[relay])?.0),
                    _ => return Err(crate::serde::presentation::errors::TokenizedRecordError::ValueError(
                        format!("The relay type {relay_type} is unrecognized"))
                    ),
                };

                Ok(Self {precedence, discovery_optional, relay })
            },
            &[_, _, _, _, ..] => Err(crate::serde::presentation::errors::TokenizedRecordError::TooManyRDataTokensError{expected: 4, received: rdata.len()}),
            _ => Err(crate::serde::presentation::errors::TokenizedRecordError::TooFewRDataTokensError{expected: 4, received: rdata.len()}),
        }
    }
}

impl ToPresentation for AMTRELAY {
    #[inline]
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        self.precedence.to_presentation_format(out_buffer);
        self.discovery_optional.to_presentation_format(out_buffer);
        self.relay.relay_type().to_presentation_format(out_buffer);
        match &self.relay {
            RelayType::Unknown(_, _) => panic!("There is no process for writing an unknown record to presentation format"),
            RelayType::Empty => out_buffer.push(".".to_string()),
            RelayType::Ipv4(address) => address.to_presentation_format(out_buffer),
            RelayType::Ipv6(address) => address.to_presentation_format(out_buffer),
            RelayType::DomainName(domain_name) => domain_name.to_presentation_format(out_buffer),
        }
    }
}

#[cfg(test)]
mod circular_serde_sanity_test {
    use std::{net::{Ipv4Addr, Ipv6Addr}, str::FromStr};

    use ux::{u1, u7};

    use crate::{serde::wire::circular_test::gen_test_circular_serde_sanity_test, types::domain_name::DomainName};
    use super::{AMTRELAY, RelayType};

    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test_domain,
        AMTRELAY { precedence: 1, discovery_optional: u1::new(0), relay: RelayType::DomainName(DomainName::from_utf8("www.example.org.").unwrap()) }
    );
    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test_ipv4,
        AMTRELAY { precedence: 2, discovery_optional: u1::new(1), relay: RelayType::Ipv4(Ipv4Addr::from_str("192.168.86.1").unwrap()) }
    );
    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test_ipv6,
        AMTRELAY { precedence: 3, discovery_optional: u1::new(0), relay: RelayType::Ipv6(Ipv6Addr::from_str("a:9:8:7:6:5:4:3").unwrap()) }
    );
    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test_empty,
        AMTRELAY { precedence: 4, discovery_optional: u1::new(0), relay: RelayType::Empty }
    );
    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test_unknown,
        AMTRELAY { precedence: 5, discovery_optional: u1::new(0), relay: RelayType::Unknown(u7::new(10), vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]) }
    );
}

#[cfg(test)]
mod tokenizer_tests {
    use std::net::{Ipv4Addr, Ipv6Addr};

    use ux::u1;

    use crate::{serde::presentation::test_from_tokenized_rdata::{gen_ok_record_test, gen_fail_record_test}, types::domain_name::DomainName, resource_record::types::amtrelay::RelayType};
    use super::AMTRELAY;

    const GOOD_PRECEDENCE: &str = "1";
    const BAD_PRECEDENCE: &str = "-1";
    const EMPTY_PRECEDENCE: &str = "";

    const GOOD_DISCOVERY: &str = "1";
    const BAD_DISCOVERY: &str = "1";

    const RELAY_TYPE_EMPTY: &str = "0";
    const RELAY_TYPE_IPV4: &str = "1";
    const RELAY_TYPE_IPV6: &str = "2";
    const RELAY_TYPE_DOMAIN: &str = "3";
    const RELAY_TYPE_UNKNOWN: &str = "4";

    const GOOD_EMPTY: &str = ".";
    const BAD_EMPTY: &str = "example.";

    const GOOD_DOMAIN: &str = "www.example.org.";
    const BAD_DOMAIN: &str = "..www.example.org.";

    const GOOD_IPV4: &str = "192.168.86.1";
    const BAD_IPV4: &str = "192.168.86.1.10";

    const GOOD_IPV6: &str = "a:9:8:7:6:5:4:3";
    const BAD_IPV6: &str = "a:9:8:7:6:5:4:3:2:1";

    gen_ok_record_test!(test_ok_empty, AMTRELAY, AMTRELAY { precedence: 1, discovery_optional: u1::new(1), relay: RelayType::Empty }, [GOOD_PRECEDENCE, GOOD_DISCOVERY, RELAY_TYPE_EMPTY, GOOD_EMPTY]);
    gen_ok_record_test!(test_ok_ipv4, AMTRELAY, AMTRELAY { precedence: 1, discovery_optional: u1::new(1), relay: RelayType::Ipv4(Ipv4Addr::new(192, 168, 86, 1)) }, [GOOD_PRECEDENCE, GOOD_DISCOVERY, RELAY_TYPE_IPV4, GOOD_IPV4]);
    gen_ok_record_test!(test_ok_ipv6, AMTRELAY, AMTRELAY { precedence: 1, discovery_optional: u1::new(1), relay: RelayType::Ipv6(Ipv6Addr::new(10, 9, 8, 7, 6, 5, 4, 3)) }, [GOOD_PRECEDENCE, GOOD_DISCOVERY, RELAY_TYPE_IPV6, GOOD_IPV6]);
    gen_ok_record_test!(test_ok_domain, AMTRELAY, AMTRELAY { precedence: 1, discovery_optional: u1::new(1), relay: RelayType::DomainName(DomainName::from_utf8(GOOD_DOMAIN).unwrap()) }, [GOOD_PRECEDENCE, GOOD_DISCOVERY, RELAY_TYPE_DOMAIN, GOOD_DOMAIN]);

    // Bad value tests
    gen_fail_record_test!(test_fail_bad_precedence, AMTRELAY, [BAD_PRECEDENCE, GOOD_DISCOVERY, RELAY_TYPE_EMPTY, GOOD_EMPTY]);
    gen_fail_record_test!(test_fail_bad_empty_precedence, AMTRELAY, [EMPTY_PRECEDENCE, GOOD_DISCOVERY, RELAY_TYPE_EMPTY, GOOD_EMPTY]);
    gen_fail_record_test!(test_fail_bad_discovery, AMTRELAY, [BAD_PRECEDENCE, BAD_DISCOVERY, RELAY_TYPE_EMPTY, GOOD_EMPTY]);
    gen_fail_record_test!(test_fail_bad_relay_type, AMTRELAY, [BAD_PRECEDENCE, GOOD_DISCOVERY, RELAY_TYPE_UNKNOWN, GOOD_EMPTY]);
    gen_fail_record_test!(test_fail_bad_empty, AMTRELAY, [BAD_PRECEDENCE, GOOD_DISCOVERY, RELAY_TYPE_EMPTY, BAD_EMPTY]);
    gen_fail_record_test!(test_fail_bad_ipv4, AMTRELAY, [BAD_PRECEDENCE, GOOD_DISCOVERY, RELAY_TYPE_IPV4, BAD_IPV4]);
    gen_fail_record_test!(test_fail_bad_ipv6, AMTRELAY, [BAD_PRECEDENCE, GOOD_DISCOVERY, RELAY_TYPE_IPV6, BAD_IPV6]);
    gen_fail_record_test!(test_fail_bad_domain, AMTRELAY, [BAD_PRECEDENCE, GOOD_DISCOVERY, RELAY_TYPE_DOMAIN, BAD_DOMAIN]);

    // Relay type does not match relay value tests
    gen_fail_record_test!(test_fail_empty_type_but_ipv4_relay, AMTRELAY, [GOOD_PRECEDENCE, GOOD_DISCOVERY, RELAY_TYPE_EMPTY, GOOD_IPV4]);
    gen_fail_record_test!(test_fail_empty_type_but_ipv6_relay, AMTRELAY, [GOOD_PRECEDENCE, GOOD_DISCOVERY, RELAY_TYPE_EMPTY, GOOD_IPV6]);
    gen_fail_record_test!(test_fail_empty_type_but_domain_relay, AMTRELAY, [GOOD_PRECEDENCE, GOOD_DISCOVERY, RELAY_TYPE_EMPTY, GOOD_DOMAIN]);

    gen_fail_record_test!(test_fail_ipv4_type_but_empty_relay, AMTRELAY, [GOOD_PRECEDENCE, GOOD_DISCOVERY, RELAY_TYPE_IPV4, GOOD_EMPTY]);
    gen_fail_record_test!(test_fail_ipv4_type_but_ipv6_relay, AMTRELAY, [GOOD_PRECEDENCE, GOOD_DISCOVERY, RELAY_TYPE_IPV4, GOOD_IPV6]);
    gen_fail_record_test!(test_fail_ipv4_type_but_domain_relay, AMTRELAY, [GOOD_PRECEDENCE, GOOD_DISCOVERY, RELAY_TYPE_IPV4, GOOD_DOMAIN]);

    gen_fail_record_test!(test_fail_ipv6_type_but_empty_relay, AMTRELAY, [GOOD_PRECEDENCE, GOOD_DISCOVERY, RELAY_TYPE_IPV6, GOOD_EMPTY]);
    gen_fail_record_test!(test_fail_ipv6_type_but_ipv4_relay, AMTRELAY, [GOOD_PRECEDENCE, GOOD_DISCOVERY, RELAY_TYPE_IPV6, GOOD_IPV4]);
    gen_fail_record_test!(test_fail_ipv6_type_but_domain_relay, AMTRELAY, [GOOD_PRECEDENCE, GOOD_DISCOVERY, RELAY_TYPE_IPV6, GOOD_DOMAIN]);

    // Incorrect number of tokens tests
    gen_fail_record_test!(test_fail_three_tokens, AMTRELAY, [GOOD_PRECEDENCE, GOOD_DISCOVERY, RELAY_TYPE_EMPTY]);
    gen_fail_record_test!(test_fail_two_tokens, AMTRELAY, [GOOD_PRECEDENCE, GOOD_DISCOVERY]);
    gen_fail_record_test!(test_fail_one_token, AMTRELAY, [GOOD_PRECEDENCE]);
    gen_fail_record_test!(test_fail_no_tokens, AMTRELAY, []);
}

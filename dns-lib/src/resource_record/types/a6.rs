use std::net::Ipv6Addr;

use dns_macros::RData;

use crate::{
    serde::{
        presentation::{
            from_presentation::FromPresentation, from_tokenized_rdata::FromTokenizedRData,
            to_presentation::ToPresentation,
        },
        wire::{
            from_wire::FromWire,
            read_wire::{ReadWire, ReadWireError},
            to_wire::ToWire,
            write_wire::WriteWire,
        },
    },
    types::domain_name::DomainName,
};

const IPV6_ADDRESS_LENGTH: usize = 128 / 8;

/// (Original)  https://datatracker.ietf.org/doc/html/rfc2874#section-3
/// (Updated)   https://datatracker.ietf.org/doc/html/rfc3226
/// (Obsoleted) https://datatracker.ietf.org/doc/html/rfc6563
#[derive(Clone, PartialEq, Eq, Hash, Debug, RData)]
pub struct A6 {
    prefix_length: u8,
    ipv6_address: Option<Ipv6Addr>,
    domain_name: Option<DomainName>,
}

impl A6 {
    const MAX_PREFIX_LENGTH: u8 = 128;
}

impl ToWire for A6 {
    #[inline]
    fn to_wire_format<'a, 'b>(
        &self,
        wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>,
        compression: &mut Option<crate::types::c_domain_name::CompressionMap>,
    ) -> Result<(), crate::serde::wire::write_wire::WriteWireError>
    where
        'a: 'b,
    {
        self.prefix_length.to_wire_format(wire, compression)?;

        // Serialize the IpV6 Address
        match self.ipv6_address {
            Some(ip_address) => {
                let ip_bytes = &mut [0_u8; IPV6_ADDRESS_LENGTH];
                let mut ip_bytes = WriteWire::from_bytes(ip_bytes);
                ip_address.to_wire_format(&mut ip_bytes, compression)?;
                let mut byte_count = (Self::MAX_PREFIX_LENGTH - self.prefix_length) / 8;
                let remaining_bits = (Self::MAX_PREFIX_LENGTH - self.prefix_length) % 8;
                if remaining_bits != 0 {
                    byte_count += 1;
                }
                wire.write_bytes(
                    &ip_bytes.current()
                        [(IPV6_ADDRESS_LENGTH - (byte_count as usize))..ip_bytes.current_len()],
                )?;
            }
            None => (),
        };

        self.domain_name.to_wire_format(wire, compression)?;

        Ok(())
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        self.prefix_length.serial_length()                              //< The prefix length
        + ((Self::MAX_PREFIX_LENGTH - self.prefix_length) / 8) as u16   //< The IpV6 address, with reduced length.
        + if (Self::MAX_PREFIX_LENGTH - self.prefix_length) % 8 != 0 { 1 } else { 0 }
        + self.domain_name.serial_length() //< The domain name.
    }
}

impl FromWire for A6 {
    #[inline]
    fn from_wire_format<'a, 'b>(
        wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>,
    ) -> Result<Self, crate::serde::wire::read_wire::ReadWireError>
    where
        Self: Sized,
        'a: 'b,
    {
        // Read Prefix Length
        let prefix_length = u8::from_wire_format(wire)?;
        // Lower bound does not need to be checked for an unsigned number.
        if prefix_length > Self::MAX_PREFIX_LENGTH {
            return Err(ReadWireError::OutOfBoundsError(String::from(
                "prefix length is outside of bounds 0 - 128 (inclusive)",
            )));
        }

        // prefix length is the number of bits.
        // byte count is the number of bytes, rounded up so that we can bounds check the remaining wire bytes count.
        let mut byte_count = (Self::MAX_PREFIX_LENGTH - prefix_length) / 8;
        let remaining_bits = (Self::MAX_PREFIX_LENGTH - prefix_length) % 8;
        if remaining_bits != 0 {
            byte_count += 1;
        }

        // Read IP Address
        let ipv6_address = match prefix_length {
            Self::MAX_PREFIX_LENGTH => None,
            _ => {
                let ipv6_wire_bytes = wire.take_or_err(byte_count as usize, || {
                    format!("IPv6 length is greater than the number of bytes left on the wire")
                })?;
                let mut ipv6_address_buffer = [0; IPV6_ADDRESS_LENGTH];
                let index_offset: usize = IPV6_ADDRESS_LENGTH - byte_count as usize;
                ipv6_address_buffer[index_offset..].copy_from_slice(&ipv6_wire_bytes);
                let ipv6_address = Ipv6Addr::from_wire_format(&mut ReadWire::from_bytes(
                    &mut ipv6_address_buffer,
                ))?;
                Some(ipv6_address)
            }
        };

        // Read Domain Name
        // Shift past prefix ipv6 address.
        let domain_name = match prefix_length {
            0 => None,
            _ => Some(DomainName::from_wire_format(wire)?),
        };

        Ok(Self {
            prefix_length,
            ipv6_address,
            domain_name,
        })
    }
}

impl FromTokenizedRData for A6 {
    #[inline]
    fn from_tokenized_rdata(
        rdata: &Vec<&str>,
    ) -> Result<Self, crate::serde::presentation::errors::TokenizedRecordError>
    where
        Self: Sized,
    {
        match rdata.as_slice() {
            &[token1, token2] => {
                let (prefix_length, _) = u8::from_token_format(&[token1])?;
                if prefix_length > Self::MAX_PREFIX_LENGTH {
                    return Err(
                        crate::serde::presentation::errors::TokenizedRecordError::OutOfBoundsError(
                            String::from("Prefix length is outside of bounds 0 - 128 (inclusive)"),
                        ),
                    );
                }

                match prefix_length {
                    0 => {
                        let (address, _) = Ipv6Addr::from_token_format(&[token2])?;
                        return Ok(Self {
                            prefix_length,
                            ipv6_address: Some(address),
                            domain_name: None,
                        });
                    }
                    128 => {
                        let (domain_name, _) = DomainName::from_token_format(&[token2])?;
                        return Ok(Self {
                            prefix_length,
                            ipv6_address: None,
                            domain_name: Some(domain_name),
                        });
                    }
                    _ => {
                        return Err(
                            crate::serde::presentation::errors::TokenizedRecordError::ValueError(
                                format!(
                                    "With two tokens, the prefix length for an A6 record must be 128 or 0. Instead, it was {prefix_length}"
                                ),
                            ),
                        );
                    }
                }
            }
            &[token1, token2, token3] => {
                let (prefix_length, _) = u8::from_token_format(&[token1])?;
                if prefix_length > Self::MAX_PREFIX_LENGTH {
                    return Err(
                        crate::serde::presentation::errors::TokenizedRecordError::OutOfBoundsError(
                            String::from("Prefix length is outside of bounds 0 - 128 (inclusive)"),
                        ),
                    );
                }

                if prefix_length == 0 {
                    return Err(
                        crate::serde::presentation::errors::TokenizedRecordError::ValueError(
                            String::from(
                                "With three tokens, the prefix length for an A6 record cannot be 0",
                            ),
                        ),
                    );
                }

                let (address, _) = Ipv6Addr::from_token_format(&[token2])?;
                let (domain_name, _) = DomainName::from_token_format(&[token3])?;

                return Ok(Self {
                    prefix_length,
                    ipv6_address: Some(address),
                    domain_name: Some(domain_name),
                });
            }
            _ => {
                return Err(
                    crate::serde::presentation::errors::TokenizedRecordError::ValueError(format!(
                        "An A6 record must have either 2 or 3 rdata tokens. It has {}",
                        rdata.len()
                    )),
                );
            }
        }
    }
}

impl ToPresentation for A6 {
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        match self {
            Self {
                prefix_length,
                ipv6_address: None,
                domain_name: Some(domain_name),
            } => {
                prefix_length.to_presentation_format(out_buffer);
                domain_name.to_presentation_format(out_buffer);
            }
            Self {
                prefix_length,
                ipv6_address: Some(ipv6_address),
                domain_name: None,
            } => {
                prefix_length.to_presentation_format(out_buffer);
                ipv6_address.to_presentation_format(out_buffer);
            }
            Self {
                prefix_length,
                ipv6_address: Some(ipv6_address),
                domain_name: Some(domain_name),
            } => {
                prefix_length.to_presentation_format(out_buffer);
                ipv6_address.to_presentation_format(out_buffer);
                domain_name.to_presentation_format(out_buffer);
            }
            _ => panic!(
                "A6 record is in an illegal state. It has both ipv6_address and domain_name set to None"
            ),
        }
    }
}

#[cfg(test)]
mod circular_serde_sanity_test {
    use std::net::Ipv6Addr;

    use super::A6;
    use crate::{
        serde::wire::circular_test::gen_test_circular_serde_sanity_test,
        types::domain_name::DomainName,
    };

    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test_zero_prefix_length,
        A6 {
            prefix_length: 0,
            ipv6_address: Some(Ipv6Addr::new(10, 9, 8, 7, 6, 5, 4, 3)),
            domain_name: None
        }
    );
    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test_max_prefix_length,
        A6 {
            prefix_length: 128,
            ipv6_address: None,
            domain_name: Some(DomainName::from_utf8("www.example.org.").unwrap())
        }
    );
    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test,
        A6 {
            prefix_length: 64,
            ipv6_address: Some(Ipv6Addr::new(0, 0, 0, 0, 10, 9, 8, 7)),
            domain_name: Some(DomainName::from_utf8("www.example.org.").unwrap())
        }
    );
}

#[cfg(test)]
mod tokenizer_tests {
    use super::A6;
    use crate::{
        serde::presentation::test_from_tokenized_rdata::{
            gen_fail_record_test, gen_ok_record_test,
        },
        types::domain_name::DomainName,
    };
    use std::net::Ipv6Addr;

    const GOOD_DOMAIN: &str = "www.example.org.";
    const BAD_DOMAIN: &str = "..www.example.org.";
    const GOOD_IP: &str = "a:9:8:7:6:5:4:3";
    const BAD_IP: &str = "a:9:8:7:6:5:4:3:2:1";

    gen_ok_record_test!(
        test_ok_zero_prefix,
        A6,
        A6 {
            prefix_length: 0,
            ipv6_address: Some(Ipv6Addr::new(10, 9, 8, 7, 6, 5, 4, 3)),
            domain_name: None
        },
        ["0", GOOD_IP]
    );
    gen_ok_record_test!(
        test_ok_max_prefix,
        A6,
        A6 {
            prefix_length: 128,
            ipv6_address: None,
            domain_name: Some(DomainName::from_utf8(GOOD_DOMAIN).unwrap())
        },
        ["128", GOOD_DOMAIN]
    );
    gen_ok_record_test!(
        test_ok,
        A6,
        A6 {
            prefix_length: 64,
            ipv6_address: Some(Ipv6Addr::new(0, 0, 0, 0, 10, 9, 8, 7)),
            domain_name: Some(DomainName::from_utf8("www.example.org.").unwrap())
        },
        ["64", "::a:9:8:7", GOOD_DOMAIN]
    );

    gen_fail_record_test!(
        test_fail_four_tokens,
        A6,
        ["64", "::a:9:8:7", GOOD_DOMAIN, GOOD_DOMAIN]
    );
    gen_fail_record_test!(test_fail_one_token_zero_prefix, A6, ["0"]);
    gen_fail_record_test!(test_fail_one_token_max_prefix, A6, ["128"]);
    gen_fail_record_test!(test_fail_one_token, A6, ["64"]);
    gen_fail_record_test!(test_fail_no_tokens, A6, []);

    gen_fail_record_test!(test_fail_two_tokens_bad_ip, A6, ["0", BAD_IP]);
    gen_fail_record_test!(
        test_fail_two_tokens_bad_negative_prefix,
        A6,
        ["-1", GOOD_IP]
    );
    gen_fail_record_test!(test_fail_two_tokens_bad_large_prefix, A6, ["129", GOOD_IP]);

    gen_fail_record_test!(
        test_fail_three_tokens_bad_ip,
        A6,
        ["64", BAD_IP, GOOD_DOMAIN]
    );
    gen_fail_record_test!(
        test_fail_three_tokens_bad_domain,
        A6,
        ["64", GOOD_IP, BAD_DOMAIN]
    );
    gen_fail_record_test!(
        test_fail_three_tokens_bad_negative_prefix,
        A6,
        ["-1", GOOD_IP, GOOD_DOMAIN]
    );
    gen_fail_record_test!(
        test_fail_three_tokens_bad_large_prefix,
        A6,
        ["129", GOOD_IP, GOOD_DOMAIN]
    );
}

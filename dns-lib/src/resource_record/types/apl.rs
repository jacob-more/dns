use std::{net::{Ipv4Addr, Ipv6Addr}, fmt::Display};

use dns_macros::RTypeCode;
use lazy_static::lazy_static;
use regex::Regex;
use ux::{u1, u7};

use crate::{resource_record::address_family::AddressFamily, serde::{wire::{to_wire::ToWire, from_wire::FromWire, write_wire::WriteWire, read_wire::{ReadWireError, ReadWire}}, presentation::{from_tokenized_record::FromTokenizedRecord, from_presentation::FromPresentation, errors::TokenizedRecordError, to_presentation::ToPresentation}}};

/// (Original) https://datatracker.ietf.org/doc/html/rfc3123
#[derive(Clone, PartialEq, Eq, Hash, Debug, RTypeCode)]
pub struct APL {
    address_family: AddressFamily,
    prefix: u8,
    negation_flag: bool,
    afd_length: u7,
    afd_part: AFDPart,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum AFDPart {
    Ipv4(Ipv4Addr),
    Ipv6(Ipv6Addr),
}

impl Display for AFDPart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AFDPart::Ipv4(address) => write!(f, "{address}"),
            AFDPart::Ipv6(address) => write!(f, "{address}"),
        }
    }
}

const IPV4_ADDRESS_LENGTH: usize = 4;
const IPV4_MAX_BITS: usize = IPV4_ADDRESS_LENGTH * 8;
const IPV6_ADDRESS_LENGTH: usize = 16;
const IPV6_MAX_BITS: usize = IPV6_ADDRESS_LENGTH * 8;

impl ToWire for APL {
    #[inline]
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>, compression: &mut Option<crate::serde::wire::compression_map::CompressionMap>) -> Result<(), crate::serde::wire::write_wire::WriteWireError> where 'a: 'b {
        self.address_family.to_wire_format(wire, compression)?;
        self.prefix.to_wire_format(wire, compression)?;
        let negation_flag = match self.negation_flag {
            false => u1::new(0),
            true => u1::new(1),
        };
        (negation_flag, self.afd_length).to_wire_format(wire, compression)?;

        let byte_count = u8::from(self.afd_length);
        match self.afd_part {
            AFDPart::Ipv4(address) => {
                let buffer = &mut [0_u8; IPV4_ADDRESS_LENGTH];
                let mut buffer = WriteWire::from_bytes(buffer);
                Ipv4Addr::to_wire_format(&address, &mut buffer, &mut None)?;
                wire.write_bytes(&buffer.current_state()[..(byte_count as usize)])?;
            },
            AFDPart::Ipv6(address) => {
                let buffer = &mut [0_u8; IPV6_ADDRESS_LENGTH];
                let mut buffer = WriteWire::from_bytes(buffer);
                Ipv6Addr::to_wire_format(&address, &mut buffer, &mut None)?;
                wire.write_bytes(&buffer.current_state()[..(byte_count as usize)])?;
            },
        }

        Ok(())
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        self.address_family.serial_length()
        + self.prefix.serial_length()
        + 1     //< (self.negation_flag, self.afd_length).serial_length()
        + u16::from(self.afd_length)
    }
}

impl FromWire for APL {
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>) -> Result<Self, crate::serde::wire::read_wire::ReadWireError> where Self: Sized, 'a: 'b {
        let address_family = AddressFamily::from_wire_format(wire)?;
        let prefix = u8::from_wire_format(wire)?;
        let (negation_flag, afd_length) = <(u1, u7)>::from_wire_format(wire)?;
        let negation_flag = match u16::from(negation_flag) {
            1 => true,
            0 => false,
            _ => unreachable!("A u1 can only be a 1 or a 0"),
        };

        let byte_count = u8::from(afd_length) as usize;

        let afd_part = match address_family {
            AddressFamily::Ipv4 => {
                if (prefix as usize) > IPV4_MAX_BITS {
                    return Err(ReadWireError::OutOfBoundsError(
                        format!("an Ipv4 address cannot have more than {IPV4_MAX_BITS} bits")
                    ));
                }
        
                // Don't need to bound check byte_count against IPV4_ADDRESS_LENGTH because
                // that will be done by the match statement when creating the buffer.
        
                if wire.full_state_len() < byte_count {
                    return Err(ReadWireError::OverflowError(
                        String::from("there are not enough bytes remaining in the wire to read the ipv4 address")
                    ));
                }
        
                let bytes = wire.current_state();
                // Create a 32 bit (4 byte) buffer that will be used to create the Ipv4 address.
                // Is this the best way to do this? Probably not. But it gets the job done.
                let buffer: [u8; IPV4_ADDRESS_LENGTH] = match byte_count {
                    0 => [0,        0,        0,        0       ],
                    1 => [bytes[0], 0,        0,        0       ],
                    2 => [bytes[0], bytes[1], 0,        0       ],
                    3 => [bytes[0], bytes[1], bytes[2], 0       ],
                    4 => [bytes[0], bytes[1], bytes[2], bytes[3]],

                    _ => return Err(ReadWireError::OutOfBoundsError(
                        format!("an Ipv4 address cannot have more than {IPV4_ADDRESS_LENGTH} bytes")
                    )),
                };
                let address = AFDPart::Ipv4(
                    Ipv4Addr::from_wire_format(&mut ReadWire::from_bytes(&buffer))?
                );
                wire.shift(byte_count)?;
                address
            },
            AddressFamily::Ipv6 => {
                if (prefix as usize) > IPV6_MAX_BITS {
                    return Err(ReadWireError::OutOfBoundsError(
                        format!("an Ipv6 address cannot have more than {IPV6_MAX_BITS} bits")
                    ));
                }
        
                // Don't need to bound check byte_count against IPV6_ADDRESS_LENGTH because
                // that will be done by the match statement when creating the buffer.
                
                if wire.full_state_len() < byte_count {
                    return Err(ReadWireError::OverflowError(
                        String::from("there are not enough bytes remaining in the wire to read the ipv6 address")
                    ));
                }
        
                let bytes = wire.current_state();
                // Create a 128 bit (16 byte) buffer that will be used to create the Ipv6 address.
                // Is this the best way to do this? Probably not. But it gets the job done.
                let buffer: [u8; IPV6_ADDRESS_LENGTH] = match byte_count {
        
                    0  => [0,        0,        0,        0,        0,        0,        0,        0,        0,        0,        0,         0,         0,         0,         0,         0       ],
                    1  => [bytes[0], 0,        0,        0,        0,        0,        0,        0,        0,        0,        0,         0,         0,         0,         0,         0       ],
                    2  => [bytes[0], bytes[1], 0,        0,        0,        0,        0,        0,        0,        0,        0,         0,         0,         0,         0,         0       ],
                    3  => [bytes[0], bytes[1], bytes[2], 0,        0,        0,        0,        0,        0,        0,        0,         0,         0,         0,         0,         0       ],
                    4  => [bytes[0], bytes[1], bytes[2], bytes[3], 0,        0,        0,        0,        0,        0,        0,         0,         0,         0,         0,         0       ],
                    5  => [bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], 0,        0,        0,        0,        0,        0,         0,         0,         0,         0,         0       ],
                    6  => [bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], 0,        0,        0,        0,        0,         0,         0,         0,         0,         0       ],
                    7  => [bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], 0,        0,        0,        0,         0,         0,         0,         0,         0       ],
                    8  => [bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7], 0,        0,        0,         0,         0,         0,         0,         0       ],
                    9  => [bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7], bytes[8], 0,        0,         0,         0,         0,         0,         0       ],
                    10 => [bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7], bytes[8], bytes[9], 0,         0,         0,         0,         0,         0       ],
                    11 => [bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7], bytes[8], bytes[9], bytes[10], 0,         0,         0,         0,         0       ],
                    12 => [bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7], bytes[8], bytes[9], bytes[10], bytes[11], 0,         0,         0,         0       ],
                    13 => [bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7], bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], 0,         0,         0       ],
                    14 => [bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7], bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], 0,         0       ],
                    15 => [bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7], bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], 0       ],
                    16 => [bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7], bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]],
        
                    _ => return Err(ReadWireError::OutOfBoundsError(
                        format!("an Ipv6 address cannot have more than {IPV6_ADDRESS_LENGTH} bytes")
                    )),
                };
                let address = AFDPart::Ipv6(
                    Ipv6Addr::from_wire_format(&mut ReadWire::from_bytes(&buffer))?
                );
                wire.shift(byte_count)?;
                address
            },
            _ => return Err(ReadWireError::VersionError(
                format!("Only families Ipv4 ('1') and Ipv6 ('2') are supported. Found '{address_family}'")
            )),
        };

        Ok(Self {
            address_family,
            prefix,
            negation_flag,
            afd_length,
            afd_part,
        })
    }
}

impl FromTokenizedRecord for APL {
    #[inline]
    fn from_tokenized_record<'a, 'b>(record: &crate::serde::presentation::tokenizer::tokenizer::ResourceRecord<'a>) -> Result<Self, crate::serde::presentation::errors::TokenizedRecordError<'b>> where Self: Sized, 'a: 'b {
        lazy_static!(
            static ref REGEX_ADDRESS_FAMILY: Regex = Regex::new(r"\A([0-9]+):").unwrap();
            static ref REGEX_NEGATION_FLAG: Regex = Regex::new(r"\A!").unwrap();
            static ref REGEX_PREFIX: Regex = Regex::new(r"/([0-9]+)\z").unwrap();
        );

        // Although the official documentation indicates that an APL record can define multiple APL
        // records per line, we will only support one per line. Supporting multiple records per line
        // would require redesigning this trait.

        match record.rdata.as_slice() {
            &[mut token] => {
                let mut negation_flag = false;
    
                if REGEX_NEGATION_FLAG.is_match_at(token, 0) {
                    token = &token[1..];
                    negation_flag = true;
                }
    
                let address_family = match REGEX_ADDRESS_FAMILY.find_at(token, 0) {
                    Some(address_family_match) => {
                        // Note: also removes the colon. So, remember to not include the colon in the
                        //       address family itself.
                        let address_family_str = &token[..(address_family_match.end()-1)];
                        token = &token[address_family_match.end()..];
    
                        AddressFamily::from_code(
                            u16::from_token_format(address_family_str)?
                        )
                    },
                    None => return Err(TokenizedRecordError::ValueError(
                        format!("Address family unspecified; must prefix address with digits that specify the address family followed by a colon")
                    )),
                };
    
                let prefix = match REGEX_PREFIX.find(token) {
                    Some(prefix_match) => {
                        // Note: Also removes slash. So, remember not to include the slash in the prefix
                        //       itself.
                        let prefix_str = &token[(prefix_match.start()+1)..];
                        token = &token[..prefix_match.start()];
    
                        u8::from_token_format(prefix_str)?
                    },
                    None => return Err(TokenizedRecordError::ValueError(
                        format!("Prefix unspecified; the address must be followed by a slash and digits indicating the prefix length")
                    )),
                };

                let (afd_length, afd_part) = match address_family {
                    AddressFamily::Ipv4 => {
                        if (prefix as usize) > IPV4_MAX_BITS {
                            return Err(TokenizedRecordError::OutOfBoundsError(
                                format!("an Ipv4 address cannot have more than {IPV4_MAX_BITS} bits")
                            ))
                        }
                        let afd_part = Ipv4Addr::from_token_format(token)?;
                        let afd_length = match afd_part.octets() {
                            [0, 0, 0, 0] => u7::new(0),
                            [_, 0, 0, 0] => u7::new(1),
                            [_, _, 0, 0] => u7::new(2),
                            [_, _, _, 0] => u7::new(3),
                            [_, _, _, _] => u7::new(4),
                        };
                        (afd_length, AFDPart::Ipv4(afd_part))
                    },
                    AddressFamily::Ipv6 => {
                        if (prefix as usize) > IPV6_MAX_BITS {
                            return Err(TokenizedRecordError::OutOfBoundsError(
                                format!("an Ipv6 address cannot have more than {IPV6_MAX_BITS} bits")
                            ))
                        }
                        let afd_part = Ipv6Addr::from_token_format(token)?;
                        let afd_length = match afd_part.octets() {
                            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] => u7::new(0),
                            [_, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] => u7::new(1),
                            [_, _, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] => u7::new(2),
                            [_, _, _, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] => u7::new(3),
                            [_, _, _, _, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] => u7::new(4),
                            [_, _, _, _, _, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] => u7::new(5),
                            [_, _, _, _, _, _, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] => u7::new(6),
                            [_, _, _, _, _, _, _, 0, 0, 0, 0, 0, 0, 0, 0, 0] => u7::new(7),
                            [_, _, _, _, _, _, _, _, 0, 0, 0, 0, 0, 0, 0, 0] => u7::new(8),
                            [_, _, _, _, _, _, _, _, _, 0, 0, 0, 0, 0, 0, 0] => u7::new(9),
                            [_, _, _, _, _, _, _, _, _, _, 0, 0, 0, 0, 0, 0] => u7::new(10),
                            [_, _, _, _, _, _, _, _, _, _, _, 0, 0, 0, 0, 0] => u7::new(11),
                            [_, _, _, _, _, _, _, _, _, _, _, _, 0, 0, 0, 0] => u7::new(12),
                            [_, _, _, _, _, _, _, _, _, _, _, _, _, 0, 0, 0] => u7::new(13),
                            [_, _, _, _, _, _, _, _, _, _, _, _, _, _, 0, 0] => u7::new(14),
                            [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, 0] => u7::new(15),
                            [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _] => u7::new(16),
                        };
                        (afd_length, AFDPart::Ipv6(afd_part))
                    },
                    _ => return Err(TokenizedRecordError::ValueError(
                        format!("Only families Ipv4 ('1') and Ipv6 ('2') are supported. Found '{address_family}'")
                    )),
                };

                return Ok(Self {
                    address_family,
                    prefix,
                    negation_flag,
                    afd_length,
                    afd_part,
                })
            },
            &[] => return Err(TokenizedRecordError::TooFewRDataTokensError(1, record.rdata.len())),
            &[_, ..] => return Err(TokenizedRecordError::TooManyRDataTokensError(1, record.rdata.len())),
        }
    }
}

impl ToPresentation for APL {
    #[inline]
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        match self.negation_flag {
            true => out_buffer.push(format!("!{0}:{1}/{2}", self.address_family, self.afd_part, self.prefix)),
            false => out_buffer.push(format!("{0}:{1}/{2}", self.address_family, self.afd_part, self.prefix)),
        }
    }
}

#[cfg(test)]
mod circular_serde_sanity_test {
    use std::net::{Ipv4Addr, Ipv6Addr};

    use ux::u7;

    use crate::{serde::wire::circular_test::gen_test_circular_serde_sanity_test, resource_record::address_family::AddressFamily};
    use super::{APL, AFDPart};

    gen_test_circular_serde_sanity_test!(
        ipv4_4_record_circular_serde_sanity_test,
        APL { address_family: AddressFamily::Ipv4, prefix: 32, negation_flag: false, afd_length: u7::new(4), afd_part: AFDPart::Ipv4(Ipv4Addr::new(192, 168, 86, 1)) }
    );
    gen_test_circular_serde_sanity_test!(
        ipv4_3_record_circular_serde_sanity_test,
        APL { address_family: AddressFamily::Ipv4, prefix: 24, negation_flag: false, afd_length: u7::new(3), afd_part: AFDPart::Ipv4(Ipv4Addr::new(192, 168, 86, 0)) }
    );
    gen_test_circular_serde_sanity_test!(
        ipv4_2_record_circular_serde_sanity_test,
        APL { address_family: AddressFamily::Ipv4, prefix: 16, negation_flag: false, afd_length: u7::new(2), afd_part: AFDPart::Ipv4(Ipv4Addr::new(192, 168, 0, 0)) }
    );
    gen_test_circular_serde_sanity_test!(
        ipv4_1_record_circular_serde_sanity_test,
        APL { address_family: AddressFamily::Ipv4, prefix: 8, negation_flag: false, afd_length: u7::new(1), afd_part: AFDPart::Ipv4(Ipv4Addr::new(192, 0, 0, 0)) }
    );
    gen_test_circular_serde_sanity_test!(
        ipv4_0_record_circular_serde_sanity_test,
        APL { address_family: AddressFamily::Ipv4, prefix: 0, negation_flag: false, afd_length: u7::new(0), afd_part: AFDPart::Ipv4(Ipv4Addr::new(0, 0, 0, 0)) }
    );

    gen_test_circular_serde_sanity_test!(
        ipv6_16_record_circular_serde_sanity_test,
        APL { address_family: AddressFamily::Ipv6, prefix: 128, negation_flag: false, afd_length: u7::new(16), afd_part: AFDPart::Ipv6(Ipv6Addr::new(10, 9, 8, 7, 6, 5, 4, 3)) }
    );
    gen_test_circular_serde_sanity_test!(
        ipv6_15_record_circular_serde_sanity_test,
        APL { address_family: AddressFamily::Ipv6, prefix: 120, negation_flag: false, afd_length: u7::new(15), afd_part: AFDPart::Ipv6(Ipv6Addr::new(10, 9, 8, 7, 6, 5, 4, 0)) }
    );
    gen_test_circular_serde_sanity_test!(
        ipv6_14_record_circular_serde_sanity_test,
        APL { address_family: AddressFamily::Ipv6, prefix: 112, negation_flag: false, afd_length: u7::new(14), afd_part: AFDPart::Ipv6(Ipv6Addr::new(10, 9, 8, 7, 6, 5, 4, 0)) }
    );
    gen_test_circular_serde_sanity_test!(
        ipv6_13_record_circular_serde_sanity_test,
        APL { address_family: AddressFamily::Ipv6, prefix: 104, negation_flag: false, afd_length: u7::new(13), afd_part: AFDPart::Ipv6(Ipv6Addr::new(10, 9, 8, 7, 6, 5, 0, 0)) }
    );
    gen_test_circular_serde_sanity_test!(
        ipv6_12_record_circular_serde_sanity_test,
        APL { address_family: AddressFamily::Ipv6, prefix: 96, negation_flag: false, afd_length: u7::new(12), afd_part: AFDPart::Ipv6(Ipv6Addr::new(10, 9, 8, 7, 6, 5, 0, 0)) }
    );
    gen_test_circular_serde_sanity_test!(
        ipv6_11_record_circular_serde_sanity_test,
        APL { address_family: AddressFamily::Ipv6, prefix: 88, negation_flag: false, afd_length: u7::new(11), afd_part: AFDPart::Ipv6(Ipv6Addr::new(10, 9, 8, 7, 6, 0, 0, 0)) }
    );
    gen_test_circular_serde_sanity_test!(
        ipv6_10_record_circular_serde_sanity_test,
        APL { address_family: AddressFamily::Ipv6, prefix: 80, negation_flag: false, afd_length: u7::new(10), afd_part: AFDPart::Ipv6(Ipv6Addr::new(10, 9, 8, 7, 6, 0, 0, 0)) }
    );
    gen_test_circular_serde_sanity_test!(
        ipv6_9_record_circular_serde_sanity_test,
        APL { address_family: AddressFamily::Ipv6, prefix: 72, negation_flag: false, afd_length: u7::new(9), afd_part: AFDPart::Ipv6(Ipv6Addr::new(10, 9, 8, 7, 0, 0, 0, 0)) }
    );
    gen_test_circular_serde_sanity_test!(
        ipv6_8_record_circular_serde_sanity_test,
        APL { address_family: AddressFamily::Ipv6, prefix: 64, negation_flag: false, afd_length: u7::new(8), afd_part: AFDPart::Ipv6(Ipv6Addr::new(10, 9, 8, 7, 0, 0, 0, 0)) }
    );
    gen_test_circular_serde_sanity_test!(
        ipv6_7_record_circular_serde_sanity_test,
        APL { address_family: AddressFamily::Ipv6, prefix: 56, negation_flag: false, afd_length: u7::new(7), afd_part: AFDPart::Ipv6(Ipv6Addr::new(10, 9, 8, 0, 0, 0, 0, 0)) }
    );
    gen_test_circular_serde_sanity_test!(
        ipv6_6_record_circular_serde_sanity_test,
        APL { address_family: AddressFamily::Ipv6, prefix: 48, negation_flag: false, afd_length: u7::new(6), afd_part: AFDPart::Ipv6(Ipv6Addr::new(10, 9, 8, 0, 0, 0, 0, 0)) }
    );
    gen_test_circular_serde_sanity_test!(
        ipv6_5_record_circular_serde_sanity_test,
        APL { address_family: AddressFamily::Ipv6, prefix: 40, negation_flag: false, afd_length: u7::new(5), afd_part: AFDPart::Ipv6(Ipv6Addr::new(10, 9, 0, 0, 0, 0, 0, 0)) }
    );
    gen_test_circular_serde_sanity_test!(
        ipv6_4_record_circular_serde_sanity_test,
        APL { address_family: AddressFamily::Ipv6, prefix: 32, negation_flag: false, afd_length: u7::new(4), afd_part: AFDPart::Ipv6(Ipv6Addr::new(10, 9, 0, 0, 0, 0, 0, 0)) }
    );
    gen_test_circular_serde_sanity_test!(
        ipv6_3_record_circular_serde_sanity_test,
        APL { address_family: AddressFamily::Ipv6, prefix: 24, negation_flag: false, afd_length: u7::new(3), afd_part: AFDPart::Ipv6(Ipv6Addr::new(10, 0, 0, 0, 0, 0, 0, 0)) }
    );
    gen_test_circular_serde_sanity_test!(
        ipv6_2_record_circular_serde_sanity_test,
        APL { address_family: AddressFamily::Ipv6, prefix: 16, negation_flag: false, afd_length: u7::new(2), afd_part: AFDPart::Ipv6(Ipv6Addr::new(10, 0, 0, 0, 0, 0, 0, 0)) }
    );
    gen_test_circular_serde_sanity_test!(
        ipv6_1_record_circular_serde_sanity_test,
        APL { address_family: AddressFamily::Ipv6, prefix: 8, negation_flag: false, afd_length: u7::new(1), afd_part: AFDPart::Ipv6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0)) }
    );
    gen_test_circular_serde_sanity_test!(
        ipv6_0_record_circular_serde_sanity_test,
        APL { address_family: AddressFamily::Ipv6, prefix: 0, negation_flag: false, afd_length: u7::new(0), afd_part: AFDPart::Ipv6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0)) }
    );

    gen_test_circular_serde_sanity_test!(
        negation_flag_record_circular_serde_sanity_test,
        APL { address_family: AddressFamily::Ipv4, prefix: 0, negation_flag: true, afd_length: u7::new(0), afd_part: AFDPart::Ipv4(Ipv4Addr::new(0, 0, 0, 0)) }
    );
}

#[cfg(test)]
mod tokenizer_tests {
    use std::net::{Ipv4Addr, Ipv6Addr};

    use lazy_static::lazy_static;
    use ux::u7;

    use crate::{serde::presentation::test_from_tokenized_record::{gen_ok_record_test, gen_fail_record_test}, resource_record::{address_family::AddressFamily, types::apl::AFDPart}};
    use super::APL;

    const IPV4_FAMILY: &str = "1";
    const IPV6_FAMILY: &str = "2";
    const OTHER_FAMILY: &str = "3";

    const GOOD_IPV4: &str = "192.168.86.1";
    const BAD_IPV4: &str = "192.168.86.1.10";

    const GOOD_IPV6: &str = "a:9:8:7:6:5:4:3";
    const BAD_IPV6: &str = "a:9:8:7:6:5:4:3:2:1";

    lazy_static!(
        static ref TEST_OK_IPV4_TOKEN: String = format!("{IPV4_FAMILY}:{GOOD_IPV4}/32");
        static ref TEST_OK_NEGATED_IPV4_TOKEN: String = format!("!{IPV4_FAMILY}:{GOOD_IPV4}/32");
        static ref TEST_OK_IPV6_TOKEN: String = format!("{IPV6_FAMILY}:{GOOD_IPV6}/128");
        static ref TEST_OK_NEGATED_IPV6_TOKEN: String = format!("!{IPV6_FAMILY}:{GOOD_IPV6}/128");
    );

    gen_ok_record_test!(
        test_ok_ipv4,
        APL,
        APL { address_family: AddressFamily::Ipv4, prefix: 32, negation_flag: false, afd_length: u7::new(4), afd_part: AFDPart::Ipv4(Ipv4Addr::new(192, 168, 86, 1)) },
        [TEST_OK_IPV4_TOKEN.as_str()]
    );
    gen_ok_record_test!(
        test_ok_negated_ipv4,
        APL,
        APL { address_family: AddressFamily::Ipv4, prefix: 32, negation_flag: true, afd_length: u7::new(4), afd_part: AFDPart::Ipv4(Ipv4Addr::new(192, 168, 86, 1)) },
        [TEST_OK_NEGATED_IPV4_TOKEN.as_str()]
    );
    gen_ok_record_test!(
        test_ok_ipv6,
        APL,
        APL { address_family: AddressFamily::Ipv6, prefix: 128, negation_flag: false, afd_length: u7::new(16), afd_part: AFDPart::Ipv6(Ipv6Addr::new(10, 9, 8, 7, 6, 5, 4, 3)) },
        [TEST_OK_IPV6_TOKEN.as_str()]
    );
    gen_ok_record_test!(
        test_ok_negated_ipv6,
        APL,
        APL { address_family: AddressFamily::Ipv6, prefix: 128, negation_flag: true, afd_length: u7::new(16), afd_part: AFDPart::Ipv6(Ipv6Addr::new(10, 9, 8, 7, 6, 5, 4, 3)) },
        [TEST_OK_NEGATED_IPV6_TOKEN.as_str()]
    );

    gen_fail_record_test!(test_fail_two_token, APL, [TEST_OK_IPV4_TOKEN.as_str(), TEST_OK_IPV4_TOKEN.as_str()]);
    gen_fail_record_test!(test_fail_no_tokens, APL, []);

    lazy_static!(
        static ref TEST_FAIL_BAD_IPV4_TOKEN: String = format!("{IPV4_FAMILY}:{BAD_IPV4}/32");
        static ref TEST_FAIL_NEGATED_BAD_IPV4_TOKEN: String = format!("!{IPV4_FAMILY}:{BAD_IPV4}/32");
        static ref TEST_FAIL_BAD_IPV6_TOKEN: String = format!("{IPV6_FAMILY}:{BAD_IPV6}/128");
        static ref TEST_FAIL_NEGATED_BAD_IPV6_TOKEN: String = format!("!{IPV6_FAMILY}:{BAD_IPV6}/128");
        static ref TEST_FAIL_BAD_FAMILY_TOKEN: String = format!("{OTHER_FAMILY}:{GOOD_IPV4}/32");
        static ref TEST_FAIL_NEGATED_BAD_FAMILY_TOKEN: String = format!("!{OTHER_FAMILY}:{GOOD_IPV4}/32");
    );

    gen_fail_record_test!(test_fail_bad_ipv4, APL, [TEST_FAIL_BAD_IPV4_TOKEN.as_str()]);
    gen_fail_record_test!(test_fail_negated_bad_ipv4, APL, [TEST_FAIL_NEGATED_BAD_IPV4_TOKEN.as_str()]);
    gen_fail_record_test!(test_fail_bad_ipv6, APL, [TEST_FAIL_BAD_IPV6_TOKEN.as_str()]);
    gen_fail_record_test!(test_fail_negated_bad_ipv6, APL, [TEST_FAIL_NEGATED_BAD_IPV6_TOKEN.as_str()]);
    gen_fail_record_test!(test_fail_bad_family, APL, [TEST_FAIL_BAD_FAMILY_TOKEN.as_str()]);
    gen_fail_record_test!(test_fail_negated_bad_family, APL, [TEST_FAIL_NEGATED_BAD_FAMILY_TOKEN.as_str()]);

    lazy_static!(
        static ref TEST_FAIL_BAD_FAMILY_SEPARATOR_DOT_TOKEN: String = format!("{IPV4_FAMILY}.{GOOD_IPV4}/32");
        static ref TEST_FAIL_BAD_FAMILY_SEPARATOR_SLASH_TOKEN: String = format!("!{IPV4_FAMILY}.{GOOD_IPV4}/32");
        static ref TEST_FAIL_NEGATED_BAD_FAMILY_SEPARATOR_DOT_TOKEN: String = format!("{IPV4_FAMILY}.{GOOD_IPV4}/32");
        static ref TEST_FAIL_NEGATED_BAD_FAMILY_SEPARATOR_SLASH_TOKEN: String = format!("!{IPV4_FAMILY}.{GOOD_IPV4}/32");
        static ref TEST_FAIL_BAD_PREFIX_SEPARATOR_DOT_TOKEN: String = format!("{IPV4_FAMILY}:{GOOD_IPV4}.32");
        static ref TEST_FAIL_BAD_PREFIX_SEPARATOR_COLON_TOKEN: String = format!("!{IPV4_FAMILY}:{GOOD_IPV4}:32");
        static ref TEST_FAIL_NEGATED_BAD_PREFIX_SEPARATOR_DOT_TOKEN: String = format!("{IPV4_FAMILY}:{GOOD_IPV4}.32");
        static ref TEST_FAIL_NEGATED_BAD_PREFIX_SEPARATOR_COLON_TOKEN: String = format!("!{IPV4_FAMILY}:{GOOD_IPV4}:32");
    );

    gen_fail_record_test!(test_fail_bad_family_separator_dot, APL, [TEST_FAIL_BAD_FAMILY_SEPARATOR_DOT_TOKEN.as_str()]);
    gen_fail_record_test!(test_fail_bad_family_separator_slash, APL, [TEST_FAIL_BAD_FAMILY_SEPARATOR_SLASH_TOKEN.as_str()]);
    gen_fail_record_test!(test_fail_negated_bad_family_separator_dot, APL, [TEST_FAIL_NEGATED_BAD_FAMILY_SEPARATOR_DOT_TOKEN.as_str()]);
    gen_fail_record_test!(test_fail_negated_bad_family_separator_slash, APL, [TEST_FAIL_NEGATED_BAD_FAMILY_SEPARATOR_SLASH_TOKEN.as_str()]);
    gen_fail_record_test!(test_fail_bad_prefix_separator_dot, APL, [TEST_FAIL_BAD_PREFIX_SEPARATOR_DOT_TOKEN.as_str()]);
    gen_fail_record_test!(test_fail_bad_prefix_separator_colon, APL, [TEST_FAIL_BAD_PREFIX_SEPARATOR_COLON_TOKEN.as_str()]);
    gen_fail_record_test!(test_fail_negated_bad_prefix_separator_dot, APL, [TEST_FAIL_NEGATED_BAD_PREFIX_SEPARATOR_DOT_TOKEN.as_str()]);
    gen_fail_record_test!(test_fail_negated_bad_prefix_separator_colon, APL, [TEST_FAIL_NEGATED_BAD_PREFIX_SEPARATOR_COLON_TOKEN.as_str()]);

    lazy_static!(
        static ref TEST_FAIL_NO_PREFIX_TOKEN: String = format!("{IPV4_FAMILY}:{GOOD_IPV4}");
        static ref TEST_FAIL_NEGATED_NO_PREFIX_TOKEN: String = format!("!{IPV4_FAMILY}:{GOOD_IPV4}");
        static ref TEST_FAIL_NO_PREFIX_WITH_SLASH_TOKEN: String = format!("{IPV4_FAMILY}:{GOOD_IPV4}/");
        static ref TEST_FAIL_NEGATED_NO_PREFIX_WITH_SLASH_TOKEN: String = format!("!{IPV4_FAMILY}:{GOOD_IPV4}/");
        static ref TEST_FAIL_NO_FAMILY_TOKEN: String = format!("{GOOD_IPV4}/32");
        static ref TEST_FAIL_NEGATED_NO_FAMILY_TOKEN: String = format!("!{GOOD_IPV4}/32");
        static ref TEST_FAIL_NO_FAMILY_WITH_COLON_TOKEN: String = format!(":{GOOD_IPV4}/32");
        static ref TEST_FAIL_NEGATED_NO_FAMILY_WITH_COLON_TOKEN: String = format!("!:{GOOD_IPV4}/32");
    );

    gen_fail_record_test!(test_fail_no_prefix, APL, [TEST_FAIL_BAD_FAMILY_SEPARATOR_DOT_TOKEN.as_str()]);
    gen_fail_record_test!(test_fail_negated_no_prefix, APL, [TEST_FAIL_BAD_FAMILY_SEPARATOR_SLASH_TOKEN.as_str()]);
    gen_fail_record_test!(test_fail_no_prefix_with_slash, APL, [TEST_FAIL_NEGATED_BAD_FAMILY_SEPARATOR_DOT_TOKEN.as_str()]);
    gen_fail_record_test!(test_fail_negated_no_prefix_with_slash, APL, [TEST_FAIL_NEGATED_BAD_FAMILY_SEPARATOR_SLASH_TOKEN.as_str()]);
    gen_fail_record_test!(test_fail_no_family, APL, [TEST_FAIL_BAD_PREFIX_SEPARATOR_DOT_TOKEN.as_str()]);
    gen_fail_record_test!(test_fail_negated_no_family, APL, [TEST_FAIL_BAD_PREFIX_SEPARATOR_COLON_TOKEN.as_str()]);
    gen_fail_record_test!(test_fail_no_family_with_colon, APL, [TEST_FAIL_NEGATED_BAD_PREFIX_SEPARATOR_DOT_TOKEN.as_str()]);
    gen_fail_record_test!(test_fail_negated_no_family_with_colon, APL, [TEST_FAIL_NEGATED_BAD_PREFIX_SEPARATOR_COLON_TOKEN.as_str()]);

    lazy_static!(
        static ref TEST_FAIL_BAD_CHARS_PREFIX_TOKEN: String = format!("{IPV4_FAMILY}:{GOOD_IPV4}/BADCHARACTERS");
        static ref TEST_FAIL_NEGATED_BAD_CHARS_PREFIX_TOKEN: String = format!("!{IPV4_FAMILY}:{GOOD_IPV4}/BADCHARACTERS");
        static ref TEST_FAIL_BAD_CHARS_FAMILY_TOKEN: String = format!("BADCHARACTERS:{GOOD_IPV4}/32");
        static ref TEST_FAIL_NEGATED_BAD_CHARS_FAMILY_TOKEN: String = format!("!BADCHARACTERS:{GOOD_IPV4}/32");
    );

    gen_fail_record_test!(test_fail_bad_chars_prefix_token, APL, [TEST_FAIL_BAD_CHARS_PREFIX_TOKEN.as_str()]);
    gen_fail_record_test!(test_fail_negated_bad_chars_prefix_token, APL, [TEST_FAIL_NEGATED_BAD_CHARS_PREFIX_TOKEN.as_str()]);
    gen_fail_record_test!(test_fail_bad_chars_family_token, APL, [TEST_FAIL_BAD_CHARS_FAMILY_TOKEN.as_str()]);
    gen_fail_record_test!(test_fail_negated_bad_chars_family_token, APL, [TEST_FAIL_NEGATED_BAD_CHARS_FAMILY_TOKEN.as_str()]);
}

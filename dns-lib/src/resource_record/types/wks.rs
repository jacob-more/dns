use std::net::Ipv4Addr;

use dns_macros::{ToWire, FromWire, RTypeCode};
use lazy_static::lazy_static;
use regex::Regex;

use crate::{serde::presentation::{from_tokenized_rdata::FromTokenizedRData, to_presentation::ToPresentation, errors::{TokenizedRecordError, TokenError}, from_presentation::FromPresentation}, resource_record::{protocol::Protocol, port_from_service::port_from_service}};

#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, RTypeCode)]
pub struct WKS {
    address: Ipv4Addr,
    protocol: Protocol,
    bit_map: Vec<u8>,
}

impl FromTokenizedRData for WKS {
    #[inline]
    fn from_tokenized_rdata<'a, 'b>(rdata: &Vec<&'a str>) -> Result<Self, crate::serde::presentation::errors::TokenizedRecordError<'b>> where Self: Sized, 'a: 'b {
        lazy_static!{
            static ref REGEX_UNSIGNED_INT: Regex = Regex::new(r"\A((\d)+)\z").unwrap();
        }

        if rdata.len() < 3 {
            return Err(TokenizedRecordError::TooFewRDataTokensError{expected: 3, received: rdata.len()});
        }
        
        let address = Ipv4Addr::from_token_format(rdata[0])?;
        let protocol = Protocol::from_token_format(rdata[1])?;
        let mut port_bitmap: Vec<u8> = Vec::new();

        for service in &rdata[2..] {
            if REGEX_UNSIGNED_INT.is_match_at(service, 0) {
                add_port_to_bitmap(
                    &mut port_bitmap,
                    &u16::from_token_format(service)?
                );
            } else {
                let ports = match port_from_service(service, &protocol) {
                    Ok(ports) => ports,
                    Err(error) => Err(TokenError::PortError(error))?,
                };
                for port in ports {
                    add_port_to_bitmap(&mut port_bitmap, port);
                }
            }
        }

        Ok(Self {
            address,
            protocol,
            bit_map: port_bitmap,
        })
    }
}

#[inline]
fn add_port_to_bitmap(bitmap: &mut Vec<u8>, port: &u16) {
    let bitmap_index = (port / 8) as usize;
    let bit_offset = 7 - (port % 8);
    while bitmap.len() <= bitmap_index {
        bitmap.push(0);
    }
    if let Some(byte) = bitmap.get_mut(bitmap_index) {
        *byte |= 0b00000001 << bit_offset;
    } else {
        panic!("Inconsistent State Reached: Bitmap had bytes added but byte was None")
    }
}

impl ToPresentation for WKS {
    #[inline]
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        self.address.to_presentation_format(out_buffer);
        self.protocol.to_presentation_format(out_buffer);
        for (index, byte) in self.bit_map.iter().enumerate() {
            // TODO: There is not currently a mapping from port numbers -> services but that might
            //       be something worth adding instead of always writing raw port numbers
            let index = index as u16;
            if *byte == 0 {
                continue;
            }
            if *byte & 0b00000001 == 0b00000001 {
                ((index * 8) + 7).to_presentation_format(out_buffer);
            }
            if *byte & 0b00000010 == 0b00000010 {
                ((index * 8) + 6).to_presentation_format(out_buffer);
            }
            if *byte & 0b00000100 == 0b00000100 {
                ((index * 8) + 5).to_presentation_format(out_buffer);
            }
            if *byte & 0b00001000 == 0b00001000 {
                ((index * 8) + 4).to_presentation_format(out_buffer);
            }
            if *byte & 0b00010000 == 0b00010000 {
                ((index * 8) + 3).to_presentation_format(out_buffer);
            }
            if *byte & 0b00100000 == 0b00100000 {
                ((index * 8) + 2).to_presentation_format(out_buffer);
            }
            if *byte & 0b01000000 == 0b01000000 {
                ((index * 8) + 1).to_presentation_format(out_buffer);
            }
            if *byte & 0b10000000 == 0b10000000 {
                ((index * 8) + 0).to_presentation_format(out_buffer);
            }
        }
    }
}

#[cfg(test)]
mod circular_serde_sanity_test {
    use std::net::Ipv4Addr;

    use crate::{serde::wire::circular_test::gen_test_circular_serde_sanity_test, resource_record::protocol::Protocol};
    use super::WKS;

    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test,
        WKS {
            address: Ipv4Addr::new(192, 168, 86, 1),
            protocol: Protocol::TCP,
            bit_map: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20]
        }
    );
    gen_test_circular_serde_sanity_test!(
        empty_bitmap_record_circular_serde_sanity_test,
        WKS {
            address: Ipv4Addr::new(192, 168, 86, 1),
            protocol: Protocol::UDP,
            bit_map: vec![]
        }
    );
    // Note: although there could technically be u16::MAX ports, since the serial length is a u16
    //       and this record has other fields, that would cause integer overflow so the test would
    //       fail.
    gen_test_circular_serde_sanity_test!(
        large_bitmap_record_circular_serde_sanity_test,
        WKS {
            address: Ipv4Addr::new(192, 168, 86, 1),
            protocol: Protocol::UDP,
            bit_map: Vec::from_iter([u8::MAX].repeat((u16::MAX - (4 + 1)) as usize))
        }
    );
}

#[cfg(test)]
mod tokenizer_tests {
    use std::net::Ipv4Addr;
    use crate::{serde::presentation::test_from_tokenized_rdata::{gen_ok_record_test, gen_fail_record_test}, resource_record::protocol::Protocol};
    use super::WKS;

    const GOOD_IP: &str = "192.168.86.1";
    const BAD_IP: &str = "192.168.86.1.9";

    const GOOD_PROTOCOL: &str = "TCP";
    const BAD_PROTOCOL: &str = "THIS IS NOT A PROTOCOL AND WILL FAIL";

    // TCPMUX is port 1
    const GOOD_PORT_TCPMUX: &str = "tcpmux";
    // FTP is port 21
    const GOOD_PORT_FTP: &str = "ftp";
    // SSH is port 22
    const GOOD_PORT_SSH: &str = "ssh";
    const BAD_PORT: &str = "THIS IS NOT A PORT AND WILL FAIL";

    gen_ok_record_test!(
        test_ok_ftp, WKS, WKS {
            address: Ipv4Addr::new(192, 168, 86, 1),
            protocol: Protocol::TCP,
            bit_map: vec![0b00000000, 0b00000000, 0b00000100]
        }, [GOOD_IP, GOOD_PROTOCOL, GOOD_PORT_FTP]
    );
    gen_ok_record_test!(
        test_ok_ftp_ssh, WKS, WKS {
            address: Ipv4Addr::new(192, 168, 86, 1),
            protocol: Protocol::TCP,
            bit_map: vec![0b00000000, 0b00000000, 0b00000110]
        }, [GOOD_IP, GOOD_PROTOCOL, GOOD_PORT_FTP, GOOD_PORT_SSH]
    );
    gen_ok_record_test!(
        test_ok_tcpmux_ftp, WKS, WKS {
            address: Ipv4Addr::new(192, 168, 86, 1),
            protocol: Protocol::TCP,
            bit_map: vec![0b01000000, 0b00000000, 0b00000100]
        }, [GOOD_IP, GOOD_PROTOCOL, GOOD_PORT_FTP, GOOD_PORT_TCPMUX]
    );
    gen_fail_record_test!(test_fail_good_and_bad_port, WKS, [GOOD_IP, GOOD_PROTOCOL, GOOD_PORT_SSH, BAD_PORT]);
    gen_fail_record_test!(test_fail_bad_port, WKS, [GOOD_IP, GOOD_PROTOCOL, BAD_PORT]);
    gen_fail_record_test!(test_fail_bad_protocol, WKS, [GOOD_IP, BAD_PROTOCOL, GOOD_PORT_SSH]);
    gen_fail_record_test!(test_fail_bad_ip, WKS, [BAD_IP, GOOD_PROTOCOL, GOOD_PORT_SSH]);
    gen_fail_record_test!(test_fail_two_tokens, WKS, [GOOD_IP, GOOD_PROTOCOL]);
    gen_fail_record_test!(test_fail_one_token, WKS, [GOOD_IP]);
    gen_fail_record_test!(test_fail_no_tokens, WKS, []);
}

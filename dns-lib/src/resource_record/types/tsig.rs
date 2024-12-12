use dns_macros::RData;
use ux::u48;

use crate::{resource_record::rcode::RCode, serde::wire::{from_wire::FromWire, to_wire::ToWire}, types::domain_name::DomainName};

/// (Original) https://datatracker.ietf.org/doc/html/rfc8945#name-tsig-rr-format
#[derive(Clone, PartialEq, Eq, Hash, Debug, RData)]
pub struct TSIG {
    algorithm_name: DomainName,
    time_signed: u48,
    fudge: u16,
    mac: Vec<u8>,
    original_id: u16,
    error: RCode,
    other_data: Vec<u8>,
}

impl ToWire for TSIG {
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>, compression: &mut Option<crate::types::c_domain_name::CompressionMap>) -> Result<(), crate::serde::wire::write_wire::WriteWireError> where 'a: 'b {
        self.algorithm_name.to_wire_format(wire, compression)?;
        self.time_signed.to_wire_format(wire, compression)?;
        self.fudge.to_wire_format(wire, compression)?;
        (self.mac.len() as u16).to_wire_format(wire, compression)?;
        self.mac.to_wire_format(wire, compression)?;
        self.original_id.to_wire_format(wire, compression)?;
        self.error.to_wire_format(wire, compression)?;
        (self.other_data.len() as u16).to_wire_format(wire, compression)?;
        self.other_data.to_wire_format(wire, compression)?;

        Ok(())
    }

    fn serial_length(&self) -> u16 {
        self.algorithm_name.serial_length()
        + self.time_signed.serial_length()
        + self.fudge.serial_length()
        + (self.mac.len() as u16).serial_length()
        + (self.mac.len() as u16)
        + self.original_id.serial_length()
        + self.error.serial_length()
        + (self.other_data.len() as u16).serial_length()
        + (self.other_data.len() as u16)
    }
}

impl FromWire for TSIG {
    fn from_wire_format<'a, 'b>(wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>) -> Result<Self, crate::serde::wire::read_wire::ReadWireError> where Self: Sized, 'a: 'b {
        let algorithm_name = DomainName::from_wire_format(wire)?;
        let time_signed = u48::from_wire_format(wire)?;
        let fudge = u16::from_wire_format(wire)?;

        let mac_len = u16::from_wire_format(wire)? as usize;
        let mac = wire.take(mac_len)?.to_vec();

        let original_id = u16::from_wire_format(wire)?;
        let error = RCode::from_wire_format(wire)?;

        let other_data_len = u16::from_wire_format(wire)? as usize;
        let other_data = wire.take(other_data_len)?.to_vec();

        Ok(Self {
            algorithm_name,
            time_signed,
            fudge,
            mac,
            original_id,
            error,
            other_data,
        })
    }
}

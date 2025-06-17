use std::fmt::Display;

use tinyvec::TinyVec;
use ux::{u1, u3, u4};

use crate::{
    resource_record::{opcode::OpCode, rcode::RCode, resource_record::ResourceRecord},
    serde::wire::{
        from_wire::FromWire, read_wire::ReadWireError, to_wire::ToWire, write_wire::WriteWireError,
    },
};

use super::{qr::QR, question::Question};

/// The theoretical minimum number of bytes that a Message requires. This
/// message would include ONLY header bytes.
const MIN_MESSAGE_BYTE_LEN: u16 = 96;

/// https://datatracker.ietf.org/doc/html/rfc1035#section-4
#[derive(Clone, PartialEq, Hash, Debug)]
pub struct Message {
    pub id: u16,

    // Flags
    pub qr: QR,
    pub opcode: OpCode,
    pub authoritative_answer: bool,
    pub truncation: bool,
    pub recursion_desired: bool,
    pub recursion_available: bool,
    pub z: u3,
    pub rcode: RCode,

    // Data
    pub question: TinyVec<[Question; 1]>,
    pub answer: Vec<ResourceRecord>,
    pub authority: Vec<ResourceRecord>,
    pub additional: Vec<ResourceRecord>,
}

impl Message {
    #[inline]
    pub fn qr_flag(&self) -> &QR {
        &self.qr
    }

    #[inline]
    pub fn opcode_flag(&self) -> &OpCode {
        &self.opcode
    }

    #[inline]
    pub fn authoritative_answer_flag(&self) -> bool {
        self.authoritative_answer
    }

    #[inline]
    pub fn truncation_flag(&self) -> bool {
        self.truncation
    }

    #[inline]
    pub fn recursion_desired_flag(&self) -> bool {
        self.recursion_desired
    }

    #[inline]
    pub fn recursion_available_flag(&self) -> bool {
        self.recursion_available
    }

    #[inline]
    pub fn z_flag(&self) -> u3 {
        self.z
    }

    #[inline]
    pub fn rcode_flag(&self) -> &RCode {
        &self.rcode
    }

    #[inline]
    pub fn question(&self) -> &[Question] {
        &self.question
    }

    #[inline]
    pub fn answer(&self) -> &[ResourceRecord] {
        &self.answer
    }

    #[inline]
    pub fn authority(&self) -> &[ResourceRecord] {
        &self.authority
    }

    #[inline]
    pub fn additional(&self) -> &[ResourceRecord] {
        &self.additional
    }

    /// Truncates the message until it contains at most `byte_len` bytes in its
    /// serialized form. It then sets the `truncation` flag to `true`.
    #[inline]
    pub fn truncate(&mut self, byte_len: u16) {
        // TODO: Consider coming up with a system that can determine the compressed message size to
        // get a better grasp on which records actually need to be dropped. This might not be
        // needed though, since the client should NEVER use the contents of a truncated message
        // and is supposed to re-query via TCP.

        let mut total_serial_len = self.serial_length();

        // Don't truncate if we don't have to.
        if total_serial_len <= byte_len {
            return;
        }
        self.truncation = true;

        while let Some(record) = self.additional.pop() {
            total_serial_len -= record.serial_length();
            if total_serial_len <= byte_len {
                return;
            }
        }

        while let Some(record) = self.authority.pop() {
            total_serial_len -= record.serial_length();
            if total_serial_len <= byte_len {
                return;
            }
        }

        while let Some(record) = self.answer.pop() {
            total_serial_len -= record.serial_length();
            if total_serial_len <= byte_len {
                return;
            }
        }

        // Should we remove the question? Since the message still has an ID, it
        // can still be identified by the client. This way, they can at least
        // be notified that the message was truncated.
        while let Some(record) = self.question.pop() {
            total_serial_len -= record.serial_length();
            if total_serial_len <= byte_len {
                return;
            }
        }

        // TODO: make sure the OPT record is handled correctly once that has been implemented. It
        // may need special handling.
    }
}

impl Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: Come up with a better display than the default debug implementation.
        write!(f, "{self:?}")
    }
}

impl From<Question> for Message {
    #[inline]
    fn from(question: Question) -> Self {
        Self {
            id: 0, //< An ID will be assigned when the message is sent over the network
            qr: QR::Query,
            opcode: OpCode::Query,
            authoritative_answer: false,
            truncation: false,
            recursion_desired: false,
            recursion_available: false,
            z: u3::new(0),
            rcode: RCode::NoError,
            question: TinyVec::from([question]),
            answer: vec![],
            authority: vec![],
            additional: vec![],
        }
    }
}

impl From<&Question> for Message {
    #[inline]
    fn from(question: &Question) -> Self {
        Self::from(question.clone())
    }
}

#[inline]
const fn bool_to_u1(boolean: bool) -> u1 {
    match boolean {
        true => u1::new(1),
        false => u1::new(0),
    }
}

#[inline]
fn u1_to_bool(integer: u1) -> bool {
    match u16::from(integer) {
        1 => true,
        _ => false,
    }
}

impl Message {
    #[inline]
    pub fn to_wire_format_with_two_octet_length<'a, 'b>(
        &self,
        wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>,
        compression: &mut Option<crate::types::c_domain_name::CompressionMap>,
    ) -> Result<(), crate::serde::wire::write_wire::WriteWireError>
    where
        'a: 'b,
    {
        // Push two bytes onto the wire. These will be replaced with the u16 that indicates the wire
        // length.
        let two_octet_length_offset = wire.current_len();
        wire.write_bytes(&[0, 0])?;

        let wire_start_offset = wire.current_len();
        self.to_wire_format(wire, compression)?;
        let wire_end_offset = wire.current_len();

        let wire_length = wire_end_offset - wire_start_offset;
        if wire_length > u16::MAX as usize {
            return Err(WriteWireError::OverflowError(format!(
                "Tried to write {} bytes but the length octet can be at most {}",
                wire_length,
                u16::MAX
            )));
        }
        wire.write_bytes_at(&(wire_length as u16).to_be_bytes(), two_octet_length_offset)
    }
}

impl ToWire for Message {
    #[inline]
    fn to_wire_format<'a, 'b>(
        &self,
        wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>,
        compression: &mut Option<crate::types::c_domain_name::CompressionMap>,
    ) -> Result<(), crate::serde::wire::write_wire::WriteWireError>
    where
        'a: 'b,
    {
        self.id.to_wire_format(wire, compression)?;

        let qr = match self.qr {
            QR::Query => u1::new(0),
            QR::Response => u1::new(1),
        };
        let opcode = self.opcode.code();
        let aa = bool_to_u1(self.authoritative_answer);
        let tc = bool_to_u1(self.truncation);
        let rd = bool_to_u1(self.recursion_desired);
        (qr, opcode, aa, tc, rd).to_wire_format(wire, compression)?;

        let ra = bool_to_u1(self.recursion_available);
        let z = self.z;
        let rcode = match self.rcode.code() {
            rcode @ 0..=255 => u4::new(rcode as u8),
            rcode @ 256.. => {
                return Err(WriteWireError::OutOfBoundsError(format!(
                    "The Message RCode must be within the range 0 to 255 but it was {rcode}"
                )));
            }
        };
        (ra, z, rcode).to_wire_format(wire, compression)?;

        (self.question.len() as u16).to_wire_format(wire, compression)?;
        (self.answer.len() as u16).to_wire_format(wire, compression)?;
        (self.authority.len() as u16).to_wire_format(wire, compression)?;
        (self.additional.len() as u16).to_wire_format(wire, compression)?;

        self.question
            .iter()
            .try_for_each(|question| question.to_wire_format(wire, compression))?;
        self.answer.to_wire_format(wire, compression)?;
        self.authority.to_wire_format(wire, compression)?;
        self.additional.to_wire_format(wire, compression)
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        self.id.serial_length()

        // Flags
        + 2  //< Covers all the flags (a u16).

        // Counts
        + (self.question.len() as u16).serial_length()
        + (self.answer.len() as u16).serial_length()
        + (self.authority.len() as u16).serial_length()
        + (self.additional.len() as u16).serial_length()

        // Data
        + self.question.iter().fold(0, |sum, question| sum + question.serial_length())
        + self.answer.serial_length()
        + self.authority.serial_length()
        + self.additional.serial_length()
    }
}

impl FromWire for Message {
    #[inline]
    fn from_wire_format<'a, 'b>(
        wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>,
    ) -> Result<Self, crate::serde::wire::read_wire::ReadWireError>
    where
        Self: Sized,
        'a: 'b,
    {
        let id = u16::from_wire_format(wire)?;
        let (qr, opcode, aa, tc, rd) = <(u1, u4, u1, u1, u1)>::from_wire_format(wire)?;

        let qr = match u16::from(qr) {
            0 => QR::Query,
            1 => QR::Response,
            _ => {
                return Err(ReadWireError::ValueError(String::from(
                    "incorrect query response value. Only allowed to be 0 (query) or 1 (response)",
                )));
            }
        };

        let opcode = OpCode::from_code(opcode);
        let aa = u1_to_bool(aa);
        let tc = u1_to_bool(tc);
        let rd = u1_to_bool(rd);

        let (ra, z, rcode) = <(u1, u3, u4)>::from_wire_format(wire)?;

        let ra = u1_to_bool(ra);
        let rcode = RCode::from_code(rcode.into());

        let mut qd_count = u16::from_wire_format(wire)?;
        let mut an_count = u16::from_wire_format(wire)?;
        let mut ns_count = u16::from_wire_format(wire)?;
        let mut ar_count = u16::from_wire_format(wire)?;

        let mut question = TinyVec::with_capacity(qd_count as usize);
        let mut answer = Vec::with_capacity(an_count as usize);
        let mut authority = Vec::with_capacity(ns_count as usize);
        let mut additional = Vec::with_capacity(ar_count as usize);

        while qd_count > 0 {
            question.push(Question::from_wire_format(wire)?);
            qd_count -= 1;
        }
        while an_count > 0 {
            // TODO: once handling of Unknown RRs is implemented, this match can be collapsed back
            // down into a simple `?`.
            match ResourceRecord::from_wire_format(wire) {
                Ok(record) => answer.push(record),
                Err(error @ ReadWireError::UnsupportedRType(_)) => println!("{error}"),
                Err(error) => return Err(error),
            }
            an_count -= 1;
        }
        while ns_count > 0 {
            // TODO: once handling of Unknown RRs is implemented, this match can be collapsed back
            // down into a simple `?`.
            match ResourceRecord::from_wire_format(wire) {
                Ok(record) => authority.push(record),
                Err(error @ ReadWireError::UnsupportedRType(_)) => println!("{error}"),
                Err(error) => return Err(error),
            }
            ns_count -= 1;
        }
        while ar_count > 0 {
            // TODO: once handling of Unknown RRs is implemented, this match can be collapsed back
            // down into a simple `?`.
            match ResourceRecord::from_wire_format(wire) {
                Ok(record) => additional.push(record),
                Err(error @ ReadWireError::UnsupportedRType(_)) => println!("{error}"),
                Err(error) => return Err(error),
            }
            ar_count -= 1;
        }

        Ok(Self {
            id,

            // Flags
            qr,
            opcode,
            authoritative_answer: aa,
            truncation: tc,
            recursion_desired: rd,
            recursion_available: ra,
            z: z,
            rcode: rcode,

            // Data
            question,
            answer,
            authority,
            additional,
        })
    }
}

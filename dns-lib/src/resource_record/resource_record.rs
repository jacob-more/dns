use std::fmt::Display;

use crate::{serde::{presentation::{errors::TokenizedRecordError, from_presentation::FromPresentation, from_tokenized_rdata::FromTokenizedRData, to_presentation::ToPresentation}, wire::{from_wire::FromWire, read_wire::{ReadWireError, SliceWireVisibility}, to_wire::ToWire}}, types::c_domain_name::CDomainName};

use super::{rclass::RClass, rtype::RType, time::Time, types::{a::A, a6::A6, aaaa::AAAA, afsdb::AFSDB, amtrelay::AMTRELAY, any::ANY, apl::APL, axfr::AXFR, caa::CAA, cert::CERT, cname::CNAME, dname::DNAME, dnskey::DNSKEY, hinfo::HINFO, maila::MAILA, mailb::MAILB, mb::MB, md::MD, mf::MF, mg::MG, minfo::MINFO, mr::MR, mx::MX, ns::NS, nsec::NSEC, null::NULL, ptr::PTR, rrsig::RRSIG, soa::SOA, tlsa::TLSA, tsig::TSIG, txt::TXT, wks::WKS}};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct RRHeader {
    name: CDomainName,
    rclass: RClass,
    ttl: Time,
}

impl RRHeader {
    /// A less strict version of equality. If two records match, that means that
    /// the records have the following equalities:
    /// 
    ///  1. `name`
    ///  2. `rclass`
    /// 
    /// The `ttl` value is not included because if two records that share rdata are compared,
    /// it is often useful to consider them as being equal as having a different ttl does
    /// not change that they are conveying the same information.
    #[inline]
    pub fn matches(&self, other: &Self) -> bool {
        (self.name == other.name) && (self.rclass == other.rclass)
    }

    #[inline]
    pub const fn get_name(&self) -> &CDomainName {
        &self.name
    }

    #[inline]
    pub const fn get_rclass(&self) -> RClass {
        self.rclass
    }

    #[inline]
    pub const fn get_ttl(&self) -> &Time {
        &self.ttl
    }

    #[inline]
    pub fn set_ttl(&mut self, new_ttl: Time) {
        self.ttl = new_ttl;
    }
}

macro_rules! gen_resource_record {
    ($(($record:ident, $presentation_rule:ident)),+$(,)?) => {
        /// https://datatracker.ietf.org/doc/html/rfc1035#section-4.1.3
        #[derive(Clone, PartialEq, Eq, Hash, Debug)]
        pub enum ResourceRecord {
            $($record(RRHeader, $record),)+
        }

        impl ResourceRecord {
            #[inline]
            const fn header_and_rtype(&self) -> (&RRHeader, RType) {
                match self {
                    $(Self::$record(header, _) => (header, RType::$record),)+
                }
            }

            #[inline]
            fn mut_header_and_rtype(&mut self) -> (&mut RRHeader, RType) {
                match self {
                    $(Self::$record(header, _) => (header, RType::$record),)+
                }
            }

            #[inline]
            pub const fn name(&self) -> &CDomainName {
                self.header_and_rtype().0.get_name()
            }
        
            #[inline]
            pub const fn rtype(&self) -> RType {
                self.header_and_rtype().1
            }
        
            #[inline]
            pub const fn rclass(&self) -> RClass {
                self.header_and_rtype().0.get_rclass()
            }
        
            #[inline]
            pub const fn ttl(&self) -> &Time {
                &self.header_and_rtype().0.get_ttl()
            }
        
            #[inline]
            pub fn set_ttl(&mut self, new_ttl: Time) {
                self.mut_header_and_rtype().0.set_ttl(new_ttl);
            }

            #[inline]
            pub fn rd_length(&self) -> u16 {
                match self {
                    $(Self::$record(_, rdata) => rdata.serial_length(),)+
                }
            }

            /// A less strict version of equality. If two records match, that means that
            /// the records have the following equalities:
            /// 
            ///  1. `name`
            ///  2. `rtype` if available
            ///  3. `rclass`
            ///  4. `rdata`
            /// 
            /// The `ttl` value is not included because if two records that share rdata are compared,
            /// it is often useful to consider them as being equal as having a different ttl does
            /// not change that they are conveying the same information.
            #[inline]
            pub fn matches(&self, other: &Self) -> bool {
                match (self, other) {
                    $((Self::$record(self_header, self_rdata), Self::$record(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),)+

                    // If the resource type is not the same, then the records are not the same.
                    (_, _) => false,
                }
            }
        }

        impl ToWire for ResourceRecord {
            fn to_wire_format<'a, 'b>(&self, wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>, compression: &mut Option<crate::serde::wire::compression_map::CompressionMap>) -> Result<(), crate::serde::wire::write_wire::WriteWireError> where 'a: 'b {
                let (header, rtype) = self.header_and_rtype();
                header.name.to_wire_format(wire, compression)?;
                rtype.to_wire_format(wire, compression)?;
                header.rclass.to_wire_format(wire, compression)?;
                header.ttl.to_wire_format(wire, compression)?;
        
                // If any compression is done, we may need to modify the rd_length.
                let rd_length_offset = wire.current_len();
                0_u16.to_wire_format(wire, compression)?;
        
                let rdata_offset = wire.current_len();
                match self {
                    $(Self::$record(_, rdata) => rdata.to_wire_format(wire, compression)?,)+
                };
        
                // Replace the rd_length with the actual number of bytes that got written. This way,
                // even if it got compressed, it will be accurate.
                let actual_rd_length = (wire.current_len() - rdata_offset) as u16;
                wire.write_bytes_at(&actual_rd_length.to_be_bytes(), rd_length_offset)?;
        
                Ok(())
            }
        
            fn serial_length(&self) -> u16 {
                let (header, rtype) = self.header_and_rtype();
                let rd_length = self.rd_length();
                return header.name.serial_length()
                    + rtype.serial_length()
                    + header.rclass.serial_length()
                    + header.ttl.serial_length()
                    + rd_length.serial_length()
                    + rd_length;
            }
        }

        impl FromWire for ResourceRecord {
            fn from_wire_format<'a, 'b>(wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>) -> Result<Self, crate::serde::wire::read_wire::ReadWireError> where Self: Sized, 'a: 'b {
                let name = CDomainName::from_wire_format(wire)?;
                let rtype = RType::from_wire_format(wire)?;
                let rclass = RClass::from_wire_format(wire)?;
                let ttl = Time::from_wire_format(wire)?;
                let header = RRHeader { name, rclass, ttl };
                // We will not store the wire_rd_length, instead, we will recalculate it since
                // things like domain name compression could cause it to change its value.
                let wire_rd_length = u16::from_wire_format(wire)?;
        
                // The lower bound is None because it needs access to the previous parts of the wire
                // when de-referencing the domain name pointers. No pointer should point past the
                // end of the rdata section (forward pointers) for domain name compression so
                // blocking off the end should not cause any problems when decompressing.
                // An upper bound is required to prevent any of the deserializers that fully consume
                // the rdata section from continuing past the end.
                let mut rdata_wire = wire.slice_from_current(..(wire_rd_length as usize), SliceWireVisibility::Entire)?;
                let (rr_record, rd_length) = match rtype {
                    $(RType::$record => {
                        let rdata = <$record>::from_wire_format(&mut rdata_wire)?;
                        let rd_length = rdata.serial_length();
                        (Self::$record(header, rdata), rd_length)
                    },)+
                    _ => return Err(ReadWireError::UnsupportedRType(rtype)),
                };
                wire.shift(rdata_wire.current_offset() - wire.current_offset())?;
        
                if rd_length > u16::MAX {
                    return Err(ReadWireError::OverflowError(
                        format!("Expected rd_length to be at most {0} bytes. rd_length is actually {1}", u16::MAX, rd_length)
                    ));
                }
        
                return Ok(rr_record);
            }
        }

        impl ResourceRecord {
            pub fn from_tokenized_record<'a, 'b>(record: &crate::serde::presentation::tokenizer::tokenizer::ResourceRecordToken<'a>) -> Result<Self, TokenizedRecordError<'b>> where Self: Sized, 'a: 'b {
                let rr_header = RRHeader {
                    name: CDomainName::from_token_format(&[record.domain_name])?.0,
                    rclass: RClass::from_token_format(&[record.rclass])?.0,
                    ttl: Time::from_token_format(&[record.ttl])?.0,
                };

                let (rtype, _) = RType::from_token_format(&[record.rtype])?;
                let record = match rtype {
                    $(RType::$record => gen_from_presentation!($record, rtype, rr_header, record, $presentation_rule),)+
                    _ => return Err(TokenizedRecordError::UnsupportedRType(rtype)),
                };

                return Ok(record)
            }
        }

        impl ToPresentation for ResourceRecord {
            fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
                let (rr_header, rtype) = self.header_and_rtype();
                rr_header.name.to_presentation_format(out_buffer);
                rr_header.ttl.to_presentation_format(out_buffer);
                rr_header.rclass.to_presentation_format(out_buffer);
                rtype.to_presentation_format(out_buffer);
                match self {
                    $(Self::$record(_, rdata) => gen_to_presentation!($record, rtype, rdata, out_buffer, $presentation_rule),)+
                }
            }
        }
    };
}

macro_rules! gen_to_presentation {
    ($record:ident, $rtype_var:expr, $rdata_var:expr, $out_buffer_var:expr, presentation_forbidden) => { panic!("Cannot convert {} to presentation", $rtype_var) };
    ($record:ident, $rtype_var:expr, $rdata_var:expr, $out_buffer_var:expr, presentation_allowed) => { $rdata_var.to_presentation_format($out_buffer_var) };
}

macro_rules! gen_from_presentation {
    ($record:ident, $rtype_var:expr, $rr_header_var:expr, $record_var:expr, presentation_forbidden) => { return Err(TokenizedRecordError::RTypeNotAllowed($rtype_var)) };
    ($record:ident, $rtype_var:expr, $rr_header_var:expr, $record_var:expr, presentation_allowed) => { Self::$record($rr_header_var, <$record>::from_tokenized_rdata(&$record_var.rdata)?) };
}

gen_resource_record!(
    // Unknown(RRHeader, RType, Unknown),
    (A, presentation_allowed),
    (A6, presentation_allowed),
    (AAAA, presentation_allowed),
    (AFSDB, presentation_allowed),
    (AMTRELAY, presentation_allowed),
    (ANY, presentation_forbidden),
    (APL, presentation_allowed),
    // ATMA(RRHeader, ATMA),
    // AVC(RRHeader, AVC),
    (AXFR, presentation_forbidden),
    (CAA, presentation_allowed),
    // CDNSKEY(RRHeader, CDNSKEY),
    // CDS(RRHeader, CDS),
    (CERT, presentation_allowed),
    (CNAME, presentation_allowed),
    // CSYNC(RRHeader, CSYNC),
    // DHCID(RRHeader, DHCID),
    // DLV(RRHeader, DLV),
    (DNAME, presentation_allowed),
    (DNSKEY, presentation_allowed),
    // DOA(RRHeader, DOA),
    // DS(RRHeader, DS),
    // EID(RRHeader, EID),
    // EUI48(RRHeader, EUI48),
    // EUI64(RRHeader, EUI64),
    // GID(RRHeader, GID),
    // GPOS(RRHeader, GPOS),
    (HINFO, presentation_allowed),
    // HIP(RRHeader, HIP),
    // HTTPS(RRHeader, HTTPS),
    // IPSECKEY(RRHeader, IPSECKEY),
    // ISDN(RRHeader, ISDN),
    // IXFR(RRHeader, IXFR),
    // KEY(RRHeader, KEY),
    // KX(RRHeader, KX),
    // L32(RRHeader, L32),
    // L64(RRHeader, L64),
    // LOC(RRHeader, LOC),
    // LP(RRHeader, LP),
    (MAILA, presentation_forbidden),
    (MAILB, presentation_forbidden),
    (MB, presentation_allowed),
    (MD, presentation_allowed),
    (MF, presentation_allowed),
    (MG, presentation_allowed),
    (MINFO, presentation_allowed),
    (MR, presentation_allowed),
    (MX, presentation_allowed),
    // NAPTR(RRHeader, NAPTR),
    // NID(RRHeader, NID),
    // NIMLOC(RRHeader, NIMLOC),
    // NINFO(RRHeader, NINFO),
    (NS, presentation_allowed),
    // NSAP_PTR(RRHeader, NSAP_PTR),
    // NSAP(RRHeader, NSAP),
    (NSEC, presentation_allowed),
    // NSEC3(RRHeader, NSEC3),
    // NSEC3PARAM(RRHeader, NSEC3PARAM),
    (NULL, presentation_forbidden),
    // NXT(RRHeader, NXT),
    // OPENPGPKEY(RRHeader, OPENPGPKEY),
    // OPT(RRHeader, OPT),
    (PTR, presentation_allowed),
    // PX(RRHeader, PX),
    // RKEY(RRHeader, RKEY),
    // RP(RRHeader, RP),
    (RRSIG, presentation_allowed),
    // RT(RRHeader, RT),
    // SIG(RRHeader, SIG),
    // SINK(RRHeader, SINK),
    // SMIMEA(RRHeader, SMIMEA),
    (SOA, presentation_allowed),
    // SPF(RRHeader, SPF),
    // SRV(RRHeader, SRV),
    // SSHFP(RRHeader, SSHFP),
    // SVCB(RRHeader, SVCB),
    // TA(RRHeader, TA),
    // TALINK(RRHeader, TALINK),
    // TKEY(RRHeader, TKEY),
    (TLSA, presentation_allowed),
    (TSIG, presentation_forbidden),
    (TXT, presentation_allowed),
    // UID(RRHeader, UID),
    // UINFO(RRHeader, UINFO),
    // UNSPEC(RRHeader, UNSPEC),
    // URI(RRHeader, URI),
    (WKS, presentation_allowed),
    // X25(RRHeader, X25),
    // ZONEMD(RRHeader, ZONEMD),
);

impl Display for ResourceRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut buffer = Vec::new();
        self.to_presentation_format(&mut buffer);
        write!(f, "{}", buffer.join("\t"))
    }
}

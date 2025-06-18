use std::{error::Error, fmt::Display, hash::Hash, ops::Deref};

use crate::{
    serde::{
        presentation::{
            errors::TokenizedRecordError, from_presentation::FromPresentation,
            from_tokenized_rdata::FromTokenizedRData, to_presentation::ToPresentation,
        },
        wire::{
            from_wire::FromWire,
            read_wire::{ReadWireError, SliceWireVisibility},
            to_wire::ToWire,
        },
    },
    types::c_domain_name::CDomainName,
};

use super::{
    rclass::RClass,
    rtype::RType,
    time::Time,
    types::{
        a::A, a6::A6, aaaa::AAAA, afsdb::AFSDB, amtrelay::AMTRELAY, any::ANY, apl::APL, axfr::AXFR,
        caa::CAA, cdnskey::CDNSKEY, cds::CDS, cert::CERT, cname::CNAME, csync::CSYNC, dname::DNAME,
        dnskey::DNSKEY, ds::DS, eui48::EUI48, eui64::EUI64, hinfo::HINFO, maila::MAILA,
        mailb::MAILB, mb::MB, md::MD, mf::MF, mg::MG, minfo::MINFO, mr::MR, mx::MX, naptr::NAPTR,
        ns::NS, nsec::NSEC, null::NULL, ptr::PTR, rrsig::RRSIG, soa::SOA, srv::SRV, tlsa::TLSA,
        tsig::TSIG, txt::TXT, wks::WKS,
    },
};

#[derive(Debug)]
pub enum TryFromResourceRecordError {
    UnexpectedRType { expected: RType, actual: RType },
}
impl Error for TryFromResourceRecordError {}
impl Display for TryFromResourceRecordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnexpectedRType { expected, actual } => write!(
                f,
                "Expected Resource Record Type {expected} but was {actual}"
            ),
        }
    }
}

pub trait RData: ToWire + PartialEq + Clone + Hash {
    fn get_rtype(&self) -> RType;
}

#[derive(Debug, Clone)]
pub struct ResourceRecord<RDataT: RData = RecordData> {
    name: CDomainName,
    rclass: RClass,
    ttl: Time,
    rdata: RDataT,
}

impl<RDataT: RData> ResourceRecord<RDataT> {
    #[inline]
    pub const fn new(name: CDomainName, rclass: RClass, ttl: Time, rdata: RDataT) -> Self {
        Self {
            name,
            rclass,
            ttl,
            rdata,
        }
    }

    #[inline]
    pub const fn get_name(&self) -> &CDomainName {
        &self.name
    }

    #[inline]
    pub fn into_name(self) -> CDomainName {
        self.name
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
    pub fn into_ttl(self) -> Time {
        self.ttl
    }

    #[inline]
    pub const fn set_ttl(&mut self, new_ttl: Time) {
        self.ttl = new_ttl;
    }

    #[inline]
    pub fn get_rtype(&self) -> RType {
        self.rdata.get_rtype()
    }

    #[inline]
    pub const fn get_rdata(&self) -> &RDataT {
        &self.rdata
    }

    #[inline]
    pub fn into_rdata(self) -> RDataT {
        self.rdata
    }
}

impl<RDataT: RData> Deref for ResourceRecord<RDataT> {
    type Target = RDataT;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.get_rdata()
    }
}

impl<RDataT: RData> PartialEq for ResourceRecord<RDataT> {
    /// A less strict version of equality. If two records match, that means that
    /// the records have the following equalities:
    ///
    ///  1. `name`
    ///  2. `rclass`
    ///  3. `rdata`
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        (self.name == other.name) && (self.rclass == other.rclass) && (self.rdata == other.rdata)
    }
}

impl<RDataT: RData> Hash for ResourceRecord<RDataT> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.rclass.hash(state);
        self.rdata.hash(state);
    }
}

impl<RDataT: RData> ToWire for ResourceRecord<RDataT> {
    fn to_wire_format<'a, 'b>(
        &self,
        wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>,
        compression: &mut Option<crate::types::c_domain_name::CompressionMap>,
    ) -> Result<(), crate::serde::wire::write_wire::WriteWireError>
    where
        'a: 'b,
    {
        self.name.to_wire_format(wire, compression)?;
        self.rdata.get_rtype().to_wire_format(wire, compression)?;
        self.rclass.to_wire_format(wire, compression)?;
        self.ttl.to_wire_format(wire, compression)?;

        // If any compression is done, we may need to modify the rd_length.
        let rd_length_offset = wire.current_len();
        0_u16.to_wire_format(wire, compression)?;

        let rdata_offset = wire.current_len();
        self.rdata.to_wire_format(wire, compression)?;

        // Replace the rd_length with the actual number of bytes that got written. This way,
        // even if it got compressed, it will be accurate.
        let actual_rd_length = (wire.current_len() - rdata_offset) as u16;
        wire.write_bytes_at(&actual_rd_length.to_be_bytes(), rd_length_offset)?;

        Ok(())
    }

    fn serial_length(&self) -> u16 {
        let rd_length = self.rdata.serial_length();
        self.name.serial_length()
            + self.rdata.get_rtype().serial_length()
            + self.rclass.serial_length()
            + self.ttl.serial_length()
            + rd_length.serial_length()
            + rd_length
    }
}

macro_rules! gen_record_data {
    ($(($record:ident, $presentation_rule:ident)),+$(,)?) => {
        /// https://datatracker.ietf.org/doc/html/rfc1035#section-4.1.3
        #[derive(Clone, PartialEq, Eq, Hash, Debug)]
        pub enum RecordData {
            $($record($record),)+
        }

        impl RData for RecordData {
            #[inline]
            fn get_rtype(&self) -> RType {
                match self {
                    $(Self::$record(_) => RType::$record,)+
                }
            }
        }

        impl ToWire for RecordData {
            fn to_wire_format<'a, 'b>(&self, wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>, compression: &mut Option<crate::types::c_domain_name::CompressionMap>) -> Result<(), crate::serde::wire::write_wire::WriteWireError> where 'a: 'b {
                match self {
                    $(Self::$record(rdata) => rdata.to_wire_format(wire, compression),)+
                }
            }

            fn serial_length(&self) -> u16 {
                match self {
                    $(Self::$record(rdata) => rdata.serial_length(),)+
                }
            }
        }

        impl FromWire for ResourceRecord<RecordData> {
            fn from_wire_format<'a, 'b>(wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>) -> Result<Self, ReadWireError> where Self: Sized, 'a: 'b {
                let name = CDomainName::from_wire_format(wire)?;
                let rtype = RType::from_wire_format(wire)?;
                let rclass = RClass::from_wire_format(wire)?;
                let ttl = Time::from_wire_format(wire)?;
                let wire_rd_length = u16::from_wire_format(wire)?;

                // The lower bound is None because it needs access to the previous parts of the wire when
                // de-referencing the domain name pointers. No pointer should point past the end of the
                // rdata section (forward pointers) for domain name compression so blocking off the end
                // should not cause any problems when decompressing. An upper bound is required to prevent
                // any of the deserializers that fully consume the rdata section from continuing past the
                // end.
                let mut rdata_wire = wire.slice_from_current(..(wire_rd_length as usize), SliceWireVisibility::Entire)?;
                let rdata = match &rtype {
                    $(RType::$record => RecordData::$record(<$record>::from_wire_format(&mut rdata_wire)?),)+
                    _ => return Err(ReadWireError::UnsupportedRType(rtype)),
                };

                // The true size might be different than the expected size due to factors such as
                // domain name decompression.
                let rd_length = rdata.serial_length();
                if rd_length > u16::MAX {
                    return Err(ReadWireError::OverflowError(
                        format!("Expected rd_length to be at most {0} bytes. rd_length is actually {1}", u16::MAX, rd_length)
                    ));
                }

                wire.shift(wire_rd_length as usize)?;

                Ok(Self { name, rclass, ttl, rdata })
            }
        }

        $(
            impl FromWire for ResourceRecord<$record> {
                fn from_wire_format<'a, 'b>(wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>) -> Result<Self, ReadWireError> where Self: Sized, 'a: 'b {
                    let name = CDomainName::from_wire_format(wire)?;
                    let rtype = RType::from_wire_format(wire)?;
                    if rtype != RType::$record {
                        return Err(ReadWireError::UnexpectedRType{
                            expected: RType::$record,
                            actual: rtype
                        });
                    }
                    let rclass = RClass::from_wire_format(wire)?;
                    let ttl = Time::from_wire_format(wire)?;
                    // We will not store the `wire_rd_length`, instead, we will recalculate it since things like
                    // domain name decompression could cause it to change.
                    let wire_rd_length = u16::from_wire_format(wire)?;

                    // The lower bound is None because it needs access to the previous parts of the wire when
                    // de-referencing the domain name pointers. No pointer should point past the end of the
                    // rdata section (forward pointers) for domain name compression so blocking off the end
                    // should not cause any problems when decompressing. An upper bound is required to prevent
                    // any of the deserializers that fully consume the rdata section from continuing past the
                    // end.
                    let mut rdata_wire = wire.slice_from_current(..(wire_rd_length as usize), SliceWireVisibility::Entire)?;
                    let rdata = <$record>::from_wire_format(&mut rdata_wire)?;

                    // The true size might be different than the expected size due to factors such as
                    // domain name decompression.
                    let rd_length = rdata.serial_length();
                    if rd_length > u16::MAX {
                        return Err(ReadWireError::OverflowError(
                            format!("Expected rd_length to be at most {0} bytes. rd_length is actually {1}", u16::MAX, rd_length)
                        ));
                    }

                    wire.shift(wire_rd_length as usize)?;

                    Ok(Self { name, rclass, ttl, rdata })
                }
            }
        )+

        impl ResourceRecord<RecordData> {
            pub fn from_tokenized_record<'a>(record: &crate::serde::presentation::tokenizer::tokenizer::ResourceRecordToken<'a>) -> Result<Self, TokenizedRecordError> where Self: Sized {
                let name = CDomainName::from_token_format(&[record.domain_name])?.0;
                let rclass = RClass::from_token_format(&[record.rclass])?.0;
                let ttl = Time::from_token_format(&[record.ttl])?.0;

                let (rtype, _) = RType::from_token_format(&[record.rtype])?;
                let record = match rtype {
                    $(RType::$record => gen_from_presentation!($record, rtype, name, rclass, ttl, record, $presentation_rule),)+
                    _ => return Err(TokenizedRecordError::UnsupportedRType(rtype)),
                };

                Ok(record)
            }
        }

        impl ToPresentation for ResourceRecord<RecordData> {
            fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
                let rtype = self.get_rtype();
                self.name.to_presentation_format(out_buffer);
                self.ttl.to_presentation_format(out_buffer);
                self.rclass.to_presentation_format(out_buffer);
                rtype.to_presentation_format(out_buffer);
                match &self.rdata {
                    $(RecordData::$record(rdata) => gen_to_presentation!($record, rtype, rdata, out_buffer, $presentation_rule),)+
                }
            }
        }

        $(resource_record_to_presentation!($record, $presentation_rule);)+

        impl Display for ResourceRecord<RecordData> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let mut buffer = Vec::new();
                self.to_presentation_format(&mut buffer);
                write!(f, "{}", buffer.join("\t"))
            }
        }

        $(resource_record_gen_display!($record, $presentation_rule);)+

        $(
            impl From<ResourceRecord<$record>> for ResourceRecord<RecordData> {
                fn from(rr: ResourceRecord<$record>) -> Self {
                    Self {
                        name: rr.name,
                        rclass: rr.rclass,
                        ttl: rr.ttl,
                        rdata: RecordData::$record(rr.rdata),
                    }
                }
            }

            impl From<&ResourceRecord<$record>> for ResourceRecord<RecordData> {
                fn from(rr: &ResourceRecord<$record>) -> Self {
                    Self {
                        name: rr.name.clone(),
                        rclass: rr.rclass,
                        ttl: rr.ttl,
                        rdata: RecordData::$record(rr.rdata.clone()),
                    }
                }
            }
        )+

        $(
            impl TryFrom<ResourceRecord<RecordData>> for ResourceRecord<$record> {
                type Error = TryFromResourceRecordError;

                fn try_from(rr: ResourceRecord<RecordData>) -> Result<Self, Self::Error> {
                    match rr.rdata {
                        RecordData::$record(rdata) => {
                            Ok(Self {
                                name: rr.name,
                                rclass: rr.rclass,
                                ttl: rr.ttl,
                                rdata,
                            })
                        },
                        rdata => {
                            Err(TryFromResourceRecordError::UnexpectedRType {
                                expected: RType::$record,
                                actual: rdata.get_rtype(),
                            })
                        }
                    }
                }
            }

            impl TryFrom<&ResourceRecord<RecordData>> for ResourceRecord<$record> {
                type Error = TryFromResourceRecordError;

                fn try_from(rr: &ResourceRecord<RecordData>) -> Result<Self, Self::Error> {
                    match &rr.rdata {
                        RecordData::$record(rdata) => {
                            Ok(Self {
                                name: rr.name.clone(),
                                rclass: rr.rclass,
                                ttl: rr.ttl,
                                rdata: rdata.clone(),
                            })
                        },
                        rdata => {
                            Err(TryFromResourceRecordError::UnexpectedRType {
                                expected: RType::$record,
                                actual: rdata.get_rtype(),
                            })
                        }
                    }
                }
            }
        )+
    };
}

macro_rules! gen_to_presentation {
    ($record:ident, $rtype_var:expr, $rdata_var:expr, $out_buffer_var:expr, presentation_forbidden) => {{
        // clears the warning generated because the `rdata` field is unused.
        let _ = $rdata_var;

        panic!("Cannot convert {} to presentation", $rtype_var);
    }};
    ($record:ident, $rtype_var:expr, $rdata_var:expr, $out_buffer_var:expr, presentation_allowed) => {
        $rdata_var.to_presentation_format($out_buffer_var)
    };
}

macro_rules! resource_record_to_presentation {
    ($record:ident, presentation_forbidden) => {
        // No presentation format
    };
    ($record:ident, presentation_allowed) => {
        impl ToPresentation for ResourceRecord<$record> {
            fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
                let rtype = self.get_rtype();
                self.name.to_presentation_format(out_buffer);
                self.ttl.to_presentation_format(out_buffer);
                self.rclass.to_presentation_format(out_buffer);
                rtype.to_presentation_format(out_buffer);
                self.rdata.to_presentation_format(out_buffer);
            }
        }
    };
}

macro_rules! resource_record_gen_display {
    ($record:ident, presentation_forbidden) => {
        // No presentation format
    };
    ($record:ident, presentation_allowed) => {
        impl Display for ResourceRecord<$record> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let mut buffer = Vec::new();
                self.rdata.to_presentation_format(&mut buffer);
                write!(f, "{}", buffer.join("\t"))
            }
        }
    };
}

macro_rules! gen_from_presentation {
    ($record:ident, $rtype_var:expr, $name_var:expr, $rclass_var:expr, $ttl_var:expr, $record_var:expr, presentation_forbidden) => {
        return Err(TokenizedRecordError::RTypeNotAllowed($rtype_var))
    };
    ($record:ident, $rtype_var:expr, $name_var:expr, $rclass_var:expr, $ttl_var:expr, $record_var:expr, presentation_allowed) => {
        Self {
            name: $name_var,
            rclass: $rclass_var,
            ttl: $ttl_var,
            rdata: RecordData::$record(<$record>::from_tokenized_rdata(&$record_var.rdata)?),
        }
    };
}

gen_record_data!(
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
    (CDNSKEY, presentation_allowed),
    (CDS, presentation_allowed),
    (CERT, presentation_allowed),
    (CNAME, presentation_allowed),
    (CSYNC, presentation_allowed),
    // DHCID(RRHeader, DHCID),
    // DLV(RRHeader, DLV),
    (DNAME, presentation_allowed),
    (DNSKEY, presentation_allowed),
    // DOA(RRHeader, DOA),
    (DS, presentation_allowed),
    // EID(RRHeader, EID),
    (EUI48, presentation_allowed),
    (EUI64, presentation_allowed),
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
    (NAPTR, presentation_allowed),
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
    (SRV, presentation_allowed),
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

use std::fmt::Display;

use crate::{types::c_domain_name::CDomainName, serde::{wire::{to_wire::ToWire, from_wire::FromWire, read_wire::ReadWireError}, presentation::{from_tokenized_record::FromTokenizedRecord, from_presentation::FromPresentation, errors::TokenizedRecordError, to_presentation::ToPresentation}}};

use super::{rclass::RClass, types::{a::A, aaaa::AAAA, any::ANY, axfr::AXFR, cname::CNAME, dname::DNAME, hinfo::HINFO, maila::MAILA, mailb::MAILB, mb::MB, md::MD, mf::MF, mg::MG, minfo::MINFO, mr::MR, mx::MX, ns::NS, null::NULL, soa::SOA, txt::TXT}, rtype::RType, time::Time};

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

/// https://datatracker.ietf.org/doc/html/rfc1035#section-4.1.3
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum ResourceRecord {
    // Unknown(RRHeader, RType, Unknown),
    A(RRHeader, A),
    // A6(RRHeader, A6),
    AAAA(RRHeader, AAAA),
    // AFSDB(RRHeader, AFSDB),
    // AMTRELAY(RRHeader, AMTRELAY),
    ANY(RRHeader, ANY),
    // APL(RRHeader, APL),
    // ATMA(RRHeader, ATMA),
    // AVC(RRHeader, AVC),
    AXFR(RRHeader, AXFR),
    // CAA(RRHeader, CAA),
    // CDNSKEY(RRHeader, CDNSKEY),
    // CDS(RRHeader, CDS),
    // CERT(RRHeader, CERT),
    CNAME(RRHeader, CNAME),
    // CSYNC(RRHeader, CSYNC),
    // DHCID(RRHeader, DHCID),
    // DLV(RRHeader, DLV),
    DNAME(RRHeader, DNAME),
    // DNSKEY(RRHeader, DNSKEY),
    // DOA(RRHeader, DOA),
    // DS(RRHeader, DS),
    // EID(RRHeader, EID),
    // EUI48(RRHeader, EUI48),
    // EUI64(RRHeader, EUI64),
    // GID(RRHeader, GID),
    // GPOS(RRHeader, GPOS),
    HINFO(RRHeader, HINFO),
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
    MAILA(RRHeader, MAILA),
    MAILB(RRHeader, MAILB),
    MB(RRHeader, MB),
    MD(RRHeader, MD),
    MF(RRHeader, MF),
    MG(RRHeader, MG),
    MINFO(RRHeader, MINFO),
    MR(RRHeader, MR),
    MX(RRHeader, MX),
    // NAPTR(RRHeader, NAPTR),
    // NID(RRHeader, NID),
    // NIMLOC(RRHeader, NIMLOC),
    // NINFO(RRHeader, NINFO),
    NS(RRHeader, NS),
    // NSAP_PTR(RRHeader, NSAP_PTR),
    // NSAP(RRHeader, NSAP),
    // NSEC(RRHeader, NSEC),
    // NSEC3(RRHeader, NSEC3),
    // NSEC3PARAM(RRHeader, NSEC3PARAM),
    NULL(RRHeader, NULL),
    // NXT(RRHeader, NXT),
    // OPENPGPKEY(RRHeader, OPENPGPKEY),
    // OPT(RRHeader, OPT),
    // PTR(RRHeader, PTR),
    // PX(RRHeader, PX),
    // RKEY(RRHeader, RKEY),
    // RP(RRHeader, RP),
    // RRSIG(RRHeader, RRSIG),
    // RT(RRHeader, RT),
    // SIG(RRHeader, SIG),
    // SINK(RRHeader, SINK),
    // SMIMEA(RRHeader, SMIMEA),
    SOA(RRHeader, SOA),
    // SPF(RRHeader, SPF),
    // SRV(RRHeader, SRV),
    // SSHFP(RRHeader, SSHFP),
    // SVCB(RRHeader, SVCB),
    // TA(RRHeader, TA),
    // TALINK(RRHeader, TALINK),
    // TKEY(RRHeader, TKEY),
    // TLSA(RRHeader, TLSA),
    // TSIG(RRHeader, TSIG),
    TXT(RRHeader, TXT),
    // UID(RRHeader, UID),
    // UINFO(RRHeader, UINFO),
    // UNSPEC(RRHeader, UNSPEC),
    // URI(RRHeader, URI),
    // WKS(RRHeader, WKS),
    // X25(RRHeader, X25),
    // ZONEMD(RRHeader, ZONEMD),
}

impl Display for ResourceRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut buffer = Vec::new();
        self.to_presentation_format(&mut buffer);
        write!(f, "{}", buffer.join("\t"))
    }
}

impl ResourceRecord {
    #[inline]
    const fn header_and_rtype(&self) -> (&RRHeader, RType) {
        match self {
            // Self::Unknown(header, rtype, _) => (header, *rtype),
            Self::A(header, _) => (header, RType::A),
            // Self::A6(header, _) => (header, RType::A6),
            Self::AAAA(header, _) => (header, RType::AAAA),
            // Self::AFSDB(header, _) => (header, RType::AFSDB),
            // Self::AMTRELAY(header, _) => (header, RType::AMTRELAY),
            // Self::APL(header, _) => (header, RType::APL),
            // Self::ATMA(header, _) => (header, RType::ATMA),
            // Self::AVC(header, _) => (header, RType::AVC),
            Self::AXFR(header, _) => (header, RType::AXFR),
            // Self::CAA(header, _) => (header, RType::CAA),
            // Self::CDNSKEY(header, _) => (header, RType::CDNSKEY),
            // Self::CDS(header, _) => (header, RType::CDS),
            // Self::CERT(header, _) => (header, RType::CERT),
            Self::CNAME(header, _) => (header, RType::CNAME),
            // Self::CSYNC(header, _) => (header, RType::CSYNC),
            // Self::DHCID(header, _) => (header, RType::DHCID),
            // Self::DLV(header, _) => (header, RType::DLV),
            Self::DNAME(header, _) => (header, RType::DNAME),
            // Self::DNSKEY(header, _) => (header, RType::DNSKEY),
            // Self::DOA(header, _) => (header, RType::DOA),
            // Self::DS(header, _) => (header, RType::DS),
            // Self::EID(header, _) => (header, RType::EID),
            // Self::EUI48(header, _) => (header, RType::EUI48),
            // Self::EUI64(header, _) => (header, RType::EUI64),
            // Self::GID(header, _) => (header, RType::GID),
            // Self::GPOS(header, _) => (header, RType::GPOS),
            Self::HINFO(header, _) => (header, RType::HINFO),
            // Self::HIP(header, _) => (header, RType::HIP),
            // Self::HTTPS(header, _) => (header, RType::HTTPS),
            // Self::IPSECKEY(header, _) => (header, RType::IPSECKEY),
            // Self::ISDN(header, _) => (header, RType::ISDN),
            // Self::IXFR(header, _) => (header, RType::IXFR),
            // Self::KEY(header, _) => (header, RType::KEY),
            // Self::KX(header, _) => (header, RType::KX),
            // Self::L32(header, _) => (header, RType::L32),
            // Self::L64(header, _) => (header, RType::L64),
            // Self::LOC(header, _) => (header, RType::LOC),
            // Self::LP(header, _) => (header, RType::LP),
            Self::MAILA(header, _) => (header, RType::MAILA),
            Self::MAILB(header, _) => (header, RType::MAILB),
            Self::MB(header, _) => (header, RType::MB),
            Self::MD(header, _) => (header, RType::MD),
            Self::MF(header, _) => (header, RType::MF),
            Self::MG(header, _) => (header, RType::MG),
            Self::MINFO(header, _) => (header, RType::MINFO),
            Self::MR(header, _) => (header, RType::MR),
            Self::MX(header, _) => (header, RType::MX),
            // Self::NAPTR(header, _) => (header, RType::NAPTR),
            // Self::NID(header, _) => (header, RType::NID),
            // Self::NIMLOC(header, _) => (header, RType::NIMLOC),
            // Self::NINFO(header, _) => (header, RType::NINFO),
            Self::NS(header, _) => (header, RType::NS),
            // Self::NSAP_PTR(header, _) => (header, RType::NSAP_PTR),
            // Self::NSAP(header, _) => (header, RType::NSAP),
            // Self::NSEC(header, _) => (header, RType::NSEC),
            // Self::NSEC3(header, _) => (header, RType::NSEC3),
            // Self::NSEC3PARAM(header, _) => (header, RType::NSEC3PARAM),
            Self::NULL(header, _) => (header, RType::NULL),
            // Self::NXT(header, _) => (header, RType::NXT),
            // Self::OPENPGPKEY(header, _) => (header, RType::OPENPGPKEY),
            // Self::OPT(header, _) => (header, RType::OPT),
            // Self::PTR(header, _) => (header, RType::PTR),
            // Self::PX(header, _) => (header, RType::PX),
            // Self::RKEY(header, _) => (header, RType::RKEY),
            // Self::RP(header, _) => (header, RType::RP),
            // Self::RRSIG(header, _) => (header, RType::RRSIG),
            // Self::RT(header, _) => (header, RType::RT),
            // Self::SIG(header, _) => (header, RType::SIG),
            // Self::SINK(header, _) => (header, RType::SINK),
            // Self::SMIMEA(header, _) => (header, RType::SMIMEA),
            Self::SOA(header, _) => (header, RType::SOA),
            // Self::SPF(header, _) => (header, RType::SPF),
            // Self::SRV(header, _) => (header, RType::SRV),
            // Self::SSHFP(header, _) => (header, RType::SSHFP),
            Self::ANY(header, _) => (header, RType::ANY),
            // Self::SVCB(header, _) => (header, RType::SVCB),
            // Self::TA(header, _) => (header, RType::TA),
            // Self::TALINK(header, _) => (header, RType::TALINK),
            // Self::TKEY(header, _) => (header, RType::TKEY),
            // Self::TLSA(header, _) => (header, RType::TLSA),
            // Self::TSIG(header, _) => (header, RType::TSIG),
            Self::TXT(header, _) => (header, RType::TXT),
            // Self::UID(header, _) => (header, RType::UID),
            // Self::UINFO(header, _) => (header, RType::UINFO),
            // Self::UNSPEC(header, _) => (header, RType::UNSPEC),
            // Self::URI(header, _) => (header, RType::URI),
            // Self::WKS(header, _) => (header, RType::WKS),
            // Self::X25(header, _) => (header, RType::X25),
            // Self::ZONEMD(header, _) => (header, RType::ZONEMD),
        }
    }

    #[inline]
    fn mut_header_and_rtype(&mut self) -> (&mut RRHeader, RType) {
        match self {
            // Self::Unknown(header, rtype, _) => (header, *rtype),
            Self::A(header, _) => (header, RType::A),
            // Self::A6(header, _) => (header, RType::A6),
            Self::AAAA(header, _) => (header, RType::AAAA),
            // Self::AFSDB(header, _) => (header, RType::AFSDB),
            // Self::AMTRELAY(header, _) => (header, RType::AMTRELAY),
            // Self::APL(header, _) => (header, RType::APL),
            // Self::ATMA(header, _) => (header, RType::ATMA),
            // Self::AVC(header, _) => (header, RType::AVC),
            Self::AXFR(header, _) => (header, RType::AXFR),
            // Self::CAA(header, _) => (header, RType::CAA),
            // Self::CDNSKEY(header, _) => (header, RType::CDNSKEY),
            // Self::CDS(header, _) => (header, RType::CDS),
            // Self::CERT(header, _) => (header, RType::CERT),
            Self::CNAME(header, _) => (header, RType::CNAME),
            // Self::CSYNC(header, _) => (header, RType::CSYNC),
            // Self::DHCID(header, _) => (header, RType::DHCID),
            // Self::DLV(header, _) => (header, RType::DLV),
            Self::DNAME(header, _) => (header, RType::DNAME),
            // Self::DNSKEY(header, _) => (header, RType::DNSKEY),
            // Self::DOA(header, _) => (header, RType::DOA),
            // Self::DS(header, _) => (header, RType::DS),
            // Self::EID(header, _) => (header, RType::EID),
            // Self::EUI48(header, _) => (header, RType::EUI48),
            // Self::EUI64(header, _) => (header, RType::EUI64),
            // Self::GID(header, _) => (header, RType::GID),
            // Self::GPOS(header, _) => (header, RType::GPOS),
            Self::HINFO(header, _) => (header, RType::HINFO),
            // Self::HIP(header, _) => (header, RType::HIP),
            // Self::HTTPS(header, _) => (header, RType::HTTPS),
            // Self::IPSECKEY(header, _) => (header, RType::IPSECKEY),
            // Self::ISDN(header, _) => (header, RType::ISDN),
            // Self::IXFR(header, _) => (header, RType::IXFR),
            // Self::KEY(header, _) => (header, RType::KEY),
            // Self::KX(header, _) => (header, RType::KX),
            // Self::L32(header, _) => (header, RType::L32),
            // Self::L64(header, _) => (header, RType::L64),
            // Self::LOC(header, _) => (header, RType::LOC),
            // Self::LP(header, _) => (header, RType::LP),
            Self::MAILA(header, _) => (header, RType::MAILA),
            Self::MAILB(header, _) => (header, RType::MAILB),
            Self::MB(header, _) => (header, RType::MB),
            Self::MD(header, _) => (header, RType::MD),
            Self::MF(header, _) => (header, RType::MF),
            Self::MG(header, _) => (header, RType::MG),
            Self::MINFO(header, _) => (header, RType::MINFO),
            Self::MR(header, _) => (header, RType::MR),
            Self::MX(header, _) => (header, RType::MX),
            // Self::NAPTR(header, _) => (header, RType::NAPTR),
            // Self::NID(header, _) => (header, RType::NID),
            // Self::NIMLOC(header, _) => (header, RType::NIMLOC),
            // Self::NINFO(header, _) => (header, RType::NINFO),
            Self::NS(header, _) => (header, RType::NS),
            // Self::NSAP_PTR(header, _) => (header, RType::NSAP_PTR),
            // Self::NSAP(header, _) => (header, RType::NSAP),
            // Self::NSEC(header, _) => (header, RType::NSEC),
            // Self::NSEC3(header, _) => (header, RType::NSEC3),
            // Self::NSEC3PARAM(header, _) => (header, RType::NSEC3PARAM),
            Self::NULL(header, _) => (header, RType::NULL),
            // Self::NXT(header, _) => (header, RType::NXT),
            // Self::OPENPGPKEY(header, _) => (header, RType::OPENPGPKEY),
            // Self::OPT(header, _) => (header, RType::OPT),
            // Self::PTR(header, _) => (header, RType::PTR),
            // Self::PX(header, _) => (header, RType::PX),
            // Self::RKEY(header, _) => (header, RType::RKEY),
            // Self::RP(header, _) => (header, RType::RP),
            // Self::RRSIG(header, _) => (header, RType::RRSIG),
            // Self::RT(header, _) => (header, RType::RT),
            // Self::SIG(header, _) => (header, RType::SIG),
            // Self::SINK(header, _) => (header, RType::SINK),
            // Self::SMIMEA(header, _) => (header, RType::SMIMEA),
            Self::SOA(header, _) => (header, RType::SOA),
            // Self::SPF(header, _) => (header, RType::SPF),
            // Self::SRV(header, _) => (header, RType::SRV),
            // Self::SSHFP(header, _) => (header, RType::SSHFP),
            Self::ANY(header, _) => (header, RType::ANY),
            // Self::SVCB(header, _) => (header, RType::SVCB),
            // Self::TA(header, _) => (header, RType::TA),
            // Self::TALINK(header, _) => (header, RType::TALINK),
            // Self::TKEY(header, _) => (header, RType::TKEY),
            // Self::TLSA(header, _) => (header, RType::TLSA),
            // Self::TSIG(header, _) => (header, RType::TSIG),
            Self::TXT(header, _) => (header, RType::TXT),
            // Self::UID(header, _) => (header, RType::UID),
            // Self::UINFO(header, _) => (header, RType::UINFO),
            // Self::UNSPEC(header, _) => (header, RType::UNSPEC),
            // Self::URI(header, _) => (header, RType::URI),
            // Self::WKS(header, _) => (header, RType::WKS),
            // Self::X25(header, _) => (header, RType::X25),
            // Self::ZONEMD(header, _) => (header, RType::ZONEMD),
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
            // Self::Unknown(_, _, rdata) => rdata.serial_length(),
            Self::A(_, rdata) => rdata.serial_length(),
            // Self::A6(_, rdata) => rdata.serial_length(),
            Self::AAAA(_, rdata) => rdata.serial_length(),
            // Self::AFSDB(_, rdata) => rdata.serial_length(),
            // Self::AMTRELAY(_, rdata) => rdata.serial_length(),
            // Self::APL(_, rdata) => rdata.serial_length(),
            // Self::ATMA(_, rdata) => rdata.serial_length(),
            // Self::AVC(_, rdata) => rdata.serial_length(),
            Self::AXFR(_, rdata) => rdata.serial_length(),
            // Self::CAA(_, rdata) => rdata.serial_length(),
            // Self::CDNSKEY(_, rdata) => rdata.serial_length(),
            // Self::CDS(_, rdata) => rdata.serial_length(),
            // Self::CERT(_, rdata) => rdata.serial_length(),
            Self::CNAME(_, rdata) => rdata.serial_length(),
            // Self::CSYNC(_, rdata) => rdata.serial_length(),
            // Self::DHCID(_, rdata) => rdata.serial_length(),
            // Self::DLV(_, rdata) => rdata.serial_length(),
            Self::DNAME(_, rdata) => rdata.serial_length(),
            // Self::DNSKEY(_, rdata) => rdata.serial_length(),
            // Self::DOA(_, rdata) => rdata.serial_length(),
            // Self::DS(_, rdata) => rdata.serial_length(),
            // Self::EID(_, rdata) => rdata.serial_length(),
            // Self::EUI48(_, rdata) => rdata.serial_length(),
            // Self::EUI64(_, rdata) => rdata.serial_length(),
            // Self::GID(_, rdata) => rdata.serial_length(),
            // Self::GPOS(_, rdata) => rdata.serial_length(),
            Self::HINFO(_, rdata) => rdata.serial_length(),
            // Self::HIP(_, rdata) => rdata.serial_length(),
            // Self::HTTPS(_, rdata) => rdata.serial_length(),
            // Self::IPSECKEY(_, rdata) => rdata.serial_length(),
            // Self::ISDN(_, rdata) => rdata.serial_length(),
            // Self::IXFR(_, rdata) => rdata.serial_length(),
            // Self::KEY(_, rdata) => rdata.serial_length(),
            // Self::KX(_, rdata) => rdata.serial_length(),
            // Self::L32(_, rdata) => rdata.serial_length(),
            // Self::L64(_, rdata) => rdata.serial_length(),
            // Self::LOC(_, rdata) => rdata.serial_length(),
            // Self::LP(_, rdata) => rdata.serial_length(),
            Self::MAILA(_, rdata) => rdata.serial_length(),
            Self::MAILB(_, rdata) => rdata.serial_length(),
            Self::MB(_, rdata) => rdata.serial_length(),
            Self::MD(_, rdata) => rdata.serial_length(),
            Self::MF(_, rdata) => rdata.serial_length(),
            Self::MG(_, rdata) => rdata.serial_length(),
            Self::MINFO(_, rdata) => rdata.serial_length(),
            Self::MR(_, rdata) => rdata.serial_length(),
            Self::MX(_, rdata) => rdata.serial_length(),
            // Self::NAPTR(_, rdata) => rdata.serial_length(),
            // Self::NID(_, rdata) => rdata.serial_length(),
            // Self::NIMLOC(_, rdata) => rdata.serial_length(),
            // Self::NINFO(_, rdata) => rdata.serial_length(),
            Self::NS(_, rdata) => rdata.serial_length(),
            // Self::NSAP_PTR(_, rdata) => rdata.serial_length(),
            // Self::NSAP(_, rdata) => rdata.serial_length(),
            // Self::NSEC(_, rdata) => rdata.serial_length(),
            // Self::NSEC3(_, rdata) => rdata.serial_length(),
            // Self::NSEC3PARAM(_, rdata) => rdata.serial_length(),
            Self::NULL(_, rdata) => rdata.serial_length(),
            // Self::NXT(_, rdata) => rdata.serial_length(),
            // Self::OPENPGPKEY(_, rdata) => rdata.serial_length(),
            // Self::OPT(_, rdata) => rdata.serial_length(),
            // Self::PTR(_, rdata) => rdata.serial_length(),
            // Self::PX(_, rdata) => rdata.serial_length(),
            // Self::RKEY(_, rdata) => rdata.serial_length(),
            // Self::RP(_, rdata) => rdata.serial_length(),
            // Self::RRSIG(_, rdata) => rdata.serial_length(),
            // Self::RT(_, rdata) => rdata.serial_length(),
            // Self::SIG(_, rdata) => rdata.serial_length(),
            // Self::SINK(_, rdata) => rdata.serial_length(),
            // Self::SMIMEA(_, rdata) => rdata.serial_length(),
            Self::SOA(_, rdata) => rdata.serial_length(),
            // Self::SPF(_, rdata) => rdata.serial_length(),
            // Self::SRV(_, rdata) => rdata.serial_length(),
            // Self::SSHFP(_, rdata) => rdata.serial_length(),
            Self::ANY(_, rdata) => rdata.serial_length(),
            // Self::SVCB(_, rdata) => rdata.serial_length(),
            // Self::TA(_, rdata) => rdata.serial_length(),
            // Self::TALINK(_, rdata) => rdata.serial_length(),
            // Self::TKEY(_, rdata) => rdata.serial_length(),
            // Self::TLSA(_, rdata) => rdata.serial_length(),
            // Self::TSIG(_, rdata) => rdata.serial_length(),
            Self::TXT(_, rdata) => rdata.serial_length(),
            // Self::UID(_, rdata) => rdata.serial_length(),
            // Self::UINFO(_, rdata) => rdata.serial_length(),
            // Self::UNSPEC(_, rdata) => rdata.serial_length(),
            // Self::URI(_, rdata) => rdata.serial_length(),
            // Self::WKS(_, rdata) => rdata.serial_length(),
            // Self::X25(_, rdata) => rdata.serial_length(),
            // Self::ZONEMD(_, rdata) => rdata.serial_length(),
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
            // (Self::Unknown(self_header, self_rtype, self_rdata), Self::Unknown(other_header, other_rtype, other_rdata)) => (self_rtype == other_rtype) && (self_header.matches(other_header)) && (self_rdata == other_rdata),
            (Self::A(self_header, self_rdata), Self::A(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::A6(self_header, self_rdata), Self::A6(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            (Self::AAAA(self_header, self_rdata), Self::AAAA(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::AFSDB(self_header, self_rdata), Self::AFSDB(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::AMTRELAY(self_header, self_rdata), Self::AMTRELAY(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::APL(self_header, self_rdata), Self::APL(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::ATMA(self_header, self_rdata), Self::ATMA(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::AVC(self_header, self_rdata), Self::AVC(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            (Self::AXFR(self_header, self_rdata), Self::AXFR(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::CAA(self_header, self_rdata), Self::CAA(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::CDNSKEY(self_header, self_rdata), Self::CDNSKEY(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::CDS(self_header, self_rdata), Self::CDS(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::CERT(self_header, self_rdata), Self::CERT(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            (Self::CNAME(self_header, self_rdata), Self::CNAME(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::CSYNC(self_header, self_rdata), Self::CSYNC(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::DHCID(self_header, self_rdata), Self::DHCID(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::DLV(self_header, self_rdata), Self::DLV(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            (Self::DNAME(self_header, self_rdata), Self::DNAME(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::DNSKEY(self_header, self_rdata), Self::DNSKEY(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::DOA(self_header, self_rdata), Self::DOA(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::DS(self_header, self_rdata), Self::DS(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::EID(self_header, self_rdata), Self::EID(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::EUI48(self_header, self_rdata), Self::EUI48(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::EUI64(self_header, self_rdata), Self::EUI64(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::GID(self_header, self_rdata), Self::GID(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::GPOS(self_header, self_rdata), Self::GPOS(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            (Self::HINFO(self_header, self_rdata), Self::HINFO(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::HIP(self_header, self_rdata), Self::HIP(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::HTTPS(self_header, self_rdata), Self::HTTPS(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::IPSECKEY(self_header, self_rdata), Self::IPSECKEY(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::ISDN(self_header, self_rdata), Self::ISDN(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::IXFR(self_header, self_rdata), Self::IXFR(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::KEY(self_header, self_rdata), Self::KEY(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::KX(self_header, self_rdata), Self::KX(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::L32(self_header, self_rdata), Self::L32(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::L64(self_header, self_rdata), Self::L64(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::LOC(self_header, self_rdata), Self::LOC(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::LP(self_header, self_rdata), Self::LP(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            (Self::MAILA(self_header, self_rdata), Self::MAILA(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            (Self::MAILB(self_header, self_rdata), Self::MAILB(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            (Self::MB(self_header, self_rdata), Self::MB(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            (Self::MD(self_header, self_rdata), Self::MD(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            (Self::MF(self_header, self_rdata), Self::MF(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            (Self::MG(self_header, self_rdata), Self::MG(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            (Self::MINFO(self_header, self_rdata), Self::MINFO(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            (Self::MR(self_header, self_rdata), Self::MR(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            (Self::MX(self_header, self_rdata), Self::MX(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::NAPTR(self_header, self_rdata), Self::NAPTR(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::NID(self_header, self_rdata), Self::NID(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::NIMLOC(self_header, self_rdata), Self::NIMLOC(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::NINFO(self_header, self_rdata), Self::NINFO(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            (Self::NS(self_header, self_rdata), Self::NS(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::NSAP_PTR(self_header, self_rdata), Self::NSAP_PTR(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::NSAP(self_header, self_rdata), Self::NSAP(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::NSEC(self_header, self_rdata), Self::NSEC(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::NSEC3(self_header, self_rdata), Self::NSEC3(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::NSEC3PARAM(self_header, self_rdata), Self::NSEC3PARAM(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            (Self::NULL(self_header, self_rdata), Self::NULL(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::NXT(self_header, self_rdata), Self::NXT(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::OPENPGPKEY(self_header, self_rdata), Self::OPENPGPKEY(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::OPT(self_header, self_rdata), Self::OPT(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::PTR(self_header, self_rdata), Self::PTR(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::PX(self_header, self_rdata), Self::PX(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::RKEY(self_header, self_rdata), Self::RKEY(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::RP(self_header, self_rdata), Self::RP(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::RRSIG(self_header, self_rdata), Self::RRSIG(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::RT(self_header, self_rdata), Self::RT(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::SIG(self_header, self_rdata), Self::SIG(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::SINK(self_header, self_rdata), Self::SINK(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::SMIMEA(self_header, self_rdata), Self::SMIMEA(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            (Self::SOA(self_header, self_rdata), Self::SOA(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::SPF(self_header, self_rdata), Self::SPF(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::SRV(self_header, self_rdata), Self::SRV(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::SSHFP(self_header, self_rdata), Self::SSHFP(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::ANY(self_header, self_rdata), Self::ANY(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::SVCB(self_header, self_rdata), Self::SVCB(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::TA(self_header, self_rdata), Self::TA(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::TALINK(self_header, self_rdata), Self::TALINK(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::TKEY(self_header, self_rdata), Self::TKEY(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::TLSA(self_header, self_rdata), Self::TLSA(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::TSIG(self_header, self_rdata), Self::TSIG(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            (Self::TXT(self_header, self_rdata), Self::TXT(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::UID(self_header, self_rdata), Self::UID(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::UINFO(self_header, self_rdata), Self::UINFO(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::UNSPEC(self_header, self_rdata), Self::UNSPEC(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::URI(self_header, self_rdata), Self::URI(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::WKS(self_header, self_rdata), Self::WKS(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::X25(self_header, self_rdata), Self::X25(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),
            // (Self::ZONEMD(self_header, self_rdata), Self::ZONEMD(other_header, other_rdata)) => (self_header.matches(other_header)) && (self_rdata == other_rdata),

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
        let rd_length_offset = wire.len();
        0_u16.to_wire_format(wire, compression)?;

        let rdata_offset = wire.len();
        match self {
            // Self::Unknown(_, _, rdata) => rdata.to_wire_format(wire, compression)?,
            Self::A(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::A6(_, rdata) => rdata.to_wire_format(wire, compression)?,
            Self::AAAA(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::AFSDB(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::AMTRELAY(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::APL(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::ATMA(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::AVC(_, rdata) => rdata.to_wire_format(wire, compression)?,
            Self::AXFR(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::CAA(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::CDNSKEY(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::CDS(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::CERT(_, rdata) => rdata.to_wire_format(wire, compression)?,
            Self::CNAME(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::CSYNC(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::DHCID(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::DLV(_, rdata) => rdata.to_wire_format(wire, compression)?,
            Self::DNAME(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::DNSKEY(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::DOA(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::DS(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::EID(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::EUI48(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::EUI64(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::GID(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::GPOS(_, rdata) => rdata.to_wire_format(wire, compression)?,
            Self::HINFO(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::HIP(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::HTTPS(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::IPSECKEY(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::ISDN(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::IXFR(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::KEY(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::KX(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::L32(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::L64(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::LOC(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::LP(_, rdata) => rdata.to_wire_format(wire, compression)?,
            Self::MAILA(_, rdata) => rdata.to_wire_format(wire, compression)?,
            Self::MAILB(_, rdata) => rdata.to_wire_format(wire, compression)?,
            Self::MB(_, rdata) => rdata.to_wire_format(wire, compression)?,
            Self::MD(_, rdata) => rdata.to_wire_format(wire, compression)?,
            Self::MF(_, rdata) => rdata.to_wire_format(wire, compression)?,
            Self::MG(_, rdata) => rdata.to_wire_format(wire, compression)?,
            Self::MINFO(_, rdata) => rdata.to_wire_format(wire, compression)?,
            Self::MR(_, rdata) => rdata.to_wire_format(wire, compression)?,
            Self::MX(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::NAPTR(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::NID(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::NIMLOC(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::NINFO(_, rdata) => rdata.to_wire_format(wire, compression)?,
            Self::NS(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::NSAP_PTR(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::NSAP(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::NSEC(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::NSEC3(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::NSEC3PARAM(_, rdata) => rdata.to_wire_format(wire, compression)?,
            Self::NULL(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::NXT(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::OPENPGPKEY(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::OPT(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::PTR(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::PX(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::RKEY(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::RP(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::RRSIG(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::RT(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::SIG(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::SINK(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::SMIMEA(_, rdata) => rdata.to_wire_format(wire, compression)?,
            Self::SOA(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::SPF(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::SRV(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::SSHFP(_, rdata) => rdata.to_wire_format(wire, compression)?,
            Self::ANY(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::SVCB(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::TA(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::TALINK(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::TKEY(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::TLSA(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::TSIG(_, rdata) => rdata.to_wire_format(wire, compression)?,
            Self::TXT(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::UID(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::UINFO(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::UNSPEC(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::URI(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::WKS(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::X25(_, rdata) => rdata.to_wire_format(wire, compression)?,
            // Self::ZONEMD(_, rdata) => rdata.to_wire_format(wire, compression)?,
        };

        // Replace the rd_length with the actual number of bytes that got written. This way, even if
        // it got compressed, it will be accurate.
        let actual_rd_length = (wire.len() - rdata_offset) as u16;
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
        // We will not store the wire_rd_length, instead, we will recalculate it since things like
        // domain name compression could cause it to change its value.
        let wire_rd_length = u16::from_wire_format(wire)?;

        // The lower bound is None because it needs access to the previous parts of the wire when de-referencing the domain name pointers.
        // No pointer should point past the end of the rdata section (forward pointers) for domain name compression so blocking off the end
        // should not cause any problems when decompressing.
        // An upper bound is required to prevent any of the deserializers that fully consume the rdata section from continueing past the end.
        let mut rdata_wire = wire.section_from_current_state(None, Some(wire_rd_length as usize))?;
        let (rr_record, rd_length) = match rtype {
            RType::Unknown(_) => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = Unknown::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::Unknown(header, rtype, rdata), rd_length)
            },
            RType::A => {
                let rdata = A::from_wire_format(&mut rdata_wire)?;
                let rd_length = rdata.serial_length();
                (Self::A(header, rdata), rd_length)
            },
            RType::NS => {
                let rdata = NS::from_wire_format(&mut rdata_wire)?;
                let rd_length = rdata.serial_length();
                (Self::NS(header, rdata), rd_length)
            },
            RType::MD => {
                let rdata = MD::from_wire_format(&mut rdata_wire)?;
                let rd_length = rdata.serial_length();
                (Self::MD(header, rdata), rd_length)
            },
            RType::MF => {
                let rdata = MF::from_wire_format(&mut rdata_wire)?;
                let rd_length = rdata.serial_length();
                (Self::MF(header, rdata), rd_length)
            },
            RType::CNAME => {
                let rdata = CNAME::from_wire_format(&mut rdata_wire)?;
                let rd_length = rdata.serial_length();
                (Self::CNAME(header, rdata), rd_length)
            },
            RType::SOA => {
                let rdata = SOA::from_wire_format(&mut rdata_wire)?;
                let rd_length = rdata.serial_length();
                (Self::SOA(header, rdata), rd_length)
            },
            RType::MB => {
                let rdata = MB::from_wire_format(&mut rdata_wire)?;
                let rd_length = rdata.serial_length();
                (Self::MB(header, rdata), rd_length)
            },
            RType::MG => {
                let rdata = MG::from_wire_format(&mut rdata_wire)?;
                let rd_length = rdata.serial_length();
                (Self::MG(header, rdata), rd_length)
            },
            RType::MR => {
                let rdata = MR::from_wire_format(&mut rdata_wire)?;
                let rd_length = rdata.serial_length();
                (Self::MR(header, rdata), rd_length)
            },
            RType::NULL => {
                let rdata = NULL::from_wire_format(&mut rdata_wire)?;
                let rd_length = rdata.serial_length();
                (Self::NULL(header, rdata), rd_length)
            },
            RType::WKS => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = WKS::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::WKS(header, rdata), rd_length)
            },
            RType::PTR => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = PTR::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::PTR(header, rdata), rd_length)
            },
            RType::HINFO => {
                let rdata = HINFO::from_wire_format(&mut rdata_wire)?;
                let rd_length = rdata.serial_length();
                (Self::HINFO(header, rdata), rd_length)
            },
            RType::MINFO => {
                let rdata = MINFO::from_wire_format(&mut rdata_wire)?;
                let rd_length = rdata.serial_length();
                (Self::MINFO(header, rdata), rd_length)
            },
            RType::MX => {
                let rdata = MX::from_wire_format(&mut rdata_wire)?;
                let rd_length = rdata.serial_length();
                (Self::MX(header, rdata), rd_length)
            },
            RType::TXT => {
                let rdata = TXT::from_wire_format(&mut rdata_wire)?;
                let rd_length = rdata.serial_length();
                (Self::TXT(header, rdata), rd_length)
            },
            RType::RP => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = RP::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::RP(header, rdata), rd_length)
            },
            RType::AFSDB => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = AFSDB::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::AFSDB(header, rdata), rd_length)
            },
            RType::X25 => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = X25::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::X25(header, rdata), rd_length)
            },
            RType::ISDN => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = ISDN::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::ISDN(header, rdata), rd_length)
            },
            RType::RT => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = RT::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::RT(header, rdata), rd_length)
            },
            RType::NSAP => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = NSAP::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::NSAP(header, rdata), rd_length)
            },
            RType::NSAP_PTR => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = NSAP_PTR::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::NSAP_PTR(header, rdata), rd_length)
            },
            RType::SIG => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = SIG::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::SIG(header, rdata), rd_length)
            },
            RType::KEY => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = KEY::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::KEY(header, rdata), rd_length)
            },
            RType::PX => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = PX::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::PX(header, rdata), rd_length)
            },
            RType::GPOS => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = GPOS::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::GPOS(header, rdata), rd_length)
            },
            RType::AAAA => {
                let rdata = AAAA::from_wire_format(&mut rdata_wire)?;
                let rd_length = rdata.serial_length();
                (Self::AAAA(header, rdata), rd_length)
            },
            RType::LOC => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = LOC::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::LOC(header, rdata), rd_length)
            },
            RType::NXT => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = NXT::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::NXT(header, rdata), rd_length)
            },
            RType::EID => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = EID::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::EID(header, rdata), rd_length)
            },
            RType::NIMLOC => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = NIMLOC::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::NIMLOC(header, rdata), rd_length)
            },
            RType::SRV => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = SRV::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::SRV(header, rdata), rd_length)
            },
            RType::ATMA => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = ATMA::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::ATMA(header, rdata), rd_length)
            },
            RType::NAPTR => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = NAPTR::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::NAPTR(header, rdata), rd_length)
            },
            RType::KX => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = KX::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::KX(header, rdata), rd_length)
            },
            RType::CERT => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = CERT::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::CERT(header, rdata), rd_length)
            },
            RType::A6 => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = A6::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::A6(header, rdata), rd_length)
            },
            RType::DNAME => {
                let rdata = DNAME::from_wire_format(&mut rdata_wire)?;
                let rd_length = rdata.serial_length();
                (Self::DNAME(header, rdata), rd_length)
            },
            RType::SINK => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = SINK::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::SINK(header, rdata), rd_length)
            },
            RType::OPT => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = OPT::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::OPT(header, rdata), rd_length)
            },
            RType::APL => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = APL::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::APL(header, rdata), rd_length)
            },
            RType::DS => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = DS::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::DS(header, rdata), rd_length)
            },
            RType::SSHFP => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = SSHFP::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::SSHFP(header, rdata), rd_length)
            },
            RType::IPSECKEY => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = IPSECKEY::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::IPSECKEY(header, rdata), rd_length)
            },
            RType::RRSIG => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = RRSIG::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::RRSIG(header, rdata), rd_length)
            },
            RType::NSEC => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = NSEC::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::NSEC(header, rdata), rd_length)
            },
            RType::DNSKEY => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = DNSKEY::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::DNSKEY(header, rdata), rd_length)
            },
            RType::DHCID => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = DHCID::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::DHCID(header, rdata), rd_length)
            },
            RType::NSEC3 => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = NSEC3::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::NSEC3(header, rdata), rd_length)
            },
            RType::NSEC3PARAM => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = NSEC3PARAM::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::NSEC3PARAM(header, rdata), rd_length)
            },
            RType::TLSA => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = TLSA::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::TLSA(header, rdata), rd_length)
            },
            RType::SMIMEA => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = SMIMEA::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::SMIMEA(header, rdata), rd_length)
            },
            RType::HIP => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = HIP::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::HIP(header, rdata), rd_length)
            },
            RType::NINFO => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = NINFO::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::NINFO(header, rdata), rd_length)
            },
            RType::RKEY => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = RKEY::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::RKEY(header, rdata), rd_length)
            },
            RType::TALINK => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = TALINK::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::TALINK(header, rdata), rd_length)
            },
            RType::CDS => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = CDS::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::CDS(header, rdata), rd_length)
            },
            RType::CDNSKEY => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = CDNSKEY::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::CDNSKEY(header, rdata), rd_length)
            },
            RType::OPENPGPKEY => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = OPENPGPKEY::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::OPENPGPKEY(header, rdata), rd_length)
            },
            RType::CSYNC => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = CSYNC::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::CSYNC(header, rdata), rd_length)
            },
            RType::ZONEMD => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = ZONEMD::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::ZONEMD(header, rdata), rd_length)
            },
            RType::SVCB => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = SVCB::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::SVCB(header, rdata), rd_length)
            },
            RType::HTTPS => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = HTTPS::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::HTTPS(header, rdata), rd_length)
            },
            RType::SPF => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = SPF::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::SPF(header, rdata), rd_length)
            },
            RType::UINFO => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = UINFO::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::UINFO(header, rdata), rd_length)
            },
            RType::UID => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = UID::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::UID(header, rdata), rd_length)
            },
            RType::GID => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = GID::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::GID(header, rdata), rd_length)
            },
            RType::UNSPEC => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = UNSPEC::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::UNSPEC(header, rdata), rd_length)
            },
            RType::NID => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = NID::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::NID(header, rdata), rd_length)
            },
            RType::L32 => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = L32::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::L32(header, rdata), rd_length)
            },
            RType::L64 => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = L64::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::L64(header, rdata), rd_length)
            },
            RType::LP => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = LP::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::LP(header, rdata), rd_length)
            },
            RType::EUI48 => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = EUI48::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::EUI48(header, rdata), rd_length)
            },
            RType::EUI64 => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = EUI64::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::EUI64(header, rdata), rd_length)
            },
            RType::TKEY => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = TKEY::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::TKEY(header, rdata), rd_length)
            },
            RType::TSIG => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = TSIG::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::TSIG(header, rdata), rd_length)
            },
            RType::IXFR => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = IXFR::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::IXFR(header, rdata), rd_length)
            },
            RType::AXFR => {
                let rdata = AXFR::from_wire_format(&mut rdata_wire)?;
                let rd_length = rdata.serial_length();
                (Self::AXFR(header, rdata), rd_length)
            },
            RType::MAILB => {
                let rdata = MAILB::from_wire_format(&mut rdata_wire)?;
                let rd_length = rdata.serial_length();
                (Self::MAILB(header, rdata), rd_length)
            },
            RType::MAILA => {
                let rdata = MAILA::from_wire_format(&mut rdata_wire)?;
                let rd_length = rdata.serial_length();
                (Self::MAILA(header, rdata), rd_length)
            },
            RType::ANY => {
                let rdata = ANY::from_wire_format(&mut rdata_wire)?;
                let rd_length = rdata.serial_length();
                (Self::ANY(header, rdata), rd_length)
            },
            RType::URI => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = URI::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::URI(header, rdata), rd_length)
            },
            RType::CAA => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = CAA::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::CAA(header, rdata), rd_length)
            },
            RType::AVC => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = AVC::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::AVC(header, rdata), rd_length)
            },
            RType::DOA => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = DOA::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::DOA(header, rdata), rd_length)
            },
            RType::AMTRELAY => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = AMTRELAY::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::AMTRELAY(header, rdata), rd_length)
            },
            RType::TA => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = TA::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::TA(header, rdata), rd_length)
            },
            RType::DLV => {
                return Err(ReadWireError::UnsupportedRType(rtype));
                // let rdata = DLV::from_wire_format(&mut rdata_wire)?;
                // let rd_length = rdata.serial_length();
                // (Self::DLV(header, rdata), rd_length)
            },
        };
        wire.shift(rdata_wire.current_state_offset() - wire.current_state_offset())?;

        if rd_length > u16::MAX {
            return Err(ReadWireError::OverflowError(
                format!("Expected rd_length to be at most {0} bytes. rd_length is actually {1}", u16::MAX, rd_length)
            ));
        }

        return Ok(rr_record);
    }
}

impl FromTokenizedRecord for ResourceRecord {
    fn from_tokenized_record<'a, 'b>(record: &crate::serde::presentation::tokenizer::tokenizer::ResourceRecord<'a>) -> Result<Self, TokenizedRecordError<'b>> where Self: Sized, 'a: 'b {
        let rr_header = RRHeader {
            name: CDomainName::from_token_format(record.domain_name)?,
            rclass: RClass::from_token_format(record.rclass)?,
            ttl: Time::from_token_format(record.ttl)?,
        };

        let rtype = RType::from_token_format(record.rtype)?;
        let record = match rtype {
            RType::A => Self::A(rr_header, A::from_tokenized_record(record)?),
            RType::NS => Self::NS(rr_header, NS::from_tokenized_record(record)?),
            RType::MD => Self::MD(rr_header, MD::from_tokenized_record(record)?),
            RType::MF => Self::MF(rr_header, MF::from_tokenized_record(record)?),
            RType::CNAME => Self::CNAME(rr_header, CNAME::from_tokenized_record(record)?),
            RType::SOA => Self::SOA(rr_header, SOA::from_tokenized_record(record)?),
            RType::MB => Self::MB(rr_header, MB::from_tokenized_record(record)?),
            RType::MG => Self::MG(rr_header, MG::from_tokenized_record(record)?),
            RType::MR => Self::MR(rr_header, MR::from_tokenized_record(record)?),
            RType::HINFO => Self::HINFO(rr_header, HINFO::from_tokenized_record(record)?),
            RType::MINFO => Self::MINFO(rr_header, MINFO::from_tokenized_record(record)?),
            RType::MX => Self::MX(rr_header, MX::from_tokenized_record(record)?),
            RType::TXT => Self::TXT(rr_header, TXT::from_tokenized_record(record)?),
            RType::AAAA => Self::AAAA(rr_header, AAAA::from_tokenized_record(record)?),
            RType::DNAME => Self::DNAME(rr_header, DNAME::from_tokenized_record(record)?),
            
            RType::ANY => return Err(TokenizedRecordError::RTypeNotAllowed(rtype)),
            RType::AXFR => return Err(TokenizedRecordError::RTypeNotAllowed(rtype)),
            RType::MAILA => return Err(TokenizedRecordError::RTypeNotAllowed(rtype)),
            RType::MAILB => return Err(TokenizedRecordError::RTypeNotAllowed(rtype)),
            RType::NULL => return Err(TokenizedRecordError::RTypeNotAllowed(rtype)),

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
            ResourceRecord::A(_, rdata) => rdata.to_presentation_format(out_buffer),
            ResourceRecord::AAAA(_, rdata) => rdata.to_presentation_format(out_buffer),
            ResourceRecord::ANY(_, _) => panic!("Cannot convert {rtype} to presentation"),
            ResourceRecord::AXFR(_, _) => panic!("Cannot convert {rtype} to presentation"),
            ResourceRecord::CNAME(_, rdata) => rdata.to_presentation_format(out_buffer),
            ResourceRecord::DNAME(_, rdata) => rdata.to_presentation_format(out_buffer),
            ResourceRecord::HINFO(_, rdata) => rdata.to_presentation_format(out_buffer),
            ResourceRecord::MAILA(_, _) => panic!("Cannot convert {rtype} to presentation"),
            ResourceRecord::MAILB(_, _) => panic!("Cannot convert {rtype} to presentation"),
            ResourceRecord::MB(_, rdata) => rdata.to_presentation_format(out_buffer),
            ResourceRecord::MD(_, rdata) => rdata.to_presentation_format(out_buffer),
            ResourceRecord::MF(_, rdata) => rdata.to_presentation_format(out_buffer),
            ResourceRecord::MG(_, rdata) => rdata.to_presentation_format(out_buffer),
            ResourceRecord::MINFO(_, rdata) => rdata.to_presentation_format(out_buffer),
            ResourceRecord::MR(_, rdata) => rdata.to_presentation_format(out_buffer),
            ResourceRecord::MX(_, rdata) => rdata.to_presentation_format(out_buffer),
            ResourceRecord::NS(_, rdata) => rdata.to_presentation_format(out_buffer),
            ResourceRecord::NULL(_, _) => panic!("Cannot convert {rtype} to presentation"),
            ResourceRecord::SOA(_, rdata) => rdata.to_presentation_format(out_buffer),
            ResourceRecord::TXT(_, rdata) => rdata.to_presentation_format(out_buffer),
        }
    }
}

use std::{fmt::Display, error::Error};

use crate::serde::{wire::{to_wire::ToWire, from_wire::FromWire}, presentation::{from_presentation::FromPresentation, to_presentation::ToPresentation}};

#[derive(Debug)]
pub enum RTypeError<'a> {
    UnknownMnemonic(&'a str),
}
impl<'a> Error for RTypeError<'a> {}
impl<'a> Display for RTypeError<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownMnemonic(mnemonic) => write!(f, "unknown type mnemonic '{mnemonic}'"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum RType {
    Unknown(u16),

    A,
    NS,
    MD,
    MF,
    CNAME,
    SOA,
    MB,
    MG,
    MR,
    NULL,
    WKS,
    PTR,
    HINFO,
    MINFO,
    MX,
    TXT,
    RP,
    AFSDB,
    X25,
    ISDN,
    RT,
    NSAP,
    #[allow(non_camel_case_types)] NSAP_PTR,
    SIG,
    KEY,
    PX,
    GPOS,
    AAAA,
    LOC,
    NXT,
    EID,
    NIMLOC,
    SRV,
    ATMA,
    NAPTR,
    KX,
    CERT,
    A6,
    DNAME,
    SINK,
    OPT,
    APL,
    DS,
    SSHFP,
    IPSECKEY,
    RRSIG,
    NSEC,
    DNSKEY,
    DHCID,
    NSEC3,
    NSEC3PARAM,
    TLSA,
    SMIMEA,

    HIP,
    NINFO,
    RKEY,
    TALINK,
    CDS,
    CDNSKEY,
    OPENPGPKEY,
    CSYNC,
    ZONEMD,
    SVCB,
    HTTPS,
            
    SPF,
    UINFO,
    UID,
    GID,
    UNSPEC,
    NID,
    L32,
    L64,
    LP,
    EUI48,
    EUI64,

    TKEY,
    TSIG,
    IXFR,
    AXFR,
    MAILB,
    MAILA,
    ANY, // *
    URI,
    CAA,
    AVC,
    DOA,
    AMTRELAY,

    TA,
    DLV,
}

impl RType {
    pub const MIN: u16 = u16::MIN;
    pub const MAX: u16 = u16::MAX;

    #[inline]
    pub const fn code(&self) -> u16 {
        return match self {
            Self::Unknown(x) => *x,

            Self::A =>            1,
            Self::NS =>           2,
            Self::MD =>           3,
            Self::MF =>           4,
            Self::CNAME =>        5,
            Self::SOA =>          6,
            Self::MB =>           7,
            Self::MG =>           8,
            Self::MR =>           9,
            Self::NULL =>         10,
            Self::WKS =>          11,
            Self::PTR =>          12,
            Self::HINFO =>        13,
            Self::MINFO =>        14,
            Self::MX =>           15,
            Self::TXT =>          16,
            Self::RP =>           17,
            Self::AFSDB =>        18,
            Self::X25 =>          19,
            Self::ISDN =>         20,
            Self::RT =>           21,
            Self::NSAP =>         22,
            Self::NSAP_PTR =>     23,
            Self::SIG =>          24,
            Self::KEY =>          25,
            Self::PX =>           26,
            Self::GPOS =>         27,
            Self::AAAA =>         28,
            Self::LOC =>          29,
            Self::NXT =>          30,
            Self::EID =>          31,
            Self::NIMLOC =>       32,
            Self::SRV =>          33,
            Self::ATMA =>         34,
            Self::NAPTR =>        35,
            Self::KX =>           36,
            Self::CERT =>         37,
            Self::A6 =>           38,
            Self::DNAME =>        39,
            Self::SINK =>         40,
            Self::OPT =>          41,
            Self::APL =>          42,
            Self::DS =>           43,
            Self::SSHFP =>        44,
            Self::IPSECKEY =>     45,
            Self::RRSIG =>        46,
            Self::NSEC =>         47,
            Self::DNSKEY =>       48,
            Self::DHCID =>        49,
            Self::NSEC3 =>        50,
            Self::NSEC3PARAM =>   51,
            Self::TLSA =>         52,
            Self::SMIMEA =>       53,
        
            Self::HIP =>          55,
            Self::NINFO =>        56,
            Self::RKEY =>         57,
            Self::TALINK =>       58,
            Self::CDS =>          59,
            Self::CDNSKEY =>      60,
            Self::OPENPGPKEY =>   61,
            Self::CSYNC =>        62,
            Self::ZONEMD =>       63,
            Self::SVCB =>         64,
            Self::HTTPS =>        65,

            Self::SPF =>      99,
            Self::UINFO =>    100,
            Self::UID =>      101,
            Self::GID =>      102,
            Self::UNSPEC =>   103,
            Self::NID =>      104,
            Self::L32 =>      105,
            Self::L64 =>      106,
            Self::LP =>       107,
            Self::EUI48 =>    108,
            Self::EUI64 =>    109,

            Self::TKEY =>     249,
            Self::TSIG =>     250,
            Self::IXFR =>     251,
            Self::AXFR =>     252,
            Self::MAILB =>    253,
            Self::MAILA =>    254,
            Self::ANY =>      255, // *
            Self::URI =>      256,
            Self::CAA =>      257,
            Self::AVC =>      258,
            Self::DOA =>      259,
            Self::AMTRELAY => 260,

            Self::TA =>   32768,
            Self::DLV =>  32769,
        };
    }

    #[inline]
    pub fn mnemonic(&self) -> String {
        return match self {
            Self::Unknown(code) => format!("TYPE{code}"),

            Self::A =>            "A".to_string(),
            Self::NS =>           "NS".to_string(),
            Self::MD =>           "MD".to_string(),
            Self::MF =>           "MF".to_string(),
            Self::CNAME =>        "CNAME".to_string(),
            Self::SOA =>          "SOA".to_string(),
            Self::MB =>           "MB".to_string(),
            Self::MG =>           "MG".to_string(),
            Self::MR =>           "MR".to_string(),
            Self::NULL =>         "NULL".to_string(),
            Self::WKS =>          "WKS".to_string(),
            Self::PTR =>          "PTR".to_string(),
            Self::HINFO =>        "HINFO".to_string(),
            Self::MINFO =>        "MINFO".to_string(),
            Self::MX =>           "MX".to_string(),
            Self::TXT =>          "TXT".to_string(),
            Self::RP =>           "RP".to_string(),
            Self::AFSDB =>        "AFSDB".to_string(),
            Self::X25 =>          "X25".to_string(),
            Self::ISDN =>         "ISDN".to_string(),
            Self::RT =>           "RT".to_string(),
            Self::NSAP =>         "NSAP".to_string(),
            Self::NSAP_PTR =>     "NSAP-PTR".to_string(),
            Self::SIG =>          "SIG".to_string(),
            Self::KEY =>          "KEY".to_string(),
            Self::PX =>           "PX".to_string(),
            Self::GPOS =>         "GPOS".to_string(),
            Self::AAAA =>         "AAAA".to_string(),
            Self::LOC =>          "LOC".to_string(),
            Self::NXT =>          "NXT".to_string(),
            Self::EID =>          "EID".to_string(),
            Self::NIMLOC =>       "NIMLOC".to_string(),
            Self::SRV =>          "SRV".to_string(),
            Self::ATMA =>         "ATMA".to_string(),
            Self::NAPTR =>        "NAPTR".to_string(),
            Self::KX =>           "KX".to_string(),
            Self::CERT =>         "CERT".to_string(),
            Self::A6 =>           "A6".to_string(),
            Self::DNAME =>        "DNAME".to_string(),
            Self::SINK =>         "SINK".to_string(),
            Self::OPT =>          "OPT".to_string(),
            Self::APL =>          "APL".to_string(),
            Self::DS =>           "DS".to_string(),
            Self::SSHFP =>        "SSHFP".to_string(),
            Self::IPSECKEY =>     "IPSECKEY".to_string(),
            Self::RRSIG =>        "RRSIG".to_string(),
            Self::NSEC =>         "NSEC".to_string(),
            Self::DNSKEY =>       "DNSKEY".to_string(),
            Self::DHCID =>        "DHCID".to_string(),
            Self::NSEC3 =>        "NSEC3".to_string(),
            Self::NSEC3PARAM =>   "NSEC3PARAM".to_string(),
            Self::TLSA =>         "TLSA".to_string(),
            Self::SMIMEA =>       "SMIMEA".to_string(),
        
            Self::HIP =>          "HIP".to_string(),
            Self::NINFO =>        "NINFO".to_string(),
            Self::RKEY =>         "RKEY".to_string(),
            Self::TALINK =>       "TALINK".to_string(),
            Self::CDS =>          "CDS".to_string(),
            Self::CDNSKEY =>      "CDNSKEY".to_string(),
            Self::OPENPGPKEY =>   "OPENPGPKEY".to_string(),
            Self::CSYNC =>        "CSYNC".to_string(),
            Self::ZONEMD =>       "ZONEMD".to_string(),
            Self::SVCB =>         "SVCB".to_string(),
            Self::HTTPS =>        "HTTPS".to_string(),

            Self::SPF =>      "SPF".to_string(),
            Self::UINFO =>    "UINFO".to_string(),
            Self::UID =>      "UID".to_string(),
            Self::GID =>      "GID".to_string(),
            Self::UNSPEC =>   "UNSPEC".to_string(),
            Self::NID =>      "NID".to_string(),
            Self::L32 =>      "L32".to_string(),
            Self::L64 =>      "L64".to_string(),
            Self::LP =>       "LP".to_string(),
            Self::EUI48 =>    "EUI48".to_string(),
            Self::EUI64 =>    "EUI64".to_string(),

            Self::TKEY =>     "TKEY".to_string(),
            Self::TSIG =>     "TSIG".to_string(),
            Self::IXFR =>     "IXFR".to_string(),
            Self::AXFR =>     "AXFR".to_string(),
            Self::MAILB =>    "MAILB".to_string(),
            Self::MAILA =>    "MAILA".to_string(),
            Self::ANY =>      "*".to_string(), // *
            Self::URI =>      "URI".to_string(),
            Self::CAA =>      "CAA".to_string(),
            Self::AVC =>      "AVC".to_string(),
            Self::DOA =>      "DOA".to_string(),
            Self::AMTRELAY => "AMTRELAY".to_string(),

            Self::TA =>   "TA".to_string(),
            Self::DLV =>  "DLV".to_string(),
        };
    }

    #[inline]
    pub const fn description(&self) -> Option<&str> {
        return match self {
            Self::Unknown(_) => None,

            Self::A =>            Some("a host address"),
            Self::NS =>           Some("an authoritative name server"),
            Self::MD =>           Some("a mail destination (OBSOLETE - use MX)"),
            Self::MF =>           Some("a mail forwarder (OBSOLETE - use MX)"),
            Self::CNAME =>        Some("the canonical name for an alias"),
            Self::SOA =>          Some("marks the start of a zone of authority"),
            Self::MB =>           Some("a mailbox domain name (EXPERIMENTAL)"),
            Self::MG =>           Some("a mail group member (EXPERIMENTAL)"),
            Self::MR =>           Some("a mail rename domain name (EXPERIMENTAL)"),
            Self::NULL =>         Some("a null RR (EXPERIMENTAL)"),
            Self::WKS =>          Some("a well known service description"),
            Self::PTR =>          Some("a domain name pointer"),
            Self::HINFO =>        Some("host information"),
            Self::MINFO =>        Some("mailbox or mail list information"),
            Self::MX =>           Some("mail exchange"),
            Self::TXT =>          Some("text strings"),
            Self::RP =>           Some("for Responsible Person"),
            Self::AFSDB =>        Some("for AFS Data Base location"),
            Self::X25 =>          Some("for X.25 PSDN address"),
            Self::ISDN =>         Some("for ISDN address"),
            Self::RT =>           Some("for Route Through"),
            Self::NSAP =>         Some("for NSAP address, NSAP style A record (DEPRECATED)"),
            Self::NSAP_PTR =>     Some("for domain name pointer, NSAP style (DEPRECATED)"),
            Self::SIG =>          Some("for security signature"),
            Self::KEY =>          Some("for security key"),
            Self::PX =>           Some("X.400 mail mapping information"),
            Self::GPOS =>         Some("Geographical Position"),
            Self::AAAA =>         Some("IP6 Address"),
            Self::LOC =>          Some("Location Information"),
            Self::NXT =>          Some("Next Domain (OBSOLETE)"),
            Self::EID =>          Some("Endpoint Identifier"),
            Self::NIMLOC =>       Some("Nimrod Locator"),
            Self::SRV =>          Some("Server Selection"),
            Self::ATMA =>         Some("ATM Address"),
            Self::NAPTR =>        Some("Naming Authority Pointer"),
            Self::KX =>           Some("Key Exchanger"),
            Self::CERT =>         Some("CERT"),
            Self::A6 =>           Some("A6 (OBSOLETE - use AAAA)"),
            Self::DNAME =>        Some("DNAME"),
            Self::SINK =>         Some("SINK"),
            Self::OPT =>          Some("OPT"),
            Self::APL =>          Some("APL"),
            Self::DS =>           Some("Delegation Signer"),
            Self::SSHFP =>        Some("SSH Key Fingerprint"),
            Self::IPSECKEY =>     Some("IPSECKEY"),
            Self::RRSIG =>        Some("RRSIG"),
            Self::NSEC =>         Some("NSEC"),
            Self::DNSKEY =>       Some("DNSKEY"),
            Self::DHCID =>        Some("DHCID"),
            Self::NSEC3 =>        Some("NSEC3"),
            Self::NSEC3PARAM =>   Some("NSEC3PARAM"),
            Self::TLSA =>         Some("TLSA"),
            Self::SMIMEA =>       Some("S/MIME cert association"),

            Self::HIP =>          Some("Host Identity Protocol"),
            Self::NINFO =>        Some("NINFO"),
            Self::RKEY =>         Some("RKEY"),
            Self::TALINK =>       Some("Trust Anchor LINK"),
            Self::CDS =>          Some("Child DS"),
            Self::CDNSKEY =>      Some("DNSKEY(s) the Child wants reflected in DS"),
            Self::OPENPGPKEY =>   Some("OpenPGP Key"),
            Self::CSYNC =>        Some("Child-To-Parent Synchronization"),
            Self::ZONEMD =>       Some("Message Digest Over Zone Data"),
            Self::SVCB =>         Some("General Purpose Service Binding"),
            Self::HTTPS =>        Some("Service Binding type for use with HTTP"),

            Self::SPF =>      None,
            Self::UINFO =>    None,
            Self::UID =>      None,
            Self::GID =>      None,
            Self::UNSPEC =>   None,
            Self::NID =>      None,
            Self::L32 =>      None,
            Self::L64 =>      None,
            Self::LP =>       None,
            Self::EUI48 =>    Some("an EUI-48 address"),
            Self::EUI64 =>    Some("an EUI-64 address"),

            Self::TKEY =>     Some("Transaction Key"),
            Self::TSIG =>     Some("Transaction Signature"),
            Self::IXFR =>     Some("incremental transfer"),
            Self::AXFR =>     Some("transfer of an entire zone"),
            Self::MAILB =>    Some("mailbox-related RRs (MB, MG or MR)"),
            Self::MAILA =>    Some("mail agent RRs (OBSOLETE - see MX)"),
            Self::ANY =>      Some("A request for some or all records the server has available"),
            Self::URI =>      Some("URI"),
            Self::CAA =>      Some("Certification Authority Restriction"),
            Self::AVC =>      Some("Application Visibility and Control"),
            Self::DOA =>      Some("Digital Object Architecture"),
            Self::AMTRELAY => Some("Automatic Multicast Tunneling Relay"),

            Self::TA =>   Some("DNSSEC Trust Authorities"),
            Self::DLV =>  Some("DNSSEC Lookaside Validation (OBSOLETE)"),
        };
    }

    #[inline]
    pub const fn from_code(value: u16) -> Self {
        return match value {
            1 =>  Self::A,
            2 =>  Self::NS,
            3 =>  Self::MD,
            4 =>  Self::MF,
            5 =>  Self::CNAME,
            6 =>  Self::SOA,
            7 =>  Self::MB,
            8 =>  Self::MG,
            9 =>  Self::MR,
            10 => Self::NULL,
            11 => Self::WKS,
            12 => Self::PTR,
            13 => Self::HINFO,
            14 => Self::MINFO,
            15 => Self::MX,
            16 => Self::TXT,
            17 => Self::RP,
            18 => Self::AFSDB,
            19 => Self::X25,
            20 => Self::ISDN,
            21 => Self::RT,
            22 => Self::NSAP,
            23 => Self::NSAP_PTR,
            24 => Self::SIG,
            25 => Self::KEY,
            26 => Self::PX,
            27 => Self::GPOS,
            28 => Self::AAAA,
            29 => Self::LOC,
            30 => Self::NXT,
            31 => Self::EID,
            32 => Self::NIMLOC,
            33 => Self::SRV,
            34 => Self::ATMA,
            35 => Self::NAPTR,
            36 => Self::KX,
            37 => Self::CERT,
            38 => Self::A6,
            39 => Self::DNAME,
            40 => Self::SINK,
            41 => Self::OPT,
            42 => Self::APL,
            43 => Self::DS,
            44 => Self::SSHFP,
            45 => Self::IPSECKEY,
            46 => Self::RRSIG,
            47 => Self::NSEC,
            48 => Self::DNSKEY,
            49 => Self::DHCID,
            50 => Self::NSEC3,
            51 => Self::NSEC3PARAM,
            52 => Self::TLSA,
            53 => Self::SMIMEA,

            55 => Self::HIP,
            56 => Self::NINFO,
            57 => Self::RKEY,
            58 => Self::TALINK,
            59 => Self::CDS,
            60 => Self::CDNSKEY,
            61 => Self::OPENPGPKEY,
            62 => Self::CSYNC,
            63 => Self::ZONEMD,
            64 => Self::SVCB,
            65 => Self::HTTPS,

            99 =>  Self::SPF,
            100 => Self::UINFO,
            101 => Self::UID,
            102 => Self::GID,
            103 => Self::UNSPEC,
            104 => Self::NID,
            105 => Self::L32,
            106 => Self::L64,
            107 => Self::LP,
            108 => Self::EUI48,
            109 => Self::EUI64,

            249 => Self::TKEY,
            250 => Self::TSIG,
            251 => Self::IXFR,
            252 => Self::AXFR,
            253 => Self::MAILB,
            254 => Self::MAILA,
            255 => Self::ANY,
            256 => Self::URI,
            257 => Self::CAA,
            258 => Self::AVC,
            259 => Self::DOA,
            260 => Self::AMTRELAY,

            32768 => Self::TA,
            32769 => Self::DLV,

            _ => Self::Unknown(value),
        };
    }

    #[inline]
    pub fn from_str(rtype: &str) -> Result<RType, RTypeError> {
        match rtype {
            "A" =>          Ok(RType::A),
            "NS" =>         Ok(RType::NS),
            "MD" =>         Ok(RType::MD),
            "MF" =>         Ok(RType::MF),
            "CNAME" =>      Ok(RType::CNAME),
            "SOA" =>        Ok(RType::SOA),
            "MB" =>         Ok(RType::MB),
            "MG" =>         Ok(RType::MG),
            "MR" =>         Ok(RType::MR),
            "NULL" =>       Ok(RType::NULL),
            "WKS" =>        Ok(RType::WKS),
            "PTR" =>        Ok(RType::PTR),
            "HINFO" =>      Ok(RType::HINFO),
            "MINFO" =>      Ok(RType::MINFO),
            "MX" =>         Ok(RType::MX),
            "TXT" =>        Ok(RType::TXT),
            "RP" =>         Ok(RType::RP),
            "AFSDB" =>      Ok(RType::AFSDB),
            "X25" =>        Ok(RType::X25),
            "ISDN" =>       Ok(RType::ISDN),
            "RT" =>         Ok(RType::RT),
            "NSAP" =>       Ok(RType::NSAP),
            "NSAP-PTR" =>   Ok(RType::NSAP_PTR),
            "SIG" =>        Ok(RType::SIG),
            "KEY" =>        Ok(RType::KEY),
            "PX" =>         Ok(RType::PX),
            "GPOS" =>       Ok(RType::GPOS),
            "AAAA" =>       Ok(RType::AAAA),
            "LOC" =>        Ok(RType::LOC),
            "NXT" =>        Ok(RType::NXT),
            "EID" =>        Ok(RType::EID),
            "NIMLOC" =>     Ok(RType::NIMLOC),
            "SRV" =>        Ok(RType::SRV),
            "ATMA" =>       Ok(RType::ATMA),
            "NAPTR" =>      Ok(RType::NAPTR),
            "KX" =>         Ok(RType::KX),
            "CERT" =>       Ok(RType::CERT),
            "A6" =>         Ok(RType::A6),
            "DNAME" =>      Ok(RType::DNAME),
            "SINK" =>       Ok(RType::SINK),
            "OPT" =>        Ok(RType::OPT),
            "APL" =>        Ok(RType::APL),
            "DS" =>         Ok(RType::DS),
            "SSHFP" =>      Ok(RType::SSHFP),
            "IPSECKEY" =>   Ok(RType::IPSECKEY),
            "RRSIG" =>      Ok(RType::RRSIG),
            "NSEC" =>       Ok(RType::NSEC),
            "DNSKEY" =>     Ok(RType::DNSKEY),
            "DHCID" =>      Ok(RType::DHCID),
            "NSEC3" =>      Ok(RType::NSEC3),
            "NSEC3PARAM" => Ok(RType::NSEC3PARAM),
            "TLSA" =>       Ok(RType::TLSA),
            "SMIMEA" =>     Ok(RType::SMIMEA),
    
            "HIP" =>        Ok(RType::HIP),
            "NINFO" =>      Ok(RType::NINFO),
            "RKEY" =>       Ok(RType::RKEY),
            "TALINK" =>     Ok(RType::TALINK),
            "CDS" =>        Ok(RType::CDS),
            "CDNSKEY" =>    Ok(RType::CDNSKEY),
            "OPENPGPKEY" => Ok(RType::OPENPGPKEY),
            "CSYNC" =>      Ok(RType::CSYNC),
            "ZONEMD" =>     Ok(RType::ZONEMD),
            "SVCB" =>       Ok(RType::SVCB),
            "HTTPS" =>      Ok(RType::HTTPS),
    
            "SPF" =>    Ok(RType::SPF),
            "UINFO" =>  Ok(RType::UINFO),
            "UID" =>    Ok(RType::UID),
            "GID" =>    Ok(RType::GID),
            "UNSPEC" => Ok(RType::UNSPEC),
            "NID" =>    Ok(RType::NID),
            "L32" =>    Ok(RType::L32),
            "L64" =>    Ok(RType::L64),
            "LP" =>     Ok(RType::LP),
            "EUI48" =>  Ok(RType::EUI48),
            "EUI64" =>  Ok(RType::EUI64),
    
            "TKEY" =>       Ok(RType::TKEY),
            "TSIG" =>       Ok(RType::TSIG),
            "IXFR" =>       Ok(RType::IXFR),
            "AXFR" =>       Ok(RType::AXFR),
            "MAILB" =>      Ok(RType::MAILB),
            "MAILA" =>      Ok(RType::MAILA),
            "*" =>          Ok(RType::ANY),
            "UR" =>         Ok(RType::URI),
            "CAA" =>        Ok(RType::CAA),
            "AVC" =>        Ok(RType::AVC),
            "DOA" =>        Ok(RType::DOA),
            "AMTRELAY" =>   Ok(RType::AMTRELAY),
    
            "TA" =>     Ok(RType::TA),
            "DLV" =>    Ok(RType::DLV),
    
            _ => {
                const WILDCARD: &str = "TYPE";
                if !rtype.starts_with(WILDCARD) {
                    return Err(RTypeError::UnknownMnemonic(rtype));
                }
                let rtype = match u16::from_str_radix(&rtype[WILDCARD.len()..], 10) {
                    Ok(rtype) => rtype,
                    Err(_) => return Err(RTypeError::UnknownMnemonic(rtype)),
                };
                // Note: we don't directly assign it to Unknown since it could be a known code that
                //       just uses the 'TYPE(\d)+' syntax.
                Ok(Self::from_code(rtype))
            },
        }
    }
}

impl Display for RType {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.mnemonic())
    }
}

pub trait RTypeCode {
    fn rtype(&self) -> RType;
}

impl ToWire for RType {
    #[inline]
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>, compression: &mut Option<crate::serde::wire::compression_map::CompressionMap>) -> Result<(), crate::serde::wire::write_wire::WriteWireError> where 'a: 'b {
        self.code().to_wire_format(wire, compression)
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        self.code().serial_length()
    }
}

impl FromWire for RType {
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>) -> Result<Self, crate::serde::wire::read_wire::ReadWireError> where Self: Sized, 'a: 'b {
        Ok(Self::from_code(
            u16::from_wire_format(wire)?
        ))
    }
}

impl FromPresentation for RType {
    #[inline]
    fn from_token_format<'a, 'b>(token: &'a str) -> Result<Self, crate::serde::presentation::errors::TokenError<'b>> where Self: Sized, 'a: 'b {
        Ok(Self::from_str(token)?)
    }
}

impl ToPresentation for RType {
    #[inline]
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        out_buffer.push(self.mnemonic())
    }
}

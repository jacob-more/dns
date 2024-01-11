use std::fmt::Display;

use crate::serde::{presentation::{to_presentation::ToPresentation, from_presentation::FromPresentation}, wire::{from_wire::FromWire, to_wire::ToWire}};

/// https://www.iana.org/assignments/address-family-numbers/address-family-numbers.xhtml#address-family-numbers-2
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum AddressFamily {
    Unknown(u16),

    Ipv4,   // aka. IP
    Ipv6,
    NSAP,
    HDLC,
    BBN1822,
    IEEE802,
    E163,
    E164,
    F69,
    X121,
    IPX,
    Appletalk,
    DecnetIV,
    BanyanVines,
    E164WithNSAPFormatSubaddress,
    DNS,
    DistinguishedName,
    ASNumber,
    XTPOverIpv4,
    XTPOverIpv6,
    XTPNativeModeXTP,
    FibreChannelWorldWidePortName,
    FibreChannelWorldWideNodeName,
    GWID,
    AFIForL2VPNInformation,
    MplsTpSectionEndpointIdentifier,
    MplsTpLspEndpointIdentifier,
    MplsTpPseudowireEndpointIdentifier,
    MTIpv4,
    MTIpv6,
    #[allow(non_camel_case_types)] BGP_SFC,

    EIGRPCommonServiceFamily,
    EIGRPIpv4ServiceFamily,
    EIGRPIpv6ServiceFamily,
    LISPCanonicalAddressFormat,
    #[allow(non_camel_case_types)] BGP_LS,
    MAC48Bit,
    MAC64Bit,
    OUI,
    MAC24,
    MAC40,
    IPv6_64,
    RBridgePortID,
    TRILLNickname,
    UniversallyUniqueIdentifier,
    RoutingPolicyAFI,
    MPLSNamespaces,
}

impl AddressFamily {
    pub const MIN: u16 = u16::MIN;
    pub const MAX: u16 = u16::MAX;

    #[inline]
    pub fn code(&self) -> u16 {
        match self {
            Self::Unknown(x) => *x,

            Self::Ipv4 =>                               1,
            Self::Ipv6 =>                               2,
            Self::NSAP =>                               3,
            Self::HDLC =>                               4,
            Self::BBN1822 =>                            5,
            Self::IEEE802 =>                            6,
            Self::E163 =>                               7,
            Self::E164 =>                               8,
            Self::F69 =>                                9,
            Self::X121 =>                               10,
            Self::IPX =>                                11,
            Self::Appletalk =>                          12,
            Self::DecnetIV =>                           13,
            Self::BanyanVines =>                        14,
            Self::E164WithNSAPFormatSubaddress =>       15,
            Self::DNS =>                                16,
            Self::DistinguishedName =>                  17,
            Self::ASNumber =>                           18,
            Self::XTPOverIpv4 =>                        19,
            Self::XTPOverIpv6 =>                        20,
            Self::XTPNativeModeXTP =>                   21,
            Self::FibreChannelWorldWidePortName =>      22,
            Self::FibreChannelWorldWideNodeName =>      23,
            Self::GWID =>                               24,
            Self::AFIForL2VPNInformation =>             25,
            Self::MplsTpSectionEndpointIdentifier =>    26,
            Self::MplsTpLspEndpointIdentifier =>        27,
            Self::MplsTpPseudowireEndpointIdentifier => 28,
            Self::MTIpv4 =>                             29,
            Self::MTIpv6 =>                             30,
            Self::BGP_SFC =>                            31,

            Self::EIGRPCommonServiceFamily =>    16384,
            Self::EIGRPIpv4ServiceFamily =>      16385,
            Self::EIGRPIpv6ServiceFamily =>      16386,
            Self::LISPCanonicalAddressFormat =>  16387,
            Self::BGP_LS =>                      16388,
            Self::MAC48Bit =>                    16389,
            Self::MAC64Bit =>                    16390,
            Self::OUI =>                         16391,
            Self::MAC24 =>                       16392,
            Self::MAC40 =>                       16393,
            Self::IPv6_64 =>                     16394,
            Self::RBridgePortID =>               16395,
            Self::TRILLNickname =>               16396,
            Self::UniversallyUniqueIdentifier => 16397,
            Self::RoutingPolicyAFI =>            16398,
            Self::MPLSNamespaces =>              16399,
        }
    }

    #[inline]
    pub fn mnemonic(&self) -> String {
        self.code().to_string()
    }

    #[inline]
    pub fn from_code(value: u16) -> Self {
        match value {
            1 =>  Self::Ipv4,
            2 =>  Self::Ipv6,
            3 =>  Self::NSAP,
            4 =>  Self::HDLC,
            5 =>  Self::BBN1822,
            6 =>  Self::IEEE802,
            7 =>  Self::E163,
            8 =>  Self::E164,
            9 =>  Self::F69,
            10 => Self::X121,
            11 => Self::IPX,
            12 => Self::Appletalk,
            13 => Self::DecnetIV,
            14 => Self::BanyanVines,
            15 => Self::E164WithNSAPFormatSubaddress,
            16 => Self::DNS,
            17 => Self::DistinguishedName,
            18 => Self::ASNumber,
            19 => Self::XTPOverIpv4,
            20 => Self::XTPOverIpv6,
            21 => Self::XTPNativeModeXTP,
            22 => Self::FibreChannelWorldWidePortName,
            23 => Self::FibreChannelWorldWideNodeName,
            24 => Self::GWID,
            25 => Self::AFIForL2VPNInformation,
            26 => Self::MplsTpSectionEndpointIdentifier,
            27 => Self::MplsTpLspEndpointIdentifier,
            28 => Self::MplsTpPseudowireEndpointIdentifier,
            29 => Self::MTIpv4,
            30 => Self::MTIpv6,
            31 => Self::BGP_SFC,

            16384 => Self::EIGRPCommonServiceFamily,
            16385 => Self::EIGRPIpv4ServiceFamily,
            16386 => Self::EIGRPIpv6ServiceFamily,
            16387 => Self::LISPCanonicalAddressFormat,
            16388 => Self::BGP_LS,
            16389 => Self::MAC48Bit,
            16390 => Self::MAC64Bit,
            16391 => Self::OUI,
            16392 => Self::MAC24,
            16393 => Self::MAC40,
            16394 => Self::IPv6_64,
            16395 => Self::RBridgePortID,
            16396 => Self::TRILLNickname,
            16397 => Self::UniversallyUniqueIdentifier,
            16398 => Self::RoutingPolicyAFI,
            16399 => Self::MPLSNamespaces,

            _ => Self::Unknown(value),
        }
    }
}

impl Display for AddressFamily {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.mnemonic())
    }
}

impl ToWire for AddressFamily {
    #[inline]
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>, compression: &mut Option<crate::serde::wire::compression_map::CompressionMap>) -> Result<(), crate::serde::wire::write_wire::WriteWireError> where 'a: 'b {
        self.code().to_wire_format(wire, compression)
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        self.code().serial_length()
    }
}

impl FromWire for AddressFamily {
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>) -> Result<Self, crate::serde::wire::read_wire::ReadWireError> where Self: Sized, 'a: 'b {
        Ok(Self::from_code(
            u16::from_wire_format(wire)?
        ))
    }
}

impl FromPresentation for AddressFamily {
    fn from_token_format<'a, 'b>(token: &'a str) -> Result<Self, crate::serde::presentation::errors::TokenError<'b>> where Self: Sized, 'a: 'b {
        Ok(Self::from_code(
            u16::from_token_format(token)?
        ))
    }
}

impl ToPresentation for AddressFamily {
    #[inline]
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        out_buffer.push(self.code().to_string())
    }
}

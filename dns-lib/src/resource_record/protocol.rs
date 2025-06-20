use std::{error::Error, fmt::Display};

use crate::gen_enum::enum_encoding;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum ProtocolError {
    UnknownMnemonic(String),
}
impl Error for ProtocolError {}
impl Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownMnemonic(protocol) => write!(f, "Unknown Protocol Mnemonic: {protocol}"),
        }
    }
}

enum_encoding!(
    Protocol,
    u8,
    ProtocolError,
    (
        (HOPOPT, "HOPOPT", 0),
        (ICMP, "ICMP", 1),
        (IGMP, "IGMP", 2),
        (GGP, "GGP", 3),
        (IPv4, "IPv4", 4),
        (ST, "ST", 5),
        (TCP, "TCP", 6),
        (CBT, "CBT", 7),
        (EGP, "EGP", 8),
        (IGP, "IGP", 9),
        (BBN_RCC_MON, "BBN-RCC-MON", 10),
        (NVP_II, "NVP-II", 11),
        (PUP, "PUP", 12),
        (ARGUS, "ARGUS", 13),
        (EMCON, "EMCON", 14),
        (XNET, "XNET", 15),
        (CHAOS, "CHAOS", 16),
        (UDP, "UDP", 17),
        (MUX, "MUX", 18),
        (DCN_MEAS, "DCN-MEAS", 19),
        (HMP, "HMP", 20),
        (PRM, "PRM", 21),
        (XNS_IDP, "XNS-IDP", 22),
        (TRUNK_1, "TRUNK-1", 23),
        (TRUNK_2, "TRUNK-2", 24),
        (LEAF_1, "LEAF-1", 25),
        (LEAF_2, "LEAF-2", 26),
        (RDP, "RDP", 27),
        (IRTP, "IRTP", 28),
        (ISO_TP4, "ISO-TP4", 29),
        (NETBLT, "NETBLT", 30),
        (MFE_NSP, "MFE-NSP", 31),
        (MERIT_INP, "MERIT-INP", 32),
        (DCCP, "DCCP", 33),
        (THREE_PC, "3PC", 34),
        (IDPR, "IDPR", 35),
        (XTP, "XTP", 36),
        (DDP, "DDP", 37),
        (IDPR_CMTP, "IDPR-CMTP", 38),
        (TP_PLUS_PLUS, "TP++", 39),
        (IL, "IL", 40),
        (IPv6, "IPv6", 41),
        (SDRP, "SDRP", 42),
        (IPv6_Route, "IPv6-Route", 43),
        (IPv6_Frag, "IPv6-Frag", 44),
        (IDRP, "IDRP", 45),
        (RSVP, "RSVP", 46),
        (GRE, "GRE", 47),
        (DSR, "DSR", 48),
        (BNA, "BNA", 49),
        (ESP, "ESP", 50),
        (AH, "AH", 51),
        (I_NLSP, "I-NLSP", 52),
        (SWIPE, "SWIPE", 53),
        (NARP, "NARP", 54),
        (Min_IPv4, "Min-IPv4", 55),
        (TLSP, "TLSP", 56),
        (SKIP, "SKIP", 57),
        (IPv6_ICMP, "IPv6-ICMP", 58),
        (IPv6_NoNxt, "IPv6-NoNxt", 59),
        (IPv6_Opts, "IPv6-Opts", 60),
        (CFTP, "CFTP", 62),
        (SAT_EXPAK, "SAT-EXPAK", 64),
        (KRYPTOLAN, "KRYPTOLAN", 65),
        (RVD, "RVD", 66),
        (IPPC, "IPPC", 67),
        (SAT_MON, "SAT-MON", 69),
        (VISA, "VISA", 70),
        (IPCV, "IPCV", 71),
        (CPNX, "CPNX", 72),
        (CPHB, "CPHB", 73),
        (WSN, "WSN", 74),
        (PVP, "PVP", 75),
        (BR_SAT_MON, "BR-SAT-MON", 76),
        (SUN_ND, "SUN-ND", 77),
        (WB_MON, "WB-MON", 78),
        (WB_EXPAK, "WB-EXPAK", 79),
        (ISO_IP, "ISO-IP", 80),
        (VMTP, "VMTP", 81),
        (VINES, "SECURE-VMTP", 82),
        (SECURE_VMTP, "VINES", 83),
        (IPTM, "IPTM", 84),
        (NSFNET_IGP, "NSFNET-IGP", 85),
        (DGP, "DGP", 86),
        (TCF, "TCF", 87),
        (EIGRP, "EIGRP", 88),
        (OSPFIGP, "OSPFIGP", 89),
        (Sprite_RPC, "Sprite-RPC", 90),
        (LARP, "LARP", 91),
        (MTP, "MTP", 92),
        (AX_25, "AX.25", 93),
        (IPIP, "IPIP", 94),
        (MICP, "MICP", 95),
        (SCC_SP, "SCC-SP", 96),
        (ETHERIP, "ETHERIP", 97),
        (ENCAP, "ENCAP", 98),
        (GMTP, "GMTP", 100),
        (IFMP, "IFMP", 101),
        (PNNI, "PNNI", 102),
        (PIM, "PIM", 103),
        (ARIS, "ARIS", 104),
        (SCPS, "SCPS", 105),
        (QNX, "QNX", 106),
        (A_N, "A/N", 107),
        (IPComp, "IPComp", 108),
        (SNP, "SNP", 109),
        (Compaq_Peer, "Compaq-Peer", 110),
        (IPX_in_IP, "IPX-in-IP", 111),
        (VRRP, "VRRP", 112),
        (PGM, "PGM", 113),
        (L2TP, "L2TP", 115),
        (DDX, "DDX", 116),
        (IATP, "IATP", 117),
        (STP, "STP", 118),
        (SRP, "SRP", 119),
        (UTI, "UTI", 120),
        (SMP, "SMP", 121),
        (SM, "SM", 122),
        (PTP, "PTP", 123),
        (ISIS_over_IPv4, "ISIS over IPv4", 124),
        (FIRE, "FIRE", 125),
        (CRTP, "CRTP", 126),
        (CRUDP, "CRUDP", 127),
        (SSCOPMCE, "SSCOPMCE", 128),
        (IPLT, "IPLT", 129),
        (SPS, "SPS", 130),
        (PIPE, "PIPE", 131),
        (SCTP, "SCTP", 132),
        (FC, "FC", 133),
        (RSVP_E2E_IGNORE, "RSVP-E2E-IGNORE", 134),
        (Mobility_Header, "Mobility Header", 135),
        (UDPLite, "UDPLite", 136),
        (MPLS_in_IP, "MPLS-in-IP", 137),
        (manet, "manet", 138),
        (HIP, "HIP", 139),
        (Shim6, "Shim6", 140),
        (WESP, "WESP", 141),
        (ROHC, "ROHC", 142),
        (Ethernet, "Ethernet", 143),
        (AGGFRAG, "AGGFRAG", 144),
        (NSH, "NSH", 145),
    ),
    code_or_mnemonic_from_str,
    mnemonic_presentation,
    mnemonic_display
);

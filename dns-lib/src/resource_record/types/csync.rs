use dns_macros::{ToWire, FromWire, FromTokenizedRData, RData, ToPresentation};

use crate::types::rtype_bitmap::RTypeBitmap;


const IMMEDIATE_FLAG_MASK: u16   = 0b0000_0000_0000_0001;
const SOA_MINIMUM_FLAG_MASK: u16 = 0b0000_0000_0000_0010;


/// https://datatracker.ietf.org/doc/html/rfc7477#section-2.1
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, ToPresentation, FromTokenizedRData, RData)]
pub struct CSYNC {
    serial: u32,
    flags: u16,
    type_bit_map: RTypeBitmap
}

impl CSYNC {
    #[inline]
    pub fn new(serial: u32, flags: u16, type_bit_map: RTypeBitmap) -> Self {
        Self { serial, flags, type_bit_map  }
    }

    #[inline]
    pub fn serial(&self) -> u32 { self.serial }

    #[inline]
    pub fn flags(&self) -> u16 { self.flags }

    #[inline]
    pub fn immediate_flag(&self) -> bool {
        (self.flags & IMMEDIATE_FLAG_MASK) == IMMEDIATE_FLAG_MASK
    }

    #[inline]
    pub fn soa_minimum_flag(&self) -> bool {
        (self.flags & SOA_MINIMUM_FLAG_MASK) == SOA_MINIMUM_FLAG_MASK
    }
}

#[cfg(test)]
mod circular_serde_sanity_test {
    use crate::{resource_record::rtype::RType, serde::wire::circular_test::gen_test_circular_serde_sanity_test, types::rtype_bitmap::RTypeBitmap};
    use super::CSYNC;

    gen_test_circular_serde_sanity_test!(
        rfc_7477_example_record_circular_serde_sanity_test,
        CSYNC { serial: 66, flags: 3, type_bit_map: RTypeBitmap::from_rtypes([RType::A, RType::NS, RType::AAAA].iter()) }
    );
}

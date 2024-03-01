use crate::{resource_record::rtype::RType, serde::wire::{from_wire::FromWire, read_wire::ReadWireError, to_wire::ToWire}};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
struct WindowBlock {
    window_block_number: u8,
    bitmap_length: u8,  //< Must be between 1 and 32, inclusive.
    map: Vec<u8>,
}

impl WindowBlock {
    const MIN_BITMAP_LENGTH: u8 = 1;
    const MAX_BITMAP_LENGTH: u8 = 32;

    #[inline]
    pub fn to_rtypes<'a, 'b>(&'a self) -> impl Iterator<Item = RType> + 'b where 'a: 'b {
        // The pairs (Mask, Offset) are the 8 bit-masks that can be applied to a byte to test if
        // that bit is a 1. If a given bit is a one, it indicates that the rtype represented by that
        // bit is in the set.
        static MASK_AND_OFFSET: [(u8, u16); 8] = [(0b1000_0000, 0), (0b0100_0000, 1), (0b0010_0000, 2), (0b0001_0000, 3), (0b0000_1000, 4), (0b0000_0100, 5), (0b0000_0010, 6), (0b0000_0001, 7)];

        let block_number = self.window_block_number as u16;
        self.map.iter().enumerate().flat_map(move |(bye_index, byte)|
            MASK_AND_OFFSET.iter().filter_map(move |(mask, offset)| {
                if (byte & mask) == *mask {
                    Some(RType::from_code(((block_number * 256) + (bye_index as u16 * 8)) + offset))
                } else {
                    None
                }
            })
        )
    }
}

impl ToWire for WindowBlock {
    #[inline]
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>, compression: &mut Option<crate::serde::wire::compression_map::CompressionMap>) -> Result<(), crate::serde::wire::write_wire::WriteWireError> where 'a: 'b {
        self.window_block_number.to_wire_format(wire, compression)?;
        self.bitmap_length.to_wire_format(wire, compression)?;
        wire.write_bytes(&self.map)
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        self.window_block_number.serial_length()
        + self.bitmap_length.serial_length()
        + (self.map.len() as u16)
    }
}

impl FromWire for WindowBlock {
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>) -> Result<Self, crate::serde::wire::read_wire::ReadWireError> where Self: Sized, 'a: 'b {
        let window_block_number = u8::from_wire_format(wire)?;
        let bitmap_length = u8::from_wire_format(wire)?;

        if (bitmap_length < Self::MIN_BITMAP_LENGTH) || (bitmap_length > Self::MAX_BITMAP_LENGTH) {
            return Err(ReadWireError::OutOfBoundsError(
                format!("the bitmap length must be between {0} and {1} (inclusive)", Self::MIN_BITMAP_LENGTH, Self::MAX_BITMAP_LENGTH)
            ));
        }
        
        let map = <Vec<u8>>::from_wire_format(&mut wire.section_from_current_state(Some(0), Some(bitmap_length as usize))?)?;
        wire.shift(bitmap_length as usize)?;

        return Ok(WindowBlock {
            window_block_number,
            bitmap_length,
            map,
        });
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct RTypeBitmap {
    blocks: Vec<WindowBlock>
}

impl RTypeBitmap {
    #[inline]
    pub fn from_rtypes(type_codes: impl Iterator<Item = RType>) -> Self {
        // There are 256 possible 32-byte windows.
        // Each window is represented as a tuple: (32-byte array, 1-byte bitmap_length).
        let mut all_windows = [([0_u8; 32], 0_u8); 256];

        for rtype in type_codes {
            let code = rtype.code() as usize;
            let window_index = code / 256;
            let byte_index = (code % 256) / 8;
            let bit_index = (code % 256) % 8;
            let mask = 0b1000_0000_u8 >> bit_index;
            
            all_windows[window_index].0[byte_index] |= mask;
            all_windows[window_index].1 = all_windows[window_index].1.max((byte_index as u8) + 1);
        }

        let window_blocks = all_windows.iter()
            .enumerate()
            .filter(|(_, (_, bitmap_length))| *bitmap_length != 0)
            .map(|(window_block_number, (map, bitmap_length))| WindowBlock {
                window_block_number: window_block_number as u8,
                bitmap_length: *bitmap_length,
                map: map[0..(*bitmap_length as usize)].into(),
            });

        Self { blocks: window_blocks.collect() }
    }

    #[inline]
    pub fn to_rtypes<'a, 'b>(&'a self) -> impl Iterator<Item = RType> + 'b where 'a: 'b {
        self.blocks.iter().flat_map(|window| window.to_rtypes())
    }

    #[inline]
    pub fn rtype_count(&self) -> usize {
        self.blocks.iter().fold(0, |accumulator, window|
            window.map.iter().fold(accumulator, |accumulator, byte|
                accumulator + (byte.count_ones() as usize)
            )
        )
    }

    #[inline]
    pub fn has_rtype(&self, rtype: &RType) -> bool {
        let code = rtype.code();
        let window_index = code / 256;
        let byte_index = (code % 256) / 8;
        let bit_index = (code % 256) % 8;
        let mask = 0b1000_0000_u8 >> bit_index;

        for block in &self.blocks {
            if block.window_block_number == (window_index as u8) {
                match block.map.get(byte_index as usize) {
                    Some(byte) => return (byte & mask) == mask,
                    None => return false,
                }
            }
        }
        return false;
    }
}

impl ToWire for RTypeBitmap {
    #[inline]
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>, compression: &mut Option<crate::serde::wire::compression_map::CompressionMap>) -> Result<(), crate::serde::wire::write_wire::WriteWireError> where 'a: 'b {
        self.blocks.to_wire_format(wire, compression)
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        self.blocks.serial_length()
    }
}

impl FromWire for RTypeBitmap {
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>) -> Result<Self, ReadWireError> where Self: Sized, 'a: 'b {
        Ok(RTypeBitmap { blocks: <Vec<WindowBlock>>::from_wire_format(wire)? })
    }
}

#[cfg(test)]
mod circular_sanity_tests {
    use crate::{resource_record::rtype::RType, serde::wire::circular_test::gen_test_circular_serde_sanity_test};
    use super::{RTypeBitmap, WindowBlock};

    macro_rules! circular_sanity_test {
        ($test_encoding:ident, $test_wire:ident, $input:expr) => {
            #[test]
            fn $test_encoding() {
                let init_rtypes = $input;
                let non_init_rtypes = (0..=u16::MAX).into_iter().map(|code| RType::from_code(code)).filter(|code| !init_rtypes.contains(code));
                let bitmap = RTypeBitmap::from_rtypes(init_rtypes.clone().into_iter());

                // Check that the bitmap claims to contain the correct RTypes
                for rtype in init_rtypes.clone().into_iter() {
                    assert!(bitmap.has_rtype(&rtype), "`RType::{rtype}` is missing from the bitmap when `has_rtype()` is called even though it was in the input rtypes")
                }
                for rtype in non_init_rtypes.clone() {
                    assert!(!bitmap.has_rtype(&rtype), "`RType::{rtype}` is found in the bitmap when `has_rtype()` is called even though it was not in the input rtypes")
                }
                // Check the correctness of the `bitmap_length`
                for window in &bitmap.blocks {
                    assert_eq!(window.bitmap_length as usize, window.map.len());
                    assert!(window.bitmap_length >= WindowBlock::MIN_BITMAP_LENGTH);
                    assert!(window.bitmap_length <= WindowBlock::MAX_BITMAP_LENGTH);
                }

                let final_rtypes: Vec<RType> = bitmap.to_rtypes().collect();
                assert_eq!(final_rtypes.len(), bitmap.rtype_count(), "The length reported by `rtype_count()` is different than the length of the iterator generated by `to_rtypes()`");
                for rtype in init_rtypes.iter() {
                    assert!(final_rtypes.contains(rtype), "`RType::{rtype}` is missing from rtypes generated by `to_rtypes()` even though it were in the input rtypes")
                }
                for rtype in non_init_rtypes.clone() {
                    assert!(!final_rtypes.contains(&rtype), "`RType::{rtype}` is found in the rtypes generated by `to_rtypes()` even though it was not in the input rtypes")
                }
            }

            gen_test_circular_serde_sanity_test!($test_wire, RTypeBitmap::from_rtypes($input.into_iter()));
        }
    }

    circular_sanity_test!(test_0_rtypes_from_to_collection, test_0_rtypes_wire, vec![]);
    circular_sanity_test!(test_1_rtypes_from_to_collection, test_1_rtypes_wire, vec![RType::A]);
    circular_sanity_test!(test_2_rtypes_from_to_collection, test_2_rtypes_wire, vec![RType::A, RType::AAAA]);
    circular_sanity_test!(test_3_rtypes_from_to_collection, test_3_rtypes_wire, vec![RType::A, RType::AAAA, RType::NS]);
    circular_sanity_test!(test_4_rtypes_from_to_collection, test_4_rtypes_wire, vec![RType::A, RType::AAAA, RType::NS, RType::HTTPS]);
    circular_sanity_test!(test_5_rtypes_from_to_collection, test_5_rtypes_wire, vec![RType::A, RType::NS, RType::MF, RType::CNAME, RType::SOA]);
    circular_sanity_test!(test_6_rtypes_from_to_collection, test_6_rtypes_wire, vec![RType::TKEY, RType::TSIG, RType::IXFR, RType::AXFR, RType::MAILB, RType::MAILA]);
    circular_sanity_test!(test_7_rtypes_from_to_collection, test_7_rtypes_wire, vec![RType::HIP, RType::NINFO, RType::RKEY, RType::TALINK, RType::CDS, RType::CDNSKEY, RType::A6]);
    circular_sanity_test!(test_8_rtypes_from_to_collection, test_8_rtypes_wire, vec![RType::NS, RType::NULL, RType::AFSDB, RType::PX, RType::ATMA, RType::APL, RType::NSEC3, RType::TALINK]);
    circular_sanity_test!(test_9_rtypes_from_to_collection, test_9_rtypes_wire, vec![RType::NS, RType::NULL, RType::AFSDB, RType::PX, RType::ATMA, RType::APL, RType::NSEC3, RType::TALINK, RType::SPF]);
    circular_sanity_test!(test_10_rtypes_from_to_collection, test_10_rtypes_wire, vec![RType::NS, RType::NULL, RType::AFSDB, RType::PX, RType::ATMA, RType::APL, RType::NSEC3, RType::TALINK, RType::SPF, RType::CAA]);
    circular_sanity_test!(test_11_rtypes_from_to_collection, test_11_rtypes_wire, vec![RType::A, RType::NS, RType::MF, RType::CNAME, RType::SOA, RType::MB, RType::MG, RType::MR, RType::NULL, RType::WKS, RType::PTR, RType::DLV]);
    circular_sanity_test!(test_12_rtypes_from_to_collection, test_12_rtypes_wire, vec![RType::A, RType::NS, RType::MF, RType::CNAME, RType::SOA, RType::MB, RType::MG, RType::MR, RType::NULL, RType::WKS, RType::PTR, RType::HINFO]);
    circular_sanity_test!(test_12_rtypes_2_repeats_from_to_collection, test_12_rtypes_2_repeats_wire, vec![RType::SOA, RType::NS, RType::MF, RType::CNAME, RType::SOA, RType::MB, RType::MG, RType::MR, RType::WKS, RType::WKS, RType::PTR, RType::HINFO]);
}

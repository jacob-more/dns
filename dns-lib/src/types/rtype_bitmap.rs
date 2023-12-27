use crate::serde::wire::{to_wire::ToWire, from_wire::FromWire, read_wire::ReadWireError};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
struct WindowBlock {
    window_block_number: u8,
    bitmap_length: u8,  //< Must be between 1 and 32, inclusive.
    map: Vec<u8>,
}

impl WindowBlock {
    const MIN_BITMAP_LENGTH: u8 = 1;
    const MAX_BITMAP_LENGTH: u8 = 32;
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

// TODO: Implement a function to convert from a collection of RR Type Codes into the RTypeBitmap

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

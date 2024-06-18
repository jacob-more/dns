use std::{error::Error, fmt::Display, ops::{Bound, RangeBounds}};

use crate::{types::{c_domain_name::CDomainNameError, ascii::AsciiError, base16::Base16Error, base32::Base32Error, extended_base32::ExtendedBase32Error, base64::Base64Error, domain_name::DomainNameError}, resource_record::rtype::RType};

#[derive(Debug)]
pub enum ReadWireError {
    FormatError(String),
    OverflowError(String),
    UnderflowError(String),
    OutOfBoundsError(String),
    UnsupportedRType(RType),
    ValueError(String),
    VersionError(String),
    CDomainNameError(CDomainNameError),
    DomainNameError(DomainNameError),
    AsciiError(AsciiError),
    Base16Error(Base16Error),
    Base32Error(Base32Error),
    ExtendedBase32Error(ExtendedBase32Error),
    Base64Error(Base64Error),
}
impl Error for ReadWireError {}
impl Display for ReadWireError {
     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FormatError(error) => write!(f, "Read Wire Format Error: {error}"),
            Self::OverflowError(error) => write!(f, "Read Wire Overflow Error: {error}"),
            Self::UnderflowError(error) => write!(f, "Read Wire Underflow Error: {error}"),
            Self::OutOfBoundsError(error) => write!(f, "Read Wire Out Of Bounds Error: {error}"),
            Self::UnsupportedRType(rtype) => write!(f, "Resource Record Type {rtype} is not supported"),
            Self::ValueError(error) => write!(f, "Read Wire Value Error: {error}"),
            Self::VersionError(error) => write!(f, "Read Wire Version Error: {error}"),

            Self::CDomainNameError(error) => write!(f, "Read Wire Compressible Domain Name Error: {error}"),
            Self::DomainNameError(error) => write!(f, "Read Wire Incompressible Domain Name Error: {error}"),
            Self::AsciiError(error) => write!(f, "Read Wire Ascii Error: {error}"),

            Self::Base16Error(error) => write!(f, "Read Wire Base16 Error: {error}"),
            Self::Base32Error(error) => write!(f, "Read Wire Bsse32 Error: {error}"),
            Self::ExtendedBase32Error(error) => write!(f, "Read Wire Extended Base32 Error: {error}"),
            Self::Base64Error(error) => write!(f, "Read Wire Base64 Error: {error}"),
        }
    }
}
impl From<CDomainNameError> for ReadWireError {
    fn from(value: CDomainNameError) -> Self {
        ReadWireError::CDomainNameError(value)
    }
}
impl From<DomainNameError> for ReadWireError {
    fn from(value: DomainNameError) -> Self {
        ReadWireError::DomainNameError(value)
    }
}
impl From<AsciiError> for ReadWireError {
    fn from(value: AsciiError) -> Self {
        ReadWireError::AsciiError(value)
    }
}
impl From<Base16Error> for ReadWireError {
    fn from(value: Base16Error) -> Self {
        ReadWireError::Base16Error(value)
    }
}
impl From<Base32Error> for ReadWireError {
    fn from(value: Base32Error) -> Self {
        ReadWireError::Base32Error(value)
    }
}
impl From<ExtendedBase32Error> for ReadWireError {
    fn from(value: ExtendedBase32Error) -> Self {
        ReadWireError::ExtendedBase32Error(value)
    }
}
impl From<Base64Error> for ReadWireError {
    fn from(value: Base64Error) -> Self {
        ReadWireError::Base64Error(value)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum WireVisibility {
    Entire,
    Current,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SliceWireVisibility {
    Entire,
    Current,
    Slice,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ReadWire<'a> {
    wire: &'a [u8],
    offset: usize,
}

impl<'a> ReadWire<'a> {
    #[inline]
    pub fn from_bytes(wire: &'a [u8]) -> Self {
        Self { wire, offset: 0 }
    }

    #[inline]
    pub fn current(&'a self) -> &'a [u8] { &self.wire[self.offset..] }

    #[inline]
    pub fn current_len(&self) -> usize { self.current().len() }

    #[inline]
    pub fn current_offset(&self) -> usize { self.offset }

    #[inline]
    pub fn is_end_reached(&self) -> bool { self.offset >= self.wire.len() }

    #[inline]
    pub fn with_offset(&'a self, offset: usize, visibility: WireVisibility) -> Result<Self, ReadWireError> {
        if self.wire.len() < offset {
            Err(ReadWireError::OverflowError(
                String::from("offset went past the end of the wire")
            ))
        } else {
            match visibility {
                WireVisibility::Entire => Ok(Self { wire: self.wire(), offset }),
                WireVisibility::Current => Ok(Self { wire: self.current(), offset }),
            }
        }
    }

    #[inline]
    pub fn set_offset(&mut self, offset: usize) -> Result<(), ReadWireError> {
        if self.wire.len() < offset {
            return Err(ReadWireError::OverflowError(
                String::from("offset went past the end of the wire")
            ));
        } else {
            self.offset = offset;
            return Ok(());
        }
    }

    #[inline]
    pub fn from_offset(&'a self, offset: usize) -> Result<&'a [u8], ReadWireError> {
        if self.wire.len() < offset {
            return Err(ReadWireError::OverflowError(
                String::from("offset went past the end of the wire")
            ));
        } else {
            return Ok(&self.wire[offset..]);
        }
    }

    #[inline]
    pub fn wire(&'a self) -> &'a [u8] { self.wire }

    #[inline]
    pub fn wire_len(&self) -> usize { self.wire.len() }

    #[inline]
    pub fn shift(&mut self, shift: usize) -> Result<(), ReadWireError> {
        let new_offset = self.offset + shift;
        if self.wire.len() < new_offset {
            return Err(ReadWireError::OverflowError(
                String::from("attempted to shift past the end of the wire")
            ));
        } else {
            self.offset = new_offset;
            return Ok(());
        }
    }

    #[inline]
    pub fn get(&'a self, count: usize) -> Result<&'a [u8], ReadWireError> {
        if self.current_len() >= count {
            Ok(&self.wire[self.offset..(self.offset + count)])
        } else {
            Err(ReadWireError::OverflowError(
                format!("getting {count} bytes would have read past the end of the wire")
            ))
        }
    }

    #[inline]
    pub fn take(&'a mut self, count: usize) -> Result<&'a [u8], ReadWireError> {
        if self.current_len() >= count {
            let offset = self.offset;
            self.offset += count;
            Ok(&self.wire[offset..(offset + count)])
        } else {
            Err(ReadWireError::OverflowError(
                format!("getting {count} bytes would have read past the end of the wire")
            ))
        }
    }

    #[inline]
    pub fn take_all(&'a mut self) -> Result<&'a [u8], ReadWireError> {
        self.take(self.current_len())
    }

    #[inline]
    pub fn get_byte(&self) -> Result<u8, ReadWireError> {
        if self.current_len() >= 1 {
            Ok(self.wire[self.offset + 1])
        } else {
            Err(ReadWireError::OverflowError(
                format!("getting a byte would have read past the end of the wire")
            ))
        }
    }

    #[inline]
    pub fn take_byte(&mut self) -> Result<u8, ReadWireError> {
        if self.current_len() >= 1 {
            let offset = self.offset;
            self.offset += 1;
            Ok(self.wire[offset + 1])
        } else {
            Err(ReadWireError::OverflowError(
                format!("getting a byte would have read past the end of the wire")
            ))
        }
    }

    #[inline]
    pub fn section_from_current(&self, lower_bound: Option<usize>, upper_bound: Option<usize>) -> Result<Self, ReadWireError> {
        let (lower_bound, new_offset) = match lower_bound {
            None => (0, self.offset),
            Some(lower_bound) => (lower_bound + self.offset, 0),
        };
        let upper_bound = match upper_bound {
            None => self.wire.len(),
            Some(upper_bound) => upper_bound + self.offset,
        };

        if lower_bound > upper_bound {
            return Err(ReadWireError::ValueError(
                String::from("lower bound cannot be greater than the upper bound")
            ));
        } else if upper_bound > self.wire.len() {
            return Err(ReadWireError::OverflowError(
                String::from("upper bound cannot be greater than the end of the wire")
            ));
        } else {
            return Ok(Self {
                wire: &self.wire[lower_bound..upper_bound],
                offset: new_offset,
            });
        }
    }

    /// Gets a slice of `self` for the given range. The amount of wire that is visible to the
    /// returned `ReadWire` is dependent on the value of `visibility`. However, this `ReadWire` can
    /// only make as much of the wire visible as `self` can see.
    /// 
    /// `start` is the lower bound of the range, where `start == 0` is equivalent to the current
    /// wire offset.
    /// `end` is the upper bound of the range, where `end == 0` is equivalent to the current wire
    /// offset.
    /// In other words, both `start` and `end` are relative to `self.current_offset()`.
    /// 
    /// SliceWireVisibility::Entire - The entire wire, from `0` to `len` can be made visible.
    ///   `start` will be used to determine the new offset. `end` will limit how much of the tail
    ///   of the wire is visible.
    /// SliceWireVisibility::Current - Only the current wire can be made visible, from
    ///   `self.current_offset()` to `len`. `start` will be used to determine the new offset.
    ///   `end` will limit how much of the tail of the wire is visible.
    /// SliceWireVisibility::Slice - Only the wire within the bounds of `start` to `end` will be
    ///   visible. The offset of the new wire will be `0`.
    #[inline]
    pub fn slice_from_current(&self, range: impl RangeBounds<usize>, visibility: SliceWireVisibility) -> Result<Self, ReadWireError> {
        let current_len = self.current_len();
        let start = match range.start_bound() {
            Bound::Included(&start) => start,
            Bound::Excluded(&start) => start.checked_add(1).expect("out of range"),
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(&end) => end.checked_add(1).expect("out of range"),
            Bound::Excluded(&end) => end,
            Bound::Unbounded => current_len,
        };
        if end < start {
            panic!("range start must not be greater than end: {:?} <= {:?}", start, end);
        }
        if current_len < end {
            panic!("range end out of bounds: {:?} <= {:?}", end, current_len);
        }

        match visibility {
            SliceWireVisibility::Entire => Ok(Self { wire: &self.wire[..(self.offset + end)], offset: self.offset + start }),
            SliceWireVisibility::Current => Ok(Self { wire: &self.wire[self.offset..(self.offset + end)], offset: start }),
            SliceWireVisibility::Slice => Ok(Self { wire: &self.wire[(self.offset + start)..(self.offset + end)], offset: 0 }),
        }
    }
}

#[cfg(test)]
mod test_current_state {
    use super::ReadWire;

    #[test]
    fn test_current_state_empty() {
        let wire = &[];
        let read_wire = ReadWire::from_bytes(wire);

        assert_eq!(wire, read_wire.current());
        assert_eq!(0, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(read_wire.is_end_reached());
    }

    #[test]
    fn test_current_state_one_byte() {
        let wire = &[1];
        let read_wire = ReadWire::from_bytes(wire);

        assert_eq!(wire, read_wire.current());
        assert_eq!(1, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_current_state_two_bytes() {
        let wire = &[1, 2];
        let read_wire = ReadWire::from_bytes(wire);

        assert_eq!(wire, read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }
}

#[cfg(test)]
mod test_set_offset {
    use super::ReadWire;

    #[test]
    fn test_two_bytes_set_offset_middle() {
        let wire = &[1, 2];
        let mut read_wire = ReadWire::from_bytes(wire);

        let expected_wire = &[2];

        assert!(read_wire.set_offset(1).is_ok());

        // Verify state.
        assert_eq!(expected_wire, read_wire.current());
        assert_eq!(1, read_wire.current_len());
        assert_eq!(1, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_empty_set_offset_end() {
        let wire = &[];
        let mut read_wire = ReadWire::from_bytes(wire);

        let expected_wire: &[u8; 0] = &[];

        assert!(read_wire.set_offset(0).is_ok());

        // Verify state.
        assert_eq!(expected_wire, read_wire.current());
        assert_eq!(0, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(read_wire.is_end_reached());
    }

    #[test]
    fn test_one_byte_set_offset_end() {
        let wire = &[1];
        let mut read_wire = ReadWire::from_bytes(wire);

        let expected_wire: &[u8; 0] = &[];

        assert!(read_wire.set_offset(1).is_ok());

        // Verify state.
        assert_eq!(expected_wire, read_wire.current());
        assert_eq!(0, read_wire.current_len());
        assert_eq!(1, read_wire.current_offset());
        assert!(read_wire.is_end_reached());
    }

    #[test]
    fn test_two_bytes_set_offset_end() {
        let wire = &[1, 2];
        let mut read_wire = ReadWire::from_bytes(wire);

        let expected_wire: &[u8; 0] = &[];

        assert!(read_wire.set_offset(2).is_ok());

        // Verify state.
        assert_eq!(expected_wire, read_wire.current());
        assert_eq!(0, read_wire.current_len());
        assert_eq!(2, read_wire.current_offset());
        assert!(read_wire.is_end_reached());
    }

    #[test]
    fn test_empty_set_offset_past_end() {
        let wire = &[];
        let mut read_wire = ReadWire::from_bytes(wire);

        // Verify setting offset past the end fails.
        assert!(read_wire.set_offset(1).is_err());

        // Verify state.
        assert_eq!(wire, read_wire.current());
        assert_eq!(0, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(read_wire.is_end_reached());
    }

    #[test]
    fn test_one_byte_set_offset_past_end() {
        let wire = &[1];
        let mut read_wire = ReadWire::from_bytes(wire);

        // Verify setting offset past the end fails.
        assert!(read_wire.set_offset(2).is_err());

        // Verify state.
        assert_eq!(wire, read_wire.current());
        assert_eq!(1, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_two_bytes_set_offset_past_end() {
        let wire = &[1, 2];
        let mut read_wire = ReadWire::from_bytes(wire);

        // Verify setting offset past the end fails.
        assert!(read_wire.set_offset(3).is_err());

        // Verify state.
        assert_eq!(wire, read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }
}

#[cfg(test)]
mod test_state_from_offset {
    use super::ReadWire;

    #[test]
    fn test_two_bytes_state_from_offset_middle() {
        let wire = &[1, 2];
        let read_wire = ReadWire::from_bytes(wire);

        let expected_wire = &[2];

        let actual_read_wire = read_wire.from_offset(1);
        assert!(actual_read_wire.is_ok());
        let actual_wire = actual_read_wire.unwrap();
        assert_eq!(expected_wire, actual_wire);

        // Verify original state is unchanged.
        assert_eq!(wire, read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_empty_state_from_offset_end() {
        let wire = &[];
        let read_wire = ReadWire::from_bytes(wire);

        let expected_wire: &[u8; 0] = &[];

        let actual_read_wire = read_wire.from_offset(0);
        assert!(actual_read_wire.is_ok());
        let actual_wire = actual_read_wire.unwrap();
        assert_eq!(expected_wire, actual_wire);

        // Verify original state is unchanged.
        assert_eq!(wire, read_wire.current());
        assert_eq!(0, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(read_wire.is_end_reached());
    }

    #[test]
    fn test_one_byte_state_from_offset_end() {
        let wire = &[1];
        let read_wire = ReadWire::from_bytes(wire);

        let expected_wire: &[u8; 0] = &[];

        let actual_read_wire = read_wire.from_offset(1);
        assert!(actual_read_wire.is_ok());
        let actual_wire = actual_read_wire.unwrap();
        assert_eq!(expected_wire, actual_wire);

        // Verify original state is unchanged.
        assert_eq!(wire, read_wire.current());
        assert_eq!(1, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_two_bytes_state_from_offset_end() {
        let wire = &[1, 2];
        let read_wire = ReadWire::from_bytes(wire);

        let expected_wire: &[u8; 0] = &[];

        let actual_read_wire = read_wire.from_offset(2);
        assert!(actual_read_wire.is_ok());
        let actual_wire = actual_read_wire.unwrap();
        assert_eq!(expected_wire, actual_wire);

        // Verify original state is unchanged.
        assert_eq!(wire, read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_empty_state_from_offset_past_end() {
        let wire = &[];
        let read_wire = ReadWire::from_bytes(wire);

        let actual_read_wire = read_wire.from_offset(1);
        assert!(actual_read_wire.is_err());

        // Verify original state is unchanged.
        assert_eq!(wire, read_wire.current());
        assert_eq!(0, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(read_wire.is_end_reached());
    }

    #[test]
    fn test_one_byte_state_from_offset_past_end() {
        let wire = &[1];
        let read_wire = ReadWire::from_bytes(wire);

        let actual_read_wire = read_wire.from_offset(2);
        assert!(actual_read_wire.is_err());

        // Verify original state is unchanged.
        assert_eq!(wire, read_wire.current());
        assert_eq!(1, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_two_bytes_state_from_offset_past_end() {
        let wire = &[1, 2];
        let read_wire = ReadWire::from_bytes(wire);

        let actual_read_wire = read_wire.from_offset(3);
        assert!(actual_read_wire.is_err());

        // Verify original state is unchanged.
        assert_eq!(wire, read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }
}

#[cfg(test)]
mod test_shift {
    use super::ReadWire;

    #[test]
    fn test_two_bytes_shift_none() {
        let wire = &[1, 2];
        let mut read_wire = ReadWire::from_bytes(wire);

        let expected_wire = &[1, 2];

        assert!(read_wire.shift(0).is_ok());

        // Verify state.
        assert_eq!(expected_wire, read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_two_bytes_shift_middle() {
        let wire = &[1, 2];
        let mut read_wire = ReadWire::from_bytes(wire);

        let expected_wire = &[2];

        assert!(read_wire.shift(1).is_ok());

        // Verify state.
        assert_eq!(expected_wire, read_wire.current());
        assert_eq!(1, read_wire.current_len());
        assert_eq!(1, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_empty_shift_end() {
        let wire = &[];
        let mut read_wire = ReadWire::from_bytes(wire);

        let expected_wire: &[u8; 0] = &[];

        assert!(read_wire.shift(0).is_ok());

        // Verify state.
        assert_eq!(expected_wire, read_wire.current());
        assert_eq!(0, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(read_wire.is_end_reached());
    }

    #[test]
    fn test_one_byte_shift_end() {
        let wire = &[1];
        let mut read_wire = ReadWire::from_bytes(wire);

        let expected_wire: &[u8; 0] = &[];

        assert!(read_wire.shift(1).is_ok());

        // Verify state.
        assert_eq!(expected_wire, read_wire.current());
        assert_eq!(0, read_wire.current_len());
        assert_eq!(1, read_wire.current_offset());
        assert!(read_wire.is_end_reached());
    }

    #[test]
    fn test_two_bytes_shift_end() {
        let wire = &[1, 2];
        let mut read_wire = ReadWire::from_bytes(wire);

        let expected_wire: &[u8; 0] = &[];

        assert!(read_wire.shift(2).is_ok());

        // Verify state.
        assert_eq!(expected_wire, read_wire.current());
        assert_eq!(0, read_wire.current_len());
        assert_eq!(2, read_wire.current_offset());
        assert!(read_wire.is_end_reached());
    }

    #[test]
    fn test_empty_shift_past_end() {
        let wire = &[];
        let mut read_wire = ReadWire::from_bytes(wire);

        assert!(read_wire.shift(1).is_err());

        // Verify state.
        assert_eq!(wire, read_wire.current());
        assert_eq!(0, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(read_wire.is_end_reached());
    }

    #[test]
    fn test_one_byte_shift_past_end() {
        let wire = &[1];
        let mut read_wire = ReadWire::from_bytes(wire);

        assert!(read_wire.shift(2).is_err());

        // Verify state.
        assert_eq!(wire, read_wire.current());
        assert_eq!(1, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_two_bytes_shift_past_end() {
        let wire = &[1, 2];
        let mut read_wire = ReadWire::from_bytes(wire);

        assert!(read_wire.shift(3).is_err());

        // Verify state.
        assert_eq!(wire, read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_two_bytes_shift_from_middle_to_end() {
        let wire = &[1, 2];
        let mut read_wire = ReadWire { wire, offset: 1 };

        let expected_wire: &[u8; 0] = &[];

        assert!(read_wire.shift(1).is_ok());

        // Verify state.
        assert_eq!(expected_wire, read_wire.current());
        assert_eq!(0, read_wire.current_len());
        assert_eq!(2, read_wire.current_offset());
        assert!(read_wire.is_end_reached());
    }

    #[test]
    fn test_one_byte_shift_from_end_to_end() {
        let wire = &[1, 2];
        let mut read_wire = ReadWire { wire, offset: 2 };

        let expected_wire: &[u8; 0] = &[];

        assert!(read_wire.shift(0).is_ok());

        // Verify state.
        assert_eq!(expected_wire, read_wire.current());
        assert_eq!(0, read_wire.current_len());
        assert_eq!(2, read_wire.current_offset());
        assert!(read_wire.is_end_reached());
    }

    #[test]
    fn test_two_bytes_shift_from_middle_past_end() {
        let wire = &[1, 2];
        let mut read_wire = ReadWire { wire, offset: 1 };

        let expected_wire = &[2];

        assert!(read_wire.shift(2).is_err());

        // Verify state.
        assert_eq!(expected_wire, read_wire.current());
        assert_eq!(1, read_wire.current_len());
        assert_eq!(1, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_one_byte_shift_from_end_past_end() {
        let wire = &[1, 2];
        let mut read_wire = ReadWire { wire, offset: 2 };

        let expected_wire: &[u8; 0] = &[];

        assert!(read_wire.shift(1).is_err());

        // Verify state.
        assert_eq!(expected_wire, read_wire.current());
        assert_eq!(0, read_wire.current_len());
        assert_eq!(2, read_wire.current_offset());
        assert!(read_wire.is_end_reached());
    }
}

#[cfg(test)]
mod test_section_from_current_state {
    use super::ReadWire;

    #[test]
    fn test_empty_get_none_to_none() {
        let wire = &[];
        let read_wire = ReadWire::from_bytes(wire);

        let expected_wire: &[u8; 0] = &[];
        let expected_read_wire = ReadWire { wire: expected_wire, offset: 0 };

        let actual_read_wire_result = read_wire.section_from_current(None, None);
        assert!(actual_read_wire_result.is_ok());
        let actual_read_wire = actual_read_wire_result.unwrap();
        assert_eq!(expected_read_wire, actual_read_wire);

        // Verify original wire's state.
        assert_eq!(wire, read_wire.current());
        assert_eq!(0, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(read_wire.is_end_reached());
    }

    #[test]
    fn test_one_byte_get_none_to_none() {
        let wire = &[1];
        let read_wire = ReadWire::from_bytes(wire);

        let expected_wire= &[1];
        let expected_read_wire = ReadWire { wire: expected_wire, offset: 0 };

        let actual_read_wire_result = read_wire.section_from_current(None, None);
        assert!(actual_read_wire_result.is_ok());
        let actual_read_wire = actual_read_wire_result.unwrap();
        assert_eq!(expected_read_wire, actual_read_wire);

        // Verify original wire's state.
        assert_eq!(wire, read_wire.current());
        assert_eq!(1, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_two_bytes_get_none_to_none() {
        let wire = &[1, 2];
        let read_wire = ReadWire::from_bytes(wire);

        let expected_wire= &[1, 2];
        let expected_read_wire = ReadWire { wire: expected_wire, offset: 0 };

        let actual_read_wire_result = read_wire.section_from_current(None, None);
        assert!(actual_read_wire_result.is_ok());
        let actual_read_wire = actual_read_wire_result.unwrap();
        assert_eq!(expected_read_wire, actual_read_wire);

        // Verify original wire's state.
        assert_eq!(wire, read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_two_bytes_get_start_to_none() {
        let wire = &[1, 2];
        let read_wire = ReadWire::from_bytes(wire);

        let expected_wire= &[1, 2];
        let expected_read_wire = ReadWire { wire: expected_wire, offset: 0 };

        let actual_read_wire_result = read_wire.section_from_current(Some(0), None);
        assert!(actual_read_wire_result.is_ok());
        let actual_read_wire = actual_read_wire_result.unwrap();
        assert_eq!(expected_read_wire, actual_read_wire);

        // Verify original wire's state.
        assert_eq!(wire, read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_two_bytes_get_middle_to_none() {
        let wire = &[1, 2];
        let read_wire = ReadWire::from_bytes(wire);

        let expected_wire= &[2];
        let expected_read_wire = ReadWire { wire: expected_wire, offset: 0 };

        let actual_read_wire_result = read_wire.section_from_current(Some(1), None);
        assert!(actual_read_wire_result.is_ok());
        let actual_read_wire = actual_read_wire_result.unwrap();
        assert_eq!(expected_read_wire, actual_read_wire);

        // Verify original wire's state.
        assert_eq!(wire, read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_two_bytes_get_end_to_none() {
        let wire = &[1, 2];
        let read_wire = ReadWire::from_bytes(wire);

        let expected_wire= &[];
        let expected_read_wire = ReadWire { wire: expected_wire, offset: 0 };

        let actual_read_wire_result = read_wire.section_from_current(Some(2), None);
        assert!(actual_read_wire_result.is_ok());
        let actual_read_wire = actual_read_wire_result.unwrap();
        assert_eq!(expected_read_wire, actual_read_wire);

        // Verify original wire's state.
        assert_eq!(wire, read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_two_bytes_get_none_to_start() {
        let wire = &[1, 2];
        let read_wire = ReadWire::from_bytes(wire);

        let expected_wire= &[];
        let expected_read_wire = ReadWire { wire: expected_wire, offset: 0 };

        let actual_read_wire_result = read_wire.section_from_current(None, Some(0));
        assert!(actual_read_wire_result.is_ok());
        let actual_read_wire = actual_read_wire_result.unwrap();
        assert_eq!(expected_read_wire, actual_read_wire);

        // Verify original wire's state.
        assert_eq!(wire, read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_two_bytes_get_none_to_middle() {
        let wire = &[1, 2];
        let read_wire = ReadWire::from_bytes(wire);

        let expected_wire= &[1];
        let expected_read_wire = ReadWire { wire: expected_wire, offset: 0 };

        let actual_read_wire_result = read_wire.section_from_current(None, Some(1));
        assert!(actual_read_wire_result.is_ok());
        let actual_read_wire = actual_read_wire_result.unwrap();
        assert_eq!(expected_read_wire, actual_read_wire);

        // Verify original wire's state.
        assert_eq!(wire, read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_two_bytes_get_none_to_end() {
        let wire = &[1, 2];
        let read_wire = ReadWire::from_bytes(wire);

        let expected_wire= &[1, 2];
        let expected_read_wire = ReadWire { wire: expected_wire, offset: 0 };

        let actual_read_wire_result = read_wire.section_from_current(None, Some(2));
        assert!(actual_read_wire_result.is_ok());
        let actual_read_wire = actual_read_wire_result.unwrap();
        assert_eq!(expected_read_wire, actual_read_wire);

        // Verify original wire's state.
        assert_eq!(wire, read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_two_bytes_get_start_to_start() {
        let wire = &[1, 2];
        let read_wire = ReadWire::from_bytes(wire);

        let expected_wire= &[];
        let expected_read_wire = ReadWire { wire: expected_wire, offset: 0 };

        let actual_read_wire_result = read_wire.section_from_current(Some(0), Some(0));
        assert!(actual_read_wire_result.is_ok());
        let actual_read_wire = actual_read_wire_result.unwrap();
        assert_eq!(expected_read_wire, actual_read_wire);

        // Verify original wire's state.
        assert_eq!(wire, read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_two_bytes_get_start_to_middle() {
        let wire = &[1, 2];
        let read_wire = ReadWire::from_bytes(wire);

        let expected_wire= &[1];
        let expected_read_wire = ReadWire { wire: expected_wire, offset: 0 };

        let actual_read_wire_result = read_wire.section_from_current(Some(0), Some(1));
        assert!(actual_read_wire_result.is_ok());
        let actual_read_wire = actual_read_wire_result.unwrap();
        assert_eq!(expected_read_wire, actual_read_wire);

        // Verify original wire's state.
        assert_eq!(wire, read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_two_bytes_get_start_to_end() {
        let wire = &[1, 2];
        let read_wire = ReadWire::from_bytes(wire);

        let expected_wire= &[1, 2];
        let expected_read_wire = ReadWire { wire: expected_wire, offset: 0 };

        let actual_read_wire_result = read_wire.section_from_current(Some(0), Some(2));
        assert!(actual_read_wire_result.is_ok());
        let actual_read_wire = actual_read_wire_result.unwrap();
        assert_eq!(expected_read_wire, actual_read_wire);

        // Verify original wire's state.
        assert_eq!(wire, read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_two_bytes_get_start_to_past_end() {
        let wire = &[1, 2];
        let read_wire = ReadWire::from_bytes(wire);

        let actual_read_wire_result = read_wire.section_from_current(Some(0), Some(3));
        assert!(actual_read_wire_result.is_err());

        // Verify original wire's state.
        assert_eq!(wire, read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_two_bytes_get_middle_to_start() {
        let wire = &[1, 2];
        let read_wire = ReadWire::from_bytes(wire);

        let actual_read_wire_result = read_wire.section_from_current(Some(1), Some(0));
        assert!(actual_read_wire_result.is_err());
        // Verify original wire's state.
        assert_eq!(wire, read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_two_bytes_get_middle_to_middle() {
        let wire = &[1, 2];
        let read_wire = ReadWire::from_bytes(wire);

        let expected_wire= &[];
        let expected_read_wire = ReadWire { wire: expected_wire, offset: 0 };

        let actual_read_wire_result = read_wire.section_from_current(Some(1), Some(1));
        assert!(actual_read_wire_result.is_ok());
        let actual_read_wire = actual_read_wire_result.unwrap();
        assert_eq!(expected_read_wire, actual_read_wire);

        // Verify original wire's state.
        assert_eq!(wire, read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_two_bytes_get_middle_to_end() {
        let wire = &[1, 2];
        let read_wire = ReadWire::from_bytes(wire);

        let expected_wire= &[2];
        let expected_read_wire = ReadWire { wire: expected_wire, offset: 0 };

        let actual_read_wire_result = read_wire.section_from_current(Some(1), Some(2));
        assert!(actual_read_wire_result.is_ok());
        let actual_read_wire = actual_read_wire_result.unwrap();
        assert_eq!(expected_read_wire, actual_read_wire);

        // Verify original wire's state.
        assert_eq!(wire, read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_two_bytes_get_middle_to_past_end() {
        let wire = &[1, 2];
        let read_wire = ReadWire::from_bytes(wire);

        let actual_read_wire_result = read_wire.section_from_current(Some(1), Some(3));
        assert!(actual_read_wire_result.is_err());

        // Verify original wire's state.
        assert_eq!(wire, read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_two_bytes_get_end_to_start() {
        let wire = &[1, 2];
        let read_wire = ReadWire::from_bytes(wire);

        let actual_read_wire_result = read_wire.section_from_current(Some(1), Some(0));
        assert!(actual_read_wire_result.is_err());

        // Verify original wire's state.
        assert_eq!(wire, read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_two_bytes_get_end_to_middle() {
        let wire = &[1, 2];
        let read_wire = ReadWire::from_bytes(wire);

        let actual_read_wire_result = read_wire.section_from_current(Some(2), Some(1));
        assert!(actual_read_wire_result.is_err());

        // Verify original wire's state.
        assert_eq!(wire, read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_two_bytes_get_end_to_end() {
        let wire = &[1, 2];
        let read_wire = ReadWire::from_bytes(wire);

        let expected_wire= &[];
        let expected_read_wire = ReadWire { wire: expected_wire, offset: 0 };

        let actual_read_wire_result = read_wire.section_from_current(Some(2), Some(2));
        assert!(actual_read_wire_result.is_ok());
        let actual_read_wire = actual_read_wire_result.unwrap();
        assert_eq!(expected_read_wire, actual_read_wire);

        // Verify original wire's state.
        assert_eq!(wire, read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_two_bytes_get_end_to_past_end() {
        let wire = &[1, 2];
        let read_wire = ReadWire::from_bytes(wire);

        let actual_read_wire_result = read_wire.section_from_current(Some(2), Some(3));
        assert!(actual_read_wire_result.is_err());

        // Verify original wire's state.
        assert_eq!(wire, read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_two_bytes_get_past_end_to_start() {
        let wire = &[1, 2];
        let read_wire = ReadWire::from_bytes(wire);

        let actual_read_wire_result = read_wire.section_from_current(Some(3), Some(0));
        assert!(actual_read_wire_result.is_err());

        // Verify original wire's state.
        assert_eq!(wire, read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_two_bytes_get_past_end_to_middle() {
        let wire = &[1, 2];
        let read_wire = ReadWire::from_bytes(wire);

        let actual_read_wire_result = read_wire.section_from_current(Some(3), Some(1));
        assert!(actual_read_wire_result.is_err());

        // Verify original wire's state.
        assert_eq!(wire, read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_two_bytes_get_past_end_to_end() {
        let wire = &[1, 2];
        let read_wire = ReadWire::from_bytes(wire);

        let actual_read_wire_result = read_wire.section_from_current(Some(3), Some(2));
        assert!(actual_read_wire_result.is_err());

        // Verify original wire's state.
        assert_eq!(wire, read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_two_bytes_get_past_end_to_past_end() {
        let wire = &[1, 2];
        let read_wire = ReadWire::from_bytes(wire);

        let actual_read_wire_result = read_wire.section_from_current(Some(3), Some(3));
        assert!(actual_read_wire_result.is_err());

        // Verify original wire's state.
        assert_eq!(wire, read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(0, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_one_byte_with_offset_1_get_none_to_none() {
        let wire = &[0];
        let read_wire = ReadWire { wire, offset: 1 };

        let expected_wire = &[0];
        let expected_read_wire = ReadWire { wire: expected_wire, offset: 1 };

        let actual_read_wire_result = read_wire.section_from_current(None, None);
        assert!(actual_read_wire_result.is_ok());
        let actual_read_wire = actual_read_wire_result.unwrap();
        assert_eq!(expected_read_wire, actual_read_wire);

        // Verify original wire's state.
        assert_eq!(&wire[1..], read_wire.current());
        assert_eq!(0, read_wire.current_len());
        assert_eq!(1, read_wire.current_offset());
        assert!(read_wire.is_end_reached());
    }

    #[test]
    fn test_two_bytes_with_offset_1_get_none_to_none() {
        let wire = &[0, 1];
        let read_wire = ReadWire { wire, offset: 1 };

        let expected_wire= &[0, 1];
        let expected_read_wire = ReadWire { wire: expected_wire, offset: 1 };

        let actual_read_wire_result = read_wire.section_from_current(None, None);
        assert!(actual_read_wire_result.is_ok());
        let actual_read_wire = actual_read_wire_result.unwrap();
        assert_eq!(expected_read_wire, actual_read_wire);

        // Verify original wire's state.
        assert_eq!(&wire[1..], read_wire.current());
        assert_eq!(1, read_wire.current_len());
        assert_eq!(1, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_three_bytes_with_offset_1_get_none_to_none() {
        let wire = &[0, 1, 2];
        let read_wire = ReadWire { wire, offset: 1 };

        let expected_wire= &[0, 1, 2];
        let expected_read_wire = ReadWire { wire: expected_wire, offset: 1 };

        let actual_read_wire_result = read_wire.section_from_current(None, None);
        assert!(actual_read_wire_result.is_ok());
        let actual_read_wire = actual_read_wire_result.unwrap();
        assert_eq!(expected_read_wire, actual_read_wire);

        // Verify original wire's state.
        assert_eq!(&wire[1..], read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(1, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_three_bytes_with_offset_1_get_start_to_none() {
        let wire = &[0, 1, 2];
        let read_wire = ReadWire { wire, offset: 1 };

        let expected_wire= &[1, 2];
        let expected_read_wire = ReadWire { wire: expected_wire, offset: 0 };

        let actual_read_wire_result = read_wire.section_from_current(Some(0), None);
        assert!(actual_read_wire_result.is_ok());
        let actual_read_wire = actual_read_wire_result.unwrap();
        assert_eq!(expected_read_wire, actual_read_wire);

        // Verify original wire's state.
        assert_eq!(&wire[1..], read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(1, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_three_bytes_with_offset_1_get_middle_to_none() {
        let wire = &[0, 1, 2];
        let read_wire = ReadWire { wire, offset: 1 };

        let expected_wire= &[2];
        let expected_read_wire = ReadWire { wire: expected_wire, offset: 0 };

        let actual_read_wire_result = read_wire.section_from_current(Some(1), None);
        assert!(actual_read_wire_result.is_ok());
        let actual_read_wire = actual_read_wire_result.unwrap();
        assert_eq!(expected_read_wire, actual_read_wire);

        // Verify original wire's state.
        assert_eq!(&wire[1..], read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(1, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_three_bytes_with_offset_1_get_end_to_none() {
        let wire = &[0, 1, 2];
        let read_wire = ReadWire { wire, offset: 1 };

        let expected_wire= &[];
        let expected_read_wire = ReadWire { wire: expected_wire, offset: 0 };

        let actual_read_wire_result = read_wire.section_from_current(Some(2), None);
        assert!(actual_read_wire_result.is_ok());
        let actual_read_wire = actual_read_wire_result.unwrap();
        assert_eq!(expected_read_wire, actual_read_wire);

        // Verify original wire's state.
        assert_eq!(&wire[1..], read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(1, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_three_bytes_with_offset_1_get_past_end_to_none() {
        let wire = &[0, 1, 2];
        let read_wire = ReadWire { wire, offset: 1 };

        let actual_read_wire_result = read_wire.section_from_current(Some(3), None);
        assert!(actual_read_wire_result.is_err());

        // Verify original wire's state.
        assert_eq!(&wire[1..], read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(1, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_three_bytes_with_offset_1_get_none_to_start() {
        let wire = &[0, 1, 2];
        let read_wire = ReadWire { wire, offset: 1 };

        let expected_wire= &[0];
        let expected_read_wire = ReadWire { wire: expected_wire, offset: 1 };

        let actual_read_wire_result = read_wire.section_from_current(None, Some(0));
        assert!(actual_read_wire_result.is_ok());
        let actual_read_wire = actual_read_wire_result.unwrap();
        assert_eq!(expected_read_wire, actual_read_wire);

        // Verify original wire's state.
        assert_eq!(&wire[1..], read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(1, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_three_bytes_with_offset_1_get_none_to_middle() {
        let wire = &[0, 1, 2];
        let read_wire = ReadWire { wire, offset: 1 };

        let expected_wire= &[0, 1];
        let expected_read_wire = ReadWire { wire: expected_wire, offset: 1 };

        let actual_read_wire_result = read_wire.section_from_current(None, Some(1));
        assert!(actual_read_wire_result.is_ok());
        let actual_read_wire = actual_read_wire_result.unwrap();
        assert_eq!(expected_read_wire, actual_read_wire);

        // Verify original wire's state.
        assert_eq!(&wire[1..], read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(1, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_three_bytes_with_offset_1_get_none_to_end() {
        let wire = &[0, 1, 2];
        let read_wire = ReadWire { wire, offset: 1 };

        let expected_wire= &[0, 1, 2];
        let expected_read_wire = ReadWire { wire: expected_wire, offset: 1 };

        let actual_read_wire_result = read_wire.section_from_current(None, Some(2));
        assert!(actual_read_wire_result.is_ok());
        let actual_read_wire = actual_read_wire_result.unwrap();
        assert_eq!(expected_read_wire, actual_read_wire);

        // Verify original wire's state.
        assert_eq!(&wire[1..], read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(1, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_three_bytes_with_offset_1_get_none_to_past_end() {
        let wire = &[0, 1, 2];
        let read_wire = ReadWire { wire, offset: 1 };

        let actual_read_wire_result = read_wire.section_from_current(None, Some(3));
        assert!(actual_read_wire_result.is_err());

        // Verify original wire's state.
        assert_eq!(&wire[1..], read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(1, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_three_bytes_with_offset_1_get_start_to_start() {
        let wire = &[0, 1, 2];
        let read_wire = ReadWire { wire, offset: 1 };

        let expected_wire= &[];
        let expected_read_wire = ReadWire { wire: expected_wire, offset: 0 };

        let actual_read_wire_result = read_wire.section_from_current(Some(0), Some(0));
        assert!(actual_read_wire_result.is_ok());
        let actual_read_wire = actual_read_wire_result.unwrap();
        assert_eq!(expected_read_wire, actual_read_wire);

        // Verify original wire's state.
        assert_eq!(&wire[1..], read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(1, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_three_bytes_with_offset_1_get_start_to_middle() {
        let wire = &[0, 1, 2];
        let read_wire = ReadWire { wire, offset: 1 };

        let expected_wire= &[1];
        let expected_read_wire = ReadWire { wire: expected_wire, offset: 0 };

        let actual_read_wire_result = read_wire.section_from_current(Some(0), Some(1));
        assert!(actual_read_wire_result.is_ok());
        let actual_read_wire = actual_read_wire_result.unwrap();
        assert_eq!(expected_read_wire, actual_read_wire);

        // Verify original wire's state.
        assert_eq!(&wire[1..], read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(1, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_three_bytes_with_offset_1_get_start_to_end() {
        let wire = &[0, 1, 2];
        let read_wire = ReadWire { wire, offset: 1 };

        let expected_wire= &[1, 2];
        let expected_read_wire = ReadWire { wire: expected_wire, offset: 0 };

        let actual_read_wire_result = read_wire.section_from_current(Some(0), Some(2));
        assert!(actual_read_wire_result.is_ok());
        let actual_read_wire = actual_read_wire_result.unwrap();
        assert_eq!(expected_read_wire, actual_read_wire);

        // Verify original wire's state.
        assert_eq!(&wire[1..], read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(1, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_three_bytes_with_offset_1_get_start_to_past_end() {
        let wire = &[0, 1, 2];
        let read_wire = ReadWire { wire, offset: 1 };

        let actual_read_wire_result = read_wire.section_from_current(Some(0), Some(3));
        assert!(actual_read_wire_result.is_err());

        // Verify original wire's state.
        assert_eq!(&wire[1..], read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(1, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_three_bytes_with_offset_1_get_middle_to_start() {
        let wire = &[0, 1, 2];
        let read_wire = ReadWire { wire, offset: 1 };

        let actual_read_wire_result = read_wire.section_from_current(Some(1), Some(0));
        assert!(actual_read_wire_result.is_err());
        // Verify original wire's state.
        assert_eq!(&wire[1..], read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(1, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_three_bytes_with_offset_1_get_middle_to_middle() {
        let wire = &[0, 1, 2];
        let read_wire = ReadWire { wire, offset: 1 };

        let expected_wire= &[];
        let expected_read_wire = ReadWire { wire: expected_wire, offset: 0 };

        let actual_read_wire_result = read_wire.section_from_current(Some(1), Some(1));
        assert!(actual_read_wire_result.is_ok());
        let actual_read_wire = actual_read_wire_result.unwrap();
        assert_eq!(expected_read_wire, actual_read_wire);

        // Verify original wire's state.
        assert_eq!(&wire[1..], read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(1, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_three_bytes_with_offset_1_get_middle_to_end() {
        let wire = &[0, 1, 2];
        let read_wire = ReadWire { wire, offset: 1 };

        let expected_wire= &[2];
        let expected_read_wire = ReadWire { wire: expected_wire, offset: 0 };

        let actual_read_wire_result = read_wire.section_from_current(Some(1), Some(2));
        assert!(actual_read_wire_result.is_ok());
        let actual_read_wire = actual_read_wire_result.unwrap();
        assert_eq!(expected_read_wire, actual_read_wire);

        // Verify original wire's state.
        assert_eq!(&wire[1..], read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(1, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_three_bytes_with_offset_1_get_middle_to_past_end() {
        let wire = &[0, 1, 2];
        let read_wire = ReadWire { wire, offset: 1 };

        let actual_read_wire_result = read_wire.section_from_current(Some(1), Some(3));
        assert!(actual_read_wire_result.is_err());

        // Verify original wire's state.
        assert_eq!(&wire[1..], read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(1, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_three_bytes_with_offset_1_get_end_to_start() {
        let wire = &[0, 1, 2];
        let read_wire = ReadWire { wire, offset: 1 };

        let actual_read_wire_result = read_wire.section_from_current(Some(1), Some(0));
        assert!(actual_read_wire_result.is_err());

        // Verify original wire's state.
        assert_eq!(&wire[1..], read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(1, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_three_bytes_with_offset_1_get_end_to_middle() {
        let wire = &[0, 1, 2];
        let read_wire = ReadWire { wire, offset: 1 };

        let actual_read_wire_result = read_wire.section_from_current(Some(2), Some(1));
        assert!(actual_read_wire_result.is_err());

        // Verify original wire's state.
        assert_eq!(&wire[1..], read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(1, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_three_bytes_with_offset_1_get_end_to_end() {
        let wire = &[0, 1, 2];
        let read_wire = ReadWire { wire, offset: 1 };

        let expected_wire= &[];
        let expected_read_wire = ReadWire { wire: expected_wire, offset: 0 };

        let actual_read_wire_result = read_wire.section_from_current(Some(2), Some(2));
        assert!(actual_read_wire_result.is_ok());
        let actual_read_wire = actual_read_wire_result.unwrap();
        assert_eq!(expected_read_wire, actual_read_wire);

        // Verify original wire's state.
        assert_eq!(&wire[1..], read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(1, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }

    #[test]
    fn test_three_bytes_with_offset_1_get_end_to_past_end() {
        let wire = &[0, 1, 2];
        let read_wire = ReadWire { wire, offset: 1 };

        let actual_read_wire_result = read_wire.section_from_current(Some(2), Some(3));
        assert!(actual_read_wire_result.is_err());

        // Verify original wire's state.
        assert_eq!(&wire[1..], read_wire.current());
        assert_eq!(2, read_wire.current_len());
        assert_eq!(1, read_wire.current_offset());
        assert!(!read_wire.is_end_reached());
    }
}

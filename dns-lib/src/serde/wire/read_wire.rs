use std::{
    error::Error,
    fmt::Display,
    ops::{Bound, RangeBounds},
};

use crate::{
    resource_record::rtype::RType,
    types::{
        ascii::AsciiError, base16::Base16Error, base32::Base32Error, base64::Base64Error,
        c_domain_name::CDomainNameError, domain_name::DomainNameError,
        extended_base32::ExtendedBase32Error,
    },
};

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum ReadWireError {
    FormatError(String),
    OverflowError(String),
    OutOfBoundsError(String),
    UnsupportedRType(RType),
    UnexpectedRType { expected: RType, actual: RType },
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
            Self::OutOfBoundsError(error) => write!(f, "Read Wire Out Of Bounds Error: {error}"),
            Self::UnsupportedRType(rtype) => {
                write!(f, "Resource Record Type {rtype} is not supported")
            }
            Self::UnexpectedRType { expected, actual } => write!(
                f,
                "Expected Resource Record Type {expected} but found {actual}"
            ),
            Self::ValueError(error) => write!(f, "Read Wire Value Error: {error}"),
            Self::VersionError(error) => write!(f, "Read Wire Version Error: {error}"),

            Self::CDomainNameError(error) => {
                write!(f, "Read Wire Compressible Domain Name Error: {error}")
            }
            Self::DomainNameError(error) => {
                write!(f, "Read Wire Incompressible Domain Name Error: {error}")
            }
            Self::AsciiError(error) => write!(f, "Read Wire Ascii Error: {error}"),

            Self::Base16Error(error) => write!(f, "Read Wire Base16 Error: {error}"),
            Self::Base32Error(error) => write!(f, "Read Wire Bsse32 Error: {error}"),
            Self::ExtendedBase32Error(error) => {
                write!(f, "Read Wire Extended Base32 Error: {error}")
            }
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
    pub fn current(&self) -> &'a [u8] {
        &self.wire[self.offset..]
    }

    #[inline]
    pub fn current_len(&self) -> usize {
        self.current().len()
    }

    #[inline]
    pub fn current_offset(&self) -> usize {
        self.offset
    }

    #[inline]
    pub fn is_end_reached(&self) -> bool {
        self.offset >= self.wire.len()
    }

    #[inline]
    pub fn wire(&self) -> &'a [u8] {
        self.wire
    }

    #[inline]
    pub fn wire_len(&self) -> usize {
        self.wire.len()
    }

    #[inline]
    pub fn with_offset_or_err(
        &self,
        offset: usize,
        visibility: WireVisibility,
        err_msg: impl FnOnce() -> String,
    ) -> Result<Self, ReadWireError> {
        if self.wire_len() >= offset {
            match visibility {
                WireVisibility::Entire => Ok(Self {
                    wire: self.wire(),
                    offset,
                }),
                WireVisibility::Current => Ok(Self {
                    wire: self.current(),
                    offset,
                }),
            }
        } else {
            Err(ReadWireError::OverflowError(err_msg()))
        }
    }

    #[inline]
    pub fn with_offset(
        &self,
        offset: usize,
        visibility: WireVisibility,
    ) -> Result<Self, ReadWireError> {
        self.with_offset_or_err(offset, visibility, || {
            format!("setting offset to {offset} would read past the end of the wire")
        })
    }

    #[inline]
    pub fn set_offset_or_err(
        &mut self,
        offset: usize,
        err_msg: impl FnOnce() -> String,
    ) -> Result<(), ReadWireError> {
        if self.wire_len() >= offset {
            self.offset = offset;
            Ok(())
        } else {
            Err(ReadWireError::OverflowError(err_msg()))
        }
    }

    #[inline]
    pub fn set_offset(&mut self, offset: usize) -> Result<(), ReadWireError> {
        self.set_offset_or_err(offset, || {
            format!("setting offset to {offset} would read past the end of the wire")
        })
    }

    #[inline]
    pub fn shift_or_err(
        &mut self,
        shift: usize,
        err_msg: impl FnOnce() -> String,
    ) -> Result<(), ReadWireError> {
        if self.current_len() >= shift {
            self.offset += shift;
            Ok(())
        } else {
            Err(ReadWireError::OverflowError(err_msg()))
        }
    }

    #[inline]
    pub fn shift(&mut self, shift: usize) -> Result<(), ReadWireError> {
        self.shift_or_err(shift, || {
            format!("shifting {shift} bytes would have gone past the end of the wire")
        })
    }

    #[inline]
    pub fn get_or_err(
        &self,
        count: usize,
        err_msg: impl FnOnce() -> String,
    ) -> Result<&'a [u8], ReadWireError> {
        if self.current_len() >= count {
            Ok(&self.wire[self.offset..(self.offset + count)])
        } else {
            Err(ReadWireError::OverflowError(err_msg()))
        }
    }

    #[inline]
    pub fn get(&self, count: usize) -> Result<&'a [u8], ReadWireError> {
        self.get_or_err(count, || {
            format!("getting {count} bytes would have read past the end of the wire")
        })
    }

    #[inline]
    pub fn get_as_read_wire_or_err(
        &self,
        count: usize,
        err_msg: impl FnOnce() -> String,
    ) -> Result<Self, ReadWireError> {
        Ok(Self {
            wire: self.get_or_err(count, err_msg)?,
            offset: 0,
        })
    }

    #[inline]
    pub fn get_as_read_wire(&self, count: usize) -> Result<Self, ReadWireError> {
        Ok(Self {
            wire: self.get(count)?,
            offset: 0,
        })
    }

    #[inline]
    pub fn take_or_err(
        &mut self,
        count: usize,
        err_msg: impl FnOnce() -> String,
    ) -> Result<&'a [u8], ReadWireError> {
        if self.current_len() >= count {
            let offset = self.offset;
            self.offset += count;
            Ok(&self.wire[offset..(offset + count)])
        } else {
            Err(ReadWireError::OverflowError(err_msg()))
        }
    }

    #[inline]
    pub fn take(&mut self, count: usize) -> Result<&'a [u8], ReadWireError> {
        self.take_or_err(count, || {
            format!("taking {count} bytes would have read past the end of the wire")
        })
    }

    #[inline]
    pub fn take_all(&mut self) -> &'a [u8] {
        let offset = self.offset;
        self.offset = self.wire_len();
        &self.wire[offset..]
    }

    #[inline]
    pub fn take_as_read_wire_or_err(
        &mut self,
        count: usize,
        err_msg: impl FnOnce() -> String,
    ) -> Result<Self, ReadWireError> {
        Ok(Self {
            wire: self.take_or_err(count, err_msg)?,
            offset: 0,
        })
    }

    #[inline]
    pub fn take_as_read_wire(&mut self, count: usize) -> Result<Self, ReadWireError> {
        Ok(Self {
            wire: self.take(count)?,
            offset: 0,
        })
    }

    #[inline]
    pub fn take_all_as_read_wire(&mut self) -> Self {
        Self {
            wire: self.take_all(),
            offset: 0,
        }
    }

    #[inline]
    pub fn get_byte_or_err(&self, err_msg: impl FnOnce() -> String) -> Result<u8, ReadWireError> {
        if self.current_len() >= 1 {
            Ok(self.wire[self.offset])
        } else {
            Err(ReadWireError::OverflowError(err_msg()))
        }
    }

    #[inline]
    pub fn get_byte(&self) -> Result<u8, ReadWireError> {
        self.get_byte_or_err(|| {
            "getting a byte would have read past the end of the wire".to_string()
        })
    }

    #[inline]
    pub fn take_byte_or_err(
        &mut self,
        err_msg: impl FnOnce() -> String,
    ) -> Result<u8, ReadWireError> {
        if self.current_len() >= 1 {
            let offset = self.offset;
            self.offset += 1;
            Ok(self.wire[offset])
        } else {
            Err(ReadWireError::OverflowError(err_msg()))
        }
    }

    #[inline]
    pub fn take_byte(&mut self) -> Result<u8, ReadWireError> {
        self.take_byte_or_err(|| {
            "getting a byte would have read past the end of the wire".to_string()
        })
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
    pub fn slice_from_current(
        &self,
        range: impl RangeBounds<usize>,
        visibility: SliceWireVisibility,
    ) -> Result<Self, ReadWireError> {
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
            panic!(
                "range start must not be greater than end: {:?} <= {:?}",
                start, end
            );
        }
        if current_len < end {
            panic!("range end out of bounds: {:?} <= {:?}", end, current_len);
        }

        match visibility {
            SliceWireVisibility::Entire => Ok(Self {
                wire: &self.wire[..(self.offset + end)],
                offset: self.offset + start,
            }),
            SliceWireVisibility::Current => Ok(Self {
                wire: &self.wire[self.offset..(self.offset + end)],
                offset: start,
            }),
            SliceWireVisibility::Slice => Ok(Self {
                wire: &self.wire[(self.offset + start)..(self.offset + end)],
                offset: 0,
            }),
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

use std::{ops::{Bound, RangeBounds}, rc::Rc};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum InternalBytes {
    Empty,
    Vec(Rc<Vec<u8>>),
    Front { data: Rc<Vec<u8>>, end: usize },
    Tail { data: Rc<Vec<u8>>, start: usize },
    Slice { data: Rc<Vec<u8>>, start: usize, end: usize },
}

impl InternalBytes {
    /// Returns the number of bytes contained in `self`.
    #[inline]
    fn len(&self) -> usize {
        match &self {
            Self::Empty => 0,
            Self::Vec(bytes) => bytes.len(),
            Self::Front { data: _, end } => *end,
            Self::Tail { data, start } => data.len() - start,
            Self::Slice { data: _, start, end } => end - start,
        }
    }

    /// Splits the bytes at the index, returning the two halves. The first `Bytes` will contain
    /// elements from `[0, at)`. The first second `Bytes` will contain elements from `[0, at)`.
    /// 
    /// # Panics
    ///
    /// Panics if `at > len`.
    #[inline]
    fn split_at(&self, at: usize) -> (Self, Self) {
        match self.split_at_checked(at) {
            Some(pair) => pair,
            None => panic!("at > len"),
        }
    }

    /// Splits the bytes at the index, returning the two halves. The first `Bytes` will contain
    /// elements from `[0, at)`. The first second `Bytes` will contain elements from `[0, at)`.
    /// Returns `None` if `at > len`.
    #[inline]
    fn split_at_checked(&self, at: usize) -> Option<(Self, Self)> {
        let len = self.len();
        match (at, &self) {
            (0,   Self::Empty) => Some((Self::Empty, Self::Empty)),
            (1.., Self::Empty) => None,

            (0,   Self::Vec(bytes)) => Some((Self::Empty, Self::Vec(bytes.clone()))),
            (1.., Self::Vec(bytes)) => {
                if at == len {
                    Some((self.clone(), Self::Empty))
                } else if at < len {
                    Some((
                        Self::Front { data: bytes.clone(), end: at },
                        Self::Tail { data: bytes.clone(), start: at },
                    ))
                } else {
                    None
                }
            },

            (0,   Self::Front { data, end }) => Some((Self::Empty, Self::Front { data: data.clone(), end: *end })),
            (1.., Self::Front { data, end }) => {
                if at == len {
                    Some((Self::Front { data: data.clone(), end: *end }, Self::Empty))
                } else if at < len {
                    Some((
                        Self::Front { data: data.clone(), end: at },
                        Self::Slice { data: data.clone(), start: at, end: *end },
                    ))
                } else {
                    None
                }
            },

            (0,   Self::Tail { data, start }) => Some((Self::Empty, Self::Tail { data: data.clone(), start: *start })),
            (1.., Self::Tail { data, start }) => {
                if at == len {
                    Some((Self::Tail { data: data.clone(), start: *start }, Self::Empty))
                } else if at < len {
                    Some((
                        Self::Slice { data: data.clone(), start: *start, end: at },
                        Self::Tail { data: data.clone(), start: at },
                    ))
                } else {
                    None
                }
            },

            (0,   Self::Slice { data, start, end }) => Some((Self::Empty, Self::Slice { data: data.clone(), start: *start, end: *end })),
            (1.., Self::Slice { data, start, end }) => {
                if at == len {
                    Some((Self::Slice { data: data.clone(), start: *start, end: *end }, Self::Empty))
                } else if at < len {
                    let mid = start + at;
                    Some((
                        Self::Slice { data: data.clone(), start: *start, end: mid },
                        Self::Slice { data: data.clone(), start: mid, end: *end },
                    ))
                } else {
                    None
                }
            },
        }
    }

    /// Gets the byte at the index 'at' or `None` if the index is out of bounds.
    #[inline]
    fn get(&self, at: usize) -> Option<u8> {
        self.as_slice().get(at).copied()
    }

    /// Gets a slice of `self` for the given range.
    #[inline]
    fn slice(&self, range: impl RangeBounds<usize>) -> Self {
        let len = self.len();
        let start = match range.start_bound() {
            Bound::Included(&start) => start,
            Bound::Excluded(&start) => start.checked_add(1).expect("out of range"),
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(&end) => end.checked_add(1).expect("out of range"),
            Bound::Excluded(&end) => end,
            Bound::Unbounded => self.len(),
        };
        if end < start {
            panic!("range start must not be greater than end: {:?} <= {:?}", start, end);
        }
        if len < end {
            panic!("range end out of bounds: {:?} <= {:?}", end, len);
        }

        // If the length remains unchanged, then the bytes being represented by the slice are the
        // same as `self`.
        if len == (end - start) {
            return self.clone();
        }

        match &self {
            Self::Empty => Self::Empty,
            Self::Vec(bytes) => {
                if start == 0 {
                    Self::Front { data: bytes.clone(), end }
                } else if end == len {
                    Self::Tail { data: bytes.clone(), start: start }
                } else {
                    Self::Slice { data: bytes.clone(), start, end }
                }
            },
            Self::Front { data, end: _} => {
                if start == 0 {
                    Self::Front { data: data.clone(), end }
                } else {
                    Self::Slice { data: data.clone(), start, end }
                }
            },
            Self::Tail { data, start: tail_start } => {
                if end == len {
                    Self::Tail { data: data.clone(), start: tail_start + start }
                } else {
                    Self::Slice { data: data.clone(), start: tail_start + start, end: tail_start + end }
                }
            },
            Self::Slice { data, start: vstart, end: _ } => Self::Slice { data: data.clone(), start: vstart + start, end: start + end },
        }
    }

    /// Gets a slice of the section of the underlying data represented by `self`.
    #[inline]
    fn as_slice(&self) -> &[u8] {
        match &self {
            Self::Empty => &[],
            Self::Vec(bytes) => &bytes,
            Self::Front { data, end } => &data[..*end],
            Self::Tail { data, start } => &data[*start..],
            Self::Slice { data, start, end } => &data[*start..*end],
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Bytes {
    bytes: InternalBytes
}

impl Bytes {
    /// Returns the number of bytes contained in `self`.
    #[inline]
    pub fn len(&self) -> usize { self.bytes.len() }

    /// Returns `true` if `self` has a length of 0. Returns 'false' otherwise.
    #[inline]
    pub fn is_empty(&self) -> bool { self.len() == 0 }

    /// Splits the bytes at the index, returning the two halves. The first `Bytes` will contain
    /// elements from `[0, at)`. The first second `Bytes` will contain elements from `[0, at)`.
    /// 
    /// # Panics
    ///
    /// Panics if `at > len`.
    #[inline]
    pub fn split_at(&self, at: usize) -> (Self, Self) {
        let (left, right) = self.bytes.split_at(at);
        (Self { bytes: left }, Self { bytes: right })
    }

    /// Splits the bytes at the index, returning the two halves. The first `Bytes` will contain
    /// elements from `[0, at)`. The first second `Bytes` will contain elements from `[0, at)`.
    /// Returns `None` if `at > len`.
    #[inline]
    pub fn split_at_checked(&self, at: usize) -> Option<(Self, Self)> {
        let (left, right) = self.bytes.split_at_checked(at)?;
        Some((Self { bytes: left }, Self { bytes: right }))
    }

    /// Splits the bytes at the index. 'self' will contain elements from `[0, at)`. The returned
    /// `Bytes` will contain elements from '[at, len)'.
    /// 
    /// # Panics
    ///
    /// Panics if `at > len`.
    #[inline]
    pub fn split_off(&mut self, at: usize) -> Self {
        let (left, right) = self.bytes.split_at(at);
        self.bytes = left;
        Self { bytes: right }
    }

    /// Splits the bytes at the index. 'self' will contain elements from `[at, len)`. The returned
    /// `Bytes` will contain elements from '[0, at)'.
    /// 
    /// # Panics
    ///
    /// Panics if `at > len`.
    #[inline]
    pub fn split_to(&mut self, at: usize) -> Self {
        let (left, right) = self.bytes.split_at(at);
        self.bytes = right;
        Self { bytes: left }
    }

    /// Gets the byte at the index 'at' or `None` if the index is out of bounds.
    #[inline]
    pub fn get(&self, at: usize) -> Option<u8> { self.bytes.get(at) }

    /// Gets the byte at the index 'at' or `None`.
    /// 
    /// # Panics
    ///
    /// Panics if `at > len`.
    #[inline]
    pub fn get_unchecked(&self, at: usize) -> u8 {
        match self.get(at) {
            Some(byte) => byte,
            None => panic!("at > len"),
        }
    }

    /// Gets the first byte or `None` if `self` is empty (`self.is_empty()`).
    #[inline]
    pub fn first(&self) -> Option<u8> { self.get(0) }

    /// Gets the first byte.
    /// 
    /// # Panics
    ///
    /// Panics if `self.is_empty()`.
    #[inline]
    pub fn first_unchecked(&self) -> u8 { self.get_unchecked(0) }

    /// Gets the last byte or `None` if `self` is empty (`self.is_empty()`).
    #[inline]
    pub fn last(&self) -> Option<u8> { self.get(self.len() - 1) }

    /// Gets the last byte.
    /// 
    /// # Panics
    ///
    /// Panics if `self.is_empty()`.
    #[inline]
    pub fn last_unchecked(&self) -> u8 { self.get_unchecked(self.len() - 1) }

    /// Gets a slice of `self` for the given range.
    #[inline]
    pub fn slice(&self, range: impl RangeBounds<usize>) -> Self { Self { bytes: self.bytes.slice(range) } }

    /// Returns `self` as a slice of type `&[u8]`.
    #[inline]
    pub fn as_slice(&self) -> &[u8] { self.bytes.as_slice() }
}

impl From<Rc<Vec<u8>>> for Bytes {
    #[inline]
    fn from(value: Rc<Vec<u8>>) -> Self {
        if value.len() == 0 {
            Self { bytes: InternalBytes::Empty }
        } else {
            Self { bytes: InternalBytes::Vec(value) }
        }
    }
}

impl From<&Rc<Vec<u8>>> for Bytes {
    #[inline]
    fn from(value: &Rc<Vec<u8>>) -> Self {
        if value.len() == 0 {
            Self { bytes: InternalBytes::Empty }
        } else {
            Self { bytes: InternalBytes::Vec(value.clone()) }
        }
    }
}

impl From<Vec<u8>> for Bytes {
    #[inline]
    fn from(value: Vec<u8>) -> Self {
        if value.len() == 0 {
            Self { bytes: InternalBytes::Empty }
        } else {
            Self { bytes: InternalBytes::Vec(Rc::new(value)) }
        }
    }
}

impl From<&Vec<u8>> for Bytes {
    #[inline]
    fn from(value: &Vec<u8>) -> Self {
        if value.len() == 0 {
            Self { bytes: InternalBytes::Empty }
        } else {
            Self { bytes: InternalBytes::Vec(Rc::new(value.clone())) }
        }
    }
}

impl From<Vec<&u8>> for Bytes {
    #[inline]
    fn from(value: Vec<&u8>) -> Self {
        if value.len() == 0 {
            Self { bytes: InternalBytes::Empty }
        } else {
            Self { bytes: InternalBytes::Vec(Rc::new(value.iter().map(|byte| **byte).collect())) }
        }
    }
}

impl From<&Vec<&u8>> for Bytes {
    #[inline]
    fn from(value: &Vec<&u8>) -> Self {
        if value.len() == 0 {
            Self { bytes: InternalBytes::Empty }
        } else {
            Self { bytes: InternalBytes::Vec(Rc::new(value.iter().map(|byte| **byte).collect())) }
        }
    }
}

impl From<&[u8]> for Bytes {
    #[inline]
    fn from(value: &[u8]) -> Self {
        if value.len() == 0 {
            Self { bytes: InternalBytes::Empty }
        } else {
            Self { bytes: InternalBytes::Vec(Rc::new(value.to_vec())) }
        }
    }
}

impl From<&[&u8]> for Bytes {
    #[inline]
    fn from(value: &[&u8]) -> Self {
        if value.len() == 0 {
            Self { bytes: InternalBytes::Empty }
        } else {
            Self { bytes: InternalBytes::Vec(Rc::new(value.iter().map(|byte| **byte).collect())) }
        }
    }
}

impl<const N: usize> From<[u8; N]> for Bytes {
    #[inline]
    fn from(value: [u8; N]) -> Self {
        if value.len() == 0 {
            Self { bytes: InternalBytes::Empty }
        } else {
            Self { bytes: InternalBytes::Vec(Rc::new(value.to_vec())) }
        }
    }
}

impl<const N: usize> From<[&u8; N]> for Bytes {
    #[inline]
    fn from(value: [&u8; N]) -> Self {
        if value.len() == 0 {
            Self { bytes: InternalBytes::Empty }
        } else {
            Self { bytes: InternalBytes::Vec(Rc::new(value.iter().map(|byte| **byte).collect())) }
        }
    }
}

impl FromIterator<u8> for Bytes {
    fn from_iter<T: IntoIterator<Item = u8>>(iter: T) -> Self {
        Self::from(iter.into_iter().collect::<Vec<u8>>())
    }
}

impl<'a> FromIterator<&'a u8> for Bytes {
    fn from_iter<T: IntoIterator<Item = &'a u8>>(iter: T) -> Self {
        Self::from(iter.into_iter().map(|x| *x).collect::<Vec<u8>>())
    }
}

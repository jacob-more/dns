use std::{
    ops::{Bound, RangeBounds},
    rc::Rc,
};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum InternalBytes {
    Empty,
    Vec(Rc<Vec<u8>>),
    Front {
        data: Rc<Vec<u8>>,
        end: usize,
    },
    Tail {
        data: Rc<Vec<u8>>,
        start: usize,
    },
    Slice {
        data: Rc<Vec<u8>>,
        start: usize,
        end: usize,
    },
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
            Self::Slice {
                data: _,
                start,
                end,
            } => end - start,
        }
    }

    /// Splits the bytes at the index, returning the two halves. The first `Bytes` will contain
    /// elements from `[0, at)`. The second `Bytes` will contain elements from `[0, at)`.
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
    /// elements from `[0, at)`. The second `Bytes` will contain elements from `[0, at)`.
    /// Returns `None` if `at > len`.
    #[inline]
    fn split_at_checked(&self, at: usize) -> Option<(Self, Self)> {
        let len = self.len();
        match (at, &self) {
            (0, Self::Empty) => Some((Self::Empty, Self::Empty)),
            (1.., Self::Empty) => None,

            (0, Self::Vec(bytes)) => Some((Self::Empty, Self::Vec(bytes.clone()))),
            (1.., Self::Vec(bytes)) => {
                if at == len {
                    Some((self.clone(), Self::Empty))
                } else if at < len {
                    Some((
                        Self::Front {
                            data: bytes.clone(),
                            end: at,
                        },
                        Self::Tail {
                            data: bytes.clone(),
                            start: at,
                        },
                    ))
                } else {
                    None
                }
            }

            (0, Self::Front { data, end }) => Some((
                Self::Empty,
                Self::Front {
                    data: data.clone(),
                    end: *end,
                },
            )),
            (1.., Self::Front { data, end }) => {
                if at == len {
                    Some((
                        Self::Front {
                            data: data.clone(),
                            end: *end,
                        },
                        Self::Empty,
                    ))
                } else if at < len {
                    Some((
                        Self::Front {
                            data: data.clone(),
                            end: at,
                        },
                        Self::Slice {
                            data: data.clone(),
                            start: at,
                            end: *end,
                        },
                    ))
                } else {
                    None
                }
            }

            (0, Self::Tail { data, start }) => Some((
                Self::Empty,
                Self::Tail {
                    data: data.clone(),
                    start: *start,
                },
            )),
            (1.., Self::Tail { data, start }) => {
                if at == len {
                    Some((
                        Self::Tail {
                            data: data.clone(),
                            start: *start,
                        },
                        Self::Empty,
                    ))
                } else if at < len {
                    Some((
                        Self::Slice {
                            data: data.clone(),
                            start: *start,
                            end: start + at,
                        },
                        Self::Tail {
                            data: data.clone(),
                            start: start + at,
                        },
                    ))
                } else {
                    None
                }
            }

            (0, Self::Slice { data, start, end }) => Some((
                Self::Empty,
                Self::Slice {
                    data: data.clone(),
                    start: *start,
                    end: *end,
                },
            )),
            (1.., Self::Slice { data, start, end }) => {
                if at == len {
                    Some((
                        Self::Slice {
                            data: data.clone(),
                            start: *start,
                            end: *end,
                        },
                        Self::Empty,
                    ))
                } else if at < len {
                    let mid = start + at;
                    Some((
                        Self::Slice {
                            data: data.clone(),
                            start: *start,
                            end: mid,
                        },
                        Self::Slice {
                            data: data.clone(),
                            start: mid,
                            end: *end,
                        },
                    ))
                } else {
                    None
                }
            }
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
            panic!(
                "range start must not be greater than end: {:?} <= {:?}",
                start, end
            );
        }
        if len < end {
            panic!("range end out of bounds: {:?} <= {:?}", end, len);
        }

        // If the length remains unchanged, then the bytes being represented by the slice are the
        // same as `self`.
        if len == (end - start) {
            return self.clone();
        } else if 0 == (end - start) {
            return Self::Empty;
        }

        match &self {
            Self::Empty => Self::Empty,
            Self::Vec(bytes) => {
                if start == 0 {
                    Self::Front {
                        data: bytes.clone(),
                        end,
                    }
                } else if end == len {
                    Self::Tail {
                        data: bytes.clone(),
                        start,
                    }
                } else {
                    Self::Slice {
                        data: bytes.clone(),
                        start,
                        end,
                    }
                }
            }
            Self::Front { data, end: _ } => {
                if start == 0 {
                    Self::Front {
                        data: data.clone(),
                        end,
                    }
                } else {
                    Self::Slice {
                        data: data.clone(),
                        start,
                        end,
                    }
                }
            }
            Self::Tail {
                data,
                start: tail_start,
            } => {
                if end == len {
                    Self::Tail {
                        data: data.clone(),
                        start: tail_start + start,
                    }
                } else {
                    Self::Slice {
                        data: data.clone(),
                        start: tail_start + start,
                        end: tail_start + end,
                    }
                }
            }
            Self::Slice {
                data,
                start: vstart,
                end: _,
            } => Self::Slice {
                data: data.clone(),
                start: vstart + start,
                end: vstart + end,
            },
        }
    }

    /// Gets a slice of the section of the underlying data represented by `self`.
    #[inline]
    fn as_slice(&self) -> &[u8] {
        match &self {
            Self::Empty => &[],
            Self::Vec(bytes) => bytes,
            Self::Front { data, end } => &data[..*end],
            Self::Tail { data, start } => &data[*start..],
            Self::Slice { data, start, end } => &data[*start..*end],
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Bytes {
    bytes: InternalBytes,
}

impl Bytes {
    /// Returns the number of bytes contained in `self`.
    #[inline]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Returns `true` if `self` has a length of 0. Returns 'false' otherwise.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Splits the bytes at the index, returning the two halves. The first `Bytes` will contain
    /// elements from `[0, at)`. The second `Bytes` will contain elements from `[0, at)`.
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
    /// elements from `[0, at)`. The second `Bytes` will contain elements from `[0, at)`.
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
    pub fn get(&self, at: usize) -> Option<u8> {
        self.bytes.get(at)
    }

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
    pub fn first(&self) -> Option<u8> {
        self.get(0)
    }

    /// Gets the first byte.
    ///
    /// # Panics
    ///
    /// Panics if `self.is_empty()`.
    #[inline]
    pub fn first_unchecked(&self) -> u8 {
        self.get_unchecked(0)
    }

    /// Gets the last byte or `None` if `self` is empty (`self.is_empty()`).
    #[inline]
    pub fn last(&self) -> Option<u8> {
        self.get(self.len() - 1)
    }

    /// Gets the last byte.
    ///
    /// # Panics
    ///
    /// Panics if `self.is_empty()`.
    #[inline]
    pub fn last_unchecked(&self) -> u8 {
        self.get_unchecked(self.len() - 1)
    }

    /// Gets a slice of `self` for the given range.
    #[inline]
    pub fn slice(&self, range: impl RangeBounds<usize>) -> Self {
        Self {
            bytes: self.bytes.slice(range),
        }
    }

    /// Returns `self` as a slice of type `&[u8]`.
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        self.bytes.as_slice()
    }

    /// Returns `self` as a vector of type `Vec<u8>`.
    #[inline]
    pub fn to_vec(&self) -> Vec<u8> {
        self.as_slice().to_vec()
    }
}

impl From<Rc<Vec<u8>>> for Bytes {
    #[inline]
    fn from(value: Rc<Vec<u8>>) -> Self {
        if value.is_empty() {
            Self {
                bytes: InternalBytes::Empty,
            }
        } else {
            Self {
                bytes: InternalBytes::Vec(value),
            }
        }
    }
}

impl From<&Rc<Vec<u8>>> for Bytes {
    #[inline]
    fn from(value: &Rc<Vec<u8>>) -> Self {
        if value.is_empty() {
            Self {
                bytes: InternalBytes::Empty,
            }
        } else {
            Self {
                bytes: InternalBytes::Vec(value.clone()),
            }
        }
    }
}

impl From<Vec<u8>> for Bytes {
    #[inline]
    fn from(value: Vec<u8>) -> Self {
        if value.is_empty() {
            Self {
                bytes: InternalBytes::Empty,
            }
        } else {
            Self {
                bytes: InternalBytes::Vec(Rc::new(value)),
            }
        }
    }
}

impl From<&Vec<u8>> for Bytes {
    #[inline]
    fn from(value: &Vec<u8>) -> Self {
        if value.is_empty() {
            Self {
                bytes: InternalBytes::Empty,
            }
        } else {
            Self {
                bytes: InternalBytes::Vec(Rc::new(value.clone())),
            }
        }
    }
}

impl From<Vec<&u8>> for Bytes {
    #[inline]
    fn from(value: Vec<&u8>) -> Self {
        if value.is_empty() {
            Self {
                bytes: InternalBytes::Empty,
            }
        } else {
            Self {
                bytes: InternalBytes::Vec(Rc::new(value.iter().map(|byte| **byte).collect())),
            }
        }
    }
}

impl From<&Vec<&u8>> for Bytes {
    #[inline]
    fn from(value: &Vec<&u8>) -> Self {
        if value.is_empty() {
            Self {
                bytes: InternalBytes::Empty,
            }
        } else {
            Self {
                bytes: InternalBytes::Vec(Rc::new(value.iter().map(|byte| **byte).collect())),
            }
        }
    }
}

impl From<&[u8]> for Bytes {
    #[inline]
    fn from(value: &[u8]) -> Self {
        if value.is_empty() {
            Self {
                bytes: InternalBytes::Empty,
            }
        } else {
            Self {
                bytes: InternalBytes::Vec(Rc::new(value.to_vec())),
            }
        }
    }
}

impl From<&[&u8]> for Bytes {
    #[inline]
    fn from(value: &[&u8]) -> Self {
        if value.is_empty() {
            Self {
                bytes: InternalBytes::Empty,
            }
        } else {
            Self {
                bytes: InternalBytes::Vec(Rc::new(value.iter().map(|byte| **byte).collect())),
            }
        }
    }
}

impl<const N: usize> From<&[u8; N]> for Bytes {
    #[inline]
    fn from(value: &[u8; N]) -> Self {
        if value.is_empty() {
            Self {
                bytes: InternalBytes::Empty,
            }
        } else {
            Self {
                bytes: InternalBytes::Vec(Rc::new(value.to_vec())),
            }
        }
    }
}

impl<const N: usize> From<&[&u8; N]> for Bytes {
    #[inline]
    fn from(value: &[&u8; N]) -> Self {
        if value.is_empty() {
            Self {
                bytes: InternalBytes::Empty,
            }
        } else {
            Self {
                bytes: InternalBytes::Vec(Rc::new(value.iter().map(|byte| **byte).collect())),
            }
        }
    }
}

impl<const N: usize> From<[u8; N]> for Bytes {
    #[inline]
    fn from(value: [u8; N]) -> Self {
        if value.is_empty() {
            Self {
                bytes: InternalBytes::Empty,
            }
        } else {
            Self {
                bytes: InternalBytes::Vec(Rc::new(value.to_vec())),
            }
        }
    }
}

impl<const N: usize> From<[&u8; N]> for Bytes {
    #[inline]
    fn from(value: [&u8; N]) -> Self {
        if value.is_empty() {
            Self {
                bytes: InternalBytes::Empty,
            }
        } else {
            Self {
                bytes: InternalBytes::Vec(Rc::new(value.iter().map(|byte| **byte).collect())),
            }
        }
    }
}

impl FromIterator<u8> for Bytes {
    fn from_iter<T: IntoIterator<Item = u8>>(iter: T) -> Self {
        Self::from(iter.into_iter().collect::<Vec<_>>())
    }
}

impl<'a> FromIterator<&'a u8> for Bytes {
    fn from_iter<T: IntoIterator<Item = &'a u8>>(iter: T) -> Self {
        Self::from(iter.into_iter().copied().collect::<Vec<_>>())
    }
}

#[cfg(test)]
mod test_bytes_spits {
    use std::rc::Rc;

    use super::{Bytes, InternalBytes};

    fn new_rc_vec(data: &[u8]) -> Rc<Vec<u8>> {
        Rc::new(Vec::from(data))
    }
    fn new_empty() -> Bytes {
        Bytes {
            bytes: InternalBytes::Empty,
        }
    }
    fn new_vec(data: &[u8]) -> Bytes {
        Bytes {
            bytes: InternalBytes::Vec(new_rc_vec(data)),
        }
    }
    fn new_front(data: &[u8], end: usize) -> Bytes {
        Bytes {
            bytes: InternalBytes::Front {
                data: new_rc_vec(data),
                end,
            },
        }
    }
    fn new_tail(data: &[u8], start: usize) -> Bytes {
        Bytes {
            bytes: InternalBytes::Tail {
                data: new_rc_vec(data),
                start,
            },
        }
    }
    fn new_slice(data: &[u8], start: usize, end: usize) -> Bytes {
        Bytes {
            bytes: InternalBytes::Slice {
                data: new_rc_vec(data),
                start,
                end,
            },
        }
    }

    macro_rules! ok_split_test {
        ($test_name:ident, $init:expr, $split_at:literal, $expected_left:expr, $expected_right:expr) => {
            #[test]
            fn $test_name() {
                let init_data = $init;
                let expected_left = $expected_left;
                let expected_right = $expected_right;

                let result = init_data.split_at_checked($split_at);
                assert!(result.is_some());

                let (left, right) = result.unwrap();
                assert_eq!(left, expected_left);
                assert_eq!(right, expected_right);

                // Verify the lengths add up.
                assert_eq!(left.len() + right.len(), init_data.len());

                // Verify that the indexes all line up.
                for (index, value) in expected_left
                    .as_slice()
                    .iter()
                    .chain(expected_right.as_slice().iter())
                    .enumerate()
                {
                    let result = init_data.get(index);
                    assert!(result.is_some());
                    let result = result.unwrap();
                    assert_eq!(result, *value);
                }
            }
        };
    }

    macro_rules! err_split_test {
        ($test_name:ident, $init:expr, $split_at:literal) => {
            #[test]
            fn $test_name() {
                let init_data = $init;

                let result = init_data.split_at_checked($split_at);
                assert!(result.is_none());
            }
        };
    }

    ok_split_test!(empty_split_at_0, new_empty(), 0, new_empty(), new_empty());
    err_split_test!(empty_split_at_1, new_empty(), 1);

    ok_split_test!(
        vec_split_at_0,
        new_vec(&[0, 1, 2]),
        0,
        new_empty(),
        new_vec(&[0, 1, 2])
    );
    ok_split_test!(
        vec_split_at_1,
        new_vec(&[0, 1, 2]),
        1,
        new_front(&[0, 1, 2], 1),
        new_tail(&[0, 1, 2], 1)
    );
    ok_split_test!(
        vec_split_at_2,
        new_vec(&[0, 1, 2]),
        2,
        new_front(&[0, 1, 2], 2),
        new_tail(&[0, 1, 2], 2)
    );
    ok_split_test!(
        vec_split_at_3,
        new_vec(&[0, 1, 2]),
        3,
        new_vec(&[0, 1, 2]),
        new_empty()
    );
    err_split_test!(vec_split_at_4, new_vec(&[0, 1, 2]), 4);

    ok_split_test!(
        front_split_at_0,
        new_front(&[0, 1, 2], 2),
        0,
        new_empty(),
        new_front(&[0, 1, 2], 2)
    );
    ok_split_test!(
        front_split_at_1,
        new_front(&[0, 1, 2], 2),
        1,
        new_front(&[0, 1, 2], 1),
        new_slice(&[0, 1, 2], 1, 2)
    );
    ok_split_test!(
        front_split_at_2,
        new_front(&[0, 1, 2], 2),
        2,
        new_front(&[0, 1, 2], 2),
        new_empty()
    );
    err_split_test!(front_split_at_3, new_front(&[0, 1, 2], 2), 3);

    ok_split_test!(
        tail_split_at_0,
        new_tail(&[0, 1, 2], 1),
        0,
        new_empty(),
        new_tail(&[0, 1, 2], 1)
    );
    ok_split_test!(
        tail_split_at_1,
        new_tail(&[0, 1, 2], 1),
        1,
        new_slice(&[0, 1, 2], 1, 2),
        new_tail(&[0, 1, 2], 2)
    );
    ok_split_test!(
        tail_split_at_2,
        new_tail(&[0, 1, 2], 1),
        2,
        new_tail(&[0, 1, 2], 1),
        new_empty()
    );
    err_split_test!(tail_split_at_3, new_tail(&[0, 1, 2], 1), 3);

    ok_split_test!(
        slice_split_at_0,
        new_slice(&[0, 1, 2, 3, 4, 5], 1, 4),
        0,
        new_empty(),
        new_slice(&[0, 1, 2, 3, 4, 5], 1, 4)
    );
    ok_split_test!(
        slice_split_at_1,
        new_slice(&[0, 1, 2, 3, 4, 5], 1, 4),
        1,
        new_slice(&[0, 1, 2, 3, 4, 5], 1, 2),
        new_slice(&[0, 1, 2, 3, 4, 5], 2, 4)
    );
    ok_split_test!(
        slice_split_at_2,
        new_slice(&[0, 1, 2, 3, 4, 5], 1, 4),
        2,
        new_slice(&[0, 1, 2, 3, 4, 5], 1, 3),
        new_slice(&[0, 1, 2, 3, 4, 5], 3, 4)
    );
    ok_split_test!(
        slice_split_at_3,
        new_slice(&[0, 1, 2, 3, 4, 5], 1, 4),
        3,
        new_slice(&[0, 1, 2, 3, 4, 5], 1, 4),
        new_empty()
    );
    err_split_test!(slice_split_at_4, new_slice(&[0, 1, 2, 3, 4, 5], 1, 4), 4);

    macro_rules! ok_as_slice_test {
        ($test_name:ident, $init:expr, $slice:expr) => {
            #[test]
            fn $test_name() {
                let init_data = $init;
                let expected_slice = $slice;

                let result = init_data.as_slice();

                assert_eq!(result, expected_slice);

                // Verify the lengths are the same.
                assert_eq!(result.len(), expected_slice.len());

                // Verify that each index is the same (this checks the `get()` function)
                for (index, value) in expected_slice.iter().enumerate() {
                    let result = init_data.get(index);
                    assert!(result.is_some());
                    let result = result.unwrap();
                    assert_eq!(result, *value);
                }

                // Verify that you cannot `get()` after the last index.
                let len = expected_slice.len();
                let result = init_data.get(len);
                assert!(result.is_none());
            }
        };
    }

    ok_as_slice_test! {
        empty_as_slice,
        new_empty(),
        &[]
    }
    ok_as_slice_test! {
        vec_as_slice,
        new_vec(&[0, 1, 2]),
        &[0, 1, 2]
    }
    ok_as_slice_test! {
        front_as_slice,
        new_front(&[0, 1, 2], 2),
        &[0, 1]
    }
    ok_as_slice_test! {
        tail_as_slice,
        new_tail(&[0, 1, 2], 1),
        &[1, 2]
    }
    ok_as_slice_test! {
        slice_as_slice,
        new_slice(&[0, 1, 2, 3, 4], 1, 4),
        &[1, 2, 3]
    }

    macro_rules! ok_slice_test {
        ($test_name:ident, $init:expr, $range:expr, $expected:expr) => {
            #[test]
            fn $test_name() {
                let init_data = $init;
                let expected = $expected;

                let result = init_data.slice($range);

                assert_eq!(result, expected);
                // Verify that the correct data is being represented.
                assert_eq!(result.as_slice(), &init_data.as_slice()[$range])
            }
        };
    }

    macro_rules! err_slice_test {
        ($test_name:ident, $init:expr, $range:expr) => {
            #[test]
            #[should_panic]
            fn $test_name() {
                let init_data = $init;

                let _ = init_data.slice($range);
            }
        };
    }

    ok_slice_test! {
        empty_slice_unbounded,
        new_empty(),
        ..,
        new_empty()
    }
    ok_slice_test! {
        empty_slice_0_to_unbounded,
        new_empty(),
        0..,
        new_empty()
    }
    ok_slice_test! {
        empty_slice_unbounded_to_0,
        new_empty(),
        ..0,
        new_empty()
    }
    err_slice_test! {
        empty_slice_1_to_unbounded,
        new_empty(),
        1..
    }
    err_slice_test! {
        empty_slice_1_to_0,
        new_empty(),
        1..0
    }

    ok_slice_test! {
        vec_slice_unbounded,
        new_vec(&[0, 1, 2, 3, 4]),
        ..,
        new_vec(&[0, 1, 2, 3, 4])
    }
    ok_slice_test! {
        vec_slice_0_to_0,
        new_vec(&[0, 1, 2, 3, 4]),
        0..0,
        new_empty()
    }
    ok_slice_test! {
        vec_slice_0_to_2,
        new_vec(&[0, 1, 2, 3, 4]),
        0..2,
        new_front(&[0, 1, 2, 3, 4], 2)
    }
    ok_slice_test! {
        vec_slice_3_to_5,
        new_vec(&[0, 1, 2, 3, 4]),
        3..5,
        new_tail(&[0, 1, 2, 3, 4], 3)
    }
    ok_slice_test! {
        vec_slice_1_to_3,
        new_vec(&[0, 1, 2, 3, 4]),
        1..3,
        new_slice(&[0, 1, 2, 3, 4], 1, 3)
    }
    err_slice_test! {
        vec_slice_4_to_6,
        new_vec(&[0, 1, 2, 3, 4]),
        4..6
    }

    ok_slice_test! {
        front_slice_unbounded,
        new_front(&[0, 1, 2, 3, 4], 3),
        ..,
        new_front(&[0, 1, 2, 3, 4], 3)
    }
    ok_slice_test! {
        front_slice_0_to_0,
        new_front(&[0, 1, 2, 3, 4], 3),
        0..0,
        new_empty()
    }
    ok_slice_test! {
        front_slice_0_to_2,
        new_front(&[0, 1, 2, 3, 4], 3),
        0..2,
        new_front(&[0, 1, 2, 3, 4], 2)
    }
    ok_slice_test! {
        front_slice_2_to_3,
        new_front(&[0, 1, 2, 3, 4], 3),
        2..3,
        new_slice(&[0, 1, 2, 3, 4], 2, 3)
    }
    ok_slice_test! {
        front_slice_1_to_2,
        new_front(&[0, 1, 2, 3, 4], 3),
        1..2,
        new_slice(&[0, 1, 2, 3, 4], 1, 2)
    }
    err_slice_test! {
        front_slice_2_to_4,
        new_front(&[0, 1, 2, 3, 4], 3),
        2..4
    }

    ok_slice_test! {
        tail_slice_unbounded,
        new_tail(&[0, 1, 2, 3, 4], 1),
        ..,
        new_tail(&[0, 1, 2, 3, 4], 1)
    }
    ok_slice_test! {
        tail_slice_0_to_0,
        new_tail(&[0, 1, 2, 3, 4], 1),
        0..0,
        new_empty()
    }
    ok_slice_test! {
        tail_slice_0_to_2,
        new_tail(&[0, 1, 2, 3, 4], 1),
        0..2,
        new_slice(&[0, 1, 2, 3, 4], 1, 3)
    }
    ok_slice_test! {
        tail_slice_1_to_2,
        new_tail(&[0, 1, 2, 3, 4], 1),
        1..2,
        new_slice(&[0, 1, 2, 3, 4], 2, 3)
    }
    ok_slice_test! {
        tail_slice_2_to_4,
        new_tail(&[0, 1, 2, 3, 4], 1),
        2..4,
        new_tail(&[0, 1, 2, 3, 4], 3)
    }
    err_slice_test! {
        tail_slice_3_to_5,
        new_tail(&[0, 1, 2, 3, 4], 1),
        3..5
    }

    ok_slice_test! {
        slice_slice_unbounded,
        new_slice(&[0, 1, 2, 3, 4], 1, 3),
        ..,
        new_slice(&[0, 1, 2, 3, 4], 1, 3)
    }
    ok_slice_test! {
        slice_slice_0_to_0,
        new_slice(&[0, 1, 2, 3, 4], 1, 3),
        0..0,
        new_empty()
    }
    ok_slice_test! {
        slice_slice_0_to_1,
        new_slice(&[0, 1, 2, 3, 4], 1, 3),
        0..1,
        new_slice(&[0, 1, 2, 3, 4], 1, 2)
    }
    ok_slice_test! {
        slice_slice_1_to_2,
        new_slice(&[0, 1, 2, 3, 4], 1, 3),
        1..2,
        new_slice(&[0, 1, 2, 3, 4], 2, 3)
    }
    err_slice_test! {
        slice_slice_1_to_3,
        new_slice(&[0, 1, 2, 3, 4], 1, 3),
        1..3
    }
}

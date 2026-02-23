use std::{fmt::Debug, iter::FusedIterator};

use static_assertions::const_assert;
use tinyvec::TinyVec;

use crate::types::{
    ascii::AsciiChar,
    domain_name::{
        DomainName, DomainNameMut, DomainNameOwned, InsertError, LENGTH_OCTET_WIDTH,
        MAX_LABEL_OCTETS, MAX_LABELS, MAX_OCTETS, MakeCanonicalError, MakeFullyQualifiedError,
        PushBackError, PushFrontError, assert_domain_name_invariants, would_exceed_max_octets,
    },
    label::{Label, OwnedLabel, RefLabel},
};

/// Assert all invariants required to ensure that the raw parts of a raw domain
/// name are correct and that it is safe to pass those arguments to
/// `RawSubDomainName::from_raw_parts()`.
///
/// This function is `const`. This means that it can be called at compile time
/// in a `const` setting where it will cause compilation to fail if the
/// invariants are not met.
///
/// # Panics
///
/// This function will panic if any required invariant to ensure that it is safe
/// to use the arguments with `RawSubDomainName::from_raw_parts()` are not met.
pub const fn assert_raw_domain_name_invariants(octets: &[u8]) {
    let length_octets = &mut [0; MAX_LABELS as usize];
    let mut length_octets_index = 0;
    let mut octets_index = 0;
    while octets_index < octets.len() {
        assert!(
            length_octets_index < length_octets.len(),
            "domain name specified must be valid but its total label count exceeds MAX_LABELS",
        );
        length_octets[length_octets_index] = octets[octets_index];
        length_octets_index += 1;
        octets_index += octets[octets_index] as usize + LENGTH_OCTET_WIDTH;
    }
    let (length_octets, _) = length_octets.as_slice().split_at(length_octets_index);

    assert_domain_name_invariants(octets, length_octets);
}

#[derive(Debug, Clone)]
pub struct RawDomainVec {
    /// Octets of the domain name in uncompressed wire format.
    pub(super) octets: Vec<AsciiChar>,
}

#[derive(Debug, Clone, Copy)]
pub struct RawDomainArray<const OCTETS: usize> {
    /// Octets still contains label lengths inline despite `length_octets`
    /// containing all the length octets. This way, `octets` maintains the exact
    /// same layout as the wire format for speedy serialization/deserialization.
    pub(super) octets: [AsciiChar; OCTETS],
}

#[derive(Debug)]
pub struct RawMutDomainSlice<'a> {
    /// Octets of the domain name in uncompressed wire format.
    pub(super) octets: &'a mut [AsciiChar],
}

#[derive(Debug, Copy, Clone)]
pub struct RawDomainSlice<'a> {
    /// Octets of the domain name in uncompressed wire format.
    pub(super) octets: &'a [AsciiChar],
}

impl RawDomainVec {
    /// Verify that the `octets` field does not make any obvious invariant
    /// violations.
    pub(super) const fn assert_invariants(&self) {
        assert_raw_domain_name_invariants(self.octets.as_slice());
    }
}

impl<const OCTETS: usize> RawDomainArray<OCTETS> {
    /// Verify that the `octets` field does not make any obvious invariant
    /// violations.
    pub(super) fn assert_invariants(&self) {
        assert_raw_domain_name_invariants(self.octets.as_slice());
    }
}

impl RawDomainSlice<'_> {
    /// Verify that the `octets` field does not make any obvious invariant
    /// violations.
    pub(super) const fn assert_invariants(&self) {
        assert_raw_domain_name_invariants(self.octets);
    }
}

impl RawMutDomainSlice<'_> {
    /// Verify that the `octets` field does not make any obvious invariant
    /// violations.
    pub(super) const fn assert_invariants(&self) {
        assert_raw_domain_name_invariants(self.octets);
    }
}

impl_domain_name!(impl for RawDomainVec);
impl_domain_name!(impl for RawDomainSlice<'_>);
impl_domain_name!(impl for RawMutDomainSlice<'_>);

impl RawDomainVec {
    pub fn as_raw_domain_slice(&self) -> RawDomainSlice<'_> {
        RawDomainSlice {
            octets: &self.octets,
        }
        .and_debug_assert_invariants()
    }

    pub fn as_raw_domain_slice_mut(&mut self) -> RawMutDomainSlice<'_> {
        RawMutDomainSlice {
            octets: &mut self.octets,
        }
        .and_debug_assert_invariants()
    }

    pub fn to_raw_domain_vec(&self) -> RawDomainVec {
        self.clone().and_debug_assert_invariants()
    }
}

impl<const OCTETS: usize> RawDomainArray<OCTETS> {
    pub fn as_raw_domain_slice(&self) -> RawDomainSlice<'_> {
        RawDomainSlice {
            octets: &self.octets,
        }
        .and_debug_assert_invariants()
    }

    pub fn as_raw_domain_slice_mut(&mut self) -> RawMutDomainSlice<'_> {
        RawMutDomainSlice {
            octets: &mut self.octets,
        }
        .and_debug_assert_invariants()
    }

    pub fn to_raw_domain_vec(&self) -> RawDomainVec {
        self.as_raw_domain_slice()
            .to_raw_domain_vec()
            .and_debug_assert_invariants()
    }
}

impl<'a> RawMutDomainSlice<'a> {
    pub fn as_raw_domain_slice(&self) -> RawDomainSlice<'_> {
        RawDomainSlice {
            octets: self.octets,
        }
        .and_debug_assert_invariants()
    }

    pub fn to_raw_domain_vec(&self) -> RawDomainVec {
        self.as_raw_domain_slice()
            .to_raw_domain_vec()
            .and_debug_assert_invariants()
    }
}

impl<'a> RawDomainSlice<'a> {
    pub fn as_raw_domain_slice(&self) -> RawDomainSlice<'a> {
        (*self).and_debug_assert_invariants()
    }

    pub fn to_raw_domain_vec(&self) -> RawDomainVec {
        RawDomainVec {
            octets: self.octets.to_vec(),
        }
        .and_debug_assert_invariants()
    }
}

impl<'a> RawDomainSlice<'a> {
    /// Create a `RawSubDomainName` from its raw components.
    ///
    /// # Safety
    ///
    /// The `octets` must be a valid non-compressed wire-encoded domain name,
    /// although it is not required to be fully qualified. Requirements include:
    ///
    ///  - The total number of labels must be less than `MAX_LABELS` (128).
    ///  - The total length of this field cannot exceed `MAX_OCTETS` (256)
    ///    bytes.
    ///  - No single label may exceed a length of `MAX_LABEL_OCTETS` (63) bytes
    ///    (not including the length octet).
    ///  - Only the last label may be a root label.
    ///
    /// See RFC 1035 for details about this encoding scheme used for the
    /// `octets`.
    pub const unsafe fn from_raw_parts(octets: &'a [u8]) -> Self {
        RawDomainSlice { octets }
    }

    fn label_index_to_octet_index(&self, mut label_index: usize) -> Option<usize> {
        let mut octets_index = 0;
        while label_index > 0 {
            label_index -= 1;
            octets_index += self.octets.get(octets_index).copied()? as usize + LENGTH_OCTET_WIDTH;
        }
        Some(octets_index)
    }

    fn last_label_octet_index(&self) -> Option<usize> {
        if self.octets.is_empty() {
            return None;
        }
        let mut last_label_index = 0;
        while (last_label_index + (self.octets[last_label_index] as usize) + LENGTH_OCTET_WIDTH)
            < self.octets.len()
        {
            last_label_index += self.octets[last_label_index] as usize + LENGTH_OCTET_WIDTH;
        }
        Some(last_label_index)
    }

    /// Divides one domain into two halves at an index.
    ///
    /// The first will contain all labels from `[0, mid)` (excluding the index
    /// `mid` itself) and the second will contain all indices from
    /// `[mid, label_count)` (excluding the index `label_count` itself).
    ///
    /// # Panics
    ///
    /// Panics if `mid > label_count`.  For a non-panicking alternative see
    /// `split_at_checked()`
    pub fn split_at(&self, mid: usize) -> (RawDomainSlice<'a>, RawDomainSlice<'a>) {
        const_assert!(
            (MAX_LABELS as usize) < usize::MAX,
            "MAX_LABELS must be less than usize::MAX because if it were equal to or larger, the `mid` after the end of `length_octets` could not be represented"
        );

        self.split_at_checked(mid).expect("mid > label_count")
    }

    /// Divides one domain into two at an index, returning `None` if the domain
    /// is too short.
    ///
    /// If `mid ≤ label_count` returns a pair of domains where the first will
    /// contain all indices from `[0, mid)` (excluding the index `mid` itself)
    /// and the second will contain all indices from `[mid, label_count)`
    /// (excluding the index `label_count` itself).
    ///
    /// Otherwise, if `mid > label_count`, returns `None`.
    pub fn split_at_checked(&self, mid: usize) -> Option<(RawDomainSlice<'a>, RawDomainSlice<'a>)> {
        const_assert!(
            (MAX_LABELS as usize) < usize::MAX,
            "MAX_LABELS must be less than usize::MAX because if it were equal to or larger, the `mid` after the end of `length_octets` could not be represented"
        );
        const_assert!(
            (MAX_OCTETS as usize) < usize::MAX,
            "MAX_OCTETS must be less than usize::MAX because if it were equal to or larger, the `octets_mid` after the end of `octets` could not be represented"
        );

        let octets_mid = self.label_index_to_octet_index(mid)?;
        let (left_octets, right_octets) = self.octets.split_at(octets_mid);

        Some((
            Self {
                octets: left_octets,
            }
            .and_debug_assert_invariants(),
            Self {
                octets: right_octets,
            }
            .and_debug_assert_invariants(),
        ))
    }

    /// Returns the first label and all the rest of the labels of the domain, or
    /// `None` if it is empty.
    pub fn split_first(&self) -> Option<(&'a RefLabel, RawDomainSlice<'a>)> {
        const_assert!(
            (MAX_LABEL_OCTETS as usize) <= (usize::MAX - LENGTH_OCTET_WIDTH),
            "MAX_LABEL_OCTETS must be at most `usize::MAX - LENGTH_OCTET_WIDTH` because if it were greater, `+ LENGTH_OCTET_WIDTH` would overflow"
        );

        let first_length_octet = *self.octets.first()?;
        let (first_octets, remaining_octets) = self
            .octets
            .split_at((first_length_octet as usize) + LENGTH_OCTET_WIDTH);

        Some((
            RefLabel::from_octets(&first_octets[LENGTH_OCTET_WIDTH..]),
            Self {
                octets: remaining_octets,
            }
            .and_debug_assert_invariants(),
        ))
    }

    /// Returns the last label and all the rest of the labels of the domain, or
    /// `None` if it is empty.
    pub fn split_last(&self) -> Option<(&'a RefLabel, RawDomainSlice<'a>)> {
        let last_label_index = self.last_label_octet_index()?;
        let (remaining_octets, last_octets) = self.octets.split_at(last_label_index);

        Some((
            RefLabel::from_octets(&last_octets[LENGTH_OCTET_WIDTH..]),
            Self {
                octets: remaining_octets,
            }
            .and_debug_assert_invariants(),
        ))
    }

    /// Gets the `n`th label in the domain or `None` if `index` is out of
    /// bounds.
    pub fn get(&self, index: usize) -> Option<&'a RefLabel> {
        let octets_start = self.label_index_to_octet_index(index)?;
        let length_octet = self.octets[octets_start];
        Some(RefLabel::from_octets(
            &self.octets[(octets_start + LENGTH_OCTET_WIDTH)
                ..(octets_start + (length_octet as usize) + LENGTH_OCTET_WIDTH)],
        ))
    }

    /// Returns the first label of the domain, or `None` if it is empty.
    pub fn first(&self) -> Option<&'a RefLabel> {
        let length_octet = self.octets.first().copied()?;
        Some(RefLabel::from_octets(
            &self.octets[LENGTH_OCTET_WIDTH..((length_octet as usize) + LENGTH_OCTET_WIDTH)],
        ))
    }

    /// Returns the last label of the domain, or `None` if it is empty.
    pub fn last(&self) -> Option<&'a RefLabel> {
        let last_label_index = self.last_label_octet_index()?;
        let length_octet = self.octets[last_label_index];
        Some(RefLabel::from_octets(
            &self.octets[(self.octets.len() - (length_octet as usize))..],
        ))
    }

    pub(super) fn into_labels_iter(
        self,
    ) -> impl 'a
    + DoubleEndedIterator<Item = &'a RefLabel>
    + ExactSizeIterator
    + FusedIterator
    + Debug
    + Clone {
        LabelIter::new(self)
    }

    /// Returns an iterator over the labels of this domain name, starting from
    /// the left-most label, and iterating towards the right-most label (often,
    /// the root label).
    pub fn subdomains_iter(
        &self,
    ) -> impl 'a + DoubleEndedIterator<Item = RawDomainSlice<'a>> + FusedIterator + Debug + Clone
    {
        RawSubDomainIter::new(*self)
    }
}

impl<'a> RawMutDomainSlice<'a> {
    pub fn as_lowercase(&self) -> RawDomainVec {
        let mut uppercase_domain = self.to_raw_domain_vec();
        uppercase_domain.make_lowercase();
        uppercase_domain.and_debug_assert_invariants()
    }

    pub fn as_uppercase(&self) -> RawDomainVec {
        let mut uppercase_domain = self.to_raw_domain_vec();
        uppercase_domain.make_uppercase();
        uppercase_domain.and_debug_assert_invariants()
    }

    pub fn as_fully_qualified(&self) -> Result<RawDomainVec, MakeFullyQualifiedError> {
        if self.is_fully_qualified() {
            Ok(self.to_raw_domain_vec())
        // aka. Would adding a byte exceed the limit?
        } else if self.octets.len() >= MAX_OCTETS as usize {
            Err(MakeFullyQualifiedError::TooManyOctets)
        } else {
            let mut octets = Vec::with_capacity(self.octets.len() + LENGTH_OCTET_WIDTH);
            octets.extend_from_slice(self.octets);
            octets.push(0);

            Ok(RawDomainVec { octets }.and_debug_assert_invariants())
        }
    }

    pub fn as_canonical(&self) -> Result<RawDomainVec, MakeCanonicalError> {
        let mut domain = self.as_fully_qualified()?;
        domain.make_lowercase();
        domain.debug_assert_invariants();
        Ok(domain)
    }

    /// Divides one domain into two halves at an index.
    ///
    /// The first will contain all labels from `[0, mid)` (excluding the index
    /// `mid` itself) and the second will contain all indices from
    /// `[mid, label_count)` (excluding the index `label_count` itself).
    ///
    /// # Panics
    ///
    /// Panics if `mid > label_count`.  For a non-panicking alternative see
    /// `split_at_checked()`
    pub fn split_at_mut(&mut self, mid: usize) -> (RawMutDomainSlice<'_>, RawMutDomainSlice<'_>) {
        const_assert!(
            (MAX_LABELS as usize) < usize::MAX,
            "MAX_LABELS must be less than usize::MAX because if it were equal to or larger, the `mid` after the end of `length_octets` could not be represented"
        );

        self.split_at_mut_checked(mid).expect("mid > label_count")
    }

    /// Divides one domain into two at an index, returning `None` if the domain
    /// is too short.
    ///
    /// If `mid ≤ label_count` returns a pair of domains where the first will
    /// contain all indices from `[0, mid)` (excluding the index `mid` itself)
    /// and the second will contain all indices from `[mid, label_count)`
    /// (excluding the index `label_count` itself).
    ///
    /// Otherwise, if `mid > label_count`, returns `None`.
    pub fn split_at_mut_checked(
        &mut self,
        mid: usize,
    ) -> Option<(RawMutDomainSlice<'_>, RawMutDomainSlice<'_>)> {
        const_assert!(
            (MAX_LABELS as usize) < usize::MAX,
            "MAX_LABELS must be less than usize::MAX because if it were equal to or larger, the `mid` after the end of `length_octets` could not be represented"
        );
        const_assert!(
            (MAX_OCTETS as usize) < usize::MAX,
            "MAX_OCTETS must be less than usize::MAX because if it were equal to or larger, the `octets_mid` after the end of `octets` could not be represented"
        );

        let octets_mid = self.as_raw_domain_slice().label_index_to_octet_index(mid)?;
        let (left_octets, right_octets) = self.octets.split_at_mut(octets_mid);

        Some((
            RawMutDomainSlice {
                octets: left_octets,
            }
            .and_debug_assert_invariants(),
            RawMutDomainSlice {
                octets: right_octets,
            }
            .and_debug_assert_invariants(),
        ))
    }

    /// Returns the first label and all the rest of the labels of the domain, or
    /// `None` if it is empty.
    pub fn split_first_mut(&mut self) -> Option<(&mut RefLabel, RawMutDomainSlice<'_>)> {
        const_assert!(
            (MAX_LABEL_OCTETS as usize) <= (usize::MAX - LENGTH_OCTET_WIDTH),
            "MAX_LABEL_OCTETS must be at most `usize::MAX - LENGTH_OCTET_WIDTH` because if it were greater, `+ LENGTH_OCTET_WIDTH` would overflow"
        );

        let first_length_octet = *self.octets.first()?;
        let (first_octets, remaining_octets) = self
            .octets
            .split_at_mut((first_length_octet as usize) + LENGTH_OCTET_WIDTH);

        Some((
            RefLabel::from_octets_mut(&mut first_octets[LENGTH_OCTET_WIDTH..]),
            RawMutDomainSlice {
                octets: remaining_octets,
            }
            .and_debug_assert_invariants(),
        ))
    }

    /// Returns the last label and all the rest of the labels of the domain, or
    /// `None` if it is empty.
    pub fn split_last_mut(&mut self) -> Option<(&mut RefLabel, RawMutDomainSlice<'_>)> {
        let last_label_index = self.as_raw_domain_slice().last_label_octet_index()?;
        let (remaining_octets, last_octets) = self.octets.split_at_mut(last_label_index);

        Some((
            RefLabel::from_octets_mut(&mut last_octets[LENGTH_OCTET_WIDTH..]),
            RawMutDomainSlice {
                octets: remaining_octets,
            }
            .and_debug_assert_invariants(),
        ))
    }

    /// Divides one domain into two at an index, returning `None` if the domain
    /// is too short.
    ///
    /// If `mid ≤ label_count` returns the left domain which will contain all
    /// indices from `[0, mid)` (excluding the index `mid` itself) and `self`
    /// will contain all indices from `[mid, label_count)` (excluding the index
    /// `label_count` itself).
    ///
    /// # Panics
    ///
    /// Panics if `mid > label_count`.  For a non-panicking alternative see
    /// `split_off_before_checked()`
    pub fn split_off_before(&mut self, mid: usize) -> RawMutDomainSlice<'a> {
        const_assert!(
            (MAX_LABELS as usize) < usize::MAX,
            "MAX_LABELS must be less than usize::MAX because if it were equal to or larger, the `mid` after the end of `length_octets` could not be represented"
        );

        self.split_off_before_checked(mid)
            .expect("mid > label_count")
    }

    /// Divides one domain into two at an index, returning `None` if the domain
    /// is too short.
    ///
    /// If `mid ≤ label_count` returns the right domain which will contain all
    /// indices from `[mid, label_count)` (excluding the index `label_count`
    /// itself) and `self` will contain all indices from `[0, mid)` (excluding
    /// the index `mid` itself).
    ///
    /// # Panics
    ///
    /// Panics if `mid > label_count`.  For a non-panicking alternative see
    /// `split_off_after_checked()`
    pub fn split_off_after(&mut self, mid: usize) -> RawMutDomainSlice<'a> {
        const_assert!(
            (MAX_LABELS as usize) < usize::MAX,
            "MAX_LABELS must be less than usize::MAX because if it were equal to or larger, the `mid` after the end of `length_octets` could not be represented"
        );

        self.split_off_after_checked(mid)
            .expect("mid > label_count")
    }

    /// Divides one domain into two at an index, returning `None` if the domain
    /// is too short.
    ///
    /// If `mid ≤ label_count` returns the left domain which will contain all
    /// indices from `[0, mid)` (excluding the index `mid` itself) and `self`
    /// will contain all indices from `[mid, label_count)` (excluding the index
    /// `label_count` itself).
    ///
    /// Otherwise, if `mid > label_count`, returns `None`.
    pub fn split_off_before_checked(&mut self, mid: usize) -> Option<RawMutDomainSlice<'a>> {
        const_assert!(
            (MAX_LABELS as usize) < usize::MAX,
            "MAX_LABELS must be less than usize::MAX because if it were equal to or larger, the `mid` after the end of `length_octets` could not be represented"
        );
        const_assert!(
            (MAX_OCTETS as usize) < usize::MAX,
            "MAX_OCTETS must be less than usize::MAX because if it were equal to or larger, the `octets_mid` after the end of `octets` could not be represented"
        );

        let octets_mid = self.as_raw_domain_slice().label_index_to_octet_index(mid)?;
        let left_octets = self
            .octets
            .split_off_mut(..octets_mid)
            .expect("length octets must not sum past the end of the octets buffer");

        Some(
            RawMutDomainSlice {
                octets: left_octets,
            }
            .and_debug_assert_invariants(),
        )
    }

    /// Divides one domain into two at an index, returning `None` if the domain
    /// is too short.
    ///
    /// If `mid ≤ label_count` returns the right domain which will contain all
    /// indices from `[mid, label_count)` (excluding the index `label_count`
    /// itself) and `self` will contain all indices from `[0, mid)` (excluding
    /// the index `mid` itself).
    ///
    /// Otherwise, if `mid > label_count`, returns `None`.
    pub fn split_off_after_checked(&mut self, mid: usize) -> Option<RawMutDomainSlice<'a>> {
        const_assert!(
            (MAX_LABELS as usize) < usize::MAX,
            "MAX_LABELS must be less than usize::MAX because if it were equal to or larger, the `mid` after the end of `length_octets` could not be represented"
        );
        const_assert!(
            (MAX_OCTETS as usize) < usize::MAX,
            "MAX_OCTETS must be less than usize::MAX because if it were equal to or larger, the `octets_mid` after the end of `octets` could not be represented"
        );

        let octets_mid = self.as_raw_domain_slice().label_index_to_octet_index(mid)?;
        let right_octets = self
            .octets
            .split_off_mut(octets_mid..)
            .expect("length octets must not sum past the end of the octets buffer");

        Some(
            RawMutDomainSlice {
                octets: right_octets,
            }
            .and_debug_assert_invariants(),
        )
    }

    /// Removes and returns the first label from the rest of the labels of the
    /// domain, or `None` if it is empty.
    pub fn split_off_first_mut(&mut self) -> Option<&'a mut RefLabel> {
        const_assert!(
            (MAX_LABEL_OCTETS as usize) <= (usize::MAX - LENGTH_OCTET_WIDTH),
            "MAX_LABEL_OCTETS must be at most `usize::MAX - LENGTH_OCTET_WIDTH` because if it were greater, `+ LENGTH_OCTET_WIDTH` would overflow"
        );

        let first_length_octet = *self.octets.first()?;
        let first_octets = self
            .octets
            .split_off_mut(..((first_length_octet as usize) + LENGTH_OCTET_WIDTH))
            .expect("the first length octet must not index past the end of the octets buffer");

        Some(RefLabel::from_octets_mut(
            &mut first_octets[LENGTH_OCTET_WIDTH..],
        ))
    }

    /// Removes and returns the last label from the rest of the labels of the
    /// domain, or `None` if it is empty.
    pub fn split_off_last_mut(&mut self) -> Option<&'a mut RefLabel> {
        let last_label_index = self.as_raw_domain_slice().last_label_octet_index()?;
        let last_octets = self
            .octets
            .split_off_mut(last_label_index..)
            .expect("the first length octet must not index past the end of the octets buffer");

        Some(RefLabel::from_octets_mut(
            &mut last_octets[LENGTH_OCTET_WIDTH..],
        ))
    }

    /// Gets the `n`th label in the domain or `None` if `index` is out of
    /// bounds.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut RefLabel> {
        let octets_start = self
            .as_raw_domain_slice()
            .label_index_to_octet_index(index)?;
        let length_octet = self.octets[octets_start];
        Some(RefLabel::from_octets_mut(
            &mut self.octets[(octets_start + LENGTH_OCTET_WIDTH)
                ..(octets_start + (length_octet as usize) + LENGTH_OCTET_WIDTH)],
        ))
    }

    /// Returns the first label of the domain, or `None` if it is empty.
    pub fn first_mut(&mut self) -> Option<&mut RefLabel> {
        self.split_first_mut().map(|(first, _)| first)
    }

    /// Returns the last label of the domain, or `None` if it is empty.
    pub fn last_mut(&mut self) -> Option<&mut RefLabel> {
        self.split_last_mut().map(|(last, _)| last)
    }
}

impl DomainName for RawDomainSlice<'_> {
    fn octet_count(&self) -> u16 {
        const_assert!(
            MAX_OCTETS as usize <= u16::MAX as usize,
            "MAX_OCTETS cannot exceed u16::MAX because u16s are used for the octet count"
        );

        self.octets.len() as u16
    }
    fn label_count(&self) -> u16 {
        const_assert!(
            (MAX_LABELS as usize) <= (u16::MAX as usize),
            "MAX_LABELS cannot exceed u16::MAX because u16s are used for the label count"
        );

        let mut label_count = 0;
        let mut octets_index = 0;
        while octets_index < self.octets.len() {
            label_count += 1;
            octets_index += self.octets[octets_index] as usize + LENGTH_OCTET_WIDTH;
        }
        label_count
    }

    fn is_root(&self) -> bool {
        self.octets == [0]
    }

    fn labels_iter<'a>(
        &'a self,
    ) -> impl 'a
    + DoubleEndedIterator<Item = &'a RefLabel>
    + ExactSizeIterator
    + FusedIterator
    + Debug
    + Clone {
        (*self).into_labels_iter()
    }
}

macro_rules! impl_domain_name_as_raw_domain_slice {
    (impl $(( $($impl_generics:tt)+ ))? for $domain_type:ty) => {
        impl $(<$($impl_generics)+>)? $crate::types::domain_name::DomainName for $domain_type {
            fn octet_count(&self) -> u16 {
                self.as_raw_domain_slice().octet_count()
            }

            fn label_count(&self) -> u16 {
                self.as_raw_domain_slice().label_count()
            }

            fn is_root(&self) -> bool {
                self.as_raw_domain_slice().is_root()
            }

            fn is_lowercase(&self) -> bool {
                self.as_raw_domain_slice().is_lowercase()
            }

            fn is_uppercase(&self) -> bool {
                self.as_raw_domain_slice().is_uppercase()
            }

            fn is_fully_qualified(&self) -> bool {
                self.as_raw_domain_slice().is_fully_qualified()
            }

            fn is_canonical(&self) -> bool {
                self.as_raw_domain_slice().is_canonical()
            }

            fn first_label<'a>(&self) -> Option<&RefLabel> {
                self.as_raw_domain_slice().first()
            }

            fn last_label<'a>(&self) -> Option<&RefLabel> {
                self.as_raw_domain_slice().last()
            }

            fn labels_iter<'a>(
                &'a self,
            ) -> impl 'a
            + DoubleEndedIterator<Item = &'a RefLabel>
            + ExactSizeIterator
            + FusedIterator
            + Debug
            + Clone {
                self.as_raw_domain_slice().into_labels_iter()
            }
        }
    };
}

impl_domain_name_as_raw_domain_slice!(impl for RawDomainVec);
impl_domain_name_as_raw_domain_slice!(impl (const OCTETS: usize) for RawDomainArray<OCTETS>);
impl_domain_name_as_raw_domain_slice!(impl for RawMutDomainSlice<'_>);

impl DomainNameMut for RawDomainVec {
    fn labels_iter_mut<'a>(
        &'a mut self,
    ) -> impl 'a + DoubleEndedIterator<Item = &'a mut RefLabel> + ExactSizeIterator + FusedIterator + Debug
    {
        MutLabelIter::new(self.as_raw_domain_slice_mut())
    }
}

impl DomainNameOwned for RawDomainVec {
    fn insert<L: Label>(&mut self, index: usize, label: L) -> Result<(), InsertError> {
        let label_count = usize::from(self.label_count());
        if index > label_count {
            return Err(InsertError::OutOfBounds);
        } else if (index == label_count) && self.is_fully_qualified() {
            return Err(InsertError::FullyQualified);
        } else if (index < label_count) && label.is_root() {
            return Err(InsertError::NonTrailingRoot);
        } else if would_exceed_max_octets(self, &label) {
            return Err(InsertError::TooManyOctets);
        }

        self.octets
            .reserve(LENGTH_OCTET_WIDTH + label.octets().len());
        // Notes on overflow:
        //
        // Since a domain name cannot exceed 256 bytes, the maximum sum of octet
        // lengths cannot exceed 256 either. Since we bounded the `index` above,
        // and `length_octets.len() <= MAX_LABELS (128)`, this sum will never
        // overflow.
        let octet_insert_index = (index * LENGTH_OCTET_WIDTH)
            + self
                .length_octets_iter()
                .take(index)
                .map(usize::from)
                .sum::<usize>();
        self.octets.splice(
            octet_insert_index..octet_insert_index,
            std::iter::once(label.len()).chain(label.octets().iter().copied()),
        );

        self.debug_assert_invariants();
        Ok(())
    }

    // TODO: Does the compiler optimize the default implementation enough that we don't need this?
    fn push_front<L: Label>(&mut self, label: L) -> Result<(), PushFrontError> {
        if self.octets.first().is_some() && label.is_root() {
            return Err(PushFrontError::NonTrailingRoot);
        } else if would_exceed_max_octets(self, &label) {
            return Err(PushFrontError::TooManyOctets);
        }

        self.octets
            .reserve(LENGTH_OCTET_WIDTH + label.octets().len());
        self.octets.splice(
            0..0,
            std::iter::once(label.len()).chain(label.octets().iter().copied()),
        );

        self.debug_assert_invariants();
        Ok(())
    }

    fn push_back<L: Label>(&mut self, label: L) -> Result<(), PushBackError> {
        if self.is_fully_qualified() {
            return Err(PushBackError::FullyQualified);
        } else if would_exceed_max_octets(self, &label) {
            return Err(PushBackError::TooManyOctets);
        }

        self.octets
            .reserve(LENGTH_OCTET_WIDTH + label.octets().len());
        self.octets.push(label.len());
        self.octets.extend_from_slice(label.octets());

        self.debug_assert_invariants();
        Ok(())
    }

    fn remove(&mut self, index: usize) -> Option<OwnedLabel> {
        // TODO: we iterate over the label count several times in this function,
        //       both through helpers and directly. Check if compiler optimizes
        //       these. Otherwise, improve.
        if index >= usize::from(self.label_count()) {
            return None;
        }

        // Notes on overflow:
        //
        // Since a domain name cannot exceed 256 bytes, the maximum sum of octet
        // lengths cannot exceed 256 either. Since we bounded the `index` above,
        // and `length_octets.len() <= MAX_LABELS (128)`, this sum will never
        // overflow.
        let octet_remove_index = (index * LENGTH_OCTET_WIDTH)
            + self
                .length_octets_iter()
                .take(index)
                .map(usize::from)
                .sum::<usize>();
        let octet_count = *self
            .octets
            .get(octet_remove_index)
            .expect("BUG: the index must be in range");
        let cut_octets = self.octets.drain(
            octet_remove_index
                ..(octet_remove_index + LENGTH_OCTET_WIDTH + usize::from(octet_count)),
        );
        let label = TinyVec::from_iter(cut_octets.skip(LENGTH_OCTET_WIDTH));
        let label = OwnedLabel::from_octets(label);

        debug_assert_eq!(
            label.len(),
            octet_count,
            "the number of bytes removed must match the removed length byte"
        );
        self.debug_assert_invariants();
        Some(label)
    }

    // TODO: Does the compiler optimize the default implementation enough that we don't need this?
    fn pop_front(&mut self) -> Option<OwnedLabel> {
        let octet_count = *self.octets.first()?;
        let cut_octets = self
            .octets
            .drain(..(LENGTH_OCTET_WIDTH + usize::from(octet_count)));
        let label = TinyVec::from_iter(cut_octets.skip(LENGTH_OCTET_WIDTH));
        let label = OwnedLabel::from_octets(label);

        debug_assert_eq!(
            label.len(),
            octet_count,
            "the number of bytes removed must match the removed length byte"
        );
        self.debug_assert_invariants();
        Some(label)
    }
}

#[derive(Debug, Clone, Copy)]
struct LabelIter<'a> {
    domain: RawDomainSlice<'a>,
}

impl<'a> LabelIter<'a> {
    pub fn new(domain_name: RawDomainSlice<'a>) -> Self {
        Self {
            domain: domain_name.and_debug_assert_invariants(),
        }
    }
}

impl<'a> Iterator for LabelIter<'a> {
    type Item = &'a RefLabel;

    fn next(&mut self) -> Option<Self::Item> {
        let (first, remaining) = self.domain.split_first()?;
        self.domain = remaining.and_debug_assert_invariants();
        Some(first)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let size = self.domain.length_octets_iter().count();
        (size, Some(size))
    }

    fn count(self) -> usize {
        usize::from(self.domain.label_count())
    }

    fn last(self) -> Option<Self::Item> {
        self.domain.last()
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        // Unlike generic iterators, it is safe to use `saturating_add()` to
        // increment `n` because if `n` is `usize::MAX`, it is already past the
        // last label of the domain name so it will return `None` either way.
        match self.domain.split_at_checked(n.saturating_add(1)) {
            Some((left, right)) => {
                self.domain = right.and_debug_assert_invariants();
                left.last()
            }
            None => {
                self.domain = RawDomainSlice { octets: &[] }.and_debug_assert_invariants();
                None
            }
        }
    }
}

impl<'a> DoubleEndedIterator for LabelIter<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let (last, remaining) = self.domain.split_last()?;
        self.domain = remaining.and_debug_assert_invariants();
        Some(last)
    }

    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        let label_count = usize::from(self.domain.label_count());
        if n >= label_count {
            self.domain = RawDomainSlice { octets: &[] }.and_debug_assert_invariants();
            None
        } else {
            let (left, right) = self
                .domain
                .split_at(label_count.saturating_sub(n.saturating_add(1)));
            self.domain = left.and_debug_assert_invariants();
            right.first()
        }
    }
}

impl<'a> ExactSizeIterator for LabelIter<'a> {}
impl<'a> FusedIterator for LabelIter<'a> {}

#[derive(Debug)]
struct MutLabelIter<'a> {
    domain: RawMutDomainSlice<'a>,
}

impl<'a> MutLabelIter<'a> {
    pub fn new(domain_name: RawMutDomainSlice<'a>) -> Self {
        Self {
            domain: domain_name.and_debug_assert_invariants(),
        }
    }
}

impl<'a> Iterator for MutLabelIter<'a> {
    type Item = &'a mut RefLabel;

    fn next(&mut self) -> Option<Self::Item> {
        let first = self.domain.split_off_first_mut()?;
        self.domain.debug_assert_invariants();
        Some(first)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let size = usize::from(self.domain.label_count());
        (size, Some(size))
    }

    fn count(self) -> usize {
        usize::from(self.domain.label_count())
    }

    fn last(mut self) -> Option<Self::Item> {
        self.domain.split_off_last_mut()
    }
}

impl<'a> DoubleEndedIterator for MutLabelIter<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let last = self.domain.split_off_last_mut()?;
        self.domain.debug_assert_invariants();
        Some(last)
    }
}

impl<'a> ExactSizeIterator for MutLabelIter<'a> {}
impl<'a> FusedIterator for MutLabelIter<'a> {}

#[derive(Debug, Clone, Copy)]
struct RawSubDomainIter<'a> {
    domain: RawDomainSlice<'a>,
    /// The number of octets that have been taken from the back of the iterator
    /// Need to keep track of this value to know when to end, stop returning
    /// values.
    ///
    /// Using a `u16` to represent this length is ok, because it can be at most
    /// `MAX_OCTETS`, which is less than `u16::MAX`.
    consumed_tail_octets: u16,
}

const_assert!(
    (MAX_OCTETS as usize) <= (u16::MAX as usize),
    "MAX_OCTETS cannot exceed u16::MAX because a u16 is used to represent an octet count in SubDomainIter"
);

impl<'a> RawSubDomainIter<'a> {
    pub fn new(domain_name: RawDomainSlice<'a>) -> Self {
        Self {
            domain: domain_name.and_debug_assert_invariants(),
            consumed_tail_octets: 0,
        }
    }
}

impl<'a> Iterator for RawSubDomainIter<'a> {
    type Item = RawDomainSlice<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if (self.consumed_tail_octets as usize) >= self.domain.octets.len() {
            self.domain = RawDomainSlice { octets: &[] }.and_debug_assert_invariants();
            None
        } else {
            let (_, remaining) = self.domain.split_first()?;
            Some(std::mem::replace(
                &mut self.domain,
                remaining.and_debug_assert_invariants(),
            ))
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let min_size = self
            .domain
            .octets
            .len()
            .saturating_sub(usize::from(self.consumed_tail_octets))
            .div_ceil((MAX_LABEL_OCTETS as usize) + LENGTH_OCTET_WIDTH);
        let max_size = self
            .domain
            .octets
            .len()
            .saturating_sub(usize::from(self.consumed_tail_octets))
            .div_ceil(2);
        (min_size, Some(max_size))
    }

    fn count(self) -> usize {
        usize::from(
            RawDomainSlice {
                octets: &self.domain.octets[..self
                    .domain
                    .octets
                    .len()
                    .saturating_sub(usize::from(self.consumed_tail_octets))],
            }
            .and_debug_assert_invariants()
            .label_count(),
        )
    }

    fn last(mut self) -> Option<Self::Item> {
        self.next_back()
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        let (_, remaining) = self.domain.split_at_checked(n)?;
        if remaining.octets.len() >= self.domain.octets.len() {
            self.domain = RawDomainSlice { octets: &[] }.and_debug_assert_invariants();
            None
        } else {
            self.domain = remaining.and_debug_assert_invariants();
            self.next()
        }
    }
}

impl<'a> DoubleEndedIterator for RawSubDomainIter<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        match (RawDomainSlice {
            octets: &self.domain.octets[..self
                .domain
                .octets
                .len()
                .saturating_sub(usize::from(self.consumed_tail_octets))],
        })
        .and_debug_assert_invariants()
        .last_label_octet_index()
        {
            Some(last_label_octet_index) => {
                let (_, back) = self.domain.split_at(last_label_octet_index);
                self.consumed_tail_octets = back.octet_count();
                Some(back.and_debug_assert_invariants())
            }
            None => {
                self.domain = RawDomainSlice { octets: &[] }.and_debug_assert_invariants();
                None
            }
        }
    }

    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        let label_count = RawDomainSlice {
            octets: &self.domain.octets[..self
                .domain
                .octets
                .len()
                .saturating_sub(usize::from(self.consumed_tail_octets))],
        }
        .and_debug_assert_invariants()
        .label_count();
        if n > usize::from(label_count) {
            self.domain = RawDomainSlice { octets: &[] }.and_debug_assert_invariants();
            None
        } else {
            let (left, right) = self
                .domain
                .split_at(usize::from(label_count).saturating_sub(n.saturating_add(1)));
            self.domain = left.and_debug_assert_invariants();
            Some(right.and_debug_assert_invariants())
        }
    }
}

impl<'a> FusedIterator for RawSubDomainIter<'a> {}

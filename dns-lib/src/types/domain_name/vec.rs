use std::{
    fmt::Debug,
    hash::Hash,
    iter::FusedIterator,
    ops::{Add, Deref, DerefMut},
};

use static_assertions::const_assert;
use tinyvec::{ArrayVec, TinyVec, tiny_vec};

use crate::{
    serde::{
        presentation::{
            errors::TokenError,
            from_presentation::FromPresentation,
            parse_chars::{
                char_token::EscapableChar, escaped_to_escapable::EscapedCharsEnumerateIter,
            },
            to_presentation::ToPresentation,
        },
        wire::{from_wire::FromWire, to_wire::ToWire},
    },
    types::{
        ascii::{AsciiChar, AsciiString, constants::ASCII_PERIOD},
        domain_name::{
            DomainName, DomainNameError, DomainNameMut, LENGTH_OCTET_WIDTH,
            MAX_COMPRESSION_POINTERS, MAX_LABEL_OCTETS, MAX_LABELS, MAX_OCTETS, MakeCanonicalError,
            MakeFullyQualifiedError, RawDomainSlice, RawDomainVec, RawMutDomainSlice,
        },
        label::{Label, RefLabel},
    },
};

/// Assert all invariants required to ensure that the raw parts of a domain name
/// are correct and that it is safe to pass those arguments to
/// `DomainSlice::from_raw_parts()`.
///
/// This function is `const`. This means that it can be called at compile time
/// in a `const` setting where it will cause compilation to fail if the
/// invariants are not met.
///
/// # Panics
///
/// This function will panic if any required invariant to ensure that it is safe
/// to use the arguments with `DomainSlice::from_raw_parts()` are not met.
pub const fn assert_domain_name_invariants(octets: &[u8], length_octets: &[u8]) {
    // Verify global invariants
    // Note that we are allowed to have fewer than  `MIN_OCTETS`
    // octets.
    assert!(
        octets.len() <= MAX_OCTETS as usize,
        "domain name specified must be valid but its total length exceeds MAX_OCTETS",
    );
    assert!(
        length_octets.len() <= MAX_LABELS as usize,
        "domain name specified must be valid but its total label count exceeds MAX_LABELS",
    );
    let mut length_octets_index = 0;
    while length_octets_index < length_octets.len() {
        assert!(
            (length_octets[length_octets_index] as usize) <= (MAX_LABEL_OCTETS as usize),
            "domain name specified must be valid but the length of a label exceeds MAX_LABEL_OCTETS",
        );
        length_octets_index += 1;
    }

    assert!(
        length_octets.is_empty() == octets.is_empty(),
        "domain name octets can only be empty of length octets is also empty",
    );

    // Verify that the length octets actually sum up to the total
    // number of non-length octets in the `octets` field.
    let mut expected_total_octet_len = 0;
    let mut length_octets_index = 0;
    while length_octets_index < length_octets.len() {
        expected_total_octet_len +=
            length_octets[length_octets_index] as usize + LENGTH_OCTET_WIDTH;
        length_octets_index += 1;
    }
    assert!(
        octets.len() == expected_total_octet_len,
        "domain name length octets sum must match the count of non-length octets",
    );

    // Verify that the length octets in `octets` and `length_octets`
    // are the same.
    let mut octets_index = 0;
    let mut length_octets_index = 0;
    while length_octets_index < length_octets.len() {
        assert!(
            octets.len() > octets_index,
            "domain name octets must align with length octets",
        );
        assert!(
            octets[octets_index] == length_octets[length_octets_index],
            "domain name octets must align with length octets",
        );
        octets_index += (length_octets[length_octets_index] as usize) + LENGTH_OCTET_WIDTH;
        length_octets_index += 1;
    }
    assert!(
        octets_index == octets.len(),
        "domain name octets must align with length octets",
    );
    assert!(
        length_octets_index == length_octets.len(),
        "domain name octets must align with length octets",
    );

    // Verify that only the last label can be a root label.
    let mut length_octets_index = 0;
    while length_octets_index < length_octets.len().saturating_sub(1) {
        assert!(
            0 < length_octets[length_octets_index],
            "domain name specified must be valid but a non-terminating label has a length of zero",
        );
        length_octets_index += 1;
    }
}

/// This is a compressible domain name. This should only be used in situations where domain name
/// compression is allowed. In all other cases, use a regular DomainName.
///
/// https://www.rfc-editor.org/rfc/rfc1035
///
/// "Domain names in messages are expressed in terms of a sequence of labels.
/// Each label is represented as a one octet length field followed by that
/// number of octets.  Since every domain name ends with the null label of
/// the root, a domain name is terminated by a length byte of zero.  The
/// high order two bits of every length octet must be zero, and the
/// remaining six bits of the length field limit the label to 63 octets or
/// less."
///
/// "To simplify implementations, the total length of a domain name (i.e.,
/// label octets and label length octets) is restricted to 255 octets or
/// less."
///
/// "Although labels can contain any 8 bit values in octets that make up a
/// label, it is strongly recommended that labels follow the preferred
/// syntax described elsewhere in this memo, which is compatible with
/// existing host naming conventions.  Name servers and resolvers must
/// compare labels in a case-insensitive manner (i.e., A=a), assuming ASCII
/// with zero parity.  Non-alphabetic codes must match exactly."
///
/// https://www.rfc-editor.org/rfc/rfc1034
///
/// "The labels must follow the rules for ARPANET host names.  They must
/// start with a letter, end with a letter or digit, and have as interior
/// characters only letters, digits, and hyphen.  There are also some
/// restrictions on the length.  Labels must be 63 characters or less."
///
/// https://www.rfc-editor.org/rfc/rfc1123#page-72
///
/// This RFC lists a number of the requirements for a DNS system.
///
/// Domain names cannot be compressed: Those not defined in RFC 1035
#[derive(Debug, Clone)]
pub struct DomainVec {
    /// Octets still contains label lengths inline despite `length_octets`
    /// containing all the length octets. This way, `octets` maintains the exact
    /// same layout as the wire format for speedy serialization/deserialization.
    pub(super) octets: Vec<AsciiChar>,
    /// A separate list with all the length octets. This allows for reverse
    /// iteration and keeping track of the number of labels.
    /// A TinyVec with a length of 14 has a size of 24 bytes. This is the same
    /// size as a Vec.
    pub(super) length_octets: TinyVec<[u8; 14]>,
}

#[derive(Debug, Clone, Copy)]
pub struct DomainArray<const OCTETS: usize, const LABELS: usize> {
    /// Octets still contains label lengths inline despite `length_octets`
    /// containing all the length octets. This way, `octets` maintains the exact
    /// same layout as the wire format for speedy serialization/deserialization.
    pub(super) octets: [AsciiChar; OCTETS],
    /// A separate list with all the length octets. This allows for reverse
    /// iteration and keeping track of the number of labels.
    /// A TinyVec with a length of 14 has a size of 24 bytes. This is the same
    /// size as a Vec.
    pub(super) length_octets: [u8; LABELS],
}

#[derive(Debug)]
pub struct MutDomainSlice<'a> {
    /// Octets still contains label lengths inline despite `length_octets`
    /// containing all the length octets. This way, `octets` maintains the exact
    /// same layout as the wire format for speedy serialization/deserialization.
    pub(super) octets: &'a mut [AsciiChar],
    /// A separate list with all the length octets. This allows for reverse
    /// iteration and keeping track of the number of labels.
    pub(super) length_octets: &'a [u8],
}

#[derive(Debug, Copy, Clone)]
pub struct DomainSlice<'a> {
    /// Octets still contains label lengths inline despite `length_octets`
    /// containing all the length octets. This way, `octets` maintains the exact
    /// same layout as the wire format for speedy serialization/deserialization.
    pub(super) octets: &'a [AsciiChar],
    /// A separate list with all the length octets. This allows for reverse
    /// iteration and keeping track of the number of labels.
    pub(super) length_octets: &'a [u8],
}

const_assert!(
    (MAX_LABEL_OCTETS as usize) <= (u8::MAX as usize),
    "MAX_LABEL_OCTETS cannot exceed u8::MAX because if it were greater, we would need to represent them with a different type in the `length_octets` fields"
);

impl DomainVec {
    /// Verify that the `octets` and `length_octets` fields are in agreement and
    /// any other obvious invariant violations.
    pub(super) fn assert_invariants(&self) {
        // Note: this function is not `const` because `TinyVec::as_slice()` is
        //       not currently `const`.
        assert_domain_name_invariants(self.octets.as_slice(), self.length_octets.as_slice());
    }
}

impl<const OCTETS: usize, const LABELS: usize> DomainArray<OCTETS, LABELS> {
    /// Verify that the `octets` and `length_octets` fields are in agreement and
    /// any other obvious invariant violations.
    pub(super) const fn assert_invariants(&self) {
        assert_domain_name_invariants(self.octets.as_slice(), self.length_octets.as_slice());
    }
}

impl DomainSlice<'_> {
    /// Verify that the `octets` and `length_octets` fields are in agreement and
    /// any other obvious invariant violations.
    pub(super) const fn assert_invariants(&self) {
        assert_domain_name_invariants(self.octets, self.length_octets);
    }
}

impl MutDomainSlice<'_> {
    /// Verify that the `octets` and `length_octets` fields are in agreement and
    /// any other obvious invariant violations.
    pub(super) const fn assert_invariants(&self) {
        assert_domain_name_invariants(self.octets, self.length_octets);
    }
}

impl_domain_name!(impl for DomainVec);
impl_domain_name!(impl (const OCTETS: usize, const LABELS: usize) for DomainArray<OCTETS, LABELS>);
impl_domain_name!(impl for DomainSlice<'_>);
impl_domain_name!(impl for MutDomainSlice<'_>);

impl DomainVec {
    pub fn as_domain_slice(&self) -> DomainSlice<'_> {
        DomainSlice {
            octets: &self.octets,
            length_octets: &self.length_octets,
        }
        .and_debug_assert_invariants()
    }

    pub fn as_raw_domain_slice(&self) -> RawDomainSlice<'_> {
        RawDomainSlice {
            octets: &self.octets,
        }
        .and_debug_assert_invariants()
    }

    pub fn as_domain_slice_mut(&mut self) -> MutDomainSlice<'_> {
        MutDomainSlice {
            octets: &mut self.octets,
            length_octets: &self.length_octets,
        }
        .and_debug_assert_invariants()
    }

    pub fn as_raw_domain_slice_mut(&mut self) -> RawMutDomainSlice<'_> {
        RawMutDomainSlice {
            octets: &mut self.octets,
        }
        .and_debug_assert_invariants()
    }

    pub fn to_domain_vec(&self) -> DomainVec {
        self.clone().and_debug_assert_invariants()
    }

    pub fn to_raw_domain_vec(&self) -> RawDomainVec {
        RawDomainVec {
            octets: self.octets.clone(),
        }
        .and_debug_assert_invariants()
    }
}

impl<const OCTETS: usize, const LABELS: usize> DomainArray<OCTETS, LABELS> {
    pub fn as_domain_slice(&self) -> DomainSlice<'_> {
        DomainSlice {
            octets: &self.octets,
            length_octets: &self.length_octets,
        }
        .and_debug_assert_invariants()
    }

    pub fn as_raw_domain_slice(&self) -> RawDomainSlice<'_> {
        RawDomainSlice {
            octets: &self.octets,
        }
        .and_debug_assert_invariants()
    }

    pub fn as_domain_slice_mut(&mut self) -> MutDomainSlice<'_> {
        MutDomainSlice {
            octets: &mut self.octets,
            length_octets: &self.length_octets,
        }
        .and_debug_assert_invariants()
    }

    pub fn as_raw_domain_slice_mut(&mut self) -> RawMutDomainSlice<'_> {
        RawMutDomainSlice {
            octets: &mut self.octets,
        }
        .and_debug_assert_invariants()
    }

    pub fn to_domain_vec(&self) -> DomainVec {
        self.as_domain_slice()
            .to_domain_vec()
            .and_debug_assert_invariants()
    }

    pub fn to_raw_domain_vec(&self) -> RawDomainVec {
        self.as_raw_domain_slice()
            .to_raw_domain_vec()
            .and_debug_assert_invariants()
    }
}

impl<'a> MutDomainSlice<'a> {
    pub fn as_domain_slice(&self) -> DomainSlice<'_> {
        DomainSlice {
            octets: self.octets,
            length_octets: self.length_octets,
        }
        .and_debug_assert_invariants()
    }

    pub fn as_raw_domain_slice(&self) -> RawDomainSlice<'_> {
        RawDomainSlice {
            octets: &self.octets,
        }
        .and_debug_assert_invariants()
    }

    pub fn to_domain_vec(&self) -> DomainVec {
        self.as_domain_slice()
            .to_domain_vec()
            .and_debug_assert_invariants()
    }

    pub fn to_raw_domain_vec(&self) -> RawDomainVec {
        self.as_raw_domain_slice()
            .to_raw_domain_vec()
            .and_debug_assert_invariants()
    }
}

impl<'a> DomainSlice<'a> {
    pub fn as_domain_slice(&self) -> DomainSlice<'a> {
        (*self).and_debug_assert_invariants()
    }

    pub fn as_raw_domain_slice(&self) -> RawDomainSlice<'_> {
        RawDomainSlice {
            octets: self.octets,
        }
        .and_debug_assert_invariants()
    }

    pub fn to_domain_vec(&self) -> DomainVec {
        DomainVec {
            octets: self.octets.to_vec(),
            length_octets: TinyVec::from(self.length_octets),
        }
        .and_debug_assert_invariants()
    }

    pub fn to_raw_domain_vec(&self) -> RawDomainVec {
        self.as_raw_domain_slice()
            .to_raw_domain_vec()
            .and_debug_assert_invariants()
    }
}

impl DomainVec {
    pub fn new_root() -> Self {
        Self {
            octets: vec![0],
            length_octets: tiny_vec![0],
        }
        .and_debug_assert_invariants()
    }

    pub fn new(string: &AsciiString) -> Result<Self, DomainNameError> {
        if string.is_empty() {
            return Err(DomainNameError::EmptyString);
        }
        // As long as there are no escaped characters in the string and the name is fully qualified,
        // we expect the length to just about match the number of characters + 1 for the root label.
        let mut octets = Vec::with_capacity(string.len() + LENGTH_OCTET_WIDTH);
        // The first byte represents the length of the first label.
        octets.push(0);
        let mut length_octets = TinyVec::new();
        let mut length_octet_index = 0;

        for escaped_char_result in
            EscapedCharsEnumerateIter::from(string.iter().copied().enumerate())
        {
            match (escaped_char_result, (octets.len() - length_octet_index)) {
                (Ok((0, EscapableChar::Ascii(ASCII_PERIOD))), _) => {
                    // leading dots are illegal except for the root zone
                    if string.len() > 1 {
                        return Err(DomainNameError::LeadingDot);
                    }

                    length_octets.push(octets[length_octet_index]);
                    break;
                }
                // consecutive dots are never legal
                (Ok((1.., EscapableChar::Ascii(ASCII_PERIOD))), 1) => {
                    return Err(DomainNameError::ConsecutiveDots);
                }
                // a label is found
                (Ok((1.., EscapableChar::Ascii(ASCII_PERIOD))), 2..) => {
                    length_octets.push(octets[length_octet_index]);

                    if octets.len() > MAX_OCTETS as usize {
                        return Err(DomainNameError::LongDomain);
                    }

                    length_octet_index = octets.len();
                    octets.push(0);
                }
                (Ok((_, escapable_char)), _) => {
                    octets.push(escapable_char.into_unescaped_character());
                    octets[length_octet_index] += 1;

                    // TODO: Can we optimize this check? It might be able to do once per label as
                    // long as we still check against the maximum number of octets every time.
                    if u16::from(octets[length_octet_index]) > MAX_LABEL_OCTETS {
                        return Err(DomainNameError::LongLabel);
                    }

                    if octets.len() > MAX_OCTETS as usize {
                        return Err(DomainNameError::LongDomain);
                    }
                }
                (Err(error), _) => return Err(DomainNameError::ParseError(error)),
            }
        }

        if octets.len() >= (length_octet_index + 1) && (octets != [0]) {
            length_octets.push(octets[length_octet_index]);
        }

        octets.shrink_to_fit();
        Ok(Self {
            octets,
            length_octets,
        }
        .and_debug_assert_invariants())
    }

    pub fn from_utf8(string: &str) -> Result<Self, DomainNameError> {
        Self::new(&AsciiString::from_utf8(string)?)
    }

    pub fn from_labels<T: Label>(labels: Vec<T>) -> Result<Self, DomainNameError> {
        if labels.is_empty() {
            return Err(DomainNameError::EmptyString);
        }
        let total_octets = labels.len() + (labels.iter().map(T::len).sum::<u16>() as usize);
        if total_octets > MAX_OCTETS as usize {
            return Err(DomainNameError::LongDomain);
        }
        let mut length_octets = TinyVec::with_capacity(labels.len());
        let mut octets = Vec::with_capacity(total_octets);
        for label in labels {
            let length_octet = label.len() as u8;
            octets.push(length_octet);
            octets.extend(label.octets());
            length_octets.push(length_octet);
        }
        Ok(Self {
            octets,
            length_octets,
        }
        .and_debug_assert_invariants())
    }

    /// Converts this domain into a fully qualified domain. A domain name is
    /// fully qualified if it ends with the root label.
    pub fn make_fully_qualified(&mut self) -> Result<(), MakeFullyQualifiedError> {
        if self.is_fully_qualified() {
            Ok(())
        // aka. Would adding a byte exceed the limit?
        } else if self.octet_count() >= MAX_OCTETS {
            Err(MakeFullyQualifiedError::TooManyOctets)
        } else {
            self.octets.push(0);
            self.length_octets.push(0);
            self.debug_assert_invariants();
            Ok(())
        }
    }

    /// Converts this domain into its canonical form: lowercase and fully
    /// qualified.
    pub fn make_canonical(&mut self) -> Result<(), MakeCanonicalError> {
        match self.make_fully_qualified() {
            Ok(()) => (),
            Err(MakeFullyQualifiedError::TooManyOctets) => {
                return Err(MakeCanonicalError::TooManyOctets);
            }
        }
        self.make_lowercase();
        self.debug_assert_invariants();
        Ok(())
    }

    /// Returns an iterator over the labels of this domain name, starting from
    /// the left-most label, and iterating towards the right-most label (often,
    /// the root label).
    pub fn domain_slices_iter<'a>(
        &'a self,
    ) -> impl 'a + DoubleEndedIterator<Item = DomainSlice<'a>> + ExactSizeIterator + FusedIterator
    {
        DomainSliceIter::new(self.as_domain_slice())
    }

    /// Returns an iterator over the sub-domains that make up this domain name,
    /// starting from the longest sub-domain, and returning shorter domain names
    /// with each iteration by removing the left-most label.
    #[inline]
    pub fn search_domain_iter<'a>(
        &'a self,
    ) -> impl 'a + DoubleEndedIterator<Item = Self> + ExactSizeIterator<Item = Self> {
        SearchDomainIter::new(self)
    }
}

impl<'a> DomainSlice<'a> {
    /// Create a `DomainSlice` from its raw components.
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
    ///
    /// The `length_octets` must contain the length octets that appear in
    /// `octets` in the same order that they appear in `octets`.
    pub const unsafe fn from_raw_parts(octets: &'a [u8], length_octets: &'a [u8]) -> Self {
        DomainSlice {
            octets,
            length_octets,
        }
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
    pub fn split_at(&self, mid: usize) -> (DomainSlice<'a>, DomainSlice<'a>) {
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
    pub fn split_at_checked(&self, mid: usize) -> Option<(DomainSlice<'a>, DomainSlice<'a>)> {
        const_assert!(
            (MAX_LABELS as usize) < usize::MAX,
            "MAX_LABELS must be less than usize::MAX because if it were equal to or larger, the `mid` after the end of `length_octets` could not be represented"
        );
        const_assert!(
            (MAX_OCTETS as usize) < usize::MAX,
            "MAX_OCTETS must be less than usize::MAX because if it were equal to or larger, the `octets_mid` after the end of `octets` could not be represented"
        );

        let (left_length_octets, right_length_octets) = self.length_octets.split_at_checked(mid)?;
        // Notes on overflow:
        //
        // Since a domain name cannot exceed 256 bytes, the maximum sum of octet
        // lengths cannot exceed 256 either. Since we bounded the `mid` above,
        // and `length_octets.len() <= MAX_LABELS (128)`, this sum will never
        // overflow.
        let octets_mid = (mid * LENGTH_OCTET_WIDTH)
            + left_length_octets
                .iter()
                .map(|length_octet| *length_octet as usize)
                .sum::<usize>();
        let (left_octets, right_octets) = self.octets.split_at(octets_mid);

        Some((
            Self {
                octets: left_octets,
                length_octets: left_length_octets,
            }
            .and_debug_assert_invariants(),
            Self {
                octets: right_octets,
                length_octets: right_length_octets,
            }
            .and_debug_assert_invariants(),
        ))
    }

    /// Returns the first label and all the rest of the labels of the domain, or
    /// `None` if it is empty.
    pub fn split_first(&self) -> Option<(&'a RefLabel, DomainSlice<'a>)> {
        const_assert!(
            (MAX_LABEL_OCTETS as usize) <= (usize::MAX - LENGTH_OCTET_WIDTH),
            "MAX_LABEL_OCTETS must be at most `usize::MAX - LENGTH_OCTET_WIDTH` because if it were greater, `+ LENGTH_OCTET_WIDTH` would overflow"
        );

        let (&first_length_octet, remaining_length_octets) = self.length_octets.split_first()?;
        let (first_octets, remaining_octets) = self
            .octets
            .split_at((first_length_octet as usize) + LENGTH_OCTET_WIDTH);

        Some((
            RefLabel::from_octets(&first_octets[LENGTH_OCTET_WIDTH..]),
            Self {
                octets: remaining_octets,
                length_octets: remaining_length_octets,
            }
            .and_debug_assert_invariants(),
        ))
    }

    /// Returns the last label and all the rest of the labels of the domain, or
    /// `None` if it is empty.
    pub fn split_last(&self) -> Option<(&'a RefLabel, DomainSlice<'a>)> {
        let (&last_length_octet, remaining_length_octets) = self.length_octets.split_last()?;
        let (remaining_octets, last_octets) = self
            .octets
            .split_at(self.octets.len() - (last_length_octet as usize) - LENGTH_OCTET_WIDTH);

        Some((
            RefLabel::from_octets(&last_octets[LENGTH_OCTET_WIDTH..]),
            Self {
                octets: remaining_octets,
                length_octets: remaining_length_octets,
            }
            .and_debug_assert_invariants(),
        ))
    }

    /// Gets the `n`th label in the domain or `None` if `index` is out of
    /// bounds.
    pub fn get(&self, index: usize) -> Option<&'a RefLabel> {
        let (leading_length_octets, &[length_octet, ..]) =
            self.length_octets.split_at_checked(index)?
        else {
            return None;
        };

        // Notes on overflow:
        //
        // Since a domain name cannot exceed 256 bytes, the maximum sum of octet
        // lengths cannot exceed 256 either. Since we bounded the `index` above,
        // and `length_octets.len() <= MAX_LABELS (128)`, this sum will never
        // overflow.
        let octets_start = (index * LENGTH_OCTET_WIDTH)
            + leading_length_octets
                .iter()
                .map(|length_octet| *length_octet as usize)
                .sum::<usize>();
        Some(RefLabel::from_octets(
            &self.octets[(octets_start + LENGTH_OCTET_WIDTH)
                ..(octets_start + (length_octet as usize) + LENGTH_OCTET_WIDTH)],
        ))
    }

    /// Returns the first label of the domain, or `None` if it is empty.
    pub fn first(&self) -> Option<&'a RefLabel> {
        let &length_octet = self.length_octets.first()?;
        Some(RefLabel::from_octets(
            &self.octets[LENGTH_OCTET_WIDTH..((length_octet as usize) + LENGTH_OCTET_WIDTH)],
        ))
    }

    /// Returns the last label of the domain, or `None` if it is empty.
    pub fn last(&self) -> Option<&'a RefLabel> {
        let &length_octet = self.length_octets.last()?;
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

    pub(super) fn into_length_octets_iter(
        self,
    ) -> impl DoubleEndedIterator<Item = u8> + ExactSizeIterator + FusedIterator + Debug + Clone
    {
        self.length_octets.into_iter().copied()
    }

    /// Returns an iterator over the labels of this domain name, starting from
    /// the left-most label, and iterating towards the right-most label (often,
    /// the root label).
    pub fn subdomains_iter(
        &self,
    ) -> impl 'a + DoubleEndedIterator<Item = DomainSlice<'a>> + ExactSizeIterator + FusedIterator
    {
        DomainSliceIter::new(*self)
    }
}

impl DomainName for DomainSlice<'_> {
    fn octet_count(&self) -> u16 {
        const_assert!(
            MAX_OCTETS as usize <= u16::MAX as usize,
            "MAX_OCTETS cannot exceed u16::MAX because u16s are used for the octet count"
        );

        self.octets.len() as u16
    }

    fn label_count(&self) -> u16 {
        const_assert!(
            MAX_LABELS as usize <= u16::MAX as usize,
            "MAX_LABELS cannot exceed u16::MAX because u16s are used for the label count"
        );

        self.length_octets.len() as u16
    }

    fn is_root(&self) -> bool {
        self.octets == [0]
    }

    fn is_fully_qualified(&self) -> bool {
        self.length_octets.last().is_some_and(|octet| *octet == 0)
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

    fn length_octets_iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = u8> + ExactSizeIterator + FusedIterator + Debug + Clone
    {
        (*self).into_length_octets_iter()
    }
}

macro_rules! impl_domain_name_as_domain_slice {
    (impl $(( $($impl_generics:tt)+ ))? for $domain_type:ty) => {
        impl $(<$($impl_generics)+>)? $crate::types::domain_name::DomainName for $domain_type {
            fn octet_count(&self) -> u16 {
                self.as_domain_slice().octet_count()
            }

            fn label_count(&self) -> u16 {
                self.as_domain_slice().label_count()
            }

            fn is_root(&self) -> bool {
                self.as_domain_slice().is_root()
            }

            fn is_fully_qualified(&self) -> bool {
                self.as_domain_slice().is_fully_qualified()
            }

            fn first_label<'a>(&self) -> Option<&RefLabel> {
                self.as_domain_slice().first()
            }

            fn last_label<'a>(&self) -> Option<&RefLabel> {
                self.as_domain_slice().last()
            }

            fn labels_iter<'a>(
                &'a self,
            ) -> impl 'a
            + DoubleEndedIterator<Item = &'a RefLabel>
            + ExactSizeIterator
            + FusedIterator
            + Debug
            + Clone {
                self.as_domain_slice().into_labels_iter()
            }

            fn length_octets_iter(
                &self,
            ) -> impl DoubleEndedIterator<Item = u8>
            + ExactSizeIterator
            + FusedIterator
            + Debug
            + Clone {
                self.as_domain_slice().into_length_octets_iter()
            }
        }
    };
}
impl_domain_name_as_domain_slice!(impl for DomainVec);
impl_domain_name_as_domain_slice!(impl (const OCTETS: usize, const LABELS: usize) for DomainArray<OCTETS, LABELS>);
impl_domain_name_as_domain_slice!(impl for MutDomainSlice<'_>);

impl DomainNameMut for MutDomainSlice<'_> {
    fn make_lowercase(&mut self) {
        // Most hardware is very capable of performing ASCII vector operations.
        // So instead of iterating over each label and performing
        // `make_ascii_lowercase()` on each, we can instead replace all bytes
        // with lowercase version very quickly, and then restore the length
        // bytes from the `length_octets` field.
        self.octets.make_ascii_lowercase();
        self.restore_from_length_octets();
        self.debug_assert_invariants();
    }

    fn make_uppercase(&mut self) {
        // Most hardware is very capable of performing ASCII vector operations.
        // So instead of iterating over each label and performing
        // `make_ascii_uppercase()` on each, we can instead replace all bytes
        // with uppercase version very quickly, and then restore the length
        // bytes from the `length_octets` field.
        self.octets.make_ascii_uppercase();
        self.restore_from_length_octets();
        self.debug_assert_invariants();
    }

    fn labels_iter_mut<'a>(
        &'a mut self,
    ) -> impl 'a + DoubleEndedIterator<Item = &'a mut RefLabel> + ExactSizeIterator + FusedIterator + Debug
    {
        MutLabelIter::new(MutDomainSlice {
            octets: self.octets,
            length_octets: self.length_octets,
        })
    }
}

impl DomainNameMut for DomainVec {
    fn make_lowercase(&mut self) {
        self.as_domain_slice_mut().make_lowercase();
        self.debug_assert_invariants();
    }

    fn make_uppercase(&mut self) {
        self.as_domain_slice_mut().make_uppercase();
        self.debug_assert_invariants();
    }

    fn labels_iter_mut<'a>(
        &'a mut self,
    ) -> impl 'a + DoubleEndedIterator<Item = &'a mut RefLabel> + ExactSizeIterator + FusedIterator + Debug
    {
        MutLabelIter::new(MutDomainSlice {
            octets: self.octets.as_mut_slice(),
            length_octets: self.length_octets.as_slice(),
        })
    }
}

impl<const OCTETS: usize, const LABELS: usize> DomainNameMut for DomainArray<OCTETS, LABELS> {
    fn make_lowercase(&mut self) {
        self.as_domain_slice_mut().make_lowercase();
        self.debug_assert_invariants();
    }

    fn make_uppercase(&mut self) {
        self.as_domain_slice_mut().make_uppercase();
        self.debug_assert_invariants();
    }

    fn labels_iter_mut<'a>(
        &'a mut self,
    ) -> impl 'a + DoubleEndedIterator<Item = &'a mut RefLabel> + ExactSizeIterator + FusedIterator + Debug
    {
        MutLabelIter::new(MutDomainSlice {
            octets: self.octets.as_mut_slice(),
            length_octets: self.length_octets.as_slice(),
        })
    }
}

impl<'a> MutDomainSlice<'a> {
    /// Restores the length octets in the `octets` field by copying them from
    /// the `length_octets` field.
    ///
    /// This allows us to perform operations on the `octets` field that assume
    /// the `octets` field is made of ASCII characters and therefore might
    /// corrupt the length octets, and then restore the `octets` field to a
    /// valid state using the `length_octets` field.
    fn restore_from_length_octets(&mut self) {
        let mut index = 0;
        for &length_octet in self.length_octets {
            // TODO: Consider checking the performance of this operation. Would
            //       an unchecked index make sense here? Do we have strong
            //       enough guarantees about the correctness of the length
            //       octets and bounds of the vector?
            self.octets[index] = length_octet;
            index += (length_octet as usize) + LENGTH_OCTET_WIDTH;
        }

        // The invariants may not be upheld before this point in this function.
        // But once the operation is completed, they should be upheld again.
        self.debug_assert_invariants();
    }

    pub fn as_lowercase(&self) -> DomainVec {
        let mut uppercase_domain = self.to_domain_vec();
        uppercase_domain.make_lowercase();
        uppercase_domain.and_debug_assert_invariants()
    }

    pub fn as_uppercase(&self) -> DomainVec {
        let mut uppercase_domain = self.to_domain_vec();
        uppercase_domain.make_uppercase();
        uppercase_domain.and_debug_assert_invariants()
    }

    pub fn as_fully_qualified(&self) -> Result<DomainVec, MakeFullyQualifiedError> {
        if self.is_fully_qualified() {
            Ok(self.to_domain_vec())
        // aka. Would adding a byte exceed the limit?
        } else if self.octets.len() >= MAX_OCTETS as usize {
            Err(MakeFullyQualifiedError::TooManyOctets)
        } else {
            let mut octets = Vec::with_capacity(self.octets.len() + LENGTH_OCTET_WIDTH);
            octets.extend_from_slice(self.octets);
            octets.push(0);

            let mut length_octets =
                TinyVec::with_capacity(self.length_octets.len() + LENGTH_OCTET_WIDTH);
            length_octets.extend_from_slice(self.length_octets);
            length_octets.push(0);

            Ok(DomainVec {
                octets,
                length_octets,
            }
            .and_debug_assert_invariants())
        }
    }

    pub fn as_canonical(&self) -> Result<DomainVec, MakeCanonicalError> {
        let mut domain = match self.as_fully_qualified() {
            Ok(domain) => domain,
            Err(MakeFullyQualifiedError::TooManyOctets) => {
                return Err(MakeCanonicalError::TooManyOctets);
            }
        };
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
    pub fn split_at_mut(&mut self, mid: usize) -> (MutDomainSlice<'_>, MutDomainSlice<'_>) {
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
    ) -> Option<(MutDomainSlice<'_>, MutDomainSlice<'_>)> {
        const_assert!(
            (MAX_LABELS as usize) < usize::MAX,
            "MAX_LABELS must be less than usize::MAX because if it were equal to or larger, the `mid` after the end of `length_octets` could not be represented"
        );
        const_assert!(
            (MAX_OCTETS as usize) < usize::MAX,
            "MAX_OCTETS must be less than usize::MAX because if it were equal to or larger, the `octets_mid` after the end of `octets` could not be represented"
        );

        let (left_length_octets, right_length_octets) = self.length_octets.split_at_checked(mid)?;
        // Notes on overflow:
        //
        // Since a domain name cannot exceed 256 bytes, the maximum sum of octet
        // lengths cannot exceed 256 either. Since we bounded the `mid` above,
        // and `length_octets.len() <= MAX_LABELS (128)`, this sum will never
        // overflow.
        let octets_mid = (mid * LENGTH_OCTET_WIDTH)
            + left_length_octets
                .iter()
                .map(|length_octet| *length_octet as usize)
                .sum::<usize>();
        let (left_octets, right_octets) = self.octets.split_at_mut(octets_mid);

        Some((
            MutDomainSlice {
                octets: left_octets,
                length_octets: left_length_octets,
            }
            .and_debug_assert_invariants(),
            MutDomainSlice {
                octets: right_octets,
                length_octets: right_length_octets,
            }
            .and_debug_assert_invariants(),
        ))
    }

    /// Returns the first label and all the rest of the labels of the domain, or
    /// `None` if it is empty.
    pub fn split_first_mut(&mut self) -> Option<(&mut RefLabel, MutDomainSlice<'_>)> {
        const_assert!(
            (MAX_LABEL_OCTETS as usize) <= (usize::MAX - LENGTH_OCTET_WIDTH),
            "MAX_LABEL_OCTETS must be at most `usize::MAX - LENGTH_OCTET_WIDTH` because if it were greater, `+ LENGTH_OCTET_WIDTH` would overflow"
        );

        let (&first_length_octet, remaining_length_octets) = self.length_octets.split_first()?;
        let (first_octets, remaining_octets) = self
            .octets
            .split_at_mut((first_length_octet as usize) + LENGTH_OCTET_WIDTH);

        Some((
            RefLabel::from_octets_mut(&mut first_octets[LENGTH_OCTET_WIDTH..]),
            MutDomainSlice {
                octets: remaining_octets,
                length_octets: remaining_length_octets,
            }
            .and_debug_assert_invariants(),
        ))
    }

    /// Returns the last label and all the rest of the labels of the domain, or
    /// `None` if it is empty.
    pub fn split_last_mut(&mut self) -> Option<(&mut RefLabel, MutDomainSlice<'_>)> {
        let (&last_length_octet, remaining_length_octets) = self.length_octets.split_last()?;
        let (remaining_octets, last_octets) = self
            .octets
            .split_at_mut(self.octets.len() - (last_length_octet as usize) - LENGTH_OCTET_WIDTH);

        Some((
            RefLabel::from_octets_mut(&mut last_octets[LENGTH_OCTET_WIDTH..]),
            MutDomainSlice {
                octets: remaining_octets,
                length_octets: remaining_length_octets,
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
    pub fn split_off_before(&mut self, mid: usize) -> MutDomainSlice<'a> {
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
    pub fn split_off_after(&mut self, mid: usize) -> MutDomainSlice<'a> {
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
    pub fn split_off_before_checked(&mut self, mid: usize) -> Option<MutDomainSlice<'a>> {
        const_assert!(
            (MAX_LABELS as usize) < usize::MAX,
            "MAX_LABELS must be less than usize::MAX because if it were equal to or larger, the `mid` after the end of `length_octets` could not be represented"
        );
        const_assert!(
            (MAX_OCTETS as usize) < usize::MAX,
            "MAX_OCTETS must be less than usize::MAX because if it were equal to or larger, the `octets_mid` after the end of `octets` could not be represented"
        );

        let right_length_octets = self.length_octets.split_off(..mid)?;
        // Notes on overflow:
        //
        // Since a domain name cannot exceed 256 bytes, the maximum sum of octet
        // lengths cannot exceed 256 either. Since we bounded the `mid` above,
        // and `length_octets.len() <= MAX_LABELS (128)`, this sum will never
        // overflow.
        let octets_mid = self.octets.len()
            - ((mid * LENGTH_OCTET_WIDTH)
                + right_length_octets
                    .iter()
                    .map(|length_octet| *length_octet as usize)
                    .sum::<usize>());
        let right_octets = self
            .octets
            .split_off_mut(octets_mid..)
            .expect("length octets must not sum past the end of the octets buffer");

        Some(
            MutDomainSlice {
                octets: right_octets,
                length_octets: right_length_octets,
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
    pub fn split_off_after_checked(&mut self, mid: usize) -> Option<MutDomainSlice<'a>> {
        const_assert!(
            (MAX_LABELS as usize) < usize::MAX,
            "MAX_LABELS must be less than usize::MAX because if it were equal to or larger, the `mid` after the end of `length_octets` could not be represented"
        );
        const_assert!(
            (MAX_OCTETS as usize) < usize::MAX,
            "MAX_OCTETS must be less than usize::MAX because if it were equal to or larger, the `octets_mid` after the end of `octets` could not be represented"
        );

        let left_length_octets = self.length_octets.split_off(..mid)?;
        // Notes on overflow:
        //
        // Since a domain name cannot exceed 256 bytes, the maximum sum of octet
        // lengths cannot exceed 256 either. Since we bounded the `mid` above,
        // and `length_octets.len() <= MAX_LABELS (128)`, this sum will never
        // overflow.
        let octets_mid = (mid * LENGTH_OCTET_WIDTH)
            + left_length_octets
                .iter()
                .map(|length_octet| *length_octet as usize)
                .sum::<usize>();
        let left_octets = self
            .octets
            .split_off_mut(..octets_mid)
            .expect("length octets must not sum past the end of the octets buffer");

        Some(
            MutDomainSlice {
                octets: left_octets,
                length_octets: left_length_octets,
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

        let &first_length_octet = self.length_octets.split_off_first()?;
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
        let &last_length_octet = self.length_octets.split_off_last()?;
        let last_octets = self
            .octets
            .split_off_mut(
                ..(self.octets.len() - (last_length_octet as usize) - LENGTH_OCTET_WIDTH),
            )
            .expect("the last length octet must not index past the end of the octets buffer");

        Some(RefLabel::from_octets_mut(
            &mut last_octets[LENGTH_OCTET_WIDTH..],
        ))
    }

    /// Gets the `n`th label in the domain or `None` if `index` is out of
    /// bounds.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut RefLabel> {
        let (leading_length_octets, &[length_octet, ..]) =
            self.length_octets.split_at_checked(index)?
        else {
            return None;
        };

        // Notes on overflow:
        //
        // Since a domain name cannot exceed 256 bytes, the maximum sum of octet
        // lengths cannot exceed 256 either. Since we bounded the `index` above,
        // and `length_octets.len() <= MAX_LABELS (128)`, this sum will never
        // overflow.
        let octets_start = (index * LENGTH_OCTET_WIDTH)
            + leading_length_octets
                .iter()
                .map(|length_octet| *length_octet as usize)
                .sum::<usize>();
        Some(RefLabel::from_octets_mut(
            &mut self.octets[(octets_start + LENGTH_OCTET_WIDTH)
                ..(octets_start + (length_octet as usize) + LENGTH_OCTET_WIDTH)],
        ))
    }

    /// Returns the first label of the domain, or `None` if it is empty.
    pub fn first_mut(&mut self) -> Option<&mut RefLabel> {
        let &length_octet = self.length_octets.first()?;
        Some(RefLabel::from_octets_mut(
            &mut self.octets[LENGTH_OCTET_WIDTH..((length_octet as usize) + LENGTH_OCTET_WIDTH)],
        ))
    }

    /// Returns the last label of the domain, or `None` if it is empty.
    pub fn last_mut(&mut self) -> Option<&mut RefLabel> {
        let &length_octet = self.length_octets.last()?;
        let octets_length = self.octets.len();
        Some(RefLabel::from_octets_mut(
            &mut self.octets[(octets_length - (length_octet as usize))..],
        ))
    }
}

impl PartialEq for DomainSlice<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.octets == other.octets
    }
}
impl PartialEq<MutDomainSlice<'_>> for DomainSlice<'_> {
    fn eq(&self, other: &MutDomainSlice<'_>) -> bool {
        self.eq(&other.as_domain_slice())
    }
}
impl PartialEq<DomainVec> for DomainSlice<'_> {
    fn eq(&self, other: &DomainVec) -> bool {
        self.eq(&other.as_domain_slice())
    }
}
impl Eq for DomainSlice<'_> {}
impl Hash for DomainSlice<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.octets.hash(state);
    }
}

impl PartialEq for MutDomainSlice<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.as_domain_slice().eq(&other.as_domain_slice())
    }
}
impl PartialEq<DomainSlice<'_>> for MutDomainSlice<'_> {
    fn eq(&self, other: &DomainSlice<'_>) -> bool {
        self.as_domain_slice().eq(&other.as_domain_slice())
    }
}
impl PartialEq<DomainVec> for MutDomainSlice<'_> {
    fn eq(&self, other: &DomainVec) -> bool {
        self.as_domain_slice().eq(&other.as_domain_slice())
    }
}
impl Eq for MutDomainSlice<'_> {}
impl Hash for MutDomainSlice<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_domain_slice().hash(state);
    }
}

impl PartialEq for DomainVec {
    fn eq(&self, other: &Self) -> bool {
        self.as_domain_slice().eq(&other.as_domain_slice())
    }
}
impl PartialEq<DomainSlice<'_>> for DomainVec {
    fn eq(&self, other: &DomainSlice<'_>) -> bool {
        self.as_domain_slice().eq(&other.as_domain_slice())
    }
}
impl PartialEq<MutDomainSlice<'_>> for DomainVec {
    fn eq(&self, other: &MutDomainSlice<'_>) -> bool {
        self.as_domain_slice().eq(&other.as_domain_slice())
    }
}
impl Eq for DomainVec {}
impl Hash for DomainVec {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_domain_slice().hash(state);
    }
}

impl Add for DomainVec {
    type Output = Result<Self, DomainNameError>;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        if self.is_fully_qualified() {
            // If it is fully qualified, it already ends in a dot.
            // To add a domain to the end, you would need to add a dot between
            // them, resulting in consecutive dots.
            // This might warrant a new error value.
            return Err(DomainNameError::ConsecutiveDots);
        }

        if (self.octet_count() + rhs.octet_count()) > MAX_OCTETS {
            return Err(DomainNameError::LongDomain);
        }

        let mut octets = self.octets.clone();
        octets.extend(rhs.octets);
        let mut length_octets = self.length_octets.clone();
        length_octets.extend(rhs.length_octets);

        Ok(Self {
            octets,
            length_octets,
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct LabelIter<'a> {
    domain: DomainSlice<'a>,
}

impl<'a> LabelIter<'a> {
    pub fn new(domain_name: DomainSlice<'a>) -> Self {
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
        let size = self.domain.length_octets.len();
        (size, Some(size))
    }

    fn count(self) -> usize {
        self.domain.length_octets.len()
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
                self.domain = DomainSlice {
                    octets: &[],
                    length_octets: &[],
                }
                .and_debug_assert_invariants();
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
        if n >= self.domain.length_octets.len() {
            self.domain = DomainSlice {
                octets: &[],
                length_octets: &[],
            }
            .and_debug_assert_invariants();
            None
        } else {
            let (left, right) = self.domain.split_at(
                self.domain
                    .length_octets
                    .len()
                    .saturating_sub(n.saturating_add(1)),
            );
            self.domain = left.and_debug_assert_invariants();
            right.first()
        }
    }
}

impl<'a> ExactSizeIterator for LabelIter<'a> {}
impl<'a> FusedIterator for LabelIter<'a> {}

#[derive(Debug)]
struct MutLabelIter<'a> {
    domain: MutDomainSlice<'a>,
}

impl<'a> MutLabelIter<'a> {
    pub fn new(domain_name: MutDomainSlice<'a>) -> Self {
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
        let size = self.domain.length_octets.len();
        (size, Some(size))
    }

    fn count(self) -> usize {
        self.domain.length_octets.len()
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
struct DomainSliceIter<'a> {
    domain: DomainSlice<'a>,
    /// The number of times a `DomainSlice` has been taken from the back of
    /// the iterator. Need to keep track of this value to know when to end, stop
    /// returning values.
    ///
    /// Using a `u8` to represent this length is ok, because it can be at most
    /// `MAX_LABELS`, which is less than `u8::MAX`.
    consumed_tail: u8,
}

const_assert!(
    (MAX_LABELS as usize) <= (u8::MAX as usize),
    "MAX_LABELS cannot exceed u8::MAX because a u8 is used to represent a label count in SubDomainIter"
);

impl<'a> DomainSliceIter<'a> {
    pub fn new(domain_name: DomainSlice<'a>) -> Self {
        Self {
            domain: domain_name.and_debug_assert_invariants(),
            consumed_tail: 0,
        }
    }
}

impl<'a> Iterator for DomainSliceIter<'a> {
    type Item = DomainSlice<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if (self.consumed_tail as usize) >= self.domain.length_octets.len() {
            self.domain = DomainSlice {
                octets: &[],
                length_octets: &[],
            }
            .and_debug_assert_invariants();
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
        let size = self
            .domain
            .length_octets
            .len()
            .saturating_sub(self.consumed_tail as usize);
        (size, Some(size))
    }

    fn count(self) -> usize {
        self.domain
            .length_octets
            .len()
            .saturating_sub(self.consumed_tail as usize)
    }

    fn last(mut self) -> Option<Self::Item> {
        self.next_back()
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        if ((self.consumed_tail as usize) + n) >= self.domain.length_octets.len() {
            self.domain = DomainSlice {
                octets: &[],
                length_octets: &[],
            }
            .and_debug_assert_invariants();
            None
        } else {
            let (_, remaining) = self.domain.split_at(n);
            self.domain = remaining.and_debug_assert_invariants();
            self.next()
        }
    }
}

impl<'a> DoubleEndedIterator for DomainSliceIter<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if (self.consumed_tail as usize) >= self.domain.length_octets.len() {
            self.domain = DomainSlice {
                octets: &[],
                length_octets: &[],
            }
            .and_debug_assert_invariants();
            None
        } else {
            let (_, back) = self
                .domain
                .split_at(self.domain.length_octets.len() - (self.consumed_tail as usize) - 1);
            self.consumed_tail = self.consumed_tail.saturating_add(1);
            Some(back.and_debug_assert_invariants())
        }
    }

    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        if ((self.consumed_tail as usize) + n) >= self.domain.length_octets.len() {
            self.domain = DomainSlice {
                octets: &[],
                length_octets: &[],
            }
            .and_debug_assert_invariants();
            None
        } else {
            let (_, back) = self
                .domain
                .split_at(self.domain.length_octets.len() - (self.consumed_tail as usize) - n - 1);
            self.consumed_tail = self.consumed_tail.saturating_add(n as u8);
            Some(back)
        }
    }
}

impl<'a> ExactSizeIterator for DomainSliceIter<'a> {}
impl<'a> FusedIterator for DomainSliceIter<'a> {}

#[derive(Debug, Clone, Copy)]
struct SearchDomainIter<'a> {
    name: &'a DomainVec,
    next_octet_index: u8,
    next_length_index: u8,
    last_octet_index: u8,
    last_length_index: u8,
}

impl<'a> SearchDomainIter<'a> {
    pub fn new(domain_name: &'a DomainVec) -> Self {
        domain_name.debug_assert_invariants();
        Self {
            name: domain_name,
            next_octet_index: 0,
            next_length_index: 0,
            last_octet_index: domain_name.octets.len() as u8,
            last_length_index: domain_name.length_octets.len() as u8,
        }
    }
}

impl<'a> Iterator for SearchDomainIter<'a> {
    type Item = DomainVec;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_length_index < self.last_length_index {
            let octet_index = self.next_octet_index;
            let length_octet_index = self.next_length_index;
            self.next_octet_index += self.name.length_octets[length_octet_index as usize] + 1;
            self.next_length_index += 1;
            Some(DomainVec {
                octets: self.name.octets[(octet_index as usize)..].to_vec(),
                length_octets: TinyVec::from(
                    &self.name.length_octets[(length_octet_index as usize)..],
                ),
            })
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let size = (self.last_length_index as usize) - (self.next_length_index as usize);
        (size, Some(size))
    }
}

impl<'a> DoubleEndedIterator for SearchDomainIter<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.next_length_index < self.last_length_index {
            self.last_octet_index -=
                self.name.length_octets[(self.last_length_index as usize) - 1] + 1;
            self.last_length_index -= 1;
            Some(DomainVec {
                octets: self.name.octets[(self.last_octet_index as usize)..].to_vec(),
                length_octets: TinyVec::from(
                    &self.name.length_octets[(self.last_length_index as usize)..],
                ),
            })
        } else {
            None
        }
    }
}

impl<'a> ExactSizeIterator for SearchDomainIter<'a> {}
impl<'a> FusedIterator for SearchDomainIter<'a> {}

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::CompressibleDomainVec {}
    impl Sealed for super::CompressibleSubDomain<'_> {}
    impl Sealed for super::CompressibleMutSubDomain<'_> {}
    impl Sealed for super::IncompressibleDomainVec {}
    impl Sealed for super::IncompressibleSubDomain<'_> {}
    impl Sealed for super::IncompressibleMutSubDomain<'_> {}
}

pub trait DomainNameCompression: sealed::Sealed {}
impl DomainNameCompression for CompressibleDomainVec {}
impl DomainNameCompression for CompressibleSubDomain<'_> {}
impl DomainNameCompression for CompressibleMutSubDomain<'_> {}
impl DomainNameCompression for IncompressibleDomainVec {}
impl DomainNameCompression for IncompressibleSubDomain<'_> {}
impl DomainNameCompression for IncompressibleMutSubDomain<'_> {}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CompressibleDomainVec(pub DomainVec);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CompressibleSubDomain<'a>(pub DomainSlice<'a>);
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct CompressibleMutSubDomain<'a>(pub MutDomainSlice<'a>);
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IncompressibleDomainVec(pub DomainVec);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IncompressibleSubDomain<'a>(pub DomainSlice<'a>);
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct IncompressibleMutSubDomain<'a>(pub MutDomainSlice<'a>);

impl Deref for CompressibleDomainVec {
    type Target = DomainVec;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<'a> Deref for CompressibleSubDomain<'a> {
    type Target = DomainSlice<'a>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<'a> Deref for CompressibleMutSubDomain<'a> {
    type Target = MutDomainSlice<'a>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl Deref for IncompressibleDomainVec {
    type Target = DomainVec;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<'a> Deref for IncompressibleSubDomain<'a> {
    type Target = DomainSlice<'a>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<'a> Deref for IncompressibleMutSubDomain<'a> {
    type Target = MutDomainSlice<'a>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for CompressibleDomainVec {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl<'a> DerefMut for CompressibleSubDomain<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl<'a> DerefMut for CompressibleMutSubDomain<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl DerefMut for IncompressibleDomainVec {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl<'a> DerefMut for IncompressibleSubDomain<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl<'a> DerefMut for IncompressibleMutSubDomain<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl ToWire for CompressibleSubDomain<'_> {
    #[inline]
    fn to_wire_format<'a, 'b>(
        &self,
        wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>,
        compression: &mut Option<crate::types::domain_name::CompressionMap>,
    ) -> Result<(), crate::serde::wire::write_wire::WriteWireError>
    where
        'a: 'b,
    {
        if let Some(compression_map) = compression {
            let mut length_byte_index = 0_usize;
            while length_byte_index < self.octets.len() {
                if let Some(pointer) =
                    compression_map.find_sequence(&self.octets[length_byte_index..])
                {
                    // The pointer cannot make use of the first two bits. These are reserved for
                    // use indicating that this label is a pointer. If they are needed for the
                    // pointer itself, the pointer would be corrupted.
                    //
                    // To solve this issue, we will just not use a pointer if using one would
                    // lead to a corrupted pointer. Easy as that.
                    if (pointer & 0b1100_0000_0000_0000) != 0b0000_0000_0000_0000 {
                        break;
                    }
                    wire.write_bytes(&self.octets[..length_byte_index])?;
                    return pointer.to_wire_format(wire, compression);
                } else {
                    // Don't insert malformed pointers. Otherwise, it might overwrite an
                    // existing well-formed pointer. If we reach an index that would form a
                    // malformed pointer, then none of the pointers after this one will be well
                    // formed.
                    let pointer = wire.current_len() as u16;
                    if ((pointer & 0b1100_0000_0000_0000) != 0b0000_0000_0000_0000)
                        || (self.octets[length_byte_index..] != [0])
                    {
                        break;
                    }
                    length_byte_index += (self.octets[length_byte_index] as usize) + 1;
                }
            }
        }

        wire.write_bytes(&self.octets)
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        self.octets.len() as u16
    }
}
impl ToWire for CompressibleMutSubDomain<'_> {
    #[inline]
    fn to_wire_format<'a, 'b>(
        &self,
        wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>,
        compression: &mut Option<crate::types::domain_name::CompressionMap>,
    ) -> Result<(), crate::serde::wire::write_wire::WriteWireError>
    where
        'a: 'b,
    {
        CompressibleSubDomain(self.as_domain_slice()).to_wire_format(wire, compression)
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        CompressibleSubDomain(self.as_domain_slice()).serial_length()
    }
}
impl ToWire for CompressibleDomainVec {
    #[inline]
    fn to_wire_format<'a, 'b>(
        &self,
        wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>,
        compression: &mut Option<crate::types::domain_name::CompressionMap>,
    ) -> Result<(), crate::serde::wire::write_wire::WriteWireError>
    where
        'a: 'b,
    {
        CompressibleSubDomain(self.as_domain_slice()).to_wire_format(wire, compression)
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        CompressibleSubDomain(self.as_domain_slice()).serial_length()
    }
}
impl ToWire for IncompressibleSubDomain<'_> {
    #[inline]
    fn to_wire_format<'a, 'b>(
        &self,
        wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>,
        _compression: &mut Option<crate::types::domain_name::CompressionMap>,
    ) -> Result<(), crate::serde::wire::write_wire::WriteWireError>
    where
        'a: 'b,
    {
        // Providing a None type compression map to the CompressibleSubDomain
        // disables domain name compression while allowing us to re-use the rest
        // of its implementation.
        CompressibleSubDomain(self.0).to_wire_format(wire, &mut None)
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        CompressibleSubDomain(self.0).serial_length()
    }
}
impl ToWire for IncompressibleMutSubDomain<'_> {
    #[inline]
    fn to_wire_format<'a, 'b>(
        &self,
        wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>,
        compression: &mut Option<crate::types::domain_name::CompressionMap>,
    ) -> Result<(), crate::serde::wire::write_wire::WriteWireError>
    where
        'a: 'b,
    {
        IncompressibleSubDomain(self.as_domain_slice()).to_wire_format(wire, compression)
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        IncompressibleSubDomain(self.as_domain_slice()).serial_length()
    }
}
impl ToWire for IncompressibleDomainVec {
    #[inline]
    fn to_wire_format<'a, 'b>(
        &self,
        wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>,
        compression: &mut Option<crate::types::domain_name::CompressionMap>,
    ) -> Result<(), crate::serde::wire::write_wire::WriteWireError>
    where
        'a: 'b,
    {
        IncompressibleSubDomain(self.as_domain_slice()).to_wire_format(wire, compression)
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        IncompressibleSubDomain(self.as_domain_slice()).serial_length()
    }
}

impl FromWire for CompressibleDomainVec {
    #[inline]
    fn from_wire_format<'a, 'b>(
        wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>,
    ) -> Result<Self, crate::serde::wire::read_wire::ReadWireError>
    where
        Self: Sized,
        'a: 'b,
    {
        let mut pointer_count = 0;
        let mut fully_qualified = false;
        let mut octets = ArrayVec::<[u8; MAX_OCTETS as usize]>::new();
        let mut length_octets = TinyVec::new();

        let mut final_offset = wire.current_offset();

        while !fully_qualified {
            // Peek at the first byte. It is read differently depending on the value.
            let first_byte = u8::from_wire_format(&mut wire.get_as_read_wire(1)?)?;

            match first_byte & 0b1100_0000 {
                0b0000_0000 => {
                    let label_length = first_byte;
                    if (octets.len() + 1 + (label_length as usize)) > MAX_OCTETS as usize {
                        return Err(DomainNameError::LongDomain)?;
                    }

                    octets.extend_from_slice(wire.take((label_length as usize) + 1)?);
                    length_octets.push(label_length);
                    fully_qualified = label_length == 0;
                }
                0b1100_0000 => {
                    pointer_count += 1;
                    if pointer_count > MAX_COMPRESSION_POINTERS {
                        return Err(DomainNameError::TooManyPointers)?;
                    }

                    let pointer_bytes = u16::from_wire_format(wire)?;

                    // The final offset will be determined by the position after the first pointer.
                    // Once all the redirects have been followed, this is where we want our buffer
                    // to return to.
                    if pointer_count == 1 {
                        final_offset = wire.current_offset();
                    }

                    let pointer = pointer_bytes & 0b0011_1111_1111_1111;
                    // The pointer must point backwards in the wire. Forward pointers
                    // are forbidden.
                    if (pointer as usize) > wire.current_offset() {
                        return Err(DomainNameError::ForwardPointers)?;
                    }

                    wire.set_offset(pointer as usize)?;
                }
                _ => {
                    // 0x80 and 0x40 are reserved
                    return Err(DomainNameError::BadRData)?;
                }
            }
        }

        if pointer_count != 0 {
            wire.set_offset(final_offset)?;
        }

        let octets = octets.to_vec();
        Ok(Self(DomainVec {
            octets,
            length_octets,
        }))
    }
}
impl FromWire for IncompressibleDomainVec {
    #[inline]
    fn from_wire_format<'a, 'b>(
        wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>,
    ) -> Result<Self, crate::serde::wire::read_wire::ReadWireError>
    where
        Self: Sized,
        'a: 'b,
    {
        // IncompressibleDomainVec is REQUIRED to decompress domain names if
        // compression was used.
        Ok(Self(CompressibleDomainVec::from_wire_format(wire)?.0))
    }
}

impl ToPresentation for DomainSlice<'_> {
    #[inline]
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        out_buffer.push(self.to_string())
    }
}
impl ToPresentation for MutDomainSlice<'_> {
    #[inline]
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        self.as_domain_slice().to_presentation_format(out_buffer);
    }
}
impl ToPresentation for DomainVec {
    #[inline]
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        self.as_domain_slice().to_presentation_format(out_buffer);
    }
}
impl ToPresentation for CompressibleSubDomain<'_> {
    #[inline]
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        self.as_domain_slice().to_presentation_format(out_buffer);
    }
}
impl ToPresentation for CompressibleMutSubDomain<'_> {
    #[inline]
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        self.as_domain_slice().to_presentation_format(out_buffer);
    }
}
impl ToPresentation for CompressibleDomainVec {
    #[inline]
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        self.as_domain_slice().to_presentation_format(out_buffer);
    }
}
impl ToPresentation for IncompressibleSubDomain<'_> {
    #[inline]
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        self.as_domain_slice().to_presentation_format(out_buffer);
    }
}
impl ToPresentation for IncompressibleMutSubDomain<'_> {
    #[inline]
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        self.as_domain_slice().to_presentation_format(out_buffer);
    }
}
impl ToPresentation for IncompressibleDomainVec {
    #[inline]
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        self.as_domain_slice().to_presentation_format(out_buffer);
    }
}

impl FromPresentation for DomainVec {
    #[inline]
    fn from_token_format<'a, 'b, 'c, 'd>(
        tokens: &'c [&'a str],
    ) -> Result<(Self, &'d [&'a str]), TokenError>
    where
        Self: Sized,
        'a: 'b,
        'c: 'd,
        'c: 'd,
    {
        let (ascii_domain_name, tokens) = AsciiString::from_token_format(tokens)?;
        Ok((Self::new(&ascii_domain_name)?, tokens))
    }
}
impl FromPresentation for CompressibleDomainVec {
    #[inline]
    fn from_token_format<'a, 'b, 'c, 'd>(
        tokens: &'c [&'a str],
    ) -> Result<(Self, &'d [&'a str]), TokenError>
    where
        Self: Sized,
        'a: 'b,
        'c: 'd,
        'c: 'd,
    {
        let (domain, tokens) = DomainVec::from_token_format(tokens)?;
        Ok((Self(domain), tokens))
    }
}
impl FromPresentation for IncompressibleDomainVec {
    #[inline]
    fn from_token_format<'a, 'b, 'c, 'd>(
        tokens: &'c [&'a str],
    ) -> Result<(Self, &'d [&'a str]), TokenError>
    where
        Self: Sized,
        'a: 'b,
        'c: 'd,
        'c: 'd,
    {
        let (domain, tokens) = DomainVec::from_token_format(tokens)?;
        Ok((Self(domain), tokens))
    }
}

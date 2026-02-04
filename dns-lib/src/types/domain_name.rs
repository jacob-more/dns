use std::{
    collections::HashMap,
    error::Error,
    fmt::{Debug, Display},
    hash::Hash,
    iter::FusedIterator,
    marker::PhantomData,
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
                char_token::EscapableChar,
                escaped_to_escapable::{EscapedCharsEnumerateIter, ParseError},
            },
            to_presentation::ToPresentation,
        },
        wire::{from_wire::FromWire, to_wire::ToWire},
    },
    types::{
        ascii::{AsciiError, AsciiString, constants::ASCII_PERIOD},
        label::CaseSensitive,
    },
};

use super::{
    ascii::AsciiChar,
    label::{CaseInsensitive, Label, OwnedLabel, RefLabel, case_sensitivity::CaseSensitivity},
};

/// Maximum number of bytes that can make up a domain name, including the length
/// octets. This limit is specified in
/// [RFC 1035 section 3.3](https://www.rfc-editor.org/rfc/rfc1035#section-3.3).
pub const MAX_OCTETS: u16 = 256;
/// Minimum number of bytes that can make up a domain name, including the length
/// octets. This limit is not specified in the RFCs but is based on the
/// requirement that the wire format of domain names be terminated by a NULL
/// byte (aka. the root label) in
/// [RFC 1035 section 3.3](https://www.rfc-editor.org/rfc/rfc1035#section-3.3).
pub const MIN_OCTETS: u16 = 1;
/// Only one root label is allowed. All others must have a length of at least 1,
/// so the maximum number of length octets is just over half the maximum number
/// of octets. A domain with 129 length octets would require 128 label octets,
/// which exceeds `MAX_OCTETS` and is therefore malformed so the true maximum
/// number of labels is 128.
pub const MAX_LABELS: u16 = MAX_OCTETS.div_ceil(2);
/// Maximum number of bytes that can a single label of a domain name, not
/// including the length octet. This limit is specified in
/// [RFC 1035 section 2.3.4](https://www.rfc-editor.org/rfc/rfc1035#section-2.3.4)
pub const MAX_LABEL_OCTETS: u16 = 63;
/// Minimum number of bytes that can a single label of a domain name, not
/// including the length octet. A label with this length is also known as the
/// root label, and is used to terminate fully qualified domains.
pub const MIN_LABEL_OCTETS: u16 = 0;

/// We have 14 bits for the compression pointer
pub const MAX_COMPRESSION_OFFSET: u16 = u16::MAX >> 2 + 1;
/// This is the maximum number of compression pointers that should occur in a
/// valid message. Each label in a domain name must be at least one octet and be
/// prefixed by a length octet, except for the last label which may have a
/// length of zero. The root label won't be represented by a compression
/// pointer, hence the `- 1` to exclude the root label.
///
/// It is possible to construct a valid message that has more compression
/// pointers than this, and still doesn't loop, by pointing to a previous
/// pointer. This is not something a well written implementation should ever do
/// and is not supported by this implementation.
pub const MAX_COMPRESSION_POINTERS: u16 = MAX_LABELS - 1;

/// The width in bytes of a length octet.
pub const LENGTH_OCTET_WIDTH: usize = 1;

const_assert!(MIN_OCTETS <= MAX_OCTETS);
const_assert!(MAX_LABELS <= MAX_OCTETS);
const_assert!(
    (MAX_LABEL_OCTETS as usize) <= ((MAX_OCTETS as usize) - LENGTH_OCTET_WIDTH),
    "MAX_LABEL_OCTETS must not exceed MAX_OCTETS or else a single label could be larger than the domain name and it cannot be equal since it does not include a length octet"
);
const_assert!(MIN_LABEL_OCTETS <= MAX_LABEL_OCTETS);
const_assert!(MAX_COMPRESSION_POINTERS <= MAX_LABELS);
const_assert!(
    LENGTH_OCTET_WIDTH <= (u32::MAX as usize),
    "LENGTH_OCTET_WIDTH must not exceed u32::MAX or else it cannot be used in the .pow() method"
);
const_assert!(
    (MAX_LABELS as usize) <= 2_usize.pow(u8::BITS * LENGTH_OCTET_WIDTH as u32),
    "MAX_LABELS must not take more bytes than specified by `2_usize.pow(u8::BITS * LENGTH_OCTET_WIDTH as u32)` because LENGTH_OCTET_WIDTH bytes are needed to represent a label count"
);

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum DomainNameError {
    EmptyString,
    Fqdn,
    LongDomain,
    LongLabel,
    LeadingDot,
    ConsecutiveDots,
    InternalRootLabel,
    Buffer,
    TooManyPointers,
    ForwardPointers,
    InvalidPointer,
    BadRData,
    AsciiError(AsciiError),
    ParseError(ParseError),
}

impl Error for DomainNameError {}
impl Display for DomainNameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyString => write!(
                f,
                "Domain Cannot Be Empty: domain name must have at least one byte"
            ),
            Self::Fqdn => write!(
                f,
                "Domain Must Be Fully Qualified: indicates that a domain name does not have a closing dot"
            ),
            Self::LongDomain => write!(f, "Domain Name Exceeded {} Wire-Format Octets", MAX_OCTETS),
            Self::LongLabel => write!(f, "Label Exceeded {} Wire-Format Octets", MAX_LABEL_OCTETS),
            Self::LeadingDot => write!(
                f,
                "Bad Leading Dot: domain name must not begin with a '.' except for in the root zone"
            ),
            Self::ConsecutiveDots => write!(
                f,
                "Two Consecutive Dots: domain name must not contain two consecutive dots '..' unless one of them is escaped"
            ),
            Self::InternalRootLabel => write!(
                f,
                "Internal Root Label: domain name must not a root label unless it is the last label"
            ),
            Self::Buffer => write!(f, "Buffer size too small"),
            Self::TooManyPointers => write!(
                f,
                "Too Many Compression Pointers: the maximum compression pointers permitted is {}",
                MAX_COMPRESSION_POINTERS
            ),
            Self::ForwardPointers => write!(
                f,
                "Forward Pointer: domain name pointers can only point backwards. Cannot point forward in the buffer"
            ),
            Self::InvalidPointer => write!(
                f,
                "Invalid Pointer: domain name pointer cannot use the first two bits. These are reserved"
            ),
            Self::BadRData => write!(f, "Bad RData."),
            Self::AsciiError(error) => write!(f, "{error}"),
            Self::ParseError(error) => write!(f, "{error}"),
        }
    }
}
impl From<AsciiError> for DomainNameError {
    fn from(value: AsciiError) -> Self {
        Self::AsciiError(value)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum MakeCanonicalError {
    #[error("domain name exceeded {} maximum octet count", MAX_OCTETS)]
    TooManyOctets,
}

#[derive(thiserror::Error, Debug)]
pub enum MakeFullyQualifiedError {
    #[error("domain name exceeded {} maximum octet count", MAX_OCTETS)]
    TooManyOctets,
}

pub trait DomainName {
    /// The number of bytes this domain name has in its non-compressed wire
    /// form. This should never exceed `MAX_OCTETS`.
    ///
    /// e.g., The domain name "example.org." has an octet count of 13, including
    /// the root label.
    fn octet_count(&self) -> u16 {
        const_assert!(
            (MAX_OCTETS as usize) <= (u16::MAX as usize),
            "MAX_OCTETS cannot exceed u16::MAX because u16s are used for the octet count"
        );
        const_assert!(
            (LENGTH_OCTET_WIDTH as usize) <= (u16::MAX as usize),
            "LENGTH_OCTET_WIDTH cannot exceed u16::MAX because u16s are used for the octet count"
        );

        self.labels_iter::<CaseInsensitive>()
            .map(|label| label.len() + (LENGTH_OCTET_WIDTH as u16))
            .sum()
    }

    /// The number of labels that this domain name is made up of.
    ///
    /// e.g., The domain name "example.org." has 3 labels, including the root
    /// label.
    fn label_count(&self) -> u16 {
        const_assert!(
            (MAX_LABELS as usize) <= (u16::MAX as usize),
            "MAX_LABELS cannot exceed u16::MAX because u16s are used for the label count"
        );

        u16::try_from(self.labels_iter::<CaseInsensitive>().count())
            .expect("domain names cannot have more than u16:MAX labels")
    }

    /// A domain name is root if and only if it has exactly 1 label and that
    /// label has a length of zero.
    fn is_root(&self) -> bool {
        let mut labels = self.labels_iter::<CaseInsensitive>();
        labels.next().is_some_and(|label| label.is_root()) && labels.next().is_none()
    }

    /// A domain name is lowercase if and only if all non-length ASCII bytes are
    /// not uppercase characters.
    fn is_lowercase(&self) -> bool {
        self.labels_iter::<CaseSensitive>().all(|label| {
            label
                .octets()
                .iter()
                .all(|character| !character.is_ascii_uppercase())
        })
    }

    /// A domain name is uppercase if and only if all non-length ASCII bytes are
    /// not lowercase characters.
    fn is_uppercase(&self) -> bool {
        self.labels_iter::<CaseSensitive>().all(|label| {
            label
                .octets()
                .iter()
                .all(|character| !character.is_ascii_lowercase())
        })
    }

    /// A domain name is fully qualified if and only if it ends with a root
    /// label.
    fn is_fully_qualified(&self) -> bool {
        self.labels_iter::<CaseInsensitive>()
            .next_back()
            .is_some_and(|label| label.is_root())
    }

    /// A domain name is canonical if and only if `is_lowercase()` is true and
    /// `is_fully_qualified()` is true. See those methods for details.
    fn is_canonical(&self) -> bool {
        self.is_fully_qualified() && self.is_lowercase()
    }

    fn first_label<'a, C: 'a + CaseSensitivity>(&self) -> Option<&RefLabel<C>> {
        self.labels_iter().next()
    }

    fn last_label<'a, C: 'a + CaseSensitivity>(&self) -> Option<&RefLabel<C>> {
        self.labels_iter().last()
    }

    /// Returns an iterator over the labels of this domain name, starting from
    /// the left-most label, and iterating towards the right-most label (often,
    /// the root label).
    fn labels_iter<'a, C: 'a + CaseSensitivity>(
        &'a self,
    ) -> impl 'a
    + DoubleEndedIterator<Item = &'a RefLabel<C>>
    + ExactSizeIterator
    + FusedIterator
    + Debug
    + Clone
    + Copy;
}

pub trait DomainNameMut {
    /// Converts this domain to use all lowercase characters.
    fn make_lowercase(&mut self);

    /// Converts this domain to use all uppercase characters.
    fn make_uppercase(&mut self);

    // TODO: Iterator over mutable labels.
    //
    //fn labels_iter_mut<'a, C: 'a + CaseSensitivity>(
    //    &'a self,
    //) -> impl 'a + Iterator<Item = &'a RefLabel<C>> + DoubleEndedIterator + ExactSizeIterator + FusedIterator;
}

pub trait DomainNameCompare<T: ?Sized> {
    /// determines if two sets of labels are identical, ignoring capitalization
    fn eq_ignore_case(&self, other: &T) -> bool;

    /// Checks if this domain is indeed a parent of the `other` domain name. If
    /// `self` and `other` are the same domain name, the result is `true`.
    fn is_parent_of(&self, other: &T) -> bool;

    /// Checks if this domain is indeed a parent of the `other` domain name. If
    /// `self` and `other` are the same domain name, the result is `true`.
    fn is_parent_of_ignore_case(&self, other: &T) -> bool;

    /// Checks if this domain is indeed a child of the `other` domain name. If
    /// `self` and `other` are the same domain name, the result is `true`.
    fn is_child_of(&self, other: &T) -> bool
    where
        T: DomainNameCompare<Self>,
    {
        other.is_parent_of(self)
    }

    /// Checks if this domain is indeed a child of the `other` domain name. If
    /// `self` and `other` are the same domain name, the result is `true`.
    fn is_child_of_ignore_case(&self, other: &T) -> bool
    where
        T: DomainNameCompare<Self>,
    {
        other.is_parent_of_ignore_case(self)
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
pub struct DomainNameVec {
    /// Octets still contains label lengths inline despite `length_octets`
    /// containing all the length octets. This way, `octets` maintains the exact
    /// same layout as the wire format for speedy serialization/deserialization.
    octets: Vec<AsciiChar>,
    /// A separate list with all the length octets. This allows for reverse
    /// iteration and keeping track of the number of labels.
    /// A TinyVec with a length of 14 has a size of 24 bytes. This is the same
    /// size as a Vec.
    length_octets: TinyVec<[u8; 14]>,
}

#[derive(Debug)]
pub struct MutSubDomainName<'a> {
    /// Octets still contains label lengths inline despite `length_octets`
    /// containing all the length octets. This way, `octets` maintains the exact
    /// same layout as the wire format for speedy serialization/deserialization.
    octets: &'a mut [AsciiChar],
    /// A separate list with all the length octets. This allows for reverse
    /// iteration and keeping track of the number of labels.
    length_octets: &'a [u8],
}

#[derive(Debug, Copy, Clone)]
pub struct SubDomainName<'a> {
    /// Octets still contains label lengths inline despite `length_octets`
    /// containing all the length octets. This way, `octets` maintains the exact
    /// same layout as the wire format for speedy serialization/deserialization.
    octets: &'a [AsciiChar],
    /// A separate list with all the length octets. This allows for reverse
    /// iteration and keeping track of the number of labels.
    length_octets: &'a [u8],
}

const_assert!(
    (MAX_LABEL_OCTETS as usize) <= (u8::MAX as usize),
    "MAX_LABEL_OCTETS cannot exceed u8::MAX because if it were greater, we would need to represent them with a different type in the `length_octets` fields"
);

macro_rules! impl_domain_name_assert_invariants {
    ($domain_type:ty) => {
        impl $domain_type {
            /// Verify that the `octets` and `length_octets` fields are in
            /// agreement and any other obvious invariant violations.
            fn assert_invariants(&self) {
                assert!(self.length_octets.iter().rev().skip(1).all(|x| *x != 0));
                assert_eq!(self.length_octets.is_empty(), self.octets.is_empty());

                // Verify that the length octets actually sum up to the total
                // number of non-length octets in the `octets` field.
                let expected_total_octet_len = self
                    .length_octets
                    .iter()
                    .map(|length_octet| *length_octet as usize)
                    .sum::<usize>()
                    + (self.length_octets.len() * LENGTH_OCTET_WIDTH);
                assert_eq!(self.octets.len(), expected_total_octet_len);

                // Verify that the length octets match their counterparts in the
                // `octets` field.
                let mut index = 0;
                for length_octet in self.length_octets.iter() {
                    assert_eq!(self.octets[index], *length_octet);
                    index += (*length_octet as usize) + LENGTH_OCTET_WIDTH;
                }

                // Verify global invariants
                // Note that we are allowed to have fewer than  `MIN_OCTETS`
                // octets.
                assert!(self.octets.len() <= MAX_OCTETS as usize);
                assert!(self.length_octets.len() <= MAX_LABELS as usize);
                assert!(self.length_octets.iter().all(|length_octet| (*length_octet as usize) <= (MAX_LABEL_OCTETS as usize)));
            }

            /// Verify that the `octets` and `length_octets` fields are in
            /// agreement and any other obvious invariant violations.
            ///
            /// This is a no-op when debug_assertions is not enabled.
            fn debug_assert_invariants(&self) {
                // TODO: I call this function all over the place. Make sure to
                //       remove the extra calls or lock them behind an
                //       additional feature flag after things are properly
                //       tested.
                if cfg!(debug_assertions) {
                    self.assert_invariants();
                }
            }

            /// Verify that the `octets` and `length_octets` fields are in agreement and
            /// any other obvious invariant violations. Transfer ownership to make it
            /// easy to call on instantiation.
            ///
            /// This is a no-op when debug_assertions is not enabled.
            fn and_debug_assert_invariants(self) -> Self {
                // TODO: I call this function all over the place. Make sure to
                //       remove the extra calls or lock them behind an
                //       additional feature flag after things are properly
                //       tested.
                self.debug_assert_invariants();
                self
            }
        }
    };
}

impl_domain_name_assert_invariants!(DomainNameVec);
impl_domain_name_assert_invariants!(SubDomainName<'_>);
impl_domain_name_assert_invariants!(MutSubDomainName<'_>);

impl DomainNameVec {
    pub fn as_subdomain(&self) -> SubDomainName<'_> {
        SubDomainName {
            octets: &self.octets,
            length_octets: &self.length_octets,
        }
        .and_debug_assert_invariants()
    }

    pub fn as_subdomain_mut(&mut self) -> MutSubDomainName<'_> {
        MutSubDomainName {
            octets: &mut self.octets,
            length_octets: &self.length_octets,
        }
        .and_debug_assert_invariants()
    }

    pub fn to_domain_vec(&self) -> DomainNameVec {
        self.clone().and_debug_assert_invariants()
    }
}

impl<'a> MutSubDomainName<'a> {
    pub fn as_subdomain(&self) -> SubDomainName<'_> {
        SubDomainName {
            octets: self.octets,
            length_octets: self.length_octets,
        }
        .and_debug_assert_invariants()
    }

    pub fn to_domain_vec(&self) -> DomainNameVec {
        self.as_subdomain()
            .to_domain_vec()
            .and_debug_assert_invariants()
    }
}

impl<'a> SubDomainName<'a> {
    pub fn as_subdomain(&self) -> SubDomainName<'a> {
        (*self).and_debug_assert_invariants()
    }

    pub fn to_domain_vec(&self) -> DomainNameVec {
        DomainNameVec {
            octets: self.octets.to_vec(),
            length_octets: TinyVec::from(self.length_octets),
        }
        .and_debug_assert_invariants()
    }
}

impl DomainNameVec {
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

    pub fn from_labels<C: CaseSensitivity, T: Label<C>>(
        labels: Vec<T>,
    ) -> Result<Self, DomainNameError> {
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

    pub fn from_owned_labels<C: CaseSensitivity>(
        labels: Vec<OwnedLabel<C>>,
    ) -> Result<Self, DomainNameError> {
        if labels.is_empty() {
            return Err(DomainNameError::EmptyString);
        }
        let total_octets =
            labels.len() + (labels.iter().map(OwnedLabel::len).sum::<u16>() as usize);
        if total_octets > MAX_OCTETS as usize {
            return Err(DomainNameError::LongDomain);
        }
        let mut length_octets = TinyVec::with_capacity(labels.len());
        let mut octets = Vec::with_capacity(total_octets);
        for label in labels {
            let length_octet = label.len() as u8;
            octets.push(length_octet);
            octets.extend(label.into_octets());
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
    pub fn subdomains_iter<'a>(
        &'a self,
    ) -> impl 'a + DoubleEndedIterator<Item = SubDomainName<'a>> + ExactSizeIterator + FusedIterator
    {
        SubDomainIter::new(self.as_subdomain())
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

/// Counts the number of expressions and returns the total as a usize during
/// compile time.
macro_rules! count_expressions {
    // This first pattern is an internal implementation detail. It matches any
    // expression and replaces it with the unit type `()`.
    (@replace $_e:expr) => {()};
    ($($expression:expr),* $(,)?) => {<[()]>::len(&[$(count_expressions!(@replace $expression)),*])};
}

/// Sums the results of expressions and returns the total during compile time.
macro_rules! sum_expressions {
    ($int_ty:ty; $($expression:expr),* $(,)?) => {{
        const TOTAL: $int_ty = 0 $( + $expression )*;
        TOTAL
    }};
}

/// Given a set of arrays, concatenates all the arrays into a single array at
/// compile time. The order of elements in the resultant array is the same as
/// the order in which they were specified in the arguments.
///
/// Note that the arrays provided as arguments can be fixed-size arrays
/// (e.g., `[u8; 10]`), references to fixed-size arrays (e.g., `&[u8; 10]`), or
/// const slices (e.g., `&[u8]`).
macro_rules! concat_arrays {
    ($default:expr; $element_ty:ty; $($array:expr),* $(,)?) => {{
        const CONCATENATED_ARRAY_LEN: usize = 0 $( + $array.len() )*;
        const CONCATENATED_ARRAY: [$element_ty; CONCATENATED_ARRAY_LEN] = {
            let mut concatenated_array = [$default; CONCATENATED_ARRAY_LEN];
            let mut concatenated_array_index = 0;
            $(
                let source_array = $array;
                let mut source_index = 0;
                while source_index < source_array.len() {
                    concatenated_array[concatenated_array_index] = source_array[source_index];
                    source_index += 1;
                    concatenated_array_index += 1;
                }
            )*
            concatenated_array
        };
        CONCATENATED_ARRAY
    }};
}

macro_rules! ref_domain {
    // This pattern is exposed, and matches against the user input. It performs
    // the global limit checks that apply to all labels.
    ($($label:expr),+ $(,)?) => {{
        const TOTAL_LABELS: usize = count_expressions!($($label),+);
        const TOTAL_OCTETS: usize = (TOTAL_LABELS * $crate::types::domain_name::LENGTH_OCTET_WIDTH)
            + sum_expressions!(usize; $($label.as_bytes().len()),+);
        const OCTETS_BUFFER: [u8; TOTAL_OCTETS] = concat_arrays!(
            0; u8;
            $(
                [$label.as_bytes().len() as u8],
                $label.as_bytes(),
            )*
        );
        const LENGTH_OCTETS_BUFFER: [u8; TOTAL_LABELS] = [
            $($label.as_bytes().len() as u8),*
        ];

        const fn assert_invariants(octets: &[u8], length_octets: &[u8]) {
            use $crate::types::domain_name::{MAX_OCTETS, MAX_LABELS, MAX_LABEL_OCTETS, LENGTH_OCTET_WIDTH};

            // Verify global invariants
            // Note that we are allowed to have fewer than  `MIN_OCTETS`
            // octets.
            ::std::assert!(
                octets.len() <= MAX_OCTETS as usize,
                "domain name specified must be valid but its total length exceeds MAX_OCTETS",
            );
            ::std::assert!(
                length_octets.len() <= MAX_LABELS as usize,
                "domain name specified must be valid but its total label count exceeds MAX_LABELS",
            );
            let mut length_octets_index = 0;
            while length_octets_index < length_octets.len() {
                ::std::assert!(
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
                expected_total_octet_len += length_octets[length_octets_index] as usize;
                expected_total_octet_len += LENGTH_OCTET_WIDTH;
                length_octets_index += 1;
            }
            ::std::assert!(
                octets.len() == expected_total_octet_len,
                "domain name length octets sum must match the count of non-length octets",
            );

            // Verify that the length octets in `octets` and `length_octets`
            // are the same.
            let mut octets_index = 0;
            let mut length_octets_index = 0;
            while length_octets_index < length_octets.len() {
                ::std::assert!(
                    octets.len() > octets_index,
                    "domain name octets must align with length octets",
                );
                ::std::assert!(
                    octets[octets_index] == length_octets[length_octets_index],
                    "domain name octets must align with length octets",
                );
                octets_index += (length_octets[length_octets_index] as usize) + LENGTH_OCTET_WIDTH;
                length_octets_index += 1;
            }
            ::std::assert!(
                octets_index == octets.len(),
                "domain name octets must align with length octets",
            );
            ::std::assert!(
                length_octets_index == length_octets.len(),
                "domain name octets must align with length octets",
            );

            // Verify that only the last label can be a root label.
            let mut length_octets_index = 0;
            while length_octets_index < length_octets.len().saturating_sub(1) {
                ::std::assert!(
                    0 < length_octets[length_octets_index],
                    "domain name specified must be valid but a non-terminating label has a length of zero",
                );
                length_octets_index += 1;
            }
        }
        const _: () = assert_invariants(&OCTETS_BUFFER, &LENGTH_OCTETS_BUFFER);

        // # Safety
        //
        // This macro works from a list of labels, encoded as bytes (which are
        // allowed to exceed the range of ASCII characters). This ensures that
        // each label is a valid wire encoding.
        //
        // >  - The total number of labels must be less than `MAX_LABELS` (128).
        // >  - The total length of this field cannot exceed `MAX_OCTETS` (256)
        // >    bytes.
        // >  - No single label may exceed a length of `MAX_LABEL_OCTETS` (63)
        // >    bytes (not including the length octet).
        // >  - Only the last label may be a root label.
        // >
        // > The `length_octets` must contain the length octets that appear in
        // > `octets` in the same order that they appear in `octets`.
        //
        // All safety checks are performed by `assert_invariants()`, after all
        // expressions have been evaluated. Any inconsistencies caused by
        // evaluating the expressions more than once will be caught by those
        // checks.
        unsafe {
            $crate::types::domain_name::SubDomainName::from_raw_parts(
                &OCTETS_BUFFER,
                &LENGTH_OCTETS_BUFFER
            )
        }
    }};
}

macro_rules! domain {
    ($($label:expr),+ $(,)?) => {
        ref_domain![$($label),*]
            .to_domain_vec()
            .and_debug_assert_invariants()
    };
}

macro_rules! ref_label {
    ($label:expr, $case_sensitivity:path $(,)?) => {{
        ::static_assertions::const_assert!(
            ($label).as_bytes().len() <= ($crate::types::domain_name::MAX_LABEL_OCTETS as usize),
            "domain name label specified must be valid but it exceeds MAX_LABEL_OCTETS",
        );
        $crate::types::label::RefLabel::<$case_sensitivity>::from_octets($label.as_bytes())
    }};
    ($label:expr $(,)?) => {
        ref_label!($label, $crate::types::label::CaseInsensitive)
    };
}

macro_rules! label {
    ($label:expr$(, $case_sensitivity:path)? $(,)?) => {
        ref_label!($label, $($case_sensitivity)?).as_owned()
    };
}

impl<'a> SubDomainName<'a> {
    /// Create a `SubDomainName` from its raw components.
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
        SubDomainName {
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
    pub fn split_at(&self, mid: usize) -> (SubDomainName<'a>, SubDomainName<'a>) {
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
    pub fn split_at_checked(&self, mid: usize) -> Option<(SubDomainName<'a>, SubDomainName<'a>)> {
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
    pub fn split_first<C: CaseSensitivity>(&self) -> Option<(&'a RefLabel<C>, SubDomainName<'a>)> {
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
    pub fn split_last<C: CaseSensitivity>(&self) -> Option<(&'a RefLabel<C>, SubDomainName<'a>)> {
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
    pub fn get<C: CaseSensitivity>(&self, index: usize) -> Option<&'a RefLabel<C>> {
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
    pub fn first<C: CaseSensitivity>(&self) -> Option<&'a RefLabel<C>> {
        let &length_octet = self.length_octets.first()?;
        Some(RefLabel::from_octets(
            &self.octets[LENGTH_OCTET_WIDTH..((length_octet as usize) + LENGTH_OCTET_WIDTH)],
        ))
    }

    /// Returns the last label of the domain, or `None` if it is empty.
    pub fn last<C: CaseSensitivity>(&self) -> Option<&'a RefLabel<C>> {
        let &length_octet = self.length_octets.last()?;
        Some(RefLabel::from_octets(
            &self.octets[(self.octets.len() - (length_octet as usize))..],
        ))
    }

    fn into_labels_iter<C: 'a + CaseSensitivity>(
        self,
    ) -> impl 'a
    + DoubleEndedIterator<Item = &'a RefLabel<C>>
    + ExactSizeIterator
    + FusedIterator
    + Debug
    + Clone
    + Copy {
        LabelIter::new(self)
    }

    /// Returns an iterator over the labels of this domain name, starting from
    /// the left-most label, and iterating towards the right-most label (often,
    /// the root label).
    pub fn subdomains_iter(
        &self,
    ) -> impl 'a + DoubleEndedIterator<Item = SubDomainName<'a>> + ExactSizeIterator + FusedIterator
    {
        SubDomainIter::new(*self)
    }
}

impl DomainName for SubDomainName<'_> {
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

    fn labels_iter<'a, C: 'a + CaseSensitivity>(
        &'a self,
    ) -> impl 'a
    + DoubleEndedIterator<Item = &'a RefLabel<C>>
    + ExactSizeIterator
    + FusedIterator
    + Debug
    + Clone
    + Copy {
        (*self).into_labels_iter()
    }
}

macro_rules! impl_domain_name_as_subdomain {
    ($domain_type:ty) => {
        impl DomainName for $domain_type {
            fn octet_count(&self) -> u16 {
                self.as_subdomain().octet_count()
            }

            fn label_count(&self) -> u16 {
                self.as_subdomain().label_count()
            }

            fn is_root(&self) -> bool {
                self.as_subdomain().is_root()
            }

            fn is_lowercase(&self) -> bool {
                self.as_subdomain().is_lowercase()
            }

            fn is_uppercase(&self) -> bool {
                self.as_subdomain().is_uppercase()
            }

            fn is_fully_qualified(&self) -> bool {
                self.as_subdomain().is_fully_qualified()
            }

            fn is_canonical(&self) -> bool {
                self.as_subdomain().is_canonical()
            }

            fn first_label<'a, C: 'a + CaseSensitivity>(&self) -> Option<&RefLabel<C>> {
                self.as_subdomain().first()
            }

            fn last_label<'a, C: 'a + CaseSensitivity>(&self) -> Option<&RefLabel<C>> {
                self.as_subdomain().last()
            }

            fn labels_iter<'a, C: 'a + CaseSensitivity>(
                &'a self,
            ) -> impl 'a
            + DoubleEndedIterator<Item = &'a RefLabel<C>>
            + ExactSizeIterator
            + FusedIterator
            + Debug
            + Clone
            + Copy {
                self.as_subdomain().into_labels_iter()
            }
        }
    };
}

impl_domain_name_as_subdomain!(DomainNameVec);
impl_domain_name_as_subdomain!(MutSubDomainName<'_>);

impl DomainNameMut for MutSubDomainName<'_> {
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
}

impl DomainNameMut for DomainNameVec {
    fn make_lowercase(&mut self) {
        self.as_subdomain_mut().make_lowercase();
        self.debug_assert_invariants();
    }

    fn make_uppercase(&mut self) {
        self.as_subdomain_mut().make_uppercase();
        self.debug_assert_invariants();
    }
}

impl MutSubDomainName<'_> {
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

    pub fn as_lowercase(&self) -> DomainNameVec {
        let mut uppercase_domain = self.to_domain_vec();
        uppercase_domain.make_lowercase();
        uppercase_domain.and_debug_assert_invariants()
    }

    pub fn as_uppercase(&self) -> DomainNameVec {
        let mut uppercase_domain = self.to_domain_vec();
        uppercase_domain.make_uppercase();
        uppercase_domain.and_debug_assert_invariants()
    }

    pub fn as_fully_qualified(&self) -> Result<DomainNameVec, MakeFullyQualifiedError> {
        if self.is_fully_qualified() {
            Ok(self.to_domain_vec())
        // aka. Would adding a byte exceed the limit?
        } else if self.octets.len() >= MAX_OCTETS as usize {
            Err(MakeFullyQualifiedError::TooManyOctets)
        } else {
            let mut octets = Vec::with_capacity(self.octets.len() + LENGTH_OCTET_WIDTH);
            octets.extend_from_slice(&self.octets);
            octets.push(0);

            let mut length_octets =
                TinyVec::with_capacity(self.length_octets.len() + LENGTH_OCTET_WIDTH);
            length_octets.extend_from_slice(&self.length_octets);
            length_octets.push(0);

            Ok(DomainNameVec {
                octets,
                length_octets,
            }
            .and_debug_assert_invariants())
        }
    }

    pub fn as_canonical(&self) -> Result<DomainNameVec, MakeCanonicalError> {
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

    /// Returns an iterator over the labels of this domain name, starting from
    /// the left-most label, and iterating towards the right-most label (often,
    /// the root label).
    pub fn subdomains_iter<'a>(
        &'a self,
    ) -> impl 'a + DoubleEndedIterator<Item = SubDomainName<'a>> + ExactSizeIterator + FusedIterator
    {
        SubDomainIter::new(self.as_subdomain())
    }
}

impl PartialEq for SubDomainName<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.octets == other.octets
    }
}
impl PartialEq<MutSubDomainName<'_>> for SubDomainName<'_> {
    fn eq(&self, other: &MutSubDomainName<'_>) -> bool {
        self.eq(&other.as_subdomain())
    }
}
impl PartialEq<DomainNameVec> for SubDomainName<'_> {
    fn eq(&self, other: &DomainNameVec) -> bool {
        self.eq(&other.as_subdomain())
    }
}
impl Eq for SubDomainName<'_> {}
impl Hash for SubDomainName<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.octets.hash(state);
    }
}

impl PartialEq for MutSubDomainName<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.as_subdomain().eq(&other.as_subdomain())
    }
}
impl PartialEq<SubDomainName<'_>> for MutSubDomainName<'_> {
    fn eq(&self, other: &SubDomainName<'_>) -> bool {
        self.as_subdomain().eq(&other.as_subdomain())
    }
}
impl PartialEq<DomainNameVec> for MutSubDomainName<'_> {
    fn eq(&self, other: &DomainNameVec) -> bool {
        self.as_subdomain().eq(&other.as_subdomain())
    }
}
impl Eq for MutSubDomainName<'_> {}
impl Hash for MutSubDomainName<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_subdomain().hash(state);
    }
}

impl PartialEq for DomainNameVec {
    fn eq(&self, other: &Self) -> bool {
        self.as_subdomain().eq(&other.as_subdomain())
    }
}
impl PartialEq<SubDomainName<'_>> for DomainNameVec {
    fn eq(&self, other: &SubDomainName<'_>) -> bool {
        self.as_subdomain().eq(&other.as_subdomain())
    }
}
impl PartialEq<MutSubDomainName<'_>> for DomainNameVec {
    fn eq(&self, other: &MutSubDomainName<'_>) -> bool {
        self.as_subdomain().eq(&other.as_subdomain())
    }
}
impl Eq for DomainNameVec {}
impl Hash for DomainNameVec {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_subdomain().hash(state);
    }
}

impl Display for SubDomainName<'_> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_root() {
            return write!(f, ".");
        }

        let mut labels = self.labels_iter::<CaseInsensitive>();
        if let Some(label) = labels.next() {
            write!(f, "{label}")?;
        }
        for label in labels {
            write!(f, ".{label}")?;
        }
        Ok(())
    }
}
impl Display for MutSubDomainName<'_> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_subdomain())
    }
}
impl Display for DomainNameVec {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_subdomain())
    }
}

impl DomainNameCompare<SubDomainName<'_>> for SubDomainName<'_> {
    fn eq_ignore_case(&self, other: &SubDomainName) -> bool {
        (self.octet_count() == other.octet_count())
            && (self.label_count() == other.label_count())
            && self
                .labels_iter::<CaseInsensitive>()
                .eq(other.labels_iter())
    }

    fn is_parent_of(&self, other: &SubDomainName) -> bool {
        (self.octet_count() <= other.octet_count())
            && (self.label_count() <= other.label_count())
            // Entire parent is contained by the other (other = subdomain)
            && self.labels_iter::<CaseSensitive>()
                .rev()
                .zip(other.labels_iter().rev())
                .all(|(self_label, child_label)| self_label == child_label)
    }

    fn is_parent_of_ignore_case(&self, other: &SubDomainName) -> bool {
        (self.octet_count() <= other.octet_count())
            && (self.label_count() <= other.label_count())
            // Entire parent is contained by the other (other = subdomain)
            && self.labels_iter::<CaseInsensitive>()
                .rev()
                .zip(other.labels_iter().rev())
                .all(|(self_label, child_label)| self_label == child_label)
    }
}
impl DomainNameCompare<MutSubDomainName<'_>> for SubDomainName<'_> {
    fn eq_ignore_case(&self, other: &MutSubDomainName) -> bool {
        self.as_subdomain().eq_ignore_case(&other.as_subdomain())
    }

    fn is_parent_of(&self, other: &MutSubDomainName) -> bool {
        self.as_subdomain().is_parent_of(&other.as_subdomain())
    }

    fn is_parent_of_ignore_case(&self, other: &MutSubDomainName) -> bool {
        self.as_subdomain()
            .is_parent_of_ignore_case(&other.as_subdomain())
    }
}
impl DomainNameCompare<DomainNameVec> for SubDomainName<'_> {
    fn eq_ignore_case(&self, other: &DomainNameVec) -> bool {
        self.as_subdomain().eq_ignore_case(&other.as_subdomain())
    }

    fn is_parent_of(&self, other: &DomainNameVec) -> bool {
        self.as_subdomain().is_parent_of(&other.as_subdomain())
    }

    fn is_parent_of_ignore_case(&self, other: &DomainNameVec) -> bool {
        self.as_subdomain()
            .is_parent_of_ignore_case(&other.as_subdomain())
    }
}
impl DomainNameCompare<MutSubDomainName<'_>> for MutSubDomainName<'_> {
    fn eq_ignore_case(&self, other: &MutSubDomainName) -> bool {
        self.as_subdomain().eq_ignore_case(&other.as_subdomain())
    }

    fn is_parent_of(&self, other: &MutSubDomainName) -> bool {
        self.as_subdomain().is_parent_of(&other.as_subdomain())
    }

    fn is_parent_of_ignore_case(&self, other: &MutSubDomainName) -> bool {
        self.as_subdomain()
            .is_parent_of_ignore_case(&other.as_subdomain())
    }
}
impl DomainNameCompare<SubDomainName<'_>> for MutSubDomainName<'_> {
    fn eq_ignore_case(&self, other: &SubDomainName) -> bool {
        self.as_subdomain().eq_ignore_case(&other.as_subdomain())
    }

    fn is_parent_of(&self, other: &SubDomainName) -> bool {
        self.as_subdomain().is_parent_of(&other.as_subdomain())
    }

    fn is_parent_of_ignore_case(&self, other: &SubDomainName) -> bool {
        self.as_subdomain()
            .is_parent_of_ignore_case(&other.as_subdomain())
    }
}
impl DomainNameCompare<DomainNameVec> for MutSubDomainName<'_> {
    fn eq_ignore_case(&self, other: &DomainNameVec) -> bool {
        self.as_subdomain().eq_ignore_case(&other.as_subdomain())
    }

    fn is_parent_of(&self, other: &DomainNameVec) -> bool {
        self.as_subdomain().is_parent_of(&other.as_subdomain())
    }

    fn is_parent_of_ignore_case(&self, other: &DomainNameVec) -> bool {
        self.as_subdomain()
            .is_parent_of_ignore_case(&other.as_subdomain())
    }
}
impl DomainNameCompare<DomainNameVec> for DomainNameVec {
    fn eq_ignore_case(&self, other: &DomainNameVec) -> bool {
        self.as_subdomain().eq_ignore_case(&other.as_subdomain())
    }

    fn is_parent_of(&self, other: &DomainNameVec) -> bool {
        self.as_subdomain().is_parent_of(&other.as_subdomain())
    }

    fn is_parent_of_ignore_case(&self, other: &DomainNameVec) -> bool {
        self.as_subdomain()
            .is_parent_of_ignore_case(&other.as_subdomain())
    }
}
impl DomainNameCompare<SubDomainName<'_>> for DomainNameVec {
    fn eq_ignore_case(&self, other: &SubDomainName) -> bool {
        self.as_subdomain().eq_ignore_case(&other.as_subdomain())
    }

    fn is_parent_of(&self, other: &SubDomainName) -> bool {
        self.as_subdomain().is_parent_of(&other.as_subdomain())
    }

    fn is_parent_of_ignore_case(&self, other: &SubDomainName) -> bool {
        self.as_subdomain()
            .is_parent_of_ignore_case(&other.as_subdomain())
    }
}
impl DomainNameCompare<MutSubDomainName<'_>> for DomainNameVec {
    fn eq_ignore_case(&self, other: &MutSubDomainName) -> bool {
        self.as_subdomain().eq_ignore_case(&other.as_subdomain())
    }

    fn is_parent_of(&self, other: &MutSubDomainName) -> bool {
        self.as_subdomain().is_parent_of(&other.as_subdomain())
    }

    fn is_parent_of_ignore_case(&self, other: &MutSubDomainName) -> bool {
        self.as_subdomain()
            .is_parent_of_ignore_case(&other.as_subdomain())
    }
}

impl Add for DomainNameVec {
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
struct LabelIter<'a, C: CaseSensitivity> {
    domain: SubDomainName<'a>,
    case: PhantomData<C>,
}

impl<'a, C: CaseSensitivity> LabelIter<'a, C> {
    pub fn new(domain_name: SubDomainName<'a>) -> Self {
        Self {
            domain: domain_name.and_debug_assert_invariants(),
            case: PhantomData,
        }
    }
}

impl<'a, C: 'a + CaseSensitivity> Iterator for LabelIter<'a, C> {
    type Item = &'a RefLabel<C>;

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
                self.domain = SubDomainName {
                    octets: &[],
                    length_octets: &[],
                }
                .and_debug_assert_invariants();
                None
            }
        }
    }
}

impl<'a, C: 'a + CaseSensitivity> DoubleEndedIterator for LabelIter<'a, C> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let (last, remaining) = self.domain.split_last()?;
        self.domain = remaining.and_debug_assert_invariants();
        Some(last)
    }

    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        if n >= self.domain.length_octets.len() {
            self.domain = SubDomainName {
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

impl<'a, C: 'a + CaseSensitivity> ExactSizeIterator for LabelIter<'a, C> {}
impl<'a, C: 'a + CaseSensitivity> FusedIterator for LabelIter<'a, C> {}

#[derive(Debug, Clone, Copy)]
struct SubDomainIter<'a> {
    domain: SubDomainName<'a>,
    /// The number of times a `SubDomainName` has been taken from the back of
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

impl<'a> SubDomainIter<'a> {
    pub fn new(domain_name: SubDomainName<'a>) -> Self {
        Self {
            domain: domain_name.and_debug_assert_invariants(),
            consumed_tail: 0,
        }
    }
}

impl<'a> Iterator for SubDomainIter<'a> {
    type Item = SubDomainName<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if (self.consumed_tail as usize) >= self.domain.length_octets.len() {
            self.domain = SubDomainName {
                octets: &[],
                length_octets: &[],
            }
            .and_debug_assert_invariants();
            None
        } else {
            let (_, remaining) = self.domain.split_first::<CaseInsensitive>()?;
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
            self.domain = SubDomainName {
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

impl<'a> DoubleEndedIterator for SubDomainIter<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if (self.consumed_tail as usize) >= self.domain.length_octets.len() {
            self.domain = SubDomainName {
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
            self.domain = SubDomainName {
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

impl<'a> ExactSizeIterator for SubDomainIter<'a> {}
impl<'a> FusedIterator for SubDomainIter<'a> {}

#[derive(Debug, Clone, Copy)]
struct SearchDomainIter<'a> {
    name: &'a DomainNameVec,
    next_octet_index: u8,
    next_length_index: u8,
    last_octet_index: u8,
    last_length_index: u8,
}

impl<'a> SearchDomainIter<'a> {
    pub fn new(domain_name: &'a DomainNameVec) -> Self {
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
    type Item = DomainNameVec;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_length_index < self.last_length_index {
            let octet_index = self.next_octet_index;
            let length_octet_index = self.next_length_index;
            self.next_octet_index += self.name.length_octets[length_octet_index as usize] + 1;
            self.next_length_index += 1;
            Some(DomainNameVec {
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
            Some(DomainNameVec {
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
pub struct CompressibleDomainVec(pub DomainNameVec);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CompressibleSubDomain<'a>(pub SubDomainName<'a>);
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct CompressibleMutSubDomain<'a>(pub MutSubDomainName<'a>);
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IncompressibleDomainVec(pub DomainNameVec);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IncompressibleSubDomain<'a>(pub SubDomainName<'a>);
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct IncompressibleMutSubDomain<'a>(pub MutSubDomainName<'a>);

impl Deref for CompressibleDomainVec {
    type Target = DomainNameVec;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<'a> Deref for CompressibleSubDomain<'a> {
    type Target = SubDomainName<'a>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<'a> Deref for CompressibleMutSubDomain<'a> {
    type Target = MutSubDomainName<'a>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl Deref for IncompressibleDomainVec {
    type Target = DomainNameVec;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<'a> Deref for IncompressibleSubDomain<'a> {
    type Target = SubDomainName<'a>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<'a> Deref for IncompressibleMutSubDomain<'a> {
    type Target = MutSubDomainName<'a>;

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
        CompressibleSubDomain(self.as_subdomain()).to_wire_format(wire, compression)
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        CompressibleSubDomain(self.as_subdomain()).serial_length()
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
        CompressibleSubDomain(self.as_subdomain()).to_wire_format(wire, compression)
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        CompressibleSubDomain(self.as_subdomain()).serial_length()
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
        IncompressibleSubDomain(self.as_subdomain()).to_wire_format(wire, compression)
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        IncompressibleSubDomain(self.as_subdomain()).serial_length()
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
        IncompressibleSubDomain(self.as_subdomain()).to_wire_format(wire, compression)
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        IncompressibleSubDomain(self.as_subdomain()).serial_length()
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
        Ok(Self(DomainNameVec {
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

impl ToPresentation for SubDomainName<'_> {
    #[inline]
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        out_buffer.push(self.to_string())
    }
}
impl ToPresentation for MutSubDomainName<'_> {
    #[inline]
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        self.as_subdomain().to_presentation_format(out_buffer);
    }
}
impl ToPresentation for DomainNameVec {
    #[inline]
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        self.as_subdomain().to_presentation_format(out_buffer);
    }
}
impl ToPresentation for CompressibleSubDomain<'_> {
    #[inline]
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        self.as_subdomain().to_presentation_format(out_buffer);
    }
}
impl ToPresentation for CompressibleMutSubDomain<'_> {
    #[inline]
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        self.as_subdomain().to_presentation_format(out_buffer);
    }
}
impl ToPresentation for CompressibleDomainVec {
    #[inline]
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        self.as_subdomain().to_presentation_format(out_buffer);
    }
}
impl ToPresentation for IncompressibleSubDomain<'_> {
    #[inline]
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        self.as_subdomain().to_presentation_format(out_buffer);
    }
}
impl ToPresentation for IncompressibleMutSubDomain<'_> {
    #[inline]
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        self.as_subdomain().to_presentation_format(out_buffer);
    }
}
impl ToPresentation for IncompressibleDomainVec {
    #[inline]
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        self.as_subdomain().to_presentation_format(out_buffer);
    }
}

impl FromPresentation for DomainNameVec {
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
        let (domain, tokens) = DomainNameVec::from_token_format(tokens)?;
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
        let (domain, tokens) = DomainNameVec::from_token_format(tokens)?;
        Ok((Self(domain), tokens))
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CompressionMap {
    map: HashMap<Vec<u8>, u16>,
}

impl CompressionMap {
    #[inline]
    pub fn new() -> CompressionMap {
        Self {
            map: HashMap::new(),
        }
    }

    #[inline]
    pub fn insert_sequence(&mut self, domain: &[u8], offset: u16) {
        self.map.entry(domain.to_vec()).or_insert(offset);
    }

    #[inline]
    pub fn find_sequence(&self, domain: &[u8]) -> Option<u16> {
        self.map.get(domain).cloned()
    }
}

#[cfg(test)]
mod test {
    use std::fmt::Debug;

    use concat_idents::concat_idents;
    use rstest::rstest;
    use static_assertions::const_assert;

    use crate::{
        serde::wire::{from_wire::FromWire, to_wire::ToWire},
        types::{
            domain_name::{
                CompressibleDomainVec, DomainName, DomainNameVec, IncompressibleDomainVec,
                SubDomainName,
            },
            label::{CaseSensitive, RefLabel},
        },
    };

    fn domain_labels_iter_verify_extra_properties<'a, 'b>(
        domain_labels: impl DoubleEndedIterator<Item = &'a RefLabel<CaseSensitive>>
        + ExactSizeIterator
        + Debug
        + Clone,
        expected_labels: impl DoubleEndedIterator<Item = &'b RefLabel<CaseSensitive>>
        + ExactSizeIterator
        + Debug
        + Clone,
    ) {
        assert_eq!(expected_labels.clone().next(), domain_labels.clone().next(),);
        assert_eq!(
            expected_labels.clone().next_back(),
            domain_labels.clone().next_back(),
        );
        assert_eq!(expected_labels.clone().last(), domain_labels.clone().last(),);
        assert_eq!(
            domain_labels.clone().last(),
            domain_labels.clone().next_back(),
        );
        assert_eq!(
            expected_labels.clone().count(),
            domain_labels.clone().count(),
        );
        assert_eq!(
            expected_labels.clone().count(),
            domain_labels.clone().size_hint().0,
        );
        assert_eq!(
            expected_labels.clone().count(),
            domain_labels
                .clone()
                .size_hint()
                .1
                .expect("domain label iterators should always have a known length"),
        );
        for n in 0..(domain_labels.size_hint().0 + 1) {
            assert_eq!(expected_labels.clone().nth(n), domain_labels.clone().nth(n));
            assert_eq!(
                expected_labels.clone().rev().nth(n),
                domain_labels.clone().rev().nth(n)
            );
        }
    }

    fn impl_domain_labels_iter_test(
        domain: impl DomainName,
        expected_labels: &[&RefLabel<CaseSensitive>],
    ) {
        let actual_labels = domain.labels_iter().collect::<Vec<_>>();
        assert_eq!(expected_labels, &actual_labels);
    }

    fn impl_domain_labels_reverse_iter_test(
        domain: impl DomainName,
        expected_labels: &[&RefLabel<CaseSensitive>],
    ) {
        let mut expected_labels = expected_labels.to_vec();
        expected_labels.reverse();

        let actual_labels = domain.labels_iter().rev().collect::<Vec<_>>();
        assert_eq!(expected_labels, actual_labels);
    }

    fn impl_domain_labels_nth_iter_test(
        domain: impl DomainName,
        expected_labels: &[&RefLabel<CaseSensitive>],
    ) {
        for n in 0..(expected_labels.len() * 2) {
            let mut expected_labels = expected_labels.into_iter().copied();
            let mut domain_labels = domain.labels_iter::<CaseSensitive>();
            for _ in 0..(expected_labels.len() * 2) {
                assert_eq!(expected_labels.nth(n), domain_labels.nth(n));
                domain_labels_iter_verify_extra_properties(
                    domain_labels.clone(),
                    expected_labels.clone(),
                );
            }
        }
    }

    fn impl_domain_labels_reverse_nth_iter_test(
        domain: impl DomainName,
        expected_labels: &[&RefLabel<CaseSensitive>],
    ) {
        let mut expected_labels = expected_labels.to_vec();
        expected_labels.reverse();

        for n in 0..(expected_labels.len() * 2) {
            let mut expected_labels = expected_labels.iter().copied();
            let mut domain_labels = domain.labels_iter::<CaseSensitive>().rev();
            for _ in 0..(expected_labels.len() * 2) {
                assert_eq!(expected_labels.nth(n), domain_labels.nth(n));
                domain_labels_iter_verify_extra_properties(
                    domain_labels.clone(),
                    expected_labels.clone(),
                );
            }
        }
    }

    fn impl_domain_labels_to_str_test(
        domain: impl DomainName,
        expected_labels: &[&RefLabel<CaseSensitive>],
        expected_label_strings: &[&str],
    ) {
        assert_eq!(
            domain.labels_iter::<CaseSensitive>().count(),
            expected_labels.len()
        );
        assert_eq!(
            domain.labels_iter::<CaseSensitive>().count(),
            expected_label_strings.len()
        );
        for ((domain_label, &expected_label), &expected_label_str) in domain
            .labels_iter::<CaseSensitive>()
            .zip(expected_labels)
            .zip(expected_label_strings)
        {
            assert_eq!(expected_label, domain_label);
            assert_eq!(expected_label_str, domain_label.to_string().as_str());
            assert_eq!(expected_label_str, expected_label.to_string().as_str());
        }
    }

    /// Generates a 3 constants, each with a different prefix:
    ///
    /// - DOMAIN_* - has the specified labels in the form of a `SubDomainName`.
    /// - LABELS_* - has the specified labels in the form of slice of `RefLabel`s.
    /// - LABEL_STRINGS_* - has the specified labels in the form of a slice of string slices.
    ///
    /// Also generates a number of unit tests that use those constants as input.
    macro_rules! generate {
        // The @ident rule is used to generate identifiers. It can't be used in
        // all places like concat_idents!() can but it is a little easier to
        // read in the places where it is used.
        (@ident $prefix:tt, $name:tt $(,)?) => {
            concat_idents!(concatenated_name = $prefix, $name { concatenated_name })
        };
        (@make_constants $([$name:tt; $($label:expr),+ $(,)?]),* $(,)?) => {
            $(
                concat_idents!(name = DOMAIN_, $name {
                    const name: SubDomainName<'static> = ref_domain![$($label),+];
                });
                concat_idents!(name = LABELS_, $name {
                    const name: &[&RefLabel<CaseSensitive>] = &[
                        $(ref_label![$label, CaseSensitive]),+
                    ];
                });
                concat_idents!(name = LABEL_STRINGS_, $name {
                    const name: &[&str] = &[$($label),*];
                });
            )*
        };
        // The @validate_test_cases rule checks the provided test cases to make
        // sure they are well-formed. Otherwise, there are other valid forms
        // that domain names can take but which cannot be sent over a wire so
        // are not tested here.
        (@validate_test_cases $($name:tt),* $(,)?) => {
            $(
                // One easy mistake to make is forgetting to make the test cases
                // all fully qualified domains. This requirement stems from the
                // fact that we perform the serialization / deserialization
                // tests here and all domain names sent over a wire are required
                // to be fully qualified.
                const_assert!(
                    generate!(@ident DOMAIN_, $name).length_octets.len() > 0,
                    "Test cases in this macro MUST only involve fully qualified names"
                );
                const_assert!(
                    generate!(@ident DOMAIN_, $name).length_octets[generate!(@ident DOMAIN_, $name).length_octets.len() - 1] == 0,
                    "Test cases in this macro MUST only involve fully qualified names"
                );
            )*
        };
        (@make_domain_test DOMAIN_ LABELS_; $call:ident [$($name:tt),* $(,)?]) => {
            #[rstest]
            $(
                #[case(
                    generate!(@ident DOMAIN_, $name).to_domain_vec(),
                    generate!(@ident LABELS_, $name),
                )]
                #[case(
                    generate!(@ident DOMAIN_, $name).as_subdomain(),
                    generate!(@ident LABELS_, $name),
                )]
            )*
            fn $call(
                #[case] domain: impl DomainName,
                #[case] expected_labels: &[&RefLabel<CaseSensitive>],
            ) {
                generate!(@ident impl_, $call)(domain, expected_labels);
            }
        };
        (@make_domain_test DOMAIN_ LABELS_ LABEL_STRINGS_; $call:ident [$($name:tt),* $(,)?]) => {
            #[rstest]
            $(
                #[case(
                    generate!(@ident DOMAIN_, $name).to_domain_vec(),
                    generate!(@ident LABELS_, $name),
                    generate!(@ident LABEL_STRINGS_, $name),
                )]
                #[case(
                    generate!(@ident DOMAIN_, $name).as_subdomain(),
                    generate!(@ident LABELS_, $name),
                    generate!(@ident LABEL_STRINGS_, $name),
                )]
            )*
            fn $call(
                #[case] domain: impl DomainName,
                #[case] expected_labels: &[&RefLabel<CaseSensitive>],
                #[case] expected_label_strings: &[&str],
            ) {
                generate!(@ident impl_, $call)(domain, expected_labels, expected_label_strings);
            }
        };
        (@make_tests $($name:tt),* $(,)?) => {
            #[rstest]
            $(
                #[case(CompressibleDomainVec(
                    generate!(@ident DOMAIN_, $name).to_domain_vec()
                ))]
                #[case(IncompressibleDomainVec(
                    generate!(@ident DOMAIN_, $name).to_domain_vec()
                ))]
            )*
            fn circular_serde_sanity_test<T>(#[case] input: T) where T: Debug + ToWire + FromWire + PartialEq {
                crate::serde::wire::circular_test::circular_serde_sanity_test::<T>(input)
            }

            generate!(@make_domain_test DOMAIN_ LABELS_; domain_labels_iter_test [$($name),*]);
            generate!(@make_domain_test DOMAIN_ LABELS_; domain_labels_reverse_iter_test [$($name),*]);
            generate!(@make_domain_test DOMAIN_ LABELS_; domain_labels_nth_iter_test [$($name),*]);
            generate!(@make_domain_test DOMAIN_ LABELS_; domain_labels_reverse_nth_iter_test [$($name),*]);
            generate!(@make_domain_test DOMAIN_ LABELS_ LABEL_STRINGS_; domain_labels_to_str_test [$($name),*]);
        };
        ($([$name:tt; $($label:expr),+ $(,)?]),* $(,)?) => {
            generate!(@make_constants $([$name; $($label),*]),+);
            generate!(@validate_test_cases $($name),+);
            generate!(@make_tests $($name),+);
        };
    }
    generate![
        [ROOT; ""],
        [2_LABELS; "com", ""],
        [3_LABELS; "example", "com", ""],
        [4_LABELS; "www", "example", "com", ""],
        [10_LABELS;
            "label1",
            "next?",
            "another_one",
            "more-labels",
            "com",
            "1",
            "sub",
            "subdomain",
            "org",
            "",
        ],
        [REPETITIVE;
            "www", "example", "org", "www", "example", "org", "www", "example", "org", "www",
            "example", "org", "www", "example", "org", "www", "example", "org", "www", "example",
            "org", "www", "example", "org", "www", "example", "org", "www", "example", "org", "www",
            "example", "org", ""
        ],
        [1_LONG_LABEL; "abcdefghijklmnopqrstuvwxyz ABCDEFGHIJKLMNOPQRSTUVWXYZ 012345678", ""],
        [2_LONG_LABELS;
            "abcdefghijklmnopqrstuvwxyz ABCDEFGHIJKLMNOPQRSTUVWXYZ 012345678",
            "9 `~!@#$%^&*()-_=+[]{};:'\",<>/? abcdefghijklmnopqrstuvwxyz ABCD",
            "",
        ],
    ];

    #[test]
    fn domain_name_to_search_names() {
        let domain_search_name_pairs = vec![
            (".", vec!["."]),
            ("com.", vec!["com.", "."]),
            (
                "www.example.com.",
                vec!["www.example.com.", "example.com.", "com.", "."],
            ),
            (
                "www.example1.com.www.example2.org.",
                vec![
                    "www.example1.com.www.example2.org.",
                    "example1.com.www.example2.org.",
                    "com.www.example2.org.",
                    "www.example2.org.",
                    "example2.org.",
                    "org.",
                    ".",
                ],
            ),
            (
                "www.example1.com.www.example2.com.www.example3.org.",
                vec![
                    "www.example1.com.www.example2.com.www.example3.org.",
                    "example1.com.www.example2.com.www.example3.org.",
                    "com.www.example2.com.www.example3.org.",
                    "www.example2.com.www.example3.org.",
                    "example2.com.www.example3.org.",
                    "com.www.example3.org.",
                    "www.example3.org.",
                    "example3.org.",
                    "org.",
                    ".",
                ],
            ),
        ];

        for (domain, expected_search_names) in &domain_search_name_pairs {
            let domain_name = DomainNameVec::from_utf8(domain).unwrap();
            let expected_search_names = expected_search_names
                .into_iter()
                .map(|search_name| DomainNameVec::from_utf8(search_name).unwrap())
                .collect::<Vec<_>>();
            let actual_search_names = domain_name.search_domain_iter().collect::<Vec<_>>();
            assert_eq!(expected_search_names, actual_search_names);
        }

        for (domain, expected_search_names) in &domain_search_name_pairs {
            let domain_name = DomainNameVec::from_utf8(domain).unwrap();
            let expected_search_names = expected_search_names
                .into_iter()
                .rev()
                .map(|search_name| DomainNameVec::from_utf8(search_name).unwrap())
                .collect::<Vec<_>>();
            let actual_search_names = domain_name.search_domain_iter().rev().collect::<Vec<_>>();
            assert_eq!(expected_search_names, actual_search_names);
        }
    }
}

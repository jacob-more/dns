use std::{
    collections::HashMap,
    error::Error,
    fmt::{Debug, Display},
    hash::Hash,
    iter::FusedIterator,
};

use static_assertions::const_assert;

use crate::{
    serde::presentation::parse_chars::escaped_to_escapable::ParseError,
    types::{
        ascii::AsciiError,
        label::{CaseInsensitive, CaseSensitive, MutLabel},
    },
};

use super::label::{Label, RefLabel};

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

fn impl_is_parent_of<L>(
    child: impl DoubleEndedIterator<Item = L> + ExactSizeIterator,
    parent: impl DoubleEndedIterator<Item = L> + ExactSizeIterator,
) -> bool
where
    L: Label + Eq,
{
    let mut parent_iter = parent.rev();
    child
        .rev()
        .zip(&mut parent_iter)
        .all(|(child_label, parent_label)| child_label == parent_label)
        && parent_iter.next().is_none()
}

pub trait DomainName {
    /// The number of bytes this domain name has in its non-compressed wire
    /// form. This should never exceed `MAX_OCTETS`.
    ///
    /// e.g., The domain name "example.org." has an octet count of 13, including
    /// the root label.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{ref_domain, types::domain_name::DomainName};
    ///
    /// assert_eq!(1, ref_domain!("").octet_count());
    /// assert_eq!(5, ref_domain!("com", "").octet_count());
    /// assert_eq!(13, ref_domain!("example", "com", "").octet_count());
    /// assert_eq!(17, ref_domain!("www", "example", "com", "").octet_count());
    /// ```
    fn octet_count(&self) -> u16 {
        const_assert!(
            (MAX_OCTETS as usize) <= (u16::MAX as usize),
            "MAX_OCTETS cannot exceed u16::MAX because u16s are used for the octet count"
        );
        const_assert!(
            LENGTH_OCTET_WIDTH <= (u16::MAX as usize),
            "LENGTH_OCTET_WIDTH cannot exceed u16::MAX because u16s are used for the octet count"
        );

        self.length_octets_iter()
            .map(|length_octet| (length_octet as u16) + (LENGTH_OCTET_WIDTH as u16))
            .sum()
    }

    /// The number of labels that this domain name is made up of.
    ///
    /// e.g., The domain name "example.org." has 3 labels, including the root
    /// label.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{ref_domain, types::domain_name::DomainName};
    ///
    /// assert_eq!(1, ref_domain!("").label_count());
    /// assert_eq!(2, ref_domain!("com", "").label_count());
    /// assert_eq!(3, ref_domain!("example", "com", "").label_count());
    /// assert_eq!(4, ref_domain!("www", "example", "com", "").label_count());
    /// ```
    fn label_count(&self) -> u16 {
        const_assert!(
            (MAX_LABELS as usize) <= (u16::MAX as usize),
            "MAX_LABELS cannot exceed u16::MAX because u16s are used for the label count"
        );

        u16::try_from(self.length_octets_iter().count())
            .expect("domain names cannot have more than u16:MAX labels")
    }

    /// A domain name is root if and only if it has exactly 1 label and that
    /// label has a length of zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{ref_domain, types::domain_name::DomainName};
    ///
    /// assert!(ref_domain!("").is_root());
    /// assert!(!ref_domain!("com", "").is_root());
    /// assert!(!ref_domain!("example", "com", "").is_root());
    /// ```
    fn is_root(&self) -> bool {
        let mut length_octets = self.length_octets_iter();
        length_octets
            .next()
            .is_some_and(|length_octet| 0 == length_octet)
            && length_octets.next().is_none()
    }

    /// A domain name is lowercase if and only if all non-length ASCII bytes are
    /// not uppercase characters.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{ref_domain, types::domain_name::DomainName};
    ///
    /// assert!(ref_domain!("").is_lowercase());
    /// assert!(ref_domain!("example", "com").is_lowercase());
    /// assert!(ref_domain!("example", "com", "").is_lowercase());
    ///
    /// assert!(!ref_domain!("EXAMPLE", "com").is_lowercase());
    /// assert!(!ref_domain!("EXAMPLE", "COM", "").is_lowercase());
    /// ```
    fn is_lowercase(&self) -> bool {
        self.labels_iter().all(Label::is_lowercase)
    }

    /// A domain name is uppercase if and only if all non-length ASCII bytes are
    /// not lowercase characters.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{ref_domain, types::domain_name::DomainName};
    ///
    /// assert!(ref_domain!("").is_uppercase());
    /// assert!(ref_domain!("EXAMPLE", "COM").is_uppercase());
    /// assert!(ref_domain!("EXAMPLE", "COM", "").is_uppercase());
    ///
    /// assert!(!ref_domain!("EXAMPLE", "com").is_uppercase());
    /// assert!(!ref_domain!("example", "com", "").is_uppercase());
    /// ```
    fn is_uppercase(&self) -> bool {
        self.labels_iter().all(Label::is_uppercase)
    }

    /// A domain name is fully qualified if and only if it ends with a root
    /// label.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{ref_domain, types::domain_name::DomainName};
    ///
    /// assert!(ref_domain!("").is_fully_qualified());
    /// assert!(ref_domain!("example", "com", "").is_fully_qualified());
    /// assert!(ref_domain!("EXAMPLE", "COM", "").is_fully_qualified());
    ///
    /// assert!(!ref_domain!("com").is_fully_qualified());
    /// assert!(!ref_domain!("example", "com").is_fully_qualified());
    /// assert!(!ref_domain!("EXAMPLE", "COM").is_fully_qualified());
    /// ```
    fn is_fully_qualified(&self) -> bool {
        self.last_length_octet()
            .is_some_and(|length_octet| 0 == length_octet)
    }

    /// A domain name is canonical if and only if `is_lowercase()` is true and
    /// `is_fully_qualified()` is true. See those methods for details.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{ref_domain, types::domain_name::DomainName};
    ///
    /// assert!(ref_domain!("").is_canonical());
    /// assert!(ref_domain!("com", "").is_canonical());
    /// assert!(ref_domain!("example", "com", "").is_canonical());
    ///
    /// // The next set of examples fail because they don't end in a root label.
    /// // Some of them also fail because they are not lowercase.
    /// assert!(!ref_domain!("com").is_canonical());
    /// assert!(!ref_domain!("COM").is_canonical());
    /// assert!(!ref_domain!("example", "com").is_canonical());
    /// assert!(!ref_domain!("EXAMPLE", "COM").is_canonical());
    /// assert!(!ref_domain!("example", "COM").is_canonical());
    ///
    /// // The next set of examples fail because they are not lowercase.
    /// assert!(!ref_domain!("EXAMPLE", "COM", "").is_canonical());
    /// assert!(!ref_domain!("example", "COM", "").is_canonical());
    /// ```
    fn is_canonical(&self) -> bool {
        self.is_fully_qualified() && self.is_lowercase()
    }

    /// Checks if this domain is indeed a parent of the `other` domain name. If
    /// `self` and `other` are the same domain name, the result is `true`.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{ref_domain, types::domain_name::DomainName};
    ///
    /// let parent = ref_domain!("example", "com", "");
    /// let upper_parent = ref_domain!("EXAMPLE", "COM", "");
    ///
    /// let child = ref_domain!("www", "example", "com", "");
    /// let upper_child = ref_domain!("WWW", "EXAMPLE", "COM", "");
    ///
    /// assert!(parent.is_parent_of(&parent));
    /// assert!(parent.is_parent_of(&child));
    ///
    /// assert!(!upper_parent.is_parent_of(&parent));
    /// assert!(!upper_parent.is_parent_of(&child));
    ///
    /// assert!(upper_parent.is_parent_of(&upper_parent));
    /// assert!(upper_parent.is_parent_of(&upper_child));
    ///
    /// assert!(!child.is_parent_of(&parent));
    /// assert!(!child.is_parent_of(&upper_parent));
    /// ```
    fn is_parent_of<D: DomainName>(&self, other: &D) -> bool {
        impl_is_parent_of(
            other.labels_iter().map(CaseSensitive),
            self.labels_iter().map(CaseSensitive),
        )
    }

    /// Checks if this domain is indeed a parent of the `other` domain name. If
    /// `self` and `other` are the same domain name, the result is `true`.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{ref_domain, types::domain_name::DomainName};
    ///
    /// let parent = ref_domain!("example", "com", "");
    /// let upper_parent = ref_domain!("EXAMPLE", "COM", "");
    ///
    /// let child = ref_domain!("www", "example", "com", "");
    /// let upper_child = ref_domain!("WWW", "EXAMPLE", "COM", "");
    ///
    /// assert!(parent.is_parent_of_ignore_case(&parent));
    /// assert!(parent.is_parent_of_ignore_case(&child));
    ///
    /// assert!(upper_parent.is_parent_of_ignore_case(&parent));
    /// assert!(upper_parent.is_parent_of_ignore_case(&child));
    ///
    /// assert!(upper_parent.is_parent_of_ignore_case(&upper_parent));
    /// assert!(upper_parent.is_parent_of_ignore_case(&upper_child));
    ///
    /// assert!(!child.is_parent_of_ignore_case(&parent));
    /// assert!(!child.is_parent_of_ignore_case(&upper_parent));
    /// ```
    fn is_parent_of_ignore_case<D: DomainName>(&self, other: &D) -> bool {
        impl_is_parent_of(
            other.labels_iter().map(CaseInsensitive),
            self.labels_iter().map(CaseInsensitive),
        )
    }

    /// Checks if this domain is indeed a child of the `other` domain name. If
    /// `self` and `other` are the same domain name, the result is `true`.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{ref_domain, types::domain_name::DomainName};
    ///
    /// let parent = ref_domain!("example", "com", "");
    /// let upper_parent = ref_domain!("EXAMPLE", "COM", "");
    ///
    /// let child = ref_domain!("www", "example", "com", "");
    /// let upper_child = ref_domain!("WWW", "EXAMPLE", "COM", "");
    ///
    /// assert!(parent.is_child_of(&parent));
    /// assert!(child.is_child_of(&parent));
    ///
    /// assert!(!parent.is_child_of(&upper_parent));
    /// assert!(!child.is_child_of(&upper_parent));
    ///
    /// assert!(upper_parent.is_child_of(&upper_parent));
    /// assert!(upper_child.is_child_of(&upper_parent));
    ///
    /// assert!(!parent.is_child_of(&child));
    /// assert!(!upper_parent.is_child_of(&child));
    /// ```
    fn is_child_of<D: DomainName>(&self, other: &D) -> bool {
        impl_is_parent_of(
            self.labels_iter().map(CaseSensitive),
            other.labels_iter().map(CaseSensitive),
        )
    }

    /// Checks if this domain is indeed a child of the `other` domain name. If
    /// `self` and `other` are the same domain name, the result is `true`.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{ref_domain, types::domain_name::DomainName};
    ///
    /// let parent = ref_domain!("example", "com", "");
    /// let upper_parent = ref_domain!("EXAMPLE", "COM", "");
    ///
    /// let child = ref_domain!("www", "example", "com", "");
    /// let upper_child = ref_domain!("WWW", "EXAMPLE", "COM", "");
    ///
    /// assert!(parent.is_child_of_ignore_case(&parent));
    /// assert!(child.is_child_of_ignore_case(&parent));
    ///
    /// assert!(parent.is_child_of_ignore_case(&upper_parent));
    /// assert!(child.is_child_of_ignore_case(&upper_parent));
    ///
    /// assert!(upper_parent.is_child_of_ignore_case(&upper_parent));
    /// assert!(upper_child.is_child_of_ignore_case(&upper_parent));
    ///
    /// assert!(!parent.is_child_of_ignore_case(&child));
    /// assert!(!upper_parent.is_child_of_ignore_case(&child));
    /// ```
    fn is_child_of_ignore_case<D: DomainName>(&self, other: &D) -> bool {
        impl_is_parent_of(
            self.labels_iter().map(CaseInsensitive),
            other.labels_iter().map(CaseInsensitive),
        )
    }

    /// determines if two sets of labels are identical, ignoring capitalization
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{ref_domain, types::domain_name::DomainName};
    ///
    /// let lower_example_com_root = ref_domain!("example", "com", "");
    /// let upper_example_com_root = ref_domain!("EXAMPLE", "COM", "");
    /// let mixed_example_com_root = ref_domain!("EXAMPLE", "com", "");
    ///
    /// let lower_www_example_com_root = ref_domain!("www", "example", "com", "");
    /// let upper_www_example_com_root = ref_domain!("WWW", "EXAMPLE", "COM", "");
    /// let mixed_www_example_com_root = ref_domain!("www", "EXAMPLE", "com", "");
    ///
    /// assert!(lower_example_com_root.eq_ignore_case(&lower_example_com_root));
    /// assert!(lower_example_com_root.eq_ignore_case(&upper_example_com_root));
    /// assert!(lower_example_com_root.eq_ignore_case(&mixed_example_com_root));
    ///
    /// assert!(!lower_example_com_root.eq_ignore_case(&lower_www_example_com_root));
    /// assert!(!lower_example_com_root.eq_ignore_case(&upper_www_example_com_root));
    /// assert!(!lower_example_com_root.eq_ignore_case(&mixed_www_example_com_root));
    /// ```
    fn eq_ignore_case<D: DomainName>(&self, other: &D) -> bool {
        self.labels_iter()
            .map(CaseInsensitive)
            .eq(other.labels_iter().map(CaseInsensitive))
    }

    /// Get the label at the specified index. The left-most label is index 0.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{ref_domain, ref_label, types::{domain_name::DomainName, label::CaseInsensitive}};
    ///
    /// assert_eq!(
    ///     ref_domain!("example", "com", "").get_label(0).map(CaseInsensitive),
    ///     Some(CaseInsensitive(ref_label!("example"))),
    /// );
    /// assert_eq!(
    ///     ref_domain!("example", "com", "").get_label(1).map(CaseInsensitive),
    ///     Some(CaseInsensitive(ref_label!("com"))),
    /// );
    /// assert_eq!(
    ///     ref_domain!("example", "com", "").get_label(2).map(CaseInsensitive),
    ///     Some(CaseInsensitive(ref_label!(""))),
    /// );
    /// assert_eq!(
    ///     ref_domain!("example", "com", "").get_label(3).map(CaseInsensitive),
    ///     None,
    /// );
    /// ```
    fn get_label(&self, index: usize) -> Option<&RefLabel> {
        self.labels_iter().nth(index)
    }

    /// Get the first (left-most) label. This is equivalent to getting the label
    /// at index 0.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{ref_domain, ref_label, types::{domain_name::DomainName, label::CaseInsensitive}};
    ///
    /// assert_eq!(
    ///     ref_domain!("").first_label().map(CaseInsensitive),
    ///     Some(CaseInsensitive(ref_label!(""))),
    /// );
    /// assert_eq!(
    ///     ref_domain!("com", "").first_label().map(CaseInsensitive),
    ///     Some(CaseInsensitive(ref_label!("com"))),
    /// );
    /// assert_eq!(
    ///     ref_domain!("example", "com", "").first_label().map(CaseInsensitive),
    ///     Some(CaseInsensitive(ref_label!("example"))),
    /// );
    /// ```
    fn first_label(&self) -> Option<&RefLabel> {
        self.labels_iter().next()
    }

    /// Get the last (right-most) label. This is equivalent to getting the label
    /// at the last index.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{ref_domain, ref_label, types::{domain_name::DomainName, label::CaseInsensitive}};
    ///
    /// assert_eq!(
    ///     ref_domain!("").last_label().map(CaseInsensitive),
    ///     Some(CaseInsensitive(ref_label!(""))),
    /// );
    /// assert_eq!(
    ///     ref_domain!("com", "").last_label().map(CaseInsensitive),
    ///     Some(CaseInsensitive(ref_label!(""))),
    /// );
    /// assert_eq!(
    ///     ref_domain!("example", "com", "").last_label().map(CaseInsensitive),
    ///     Some(CaseInsensitive(ref_label!(""))),
    /// );
    /// ```
    fn last_label(&self) -> Option<&RefLabel> {
        self.labels_iter().next_back()
    }

    /// Get the length of the label at the specified index. The left-most label
    /// is index 0.
    ///
    /// This should always be equivalent to getting label at the specified
    /// index, and then getting the length of that label.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{ref_domain, types::domain_name::DomainName};
    ///
    /// assert_eq!(ref_domain!("example", "com", "").get_length_octet(0), Some(7));
    /// assert_eq!(ref_domain!("example", "com", "").get_length_octet(1), Some(3));
    /// assert_eq!(ref_domain!("example", "com", "").get_length_octet(2), Some(0));
    /// assert_eq!(ref_domain!("example", "com", "").get_length_octet(3), None);
    /// ```
    fn get_length_octet(&self, index: usize) -> Option<u8> {
        self.length_octets_iter().nth(index)
    }

    /// Get the length octet for the first (left-most) label. This is equivalent
    /// to getting the length octet for the label at index 0.
    ///
    /// This should always be equivalent to getting first label and then getting
    /// the length of that label.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{ref_domain, types::domain_name::DomainName};
    ///
    /// assert_eq!(ref_domain!("").first_length_octet(), Some(0));
    /// assert_eq!(ref_domain!("com", "").first_length_octet(), Some(3));
    /// assert_eq!(ref_domain!("example", "com", "").first_length_octet(), Some(7));
    /// ```
    fn first_length_octet(&self) -> Option<u8> {
        self.length_octets_iter().next()
    }

    /// Get the length octet for the last (right-most) label. This is equivalent
    /// to getting the length octet for the label at the last index.
    ///
    /// This should always be equivalent to getting last label and then getting
    /// the length of that label.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{ref_domain, types::domain_name::DomainName};
    ///
    /// assert_eq!(ref_domain!("").last_length_octet(), Some(0));
    /// assert_eq!(ref_domain!("example", "com").last_length_octet(), Some(3));
    /// assert_eq!(ref_domain!("www", "example", "com").last_length_octet(), Some(3));
    /// ```
    fn last_length_octet(&self) -> Option<u8> {
        self.length_octets_iter().next_back()
    }

    /// Returns an iterator over the labels of this domain name, starting from
    /// the left-most label, and iterating towards the right-most label (often,
    /// the root label).
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{ref_domain, ref_label, types::{domain_name::DomainName, label::CaseInsensitive}};
    ///
    /// let domain_name = ref_domain!("www", "example", "com", "");
    /// let mut labels_iter = domain_name.labels_iter();
    ///
    /// assert_eq!(labels_iter.next().map(CaseInsensitive), Some(CaseInsensitive(ref_label!("www"))));
    /// assert_eq!(labels_iter.next().map(CaseInsensitive), Some(CaseInsensitive(ref_label!("example"))));
    /// assert_eq!(labels_iter.next().map(CaseInsensitive), Some(CaseInsensitive(ref_label!("com"))));
    /// assert_eq!(labels_iter.next().map(CaseInsensitive), Some(CaseInsensitive(ref_label!(""))));
    /// assert_eq!(labels_iter.next().map(CaseInsensitive), None);
    /// ```
    fn labels_iter<'a>(
        &'a self,
    ) -> impl 'a
    + DoubleEndedIterator<Item = &'a RefLabel>
    + ExactSizeIterator
    + FusedIterator
    + Debug
    + Clone;

    /// Returns an iterator over the length octets of this domain name, starting
    /// from the left-most label's length octet, and iterating towards the
    /// right-most label's length octet (often, the root label).
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{ref_domain, types::domain_name::DomainName};
    ///
    /// let domain_name = ref_domain!("www", "example", "com", "");
    /// let mut length_octets_iter = domain_name.length_octets_iter();
    ///
    /// assert_eq!(length_octets_iter.next(), Some(3));
    /// assert_eq!(length_octets_iter.next(), Some(7));
    /// assert_eq!(length_octets_iter.next(), Some(3));
    /// assert_eq!(length_octets_iter.next(), Some(0));
    /// assert_eq!(length_octets_iter.next(), None);
    /// ```
    fn length_octets_iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = u8> + ExactSizeIterator + FusedIterator + Debug + Clone
    {
        self.labels_iter().map(RefLabel::len)
    }
}

pub trait DomainNameMut {
    /// Converts this domain to use all lowercase characters.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{ref_domain, domain, types::domain_name::DomainNameMut};
    ///
    /// let mut domain_name = domain!("WWW", "EXAMPLE", "COM", "");
    /// domain_name.make_lowercase();
    /// assert_eq!(domain_name, ref_domain!("www", "example", "com", ""));
    /// ```
    fn make_lowercase(&mut self) {
        self.labels_iter_mut().for_each(MutLabel::make_lowercase);
    }

    /// Converts this domain to use all uppercase characters.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{ref_domain, domain, types::domain_name::DomainNameMut};
    ///
    /// let mut domain_name = domain!("www", "example", "com", "");
    /// domain_name.make_uppercase();
    /// assert_eq!(domain_name, ref_domain!("WWW", "EXAMPLE", "COM", ""));
    /// ```
    fn make_uppercase(&mut self) {
        self.labels_iter_mut().for_each(MutLabel::make_uppercase);
    }

    /// Gets the nth label in the domain or `None` if `index` is out of
    /// bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{ref_domain, ref_label, domain, types::domain_name::DomainNameMut};
    ///
    /// let mut domain_name = domain!("www", "example", "com", "");
    /// let label = domain_name.get_mut(1);
    /// assert_eq!(label, ref_label!("example"));
    /// label.make_uppercase();
    /// assert_eq!(domain_name, ref_domain!("www", "EXAMPLE", "com", ""));
    /// ```
    fn get_mut(&mut self, index: usize) -> Option<&mut RefLabel> {
        self.labels_iter_mut().nth(index)
    }

    /// Returns the first label of the domain, or `None` if it is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{ref_domain, ref_label, domain, types::domain_name::DomainNameMut};
    ///
    /// let mut domain_name = domain!("www", "example", "com", "");
    /// let label = domain_name.first_mut();
    /// assert_eq!(label, ref_label!("www"));
    /// label.make_uppercase();
    /// assert_eq!(domain_name, ref_domain!("WWW", "example", "com", ""));
    /// ```
    fn first_mut(&mut self) -> Option<&mut RefLabel> {
        self.labels_iter_mut().next()
    }

    /// Returns the last label of the domain, or `None` if it is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{ref_domain, ref_label, domain, types::domain_name::DomainNameMut};
    ///
    /// let mut domain_name = domain!("www", "example", "com");
    /// let label = domain_name.last_mut();
    /// assert_eq!(label, ref_label!("com"));
    /// label.make_uppercase();
    /// assert_eq!(domain_name, ref_domain!("WWW", "example", "COM"));
    /// ```
    fn last_mut(&mut self) -> Option<&mut RefLabel> {
        self.labels_iter_mut().last()
    }

    /// Returns an iterator over the labels of this domain name, starting from
    /// the left-most label, and iterating towards the right-most label (often,
    /// the root label).
    ///
    /// # Examples
    ///
    /// ```
    /// use dns_lib::{ref_domain, domain, types::{domain_name::DomainNameMut, label::MutLabel}};
    ///
    /// let mut domain_name = domain!("www", "example", "com", "");
    /// domain_name.labels_iter_mut()
    ///     .nth(1)
    ///     .expect("the domain has 4 labels. Index 1 must be in-range")
    ///     .make_uppercase();
    /// assert_eq!(domain_name, ref_domain!("www", "EXAMPLE", "com", ""));
    /// ```
    fn labels_iter_mut<'a>(
        &'a mut self,
    ) -> impl 'a + DoubleEndedIterator<Item = &'a mut RefLabel> + ExactSizeIterator + FusedIterator + Debug;
}

// Need to explicitly implement each trait method so that if a more efficient
// implementation is available for that particular type, it gets used instead of
// the default one.
impl<T: DomainName> DomainName for &T {
    fn octet_count(&self) -> u16 {
        (*self).octet_count()
    }

    fn label_count(&self) -> u16 {
        (*self).label_count()
    }

    fn is_root(&self) -> bool {
        (*self).is_root()
    }

    fn is_lowercase(&self) -> bool {
        (*self).is_lowercase()
    }

    fn is_uppercase(&self) -> bool {
        (*self).is_uppercase()
    }

    fn is_fully_qualified(&self) -> bool {
        (*self).is_fully_qualified()
    }

    fn is_canonical(&self) -> bool {
        (*self).is_canonical()
    }

    fn is_parent_of<D: DomainName>(&self, other: &D) -> bool {
        (*self).is_parent_of(other)
    }

    fn is_parent_of_ignore_case<D: DomainName>(&self, other: &D) -> bool {
        (*self).is_parent_of_ignore_case(other)
    }

    fn is_child_of<D: DomainName>(&self, other: &D) -> bool {
        (*self).is_child_of(other)
    }

    fn is_child_of_ignore_case<D: DomainName>(&self, other: &D) -> bool {
        (*self).is_child_of_ignore_case(other)
    }

    fn eq_ignore_case<D: DomainName>(&self, other: &D) -> bool {
        (*self).eq_ignore_case(other)
    }

    fn get_label(&self, index: usize) -> Option<&RefLabel> {
        (*self).get_label(index)
    }

    fn first_label(&self) -> Option<&RefLabel> {
        (*self).first_label()
    }

    fn last_label(&self) -> Option<&RefLabel> {
        (*self).last_label()
    }

    fn get_length_octet(&self, index: usize) -> Option<u8> {
        (*self).get_length_octet(index)
    }

    fn first_length_octet(&self) -> Option<u8> {
        (*self).first_length_octet()
    }

    fn last_length_octet(&self) -> Option<u8> {
        (*self).last_length_octet()
    }

    fn labels_iter<'a>(
        &'a self,
    ) -> impl 'a
    + DoubleEndedIterator<Item = &'a RefLabel>
    + ExactSizeIterator
    + FusedIterator
    + Debug
    + Clone {
        (*self).labels_iter()
    }

    fn length_octets_iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = u8> + ExactSizeIterator + FusedIterator + Debug + Clone
    {
        (*self).length_octets_iter()
    }
}

// Need to explicitly implement each trait method so that if a more efficient
// implementation is available for that particular type, it gets used instead of
// the default one.
impl<T: DomainName> DomainName for &mut T {
    fn octet_count(&self) -> u16 {
        (&**self).octet_count()
    }

    fn label_count(&self) -> u16 {
        (&**self).label_count()
    }

    fn is_root(&self) -> bool {
        (&**self).is_root()
    }

    fn is_lowercase(&self) -> bool {
        (&**self).is_lowercase()
    }

    fn is_uppercase(&self) -> bool {
        (&**self).is_uppercase()
    }

    fn is_fully_qualified(&self) -> bool {
        (&**self).is_fully_qualified()
    }

    fn is_canonical(&self) -> bool {
        (&**self).is_canonical()
    }

    fn is_parent_of<D: DomainName>(&self, other: &D) -> bool {
        (&**self).is_parent_of(other)
    }

    fn is_parent_of_ignore_case<D: DomainName>(&self, other: &D) -> bool {
        (&**self).is_parent_of_ignore_case(other)
    }

    fn is_child_of<D: DomainName>(&self, other: &D) -> bool {
        (&**self).is_child_of(other)
    }

    fn is_child_of_ignore_case<D: DomainName>(&self, other: &D) -> bool {
        (&**self).is_child_of_ignore_case(other)
    }

    fn eq_ignore_case<D: DomainName>(&self, other: &D) -> bool {
        (&**self).eq_ignore_case(other)
    }

    fn get_label(&self, index: usize) -> Option<&RefLabel> {
        (&**self).get_label(index)
    }

    fn first_label(&self) -> Option<&RefLabel> {
        (&**self).first_label()
    }

    fn last_label(&self) -> Option<&RefLabel> {
        (&**self).last_label()
    }

    fn get_length_octet(&self, index: usize) -> Option<u8> {
        (&**self).get_length_octet(index)
    }

    fn first_length_octet(&self) -> Option<u8> {
        (&**self).first_length_octet()
    }

    fn last_length_octet(&self) -> Option<u8> {
        (&**self).last_length_octet()
    }

    fn labels_iter<'a>(
        &'a self,
    ) -> impl 'a
    + DoubleEndedIterator<Item = &'a RefLabel>
    + ExactSizeIterator
    + FusedIterator
    + Debug
    + Clone {
        (&**self).labels_iter()
    }

    fn length_octets_iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = u8> + ExactSizeIterator + FusedIterator + Debug + Clone
    {
        (&**self).length_octets_iter()
    }
}

// Need to explicitly implement each trait method so that if a more efficient
// implementation is available for that particular type, it gets used instead of
// the default one.
impl<T: DomainNameMut> DomainNameMut for &mut T {
    fn make_lowercase(&mut self) {
        (*self).make_lowercase();
    }

    fn make_uppercase(&mut self) {
        (*self).make_uppercase();
    }

    fn labels_iter_mut<'a>(
        &'a mut self,
    ) -> impl 'a + DoubleEndedIterator<Item = &'a mut RefLabel> + ExactSizeIterator + FusedIterator + Debug
    {
        (*self).labels_iter_mut()
    }
}

macro_rules! impl_domain_name {
    (impl $(( $($impl_generics:tt)+ ))? for $domain_type:ty) => {
        impl $(<$($impl_generics)+>)? $domain_type {
            /// Verify that the fields are in agreement and any other obvious
            /// invariant violations.
            ///
            /// This is a no-op when debug_assertions is not enabled.
            pub(super) fn debug_assert_invariants(&self) {
                // TODO: I call this function all over the place. Make sure to
                //       remove the extra calls or lock them behind an
                //       additional feature flag after things are properly
                //       tested.
                if cfg!(debug_assertions) {
                    self.assert_invariants();
                }
            }

            /// Verify that the fields are in agreement and any other obvious
            /// invariant violations. Transfer ownership to make it easy to call
            /// on instantiation.
            ///
            /// This is a no-op when debug_assertions is not enabled.
            pub(super) fn and_debug_assert_invariants(self) -> Self {
                // TODO: I call this function all over the place. Make sure to
                //       remove the extra calls or lock them behind an
                //       additional feature flag after things are properly
                //       tested.
                self.debug_assert_invariants();
                self
            }
        }

        impl $(<$($impl_generics)+>)? ::std::fmt::Display for $domain_type {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                if self.is_root() {
                    return write!(f, ".");
                }
                let mut labels = self.labels_iter();
                if let Some(label) = labels.next() {
                    write!(f, "{label}")?;
                }
                for label in labels {
                    write!(f, ".{label}")?;
                }
                Ok(())
            }
        }
    };
}

mod raw;
mod vec;

pub use raw::*;
pub use vec::*;

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
    use std::{
        fmt::{Debug, Display},
        iter::FusedIterator,
    };

    use concat_idents::concat_idents;
    use rstest::rstest;
    use static_assertions::const_assert;

    use crate::{
        ref_domain, ref_label,
        serde::wire::{from_wire::FromWire, to_wire::ToWire},
        types::{
            domain_name::{
                CompressibleDomainVec, DomainName, DomainNameMut, DomainSlice, DomainVec,
                IncompressibleDomainVec, MAX_LABEL_OCTETS,
            },
            label::{CaseSensitive, RefLabel},
        },
    };

    /// Prints domain names whose concrete type us unknown.
    struct DomainDisplay<D>(D);
    impl<D: DomainName> Display for DomainDisplay<D> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            if self.0.is_root() {
                return write!(f, ".");
            }
            let mut labels = self.0.labels_iter();
            if let Some(label) = labels.next() {
                write!(f, "{label}")?;
            }
            for label in labels {
                write!(f, ".{label}")?;
            }
            Ok(())
        }
    }

    /// Implements `DomainName` using the default implementations instead of the
    /// underlying specialized implementation. This allows us to verify the
    /// default implementations using specialized types.
    struct DefaultDomain<D>(D);
    impl<D: DomainName> DomainName for DefaultDomain<D> {
        fn labels_iter<'a>(
            &'a self,
        ) -> impl 'a
        + DoubleEndedIterator<Item = &'a RefLabel>
        + ExactSizeIterator
        + std::iter::FusedIterator
        + Debug
        + Clone {
            self.0.labels_iter()
        }
    }
    impl<D: DomainNameMut> DomainNameMut for DefaultDomain<D> {
        fn labels_iter_mut<'a>(
            &'a mut self,
        ) -> impl 'a
        + DoubleEndedIterator<Item = &'a mut RefLabel>
        + ExactSizeIterator
        + FusedIterator
        + Debug {
            self.0.labels_iter_mut()
        }
    }

    fn impl_domain_octet_count(domain: impl DomainName, octet_count: u16) {
        assert_eq!(
            domain.octet_count(),
            octet_count,
            "{} is expected to have {octet_count} octets",
            DomainDisplay(domain)
        );
    }

    fn impl_domain_label_count(domain: impl DomainName, expected_labels: &[&RefLabel]) {
        let label_count = u16::try_from(expected_labels.len())
            .expect("It must be possible to represent the number of labels using a u16");
        assert_eq!(
            domain.label_count(),
            label_count,
            "{} is expected to have {label_count} labels",
            DomainDisplay(domain)
        );
    }

    fn impl_domain_is_root(domain: impl DomainName, is_root: bool) {
        assert_eq!(
            domain.is_root(),
            is_root,
            "{} is {}expected to be to be root",
            DomainDisplay(domain),
            if is_root { "" } else { "not " }
        );
    }

    fn impl_domain_is_lowercase(domain: impl DomainName, is_lowercase: bool) {
        assert_eq!(
            domain.is_lowercase(),
            is_lowercase,
            "{} is {}expected to be to be lowercase",
            DomainDisplay(domain),
            if is_lowercase { "" } else { "not " }
        );
    }

    fn impl_domain_is_uppercase(domain: impl DomainName, is_uppercase: bool) {
        assert_eq!(
            domain.is_uppercase(),
            is_uppercase,
            "{} is {}expected to be to be uppercase",
            DomainDisplay(domain),
            if is_uppercase { "" } else { "not " }
        );
    }

    fn impl_domain_is_fully_qualified(domain: impl DomainName, is_fully_qualified: bool) {
        assert_eq!(
            domain.is_fully_qualified(),
            is_fully_qualified,
            "{} is {}expected to be to be fully qualified",
            DomainDisplay(domain),
            if is_fully_qualified { "" } else { "not " }
        );
    }

    fn impl_domain_is_canonical(domain: impl DomainName, is_canonical: bool) {
        assert_eq!(
            domain.is_canonical(),
            is_canonical,
            "{} is {}expected to be to be canonical",
            DomainDisplay(domain),
            if is_canonical { "" } else { "not " }
        );
    }

    fn impl_domain_get_label(domain: impl DomainName, expected_labels: &[&RefLabel]) {
        for (index, expected_label) in expected_labels.iter().enumerate() {
            assert_eq!(
                domain.get_label(index).map(CaseSensitive),
                Some(CaseSensitive(*expected_label))
            );
        }
        assert!(domain.get_label(expected_labels.len()).is_none());
        assert!(domain.get_label(usize::MAX).is_none());
    }

    fn impl_domain_first_label(domain: impl DomainName, expected_labels: &[&RefLabel]) {
        assert_eq!(
            domain.first_label().map(CaseSensitive),
            expected_labels.first().copied().map(CaseSensitive)
        );
        assert_eq!(
            domain.first_label().map(CaseSensitive),
            domain.get_label(0).map(CaseSensitive)
        );
        assert_eq!(
            domain.first_label().map(CaseSensitive),
            domain.labels_iter().next().map(CaseSensitive)
        );
    }

    fn impl_domain_last_label(domain: impl DomainName, expected_labels: &[&RefLabel]) {
        assert_eq!(
            domain.last_label().map(CaseSensitive),
            expected_labels.last().copied().map(CaseSensitive)
        );
        assert_eq!(
            domain.last_label().map(CaseSensitive),
            domain
                .get_label(expected_labels.len() - 1)
                .map(CaseSensitive)
        );
        assert_eq!(
            domain.last_label().map(CaseSensitive),
            domain.labels_iter().last().map(CaseSensitive)
        );
    }

    fn impl_domain_get_length_octet(domain: impl DomainName, expected_labels: &[&RefLabel]) {
        for (index, expected_label) in expected_labels.iter().enumerate() {
            assert_eq!(
                domain.get_length_octet(index),
                Some(
                    u8::try_from(expected_label.len())
                        .expect("the length of a label must fit into u8")
                )
            );
        }
        assert!(domain.get_length_octet(expected_labels.len()).is_none());
        assert!(domain.get_length_octet(usize::MAX).is_none());
    }

    fn impl_domain_first_length_octet(domain: impl DomainName, expected_labels: &[&RefLabel]) {
        assert_eq!(
            domain.first_length_octet(),
            expected_labels
                .first()
                .map(|label| u8::try_from(label.len())
                    .expect("the length of a label must fit into u8"))
        );
        assert_eq!(domain.first_length_octet(), domain.get_length_octet(0),);
        assert_eq!(
            domain.first_length_octet(),
            domain
                .labels_iter()
                .next()
                .map(|label| u8::try_from(label.len())
                    .expect("the length of a label must fit into u8"))
        );

        if expected_labels.is_empty() {
            assert!(domain.first_length_octet().is_none());
        } else {
            assert!(domain.first_length_octet().is_some());
        }
    }

    fn impl_domain_last_length_octet(domain: impl DomainName, expected_labels: &[&RefLabel]) {
        assert_eq!(
            domain.last_length_octet(),
            expected_labels
                .last()
                .map(|label| u8::try_from(label.len())
                    .expect("the length of a label must fit into u8"))
        );
        assert_eq!(
            domain.last_length_octet(),
            domain.get_length_octet(expected_labels.len() - 1),
        );
        assert_eq!(
            domain.last_length_octet(),
            domain
                .labels_iter()
                .last()
                .map(|label| u8::try_from(label.len())
                    .expect("the length of a label must fit into u8"))
        );

        if expected_labels.is_empty() {
            assert!(domain.last_length_octet().is_none());
        } else {
            assert!(domain.last_length_octet().is_some());
        }
    }

    fn domain_labels_iter_verify_extra_properties<'a, 'b>(
        domain_labels: impl DoubleEndedIterator<Item = &'a RefLabel> + Debug + Clone,
        expected_labels: impl DoubleEndedIterator<Item = &'b RefLabel>
        + ExactSizeIterator
        + Debug
        + Clone,
    ) {
        assert_eq!(
            expected_labels.clone().next().map(CaseSensitive),
            domain_labels.clone().next().map(CaseSensitive)
        );
        assert_eq!(
            expected_labels.clone().next_back().map(CaseSensitive),
            domain_labels.clone().next_back().map(CaseSensitive),
        );
        assert_eq!(
            expected_labels.clone().last().map(CaseSensitive),
            domain_labels.clone().last().map(CaseSensitive)
        );
        assert_eq!(
            domain_labels.clone().last().map(CaseSensitive),
            domain_labels.clone().next_back().map(CaseSensitive),
        );
        assert_eq!(
            expected_labels.clone().count(),
            domain_labels.clone().count(),
        );
        assert!(expected_labels.clone().count() == domain_labels.clone().size_hint().0);
        assert!(
            expected_labels.clone().count()
                == domain_labels
                    .clone()
                    .size_hint()
                    .1
                    .expect("domain label iterators should always have a known length upper bound")
        );
        for n in 0..(domain_labels.clone().count() + 1) {
            assert_eq!(
                expected_labels.clone().nth(n).map(CaseSensitive),
                domain_labels.clone().nth(n).map(CaseSensitive)
            );
            assert_eq!(
                expected_labels.clone().rev().nth(n).map(CaseSensitive),
                domain_labels.clone().rev().nth(n).map(CaseSensitive),
            );
        }
    }

    fn impl_domain_labels_iter_test(domain: impl DomainName, expected_labels: &[&RefLabel]) {
        let expected_labels = expected_labels
            .into_iter()
            .copied()
            .map(CaseSensitive)
            .collect::<Vec<_>>();
        let actual_labels = domain.labels_iter().map(CaseSensitive).collect::<Vec<_>>();
        assert_eq!(expected_labels, actual_labels);
    }

    fn impl_domain_labels_reverse_iter_test(
        domain: impl DomainName,
        expected_labels: &[&RefLabel],
    ) {
        let expected_labels = expected_labels
            .into_iter()
            .rev()
            .copied()
            .map(CaseSensitive)
            .collect::<Vec<_>>();
        let actual_labels = domain
            .labels_iter()
            .rev()
            .map(CaseSensitive)
            .collect::<Vec<_>>();
        assert_eq!(expected_labels, actual_labels);
    }

    fn impl_domain_labels_nth_iter_test(domain: impl DomainName, expected_labels: &[&RefLabel]) {
        for n in 0..(expected_labels.len() * 2) {
            let mut expected_labels = expected_labels.into_iter().copied();
            let mut domain_labels = domain.labels_iter();
            for _ in 0..(expected_labels.len() * 2) {
                assert_eq!(
                    expected_labels.nth(n).map(CaseSensitive),
                    domain_labels.nth(n).map(CaseSensitive)
                );
                domain_labels_iter_verify_extra_properties(
                    domain_labels.clone(),
                    expected_labels.clone(),
                );
            }
        }
    }

    fn impl_domain_labels_reverse_nth_iter_test(
        domain: impl DomainName,
        expected_labels: &[&RefLabel],
    ) {
        let mut expected_labels = expected_labels.to_vec();
        expected_labels.reverse();

        for n in 0..(expected_labels.len() * 2) {
            let mut expected_labels = expected_labels.iter().copied();
            let mut domain_labels = domain.labels_iter().rev();
            for _ in 0..(expected_labels.len() * 2) {
                assert_eq!(
                    expected_labels.nth(n).map(CaseSensitive),
                    domain_labels.nth(n).map(CaseSensitive)
                );
                domain_labels_iter_verify_extra_properties(
                    domain_labels.clone(),
                    expected_labels.clone(),
                );
            }
        }
    }

    fn impl_domain_labels_to_str_test(
        domain: impl DomainName,
        expected_labels: &[&RefLabel],
        expected_label_strings: &[&str],
    ) {
        assert_eq!(domain.labels_iter().count(), expected_labels.len());
        assert_eq!(domain.labels_iter().count(), expected_label_strings.len());
        for ((domain_label, &expected_label), &expected_label_str) in domain
            .labels_iter()
            .zip(expected_labels)
            .zip(expected_label_strings)
        {
            assert_eq!(CaseSensitive(expected_label), CaseSensitive(domain_label));
            assert_eq!(expected_label_str, domain_label.to_string().as_str());
            assert_eq!(expected_label_str, expected_label.to_string().as_str());
        }
    }

    fn impl_domain_length_octets_match_labels_test(
        domain: impl DomainName,
        expected_labels: &[&RefLabel],
    ) {
        assert_eq!(domain.length_octets_iter().count(), expected_labels.len());
        for (domain_length_octet, expected_length_octet) in domain
            .length_octets_iter()
            .map(usize::from)
            .zip(expected_labels.iter().map(|label| label.len()))
        {
            assert_eq!(domain_length_octet, expected_length_octet);
            assert!(domain_length_octet <= usize::from(MAX_LABEL_OCTETS));
        }
    }

    /// Generates a 3 constants, each with a different prefix:
    ///
    /// - DOMAIN_* - has the specified labels in the form of a `DomainSlice`.
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
                    const name: DomainSlice<'static> = ref_domain![$($label),+];
                });
                concat_idents!(name = LABELS_, $name {
                    const name: &[&RefLabel] = &[
                        $(ref_label![$label]),+
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
        (@make_domain_rstest_cases [$($name:tt $(, $remaining_args:expr)* $(,)?);* $(;)?] $($impl_fn:tt)*) => {
            #[rstest]
            $(
                // Test specializations
                #[case(
                    generate!(@ident DOMAIN_, $name).to_domain_vec(),
                    $($remaining_args),*
                )]
                #[case(
                    generate!(@ident DOMAIN_, $name).as_domain_slice(),
                    $($remaining_args),*
                )]
                #[case(
                    generate!(@ident DOMAIN_, $name).to_raw_domain_vec(),
                    $($remaining_args),*
                )]
                #[case(
                    generate!(@ident DOMAIN_, $name).as_raw_domain_slice(),
                    $($remaining_args),*
                )]

                // Test default implementation using specialized iterator
                #[case(
                    DefaultDomain(generate!(@ident DOMAIN_, $name).to_domain_vec()),
                    $($remaining_args),*
                )]
                #[case(
                    DefaultDomain(generate!(@ident DOMAIN_, $name).as_domain_slice()),
                    $($remaining_args),*
                )]
                #[case(
                    DefaultDomain(generate!(@ident DOMAIN_, $name).to_raw_domain_vec()),
                    $($remaining_args),*
                )]
                #[case(
                    DefaultDomain(generate!(@ident DOMAIN_, $name).as_raw_domain_slice()),
                    $($remaining_args),*
                )]
            )*
            $($impl_fn)*
        };
        (@make_domain_test DOMAIN_ $attribute_type:ty; $call:ident [$($name:tt, $attribute_value:literal),* $(,)?]) => {
            generate!(@make_domain_rstest_cases
                [$(
                    $name,
                    $attribute_value
                );*]
                fn $call(
                    #[case] domain: impl DomainName,
                    #[case] attribute: $attribute_type,
                ) {
                    generate!(@ident impl_, $call)(domain, attribute);
                }
            );
        };
        (@make_domain_test DOMAIN_ LABELS_; $call:ident [$($name:tt),* $(,)?]) => {
            generate!(@make_domain_rstest_cases
                [$(
                    $name,
                    generate!(@ident LABELS_, $name)
                );*]
                fn $call(
                    #[case] domain: impl DomainName,
                    #[case] expected_labels: &[&RefLabel],
                ) {
                    generate!(@ident impl_, $call)(domain, expected_labels);
                }
            );
        };
        (@make_domain_test DOMAIN_ LABELS_ LABEL_STRINGS_; $call:ident [$($name:tt),* $(,)?]) => {
            generate!(@make_domain_rstest_cases
                [$(
                    $name,
                    generate!(@ident LABELS_, $name),
                    generate!(@ident LABEL_STRINGS_, $name),
                );*]
                fn $call(
                    #[case] domain: impl DomainName,
                    #[case] expected_labels: &[&RefLabel],
                    #[case] expected_label_strings: &[&str],
                ) {
                    generate!(@ident impl_, $call)(domain, expected_labels, expected_label_strings);
                }
            );
        };
        (@make_tests $(
            $name:tt,
            octets = $octet_count:literal,
            is_root = $is_root:literal,
            is_lowercase = $is_lowercase:literal,
            is_uppercase = $is_uppercase:literal,
            is_fully_qualified = $is_fully_qualified:literal,
            is_canonical = $is_canonical:literal
        ),* $(,)?) => {
            #[rstest]
            $(
                #[case(CompressibleDomainVec(
                    generate!(@ident DOMAIN_, $name).to_domain_vec()
                ))]
                #[case(IncompressibleDomainVec(
                    generate!(@ident DOMAIN_, $name).to_domain_vec()
                ))]
                // TODO: serde for array, raw vec, & raw array
            )*
            fn circular_serde_sanity_test<T>(#[case] input: T) where T: Debug + ToWire + FromWire + PartialEq {
                crate::serde::wire::circular_test::circular_serde_sanity_test::<T>(input)
            }

            generate!(@make_domain_test DOMAIN_ u16;     domain_octet_count        [$($name, $octet_count       ),*]);
            generate!(@make_domain_test DOMAIN_ LABELS_; domain_label_count        [$($name                     ),*]);
            generate!(@make_domain_test DOMAIN_ bool;    domain_is_root            [$($name, $is_root           ),*]);
            generate!(@make_domain_test DOMAIN_ bool;    domain_is_lowercase       [$($name, $is_lowercase      ),*]);
            generate!(@make_domain_test DOMAIN_ bool;    domain_is_uppercase       [$($name, $is_uppercase      ),*]);
            generate!(@make_domain_test DOMAIN_ bool;    domain_is_fully_qualified [$($name, $is_fully_qualified),*]);
            generate!(@make_domain_test DOMAIN_ bool;    domain_is_canonical       [$($name, $is_canonical      ),*]);
            generate!(@make_domain_test DOMAIN_ LABELS_; domain_get_label          [$($name                     ),*]);
            generate!(@make_domain_test DOMAIN_ LABELS_; domain_first_label        [$($name                     ),*]);
            generate!(@make_domain_test DOMAIN_ LABELS_; domain_last_label         [$($name                     ),*]);
            generate!(@make_domain_test DOMAIN_ LABELS_; domain_get_length_octet   [$($name                     ),*]);
            generate!(@make_domain_test DOMAIN_ LABELS_; domain_first_length_octet [$($name                     ),*]);
            generate!(@make_domain_test DOMAIN_ LABELS_; domain_last_length_octet  [$($name                     ),*]);

            // Iterator tests
            generate!(@make_domain_test DOMAIN_ LABELS_;                domain_labels_iter_test                [$($name),*]);
            generate!(@make_domain_test DOMAIN_ LABELS_;                domain_labels_reverse_iter_test        [$($name),*]);
            generate!(@make_domain_test DOMAIN_ LABELS_;                domain_labels_nth_iter_test            [$($name),*]);
            generate!(@make_domain_test DOMAIN_ LABELS_;                domain_labels_reverse_nth_iter_test    [$($name),*]);
            generate!(@make_domain_test DOMAIN_ LABELS_;                domain_length_octets_match_labels_test [$($name),*]);
            generate!(@make_domain_test DOMAIN_ LABELS_ LABEL_STRINGS_; domain_labels_to_str_test              [$($name),*]);
        };
        ($([$name:tt; $($attribute_key:tt = $attribute_value:literal),+; $($label:expr),+ $(,)?]),* $(,)?) => {
            generate!(@make_constants $([$name; $($label),*]),+);
            generate!(@validate_test_cases $($name),+);
            generate!(@make_tests $($name, $($attribute_key = $attribute_value),+),+);
        };
    }
    generate![
        [ROOT;
            octets = 1,
            is_root = true,
            is_lowercase = true,
            is_uppercase = true,
            is_fully_qualified = true,
            is_canonical = true;
            ""
        ],
        [2_LABELS_UPPER;
            octets = 5,
            is_root = false,
            is_lowercase = false,
            is_uppercase = true,
            is_fully_qualified = true,
            is_canonical = false;
            "COM", ""
        ],
        [2_LABELS_LOWER;
            octets = 5,
            is_root = false,
            is_lowercase = true,
            is_uppercase = false,
            is_fully_qualified = true,
            is_canonical = true;
            "com", ""
        ],
        [2_LABELS_MIXED;
            octets = 5,
            is_root = false,
            is_lowercase = false,
            is_uppercase = false,
            is_fully_qualified = true,
            is_canonical = false;
            "Com", ""
        ],
        [3_LABELS_UPPER;
            octets = 13,
            is_root = false,
            is_lowercase = false,
            is_uppercase = true,
            is_fully_qualified = true,
            is_canonical = false;
            "EXAMPLE", "COM", ""
        ],
        [3_LABELS_LOWER;
            octets = 13,
            is_root = false,
            is_lowercase = true,
            is_uppercase = false,
            is_fully_qualified = true,
            is_canonical = true;
            "example", "com", ""
        ],
        [3_LABELS_MIXED;
            octets = 13,
            is_root = false,
            is_lowercase = false,
            is_uppercase = false,
            is_fully_qualified = true,
            is_canonical = false;
            "EXAMPLE", "com", ""
        ],
        [4_LABELS_UPPER;
            octets = 17,
            is_root = false,
            is_lowercase = false,
            is_uppercase = true,
            is_fully_qualified = true,
            is_canonical = false;
            "WWW", "EXAMPLE", "COM", ""
        ],
        [4_LABELS_LOWER;
            octets = 17,
            is_root = false,
            is_lowercase = true,
            is_uppercase = false,
            is_fully_qualified = true,
            is_canonical = true;
            "www", "example", "com", ""
        ],
        [4_LABELS_MIXED;
            octets = 17,
            is_root = false,
            is_lowercase = false,
            is_uppercase = false,
            is_fully_qualified = true,
            is_canonical = false;
            "WWW", "example", "Com", ""
        ],
        [10_LABELS;
            octets = 62,
            is_root = false,
            is_lowercase = true,
            is_uppercase = false,
            is_fully_qualified = true,
            is_canonical = true;
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
        [MAX_LABELS;
            octets = 255,
            is_root = false,
            is_lowercase = true,
            is_uppercase = false,
            is_fully_qualified = true,
            is_canonical = true;
            "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m",
            "n", "o", "p", "q", "r", "s", "t", "u", "v", "w", "x", "y", "z",
            "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m",
            "n", "o", "p", "q", "r", "s", "t", "u", "v", "w", "x", "y", "z",
            "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m",
            "n", "o", "p", "q", "r", "s", "t", "u", "v", "w", "x", "y", "z",
            "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m",
            "n", "o", "p", "q", "r", "s", "t", "u", "v", "w", "x", "y", "z",
            "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m",
            "n", "o", "p", "q", "r", "s", "t", "u", "v", "w", "",
        ],
        [REPETITIVE;
            octets = 177,
            is_root = false,
            is_lowercase = true,
            is_uppercase = false,
            is_fully_qualified = true,
            is_canonical = true;
            "www", "example", "org", "www", "example", "org", "www", "example", "org", "www",
            "example", "org", "www", "example", "org", "www", "example", "org", "www", "example",
            "org", "www", "example", "org", "www", "example", "org", "www", "example", "org", "www",
            "example", "org", ""
        ],
        [1_LONG_LABEL;
            octets = 65,
            is_root = false,
            is_lowercase = false,
            is_uppercase = false,
            is_fully_qualified = true,
            is_canonical = false;
            "abcdefghijklmnopqrstuvwxyz ABCDEFGHIJKLMNOPQRSTUVWXYZ 012345678", ""
        ],
        [2_LONG_LABELS;
            octets = 129,
            is_root = false,
            is_lowercase = false,
            is_uppercase = false,
            is_fully_qualified = true,
            is_canonical = false;
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
            let domain_name = DomainVec::from_utf8(domain).unwrap();
            let expected_search_names = expected_search_names
                .into_iter()
                .map(|search_name| DomainVec::from_utf8(search_name).unwrap())
                .collect::<Vec<_>>();
            let actual_search_names = domain_name.search_domain_iter().collect::<Vec<_>>();
            assert_eq!(expected_search_names, actual_search_names);
        }

        for (domain, expected_search_names) in &domain_search_name_pairs {
            let domain_name = DomainVec::from_utf8(domain).unwrap();
            let expected_search_names = expected_search_names
                .into_iter()
                .rev()
                .map(|search_name| DomainVec::from_utf8(search_name).unwrap())
                .collect::<Vec<_>>();
            let actual_search_names = domain_name.search_domain_iter().rev().collect::<Vec<_>>();
            assert_eq!(expected_search_names, actual_search_names);
        }
    }
}

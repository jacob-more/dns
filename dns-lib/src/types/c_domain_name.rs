use std::{
    collections::HashMap,
    error::Error,
    fmt::{Debug, Display},
    iter::FusedIterator,
    marker::PhantomData,
    ops::Add,
};

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
    types::ascii::{AsciiError, AsciiString, constants::ASCII_PERIOD},
};

use super::{
    ascii::AsciiChar,
    domain_name::DomainName,
    label::{CaseInsensitive, Label, OwnedLabel, RefLabel, case_sensitivity::CaseSensitivity},
};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum CDomainNameError {
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

impl Error for CDomainNameError {}
impl Display for CDomainNameError {
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
            Self::LongDomain => write!(
                f,
                "Domain Name Exceeded {} Wire-Format Octets",
                CDomainName::MAX_OCTETS
            ),
            Self::LongLabel => write!(
                f,
                "Label Exceeded {} Wire-Format Octets",
                <OwnedLabel<CaseInsensitive>>::MAX_OCTETS
            ),
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
                CDomainName::MAX_COMPRESSION_POINTERS
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
impl From<AsciiError> for CDomainNameError {
    fn from(value: AsciiError) -> Self {
        Self::AsciiError(value)
    }
}

pub trait CmpDomainName<T>: Sized {
    /// determines if two sets of labels are identical, ignoring capitalization
    fn matches(&self, other: &T) -> bool;

    /// is_parent_domain_of checks if child is indeed a child of the parent. If child and parent are
    /// the same domain true is returned as well.
    fn is_parent_domain_of(&self, child: &T) -> bool;

    /// is_child_domain_of checks if this domain is indeed a child of the parent. If child and
    /// parent are the same domain true is returned as well.
    #[inline]
    fn is_child_domain_of(&self, parent: &T) -> bool
    where
        T: CmpDomainName<Self>,
    {
        parent.is_parent_domain_of(self)
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
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct CDomainName {
    /// Octets still contains label lengths inline despite `length_octets` containing all the length
    /// octets. This way, it maintains the exact same layout as the wire format.
    octets: Vec<AsciiChar>,
    /// A separate list with all the length octets. This allows for reverse iteration and keeping
    /// track of the number of labels.
    // A TinyVec with a length of 14 has a size of 24 bytes. This is the same size as a Vec.
    length_octets: TinyVec<[u8; 14]>,
}

impl CDomainName {
    /// Maximum number of bytes that can make up a domain name, including the length octet.
    pub const MAX_OCTETS: u16 = 256;
    pub const MIN_OCTETS: u16 = 0;
    /// Only one root label is allowed. All others must have a length of at least 1, so the maximum
    /// number of length octets is just over half the maximum number of octets. The domain with 129
    /// length octets still ends up being malformed so the true max is 128.
    pub const MAX_LABELS: u16 = Self::MAX_OCTETS.div_ceil(2);

    /// We have 14 bits for the compression pointer
    pub const MAX_COMPRESSION_OFFSET: u16 = 2 << 13;
    /// This is the maximum number of compression pointers that should occur in a valid message.
    /// Each label in a domain name must be at least one octet and be separated by a period. The
    /// root label won't be represented by a compression pointer, hence the -1 to exclude the root
    /// label.
    ///
    /// It is possible to construct a valid message that has more compression pointers than this,
    /// and still doesn't loop, by pointing to a previous pointer. This is not something a well
    /// written implementation should ever do and is not supported by this implementation.
    pub const MAX_COMPRESSION_POINTERS: u16 = Self::MAX_LABELS - 1;

    pub fn new_root() -> Self {
        Self {
            octets: vec![0],
            length_octets: tiny_vec![0],
        }
    }

    pub fn new(string: &AsciiString) -> Result<Self, CDomainNameError> {
        if string.is_empty() {
            return Err(CDomainNameError::EmptyString);
        }
        // As long as there are no escaped characters in the string and the name is fully qualified,
        // we expect the length to just about match the number of characters + 1 for the root label.
        let mut octets = Vec::with_capacity(string.len() + 1);
        // The first byte represents the length of the first label.
        octets.push(0);
        let mut length_octets = TinyVec::new();
        let mut length_octet_index = 0;

        for escaped_char_result in
            EscapedCharsEnumerateIter::from(string.iter().map(|character| *character).enumerate())
        {
            match (escaped_char_result, (octets.len() - length_octet_index)) {
                (Ok((0, EscapableChar::Ascii(ASCII_PERIOD))), _) => {
                    // leading dots are illegal except for the root zone
                    if string.len() > 1 {
                        return Err(CDomainNameError::LeadingDot);
                    }

                    length_octets.push(octets[length_octet_index]);
                    break;
                }
                // consecutive dots are never legal
                (Ok((1.., EscapableChar::Ascii(ASCII_PERIOD))), 1) => {
                    return Err(CDomainNameError::ConsecutiveDots);
                }
                // a label is found
                (Ok((1.., EscapableChar::Ascii(ASCII_PERIOD))), 2..) => {
                    length_octets.push(octets[length_octet_index]);

                    if octets.len() > Self::MAX_OCTETS as usize {
                        return Err(CDomainNameError::LongDomain);
                    }

                    length_octet_index = octets.len();
                    octets.push(0);
                }
                (Ok((_, escapable_char)), _) => {
                    octets.push(escapable_char.into_unescaped_character());
                    octets[length_octet_index] += 1;

                    // TODO: Can we optimize this check? It might be able to do once per label as
                    // long as we still check against the maximum number of octets every time.
                    if octets[length_octet_index] > <OwnedLabel<CaseInsensitive>>::MAX_OCTETS {
                        return Err(CDomainNameError::LongLabel);
                    }

                    if octets.len() > Self::MAX_OCTETS as usize {
                        return Err(CDomainNameError::LongDomain);
                    }
                }
                (Err(error), _) => return Err(CDomainNameError::ParseError(error)),
            }
        }

        if octets.len() >= (length_octet_index + 1) && (&octets != &[0]) {
            length_octets.push(octets[length_octet_index]);
        }

        octets.shrink_to_fit();
        Ok(Self {
            octets,
            length_octets,
        })
    }

    #[inline]
    pub fn from_utf8(string: &str) -> Result<Self, CDomainNameError> {
        Self::new(&AsciiString::from_utf8(string)?)
    }

    #[inline]
    pub fn from_labels<'a, C: CaseSensitivity, T: Label<C>>(
        labels: Vec<T>,
    ) -> Result<Self, CDomainNameError> {
        if labels.is_empty() {
            return Err(CDomainNameError::EmptyString);
        }
        let total_octets =
            labels.len() + (labels.iter().map(|label| label.len()).sum::<u16>() as usize);
        if total_octets > Self::MAX_OCTETS as usize {
            return Err(CDomainNameError::LongDomain);
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
        })
    }

    #[inline]
    pub fn from_owned_labels<C: CaseSensitivity>(
        labels: Vec<OwnedLabel<C>>,
    ) -> Result<Self, CDomainNameError> {
        if labels.is_empty() {
            return Err(CDomainNameError::EmptyString);
        }
        let total_octets =
            labels.len() + (labels.iter().map(|label| label.len()).sum::<u16>() as usize);
        if total_octets > Self::MAX_OCTETS as usize {
            return Err(CDomainNameError::LongDomain);
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
        })
    }

    #[inline]
    pub fn label_count(&self) -> usize {
        self.length_octets.len()
    }

    /// A domain name is root if it is made up of only 1 label, that has a length of zero.
    #[inline]
    pub fn is_root(&self) -> bool {
        &self.octets == &[0]
    }

    /// A domain name is fully qualified if it ends with a root label.
    #[inline]
    pub fn is_fully_qualified(&self) -> bool {
        self.length_octets.last() == Some(&0)
    }

    /// Converts this domain into a fully qualified domain. A domain name is fully qualified if it
    /// ends with a root label.
    #[inline]
    pub fn make_fully_qualified(&mut self) -> Result<(), CDomainNameError> {
        if self.is_fully_qualified() {
            return Ok(());
        // aka. Would adding a byte exceed the limit?
        } else if self.serial_length() >= Self::MAX_OCTETS {
            return Err(CDomainNameError::LongDomain);
        } else {
            self.octets.push(0);
            self.length_octets.push(0);
            return Ok(());
        }
    }

    /// Creates a fully qualified domain from this domain.
    #[inline]
    pub fn as_fully_qualified(&self) -> Result<Self, CDomainNameError> {
        if self.is_fully_qualified() {
            return Ok(self.clone());
        // aka. Would adding a byte exceed the limit?
        } else if self.octets.len() >= Self::MAX_OCTETS as usize {
            return Err(CDomainNameError::LongDomain);
        } else {
            let mut octets = self.octets.clone();
            octets.push(0);
            let mut length_octets = self.length_octets.clone();
            length_octets.push(0);
            return Ok(Self {
                octets,
                length_octets,
            });
        }
    }

    /// as_canonical_name returns the domain name in canonical form. A name in
    /// canonical form is lowercase and fully qualified. See Section 6.2 in RFC
    /// 4034.
    ///
    /// https://www.rfc-editor.org/rfc/rfc4034#section-6
    ///
    /// For the purposes of DNS security, the canonical form of an RR is the
    /// wire format of the RR where:
    ///
    /// 1.  every domain name in the RR is fully expanded (no DNS name
    ///        compression) and fully qualified;
    ///
    /// 2.  all uppercase US-ASCII letters in the owner name of the RR are
    ///        replaced by the corresponding lowercase US-ASCII letters;
    ///
    /// 3.  if the type of the RR is NS, MD, MF, CNAME, SOA, MB, MG, MR, PTR,
    ///        HINFO, MINFO, MX, HINFO, RP, AFSDB, RT, SIG, PX, NXT, NAPTR, KX,
    ///        SRV, DNAME, A6, RRSIG, or NSEC, all uppercase US-ASCII letters in
    ///        the DNS names contained within the RDATA are replaced by the
    ///        corresponding lowercase US-ASCII letters;
    ///
    /// 4.  if the owner name of the RR is a wildcard name, the owner name is
    ///        in its original unexpanded form, including the "*" label (no
    ///        wildcard substitution); and
    ///
    /// 5.  the RR's TTL is set to its original value as it appears in the
    ///        originating authoritative zone or the Original TTL field of the
    ///        covering RRSIG RR.
    #[inline]
    pub fn as_canonical_name(&self) -> Result<Self, CDomainNameError> {
        let mut dn = self.as_lowercase();
        dn.make_fully_qualified()?;
        return Ok(dn);
    }

    #[inline]
    pub fn make_canonical_name(&mut self) -> Result<(), CDomainNameError> {
        self.make_lowercase();
        self.make_fully_qualified()?;
        Ok(())
    }

    #[inline]
    pub fn as_lowercase(&self) -> Self {
        // This will break the length octets. We use the separate vector of length octets to restore
        // them in the primary vector.
        let mut octets = self.octets.to_ascii_lowercase();
        let mut index = 0;
        for length_octet in &self.length_octets {
            octets[index] = *length_octet;
            index += (*length_octet as usize) + 1;
        }
        Self {
            octets,
            length_octets: self.length_octets.clone(),
        }
    }

    #[inline]
    pub fn make_lowercase(&mut self) {
        // This will break the length octets. We use the separate vector of length octets to restore
        // them in the primary vector.
        self.octets.make_ascii_lowercase();
        let mut index = 0;
        for length_octet in &self.length_octets {
            self.octets[index] = *length_octet;
            index += (*length_octet as usize) + 1;
        }
    }

    #[inline]
    pub fn labels<'a, C: 'a + CaseSensitivity>(
        &'a self,
    ) -> impl 'a + DoubleEndedIterator<Item = &'a RefLabel<C>> + ExactSizeIterator<Item = &'a RefLabel<C>>
    {
        CDomainLabelIter::new(self)
    }

    #[inline]
    pub fn search_domains<'a>(
        &'a self,
    ) -> impl 'a + DoubleEndedIterator<Item = Self> + ExactSizeIterator<Item = Self> {
        CDomainSearchNameIter::new(self)
    }
}

struct CDomainLabelIter<'a, C: CaseSensitivity> {
    case: PhantomData<C>,
    name: &'a CDomainName,
    next_octet_index: u8,
    next_length_index: u8,
    last_octet_index: u8,
    last_length_index: u8,
}

impl<'a, C: CaseSensitivity> CDomainLabelIter<'a, C> {
    pub fn new(c_domain_name: &'a CDomainName) -> Self {
        Self {
            case: PhantomData,
            name: &c_domain_name,
            next_octet_index: 0,
            next_length_index: 0,
            last_octet_index: c_domain_name.octets.len() as u8,
            last_length_index: c_domain_name.length_octets.len() as u8,
        }
    }
}

impl<'a, C: 'a + CaseSensitivity> Iterator for CDomainLabelIter<'a, C> {
    type Item = &'a RefLabel<C>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_length_index < self.last_length_index {
            let length = self.name.length_octets[self.next_length_index as usize];
            let label = RefLabel::from_octets(
                &self.name.octets[((self.next_octet_index as usize) + 1)
                    ..((self.next_octet_index as usize) + 1 + (length as usize))],
            );
            self.next_octet_index += length + 1;
            self.next_length_index += 1;
            return Some(label);
        } else {
            return None;
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let size = (self.last_length_index as usize) - (self.next_length_index as usize);
        (size, Some(size))
    }
}

impl<'a, C: 'a + CaseSensitivity> DoubleEndedIterator for CDomainLabelIter<'a, C> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.next_length_index < self.last_length_index {
            let length = self.name.length_octets[(self.last_length_index as usize) - 1];
            let label = RefLabel::from_octets(
                &self.name.octets[((self.last_octet_index as usize) - (length as usize))
                    ..(self.last_octet_index as usize)],
            );
            self.last_octet_index -= length + 1;
            self.last_length_index -= 1;
            return Some(label);
        } else {
            return None;
        }
    }
}

impl<'a, C: 'a + CaseSensitivity> ExactSizeIterator for CDomainLabelIter<'a, C> {}
impl<'a, C: 'a + CaseSensitivity> FusedIterator for CDomainLabelIter<'a, C> {}

struct CDomainSearchNameIter<'a> {
    name: &'a CDomainName,
    next_octet_index: u8,
    next_length_index: u8,
    last_octet_index: u8,
    last_length_index: u8,
}

impl<'a> CDomainSearchNameIter<'a> {
    pub fn new(c_domain_name: &'a CDomainName) -> Self {
        Self {
            name: &c_domain_name,
            next_octet_index: 0,
            next_length_index: 0,
            last_octet_index: c_domain_name.octets.len() as u8,
            last_length_index: c_domain_name.length_octets.len() as u8,
        }
    }
}

impl<'a> Iterator for CDomainSearchNameIter<'a> {
    type Item = CDomainName;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_length_index < self.last_length_index {
            let octet_index = self.next_octet_index;
            let length_octet_index = self.next_length_index;
            self.next_octet_index += self.name.length_octets[length_octet_index as usize] + 1;
            self.next_length_index += 1;
            return Some(CDomainName {
                octets: self.name.octets[(octet_index as usize)..].to_vec(),
                length_octets: TinyVec::from(
                    &self.name.length_octets[(length_octet_index as usize)..],
                ),
            });
        } else {
            return None;
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let size = (self.last_length_index as usize) - (self.next_length_index as usize);
        (size, Some(size))
    }
}

impl<'a> DoubleEndedIterator for CDomainSearchNameIter<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.next_length_index < self.last_length_index {
            self.last_octet_index -=
                self.name.length_octets[(self.last_length_index as usize) - 1] + 1;
            self.last_length_index -= 1;
            return Some(CDomainName {
                octets: self.name.octets[(self.last_octet_index as usize)..].to_vec(),
                length_octets: TinyVec::from(
                    &self.name.length_octets[(self.last_length_index as usize)..],
                ),
            });
        } else {
            return None;
        }
    }
}

impl<'a> ExactSizeIterator for CDomainSearchNameIter<'a> {}
impl<'a> FusedIterator for CDomainSearchNameIter<'a> {}

impl Display for CDomainName {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_root() {
            return write!(f, ".");
        }

        let mut labels = self.labels::<CaseInsensitive>();
        if let Some(label) = labels.next() {
            write!(f, "{label}")?;
        }
        while let Some(labels) = labels.next() {
            write!(f, ".{labels}")?;
        }

        Ok(())
    }
}

impl Debug for CDomainName {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CDomainName: {self}")
    }
}

impl Add for CDomainName {
    type Output = Result<Self, CDomainNameError>;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        if self.is_fully_qualified() {
            // If it is fully qualified, it already ends in a dot.
            // To add a domain to the end, you would need to add a dot between
            // them, resulting in consecutive dots.
            // This might warrant a new error value.
            return Err(CDomainNameError::ConsecutiveDots);
        }

        if (self.serial_length() + rhs.serial_length()) > Self::MAX_OCTETS {
            return Err(CDomainNameError::LongDomain);
        }

        let mut octets = self.octets.clone();
        octets.extend(rhs.octets);
        let mut length_octets = self.length_octets.clone();
        length_octets.extend(rhs.length_octets);

        return Ok(Self {
            octets,
            length_octets,
        });
    }
}

impl CmpDomainName<CDomainName> for CDomainName {
    #[inline]
    fn matches(&self, other: &CDomainName) -> bool {
        if self.serial_length() != other.serial_length() {
            return false;
        }
        if self.label_count() != other.label_count() {
            return false;
        }
        self.labels::<CaseInsensitive>().eq(other.labels())
    }

    #[inline]
    fn is_parent_domain_of(&self, child: &CDomainName) -> bool {
        if self.serial_length() > child.serial_length() {
            return false;
        }
        if self.label_count() > child.label_count() {
            return false;
        }
        // Entire parent is contained by the child (child = subdomain)
        self.labels::<CaseInsensitive>()
            .rev()
            .zip(child.labels().rev())
            .all(|(self_label, child_label)| self_label == child_label)
    }
}

impl CmpDomainName<DomainName> for CDomainName {
    #[inline]
    fn matches(&self, other: &DomainName) -> bool {
        self.matches(&other.domain_name)
    }

    #[inline]
    fn is_parent_domain_of(&self, child: &DomainName) -> bool {
        self.is_parent_domain_of(&child.domain_name)
    }
}

impl ToWire for CDomainName {
    #[inline]
    fn to_wire_format<'a, 'b>(
        &self,
        wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>,
        compression: &mut Option<crate::types::c_domain_name::CompressionMap>,
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
                        || (&self.octets[length_byte_index..] != &[0])
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

impl FromWire for CDomainName {
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
        let mut octets = ArrayVec::<[u8; Self::MAX_OCTETS as usize]>::new();
        let mut length_octets = TinyVec::new();

        let mut final_offset = wire.current_offset();

        while !fully_qualified {
            // Peek at the first byte. It is read differently depending on the value.
            let first_byte = u8::from_wire_format(&mut wire.get_as_read_wire(1)?)?;

            match first_byte & 0b1100_0000 {
                0b0000_0000 => {
                    let label_length = first_byte;
                    if (octets.len() + 1 + (label_length as usize)) > Self::MAX_OCTETS as usize {
                        return Err(CDomainNameError::LongDomain)?;
                    }

                    octets.extend_from_slice(wire.take((label_length as usize) + 1)?);
                    length_octets.push(label_length);
                    fully_qualified = label_length == 0;
                }
                0b1100_0000 => {
                    pointer_count += 1;
                    if pointer_count > Self::MAX_COMPRESSION_POINTERS {
                        return Err(CDomainNameError::TooManyPointers)?;
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
                        return Err(CDomainNameError::ForwardPointers)?;
                    }

                    wire.set_offset(pointer as usize)?;
                }
                _ => {
                    // 0x80 and 0x40 are reserved
                    return Err(CDomainNameError::BadRData)?;
                }
            }
        }

        if pointer_count != 0 {
            wire.set_offset(final_offset as usize)?;
        }

        let octets = octets.to_vec();
        Ok(Self {
            octets,
            length_octets,
        })
    }
}

impl FromPresentation for CDomainName {
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

impl ToPresentation for CDomainName {
    #[inline]
    fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
        out_buffer.push(self.to_string())
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
mod circular_serde_sanity_test {
    use tinyvec::TinyVec;

    use crate::{
        serde::wire::{
            circular_test::gen_test_circular_serde_sanity_test, from_wire::FromWire,
            read_wire::ReadWire, to_wire::ToWire, write_wire::WriteWire,
        },
        types::{
            ascii::AsciiString,
            c_domain_name::CDomainName,
            label::{CaseSensitive, Label, OwnedLabel},
        },
    };

    gen_test_circular_serde_sanity_test!(
        root_record_circular_serde_sanity_test,
        CDomainName::from_utf8(".").unwrap()
    );
    gen_test_circular_serde_sanity_test!(
        root_zone_record_circular_serde_sanity_test,
        CDomainName::from_utf8("com.").unwrap()
    );
    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test,
        CDomainName::from_utf8("www.example.com.").unwrap()
    );
    gen_test_circular_serde_sanity_test!(
        repetitive_1_record_circular_serde_sanity_test,
        CDomainName::from_utf8("www.example.com.www.example.com.").unwrap()
    );
    gen_test_circular_serde_sanity_test!(
        repetitive_2_record_circular_serde_sanity_test,
        CDomainName::from_utf8("www.example.com.www.example.com.www.example.com.").unwrap()
    );

    #[test]
    fn c_domain_name_to_labels() {
        let domain_label_pairs = vec![
            (".", vec![""]),
            ("com.", vec!["com", ""]),
            ("www.example.com.", vec!["www", "example", "com", ""]),
            (
                "www.example.com.www.example.com.",
                vec!["www", "example", "com", "www", "example", "com", ""],
            ),
            (
                "www.example.com.www.example.com.www.example.com.",
                vec![
                    "www", "example", "com", "www", "example", "com", "www", "example", "com", "",
                ],
            ),
        ];
        for (domain, expected_labels) in domain_label_pairs {
            let domain_name = CDomainName::from_utf8(domain).unwrap();
            let expected_labels = expected_labels
                .into_iter()
                .map(|label| {
                    <OwnedLabel<CaseSensitive>>::from_octets(TinyVec::from(
                        AsciiString::from_utf8(label).unwrap().as_slice(),
                    ))
                })
                .collect::<Vec<_>>();
            let actual_labels = domain_name
                .labels()
                .map(|label| label.as_owned())
                .collect::<Vec<_>>();
            assert_eq!(expected_labels, actual_labels);
        }
    }

    #[test]
    fn c_domain_name_to_search_names() {
        let domain_search_name_pairs = vec![
            (".", vec!["."]),
            ("com.", vec!["com.", "."]),
            (
                "www.example.com.",
                vec!["www.example.com.", "example.com.", "com.", "."],
            ),
            (
                "www.example.com.www.example.com.",
                vec![
                    "www.example.com.www.example.com.",
                    "example.com.www.example.com.",
                    "com.www.example.com.",
                    "www.example.com.",
                    "example.com.",
                    "com.",
                    ".",
                ],
            ),
            (
                "www.example.com.www.example.com.www.example.com.",
                vec![
                    "www.example.com.www.example.com.www.example.com.",
                    "example.com.www.example.com.www.example.com.",
                    "com.www.example.com.www.example.com.",
                    "www.example.com.www.example.com.",
                    "example.com.www.example.com.",
                    "com.www.example.com.",
                    "www.example.com.",
                    "example.com.",
                    "com.",
                    ".",
                ],
            ),
        ];
        for (domain, expected_search_names) in domain_search_name_pairs {
            let domain_name = CDomainName::from_utf8(domain).unwrap();
            let expected_search_names = expected_search_names
                .into_iter()
                .map(|search_name| CDomainName::from_utf8(search_name).unwrap())
                .collect::<Vec<_>>();
            let actual_search_names = domain_name.search_domains().collect::<Vec<_>>();
            assert_eq!(expected_search_names, actual_search_names);
        }
    }
}

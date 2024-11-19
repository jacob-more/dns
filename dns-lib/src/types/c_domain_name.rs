use std::{collections::HashMap, error::Error, fmt::{Debug, Display}, ops::Add};

use crate::{serde::{presentation::{errors::TokenError, from_presentation::FromPresentation, parse_chars::{char_token::EscapableChar, escaped_to_escapable::{EscapedCharsEnumerateIter, ParseError}, non_escaped_to_escaped}, to_presentation::ToPresentation}, wire::{from_wire::FromWire, to_wire::ToWire}}, types::ascii::{ascii_char_as_lower, constants::ASCII_PERIOD, AsciiError, AsciiString}};

use super::{ascii::AsciiChar, domain_name::DomainName};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum CDomainNameError {
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
    ParseError(ParseError)
}

impl Error for CDomainNameError {}
impl Display for CDomainNameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fqdn =>              write!(f, "Domain Must Be Fully Qualified: indicates that a domain name does not have a closing dot"),
            Self::LongDomain =>        write!(f, "Domain Name Exceeded {} Wire-Format Octets", CDomainName::MAX_OCTETS),
            Self::LongLabel =>         write!(f, "Label Exceeded {} Wire-Format Octets", Label::MAX_OCTETS),
            Self::LeadingDot =>        write!(f, "Bad Leading Dot: domain name must not begin with a '.' except for in the root zone"),
            Self::ConsecutiveDots =>   write!(f, "Two Consecutive Dots: domain name must not contain two consecutive dots '..' unless one of them is escaped"),
            Self::InternalRootLabel => write!(f, "Internal Root Label: domain name must not a root label unless it is the last label"),
            Self::Buffer =>            write!(f, "Buffer size too small"),
            Self::TooManyPointers =>   write!(f, "Too Many Compression Pointers: the maximum compression pointers permitted is {}", CDomainName::MAX_COMPRESSION_POINTERS),
            Self::ForwardPointers =>   write!(f, "Forward Pointer: domain name pointers can only point backwards. Cannot point forward in the buffer"),
            Self::InvalidPointer =>    write!(f, "Invalid Pointer: domain name pointer cannot use the first two bits. These are reserved"),
            Self::BadRData =>          write!(f, "Bad RData."),
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

pub trait CmpLabel<T>: Sized {
    /// compares the labels, downcasing them as needed, and stops at the first non-equal character.
    fn compare_label(&self, other: &T) -> bool;
}

#[derive(Clone, Default, PartialEq, Eq, Hash)]
pub struct Label<'a> {
    octets: &'a [u8],
}

impl<'a> Label<'a> {
    pub const MAX_OCTETS: u8 = 63;
    pub const MIN_OCTETS: u8 = 0;

    #[inline]
    pub fn is_root(&self) -> bool {
        self.octets.is_empty()
    }

    #[inline]
    fn iter_escaped<'b>(&'b self) -> impl Iterator<Item = EscapableChar> + 'b {
        non_escaped_to_escaped::NonEscapedIntoEscapedIter::from(self.octets.iter().map(|character| *character))
            .map(|character| match character {
                EscapableChar::Ascii(ASCII_PERIOD) => EscapableChar::EscapedAscii(ASCII_PERIOD),
                EscapableChar::Ascii(character) => EscapableChar::Ascii(character),
                _ => character,
            })
    }

    #[inline]
    pub fn as_lower(&self) -> OwnedLabel {
        OwnedLabel {
            octets: self.octets.iter().map(|character| ascii_char_as_lower(*character)).collect()
        }
    }

    #[inline]
    pub fn as_owned_label(&self) -> OwnedLabel {
        OwnedLabel { octets: self.octets.to_vec() }
    }
}

impl<'a, 'b> CmpLabel<Label<'b>> for Label<'a> {
    /// compares the labels. downcasing them as needed, and stops at the first non-equal character.
    #[inline]
    fn compare_label(&self, other: &Label<'b>) -> bool {
        // labels have the same # of characters
        (self.octets.len() == other.octets.len())
        // all characters of labels are equal when downcased
        && self.octets.iter()
            .zip(other.octets.iter())
            .all(|(char1, char2)| ascii_char_as_lower(*char1) == ascii_char_as_lower(*char2))
    }
}

impl<'a> CmpLabel<OwnedLabel> for Label<'a> {
    /// compares the labels. downcasing them as needed, and stops at the first non-equal character.
    #[inline]
    fn compare_label(&self, other: &OwnedLabel) -> bool {
        self.compare_label(&other.as_label())
    }
}

impl<'a> Display for Label<'a> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for character in self.iter_escaped() {
            write!(f, "{character}")?;
        }
        Ok(())
    }
}

impl<'a> Debug for Label<'a> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Label: {self}")
    }
}

#[derive(Clone, Default, PartialEq, Eq, Hash)]
pub struct OwnedLabel {
    octets: Vec<u8>,
}

impl OwnedLabel {
    pub const MAX_OCTETS: u8 = Label::MAX_OCTETS;
    pub const MIN_OCTETS: u8 = Label::MIN_OCTETS;

    #[inline]
    pub fn new_root() -> Self {
        Self { octets: vec![] }
    }

    #[inline]
    pub fn is_root(&self) -> bool {
        self.octets.is_empty()
    }

    #[inline]
    pub fn as_lower(&self) -> OwnedLabel {
        self.as_label().as_lower()
    }

    #[inline]
    pub fn as_label<'a>(&'a self) -> Label<'a> {
        Label { octets: &self.octets }
    }
}

impl<'a> CmpLabel<Label<'a>> for OwnedLabel {
    /// compares the labels. downcasing them as needed, and stops at the first non-equal character.
    #[inline]
    fn compare_label(&self, other: &Label<'a>) -> bool {
        self.as_label().compare_label(other)
    }
}

impl<'a> CmpLabel<OwnedLabel> for OwnedLabel {
    /// compares the labels. downcasing them as needed, and stops at the first non-equal character.
    #[inline]
    fn compare_label(&self, other: &OwnedLabel) -> bool {
        self.as_label().compare_label(&other.as_label())
    }
}

impl Display for OwnedLabel {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_label())
    }
}

impl Debug for OwnedLabel {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "OwnedLabel: {self}")
    }
}

pub trait CmpDomainName<T>: Sized {
    /// determines if two sets of labels are identical, ignoring capitalization
    fn matches(&self, other: &T) -> bool;

    /// is_subdomain checks if child is indeed a child of the parent. If child
    /// and parent are the same domain true is returned as well.
    fn is_subdomain(&self, child: &T) -> bool;
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
    label_count: u8,
    fully_qualified: bool,
    octets: Vec<AsciiChar>,
}

impl CDomainName {
    /// Maximum number of bytes that can make up a domain name, including the length octet.
    pub const MAX_OCTETS: u16 = 256;
    pub const MIN_OCTETS: u16 = 0;

    pub const MAX_COMPRESSION_OFFSET: u16 = 2 << 13;  // We have 14 bits for the compression pointer
    /// This is the maximum number of compression pointers that should occur in a
    /// semantically valid message. Each label in a domain name must be at least one
    /// octet and is separated by a period. The root label won't be represented by a
    /// compression pointer to a compression pointer, hence the -2 to exclude the
    /// smallest valid root label.
    ///
    /// It is possible to construct a valid message that has more compression pointers
    /// than this, and still doesn't loop, by pointing to a previous pointer. This is
    /// not something a well written implementation should ever do, so we leave them
    /// to trip the maximum compression pointer check.
    /// 
    /// TODO: Update this to allow for the true max.
    pub const MAX_COMPRESSION_POINTERS: u16 = ((Self::MAX_OCTETS + 1) / 2) - 2;

    pub fn new_root() -> Self {
        Self { label_count: 1, fully_qualified: true, octets: vec![0] }
    }

    pub fn new(string: &AsciiString) -> Result<Self, CDomainNameError> {
        let mut octets = Vec::new();
        // As long as there are no escaped characters in the string and the name is fully qualified,
        // we expect the length to just about match the number of characters + 1 for the root label.
        octets.reserve(string.len() + 1);
        // The first byte represents the length of the first label.
        octets.push(0);
        let mut label_count = 1;
        let mut length_octet_index = 0;
        let mut fully_qualified = true;

        for escaped_char_result in EscapedCharsEnumerateIter::from(string.iter().map(|character| *character).enumerate()) {
            match (escaped_char_result, (octets.len() - length_octet_index)) {
                (Ok((0, EscapableChar::Ascii(ASCII_PERIOD))), _) => {
                    // leading dots are illegal except for the root zone
                    if string.len() > 1 {
                        return Err(CDomainNameError::LeadingDot);
                    }

                    // The first octet is a zero, which represents an empty label.
                    // Nothing to do.
                    break;
                },
                // consecutive dots are never legal
                (Ok((1.., EscapableChar::Ascii(ASCII_PERIOD))), 1) => return Err(CDomainNameError::ConsecutiveDots),
                // a label is found
                (Ok((1.., EscapableChar::Ascii(ASCII_PERIOD))), 2..) => {
                    length_octet_index = octets.len();
                    label_count += 1;
                    octets.push(0);
                    fully_qualified = true;

                    if octets.len() > Self::MAX_OCTETS as usize {
                        return Err(CDomainNameError::LongDomain);
                    }
                },
                (Ok((_, escapable_char)), _) => {
                    octets.push(escapable_char.into_unescaped_character());
                    octets[length_octet_index] += 1;
                    fully_qualified = false;

                    // TODO: Can we optimize this check? It might be able to do once per label as
                    // long as we still check against the maximum number of octets every time.
                    if octets[length_octet_index] > Label::MAX_OCTETS {
                        return Err(CDomainNameError::LongLabel);
                    }

                    if octets.len() > Self::MAX_OCTETS as usize {
                        return Err(CDomainNameError::LongDomain);
                    }
                },
                (Err(error), _) => return Err(CDomainNameError::ParseError(error)),
            }
        }

        octets.shrink_to_fit();
        Ok(Self { label_count, fully_qualified, octets })
    }

    #[inline]
    pub fn from_utf8(string: &str) -> Result<Self, CDomainNameError> {
        Self::new(
            &AsciiString::from_utf8(string)?
        )
    }

    #[inline]
    pub fn from_labels(labels: Vec<OwnedLabel>) -> Result<Self, CDomainNameError> {
        let total_octets = labels.len() + labels.iter().map(|label| label.octets.len()).sum::<usize>();
        if total_octets > Self::MAX_OCTETS as usize {
            return Err(CDomainNameError::LongDomain);
        }
        let label_count = labels.len() as u8;
        let fully_qualified = labels.last().is_some_and(|label| label.is_root());
        let mut octets = Vec::with_capacity(total_octets);
        for label in labels {
            octets.push(label.octets.len() as u8);
            octets.extend(label.octets);
        }
        Ok(Self { label_count, fully_qualified, octets })
    }

    #[inline]
    pub fn label_count(&self) -> usize {
        self.label_count as usize
    }

    /// A domain name is root if it is made up of only 1 label, that has a length
    /// of zero.
    #[inline]
    pub fn is_root(&self) -> bool {
        // We could also check that the first octet is a zero, but this is much easier and equally
        // correct since the root label counts as a label.
        (self.label_count == 1) && self.fully_qualified
    }

    #[inline]
    pub fn is_fully_qualified(&self) -> bool {
        self.fully_qualified
    }

    /// Converts this domain into a fully qualified domain.
    #[inline]
    pub fn fully_qualified(&mut self) -> Result<(), CDomainNameError> {
        if self.is_fully_qualified() {
            return Ok(());
        // aka. Would adding a byte exceed the limit?
        } else if self.octets.len() >= Self::MAX_OCTETS as usize {
            return Err(CDomainNameError::LongDomain);
        } else {
            self.fully_qualified = true;
            self.octets.push(0);
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
            return Ok(Self {
                label_count: self.label_count + 1,
                fully_qualified: true,
                octets
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
        let mut dn = self.as_lower();
        dn.fully_qualified()?;
        return Ok(dn);
    }
    
    #[inline]
    pub fn canonical_name(&mut self) -> Result<(), CDomainNameError> {
        self.lower();
        self.fully_qualified()?;
        Ok(())
    }

    #[inline]
    pub fn as_lower(&self) -> Self {
        Self {
            label_count: self.label_count,
            fully_qualified: self.fully_qualified,
            octets: self.octets.iter().map(|character| ascii_char_as_lower(*character)).collect()
        }
    }

    #[inline]
    pub fn lower(&mut self) {
        self.octets.iter_mut().for_each(|character| *character = ascii_char_as_lower(*character));
    }

    #[inline]
    pub fn labels<'a>(&'a self) -> impl 'a + Iterator<Item = Label<'a>> {
        CDomainLabelIterator::new(self)
    }

    #[inline]
    pub fn search_domains<'a>(&'a self) -> impl 'a + Iterator<Item = Self> {
        CDomainSearchNameIterator::new(self)
    }
}

struct CDomainLabelIterator<'a> {
    name: &'a CDomainName,
    next_index: usize,
}

impl<'a> CDomainLabelIterator<'a> {
    pub fn new(c_domain_name: &'a CDomainName) -> Self {
        Self {
            name: &c_domain_name,
            next_index: 0,
        }
    }
}

impl<'a> Iterator for CDomainLabelIterator<'a> {
    type Item = Label<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let length = self.name.octets.get(self.next_index)?;
        let label = Label { octets: &self.name.octets[(self.next_index + 1)..(self.next_index + 1 + (*length as usize))] };
        self.next_index += (*length as usize) + 1;
        return Some(label);
    }
}

struct CDomainSearchNameIterator<'a> {
    name: &'a [u8],
    fully_qualified: bool,
    remaining_labels: u8,
}

impl<'a> CDomainSearchNameIterator<'a> {
    pub fn new(c_domain_name: &'a CDomainName) -> Self {
        Self {
            name: &c_domain_name.octets,
            fully_qualified: c_domain_name.fully_qualified,
            remaining_labels: c_domain_name.label_count,
        }
    }
}

impl<'a> Iterator for CDomainSearchNameIterator<'a> {
    type Item = CDomainName;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining_labels == 0 {
            return None;
        }

        let label_count = self.remaining_labels;
        let name = self.name;
        // The `+ 1` is for the length byte.
        let length = name[0] + 1;
        self.name = &name[length as usize..];
        self.remaining_labels -= 1;

        Some(CDomainName {
            label_count,
            fully_qualified: self.fully_qualified,
            octets: name.to_vec(),
        })
    }
}

impl Display for CDomainName {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_root() {
            return write!(f, ".");
        }

        let mut next_index = 0;
        if let Some(length) = self.octets.get(next_index) {
            write!(f, "{}", Label { octets: &self.octets[(next_index + 1)..(next_index + 1 + (*length as usize))] })?;
            next_index += (*length as usize) + 1;
        }
        while let Some(length) = self.octets.get(next_index) {
            write!(f, ".{}", Label { octets: &self.octets[(next_index + 1)..(next_index + 1 + (*length as usize))] })?;
            next_index += (*length as usize) + 1;
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

        if (self.octets.len() + rhs.octets.len()) > (Self::MAX_OCTETS as usize) {
            return Err(CDomainNameError::LongDomain);
        }

        let mut octets = self.octets.clone();
        octets.extend(rhs.octets);

        Ok(Self {
            label_count: self.label_count + rhs.label_count,
            fully_qualified: self.fully_qualified,
            octets
        })
    }
}

impl CmpDomainName<CDomainName> for CDomainName {
    #[inline]
    fn matches(&self, other: &CDomainName) -> bool {
        if self.octets.len() != other.octets.len() {
            return false;
        }
        // Entire parent is contained by the child (child = subdomain)
        self.octets.iter()
            .zip(other.octets.iter())
            .all(|(parent_character, child_character)| ascii_char_as_lower(*parent_character) == ascii_char_as_lower(*child_character))
    }

    #[inline]
    fn is_subdomain(&self, child: &CDomainName) -> bool {
        if self.octets.len() < child.octets.len() {
            return false;
        }
        // Entire parent is contained by the child (child = subdomain)
        self.octets.iter()
            .rev()
            .zip(child.octets.iter().rev())
            .all(|(parent_character, child_character)| ascii_char_as_lower(*parent_character) == ascii_char_as_lower(*child_character))
    }
}

impl CmpDomainName<DomainName> for CDomainName {
    #[inline]
    fn matches(&self, other: &DomainName) -> bool {
        self.matches(&other.domain_name)
    }

    #[inline]
    fn is_subdomain(&self, child: &DomainName) -> bool {
        self.matches(&child.domain_name)
    }
}

impl ToWire for CDomainName {
    #[inline]
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>, compression: &mut Option<crate::types::c_domain_name::CompressionMap>) -> Result<(), crate::serde::wire::write_wire::WriteWireError> where 'a: 'b {
        match compression {
            Some(compression_map) => {
                let mut length_byte_index = 0_usize;
                let mut compression_pointer = None;
                while length_byte_index < self.octets.len() {
                    if let Some(pointer) = compression_map.find_sequence(&self.octets[length_byte_index..]) {
                        // The pointer cannot make use of the first two bits. These are reserved for
                        // use indicating that this label is a pointer. If they are needed for the
                        // pointer itself, the pointer would be corrupted.
                        // 
                        // To solve this issue, we will just not use a pointer if using one would
                        // lead to a corrupted pointer. Easy as that.
                        if (pointer & 0b1100_0000_0000_0000) == 0b0000_0000_0000_0000 {
                            compression_pointer = Some(pointer | 0b1100_0000_0000_0000);
                            break;
                        }
                    } else {
                        // Don't insert malformed pointers. Otherwise, it might overwrite an
                        // existing well-formed pointer.
                        let pointer = (wire.current_len() + length_byte_index) as u16;
                        if (pointer & 0b1100_0000_0000_0000) == 0b0000_0000_0000_0000 {
                            compression_map.insert_sequence(&self.octets[length_byte_index..], pointer);
                        }
                    }
                    length_byte_index += (self.octets[length_byte_index] + 1) as usize;
                }
                wire.write_bytes(&self.octets[..length_byte_index])?;
                compression_pointer.to_wire_format(wire, compression)?;
                Ok(())
            },
            None => {
                wire.write_bytes(&self.octets)?;
                Ok(())
            },
        }
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        self.octets.len() as u16
    }
}

impl FromWire for CDomainName {
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>) -> Result<Self, crate::serde::wire::read_wire::ReadWireError> where Self: Sized, 'a: 'b {
        let mut label_count = 0;
        let mut pointer_count = 0;

        let mut fully_qualified = false;
        let mut octets = Vec::new();

        let mut final_offset = wire.current_offset();

        while !fully_qualified {
            // Peek at the first byte. It is read differently depending on the value.
            let first_byte = u8::from_wire_format(&mut wire.get_as_read_wire(1)?)?;

            match first_byte & 0b1100_0000 {
                0b0000_0000 => {
                    let label_length = u8::from_wire_format(wire)?;
                    if (octets.len() + 1 + label_length as usize) > Self::MAX_OCTETS as usize {
                        return Err(CDomainNameError::LongDomain)?;
                    }

                    fully_qualified = label_length == 0;
                    octets.push(label_length);
                    octets.extend_from_slice(wire.take(label_length as usize)?);
                    label_count += 1;
                },
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
                },
                _ => {
                    // 0x80 and 0x40 are reserved
                    return Err(CDomainNameError::BadRData)?;
                }
            }
        }

        if pointer_count != 0 {
            wire.set_offset(final_offset as usize)?;
        }

        octets.shrink_to_fit();
        Ok(Self { label_count, fully_qualified, octets })
    }
}

impl FromPresentation for CDomainName {
    #[inline]
    fn from_token_format<'a, 'b, 'c, 'd>(tokens: &'c [&'a str]) -> Result<(Self, &'d [&'a str]), TokenError<'b>> where Self: Sized, 'a: 'b, 'c: 'd, 'c: 'd {
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
        Self { map: HashMap::new() }
    }

    #[inline]
    pub fn insert_sequence(&mut self, domain: &[u8], offset: u16) {
        self.map.insert(domain.to_vec(), offset);
    }

    #[inline]
    pub fn find_sequence(&self, domain: &[u8]) -> Option<u16> {
        self.map.get(domain).cloned()
    }
}

#[cfg(test)]
mod circular_serde_sanity_test {
    use crate::serde::wire::circular_test::gen_test_circular_serde_sanity_test;
    use super::CDomainName;

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
}

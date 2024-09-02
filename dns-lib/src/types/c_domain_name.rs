use std::{error::Error, fmt::{Debug, Display}, ops::Add};

use tinyvec::{tiny_vec, TinyVec};

use crate::{serde::{presentation::{errors::TokenError, from_presentation::FromPresentation, parse_chars::{char_token::EscapableChar, escaped_to_escapable::{EscapedCharsEnumerateIter, EscapedToEscapableIter, ParseError}, non_escaped_to_escaped}, to_presentation::ToPresentation}, wire::{from_wire::FromWire, to_wire::ToWire}}, types::ascii::{ascii_char_as_lower, constants::ASCII_PERIOD, AsciiError, AsciiString}};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum CDomainNameError {
    Fqdn,
    LongDomain,
    LongLabel,
    LeadingDot,
    ConsecutiveDots,
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
            Self::Fqdn =>            write!(f, "Domain Must Be Fully Qualified: indicates that a domain name does not have a closing dot"),
            Self::LongDomain =>      write!(f, "Domain Name Exceeded {} Wire-Format Octets", CDomainName::MAX_OCTETS),
            Self::LongLabel =>       write!(f, "Label Exceeded {} Wire-Format Octets", Label::MAX_OCTETS),
            Self::LeadingDot =>      write!(f, "Bad Leading Dot: domain name must not begin with a '.' except for in the root zone"),
            Self::ConsecutiveDots => write!(f, "Two Consecutive Dots: domain name must not contain two consecutive dots '..' unless one of them is escaped"),
            Self::Buffer =>          write!(f, "Buffer size too small"),
            Self::TooManyPointers => write!(f, "Too Many Compression Pointers: the maximum compression pointers permitted is {}", CDomainName::MAX_COMPRESSION_POINTERS),
            Self::ForwardPointers => write!(f, "Forward Pointer: domain name pointers can only point backwards. Cannot point forward in the buffer"),
            Self::InvalidPointer =>  write!(f, "Invalid Pointer: domain name pointer cannot use the first two bits. These are reserved"),
            Self::BadRData =>        write!(f, "Bad RData."),
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

#[derive(Clone, Default, PartialEq, Eq, Hash, Debug)]
pub struct Label {
    ascii: AsciiString,
}

impl Label {
    pub const MAX_OCTETS: usize = 63;
    pub const MIN_OCTETS: usize = 0;

    #[inline]
    pub fn new(string: &AsciiString) -> Result<Self, CDomainNameError> {
        let mut ascii = Vec::with_capacity(string.len());
        for character in EscapedToEscapableIter::from(string.iter().map(|character| *character)) {
            match character {
                Ok(EscapableChar::Ascii(character)) => ascii.push(character),
                Ok(EscapableChar::EscapedAscii(character)) => ascii.push(character),
                Ok(EscapableChar::EscapedOctal(character)) => ascii.push(character),
                Err(error) => return Err(CDomainNameError::ParseError(error)),
            }
        }

        // +1 for the byte length of the string.
        if ascii.len() + 1 > Self::MAX_OCTETS {
            return Err(CDomainNameError::LongLabel);
        }

        ascii.shrink_to_fit();
        Ok(Self { ascii: AsciiString::from(&ascii) })
    }

    #[inline]
    pub fn new_root() -> Self {
        Self { ascii: AsciiString::new_empty() }
    }

    #[inline]
    pub fn from_utf8(string: &str) -> Result<Self, CDomainNameError> {
        Self::new(
            &AsciiString::from_utf8(string)?
        )
    }

    #[inline]
    pub fn as_lower(&self) -> Self {
        Self { ascii: self.ascii.as_lower() }
    }
    
    #[inline]
    pub fn lower(&mut self) {
        self.ascii.lower()
    }

    #[inline]
    pub fn is_root(&self) -> bool {
        self.ascii.is_empty()
    }

    /// compares the labels. downcasing them as needed, and stops at the first non-equal character.
    #[inline]
    pub fn compare_domain_name_label(label1: &Self, label2: &Self) -> bool {
        // labels have the same # of characters
        (label1.ascii.len() == label2.ascii.len())
        // all characters of labels are equal when downcased
        && label1.ascii.iter()
            .zip(label2.ascii.iter())
            .all(|(char1, char2)| ascii_char_as_lower(*char1) == ascii_char_as_lower(*char2))
    }

    #[inline]
    fn iter_escaped<'a>(&'a self) -> impl Iterator<Item = EscapableChar> + 'a {
        non_escaped_to_escaped::NonEscapedIntoEscapedIter::from(self.ascii.iter().map(|character| *character))
            .map(|character| match character {
                EscapableChar::Ascii(ASCII_PERIOD) => EscapableChar::EscapedAscii(ASCII_PERIOD),
                EscapableChar::Ascii(character) => EscapableChar::Ascii(character),
                _ => character,
            })
    }
}

impl Display for Label {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for character in self.iter_escaped() {
            write!(f, "{}", character)?;
        }
        Ok(())
    }
}

impl ToWire for Label {
    #[inline]
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>, compression: &mut Option<crate::serde::wire::compression_map::CompressionMap>) -> Result<(), crate::serde::wire::write_wire::WriteWireError> where 'a: 'b {
        (self.ascii.len() as u8).to_wire_format(wire, compression)?;
        self.ascii.to_wire_format(wire, compression)
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        // The string length + 1 for the length byte.
        1 + (self.ascii.len() as u16)
    }
}

impl FromWire for Label {
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>) -> Result<Self, crate::serde::wire::read_wire::ReadWireError> where Self: Sized, 'a: 'b {
        let label_length = u8::from_wire_format(wire)?;

        match label_length & 0b1100_0000 {
            0b0000_0000 => {
                if wire.current_len() < (label_length as usize) {
                    return Err(CDomainNameError::Buffer)?;
                }
        
                if (label_length as usize) > Self::MAX_OCTETS  {
                    return Err(CDomainNameError::LongLabel)?;
                }

                // This AsciiString from_wire_format fully consumes the buffer.
                // Need to make sure that it is only fed what it needs.
                let string = AsciiString::from_wire_format(
                    &mut wire.take_as_read_wire(label_length as usize)?
                )?;
        
                return Ok(Self { ascii: string });
            },
            0b1100_0000 => {
                return Err(crate::serde::wire::read_wire::ReadWireError::FormatError(
                    String::from("the label is a pointer but is being deserialized as a string"),
                ));
            },
            _ => {
                // 0x80 and 0x40 are reserved
                return Err(CDomainNameError::BadRData)?;
            }
        }
    }
}

pub trait Labels: Sized {
    fn from_labels(labels: &[Label]) -> Self;
    fn as_labels<'a>(&'a self) -> &'a [Label];
    fn to_labels(&self) -> Vec<Label>;

    #[inline]
    fn iter_labels(&self) -> impl DoubleEndedIterator<Item = &Label> + ExactSizeIterator<Item = &Label> {
        self.as_labels().iter()
    }

    #[inline]
    fn label_count(&self) -> usize {
        self.as_labels().len()
    }

    /// A domain name is root if it is made up of only 1 label, that has a length
    /// of zero.
    #[inline]
    fn is_root(&self) -> bool {
        match &self.as_labels() {
            &[label] => label.is_root(),
            _ => false,
        }
    }

    #[inline]
    fn search_domains<'a>(&'a self) -> impl 'a + DoubleEndedIterator<Item = Self> + ExactSizeIterator<Item = Self> {
        self.iter_labels()
            .enumerate()
            .map(|(index, _)| Self::from_labels(&self.as_labels()[index..]))
    }

    /// counts the number of labels the two domains have in common, starting from the right. Stops
    /// at the first non-equal pair of labels.
    #[inline]
    fn compare_domain_name<T>(&self, other: &T) -> usize where T: Labels {
        let compar_iter = self.iter_labels()
            .rev()
            .zip(other.iter_labels().rev())
            .map(|(label1, label2)| Label::compare_domain_name_label(label1, label2));
    
        let mut counter: usize = 0;
        for matched in compar_iter {
            if matched {
                counter += 1;
            } else {
                return counter;
            }
        }

        return counter;
    }

    #[inline]
    fn matches<T>(&self, other: &T) -> bool where T: Labels {
        // Same number of labels
        (self.label_count() == other.label_count())
        // all of the labels match
        && self.iter_labels()
            .rev()
            .zip(other.iter_labels().rev())
            .all(|(label1, label2)| Label::compare_domain_name_label(label1, label2))
    }

    /// is_subdomain checks if child is indeed a child of the parent. If child
    /// and parent are the same domain true is returned as well.
    #[inline]
    fn is_subdomain<T>(&self, child: &T) -> bool where T: Labels {
        // Entire parent is contained by the child (child = subdomain)
        return Self::compare_domain_name(self, child) == self.label_count();
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
    labels: TinyVec<[Label; 5]>
}

impl CDomainName {
    pub const MAX_OCTETS: u16 = 255;
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

    pub fn new(string: &AsciiString) -> Result<Self, CDomainNameError> {
        let mut labels = TinyVec::new();

        let mut label_start = 0;
        let mut was_dot = false;
        let mut serial_length: u16 = 0;
        for escaped_char_result in EscapedCharsEnumerateIter::from(string.iter().map(|character| *character).enumerate()) {
            match (escaped_char_result, string.len(), was_dot) {
                // leading dots are not legal except for the root zone
                (Ok((0, EscapableChar::Ascii(ASCII_PERIOD))), 1, _) => labels.push(Label::new_root()),
                (Ok((0, EscapableChar::Ascii(ASCII_PERIOD))), 2.., _) => return Err(CDomainNameError::LeadingDot),
                // consecutive dots are never legal
                (Ok((1.., EscapableChar::Ascii(ASCII_PERIOD))), _, true) => return Err(CDomainNameError::ConsecutiveDots),
                // a label is found
                (Ok((index @ 1.., EscapableChar::Ascii(ASCII_PERIOD))), string_len, false) => {
                    let label = Label::new(&string.from_range(label_start, index))?;
                    serial_length += label.serial_length();

                    if serial_length > Self::MAX_OCTETS {
                        return Err(CDomainNameError::LongDomain);
                    }

                    labels.push(label);
                    label_start = index + 1;
                    was_dot = true;

                    // If this is the last character in the buffer, then make sure the root label is
                    // appended as well.
                    if index == string_len-1 {
                        let label = Label::new_root();
                        serial_length += label.serial_length();

                        if serial_length > Self::MAX_OCTETS {
                            return Err(CDomainNameError::LongDomain);
                        }
                        labels.push(label);
                    }
                },
                (Ok((index, _)), string_len, _) => {
                    // If this is the last character in the buffer, then this is also the end of the
                    // label.
                    if index == string_len-1 {
                        let label = Label::new(&string.from_range(label_start, index+1))?;
                        serial_length += label.serial_length();
    
                        if serial_length > Self::MAX_OCTETS {
                            return Err(CDomainNameError::LongDomain);
                        }
    
                        labels.push(label);
                    }
                    was_dot = false;
                },
                (Err(error), _, _) => return Err(CDomainNameError::ParseError(error)),
            }
        }

        Ok(Self { labels })
    }

    #[inline]
    pub fn new_root() -> Self {
        Self { labels: tiny_vec![{Label::new_root()}] }
    }

    #[inline]
    pub fn from_utf8(string: &str) -> Result<Self, CDomainNameError> {
        Self::new(
            &AsciiString::from_utf8(string)?
        )
    }

    #[inline]
    pub fn is_fully_qualified(&self) -> bool {
        match self.labels.last() {
            Some(last_label) => last_label.is_root(),
            None => false,
        }
    }

    /// Converts this domain into a fully qualified domain.
    #[inline]
    pub fn fully_qualified(&mut self) {
        if !self.is_fully_qualified() {
            self.labels.push(Label::new_root());
        }
    }

    /// Creates a fully qualified domain from this domain.
    #[inline]
    pub fn as_fully_qualified(&self) -> Self {
        let mut copy = self.clone();
        if !self.is_fully_qualified() {
            copy.labels.push(Label::new_root());
        }
        return copy;
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
    /// 	   compression) and fully qualified;
    /// 
    /// 2.  all uppercase US-ASCII letters in the owner name of the RR are
    /// 	   replaced by the corresponding lowercase US-ASCII letters;
    /// 
    /// 3.  if the type of the RR is NS, MD, MF, CNAME, SOA, MB, MG, MR, PTR,
    /// 	   HINFO, MINFO, MX, HINFO, RP, AFSDB, RT, SIG, PX, NXT, NAPTR, KX,
    /// 	   SRV, DNAME, A6, RRSIG, or NSEC, all uppercase US-ASCII letters in
    /// 	   the DNS names contained within the RDATA are replaced by the
    /// 	   corresponding lowercase US-ASCII letters;
    /// 
    /// 4.  if the owner name of the RR is a wildcard name, the owner name is
    /// 	   in its original unexpanded form, including the "*" label (no
    /// 	   wildcard substitution); and
    /// 
    /// 5.  the RR's TTL is set to its original value as it appears in the
    /// 	   originating authoritative zone or the Original TTL field of the
    /// 	   covering RRSIG RR.
    #[inline]
    pub fn as_canonical_name(&self) -> Self {
        let mut dn = self.as_lower();
        dn.fully_qualified();
        return dn;
    }
    
    #[inline]
    pub fn canonical_name(&mut self) {
        self.lower();
        self.fully_qualified();
    }

    #[inline]
    pub fn as_lower(&self) -> Self {
        let mut lower_labels = TinyVec::with_capacity(self.labels.len());
        for label in &self.labels {
            lower_labels.push(label.as_lower());
        }
        return Self { labels: lower_labels };
    }

    #[inline]
    pub fn lower(&mut self) {
        self.labels.iter_mut()
                   .for_each(|label| label.lower());
    }
}

impl Labels for CDomainName {
    #[inline]
    fn from_labels(labels: &[Label]) -> Self {
        // TODO: validate the label input to make sure it is actually correct and valid.
        let mut labels_vec = TinyVec::with_capacity(labels.len());
        labels_vec.extend_from_slice(labels);
        Self { labels: labels_vec }
    }

    #[inline]
    fn as_labels<'a>(&'a self) -> &'a [Label] {
        &self.labels
    }

    #[inline]
    fn to_labels(&self) -> Vec<Label> {
        self.labels.to_vec()
    }
}

impl Display for CDomainName {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_root() {
            return write!(f, ".")
        }
        let mut labels = self.labels.iter();
        match labels.next() {
            None => return write!(f, "null"),
            Some(label) => write!(f, "{}", label)?,
        };
        for label in labels {
            write!(f, ".{}", label)?;
        }
        Ok(())
    }
}

impl Debug for CDomainName {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Domain Name: ")?;
        write!(f, "{}", self)
    }
}

impl Add for CDomainName {
    type Output = Result<Self, CDomainNameError>;

    #[inline]
    fn add(mut self, rhs: Self) -> Self::Output {
        if self.is_fully_qualified() {
            // If it is fully qualified, it already ends in a dot.
            // To add a domain to the end, you would need to add a dot between
            // them, resulting in consecutive dots.
            // This could also warrant a new error if I want to.
            return Err(CDomainNameError::ConsecutiveDots);
        }
        
        self.labels.extend(rhs.labels);

        let domain_name = Self {
            labels: self.labels
        };

        if domain_name.serial_length() > Self::MAX_OCTETS {
            return Err(CDomainNameError::LongDomain);
        }

        Ok(domain_name)
    }
}

impl ToWire for CDomainName {
    #[inline]
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>, compression: &mut Option<crate::serde::wire::compression_map::CompressionMap>) -> Result<(), crate::serde::wire::write_wire::WriteWireError> where 'a: 'b {
        match compression {
            Some(compression_map) => {
                for (i, label) in self.labels.iter().enumerate() {
                    let labels_tail = &self.labels[i..];
                    match compression_map.find_from_slice_labels(labels_tail) {
                        Some(pointer) => {
                            // The pointer cannot make use of the first two bits.
                            // These are reserved for use indicating that this
                            // label is a pointer. If they are needed for the pointer
                            // itself, the pointer would be corrupted.
                            // 
                            // To solve this issue, we will just not use a pointer if
                            // using one would lead to a corrupted pointer. Easy as that.
                            if (pointer & 0b1100_0000_0000_0000) != 0b0000_0000_0000_0000 {
                                label.to_wire_format(wire, &mut None)?;
                            } else {
                                (pointer | 0b1100_0000_0000_0000).to_wire_format(wire, &mut None)?;
                                break;
                            }
                        },
                        None => {
                            // Note: the length of the wire === a pointer to the index after the end of the wire.
                            //       In this case, we want a pointer to the index we are about to write, so this should work.
                            compression_map.insert_slice_labels(labels_tail, wire.current_len() as u16);
                            label.to_wire_format(wire, &mut None)?;
                        },
                    }
                }
                Ok(())
            },
            None => {
                for label in &self.labels {
                    label.to_wire_format(wire, compression)?;
                }
                Ok(())
            },
        }
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        self.labels.iter().map(|label| label.serial_length() as u16).sum()
    }
}

impl FromWire for CDomainName {
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>) -> Result<Self, crate::serde::wire::read_wire::ReadWireError> where Self: Sized, 'a: 'b {
        let mut labels = TinyVec::new();
        let mut serial_length = 0;
        let mut pointer_count = 0;

        let mut root_found = false;
        let mut label: Label;

        let mut final_offset = wire.current_offset();

        while !root_found {
            // Here, we read the first byte of each label but we don't want to save it. We are
            // just using it to see what type of label we are reading. The actual deserialization
            // of the length and string will be done by the Label.
            let first_byte = u8::from_wire_format(
                &mut wire.get_as_read_wire(1)?
            )?;

            match first_byte & 0b1100_0000 {
                0b0000_0000 => {
                    label = Label::from_wire_format(wire)?;
            
                    serial_length += label.serial_length();
                    if serial_length > Self::MAX_OCTETS {
                        return Err(CDomainNameError::LongDomain)?;
                    }
                    
                    root_found = label.is_root();
                    labels.push(label);

                    // If the end of the wire is reached, cannot keep reading labels.
                    if wire.current_len() == 0 {
                        break;
                    }
                },
                0b1100_0000 => {
                    pointer_count += 1;
                    if pointer_count > Self::MAX_COMPRESSION_POINTERS {
                        return Err(CDomainNameError::TooManyPointers)?;
                    }

                    let pointer_bytes: u16;
                    pointer_bytes = u16::from_wire_format(wire)?;

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

        Ok(Self { labels })
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

#[cfg(test)]
mod circular_serde_sanity_test {
    use crate::serde::wire::circular_test::gen_test_circular_serde_sanity_test;
    use super::CDomainName;

    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test,
        CDomainName::from_utf8("www.example.com.").unwrap()
    );
    gen_test_circular_serde_sanity_test!(
        root_record_circular_serde_sanity_test,
        CDomainName::from_utf8(".").unwrap()
    );
    gen_test_circular_serde_sanity_test!(
        root_zone_record_circular_serde_sanity_test,
        CDomainName::from_utf8("com.").unwrap()
    );
}

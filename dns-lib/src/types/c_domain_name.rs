use std::{fmt::{Display, Debug}, error::Error, ops::Add};

use crate::{types::ascii::{AsciiString, constants::{ASCII_PERIOD, EMPTY_ASCII_STRING}, AsciiError, ascii_char_as_lower}, serde::{wire::{to_wire::ToWire, from_wire::FromWire}, presentation::from_presentation::FromPresentation}};

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
            Self::AsciiError(error) => write!(f, "{}", error),
        }
    }
}
impl From<AsciiError> for CDomainNameError {
    fn from(value: AsciiError) -> Self {
        Self::AsciiError(value)
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Label {
    label: AsciiString,
}

impl Label {
    pub const MAX_OCTETS: usize = 63;
    pub const MIN_OCTETS: usize = 0;

    pub const ROOT_LABEL: Self = Self { label: EMPTY_ASCII_STRING };

    #[inline]
    pub fn new(string: &AsciiString) -> Result<Self, CDomainNameError> {
        // +1 for the byte length of the string.
        if string.len() + 1 > Self::MAX_OCTETS {
            return Err(CDomainNameError::LongLabel);
        }

        Ok(Self { label: string.clone() })
    }

    #[inline]
    pub fn from_utf8(string: &str) -> Result<Self, CDomainNameError> {
        Self::new(
            &AsciiString::from_utf8(string)?
        )
    }

    #[inline]
    pub fn as_lower(&self) -> Self {
        Self { label: self.label.as_lower() }
    }
    
    #[inline]
    pub fn lower(&mut self) {
        self.label.lower()
    }

    #[inline]
    pub fn is_root(&self) -> bool {
        self.label.is_empty()
    }

    /// compares the labels. downcasing them as needed, and stops at the first non-equal character.
    #[inline]
    pub fn compare_domain_name_label(label1: &Self, label2: &Self) -> bool {
        // labels have the same # of characters
        (label1.label.len() == label2.label.len())
        // all characters of labels are equal when downcased
        && label1.label.iter()
            .zip(label2.label.iter())
            .all(|(char1, char2)| ascii_char_as_lower(*char1) == ascii_char_as_lower(*char2))
    }

    #[inline]
    fn serial_length(&self) -> usize {
        // The string length + 1 for the length byte.
        (self.label.len() + 1).into()
    }
}

impl Display for Label {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label)
    }
}

impl ToWire for Label {
    #[inline]
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>, compression: &mut Option<crate::serde::wire::compression_map::CompressionMap>) -> Result<(), crate::serde::wire::write_wire::WriteWireError> where 'a: 'b {
        (self.label.len() as u8).to_wire_format(wire, compression)?;
        self.label.to_wire_format(wire, compression)
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        // The string length + 1 for the length byte.
        1 + (self.label.len() as u16)
    }
}

impl FromWire for Label {
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>) -> Result<Self, crate::serde::wire::read_wire::ReadWireError> where Self: Sized, 'a: 'b {
        let label_length = u8::from_wire_format(wire)?;

        match label_length & 0b1100_0000 {
            0b0000_0000 => {
                if wire.current_state_len() < (label_length as usize) {
                    return Err(CDomainNameError::Buffer)?;
                }
        
                if (label_length as usize) > Self::MAX_OCTETS  {
                    return Err(CDomainNameError::LongLabel)?;
                }

                // This AsciiString from_wire_format fully consumes the buffer.
                // Need to make sure that it is only fed what it needs.
                let mut ascii_wire = wire.section_from_current_state(Some(0), Some(label_length as usize))?;
                let string = AsciiString::from_wire_format(&mut ascii_wire)?;
                wire.shift(label_length as usize)?;
        
                return Ok(Self { label: string });
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
    labels: Vec<Label>
}

impl CDomainName {
    pub const MAX_OCTETS: usize = 255;
    pub const MIN_OCTETS: usize = 0;

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
    pub const MAX_COMPRESSION_POINTERS: usize = ((Self::MAX_OCTETS + 1) / 2) - 2;
    
    pub fn new(string: &AsciiString) -> Result<Self, CDomainNameError> {
        let mut labels: Vec<Label> = Vec::new();

        let mut label_start = 0;
        let mut was_dot = false;
        let mut serial_length: usize = 0;
        for (index, character) in string.iter().enumerate() {
            match *character {
                ASCII_PERIOD => {
                    // leading dots are not legal except for the root zone
                    if (index == 0) && (string.len() > 1) {
                        return Err(CDomainNameError::LeadingDot);
                    }

                    // two dots back to back is not legal
                    if was_dot {
                        return Err(CDomainNameError::ConsecutiveDots);
                    }

                    match Label::new(&string.from_range(label_start, index)) { //< Note: exclusive of the '.'
                        Ok(label) => {
                            serial_length += label.serial_length();

                            if serial_length > CDomainName::MAX_OCTETS {
                                return Err(CDomainNameError::LongDomain);
                            }
        
                            labels.push(label);
                            
                            label_start = index + 1;
                            was_dot = true;
                        },
                        Err(error) => {
                            return Err(error);
                        },
                    }
                }
                _ => {
                    was_dot = false;
                }
            }
        }

        let last_index = string.len();
        if last_index > label_start {
            if last_index - label_start > Label::MAX_OCTETS {
                return Err(CDomainNameError::LongLabel);
            }

            match Label::new(&string.from_range(label_start, last_index)) { //< Note: exclusive of the '.'
                Ok(label) => {
                    serial_length += label.serial_length();

                    if serial_length > CDomainName::MAX_OCTETS {
                        return Err(CDomainNameError::LongDomain);
                    }
        
                    labels.push(label);
                },
                Err(error) => {
                    return Err(error);
                },
            }
        }
        
        // TODO: I don't like the wat this last part is done. It has too many
        //       cases and ways to break. I believe there is probably a cleaner
        //       and less error prone way to do this. This way works, and we can
        //       keep it, but if I get a chance, it might be worth revisiting.
        //
        // If it is a root domain, then it should end with a "."
        // For completeness, a root label is a label with an empty string and
        // size of zero.
        if let Some(character) = string.last() {
            if *character == ASCII_PERIOD {
                serial_length += Label::ROOT_LABEL.serial_length();

                if serial_length > CDomainName::MAX_OCTETS {
                    return Err(CDomainNameError::LongDomain);
                }
    
                if let Some(last_label) = labels.last() {
                    // If the current set of labels already ends in the root label,
                    // then we cannot add it a second time.
                    if *last_label != Label::ROOT_LABEL {
                        labels.push(Label::ROOT_LABEL);
                    }
                } else {
                    labels.push(Label::ROOT_LABEL);
                }
            }
        }

        Ok(Self { labels: labels })
    }

    #[inline]
    pub fn from_utf8(string: &str) -> Result<Self, CDomainNameError> {
        Self::new(
            &AsciiString::from_utf8(string)?
        )
    }

    #[inline]
    pub fn from_labels(labels: &[Label]) -> Self {
        // TODO: validate the label input to make sure it is actually correct and valid.
        let mut labels_vec: Vec<Label> = Vec::with_capacity(labels.len());
        labels_vec.extend_from_slice(labels);
        Self { labels: labels_vec }
    }

    #[inline]
    pub fn is_fully_qualified(&self) -> bool {
        match self.labels.last() {
            Some(last_label) => last_label == &Label::ROOT_LABEL,
            None => false,
        }
    }

    /// Converts this domain into a fully qualified domain.
    #[inline]
    pub fn fully_qualified(&mut self) {
        if !self.is_fully_qualified() {
            self.labels.push(Label::ROOT_LABEL);
        }
    }

    /// Creates a fully qualified domain from this domain.
    #[inline]
    pub fn as_fully_qualified(&self) -> Self {
        let mut copy = self.clone();
        if !self.is_fully_qualified() {
            copy.labels.push(Label::ROOT_LABEL);
        }
        return copy;
    }

    #[inline]
    pub fn label_count(&self) -> usize {
        self.labels.len()
    }

    /// A domain name is root if it is made up of only 1 label, that has a length
    /// of zero.
    #[inline]
    pub fn is_root(&self) -> bool {
        match self.labels.last() {
            Some(last_label) => (self.label_count() == 1) && (last_label == &Label::ROOT_LABEL),
            None => false,
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
        let mut lower_labels = Vec::with_capacity(self.labels.len());
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

    /// is_subdomain checks if child is indeed a child of the parent. If child
    /// and parent are the same domain true is returned as well.
    #[inline]
    pub fn is_subdomain(&self, child: &Self) -> bool {
        // Entire parent is contained by the child (child = subdomain)
        return Self::compare_domain_name(self, child) == self.label_count();
    }

    #[inline]
    pub fn as_vec(&self) -> &Vec<Label> {
        &self.labels
    }

    #[inline]
    pub fn as_slice(&self) -> &[Label] {
        &self.labels
    }

    #[inline]
    pub fn search_domains<'a>(&'a self) -> impl 'a + Iterator<Item = CDomainName> {
        self.labels.iter()
            .enumerate()
            .map(|(index, _)| CDomainName::from_labels(&self.labels[index..]))
    }

    /// counts the number of labels the two domains have in common, starting from the right. Stops
    /// at the first non-equal pair of labels.
    #[inline]
    pub fn compare_domain_name(domain1: &Self, domain2: &Self) -> usize {
        let compar_iter = domain1.labels.iter()
            .rev()
            .zip(domain2.labels.iter().rev())
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
    pub fn matches(domain1: &Self, domain2: &Self) -> bool {
        // Same number of labels
        (domain1.label_count() == domain2.label_count())
        // all of the labels match
        && domain1.labels.iter()
            .rev()
            .zip(domain2.labels.iter().rev())
            .all(|(label1, label2)| Label::compare_domain_name_label(label1, label2))
    }

    #[inline]
    fn serial_length(&self) -> usize {
        self.labels.iter().map(|label| label.serial_length()).sum()
    }
}

impl Display for CDomainName {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.labels.get(0) {
            Option::None => return write!(f, "null"),
            Option::Some(label) => write!(f, "{}", label)?,
        };
        for label in self.labels.iter().skip(1) {
            write!(f, ".{}", label)?;
        }
        Ok(())
    }
}

impl Debug for CDomainName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Domain Name: ")?;
        for label in &self.labels[..(self.labels.len()-1)] {
            write!(f, "{}.", label)?;
        }
        write!(f, "{}", self.labels[self.labels.len()-1])
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

        if domain_name.serial_length() > CDomainName::MAX_OCTETS {
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
                            }
                        },
                        None => {
                            // Note: the length of the wire === a pointer to the index after the end of the wire.
                            //       In this case, we want a pointer to the index we are about to write, so this should work.
                            compression_map.insert_slice_labels(labels_tail, wire.len() as u16);
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
        let mut labels: Vec<Label> = Vec::new();
        let mut serial_length = 0;
        let mut pointer_count = 0;

        let mut root_found = false;
        let mut label: Label;

        let mut final_offset = wire.current_state_offset();

        while !root_found {
            // Here, we read the first byte of each label but we don't want to save it. We are
            // just using it to see what type of label we are reading. The actual deserialization
            // of the length and string will be done by the Label.
            let first_byte = u8::from_wire_format(
                &mut wire.section_from_current_state(Some(0), Some(1))?,
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
                    if wire.current_state_len() == 0 {
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
                        final_offset = wire.current_state_offset();
                    }

                    let pointer = pointer_bytes & 0b0011_1111_1111_1111;
                    // The pointer must point backwards in the wire. Forward pointers
                    // are forbidden.
                    if (pointer as usize) > wire.current_state_offset() {
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

        Ok(Self { labels: labels })
    }
}

impl FromPresentation for CDomainName {
    #[inline]
    fn from_token_format<'a>(token: &'a str) -> Result<Self, crate::serde::presentation::errors::TokenError> where Self: Sized {
        Ok(Self::new(
            &AsciiString::from_token_format(token)?
        )?)
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
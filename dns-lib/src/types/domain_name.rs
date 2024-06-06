use std::{fmt::Display, ops::Add, error::Error};

use dns_macros::ToPresentation;

use crate::{serde::{presentation::{errors::TokenError, from_presentation::FromPresentation}, wire::{from_wire::FromWire, to_wire::ToWire}}, types::{ascii::AsciiString, c_domain_name::{CDomainName, CDomainNameError, Label}}};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum DomainNameError {
    CDomainNameError(CDomainNameError),
}

impl Error for DomainNameError {}
impl Display for DomainNameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CDomainNameError(error) => write!(f, "{}", error),
        }
    }
}
impl From<CDomainNameError> for DomainNameError {
    fn from(value: CDomainNameError) -> Self {
        Self::CDomainNameError(value)
    }
}

/// This is an incompressible domain name. This should be used in any place where domain name compression is not
/// allowed. It is still able to decompress a domain name but it will not compress it when
/// serializing the name. If compression is required, use the CDomainName.
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToPresentation)]
pub struct DomainName {
    domain_name: CDomainName,
}

impl DomainName {
    #[inline]
    pub fn new(string: &AsciiString) -> Result<Self, DomainNameError> {
        Ok(Self { domain_name: CDomainName::new(string)? })
    }

    #[inline]
    pub fn from_utf8(string: &str) -> Result<Self, DomainNameError> {
        Ok(Self { domain_name: CDomainName::from_utf8(string)? })
    }

    #[inline]
    pub fn from_labels(labels: &[Label]) -> Self {
        Self { domain_name: CDomainName::from_labels(labels) }
    }

    #[inline]
    pub fn is_fully_qualified(&self) -> bool {
        self.domain_name.is_fully_qualified()
    }

    /// Converts this domain into a fully qualified domain.
    #[inline]
    pub fn fully_qualified(&mut self) {
        self.domain_name.fully_qualified()
    }

    /// Creates a fully qualified domain from this domain.
    #[inline]
    pub fn as_fully_qualified(&self) -> Self {
        Self { domain_name: self.domain_name.as_fully_qualified() }
    }

    #[inline]
    pub fn label_count(&self) -> usize {
        self.domain_name.label_count()
    }

    /// A domain name is root if it is made up of only 1 label, that has a length
    /// of zero.
    #[inline]
    pub fn is_root(&self) -> bool {
        self.domain_name.is_root()
    }

    #[inline]
    pub fn as_canonical_name(&self) -> Self {
        Self { domain_name: self.domain_name.as_canonical_name() }
    }
    
    #[inline]
    pub fn canonical_name(&mut self) {
        self.domain_name.canonical_name()
    }

    #[inline]
    pub fn as_lower(&self) -> Self {
        Self { domain_name: self.domain_name.as_lower() }
    }

    #[inline]
    pub fn lower(&mut self) {
        self.domain_name.lower()
    }

    /// is_subdomain checks if child is indeed a child of the parent. If child
    /// and parent are the same domain true is returned as well.
    #[inline]
    pub fn is_subdomain(&self, child: &Self) -> bool {
        self.domain_name.is_subdomain(&child.domain_name)
    }

    #[inline]
    pub fn as_vec(&self) -> &Vec<Label> {
        self.domain_name.as_vec()
    }

    #[inline]
    pub fn as_slice(&self) -> &[Label] {
        self.domain_name.as_slice()
    }

    #[inline]
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &Label> + ExactSizeIterator<Item = &Label> {
        self.domain_name.iter()
    }

    #[inline]
    pub fn search_domains<'a>(&'a self) -> impl 'a + DoubleEndedIterator<Item = Self> + ExactSizeIterator<Item = Self> {
        self.domain_name.search_domains().map(|c_domain_name| Self { domain_name: c_domain_name })
    }

    #[inline]
    pub fn compare_domain_name(domain1: &Self, domain2: &Self) -> usize {
        CDomainName::compare_domain_name(
            &domain1.domain_name,
            &domain2.domain_name
        )
    }
}

impl Display for DomainName {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.domain_name.fmt(f)
    }
}

impl Add for DomainName {
    type Output = Result<Self, DomainNameError>;

    #[inline]
    fn add(mut self, rhs: Self) -> Self::Output {
        self.domain_name = (self.domain_name + rhs.domain_name)?;
        return Ok(self);
    }
}

impl ToWire for DomainName {
    #[inline]
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>, _compression: &mut Option<crate::serde::wire::compression_map::CompressionMap>) -> Result<(), crate::serde::wire::write_wire::WriteWireError> where 'a: 'b {
        // Providing a None type compression map to the CDomainName disables domain name compression
        // while allowing us to re-use the rest of its implementation.
        self.domain_name.to_wire_format(wire, &mut None)
    }

    #[inline]
    fn serial_length(&self) -> u16 {
        self.domain_name.serial_length()
    }
}

impl FromWire for DomainName {
    #[inline]
    fn from_wire_format<'a, 'b>(wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>) -> Result<Self, crate::serde::wire::read_wire::ReadWireError> where Self: Sized, 'a: 'b {
        // DomainName must still be able to decompress domain names if compression was used so we
        // don't want to disable that.
        Ok(Self { domain_name: CDomainName::from_wire_format(wire)? })
    }
}

impl FromPresentation for DomainName {
    #[inline]
    fn from_token_format<'a, 'b, 'c, 'd>(tokens: &'c [&'a str]) -> Result<(Self, &'d [&'a str]), TokenError<'b>> where Self: Sized, 'a: 'b, 'c: 'd, 'c: 'd {
        let (cdomain_name, tokens) = CDomainName::from_token_format(tokens)?;
        Ok((Self { domain_name: cdomain_name }, tokens))
    }
}

#[cfg(test)]
mod circular_serde_sanity_test {
    use crate::serde::wire::circular_test::gen_test_circular_serde_sanity_test;
    use super::DomainName;

    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test,
        DomainName::from_utf8("www.example.com.").unwrap()
    );
    gen_test_circular_serde_sanity_test!(
        root_record_circular_serde_sanity_test,
        DomainName::from_utf8(".").unwrap()
    );
    gen_test_circular_serde_sanity_test!(
        root_zone_record_circular_serde_sanity_test,
        DomainName::from_utf8("com.").unwrap()
    );
}

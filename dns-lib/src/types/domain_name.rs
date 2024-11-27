use std::{error::Error, fmt::{Debug, Display}, ops::Add};

use dns_macros::ToPresentation;

use crate::{serde::{presentation::{errors::TokenError, from_presentation::FromPresentation}, wire::{from_wire::FromWire, to_wire::ToWire}}, types::{ascii::AsciiString, c_domain_name::{CDomainName, CDomainNameError, Label}}};

use super::c_domain_name::{CmpDomainName, OwnedLabel};

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
#[derive(Clone, PartialEq, Eq, Hash, ToPresentation)]
pub struct DomainName {
    pub(super) domain_name: CDomainName,
}

impl DomainName {
    #[inline]
    pub fn new(string: &AsciiString) -> Result<Self, DomainNameError> {
        Ok(Self { domain_name: CDomainName::new(string)? })
    }

    #[inline]
    pub fn new_root() -> Self {
        Self { domain_name: CDomainName::new_root() }
    }

    #[inline]
    pub fn from_utf8(string: &str) -> Result<Self, DomainNameError> {
        Ok(Self { domain_name: CDomainName::from_utf8(string)? })
    }

    #[inline]
    pub fn from_labels(labels: Vec<OwnedLabel>) -> Result<Self, DomainNameError> {
        Ok(Self { domain_name: CDomainName::from_labels(labels)? })
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
    pub fn is_fully_qualified(&self) -> bool {
        self.domain_name.is_fully_qualified()
    }

    /// Converts this domain into a fully qualified domain.
    #[inline]
    pub fn fully_qualified(&mut self) -> Result<(), DomainNameError> {
        Ok(self.domain_name.set_fully_qualified()?)
    }

    /// Creates a fully qualified domain from this domain.
    #[inline]
    pub fn as_fully_qualified(&self) -> Result<Self, DomainNameError> {
        Ok(Self { domain_name: self.domain_name.as_fully_qualified()? })
    }

    #[inline]
    pub fn as_canonical_name(&self) -> Result<Self, DomainNameError> {
        Ok(Self { domain_name: self.domain_name.as_canonical_name()? })
    }
    
    #[inline]
    pub fn canonical_name(&mut self) -> Result<(), DomainNameError> {
        self.domain_name.canonical_name()?;
        Ok(())
    }

    #[inline]
    pub fn as_lower(&self) -> Self {
        Self { domain_name: self.domain_name.as_lower() }
    }

    #[inline]
    pub fn lower(&mut self) {
        self.domain_name.lower()
    }

    #[inline]
    pub fn labels<'a>(&'a self) -> impl 'a + DoubleEndedIterator<Item = Label<'a>> {
        self.domain_name.labels()
    }

    #[inline]
    pub fn search_domains<'a>(&'a self) -> impl 'a + ExactSizeIterator<Item = Self> {
        self.domain_name.search_domains().map(|domain_name| DomainName { domain_name })
    }
}

impl Debug for DomainName {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DomainName: {self}")
    }
}

impl Display for DomainName {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.domain_name)
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

impl CmpDomainName<CDomainName> for DomainName {
    #[inline]
    fn matches(&self, other: &CDomainName) -> bool {
        self.domain_name.matches(other)
    }

    #[inline]
    fn is_parent_domain_of(&self, child: &CDomainName) -> bool {
        self.domain_name.is_parent_domain_of(child)
    }
}

impl CmpDomainName<DomainName> for DomainName {
    #[inline]
    fn matches(&self, other: &DomainName) -> bool {
        self.domain_name.matches(&other.domain_name)
    }

    #[inline]
    fn is_parent_domain_of(&self, child: &DomainName) -> bool {
        self.domain_name.matches(&child.domain_name)
    }
}

impl ToWire for DomainName {
    #[inline]
    fn to_wire_format<'a, 'b>(&self, wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>, _compression: &mut Option<crate::types::c_domain_name::CompressionMap>) -> Result<(), crate::serde::wire::write_wire::WriteWireError> where 'a: 'b {
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

impl From<DomainName> for CDomainName {
    fn from(domain_name: DomainName) -> Self {
        domain_name.domain_name
    }
}

impl From<&DomainName> for CDomainName {
    fn from(domain_name: &DomainName) -> Self {
        domain_name.domain_name.clone()
    }
}

impl From<CDomainName> for DomainName {
    fn from(domain_name: CDomainName) -> Self {
        Self { domain_name }
    }
}

impl From<&CDomainName> for DomainName {
    fn from(domain_name: &CDomainName) -> Self {
        Self { domain_name: domain_name.clone() }
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

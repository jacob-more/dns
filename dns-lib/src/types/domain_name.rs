use std::{fmt::Display, ops::Add, error::Error};

use crate::types::{c_domain_name::{CDomainNameError, CDomainName, Label}, ascii::AsciiString};

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
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
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
    pub fn search_domains<'a>(&'a self) -> impl 'a + Iterator<Item = CDomainName> {
        self.domain_name.search_domains()
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

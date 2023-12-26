use std::{fmt::Display, ops::Add};

use super::{c_domain_name::{CDomainNameError, CDomainName}, ascii::AsciiString};

/// This is an incompressible domain name. This should be used in any place where domain name compression is not
/// allowed. It is still able to decompress a domain name but it will not compress it when
/// serializing the name. If compression is required, use the CDomainName.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct DomainName {
    domain_name: CDomainName,
}

impl DomainName {
    #[inline]
    pub fn new(string: &AsciiString) -> Result<Self, CDomainNameError> {
        Ok(Self { domain_name: CDomainName::new(string)? })
    }

    #[inline]
    pub fn from_utf8(string: &str) -> Result<Self, CDomainNameError> {
        match CDomainName::from_utf8(string) {
            Ok(name) => Ok(Self {
                domain_name: name,
            }),
            Err(error) => Err(error),
        }
    }

    #[inline]
    pub fn is_fully_qualified(&self) -> bool {
        self.domain_name.is_fully_qualified()
    }

    #[inline]
    pub fn fully_qualified(&mut self) {
        self.domain_name.fully_qualified()
    }

    #[inline]
    pub fn as_fully_qualified(&self) -> Self {
        let name = self.domain_name.as_fully_qualified();
        Self { domain_name: name }
    }

    #[inline]
    pub fn label_count(&self) -> usize {
        self.domain_name.label_count()
    }

    #[inline]
    pub fn is_root(&self) -> bool {
        self.domain_name.is_root()
    }

    #[inline]
    pub fn as_canonical_name(&self) -> Self {
        let name = self.domain_name.as_canonical_name();
        Self { domain_name: name }
    }
    
    #[inline]
    pub fn canonical_name(&mut self) {
        self.lower();
        self.fully_qualified();
    }

    #[inline]
    pub fn as_lower(&self) -> Self {
        let name = self.domain_name.as_lower();
        Self { domain_name: name }
    }

    #[inline]
    pub fn lower(&mut self) {
        self.domain_name.lower()
    }

    #[inline]
    pub fn is_subdomain(&self, child: &Self) -> bool {
        self.domain_name.is_subdomain(&child.domain_name)
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
    type Output = Result<Self, CDomainNameError>;

    #[inline]
    fn add(mut self, rhs: Self) -> Self::Output {
        self.domain_name = (self.domain_name + rhs.domain_name)?;
        return Ok(self);
    }
}

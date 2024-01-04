use std::{collections::HashMap, error::Error, fmt::Display};

use crate::types::c_domain_name::CDomainName;

use super::{resource_record::ResourceRecord, rtype::RType};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum RRSetError {
    DifferingDomainName(CDomainName, CDomainName),
}

impl Error for RRSetError {}
impl Display for RRSetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DifferingDomainName(expected, found) => write!(f, "The domain names in the records provided are not all the same (Expected: \"{expected}\", Found: \"{found}\")"),
        }
    }
}

pub type Records = Vec<ResourceRecord>;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RRSet {
    domain_name: CDomainName,
    records: HashMap<RType, Records>
}

impl RRSet {
    #[inline]
    pub fn new(name: CDomainName) -> Self {
        Self {
            domain_name: name,
            records: HashMap::with_capacity(0),
        }
    }

    #[inline]
    pub fn from_iter<'a>(name: CDomainName, records: impl Iterator<Item = &'a ResourceRecord>) -> Result<Self, RRSetError> {
        let mut rrset_records: HashMap<RType, Records> = HashMap::new();
        for record in records {
            if !CDomainName::matches(&name, record.name()) {
                return Err(RRSetError::DifferingDomainName(name.clone(), record.name().clone()));
            }
            match rrset_records.get_mut(&record.rtype()) {
                Some(records_vec) => records_vec.push(record.clone()),
                None => {rrset_records.insert(record.rtype(), vec![record.clone()]); ()},
            }
        }

        Ok(Self {
            domain_name: name,
            records: rrset_records,
        })
    }

    #[inline]
    pub fn from_slice(name: CDomainName, records: &[ResourceRecord]) -> Result<Self, RRSetError> {
        Self::from_iter(name, records.iter())
    }

    #[inline]
    pub fn from_vec(name: CDomainName, records: &Vec<ResourceRecord>) -> Result<Self, RRSetError> {
        Self::from_iter(name, records.iter())
    }

    #[inline]
    pub fn from_iter_ignore_name<'a>(name: &CDomainName, records: impl Iterator<Item = &'a ResourceRecord>) -> Result<Self, RRSetError> {
        Self::from_iter(
            name.clone(),
            records.filter(|record| CDomainName::matches(record.name(), name))
        )
    }

    #[inline]
    pub fn from_slice_ignore_name<'a>(name: &CDomainName, records: &[ResourceRecord]) -> Result<Self, RRSetError> {
        Self::from_iter_ignore_name(name, records.iter())
    }

    #[inline]
    pub fn from_vec_ignore_name<'a>(name: &CDomainName, records: &Vec<ResourceRecord>) -> Result<Self, RRSetError> {
        Self::from_iter_ignore_name(name, records.iter())
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.records.values().len()
    }

    #[inline]
    pub fn name(&self) -> &CDomainName {
        &self.domain_name
    }

    #[inline]
    pub fn has_records(&self, rtype: &RType) -> bool {
        self.records.contains_key(rtype)
    }

    #[inline]
    pub fn has_record(&self, record: &ResourceRecord) -> bool {
        match self.records.get(&record.rtype()) {
            Some(records) => records.iter().any(|other| other.matches(record)),
            None => false,
        }
    }

    #[inline]
    pub fn get_records(&self, rtype: &RType) -> Option<&Records> {
        self.records.get(rtype)
    }

    #[inline]
    pub fn get_mut_records(&mut self, rtype: &RType) -> Option<&mut Records> {
        self.records.get_mut(rtype)
    }
}

use std::collections::HashMap;

use crate::types::c_domain_name::{CDomainName, Label};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CompressionMap {
	map: HashMap<Vec<Label>, u16>,
}

impl<'a> CompressionMap {
	#[inline]
    pub fn new() -> CompressionMap {
        Self { map: HashMap::new() }
    }

	#[inline]
    pub fn insert_domain(&mut self, domain: &'a CDomainName, offset: u16) {
        self.insert_slice_labels(domain.as_slice(), offset);
    }

	#[inline]
    pub fn insert_vec_labels(&mut self, domain: Vec<Label>, offset: u16) {
        self.map.insert(domain, offset);
    }

	#[inline]
    pub fn insert_slice_labels(&mut self, domain: &'a [Label], offset: u16) {
        self.insert_vec_labels(Vec::from(domain), offset)
    }

	#[inline]
    pub fn find_from_domain(&self, domain: &CDomainName) -> Option<u16> {
        self.find_from_slice_labels(domain.as_slice())
    }

	#[inline]
    pub fn find_from_vec_labels(&self, domain: &Vec<Label>) -> Option<u16> {
        self.find_from_slice_labels(domain)
    }

	#[inline]
    pub fn find_from_slice_labels(&self, domain: &[Label]) -> Option<u16> {
        Some(self.map.get(domain)?.clone())
    }
}

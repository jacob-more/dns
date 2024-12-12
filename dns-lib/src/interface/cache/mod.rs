use std::time::Instant;

use crate::{query::question::Question, resource_record::{rclass::RClass, rcode::RCode, resource_record::ResourceRecord, rtype::RType}, types::c_domain_name::CDomainName};

pub mod cache;

pub mod main_cache;
pub mod transaction_cache;

pub mod meta_cache;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct CacheQuery {
    pub authoritative: bool,
    pub question: Question,
}

impl CacheQuery {
    #[inline]
    pub const fn qname(&self) -> &CDomainName { &self.question.qname() }

    #[inline]
    pub const fn qtype(&self) -> RType { self.question.qtype() }

    #[inline]
    pub const fn qclass(&self) -> RClass { self.question.qclass() }
}

#[derive(Clone, PartialEq, Hash, Debug)]
pub enum CacheResponse {
    Records(Vec<CacheRecord>),
    Err(RCode),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum MetaAuth {
    Authoritative,
    NotAuthoritative,
    NotAuthoritativeBootstrap,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct CacheMeta {
    pub auth: MetaAuth,
    pub insertion_time: Instant,
}

#[derive(Clone, PartialEq, Hash, Debug)]
pub struct CacheRecord {
    pub meta: CacheMeta,
    pub record: ResourceRecord,
}

impl CacheRecord {
    #[inline]
    pub fn is_expired(&self) -> bool {
        self.meta.insertion_time.elapsed().as_secs() >= self.record.get_ttl().as_secs() as u64
    }

    #[inline]
    pub const fn is_authoritative(&self) -> bool {
        match &self.meta.auth {
            MetaAuth::Authoritative => true,
            MetaAuth::NotAuthoritative => false,
            MetaAuth::NotAuthoritativeBootstrap => false,
        }
    }

    #[inline]
    pub const fn is_bootstrap(&self) -> bool {
        match &self.meta.auth {
            MetaAuth::Authoritative => false,
            MetaAuth::NotAuthoritative => false,
            MetaAuth::NotAuthoritativeBootstrap => true,
        }
    }
}

use std::collections::hash_map::Entry;

use dns_lib::{interface::cache::{transaction_cache::TransactionCache, CacheQuery, CacheRecord, CacheResponse}, query::question::Question, resource_record::{rcode::RCode, rtype::RType}};

use super::tree_cache::{TreeCache, TreeCacheError};

#[derive(Debug)]
pub struct TransactionTreeCache {
    cache: TreeCache<Vec<CacheRecord>>
}

impl TransactionTreeCache {
    #[inline]
    pub fn new() -> Self {
        Self { cache: TreeCache::new() }
    }

    #[inline]
    fn get_records(&self, query: &CacheQuery) -> Result<Vec<CacheRecord>, TreeCacheError> {
        match query.qtype() {
            RType::ANY => {
                if let Some(node) = self.cache.get_node(&query.question)? {
                    if query.authoritative {
                        return Ok(node.records.values()
                            .flat_map(|records| records.iter().filter(|record| record.is_authoritative()))
                            .map(|record| record.clone())
                            .collect());
                    } else {
                        return Ok(node.records.values()
                            .flat_map(|records| records.iter().map(|record| record.clone()))
                            .collect());
                    }
                }
            },
            _ => {
                if let Some(node) = self.cache.get_node(&query.question)? {
                    if let Some(records) = node.records.get(&query.qtype()) {
                        if query.authoritative {
                            return Ok(records.iter()
                                .filter(|record| record.is_authoritative())
                                .map(|record| record.clone())
                                .collect()
                            )
                        } else {
                            return Ok(records.iter()
                                .map(|record| record.clone())
                                .collect()
                            )
                        }
                    }
                }
            },
        }

        return Ok(vec![]);
    }

    #[inline]
    fn insert_record(&mut self, record: CacheRecord) -> Result<(), TreeCacheError> {
        let question = Question::new(
            record.get_name().clone(),
            record.get_rtype(),
            record.get_rclass()
        );
        let node = self.cache.get_or_create_node(&question)?;
        match node.records.entry(question.qtype()) {
            Entry::Occupied(mut entry) => {
                let cached_records = entry.get_mut();
                if !cached_records.iter().any(|cached_record| cached_record.record == record.record) {
                    cached_records.push(record);
                }
            },
            Entry::Vacant(entry) => {
                let new_cache_array = vec![record];
                entry.insert(new_cache_array);
            },
        }
        Ok(())
    }

    #[inline]
    pub fn iter<'a>(&'a self) -> impl Iterator<Item = (&'a RType, &'a Vec<CacheRecord>)> + 'a {
        self.cache.iter().flat_map(|node| &node.records)
    }
}

impl TransactionCache for TransactionTreeCache {
    fn get(&self, query: &CacheQuery) -> CacheResponse {
        match self.get_records(&query) {
            Ok(records) => CacheResponse::Records(records),
            Err(_) => CacheResponse::Err(RCode::ServFail),
        }
    }

    fn insert_record(&mut self, record: CacheRecord) {
        let _ = self.insert_record(record);
    }
}

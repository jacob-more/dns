use std::{collections::hash_map::Entry, time::Instant};

use dns_lib::{
    interface::cache::{CacheQuery, CacheRecord, CacheResponse, main_cache::MainCache},
    query::question::Question,
    resource_record::{rcode::RCode, rtype::RType},
};

use super::tree_cache::{TreeCache, TreeCacheError};

pub struct MainTreeCache {
    cache: TreeCache<Vec<CacheRecord>>,
}

impl Default for MainTreeCache {
    fn default() -> Self {
        Self::new()
    }
}

impl MainTreeCache {
    #[inline]
    pub fn new() -> Self {
        Self {
            cache: TreeCache::new(),
        }
    }

    #[inline]
    fn get_records(&self, query: &CacheQuery) -> Result<Vec<CacheRecord>, TreeCacheError> {
        match query.qtype() {
            RType::ANY => {
                if let Some(node) = self.cache.get_node(query.question)? {
                    if query.authoritative {
                        return Ok(node
                            .records
                            .values()
                            .flatten()
                            .filter(|record| record.is_authoritative())
                            .filter(|record| !record.is_expired())
                            .cloned()
                            .collect());
                    } else {
                        return Ok(node
                            .records
                            .values()
                            .flatten()
                            .filter(|record| !record.is_expired())
                            .cloned()
                            .collect());
                    }
                }
            }
            _ => {
                if let Some(node) = self.cache.get_node(query.question)? {
                    if let Some(records) = node.records.get(&query.qtype()) {
                        if query.authoritative {
                            return Ok(records
                                .iter()
                                .filter(|record| record.is_authoritative())
                                .filter(|record| !record.is_expired())
                                .cloned()
                                .collect());
                        } else {
                            return Ok(records
                                .iter()
                                .filter(|record| !record.is_expired())
                                .cloned()
                                .collect());
                        }
                    }
                }
            }
        }

        Ok(vec![])
    }

    #[inline]
    fn insert_record(
        &mut self,
        record: CacheRecord,
        received_time: Instant,
    ) -> Result<(), TreeCacheError> {
        let question = Question::new(
            record.get_name().clone(),
            record.get_rtype(),
            record.get_rclass(),
        );
        let node = self.cache.get_or_create_node(&question)?;
        match node.records.entry(question.qtype()) {
            Entry::Occupied(mut entry) => {
                let cached_records = entry.get_mut();
                let mut record_matched = false;
                let mut indexes_to_remove = Vec::new();
                // Step 1: Go through all of the cached records.
                //          If a matching record is found, update the ttl. Since the record is already cached, nothing else needs to be done.
                //          If one of the cached records has expired, record the index. It will be removed during a second pass.
                //          Keep track of if a match record was found so we can add the new one if needed.
                for (index, cached_record) in cached_records.iter_mut().enumerate() {
                    if record.record == cached_record.record {
                        record_matched = true;
                        if record.is_authoritative() == cached_record.is_authoritative() {
                            cached_record.set_ttl(*record.get_ttl());
                            cached_record.meta.insertion_time = received_time;
                        }
                    }
                    if cached_record.meta.insertion_time.elapsed().as_secs()
                        >= cached_record.get_ttl().as_secs() as u64
                    {
                        indexes_to_remove.push(index);
                    }
                }

                // Step 2: Remove any of the records that were expired uses the indexes recorded in the first pass.
                //         However, use a reversed order so that the later indexes are not screwed up by removing
                //         something near the beginning.
                for index in indexes_to_remove.iter().rev() {
                    cached_records.remove(*index);
                }

                // Step 3: If no matches were found, we can now add the newest record to the cache.
                //         Note: This must be done AFTER the expired records are removed to make sure the indexes are accurate.
                if !record_matched {
                    cached_records.push(record);
                }
            }
            Entry::Vacant(entry) => {
                entry.insert(vec![record]);
            }
        }
        Ok(())
    }

    #[inline]
    pub fn iter<'a>(&'a self) -> impl Iterator<Item = (&'a RType, &'a Vec<CacheRecord>)> + 'a {
        self.cache.iter().flat_map(|node| &node.records)
    }
}

impl MainCache for MainTreeCache {
    fn get(&self, query: &CacheQuery) -> CacheResponse {
        match self.get_records(query) {
            Ok(records) => CacheResponse::Records(records),
            Err(_) => CacheResponse::Err(RCode::ServFail),
        }
    }

    fn insert_record(&mut self, record: CacheRecord) {
        // Records with TTL == 0 are not supposed to be cached
        if record.get_ttl().as_secs() != 0 {
            let received_time = Instant::now();
            let _ = self.insert_record(record, received_time);
        }
    }

    fn clean(&mut self) {
        todo!()
    }
}

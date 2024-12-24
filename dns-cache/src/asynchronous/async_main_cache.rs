use std::{collections::{hash_map::Entry, HashSet}, time::Instant};

use async_trait::async_trait;
use dns_lib::{interface::cache::{main_cache::AsyncMainCache, CacheQuery, CacheRecord, CacheResponse}, query::question::Question, resource_record::{rcode::RCode, rtype::RType}, types::c_domain_name::CDomainName};

use super::async_tree_cache::{AsyncTreeCache, AsyncTreeCacheError};

pub struct AsyncMainTreeCache {
    cache: AsyncTreeCache<Vec<CacheRecord>>
}

impl AsyncMainTreeCache {
    #[inline]
    pub fn new() -> Self {
        Self { cache: AsyncTreeCache::new() }
    }

    #[inline]
    async fn get_records(&self, query: &CacheQuery<'_>) -> Result<Vec<CacheRecord>, AsyncTreeCacheError> {
        match query.qtype() {
            RType::ANY => {
                if let Some(node) = self.cache.get_node(&query.question).await? {
                    let read_records = node.records.read().await;
                    let result;
                    if query.authoritative {
                        result = read_records.values()
                            .flatten()
                            .filter(|record| record.is_authoritative())
                            .filter(|record| !record.is_expired())
                            .map(|cache_record| cache_record.clone())
                            .collect();
                    } else {
                        result = read_records.values()
                            .flatten()
                            .filter(|record| !record.is_expired())
                            .map(|cache_record| cache_record.clone())
                            .collect();
                    }
                    drop(read_records);
                    return Ok(result);
                }
            },
            _ => {
                if let Some(node) = self.cache.get_node(&query.question).await? {
                    let read_records = node.records.read().await;
                    if let Some(records) = read_records.get(&query.qtype()) {
                        let result;
                        if query.authoritative {
                            result = records.iter()
                                .filter(|record| record.is_authoritative())
                                .filter(|record| !record.is_expired())
                                .map(|cache_record| cache_record.clone())
                                .collect();
                        } else {
                            result = records.iter()
                                .filter(|record| !record.is_expired())
                                .map(|cache_record| cache_record.clone())
                                .collect();
                        }
                        drop(read_records);
                        return Ok(result);
                    }
                    drop(read_records);
                }
            },
        }

        return Ok(vec![]);
    }

    #[inline]
    async fn insert_record(&self, record: CacheRecord, received_time: Instant) -> Result<(), AsyncTreeCacheError> {
        let question = Question::new(
            record.get_name().clone(),
            record.get_rtype(),
            record.get_rclass()
        );
        let node = self.cache.get_or_create_node(&question).await?;
        let mut write_records = node.records.write().await;
        match write_records.entry(question.qtype()) {
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
                        match (record.is_authoritative(), cached_record.is_authoritative()) {
                            (true, true) => {
                                cached_record.set_ttl(*record.get_ttl());
                                cached_record.meta.insertion_time = received_time;
                            },
                            (false, false) => {
                                cached_record.set_ttl(*record.get_ttl());
                                cached_record.meta.insertion_time = received_time;
                            },
                            // Non-authoritative records can be replaced with authoritative versions.
                            (true, false) => {
                                *cached_record = record.clone();
                                cached_record.meta.insertion_time = received_time;
                            },
                            // Authoritative records cannot be updated by non-authoritative versions.
                            (false, true) => (),
                        }
                    }
                    if cached_record.meta.insertion_time.elapsed().as_secs() >= cached_record.get_ttl().as_secs() as u64 {
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
            },
            Entry::Vacant(entry) => {
                entry.insert(vec![record]);
            },
        }
        drop(write_records);
        Ok(())
    }

    pub async fn get_domains(&self) -> HashSet<CDomainName> { self.cache.get_domains().await }
}

#[async_trait]
impl AsyncMainCache for AsyncMainTreeCache {
    async fn get(&self, query: &CacheQuery) -> CacheResponse {
        match self.get_records(&query).await {
            Ok(records) => CacheResponse::Records(records),
            Err(_) => CacheResponse::Err(RCode::ServFail),
        }
    }

    async fn insert_record(&self, record: CacheRecord) {
        if record.get_ttl().as_secs() != 0 {
            let received_time = Instant::now();
            let _ = self.insert_record(record, received_time).await;
        }
    }

    async fn clean(&self) {
        todo!()
    }
}

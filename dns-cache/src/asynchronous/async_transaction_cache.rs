use std::collections::hash_map::Entry;

use async_trait::async_trait;
use dns_lib::{
    interface::cache::{
        CacheQuery, CacheRecord, CacheResponse, transaction_cache::AsyncTransactionCache,
    },
    query::question::Question,
    resource_record::{rcode::RCode, rtype::RType},
};

use super::async_tree_cache::{AsyncTreeCache, AsyncTreeCacheError};

pub struct AsyncTransactionTreeCache {
    cache: AsyncTreeCache<Vec<CacheRecord>>,
}

impl Default for AsyncTransactionTreeCache {
    fn default() -> Self {
        Self::new()
    }
}

impl AsyncTransactionTreeCache {
    #[inline]
    pub fn new() -> Self {
        Self {
            cache: AsyncTreeCache::new(),
        }
    }

    #[inline]
    async fn get_records(
        &self,
        query: &CacheQuery<'_>,
    ) -> Result<Vec<CacheRecord>, AsyncTreeCacheError> {
        match query.qtype() {
            RType::ANY => {
                if let Some(node) = self.cache.get_node(query.question).await? {
                    let read_records = node.records.read().await;
                    let result = if query.authoritative {
                        read_records
                            .values()
                            .flatten()
                            .filter(|cached_record| cached_record.is_authoritative())
                            .cloned()
                            .collect()
                    } else {
                        read_records.values().flatten().cloned().collect()
                    };
                    drop(read_records);
                    return Ok(result);
                }
            }
            _ => {
                if let Some(node) = self.cache.get_node(query.question).await? {
                    let read_records = node.records.read().await;
                    if let Some(records) = read_records.get(&query.qtype()) {
                        let result = if query.authoritative {
                            records
                                .iter()
                                .filter(|cached_record| cached_record.is_authoritative())
                                .cloned()
                                .collect()
                        } else {
                            records.to_vec()
                        };
                        drop(read_records);
                        return Ok(result);
                    }
                    drop(read_records);
                }
            }
        }

        Ok(vec![])
    }

    #[inline]
    async fn insert_record(&self, record: CacheRecord) -> Result<(), AsyncTreeCacheError> {
        let question = Question::new(
            record.get_name().clone(),
            record.get_rtype(),
            record.get_rclass(),
        );
        let node = self.cache.get_or_create_node(&question).await?;
        let mut write_records = node.records.write().await;
        match write_records.entry(question.qtype()) {
            Entry::Occupied(mut entry) => {
                let cached_records = entry.get_mut();
                if !cached_records
                    .iter()
                    .any(|cached_record| cached_record.record == record.record)
                {
                    cached_records.push(record);
                }
            }
            Entry::Vacant(entry) => {
                entry.insert(vec![record]);
            }
        }
        drop(write_records);
        Ok(())
    }
}

#[async_trait]
impl AsyncTransactionCache for AsyncTransactionTreeCache {
    async fn get(&self, query: &CacheQuery) -> CacheResponse {
        match self.get_records(query).await {
            Ok(records) => CacheResponse::Records(records),
            Err(_) => CacheResponse::Err(RCode::ServFail),
        }
    }

    async fn insert_record(&self, record: CacheRecord) {
        let _ = self.insert_record(record).await;
    }
}

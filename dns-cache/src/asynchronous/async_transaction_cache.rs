use async_trait::async_trait;
use dns_lib::{interface::cache::{transaction_cache::AsyncTransactionCache, CacheQuery, CacheRecord, CacheResponse}, query::question::Question, resource_record::{rcode::RCode, rtype::RType}};

use super::async_tree_cache::{AsyncTreeCache, AsyncTreeCacheError};

pub struct AsyncTransactionTreeCache {
    cache: AsyncTreeCache<Vec<CacheRecord>>
}

impl AsyncTransactionTreeCache {
    #[inline]
    pub fn new() -> Self {
        Self { cache: AsyncTreeCache::new() }
    }

    #[inline]
    async fn get_records(&self, query: &CacheQuery) -> Result<Vec<CacheRecord>, AsyncTreeCacheError> {
        match query.qtype() {
            RType::ANY => {
                if let Some(node) = self.cache.get_node(&query.question).await? {
                    let read_records = node.records.read().await;
                    let result;
                    if query.authoritative {
                        result = read_records.values()
                        .flatten()
                        .filter(|cached_record| cached_record.is_authoritative())
                        .map(|cache_record| cache_record.clone())
                        .collect();
                    } else {
                        result = read_records.values()
                        .flatten()
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
                                .filter(|cached_record| cached_record.is_authoritative())
                                .map(|cache_record| cache_record.clone())
                                .collect();
                        } else {
                            result = records.iter()
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
    async fn insert_record(&self, record: CacheRecord) -> Result<(), AsyncTreeCacheError> {
        let question = Question::new(
            record.record.name().clone(),
            record.record.rtype(),
            record.record.rclass()
        );
        let node = self.cache.get_or_create_node(&question).await?;
        let mut write_records = node.records.write().await;
        if let Some(cached_records) = write_records.get_mut(&question.qtype()) {
            if !cached_records.iter().any(|cached_record| cached_record.record.matches(&record.record)) {
                cached_records.push(record);
            }
        } else {
            write_records.insert(question.qtype(), vec![record]);
        }
        drop(write_records);
        Ok(())
    }
}

#[async_trait]
impl AsyncTransactionCache for AsyncTransactionTreeCache {
    async fn get(&self, query: &CacheQuery) -> CacheResponse {
        match self.get_records(&query).await {
            Ok(records) => CacheResponse::Records(records),
            Err(_) => CacheResponse::Err(RCode::ServFail),
        }
    }

    async fn insert_record(&self, record: CacheRecord) {
        let _ = self.insert_record(record).await;
    }
}

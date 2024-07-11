use std::sync::Arc;

use async_trait::async_trait;
use dns_lib::interface::cache::{cache::AsyncCache, main_cache::AsyncMainCache, transaction_cache::AsyncTransactionCache, CacheQuery, CacheRecord, CacheResponse};
use tokio::join;

use super::{async_main_cache::AsyncMainTreeCache, async_transaction_cache::AsyncTransactionTreeCache};

pub struct AsyncTreeCache {
    main_cache: Arc<AsyncMainTreeCache>,
    transaction_cache: AsyncTransactionTreeCache
}

impl AsyncTreeCache {
    #[inline]
    pub fn new(main_cache: Arc<AsyncMainTreeCache>) -> Self {
        Self {
            main_cache,
            transaction_cache: AsyncTransactionTreeCache::new(),
        }
    }
}

#[async_trait]
impl AsyncCache for AsyncTreeCache {
    async fn get(&self, query: &CacheQuery) -> CacheResponse {
        let transaction_response = self.transaction_cache.get(query);
        let main_response = self.main_cache.get(query);
        match join!(transaction_response, main_response) {
            // Note: The transaction cache CANNOT return an error, otherwise the overall response is
            // an error since it may hold critical records.
            (CacheResponse::Err(rcode), _) => CacheResponse::Err(rcode),
            (CacheResponse::Records(mut transaction_records), CacheResponse::Records(main_records)) => {
                transaction_records.extend(main_records);
                CacheResponse::Records(transaction_records)
            },
            (CacheResponse::Records(transaction_records), CacheResponse::Err(_)) => CacheResponse::Records(transaction_records),

        }
    }

    async fn insert_record(&self, record: CacheRecord) {
        join!(
            self.transaction_cache.insert_record(record.clone()),
            self.main_cache.insert_record(record),
        );
    }
}

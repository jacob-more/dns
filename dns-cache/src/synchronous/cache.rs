use dns_lib::interface::cache::{cache::Cache, main_cache::MainCache, transaction_cache::TransactionCache, CacheQuery, CacheRecord, CacheResponse};

use super::{main_cache::MainTreeCache, transaction_cache::TransactionTreeCache};

pub struct TreeCache<'a> {
    main_cache: &'a mut MainTreeCache,
    transaction_cache: TransactionTreeCache
}

impl<'a> TreeCache<'a> {
    #[inline]
    pub fn new(main_cache: &'a mut MainTreeCache) -> Self {
        Self {
            main_cache,
            transaction_cache: TransactionTreeCache::new(),
        }
    }
}

impl<'a> Cache for TreeCache<'a> {
    fn get(&self, query: &CacheQuery) -> CacheResponse {
        let transaction_response = self.transaction_cache.get(query);
        let main_response = self.main_cache.get(query);
        match (transaction_response, main_response) {
            // Note: The transaction cache CANNOT return an error, otherwise the overall response is
            // an error since it may hold critical records.
            (CacheResponse::Err(rcode), _) => CacheResponse::Err(rcode),

            (CacheResponse::Records(mut transaction_records), CacheResponse::Records(main_records)) => {
                transaction_records.extend(main_records);
                CacheResponse::Records(transaction_records)
            },
            (transaction_records @ CacheResponse::Records(_), CacheResponse::Err(_)) => transaction_records,

        }
    }

    fn insert_record(&mut self, record: CacheRecord) {
        self.transaction_cache.insert_record(record.clone());
        self.main_cache.insert_record(record);
    }

}

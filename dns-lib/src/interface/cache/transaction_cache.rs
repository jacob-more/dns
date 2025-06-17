use async_trait::async_trait;
use futures::{Stream, StreamExt};

use super::{CacheQuery, CacheRecord, CacheResponse};

pub trait TransactionCache {
    fn get(&self, query: &CacheQuery) -> CacheResponse;
    fn insert_record(&mut self, record: CacheRecord);
    fn insert_iter(&mut self, records: impl Iterator<Item = CacheRecord> + Send) {
        records.for_each(|record| self.insert_record(record));
    }
}

#[async_trait]
pub trait AsyncTransactionCache {
    async fn get(&self, query: &CacheQuery) -> CacheResponse;
    async fn insert_record(&self, record: CacheRecord);
    async fn insert_stream(&self, records: impl Stream<Item = CacheRecord> + Send) {
        records
            .for_each_concurrent(None, |record| self.insert_record(record))
            .await;
    }
    async fn insert_iter(&self, records: impl Iterator<Item = CacheRecord> + Send) {
        self.insert_stream(futures::stream::iter(records)).await;
    }
}

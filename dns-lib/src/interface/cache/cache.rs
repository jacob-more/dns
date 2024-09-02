use std::time::Instant;

use async_trait::async_trait;
use futures::{Stream, StreamExt};
use tokio::join;

use crate::{query::message::Message, types::c_domain_name::Labels};

use super::{CacheMeta, CacheQuery, CacheRecord, CacheResponse, MetaAuth};

pub trait Cache {
    fn get(&self, query: &CacheQuery) -> CacheResponse;
    fn insert_record(&mut self, record: CacheRecord);
    fn insert_iter(&mut self, records: impl Iterator<Item = CacheRecord> + Send) {
        records.for_each(|record| self.insert_record(record));
    }
}

#[async_trait]
pub trait AsyncCache {
    async fn get(&self, query: &CacheQuery) -> CacheResponse;
    async fn insert_record(&self, record: CacheRecord);
    async fn insert_stream(&self, records: impl Stream<Item = CacheRecord> + Send) {
        records.for_each_concurrent(None, |record| self.insert_record(record)).await;
    }
    async fn insert_iter(&self, records: impl Iterator<Item = CacheRecord> + Send) {
        self.insert_stream(futures::stream::iter(records)).await;
    }

    async fn insert_message(&self, message: &Message) {
        let insertion_time = Instant::now();
        match message.question.get(0) {
            None => println!("Message could not be added to cache because it was missing a question section. {message:?}"),
            Some(question) => {
                let qname = question.qname();
                // TODO: Verify and validate authority.
                join!(
                    self.insert_iter(message.answer.iter().map(|answer| CacheRecord {
                        meta: CacheMeta {
                            auth: if message.authoritative_answer && answer.name().matches(qname) { MetaAuth::Authoritative } else { MetaAuth::NotAuthoritative },
                            insertion_time,
                        },
                        record: answer.clone(),
                    })),
                    self.insert_iter(message.authority.iter().map(|authority| CacheRecord {
                        meta: CacheMeta {
                            auth: MetaAuth::NotAuthoritative,
                            insertion_time
                        },
                        record: authority.clone()
                    })),
                    self.insert_iter(message.additional.iter().map(|additional| CacheRecord {
                        meta: CacheMeta {
                            auth: MetaAuth::NotAuthoritative,
                            insertion_time
                        },
                        record: additional.clone()
                    })),
                );
            },
        }
    }
}

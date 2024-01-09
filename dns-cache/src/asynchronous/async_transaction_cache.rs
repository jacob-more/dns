use async_trait::async_trait;
use dns_lib::{query::{question::Question, message::Message}, resource_record::{rtype::RType, resource_record::ResourceRecord, rcode::RCode}, interface::cache::transaction_cache::AsyncTransactionCache};
use ux::u3;

use super::async_tree_cache::{AsyncTreeCache, AsyncTreeCacheError};

type Records = Vec<ResourceRecord>;

pub struct AsyncTransactionTreeCache {
    cache: AsyncTreeCache<Records>
}

impl AsyncTransactionTreeCache {
    #[inline]
    pub fn new() -> Self {
        Self { cache: AsyncTreeCache::new() }
    }

    #[inline]
    async fn get_records(&self, question: &Question) -> Result<Vec<ResourceRecord>, AsyncTreeCacheError> {
        match question.qtype() {
            RType::ANY => {
                if let Some(node) = self.cache.get_node(question).await? {
                    let read_records = node.records.read().await;
                    let result = read_records.values()
                        .flatten()
                        .map(|cache_record| cache_record.clone())
                        .collect();
                    drop(read_records);
                    return Ok(result);
                }
            },
            _ => {
                if let Some(node) = self.cache.get_node(question).await? {
                    let read_records = node.records.read().await;
                    if let Some(records) = read_records.get(&question.qtype()) {
                        let result = records.iter()
                            .map(|cache_record| cache_record.clone())
                            .collect();
                        drop(read_records);
                        return Ok(result);
                    }
                }
            },
        }

        return Ok(vec![]);
    }

    #[inline]
    async fn insert_record(&self, record: ResourceRecord) -> Result<(), AsyncTreeCacheError> {
        let question = Question::new(record.name().clone(), record.rtype(), record.rclass());
        let node = self.cache.get_or_create_node(&question).await?;
        let mut write_records = node.records.write().await;
        if let Some(cached_records) = write_records.get_mut(&question.qtype()) {
            if !cached_records.iter().any(|cached_record| cached_record.matches(&record)) {
                cached_records.push(record);
            }
        } else {
            write_records.insert(
                question.qtype(),
                vec![record]
            );
        }
        drop(write_records);
        Ok(())
    }
}

#[async_trait]
impl AsyncTransactionCache for AsyncTransactionTreeCache {
    async fn get(&self, query: &dns_lib::query::message::Message) -> dns_lib::query::message::Message {
        // This function is only designed to answer one question at a time.
        // In the future, I might consider expanding this to allow multiple
        // questions if it makes sense to.
        if query.question().len() != 1 {
            return Message {
                id: query.id,
                qr: query.qr,
                opcode: query.opcode,
                authoritative_answer: false,
                truncation: false,
                recursion_desired: query.recursion_desired,
                recursion_available: false,
                z: u3::new(0),
                rcode: RCode::NotImp,
                question: query.question.clone(),
                answer: vec![],
                authority: vec![],
                additional: vec![],
            };
        }

        match self.get_records(&query.question()[0]).await {
            Ok(records) => return Message {
                id: query.id,
                qr: query.qr,
                opcode: query.opcode,
                authoritative_answer: false,
                truncation: false,
                recursion_desired: query.recursion_desired,
                recursion_available: false,
                z: u3::new(0),
                rcode: RCode::NoError,
                question: query.question.clone(),
                answer: records,
                authority: vec![],
                additional: vec![],
            },
            Err(_) => return Message {
                id: query.id,
                qr: query.qr,
                opcode: query.opcode,
                authoritative_answer: false,
                truncation: false,
                recursion_desired: query.recursion_desired,
                recursion_available: false,
                z: u3::new(0),
                rcode: RCode::ServFail,
                question: query.question.clone(),
                answer: vec![],
                authority: vec![],
                additional: vec![],
            },
        }
    }

    async fn insert(&self, records: &dns_lib::query::message::Message) {
        for record in records.answer().iter()
            .chain(records.additional().iter())
            .chain(records.authority().iter())
            // Records with TTL == 0 are not supposed to be cached
            .filter(|record| record.ttl().as_secs() != 0) {
            let _ = self.insert_record(record.clone()).await;
        }
    }
}

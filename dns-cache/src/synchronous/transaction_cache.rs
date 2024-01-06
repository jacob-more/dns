use dns_lib::{resource_record::{resource_record::ResourceRecord, rtype::RType, rcode::RCode}, query::{question::Question, message::Message}, interface::cache::transaction_cache::TransactionCache};
use ux::u3;

use super::tree_cache::{TreeCache, TreeCacheError};

type Records = Vec<ResourceRecord>;

#[derive(Debug)]
pub struct TransactionTreeCache {
    cache: TreeCache<Records>
}

impl TransactionTreeCache {
    #[inline]
    pub fn new() -> Self {
        Self { cache: TreeCache::new() }
    }

    #[inline]
    fn get_records(&self, question: &Question) -> Result<Records, TreeCacheError> {
        match question.qtype() {
            RType::ANY => {
                if let Some(node) = self.cache.get_node(question)? {
                    return Ok(node.records.values()
                        .flat_map(|records| records.iter().map(|record| record.clone()))
                        .collect());
                }
            },
            _ => {
                if let Some(node) = self.cache.get_node(question)? {
                    if let Some(records) = node.records.get(&question.qtype()) {
                        return Ok(records.clone());
                    }
                }
            },
        }

        return Ok(vec![]);
    }

    #[inline]
    fn insert_record(&mut self, record: ResourceRecord) -> Result<(), TreeCacheError> {
        let question = Question::new(record.name().clone(), record.rtype(), record.rclass());
        let node = self.cache.get_or_create_node(&question)?;
        if let Some(cached_records) = node.records.get_mut(&question.qtype()) {
            if !cached_records.iter().any(|cached_record| cached_record.matches(&record)) {
                cached_records.push(record);
            }
        } else {
            let new_cache_array = vec![record];
            node.records.insert(question.qtype(), new_cache_array);
        }
        Ok(())
    }

    #[inline]
    pub fn iter<'a>(&'a self) -> impl Iterator<Item = (&RType, &Records)> + 'a {
        self.cache.iter().flat_map(|node| &node.records)
    }
}

impl TransactionCache for TransactionTreeCache {
    fn get(&self, query: &dns_lib::query::message::Message) -> dns_lib::query::message::Message {
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

        match self.get_records(&query.question()[0]) {
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

    fn insert(&mut self, records: &dns_lib::query::message::Message) {
        for record in records.answer().iter()
            .chain(records.additional().iter())
            .chain(records.authority().iter()) {
            let _ = self.insert_record(record.clone());
        }
    }
}

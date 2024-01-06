use std::time::Instant;

use dns_lib::{query::{question::Question, message::Message}, resource_record::{rtype::RType, resource_record::ResourceRecord, rcode::RCode}, interface::cache::main_cache::MainCache};
use ux::u3;

use crate::cached_record::CachedRecord;

use super::tree_cache::{TreeCache, TreeCacheError};

type Records = Vec<CachedRecord>;

pub struct MainTreeCache {
    cache: TreeCache<Records>
}

impl MainTreeCache {
    #[inline]
    pub fn new() -> Self {
        Self { cache: TreeCache::new() }
    }

    #[inline]
    fn get_records(&self, question: &Question) -> Result<Vec<ResourceRecord>, TreeCacheError> {
        match question.qtype() {
            RType::ANY => {
                if let Some(node) = self.cache.get_node(question)? {
                    return Ok(node.records.values()
                        .flatten()
                        .filter(|record| !record.is_expired())
                        .map(|cache_record| cache_record.record.clone())
                        .collect());
                }
            },
            _ => {
                if let Some(node) = self.cache.get_node(question)? {
                    if let Some(records) = node.records.get(&question.qtype()) {
                        return Ok(records.iter()
                            .filter(|record| !record.is_expired())
                            .map(|cache_record| cache_record.record.clone())
                            .collect());
                    }
                }
            },
        }

        return Ok(vec![]);
    }

    #[inline]
    fn insert_record(&mut self, record: ResourceRecord, received_time: Instant) -> Result<(), TreeCacheError> {
        let question = Question::new(record.name().clone(), record.rtype(), record.rclass());
        let node = self.cache.get_or_create_node(&question)?;
        if let Some(cached_records) = node.records.get_mut(&question.qtype()) {
            let mut record_matched = false;
            let mut indexes_to_remove = Vec::new();
            // Step 1: Go through all of the cached records.
            //          If a matching record is found, update the ttl. Since the record is already cached, nothing else needs to be done.
            //          If one of the cached records has expired, record the index. It will be removed during a second pass.
            //          Keep track of if a match record was found so we can add the new one if needed.
            for (index, cached_record) in cached_records.iter_mut().enumerate() {
                if record.matches(&cached_record.record) {
                    record_matched = true;
                    cached_record.record.set_ttl(*record.ttl());
                    cached_record.insertion_time = received_time;
                }
                if cached_record.insertion_time.elapsed().as_secs() >= cached_record.record.ttl().as_secs() as u64 {
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
                cached_records.push(CachedRecord {
                    insertion_time: received_time,
                    record: record.clone(),
                });
            }
        } else {
            node.records.insert(
                question.qtype(),
                vec![CachedRecord { insertion_time: received_time, record }]
            );
        }
        Ok(())
    }

    #[inline]
    pub fn iter<'a>(&'a self) -> impl Iterator<Item = (&RType, &Records)> + 'a {
        self.cache.iter().flat_map(|node| &node.records)
    }
}

impl MainCache for MainTreeCache {
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
        let received_time = Instant::now();
        for record in records.answer().iter()
            .chain(records.additional().iter())
            .chain(records.authority().iter())
            // Records with TTL == 0 are not supposed to be cached
            .filter(|record| record.ttl().as_secs() != 0) {
            let _ = self.insert_record(record.clone(), received_time);
        }
    }

    fn clean(&mut self) {
        todo!()
    }
}

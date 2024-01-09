use dns_lib::{interface::cache::{cache::Cache, transaction_cache::TransactionCache, main_cache::MainCache}, query::{message::Message, qr::QR}, resource_record::{rcode::RCode, opcode::OpCode}};
use ux::u3;

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
    fn get(&self, query: &Message) -> Message {
        let mut transaction_response = self.transaction_cache.get(query);
        let main_response = self.main_cache.get(query);
        match (transaction_response.rcode, main_response.rcode) {
            (RCode::NoError, RCode::NoError) => {
                Message {
                    id: query.id,
                    qr: QR::Response,
                    opcode: OpCode::Query,
                    authoritative_answer: transaction_response.authoritative_answer && main_response.authoritative_answer,
                    truncation: transaction_response.authoritative_answer || main_response.authoritative_answer,
                    recursion_desired: query.recursion_desired,
                    recursion_available: false,
                    z: u3::new(0),
                    rcode: RCode::NoError,
                    question: transaction_response.question,
                    answer: {
                        for main_record in main_response.answer {
                            if !transaction_response.answer.iter().any(|transaction_record| main_record.matches(transaction_record)) {
                                transaction_response.answer.push(main_record);
                            }
                        }
                        transaction_response.answer
                    },
                    authority: {
                        for main_record in main_response.authority {
                            if !transaction_response.authority.iter().any(|transaction_record| main_record.matches(transaction_record)) {
                                transaction_response.authority.push(main_record);
                            }
                        }
                        transaction_response.authority
                    },
                    additional: {
                        for main_record in main_response.additional {
                            if !transaction_response.additional.iter().any(|transaction_record| main_record.matches(transaction_record)) {
                                transaction_response.additional.push(main_record);
                            }
                        }
                        transaction_response.additional
                    },
                }
            },
            (RCode::NoError, _) => {
                Message {
                    id: query.id,
                    qr: QR::Response,
                    opcode: OpCode::Query,
                    authoritative_answer: transaction_response.authoritative_answer,
                    truncation: transaction_response.authoritative_answer,
                    recursion_desired: query.recursion_desired,
                    recursion_available: false,
                    z: u3::new(0),
                    rcode: RCode::NoError,
                    question: transaction_response.question,
                    answer: transaction_response.answer,
                    authority: transaction_response.authority,
                    additional: transaction_response.additional,
                }
            },
            // Note: The transaction cache CANNOT return an error, otherwise the overall response is
            // an error since it may hold critical records.
            (transaction_rcode, _) => {
                Message {
                    id: query.id,
                    qr: QR::Response,
                    opcode: OpCode::Query,
                    authoritative_answer: transaction_response.authoritative_answer && main_response.authoritative_answer,
                    truncation: transaction_response.authoritative_answer || main_response.authoritative_answer,
                    recursion_desired: query.recursion_desired,
                    recursion_available: false,
                    z: u3::new(0),
                    rcode: transaction_rcode,
                    question: transaction_response.question,
                    answer: vec![],
                    authority: vec![],
                    additional: vec![],
                }
            },
        }
    }

    fn insert(&mut self, records: &Message) {
        self.transaction_cache.insert(records);
        self.main_cache.insert(records);
    }
}

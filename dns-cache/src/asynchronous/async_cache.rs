use std::sync::Arc;

use async_trait::async_trait;
use dns_lib::{interface::cache::{cache::AsyncCache, transaction_cache::AsyncTransactionCache, main_cache::AsyncMainCache}, query::{message::Message, qr::QR}, resource_record::{rcode::RCode, opcode::OpCode}};
use tokio::join;
use ux::u3;

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
    async fn get(&self, query: &Message) -> Message {
        let (mut transaction_response, main_response) = join!(
            self.transaction_cache.get(query),
            self.main_cache.get(query),
        );
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

    async fn insert(&self, records: &Message) {
        join!(
            self.transaction_cache.insert(records),
            self.main_cache.insert(records),
        );
    }
}

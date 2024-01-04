use async_trait::async_trait;

use crate::query::message::Message;

pub trait TransactionCache {
    fn get(&self, query: &Message) -> Message;
    fn insert(&mut self, records: &Message);
}

#[async_trait]
pub trait AsyncTransactionCache {
    async fn get(&self, query: &Message) -> Message;
    async fn insert(&mut self, records: &Message);
}

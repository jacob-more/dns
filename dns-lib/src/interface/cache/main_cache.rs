use async_trait::async_trait;

use crate::query::message::Message;

pub trait MainCache {
    fn get(&self, query: &Message);
    fn insert(&mut self, records: &Message);
}

#[async_trait]
pub trait AsyncMainCache {
    async fn get(&self, query: &Message);
    async fn insert(&mut self, records: &Message);
}

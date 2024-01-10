use std::{sync::Arc, fmt::Display};

use async_trait::async_trait;

use crate::{query::{question::Question, message::Message}, resource_record::{resource_record::ResourceRecord, rcode::RCode}};

#[derive(Debug)]
pub enum Response {
    Answer(Answer),
    Error(RCode),
}

impl Display for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Response::Answer(answer) => write!(f, "Answer:\n{answer}"),
            Response::Error(rcode) => write!(f, "Error: {rcode}"),
        }
    }
}

#[derive(Debug)]
pub struct Answer {
    pub records: Vec<ResourceRecord>,
    pub authoritative: bool,
}

impl Display for Answer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut record_iter = self.records.iter();
        match record_iter.next() {
            Some(record) => write!(f, "{record}")?,
            None => return Ok(()),
        }
        for record in record_iter {
            write!(f, "\n{record}")?;
        }
        Ok(())
    }
}

pub trait Client {
    fn query(&mut self, question: &Question) -> Message;
}

#[async_trait]
pub trait AsyncClient: Sync + Send {
    async fn query(client: Arc<Self>, question: &Question) -> Response;
}

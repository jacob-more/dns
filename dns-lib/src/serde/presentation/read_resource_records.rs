use crate::resource_record::resource_record::ResourceRecord;

use super::{tokenizer::tokenizer::Tokenizer, from_tokenized_record::FromTokenizedRecord, errors::TokenizedRecordError};

pub struct ResourceRecordReader<'a> {
    tokenizer: Tokenizer<'a>
}

impl<'a> ResourceRecordReader<'a> {
    #[inline]
    pub fn new(feed: &'a str) -> Self {
        Self { tokenizer: Tokenizer::new(feed) }
    }
}

impl<'a> Iterator for ResourceRecordReader<'a> {
    type Item = Result<ResourceRecord, TokenizedRecordError<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        let next_record_token = match self.tokenizer.next() {
            Some(Ok(record)) => record,
            Some(Err(error)) => return Some(Err(TokenizedRecordError::from(error))),
            None => return None,
        };
        match ResourceRecord::from_tokenized_record(&next_record_token) {
            Ok(record) => Some(Ok(record)),
            Err(error) => Some(Err(error)),
        }
    }
}

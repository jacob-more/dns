use super::{tokenizer::tokenizer::ResourceRecord, errors::TokenizedRecordError};

/// https://datatracker.ietf.org/doc/html/rfc1035#section-5
pub trait FromTokenizedRecord {
    fn from_tokenized_record<'a, 'b>(record: &ResourceRecord<'a>) -> Result<Self, TokenizedRecordError<'b>> where Self: Sized, 'a: 'b;
}

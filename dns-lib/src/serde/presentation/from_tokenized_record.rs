use super::{tokenizer::tokenizer::ResourceRecord, errors::TokenizedRecordError};

/// https://datatracker.ietf.org/doc/html/rfc1035#section-5
pub trait FromTokenizedRecord {
    fn from_tokenized_record<'a>(record: &'a ResourceRecord) -> Result<Self, TokenizedRecordError<'a>> where Self: Sized;
}

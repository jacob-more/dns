use super::{tokenizer::tokenizer::ResourceRecord, errors::TokenizedRecordError};

/// https://datatracker.ietf.org/doc/html/rfc1035#section-5
pub trait ToTokenizedRecord {
    fn to_tokenized_record(&self) -> Result<ResourceRecord, TokenizedRecordError>;
}

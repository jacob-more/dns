use super::{tokenizer::tokenizer::ResourceRecord, errors::TokenizedRDataError};

/// https://datatracker.ietf.org/doc/html/rfc1035#section-5
pub trait ToTokenizedRData {
    fn to_tokenized_rdata(&self) -> Result<ResourceRecord, TokenizedRDataError>;
}

use super::{tokenizer::tokenizer::ResourceRecord, errors::TokenizedRDataError};

/// https://datatracker.ietf.org/doc/html/rfc1035#section-5
pub trait FromTokenizedRData {
    fn from_tokenized_rdata<'a>(rdata: &'a ResourceRecord) -> Result<Self, TokenizedRDataError<'a>> where Self: Sized;
}

use super::errors::TokenizedRecordError;

/// https://datatracker.ietf.org/doc/html/rfc1035#section-5
pub trait FromTokenizedRData {
    fn from_tokenized_rdata(record: &Vec<&str>) -> Result<Self, TokenizedRecordError> where Self: Sized;
}

use super::errors::TokenizedRecordError;

/// https://datatracker.ietf.org/doc/html/rfc1035#section-5
pub trait FromTokenizedRData {
    fn from_tokenized_rdata<'a, 'b>(record: &Vec<&'a str>) -> Result<Self, TokenizedRecordError<'b>> where Self: Sized, 'a: 'b;
}

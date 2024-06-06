use std::path::Path;

use crate::{resource_record::resource_record::ResourceRecord, types::c_domain_name::CDomainName};

use super::{tokenizer::tokenizer::{Tokenizer, Token}, errors::TokenizedRecordError, from_presentation::FromPresentation};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum ZoneToken<'a> {
    ResourceRecord(ResourceRecord),
    Include{ file_path: &'a Path, domain_name: Option<CDomainName> }
}

pub struct ZoneFileReader<'a> {
    tokenizer: Tokenizer<'a>
}

impl<'a> ZoneFileReader<'a> {
    #[inline]
    pub fn new(feed: &'a str) -> Self {
        Self { tokenizer: Tokenizer::new(feed) }
    }

    #[inline]
    pub fn set_origin(&mut self, origin: &'a str) {
        self.tokenizer.origin = Some(origin);
    }
}

impl<'a> Iterator for ZoneFileReader<'a> {
    type Item = Result<ZoneToken<'a>, TokenizedRecordError<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        let next_token = match self.tokenizer.next() {
            Some(Ok(record)) => record,
            Some(Err(error)) => return Some(Err(TokenizedRecordError::from(error))),
            None => return None,
        };

        match next_token {
            Token::ResourceRecord(record) => match ResourceRecord::from_tokenized_record(&record) {
                Ok(record) => Some(Ok(ZoneToken::ResourceRecord(record))),
                Err(error) => Some(Err(error)),
            },
            Token::Include { file_name, domain_name } => {
                let domain_name = match domain_name {
                    Some(domain_name_str) => match CDomainName::from_token_format(&[domain_name_str]) {
                        Ok((domain_name, _)) => Some(domain_name),
                        Err(error) => return Some(Err(TokenizedRecordError::TokenError(error))),
                    },
                    None => None,
                };

                Some(Ok(ZoneToken::Include {
                    file_path: Path::new(file_name),
                    domain_name
                }))
            },
        }

    }
}

use std::fmt::Display;

use crate::serde::presentation::tokenizer::token_entries::Entry;

use super::{token_entries::{EntryIter, StringLiteral}, errors::TokenizerError};

const DEFAULT_DOMAIN_NAME: Option<&str> = None;
const DEFAULT_TTL: Option<&str> = Some("86400");
const DEFAULT_CLASS: Option<&str> = Some("IN");

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Token<'a> {
    ResourceRecord(ResourceRecordToken<'a>),
    Include{ file_name: &'a str, domain_name: Option<&'a str> }
}

impl<'a> Display for Token<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ResourceRecord(record) => write!(f, "{record}"),
            Self::Include{ file_name, domain_name } => {
                write!(f, "Include")?;
                write!(f, "\tFile Name: '{file_name}'")?;
                match domain_name {
                    Some(domain_name) => write!(f, "Origin: '{domain_name}'"),
                    None => Ok(()),
                }
            },
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ResourceRecordToken<'a> {
    pub domain_name: &'a str,
    pub ttl: &'a str,
    pub rclass: &'a str,
    pub rtype: &'a str,
    pub rdata: Vec<&'a str>,
}

impl<'a> Display for ResourceRecordToken<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Resource Record")?;
        writeln!(f, "\tDomain Name: {}", self.domain_name)?;
        writeln!(f, "\tTTL: {}", self.ttl)?;
        writeln!(f, "\tClass: {}", self.rclass)?;
        writeln!(f, "\tType: {}", self.rtype)?;
        for rdata in &self.rdata {
            writeln!(f, "\tRData: {}", rdata)?;
        }
        Ok(())
    }
}

pub struct Tokenizer<'a> {
    last_domain_name: Option<&'a str>,
    last_ttl: Option<&'a str>,
    last_rclass: Option<&'a str>,
    /// The most recent domain name associated with a $ORIGIN token. Free standing `@` is replaced
    /// by the iterator. However, relative domain names are not updated by the iterator and may need
    /// to be updated by the user.
    pub origin: Option<&'a str>,
    entry_iter: EntryIter<'a>,
}

impl<'a> Tokenizer<'a> {
    #[inline]
    pub fn new(feed: &'a str) -> Self {
        Tokenizer {
            last_domain_name: DEFAULT_DOMAIN_NAME,
            last_ttl: DEFAULT_TTL,
            last_rclass: DEFAULT_CLASS,
            origin: None,
            entry_iter: EntryIter::new(feed),
        }
    }
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = Result<Token<'a>, TokenizerError<'a>>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.entry_iter.next() {
                None => return None,
                Some(Err(error)) => return Some(Err(error)),

                // Replace any free-standing `@` with the domain name defined by the $ORIGIN token
                Some(Ok(Entry::Origin{origin})) => match (origin, self.origin) {
                    (StringLiteral::Raw(origin), _) => self.origin = Some(origin),
                    (StringLiteral::Quoted(origin), _) => self.origin = Some(origin),
                    (StringLiteral::Origin, Some(_)) => (),  //< Origin remains unchanged
                    (StringLiteral::Origin, None) => return Some(Err(TokenizerError::OriginUsedBeforeDefined)),
                },
                Some(Ok(Entry::Include{ file_name, domain_name })) => {
                    // Replace any free-standing `@` with the domain name defined by the $ORIGIN token
                    let domain_name = match (domain_name, self.origin) {
                        (Some(StringLiteral::Raw(domain_name)), _) => Some(domain_name),
                        (Some(StringLiteral::Quoted(domain_name)), _) => Some(domain_name),
                        (Some(StringLiteral::Origin), Some(origin)) => Some(origin),
                        (Some(StringLiteral::Origin), None) => return Some(Err(TokenizerError::OriginUsedBeforeDefined)),
                        // The included file inherits the parent file's origin if one is not given
                        (None, Some(origin)) => Some(origin),
                        (None, None) => None,
                    };

                    return Some(Ok(Token::Include { file_name, domain_name }));
                },
                Some(Ok(Entry::ResourceRecord{domain_name, ttl, rclass, rtype, rdata})) => {
                    // Replace any free-standing `@` with the domain name defined by the $ORIGIN token
                    let domain_name = match (domain_name, self.origin) {
                        (Some(StringLiteral::Raw(domain_name)), _) => Some(domain_name),
                        (Some(StringLiteral::Quoted(domain_name)), _) => Some(domain_name),
                        (Some(StringLiteral::Origin), Some(origin)) => Some(origin),
                        (Some(StringLiteral::Origin), None) => return Some(Err(TokenizerError::OriginUsedBeforeDefined)),
                        (None, _) => None,
                    };

                    let mut raw_rdata = Vec::with_capacity(rdata.len());
                    for rdata in rdata.iter() {
                        match (rdata, self.origin) {
                            (StringLiteral::Raw(literal), _) => raw_rdata.push(*literal),
                            (StringLiteral::Quoted(literal), _) => raw_rdata.push(*literal),
                            (StringLiteral::Origin, Some(origin)) => raw_rdata.push(origin),
                            (StringLiteral::Origin, None) => return Some(Err(TokenizerError::OriginUsedBeforeDefined)),
                        }
                    }
                    let rdata = raw_rdata;

                    // Fill in any blank domain names. If one is already defined, record it as being
                    // the last known domain name.
                    let domain_name = match (domain_name, self.last_domain_name) {
                        (Some(this_domain_name), _) => {
                            self.last_domain_name = Some(this_domain_name);
                            this_domain_name
                        },
                        (None, Some(last_domain_name)) => last_domain_name,
                        (None, None) => return Some(Err(TokenizerError::BlankDomainUsedBeforeDefined)),
                    };

                    // Fill in any blank ttl's. If one is already defined, record it as being
                    // the last known ttl.
                    let ttl = match (ttl, self.last_ttl) {
                        (Some(this_ttl), _) => {
                            self.last_ttl = Some(this_ttl);
                            this_ttl
                        },
                        (None, Some(last_ttl)) => last_ttl,
                        (None, None) => return Some(Err(TokenizerError::BlankTTLUsedBeforeDefined)),
                    };

                    // Fill in any blank classes. If one is already defined, record it as being
                    // the last known class.
                    let rclass = match (rclass, self.last_rclass) {
                        (Some(this_rclass), _) => {
                            self.last_rclass = Some(this_rclass);
                            this_rclass
                        },
                        (None, Some(last_rclass)) => last_rclass,
                        (None, None) => return Some(Err(TokenizerError::BlankClassUsedBeforeDefined)),
                    };

                    return Some(Ok(Token::ResourceRecord(ResourceRecordToken { domain_name, ttl, rclass, rtype, rdata })));
                }
            }
        }
    }
}

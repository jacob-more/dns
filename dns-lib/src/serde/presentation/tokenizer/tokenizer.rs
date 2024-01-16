use std::fmt::Display;

use crate::serde::presentation::tokenizer::entry::Entry;

use super::{entry::EntryIter, errors::TokenizerError};

const DEFAULT_DOMAIN_NAME: Option<&str> = None;
const DEFAULT_TTL: Option<&str> = Some("86400");
const DEFAULT_CLASS: Option<&str> = Some("IN");

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ResourceRecord<'a> {
    pub domain_name: &'a str,
    pub ttl: &'a str,
    pub rclass: &'a str,
    pub rtype: &'a str,
    pub rdata: Vec<&'a str>,
}

impl<'a> Display for ResourceRecord<'a> {
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
    type Item = Result<ResourceRecord<'a>, TokenizerError<'a>>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.entry_iter.next() {
                None => return None,
                Some(Err(error)) => return Some(Err(error)),
    
                Some(Ok(Entry::Origin{origin})) => self.origin = Some(origin),
                Some(Ok(Entry::Include{ file_name: _, domain_name: _ })) => todo!("Load the file and read the sub-iterator"),
                Some(Ok(Entry::ResourceRecord{mut domain_name, ttl, rclass, rtype, mut rdata})) => {
                    // Replace any free-standing `@` with the domain name defined by the $ORIGIN token
                    if let Some("@") = domain_name {
                        if self.origin.is_none() { return Some(Err(TokenizerError::OriginUsedBeforeDefined)); }
                        domain_name = self.origin;
                    }
                    for rdata in rdata.iter_mut() {
                        if "@" == *rdata {
                            if let Some(origin) = self.origin {
                                *rdata = origin;
                            } else {
                                return Some(Err(TokenizerError::OriginUsedBeforeDefined));
                            }
                        }
                    }

                    // Fill in any blank domain names. If one is already defined, record it as being
                    // the last known domain name.
                    let domain_name = if let Some(this_domain_name) = domain_name {
                        self.last_domain_name = Some(this_domain_name);
                        this_domain_name
                    } else if let Some(last_domain_name) = self.last_domain_name {
                        last_domain_name
                    } else {
                        return Some(Err(TokenizerError::BlankDomainUsedBeforeDefined));
                    };

                    // Fill in any blank ttl's. If one is already defined, record it as being
                    // the last known ttl.
                    let ttl = if let Some(this_ttl) = ttl {
                        self.last_ttl = Some(this_ttl);
                        this_ttl
                    } else if let Some(last_ttl) = self.last_ttl {
                        last_ttl
                    } else {
                        return Some(Err(TokenizerError::BlankDomainUsedBeforeDefined));
                    };

                    // Fill in any blank classes. If one is already defined, record it as being
                    // the last known class.
                    let rclass = if let Some(this_rclass) = rclass {
                        self.last_rclass = Some(this_rclass);
                        this_rclass
                    } else if let Some(last_rclass) = self.last_rclass {
                        last_rclass
                    } else {
                        return Some(Err(TokenizerError::BlankDomainUsedBeforeDefined));
                    };

                    return Some(Ok(ResourceRecord { domain_name, ttl, rclass, rtype, rdata }))
                }
            }
        }
    }
}

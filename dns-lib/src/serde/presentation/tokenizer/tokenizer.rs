use crate::serde::presentation::tokenizer::entry::Entry;

use super::{entry::EntryIter, errors::TokenizerError};

pub use super::entry::ResourceRecord;

pub struct Tokenizer<'a> {
    /// The most recent domain name. Used to fill in `None` domain names.
    last_domain_name: Option<&'a str>,
    /// The most recent domain name associated with a $ORIGIN token. Free standing `@` is replaced
    /// by the iterator. However, relative domain names are not updated by the iterator and may need
    /// to be updated by the user.
    pub origin: Option<&'a str>,
    entry_iter: EntryIter<'a>,
}

impl<'a> Tokenizer<'a> {
    pub fn new(feed: &'a str) -> Self {
        Tokenizer {
            last_domain_name: None,
            origin: None,
            entry_iter: EntryIter::new(feed),
        }
    }
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = Result<ResourceRecord<'a>, TokenizerError<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.entry_iter.next() {
                None => return None,
                Some(Err(error)) => return Some(Err(error)),
    
                Some(Ok(Entry::Origin(origin))) => self.origin = Some(origin.origin),
                Some(Ok(Entry::Include(_))) => todo!("Load the file and read the sub-iterator"),
                Some(Ok(Entry::ResourceRecord(mut rr))) => {
                    // Replace any free-standing `@` with the domain name defined by the $ORIGIN token
                    if let Some("@") = rr.domain_name {
                        if self.origin.is_none() { return Some(Err(TokenizerError::OriginUsedBeforeDefined)); }
                        rr.domain_name = self.origin;
                    }
                    for rdata in rr.rdata.iter_mut() {
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
                    if let Some(this_domain_name) = rr.domain_name {
                        self.last_domain_name = Some(this_domain_name);
                    } if let Some(last_domain_name) = self.last_domain_name {
                        rr.domain_name = Some(last_domain_name);
                    } else {
                        return Some(Err(TokenizerError::BlankDomainUsedBeforeDefined));
                    }

                    return Some(Ok(rr));
                }
            }
        }
    }
}

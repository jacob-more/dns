use std::fmt::Display;

use super::{
    errors::TokenizerError,
    raw_entries::{RawEntryIter, RawItem},
    regex::{REGEX_RCLASS, REGEX_RTYPE, REGEX_TTL},
};

/// An entry, representing a single entry in a zone file.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Entry<'a> {
    /// Using the "$ORIGIN" string, sets the origin that will be used from this point forwards while
    /// parsing (unless changed by another Origin entry). The `origin` value should be a fully
    /// qualified domain name.
    Origin { origin: StringLiteral<'a> },
    /// Using the "$TTL" string, sets the ttl that will be used from this point forwards while
    /// parsing (unless changed by another TTL entry).
    TTL { ttl: &'a str },
    /// Using the "$RCLASS" string, sets the rclass that will be used from this point forwards while
    /// parsing (unless changed by another RCLASS entry).
    RClass { rclass: &'a str },
    /// Indicates that another file should be read in at this point. The optional `domain_name` sets
    /// the initial origin when reading that file but does not affect the current origin in this
    /// file.
    Include {
        file_name: &'a str,
        domain_name: Option<StringLiteral<'a>>,
    },
    /// An entry that represents the tokens that make up a resource record. The literals that make
    /// up the record are still raw strings but some meaning has been determined based on what the
    /// strings contain in order to determine which values each literal represents.
    ResourceRecord {
        domain_name: Option<StringLiteral<'a>>,
        ttl: Option<&'a str>,
        rclass: Option<&'a str>,
        rtype: &'a str,
        rdata: Vec<StringLiteral<'a>>,
    },
}

impl<'a> Display for Entry<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Entry::Origin { origin } => {
                writeln!(f, "Origin")?;
                writeln!(f, "\tDomain Name: {origin}")
            }
            Entry::TTL { ttl } => {
                writeln!(f, "TTL")?;
                writeln!(f, "\tTTL: {ttl}")
            }
            Entry::RClass { rclass } => {
                writeln!(f, "RCLASS")?;
                writeln!(f, "\tClass: {rclass}")
            }
            Entry::Include {
                file_name,
                domain_name,
            } => {
                writeln!(f, "Include")?;
                writeln!(f, "\tFile Name: {file_name}")?;
                if let Some(domain_name) = domain_name {
                    writeln!(f, "\tDomain Name: {domain_name}")?;
                }
                Ok(())
            }
            Entry::ResourceRecord {
                domain_name,
                ttl,
                rclass,
                rtype,
                rdata,
            } => {
                writeln!(f, "Resource Record")?;
                if let Some(domain_name) = domain_name {
                    writeln!(f, "\tDomain Name: {domain_name}")?;
                }
                if let Some(ttl) = ttl {
                    writeln!(f, "\tTTL: {ttl}")?;
                }
                if let Some(rclass) = rclass {
                    writeln!(f, "\tClass: {rclass}")?;
                }
                writeln!(f, "\tType: {rtype}")?;
                for rdata in rdata {
                    writeln!(f, "\tRData: {rdata}")?;
                }
                Ok(())
            }
        }
    }
}

/// A string representing a data value in the [Entry] that could be the origin.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum StringLiteral<'a> {
    Raw(&'a str),
    Quoted(&'a str),
    Origin,
}

impl<'a> Display for StringLiteral<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StringLiteral::Raw(string) => write!(f, "{string}"),
            StringLiteral::Quoted(string) => write!(f, "{string}"),
            StringLiteral::Origin => write!(f, "@"),
        }
    }
}

/// Parses out simple meaning from the raw literal entries that were parsed out of the feed to
/// determine what types of values that entry contains. However, it does not validate for
/// correctness of most of those values. They are still stored as raw strings at this point.
pub struct EntryIter<'a> {
    token_iter: RawEntryIter<'a>,
}

impl<'a> EntryIter<'a> {
    #[inline]
    pub fn new(feed: &'a str) -> Self {
        EntryIter {
            token_iter: RawEntryIter::new(feed),
        }
    }
}

impl<'a> Iterator for EntryIter<'a> {
    type Item = Result<Entry<'a>, TokenizerError>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let entry_tokens = match self.token_iter.next() {
                Some(Ok(entry_tokens)) => entry_tokens,
                Some(Err(error)) => return Some(Err(error)),
                None => return None,
            };

            match entry_tokens.as_slice() {
                // <blank>[<comment>]
                &[] => continue, //< Skip entries that are empty

                // $ORIGIN <domain-name> [<comment>]
                &[RawItem::Text("$ORIGIN"), RawItem::Text("@")] => {
                    return Some(Ok(Entry::Origin {
                        origin: StringLiteral::Origin,
                    }));
                }
                &[RawItem::Text("$ORIGIN"), RawItem::Text(domain_name)] => {
                    return Some(Ok(Entry::Origin {
                        origin: StringLiteral::Raw(domain_name),
                    }));
                }
                &[RawItem::Text("$ORIGIN"), RawItem::QuotedText(domain_name)] => {
                    return Some(Ok(Entry::Origin {
                        origin: StringLiteral::Quoted(domain_name),
                    }));
                }

                // $TTL <ttl> [<comment>]
                &[
                    RawItem::Text("$TTL"),
                    RawItem::Text(ttl_str) | RawItem::QuotedText(ttl_str),
                ] => return Some(Ok(Entry::TTL { ttl: ttl_str })),

                // $RCLASS <rclass> [<comment>]
                &[
                    RawItem::Text("$RCLASS"),
                    RawItem::Text(rclass_str) | RawItem::QuotedText(rclass_str),
                ] => return Some(Ok(Entry::RClass { rclass: rclass_str })),

                // $INCLUDE <file-name> [<domain-name>] [<comment>]
                &[RawItem::Text("$INCLUDE"), RawItem::Text(file_name)] => {
                    return Some(Ok(Entry::Include {
                        file_name,
                        domain_name: None,
                    }));
                }
                &[
                    RawItem::Text("$INCLUDE"),
                    RawItem::Text(file_name) | RawItem::QuotedText(file_name),
                    RawItem::Text("@"),
                ] => {
                    return Some(Ok(Entry::Include {
                        file_name,
                        domain_name: Some(StringLiteral::Origin),
                    }));
                }
                &[
                    RawItem::Text("$INCLUDE"),
                    RawItem::Text(file_name) | RawItem::QuotedText(file_name),
                    RawItem::Text(domain_name),
                ] => {
                    return Some(Ok(Entry::Include {
                        file_name,
                        domain_name: Some(StringLiteral::Raw(domain_name)),
                    }));
                }
                &[
                    RawItem::Text("$INCLUDE"),
                    RawItem::Text(file_name) | RawItem::QuotedText(file_name),
                    RawItem::QuotedText(domain_name),
                ] => {
                    return Some(Ok(Entry::Include {
                        file_name,
                        domain_name: Some(StringLiteral::Quoted(domain_name)),
                    }));
                }

                // <domain-name> [<TTL>] [<class>] <type> <RDATA> [<comment>]
                &[RawItem::Text("@"), ..] => {
                    return Some(Self::parse_rr(
                        Some(StringLiteral::Origin),
                        &entry_tokens.as_slice()[1..],
                    ));
                }
                &[RawItem::Text(domain_name), ..] => {
                    return Some(Self::parse_rr(
                        Some(StringLiteral::Raw(domain_name)),
                        &entry_tokens.as_slice()[1..],
                    ));
                }
                &[RawItem::QuotedText(domain_name), ..] => {
                    return Some(Self::parse_rr(
                        Some(StringLiteral::Quoted(domain_name)),
                        &entry_tokens.as_slice()[1..],
                    ));
                }
                // <blank> [<TTL>] [<class>] <type> <RDATA> [<comment>]
                &[RawItem::Separator(_), ..] => {
                    return Some(Self::parse_rr(None, &entry_tokens.as_slice()[1..]));
                }
            }
        }
    }
}

impl<'a> EntryIter<'a> {
    #[inline]
    fn new_rr<'b>(
        domain_name: Option<StringLiteral<'a>>,
        ttl: Option<&'a str>,
        rclass: Option<&'a str>,
        rtype: &'a str,
        rdata: impl Iterator<Item = &'b RawItem<'a>>,
    ) -> Entry<'a>
    where
        'a: 'b,
    {
        Entry::ResourceRecord {
            domain_name,
            ttl,
            rclass,
            rtype,
            rdata: rdata
                .map(|token| match token {
                    RawItem::Text("@") => StringLiteral::Origin,
                    RawItem::Text(string) => StringLiteral::Raw(string),
                    RawItem::QuotedText(string) => StringLiteral::Quoted(string),
                    RawItem::Separator(string) => StringLiteral::Raw(string),
                })
                .collect(),
        }
    }

    #[inline]
    fn parse_rr(
        domain_name: Option<StringLiteral<'a>>,
        other_tokens: &[RawItem<'a>],
    ) -> Result<Entry<'a>, TokenizerError> {
        match other_tokens {
            &[RawItem::Text(token_1), RawItem::Text(token_2), ..] => {
                // If the first token is an rtype, then the rest is the rdata and we should not read it
                if (!REGEX_RCLASS.is_match(token_1)) && REGEX_RTYPE.is_match(token_1) {
                    return Self::parse_rr_rtype_first(domain_name, token_1, &other_tokens[1..]);
                }

                if (!REGEX_RCLASS.is_match(token_2)) && REGEX_RTYPE.is_match(token_2) {
                    return Self::parse_rr_rtype_second(
                        domain_name,
                        token_1,
                        token_2,
                        &other_tokens[2..],
                    );
                }

                // The match case only covers a minimum of 2 tokens. This case can only happen if
                // there are at least 3.
                if other_tokens.len() >= 3 {
                    let token_3 = other_tokens[2].into();
                    if (!REGEX_RCLASS.is_match(token_3)) && REGEX_RTYPE.is_match(token_3) {
                        return Self::parse_rr_rtype_third(
                            domain_name,
                            token_1,
                            token_2,
                            token_3,
                            &other_tokens[3..],
                        );
                    } else {
                        return Err(TokenizerError::UnknownToken(token_3.to_string()));
                    }
                }

                return Err(TokenizerError::TwoUnknownTokens(
                    token_1.to_string(),
                    token_2.to_string(),
                ));
            }
            _ => return Err(TokenizerError::UnknownTokens),
        }
    }

    #[inline]
    fn parse_rr_rtype_first(
        domain_name: Option<StringLiteral<'a>>,
        rtype: &'a str,
        other_tokens: &[RawItem<'a>],
    ) -> Result<Entry<'a>, TokenizerError> {
        Ok(Self::new_rr(
            domain_name,
            None,
            None,
            rtype,
            other_tokens.iter(),
        ))
    }

    #[inline]
    fn parse_rr_rtype_second(
        domain_name: Option<StringLiteral<'a>>,
        token_1: &'a str,
        rtype: &'a str,
        other_tokens: &[RawItem<'a>],
    ) -> Result<Entry<'a>, TokenizerError> {
        if REGEX_RCLASS.is_match(token_1) {
            Ok(Self::new_rr(
                domain_name,
                None,
                Some(token_1),
                rtype,
                other_tokens.iter(),
            ))
        } else if REGEX_TTL.is_match(token_1) {
            Ok(Self::new_rr(
                domain_name,
                Some(token_1),
                None,
                rtype,
                other_tokens.iter(),
            ))
        } else {
            Err(TokenizerError::UnknownToken(token_1.to_string()))
        }
    }

    #[inline]
    fn parse_rr_rtype_third(
        domain_name: Option<StringLiteral<'a>>,
        token_1: &'a str,
        token_2: &'a str,
        rtype: &'a str,
        other_tokens: &[RawItem<'a>],
    ) -> Result<Entry<'a>, TokenizerError> {
        if REGEX_RCLASS.is_match(token_1) && REGEX_TTL.is_match(token_2) {
            Ok(Self::new_rr(
                domain_name,
                Some(token_2),
                Some(token_1),
                rtype,
                other_tokens.iter(),
            ))
        } else if REGEX_TTL.is_match(token_1) && REGEX_RCLASS.is_match(token_2) {
            Ok(Self::new_rr(
                domain_name,
                Some(token_1),
                Some(token_2),
                rtype,
                other_tokens.iter(),
            ))
        } else {
            Err(TokenizerError::TwoUnknownTokens(
                token_1.to_string(),
                token_2.to_string(),
            ))
        }
    }
}

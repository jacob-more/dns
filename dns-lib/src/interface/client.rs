use std::{error::Error, fmt::Display, sync::Arc};

use async_trait::async_trait;

use crate::{query::{message::Message, question::Question}, resource_record::{rclass::RClass, rcode::RCode, resource_record::ResourceRecord, rtype::RType}, types::c_domain_name::{CDomainName, Labels}};

#[derive(Debug)]
pub enum Response {
    Answer(Answer),
    Error(RCode),
}

impl Display for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Response::Answer(answer) => write!(f, "Answer:\n{answer}"),
            Response::Error(rcode) => write!(f, "Error: {rcode}"),
        }
    }
}

#[derive(Debug)]
pub struct Answer {
    pub records: Vec<ResourceRecord>,
    pub authoritative: bool,
}

impl Display for Answer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut record_iter = self.records.iter();
        match record_iter.next() {
            Some(record) => write!(f, "{record}")?,
            None => return Ok(()),
        }
        for record in record_iter {
            write!(f, "\n{record}")?;
        }
        Ok(())
    }
}

pub trait Client {
    fn query(&mut self, question: &Question) -> Message;
}

#[async_trait]
pub trait AsyncClient: Sync + Send {
    async fn query(client: Arc<Self>, question: Context) -> Response;
}


#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum ContextErr {
    IllegalSearch {
        parent: String,
        child: Question,
    },
    IllegalCName {
        parent: String,
        child: Question,
    },
    CNameWillLoop {
        parent: String,
        child: Question,
    },
    IllegalDName {
        parent: String,
        child: Question,
    },
    DNameWillLoop {
        parent: String,
        child: Question,
    },
    NSWillLoop {
        parent: String,
        child: Question,
    },
}

impl Error for ContextErr {}
impl Display for ContextErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContextErr::IllegalSearch { parent, child } => write!(f, "ContextErr::IllegalSearch: Tried to create a search context for '{child}' in a context that contains '{parent}'"),
            ContextErr::IllegalCName { parent, child } => write!(f, "ContextErr::IllegalCName: Tried to create a CName context for '{child}' in a context that contains '{parent}'"),
            ContextErr::CNameWillLoop { parent, child } => write!(f, "ContextErr::CNameWillLoop: Tried to create a CName context for '{child}' in a context that contains '{parent}'"),
            ContextErr::IllegalDName { parent, child } => write!(f, "ContextErr::IllegalDName: Tried to create a DName context for '{child}' in a context that contains '{parent}'"),
            ContextErr::DNameWillLoop { parent, child } => write!(f, "ContextErr::DNameWillLoop: Tried to create a DName context for '{child}' in a context that contains '{parent}'"),
            ContextErr::NSWillLoop { parent, child } => write!(f, "ContextErr::NSWillLoop: Tried to create an NS address context for '{child}' in a context that contains '{parent}'"),
        }
    }
}

#[derive(Debug)]
pub enum Context {
    Root { query: Question },
    RootSearch {
        query: Question,
        parent: Arc<Context>,
    },
    CName {
        query: Question,
        parent: Arc<Context>,
    },
    CNameSearch {
        query: Question,
        parent: Arc<Context>,
    },
    DName {
        query: Question,
        parent: Arc<Context>,
    },
    DNameSearch {
        query: Question,
        parent: Arc<Context>,
    },
    NSAddress {
        query: Question,
        parent: Arc<Context>,
    },
    NSAddressSearch {
        query: Question,
        parent: Arc<Context>,
    },
    SubNSAddress {
        query: Question,
        parent: Arc<Context>,
    },
    SubNSAddressSearch {
        query: Question,
        parent: Arc<Context>,
    },
}

impl Context {
    #[inline]
    pub const fn new(query: Question) -> Self {
        Self::Root { query }
    }

    #[inline]
    pub fn new_search_name(self: Arc<Self>, query: Question) -> Result<Context, ContextErr> {
        match self.as_ref() {
            Context::Root { query: _ } => Ok(Self::RootSearch { query, parent: self }),
            Context::CName { query: _, parent: _ } => Ok(Self::CNameSearch { query, parent: self }),
            Context::DName { query: _, parent: _ } => Ok(Self::DNameSearch { query, parent: self }),
            Context::NSAddress { query: _, parent: _ } => Ok(Self::NSAddressSearch { query, parent: self }),
            Context::SubNSAddress { query: _, parent: _ } => Ok(Self::SubNSAddressSearch { query, parent: self }),
            Context::RootSearch { query: _, parent: _ }
          | Context::CNameSearch { query: _, parent: _ }
          | Context::DNameSearch { query: _, parent: _ }
          | Context::NSAddressSearch { query: _, parent: _ }
          | Context::SubNSAddressSearch { query: _, parent: _ } => {
                Err(ContextErr::IllegalSearch { parent: self.short_name(), child: query })
            },
        }
    }

    #[inline]
    pub fn new_cname(self: Arc<Self>, qname: CDomainName) -> Result<Context, ContextErr> {
        let query = Question::new(qname, self.qtype(), self.qclass());
        match (self.is_cname_allowed(&query), self.as_ref()) {
            (Err(error), _) => Err(error),
            (Ok(()), Context::Root { query: _ })
          | (Ok(()), Context::CName { query: _, parent: _ })
          | (Ok(()), Context::DName { query: _, parent: _ }) => {
                Ok(Self::CName { query, parent: self})
            },
            (Ok(()), Context::RootSearch { query: _, parent: _ })
          | (Ok(()), Context::CNameSearch { query: _, parent: _ })
          | (Ok(()), Context::DNameSearch { query: _, parent: _ })
          | (Ok(()), Context::NSAddress { query: _, parent: _ })
          | (Ok(()), Context::NSAddressSearch { query: _, parent: _ })
          | (Ok(()), Context::SubNSAddress { query: _, parent: _ })
          | (Ok(()), Context::SubNSAddressSearch { query: _, parent: _ }) => {
                Err(ContextErr::IllegalCName { parent: self.short_name(), child: query })
            },
        }
    }

    #[inline]
    pub fn new_dname(self: Arc<Self>, qname: CDomainName) -> Result<Context, ContextErr> {
        let query = Question::new(qname, self.qtype(), self.qclass());
        match (self.is_dname_allowed(&query), self.as_ref()) {
            (Err(error), _) => Err(error),
            (Ok(()), Context::Root { query: _ })
          | (Ok(()), Context::CName { query: _, parent: _ })
          | (Ok(()), Context::DName { query: _, parent: _ }) => {
                Ok(Self::DName { query, parent: self })
            },
            (Ok(()), Context::RootSearch { query: _, parent: _ })
          | (Ok(()), Context::CNameSearch { query: _, parent: _ })
          | (Ok(()), Context::DNameSearch { query: _, parent: _ })
          | (Ok(()), Context::NSAddress { query: _, parent: _ })
          | (Ok(()), Context::NSAddressSearch { query: _, parent: _ })
          | (Ok(()), Context::SubNSAddress { query: _, parent: _ })
          | (Ok(()), Context::SubNSAddressSearch { query: _, parent: _ }) => {
                Err(ContextErr::IllegalDName { parent: self.short_name(), child: query })
            },
        }
    }

    #[inline]
    pub fn new_ns_address(self: Arc<Self>, query: Question) -> Result<Context, ContextErr> {
        match (self.is_ns_allowed(&query), self.as_ref()) {
            (Err(error), _) => Err(error),
            (Ok(()), Context::Root { query: _ })
          | (Ok(()), Context::RootSearch { query: _, parent: _ })
          | (Ok(()), Context::CName { query: _, parent: _ })
          | (Ok(()), Context::CNameSearch { query: _, parent: _ })
          | (Ok(()), Context::DName { query: _, parent: _ })
          | (Ok(()), Context::DNameSearch { query: _, parent: _ }) => {
                Ok(Self::NSAddress { query, parent: self })
            },
            (Ok(()), Context::NSAddress { query: _, parent: _ })
          | (Ok(()), Context::NSAddressSearch { query: _, parent: _ })
          | (Ok(()), Context::SubNSAddress { query: _, parent: _ })
          | (Ok(()), Context::SubNSAddressSearch { query: _, parent: _ }) => {
                Ok(Self::SubNSAddress { query, parent: self })
            },
        }
    }

    #[inline]
    pub const fn query(&self) -> &Question {
        match self {
            Context::Root { query } => query,
            Context::RootSearch { query, parent: _ } => query,
            Context::CName { query, parent: _ } => query,
            Context::CNameSearch { query, parent: _ } => query,
            Context::DName { query, parent: _ } => query,
            Context::DNameSearch { query, parent: _ } => query,
            Context::NSAddress { query, parent: _ } => query,
            Context::NSAddressSearch { query, parent: _ } => query,
            Context::SubNSAddress { query, parent: _ } => query,
            Context::SubNSAddressSearch { query, parent: _ } => query,
        }
    }

    #[inline]
    pub const fn qname(&self) -> &CDomainName {
        self.query().qname()
    }

    #[inline]
    pub const fn qtype(&self) -> RType {
        self.query().qtype()
    }

    #[inline]
    pub const fn qclass(&self) -> RClass {
        self.query().qclass()
    }

    #[inline]
    pub const fn parent(&self) -> Option<&Arc<Context>> {
        match self {
            Context::Root { query: _ } => None,
            Context::RootSearch { query: _, parent } => Some(parent),
            Context::CName { query: _, parent } => Some(parent),
            Context::CNameSearch { query: _, parent } => Some(parent),
            Context::DName { query: _, parent } => Some(parent),
            Context::DNameSearch { query: _, parent } => Some(parent),
            Context::NSAddress { query: _, parent } => Some(parent),
            Context::NSAddressSearch { query: _, parent } => Some(parent),
            Context::SubNSAddress { query: _, parent } => Some(parent),
            Context::SubNSAddressSearch { query: _, parent } => Some(parent),
        }
    }

    #[inline]
    pub fn root(self: &Arc<Self>) -> &Arc<Context> {
        match self.as_ref() {
            Context::Root { query: _ } => self,
            Context::RootSearch { query: _, parent } => parent.root(),
            Context::CName { query: _, parent } => parent.root(),
            Context::CNameSearch { query: _, parent } => parent.root(),
            Context::DName { query: _, parent } => parent.root(),
            Context::DNameSearch { query: _, parent } => parent.root(),
            Context::NSAddress { query: _, parent } => parent.root(),
            Context::NSAddressSearch { query: _, parent } => parent.root(),
            Context::SubNSAddress { query: _, parent } => parent.root(),
            Context::SubNSAddressSearch { query: _, parent } => parent.root(),
        }
    }

    #[inline]
    pub fn is_cname_allowed(&self, child: &Question) -> Result<(), ContextErr> {
        match &self {
            Context::Root { query } => {
                if query.qname().is_subdomain(child.qname()) {
                    Err(ContextErr::CNameWillLoop { parent: self.short_name(), child: child.clone() })
                } else {
                    Ok(())
                }
            },
            Context::RootSearch { query, parent }
          | Context::CName { query, parent }
          | Context::CNameSearch { query, parent }
          | Context::DName { query, parent }
          | Context::DNameSearch { query, parent } => {
                if query.qname().is_subdomain(child.qname()) {
                    Err(ContextErr::CNameWillLoop { parent: self.short_name(), child: child.clone() })
                } else {
                    parent.is_cname_allowed(child)
                }
            },
            Context::NSAddress { query: _, parent: _ }
          | Context::NSAddressSearch { query: _, parent: _ }
          | Context::SubNSAddress { query: _, parent: _ }
          | Context::SubNSAddressSearch { query: _, parent: _ } => {
                Err(ContextErr::IllegalCName { parent: self.short_name(), child: child.clone() })
            },
        }
    }

    #[inline]
    pub fn is_dname_allowed(&self, child: &Question) -> Result<(), ContextErr> {
        match &self {
            Context::Root { query } => {
                if query.qname().is_subdomain(child.qname()) {
                    Err(ContextErr::DNameWillLoop { parent: self.short_name(), child: child.clone() })
                } else {
                    Ok(())
                }
            },
            Context::RootSearch { query, parent }
          | Context::CName { query, parent }
          | Context::CNameSearch { query, parent }
          | Context::DName { query, parent }
          | Context::DNameSearch { query, parent } => {
                if query.qname().is_subdomain(child.qname()) {
                    Err(ContextErr::DNameWillLoop { parent: self.short_name(), child: child.clone() })
                } else {
                    parent.is_dname_allowed(child)
                }
            },
            Context::NSAddress { query: _, parent: _ }
          | Context::NSAddressSearch { query: _, parent: _ }
          | Context::SubNSAddress { query: _, parent: _ }
          | Context::SubNSAddressSearch { query: _, parent: _ } => {
                Err(ContextErr::IllegalDName { parent: self.short_name(), child: child.clone() })
            },
        }
    }

    #[inline]
    pub fn is_ns_allowed(&self, child: &Question) -> Result<(), ContextErr> {
        match &self {
            Context::Root { query } => {
                if query.eq(child) {
                    Err(ContextErr::NSWillLoop { parent: self.short_name(), child: child.clone() })
                } else {
                    Ok(())
                }
            },
            Context::CName { query, parent }
          | Context::DName { query, parent }
          | Context::NSAddress { query, parent }
          | Context::SubNSAddress { query, parent } => {
                if query.eq(child) {
                    Err(ContextErr::NSWillLoop { parent: self.short_name(), child: child.clone() })
                } else {
                    parent.is_ns_allowed(child)
                }
            },
            Context::RootSearch { query: _, parent }
          | Context::CNameSearch { query: _, parent }
          | Context::DNameSearch { query: _, parent }
          | Context::NSAddressSearch { query: _, parent }
          | Context::SubNSAddressSearch { query: _, parent } => {
                parent.is_ns_allowed(child)
            },
        }
    }

    #[inline]
    fn short_name(&self) -> String {
        match &self {
            Context::Root { query } =>                          format!("Context::Root {{ qname: {}, qtype: {}, qclass: {} }}",                query.qname(), query.qtype(), query.qclass()),
            Context::RootSearch { query, parent: _ } =>         format!("Context::RootSearch {{ qname: {}, qtype: {}, qclass: {} }}",          query.qname(), query.qtype(), query.qclass()),
            Context::CName { query, parent: _ } =>              format!("Context::CName {{ qname: {}, qtype: {}, qclass: {} }}",               query.qname(), query.qtype(), query.qclass()),
            Context::CNameSearch { query, parent: _ } =>        format!("Context::CNameSearch {{ qname: {}, qtype: {}, qclass: {} }}",         query.qname(), query.qtype(), query.qclass()),
            Context::DName { query, parent: _ } =>              format!("Context::DName {{ qname: {}, qtype: {}, qclass: {} }}",               query.qname(), query.qtype(), query.qclass()),
            Context::DNameSearch { query, parent: _ } =>        format!("Context::DNameSearch {{ qname: {}, qtype: {}, qclass: {} }}",         query.qname(), query.qtype(), query.qclass()),
            Context::NSAddress { query, parent: _ } =>          format!("Context::NSAddress {{ qname: {}, qtype: {}, qclass: {} }}",           query.qname(), query.qtype(), query.qclass()),
            Context::NSAddressSearch { query, parent: _ } =>    format!("Context::NSAddressSearch {{ qname: {}, qtype: {}, qclass: {} }}",     query.qname(), query.qtype(), query.qclass()),
            Context::SubNSAddress { query, parent: _ } =>       format!("Context::SubNSAddress {{ qname: {}, qtype: {}, qclass: {} }}",        query.qname(), query.qtype(), query.qclass()),
            Context::SubNSAddressSearch { query, parent: _ } => format!("Context::SubNSAddressSearch {{ qname: {}, qtype: {}, qclass: {} }}",  query.qname(), query.qtype(), query.qclass()),
        }
    }
}

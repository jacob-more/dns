use std::fmt::Display;

use dns_macros::{FromWire, ToWire};

use crate::{
    resource_record::{rclass::RClass, rtype::RType},
    types::domain_name::{CompressibleDomainVec, DomainNameVec},
};

/// https://datatracker.ietf.org/doc/html/rfc1035#section-4.1.2
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire)]
pub struct Question {
    qname: CompressibleDomainVec,
    qtype: RType,
    qclass: RClass,
}

impl Default for Question {
    #[inline]
    fn default() -> Self {
        Self {
            qname: CompressibleDomainVec(DomainNameVec::new_root()),
            qtype: RType::Unknown(0),
            qclass: RClass::Unknown(0),
        }
    }
}

impl Question {
    #[inline]
    pub const fn new(qname: DomainNameVec, qtype: RType, qclass: RClass) -> Question {
        Question {
            qname: CompressibleDomainVec(qname),
            qtype,
            qclass,
        }
    }

    #[inline]
    pub const fn qname(&self) -> &DomainNameVec {
        &self.qname.0
    }

    #[inline]
    pub const fn qtype(&self) -> RType {
        self.qtype
    }

    #[inline]
    pub const fn qclass(&self) -> RClass {
        self.qclass
    }

    pub fn with_new_qname(&self, qname: DomainNameVec) -> Self {
        Question {
            qname: CompressibleDomainVec(qname),
            qtype: self.qtype,
            qclass: self.qclass,
        }
    }

    pub fn with_new_qclass(&self, qclass: RClass) -> Self {
        Question {
            qname: self.qname.clone(),
            qtype: self.qtype,
            qclass,
        }
    }

    pub fn with_new_qtype(&self, qtype: RType) -> Self {
        Question {
            qname: self.qname.clone(),
            qtype,
            qclass: self.qclass,
        }
    }

    pub fn with_new_qname_qtype(&self, qname: DomainNameVec, qtype: RType) -> Self {
        Question {
            qname: CompressibleDomainVec(qname),
            qtype,
            qclass: self.qclass,
        }
    }

    pub fn with_new_qname_qclass(&self, qname: DomainNameVec, qclass: RClass) -> Self {
        Question {
            qname: CompressibleDomainVec(qname),
            qtype: self.qtype,
            qclass,
        }
    }
}

impl Display for Question {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Question: {{qname: '{}', qtype: {}, qclass: {}}}",
            self.qname.0, self.qtype, self.qclass
        )
    }
}

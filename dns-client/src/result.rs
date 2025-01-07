use std::{fmt::{Debug, Display}, hash::Hash};

use dns_lib::{interface::client::ContextErr, resource_record::{rcode::RCode, resource_record::ResourceRecord, rtype::RType, types::ns::NS}, types::c_domain_name::{CDomainName, CDomainNameError}};
use network::errors::QueryError;


#[derive(Clone, PartialEq, Hash, Debug)]
pub(crate) struct QOk {
    pub answer: Vec<ResourceRecord>,
    pub name_servers: Vec<ResourceRecord<NS>>,
    pub additional: Vec<ResourceRecord>,
}

impl Display for QOk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "qok: {{ ")?;
        write!(f, "answer: {:?}", self.answer)?;
        write!(f, "name_servers: {:?}", self.name_servers)?;
        write!(f, "additional: {:?}", self.additional)?;
        write!(f, " }}")
    }
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) enum QError {
    ContextErr(ContextErr),
    CDomainNameErr(CDomainNameError),
    NetworkQueryErr(QueryError),
    CacheFailure(RCode),
    NoClosestNameServerFound(CDomainName),
    MissingRecord(RType),
    QNameIsNotChildOfDName {
        dname: CDomainName,
        qname: CDomainName,
    },
}

impl Display for QError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QError::ContextErr(context_err) => write!(f, "{context_err}"),
            QError::CDomainNameErr(cdomain_name_err) => write!(f, "{cdomain_name_err}"),
            QError::NetworkQueryErr(query_err) => write!(f, "{query_err}"),
            QError::CacheFailure(rcode) => write!(f, "the cache returned an error code '{rcode}'"),
            QError::NoClosestNameServerFound(domain) => write!(f, "could not find a closest name server for '{domain}'"),
            QError::MissingRecord(rtype) => write!(f, "could not find a {rtype} record in the set but one was expected"),
            QError::QNameIsNotChildOfDName { dname, qname } => write!(f, "the qname '{qname}' is not a child of the dname's owner '{dname}'"),
        }
    }
}

impl From<ContextErr> for QError {
    fn from(value: ContextErr) -> Self {
        Self::ContextErr(value)
    }
}

impl From<CDomainNameError> for QError {
    fn from(value: CDomainNameError) -> Self {
        Self::CDomainNameErr(value)
    }
}

impl From<QueryError> for QError {
    fn from(value: QueryError) -> Self {
        Self::NetworkQueryErr(value)
    }
}

#[derive(Clone, PartialEq, Hash, Debug)]
pub(crate) enum QResult<
    TOk: Clone + PartialEq + Hash + Debug + Display = QOk,
    TErr: Clone + PartialEq + Debug + Display = QError>
{
    Err(TErr),
    Fail(RCode),
    Ok(TOk),
}

impl<TOk, TErr> Display for QResult<TOk, TErr>
where
    TOk: Clone + PartialEq + Hash + Debug + Display,
    TErr: Clone + PartialEq + Debug + Display
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QResult::Err(qerror) => write!(f, "{qerror}"),
            QResult::Fail(rcode) => write!(f, "qerror: {rcode}"),
            QResult::Ok(qok) => write!(f, "{qok}"),
        }
    }
}

impl<TOk> From<QError> for QResult<TOk, QError>
where
    TOk: Clone + PartialEq + Hash + Debug + Display
{
    fn from(value: QError) -> Self {
        QResult::Err(value)
    }
}

impl<TOk, TErr> From<RCode> for QResult<TOk, TErr>
where
    TOk: Clone + PartialEq + Hash + Debug + Display,
    TErr: Clone + PartialEq + Debug + Display
{
    fn from(value: RCode) -> Self {
        QResult::Fail(value)
    }
}

impl<TErr> From<QOk> for QResult<QOk, TErr>
where
    TErr: Clone + PartialEq + Debug + Display
{
    fn from(value: QOk) -> Self {
        QResult::Ok(value)
    }
}

impl<TOk, TErr> From<Result<TOk, TErr>> for QResult<TOk, TErr>
where
    TOk: Clone + PartialEq + Hash + Debug + Display,
    TErr: Clone + PartialEq + Debug + Display
{
    fn from(value: Result<TOk, TErr>) -> Self {
        match value {
            Ok(ok) => QResult::Ok(ok),
            Err(err) => QResult::Err(err),
        }
    }
}

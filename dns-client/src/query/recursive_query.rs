use std::sync::Arc;

use async_recursion::async_recursion;
use dns_lib::{interface::{cache::{cache::AsyncCache, CacheQuery, CacheResponse}, client::Context}, query::question::Question, resource_record::{resource_record::{RecordData, ResourceRecord}, rtype::RType, types::ns::NS}, types::c_domain_name::{CDomainName, CmpDomainName}};
use log::{debug, trace};
use rand::{thread_rng, seq::SliceRandom};

use crate::{query::round_robin_query::query_name_servers, result::{QError, QOk, QResult}, DNSAsyncClient};


#[async_recursion]
pub(crate) async fn recursive_query<CCache>(client: Arc<DNSAsyncClient>, joined_cache: Arc<CCache>, context: Context) -> QResult where CCache: AsyncCache + Send + Sync + 'static {
    debug!(context:?; "Start recursive search");
    let cache_response = joined_cache.get(&CacheQuery { authoritative: false, question: context.query() }).await;
    // Initial Cache Check: Check to see if the records we're looking for are already cached.
    trace!(context:?; "Recursive search initial cache response: '{cache_response:?}'");
    match cache_response {
        CacheResponse::Records(records) if (records.len() == 0) => (),
        CacheResponse::Records(records) => return QResult::Ok(QOk {
            answer: records.into_iter().map(|record| record.record).collect(),
            name_servers: Vec::new(),
            additional: Vec::new(),
        }),
        CacheResponse::Err(rcode) => return QError::CacheFailure(rcode).into(),
    };

    // Discovery Stage: See if we have name servers that handle one of the parent domains of the
    // qname.
    let (search_names_max_index, mut name_servers) = match get_closest_name_server(&client, &joined_cache, context.query()).await {
        NSResponse::Error(error) => return error.into(),
        NSResponse::Records(search_names_max_index, name_servers) => (
            search_names_max_index,
            name_servers.into_iter().map(|record| record.into_rdata().into_name_server_domain_name()).collect::<Vec<_>>()
        ),
    };
    trace!(context:?; "Recursive search initial name servers: '{name_servers:?}'");
    // Bound the search names based on the max index we reached to make the next stage easier.
    // This will make sure we start the search with the child of the ancestor and continue
    // down the tree from there.
    let context = Arc::new(context);
    let search_names_context = context.clone();
    let search_names = search_names_context.qname().search_domains().take(search_names_max_index);

    // Query Stage: Query name servers for the next subdomain, following the tree to our answer.
    for (index, search_name) in search_names.enumerate().rev() {
        // Query the name servers for the child domain (aka. search_name).
        // We set the qtype to be RData::A to hide the actual qtype
        // that we're looking for.
        name_servers.shuffle(&mut thread_rng());

        let search_query = Question::new(search_name.clone(), RType::A, context.qclass());
        let search_context = match context.clone().new_search_name(search_query) {
            Ok(search_context) => Arc::new(search_context),
            Err(error) => {
                debug!(context:?; "Recursive search new search error: '{error}'");
                return QResult::Err(error.into())
            },
        };
        trace!(context:?; "Recursive search querying name servers '{name_servers:?}' with search context '{search_context:?}'");

        match query_name_servers(&client, &joined_cache, search_context, &name_servers).await {
            QResult::Err(error) => {
                trace!(context:?; "Recursive search querying name servers '{name_servers:?}' with search context response: error {error}");
                return error.into();
            },
            QResult::Fail(rcode) => {
                trace!(context:?; "Recursive search querying name servers '{name_servers:?}' with search context response: rcode {rcode}");
                return rcode.into();
            },
            QResult::Ok(QOk { answer, name_servers, additional }) if answer.is_empty() => {
                trace!(context:?; "Recursive search querying name servers '{name_servers:?}' with search context response: no records");
                return QResult::Ok(QOk { answer, name_servers, additional });
            },
            QResult::Ok(QOk { answer, name_servers: found_name_servers, additional: _ }) => {
                trace!(context:?; "Recursive search querying name servers '{name_servers:?}' with search context response: '{answer:?}'");

                if (index != 0) || (context.qtype() != RType::DNAME) {
                    if answer.iter().any(|record| record.get_rtype() == RType::DNAME) {
                        return handle_dname(client, joined_cache, context, answer, Vec::new(), Vec::new()).await;
                    }
                }

                if !found_name_servers.is_empty() {
                    name_servers.clear();
                    name_servers.extend(found_name_servers.into_iter().map(|record| record.into_rdata().into_name_server_domain_name()));
                }
            },
        }
    }

    // Check for various cached answers.
    match joined_cache.get(&CacheQuery { authoritative: false, question: context.query() }).await {
        CacheResponse::Err(rcode) => {
            trace!(context:?; "Recursive search secondary cache response: rcode '{rcode}'");
            return QError::CacheFailure(rcode).into();
        },
        CacheResponse::Records(cached_records) if cached_records.is_empty() => {
            trace!(context:?; "Recursive search secondary cache response: no records");
        },
        CacheResponse::Records(cached_records) => {
            trace!(context:?; "Recursive search secondary cache response: '{cached_records:?}'");
            if (context.qtype() != RType::CNAME) && cached_records.iter().any(|record| record.get_rtype() == RType::CNAME) {
                return handle_cname(client, joined_cache, context, cached_records.into_iter().map(|record| record.record).collect(), Vec::new(), Vec::new()).await;
            }

            if (context.qtype() != RType::DNAME) && cached_records.iter().any(|record| record.get_rtype() == RType::DNAME) {
                return handle_dname(client, joined_cache, context, cached_records.into_iter().map(|record| record.record).collect(), Vec::new(), Vec::new()).await;
            }

            return QResult::Ok(QOk {
                answer: cached_records.into_iter().map(|record| record.record).collect(),
                name_servers: Vec::new(),
                additional: Vec::new(),
            });
        },
    }

    // Query name servers for answers.
    trace!(context:?; "Recursive search: querying name servers '{name_servers:?}' with full context");
    match query_name_servers(&client, &joined_cache, context.clone(), &name_servers).await {
        QResult::Err(error) => {
            trace!(context:?; "Recursive search name server response: error '{error}'");
            return error.into();
        },
        QResult::Fail(rcode) => {
            trace!(context:?; "Recursive search name server response: rcode '{rcode}'");
            return rcode.into();
        },
        QResult::Ok(QOk { answer, name_servers: _, additional: _ }) if answer.is_empty() => {
            trace!(context:?; "Recursive search name server response: no records");
        },
        QResult::Ok(QOk { answer, name_servers, additional }) => {
            trace!(context:?; "Recursive search name server response: '{answer:?}'");
            if (context.qtype() != RType::CNAME) && answer.iter().any(|record| record.get_rtype() == RType::CNAME) {
                return handle_cname(client, joined_cache, context, answer, Vec::new(), Vec::new()).await;
            }

            if (context.qtype() != RType::DNAME) && answer.iter().any(|record| record.get_rtype() == RType::DNAME) {
                return handle_dname(client, joined_cache, context, answer, Vec::new(), Vec::new()).await;
            }

            return QResult::Ok(QOk { answer, name_servers, additional });
        },
    }

    trace!(context:?; "Recursive search no records found");
    return QResult::Ok(QOk {
        answer: Vec::new(),
        name_servers: Vec::new(),
        additional: Vec::new()
        });
}

#[derive(Clone, PartialEq, Hash, Debug)]
enum NSResponse {
    Records(usize, Vec<ResourceRecord<NS>>),
    Error(QError),
}

async fn get_closest_name_server<CCache>(_client: &Arc<DNSAsyncClient>, joined_cache: &Arc<CCache>, question: &Question) -> NSResponse where CCache: AsyncCache {
    for (index, search_name) in question.qname().search_domains().enumerate() {
        match joined_cache.get(&CacheQuery { authoritative: false, question: &question.with_new_qname_qtype(search_name.clone(), RType::NS) }).await {
            CacheResponse::Err(rcode) => return NSResponse::Error(QError::CacheFailure(rcode)),
            CacheResponse::Records(cached_name_servers) if cached_name_servers.is_empty() => continue,
            CacheResponse::Records(cached_name_servers) => {
                return NSResponse::Records(
                    index,
                    cached_name_servers.into_iter().filter_map(|record| record.record.try_into().ok()).collect()
                );
            },
        }
    }
    return NSResponse::Error(QError::NoClosestNameServerFound(question.qname().clone()));
}

async fn handle_cname<CCache>(client: Arc<DNSAsyncClient>, joined_cache: Arc<CCache>, context: Arc<Context>, mut answer: Vec<ResourceRecord>, name_servers: Vec<ResourceRecord<NS>>, mut additional: Vec<ResourceRecord>) -> QResult where CCache: AsyncCache + Send + Sync + 'static {
    debug!(context:?; "Recursive search redirected by cname");
    for record in &answer {
        if let RecordData::CNAME(cname_rdata) = record.get_rdata() {
            match context.clone().new_cname(cname_rdata.primary_name().clone()) {
                Ok(cname_context) => {
                    match recursive_query(client, joined_cache, cname_context).await {
                        result @ QResult::Err(_)
                      | result @ QResult::Fail(_) => {
                            return result;
                        },
                        QResult::Ok(QOk { answer: cname_answer, name_servers: cname_servers, additional: cname_additional }) => {
                            answer.extend(cname_answer);
                            additional.extend(cname_additional);
                            additional.extend(cname_servers.into_iter().map(|ns_record| ns_record.into()));
                            return QResult::Ok(QOk { answer, name_servers, additional });
                        },
                    }
                },
                Err(error) => {
                    trace!(context:?; "Recursive search new cname error: {error}");
                    return QError::ContextErr(error).into();
                },
            };
        }
    }

    trace!(context:?; "Recursive search new cname error: no cname record in records '{answer:?}'");
    return QError::MissingRecord(RType::CNAME).into();
}

async fn handle_dname<CCache>(client: Arc<DNSAsyncClient>, joined_cache: Arc<CCache>, context: Arc<Context>, mut answer: Vec<ResourceRecord>, name_servers: Vec<ResourceRecord<NS>>, mut additional: Vec<ResourceRecord>) -> QResult where CCache: AsyncCache + Send + Sync + 'static {
    debug!(context:?; "Recursive search redirected by dname");
    for record in &answer {
        if let RecordData::DNAME(dname_rdata) = record.get_rdata() {
            if !context.qname().is_parent_domain_of(record.get_name()) {
                trace!(context:?; "Recursive search new dname error: The query name '{}' is not a subdomain of the dname's owner name '{}'", context.qname(), record.get_name());
                return QError::QNameIsNotChildOfDName {
                    dname: record.get_name().clone(),
                    qname: context.qname().clone()
                }.into();
            }
            let dname = CDomainName::from_ref_labels(
                context.qname()
                    .case_sensitive_labels()
                    .take(record.get_name().label_count())
                    .chain(dname_rdata.target_name().case_sensitive_labels())
                    .collect()
            );

            let dname = match dname {
                Ok(dname) => dname,
                Err(error) => {
                    trace!(context:?; "Recursive search new cname error: {error}");
                    return QError::CDomainNameErr(error).into();
                },
            };

            match context.clone().new_dname(dname) {
                Ok(dname_context) => {
                    match recursive_query(client, joined_cache, dname_context).await {
                        result @ QResult::Err(_)
                      | result @ QResult::Fail(_) => {
                            return result;
                        },
                        QResult::Ok(QOk { answer: dname_answer, name_servers: dname_servers, additional: dname_additional }) => {
                            answer.extend(dname_answer);
                            additional.extend(dname_additional);
                            additional.extend(dname_servers.into_iter().map(|ns_record| ns_record.into()));
                            return QResult::Ok(QOk { answer, name_servers, additional });
                        },
                    }
                },
                Err(error) => {
                    trace!(context:?; "Recursive search new cname error: {error}");
                    return QError::ContextErr(error).into();
                },
            };
        }
    }

    trace!(context:?; "Recursive search new cname error: no dname record in records '{answer:?}'");
    return QError::MissingRecord(RType::DNAME).into();
}

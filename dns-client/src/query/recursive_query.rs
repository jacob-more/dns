use std::sync::Arc;

use async_recursion::async_recursion;
use dns_lib::{interface::{cache::{cache::AsyncCache, main_cache::AsyncMainCache, CacheQuery, CacheResponse}, client::Context}, query::question::Question, resource_record::{rcode::RCode, resource_record::{RecordData, ResourceRecord}, rtype::RType}, types::c_domain_name::{CDomainName, CmpDomainName}};
use log::{debug, trace};
use rand::{thread_rng, seq::SliceRandom};

use crate::{query::round_robin_query::query_name_servers, DNSAsyncClient};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) enum QueryResponse<T> {
    Error(RCode),
    /// There are no records contained in the `answer` section of a response.
    NoRecords,
    /// The records contained from the `answer` section of a response.
    Records(Vec<T>),
}

#[async_recursion]
pub(crate) async fn recursive_query<CCache>(client: Arc<DNSAsyncClient>, joined_cache: Arc<CCache>, context: Context) -> QueryResponse<ResourceRecord> where CCache: AsyncCache + Send + Sync + 'static {
    debug!(context:?; "Start recursive search");
    let cache_response: dns_lib::interface::cache::CacheResponse = client.cache.get(&CacheQuery {
        authoritative: false,
        question: context.query().clone(),
    }).await;
    // Initial Cache Check: Check to see if the records we're looking for are already cached.
    trace!(context:?; "Recursive search initial cache response: '{cache_response:?}'");
    match cache_response {
        CacheResponse::Records(records) if (records.len() == 0) => (),
        CacheResponse::Records(records) => return QueryResponse::Records(records.into_iter().map(|cache_record| cache_record.record).collect()),
        CacheResponse::Err(rcode) => return QueryResponse::Error(rcode),
    };

    // Discovery Stage: See if we have name servers that handle one of the parent domains of the
    // qname.
    let (search_names_max_index, mut name_servers) = match get_closest_name_server(&client, &joined_cache, context.query()).await {
        NSResponse::Error(error) => return QueryResponse::Error(error),
        NSResponse::Records(search_names_max_index, name_servers) => (search_names_max_index, name_servers),
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
        // We set the qtype to be RRTypeCode::A to hide the actual qtype
        // that we're looking for.
        name_servers.shuffle(&mut thread_rng());

        let search_query = Question::new(search_name.clone(), RType::A, context.qclass());
        let search_context = match context.clone().new_search_name(search_query) {
            Ok(search_context) => Arc::new(search_context),
            Err(error) => {
                debug!(context:?; "Recursive search new search error: '{error}'");
                return QueryResponse::Error(RCode::ServFail)
            },
        };
        trace!(context:?; "Recursive search querying name servers '{name_servers:?}' with search context '{search_context:?}'");

        match query_name_servers(&client, &joined_cache, search_context, &name_servers).await {
            QueryResponse::Error(error) => {
                trace!(context:?; "Recursive search querying name servers '{name_servers:?}' with search context response: error {error}");
                return QueryResponse::Error(error)
            },
            QueryResponse::NoRecords => {
                trace!(context:?; "Recursive search querying name servers '{name_servers:?}' with search context response: no records");
                return QueryResponse::Error(RCode::ServFail);
            },
            QueryResponse::Records(response_records) => {
                trace!(context:?; "Recursive search querying name servers '{name_servers:?}' with search context response: '{response_records:?}'");

                if (index != 0) || (context.qtype() != RType::DNAME) {
                    if response_records.iter().any(|record| record.get_rtype() == RType::DNAME) {
                        return handle_dname(client, joined_cache, context, response_records).await;
                    }
                }

                if response_records.iter().any(|record| record.get_rtype() == RType::NS) {
                    name_servers.clear();
                    for record in &response_records {
                        if let RecordData::NS(ns_rdata) = record.get_rdata() {
                            name_servers.push(ns_rdata.name_server_domain_name().clone())
                        }
                    }
                }
            },
        }
    }

    // Check for various cached answers.
    match query_cache(&joined_cache, context.query()).await {
        QueryResponse::Error(error) => {
            trace!(context:?; "Recursive search secondary cache response: error '{error}'");
            return QueryResponse::Error(error)
        },
        QueryResponse::NoRecords => {
            trace!(context:?; "Recursive search secondary cache response: no records");
        },
        QueryResponse::Records(response_records) => {
            trace!(context:?; "Recursive search secondary cache response: '{response_records:?}'");
            if (context.qtype() != RType::CNAME) && response_records.iter().any(|record| record.get_rtype() == RType::CNAME) {
                return handle_cname(client, joined_cache, context, response_records).await;
            }

            if (context.qtype() != RType::DNAME) && response_records.iter().any(|record| record.get_rtype() == RType::DNAME) {
                return handle_dname(client, joined_cache, context, response_records).await;
            }

            return QueryResponse::Records(response_records);
        },
    }

    // Query name servers for answers.
    trace!(context:?; "Recursive search: querying name servers '{name_servers:?}' with full context");
    match query_name_servers(&client, &joined_cache, context.clone(), &name_servers).await {
        QueryResponse::Error(error) => {
            trace!(context:?; "Recursive search name server response: error '{error}'");
            return QueryResponse::Error(error)
        },
        QueryResponse::NoRecords => {
            trace!(context:?; "Recursive search name server response: no records");
        },
        QueryResponse::Records(response_records) => {
            trace!(context:?; "Recursive search name server response: '{response_records:?}'");
            if (context.qtype() != RType::CNAME) && response_records.iter().any(|record| record.get_rtype() == RType::CNAME) {
                return handle_cname(client, joined_cache, context, response_records).await;
            }

            if (context.qtype() != RType::DNAME) && response_records.iter().any(|record| record.get_rtype() == RType::DNAME) {
                return handle_dname(client, joined_cache, context, response_records).await;
            }

            return QueryResponse::Records(response_records);
        },
    }

    trace!(context:?; "Recursive search no records found");
    return QueryResponse::NoRecords;
}

pub async fn query_cache<CCache>(joined_cache: &Arc<CCache>, question: &Question) -> QueryResponse<ResourceRecord> where CCache: AsyncCache {
    let response = joined_cache.get(&CacheQuery {
        authoritative: false,
        question: question.clone(),
    }).await;
    match response {
        CacheResponse::Records(records) if (records.len() == 0) => QueryResponse::NoRecords,
        CacheResponse::Records(records) => QueryResponse::Records(records.into_iter().map(|cache_record| cache_record.record).collect()),
        CacheResponse::Err(rcode) => QueryResponse::Error(rcode),
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum NSResponse<T> {
    Records(usize, Vec<T>),
    Error(RCode),
}

async fn get_closest_name_server<CCache>(_client: &Arc<DNSAsyncClient>, joined_cache: &Arc<CCache>, question: &Question) -> NSResponse<CDomainName> where CCache: AsyncCache {
    let mut name_servers = Vec::new();
    let mut max_index = 0;
    for (index, search_name) in question.qname().search_domains().enumerate() {
        max_index = index;
        match query_cache(joined_cache, &question.with_new_qname_qtype(search_name.clone(), RType::NS)).await {
            QueryResponse::Error(_) => return NSResponse::Error(RCode::ServFail),
            QueryResponse::NoRecords => continue,
            QueryResponse::Records(cached_name_servers) => {
                name_servers.reserve_exact(cached_name_servers.len());
                for record in cached_name_servers {
                    if let RecordData::NS(ns_rdata) = record.get_rdata() {
                        name_servers.push(ns_rdata.name_server_domain_name().clone())
                    }
                }
                break;
            },
        }
    }
    return NSResponse::Records(max_index, name_servers);
}

async fn handle_cname<CCache>(client: Arc<DNSAsyncClient>, joined_cache: Arc<CCache>, context: Arc<Context>, records: Vec<ResourceRecord>) -> QueryResponse<ResourceRecord> where CCache: AsyncCache + Send + Sync + 'static {
    debug!(context:?; "Recursive search redirected by cname");
    for record in &records {
        if let RecordData::CNAME(cname_rdata) = record.get_rdata() {
            match context.clone().new_cname(cname_rdata.primary_name().clone()) {
                Ok(cname_context) => {
                    match recursive_query(client, joined_cache, cname_context).await {
                        QueryResponse::Error(rcode) => return QueryResponse::Error(rcode),
                        QueryResponse::NoRecords => return QueryResponse::Records(records),
                        QueryResponse::Records(mut answer_records) => {
                            answer_records.extend(records);
                            return QueryResponse::Records(answer_records);
                        },
                    }
                },
                Err(error) => {
                    trace!(context:?; "Recursive search new cname error: {error}");
                    return QueryResponse::Error(RCode::ServFail);
                },
            };
        }
    }

    trace!(context:?; "Recursive search new cname error: no cname record in records '{records:?}'");
    return QueryResponse::Error(RCode::ServFail);
}

async fn handle_dname<CCache>(client: Arc<DNSAsyncClient>, joined_cache: Arc<CCache>, context: Arc<Context>, records: Vec<ResourceRecord>) -> QueryResponse<ResourceRecord> where CCache: AsyncCache + Send + Sync + 'static {
    debug!(context:?; "Recursive search redirected by dname");
    for record in &records {
        if let RecordData::DNAME(dname_rdata) = record.get_rdata() {
            if !context.qname().is_parent_domain_of(record.get_name()) {
                trace!(context:?; "Recursive search new dname error: The query name '{}' is not a subdomain of the dname's owner name '{}'", context.qname(), record.get_name());
                return QueryResponse::Error(RCode::ServFail);
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
                    return QueryResponse::Error(RCode::ServFail);
                },
            };

            match context.clone().new_dname(dname) {
                Ok(dname_context) => {
                    match recursive_query(client, joined_cache, dname_context).await {
                        QueryResponse::Error(rcode) => return QueryResponse::Error(rcode),
                        QueryResponse::NoRecords => return QueryResponse::Records(records),
                        QueryResponse::Records(mut answer_records) => {
                            answer_records.extend(records);
                            return QueryResponse::Records(answer_records);
                        },
                    }
                },
                Err(error) => {
                    trace!(context:?; "Recursive search new cname error: {error}");
                    return QueryResponse::Error(RCode::ServFail);
                },
            };
        }
    }

    trace!(context:?; "Recursive search new cname error: no dname record in records '{records:?}'");
    return QueryResponse::Error(RCode::ServFail);
}

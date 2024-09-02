use std::sync::Arc;

use async_recursion::async_recursion;
use dns_lib::{interface::{cache::{cache::AsyncCache, main_cache::AsyncMainCache, CacheQuery, CacheResponse}, client::Context}, query::question::Question, resource_record::{rcode::RCode, resource_record::ResourceRecord, rtype::RType}, types::c_domain_name::{CDomainName, Labels}};
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
    println!("Start: Recursive Search for '{}'", context.query());
    let cache_response: dns_lib::interface::cache::CacheResponse = client.cache.get(&CacheQuery {
        authoritative: false,
        question: context.query().clone(),
    }).await;
    // Initial Cache Check: Check to see if the records we're looking for are already cached.
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

        let search_context = match context.clone().new_search_name(Question::new(search_name.clone(), RType::A, context.qclass())) {
            Ok(search_context) => Arc::new(search_context),
            Err(error) => {
                println!("Search Lookup Error: {error}");
                return QueryResponse::Error(RCode::ServFail)
            },
        };

        match query_name_servers(&client, &joined_cache, search_context, &name_servers).await {
            QueryResponse::Error(error) => return QueryResponse::Error(error),
            QueryResponse::NoRecords => {
                println!("Failed to find records for '{search_name}' while trying to answer '{}'", context.query());
                return QueryResponse::Error(RCode::ServFail);
            },
            QueryResponse::Records(response_records) => {
                if response_records.iter().any(|record| record.rtype() == RType::NS) {
                    name_servers.clear();
                    for record in &response_records {
                        if let ResourceRecord::NS(_, ns_rdata) = record {
                            name_servers.push(ns_rdata.name_server_domain_name().clone())
                        }
                    }
                }
            },
        }
    }

    // Check for various cached answers.
    match query_cache(&joined_cache, context.query()).await {
        QueryResponse::Error(error) => return QueryResponse::Error(error),
        QueryResponse::NoRecords => (),
        QueryResponse::Records(response_records) => {
            for record in &response_records {
                if let ResourceRecord::CNAME(_, cname_rdata) = record {
                    match context.new_cname(cname_rdata.primary_name().clone()) {
                        Ok(cname_context) => return recursive_query(client, joined_cache, cname_context).await,
                        Err(error) => {
                            println!("CName Lookup Error: {error}");
                            return QueryResponse::Error(RCode::ServFail)
                        },
                    };
                }
            }

            // TODO: Add exception for DNAME, similar to CNAME

            return QueryResponse::Records(response_records);
        },
    }

    // Query name servers for answers.
    match query_name_servers(&client, &joined_cache, context.clone(), &name_servers).await {
        QueryResponse::Error(error) => return QueryResponse::Error(error),
        QueryResponse::NoRecords => (),
        QueryResponse::Records(response_records) => {
            for record in &response_records {
                if let ResourceRecord::CNAME(_, cname_rdata) = record {
                    match context.new_cname(cname_rdata.primary_name().clone()) {
                        Ok(cname_context) => return recursive_query(client, joined_cache, cname_context).await,
                        Err(error) => {
                            println!("CName Lookup Error: {error}");
                            return QueryResponse::Error(RCode::ServFail)
                        },
                    };
                }
            }

            // TODO: Add exception for DNAME, similar to CNAME

            return QueryResponse::Records(response_records);
        },
    }

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
                    if let ResourceRecord::NS(_, ns_rdata) = record {
                        name_servers.push(ns_rdata.name_server_domain_name().clone())
                    }
                }
                break;
            },
        }
    }
    return NSResponse::Records(max_index, name_servers);
}

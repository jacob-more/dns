use std::{future::Future, net::IpAddr, sync::Arc};

use async_recursion::async_recursion;
use dns_lib::{query::{question::Question, message::Message}, resource_record::{resource_record::ResourceRecord, rcode::RCode, rtype::RType}, interface::cache::{cache::AsyncCache, main_cache::AsyncMainCache}, types::c_domain_name::CDomainName};
use tokio::{sync::broadcast::{self, Sender}, join};

use crate::{DNSAsyncClient, query::network_query::query_network};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) enum QueryResponse<T> {
    Error(RCode),
    /// There are no records contained in the `answer` section of a response.
    NoRecords,
    /// The records contained from the `answer` section of a response.
    Records(Vec<T>),
}

#[async_recursion]
pub(crate) async fn recursive_query<CCache>(client: Arc<DNSAsyncClient>, joined_cache: Arc<CCache>, question: &Question) -> QueryResponse<ResourceRecord> where CCache: AsyncCache + Send + Sync {
    println!("Start: Recursive Search for '{question}'");
    let cache_response = client.cache.get(&Message::from(question)).await;
    // Initial Cache Check: Check to see if the records we're looking for are already cached.
    match (cache_response.rcode_flag(), cache_response.answer().len()) {
        (RCode::NoError, 0) => (),
        (RCode::NoError, 1..) => return QueryResponse::Records(cache_response.answer),
        (RCode::NXDomain, _) => return QueryResponse::Error(RCode::NXDomain),
        (_, _) => (),
    }

    // Discovery Stage: See if we have name servers that handle one of the parent domains of the
    // qname.
    let (search_names_max_index, mut name_servers) = match get_closest_name_server(&client, &joined_cache, question).await {
        NSResponse::Error(error) => return QueryResponse::Error(error),
        NSResponse::Records(search_names_max_index, name_servers) => (search_names_max_index, name_servers),
    };
    // Bound the search names based on the max index we reached to make the next stage easier.
    // This will make sure we start the search with the child of the ancestor and continue
    // down the tree from there.
    let search_names = question.qname().search_domains().take(search_names_max_index);

    // Query Stage: Query name servers for the next subdomain, following the tree to our answer.
    for (index, search_name) in search_names.enumerate().rev() {
        // Query the name servers for the child domain (aka. search_name).
        // We set the qtype to be RRTypeCode::A to hide the actual qtype
        // that we're looking for.
        let used_qtype = if index == 0 { question.qtype() } else { RType::A };
        match query_name_servers(&client, &joined_cache, &question.with_new_qname_qtype(search_name.clone(), used_qtype), &name_servers).await {
            QueryResponse::Error(error) => return QueryResponse::Error(error),
            QueryResponse::NoRecords => {
                println!("Failed to find records for '{search_name}' while trying to answer '{question}'");
                return QueryResponse::Error(RCode::ServFail);
            },
            QueryResponse::Records(response_records) => {
                // If we are at index 0, then we have reached the original qname.
                // We want to see if this is our answer, a CNAME, or a DNAME
                if index == 0 && response_records.iter().any(|record| record.rtype() == question.qtype()) {
                    return QueryResponse::Records(response_records);
                }

                if index == 0 {
                    for record in &response_records {
                        if let ResourceRecord::CNAME(_, cname_rdata) = record {
                            return recursive_query(client, joined_cache, &question.with_new_qname(cname_rdata.primary_name().clone())).await;
                        }
                    }
                }

                // TODO: Handle DNAME; similar to CNAME

                if response_records.iter().any(|record| record.rtype() == RType::NS) {
                    name_servers.clear();
                    for record in response_records {
                        if let ResourceRecord::NS(_, ns_rdata) = record {
                            name_servers.push(ns_rdata.name_server_domain_name().clone())
                        }
                    }
                }
            },
        }
    }

    // Check for various cached answers.
    match query_cache(&joined_cache, question).await {
        QueryResponse::Error(error) => return QueryResponse::Error(error),
        QueryResponse::NoRecords => (),
        QueryResponse::Records(response_records) => {
            for record in &response_records {
                if let ResourceRecord::CNAME(_, cname_rdata) = record {
                    return recursive_query(client, joined_cache, &question.with_new_qname(cname_rdata.primary_name().clone())).await;
                }
            }

            // TODO: Add exception for DNAME, similar to CNAME

            return QueryResponse::Records(response_records);
        },
    }

    // Query name servers for answers.
    match query_name_servers(&client, &joined_cache, question, &name_servers).await {
        QueryResponse::Error(error) => return QueryResponse::Error(error),
        QueryResponse::NoRecords => (),
        QueryResponse::Records(response_records) => {
            for record in &response_records {
                if let ResourceRecord::CNAME(_, cname_rdata) = record {
                    return recursive_query(client, joined_cache, &question.with_new_qname(cname_rdata.primary_name().clone())).await;
                }
            }

            // TODO: Add exception for DNAME, similar to CNAME

            return QueryResponse::Records(response_records);
        },
    }

    return QueryResponse::NoRecords;
}

async fn query_cache<CCache>(joined_cache: &Arc<CCache>, question: &Question) -> QueryResponse<ResourceRecord> where CCache: AsyncCache {
    let response = joined_cache.get(&Message::from(question)).await;
    match (response.rcode_flag(), response.answer().len()) {
        (RCode::NoError, 0) => QueryResponse::NoRecords,
        (RCode::NoError, 1..) => QueryResponse::Records(response.answer),
        (error, _) => QueryResponse::Error(error.clone())
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

#[async_recursion]
async fn query_name_servers<CCache>(client: &Arc<DNSAsyncClient>, joined_cache: &Arc<CCache>, question: &Question, name_servers: &[CDomainName]) -> QueryResponse<ResourceRecord> where CCache: AsyncCache + Send + Sync {
    // Active Query Check: Check to see if a query has already been made for the record.
    let read_locked_active_queries = client.active_query_manager.read().await;
    match read_locked_active_queries.get(question) {
        Some(sender) => {
            println!("Already Forwarded (1): '{question}'");
            let mut receiver = sender.subscribe();
            drop(read_locked_active_queries);
            match receiver.recv().await {
                Ok(QueryResponse::Error(error)) => println!("Recoverable Internal Error: {error}\nContinuing Query,"),
                Ok(QueryResponse::NoRecords) => (),
                Ok(QueryResponse::Records(response_records)) => return QueryResponse::Records(response_records),
                // In the event of an error passing messages between tasks, we can continue this query.
                Err(error) => println!("Recoverable Internal Error: {error}\nContinuing Query,"),
            }
        },
        None => drop(read_locked_active_queries),
    }

    let sender;
    let mut write_locked_active_queries = client.active_query_manager.write().await;
    // Since we switched our locks, we should check again that the query has not already been started.
    match write_locked_active_queries.get(question) {
        Some(sender) => {
            println!("Already Forwarded (2): '{question}'");
            let mut receiver = sender.subscribe();
            drop(write_locked_active_queries);
            match receiver.recv().await {
                Ok(QueryResponse::Error(error)) => {
                    println!("Recoverable Internal Error: {error}\nRestarting Name Servers Query,");
                    return query_name_servers(client, joined_cache, question, name_servers).await;
                },
                Ok(QueryResponse::NoRecords) => return QueryResponse::NoRecords,
                Ok(QueryResponse::Records(response_records)) => return QueryResponse::Records(response_records),
                // In the event of an error passing messages between tasks, we can continue this query.
                Err(error) => {
                    println!("Recoverable Internal Error: {error}\nRestarting Name Servers Query,");
                    return query_name_servers(client, joined_cache, question, name_servers).await;
                },
            }
        },
        None => {
            (sender, _) = broadcast::channel(1);
            // IMPORTANT: Any return statements after this should use `sender_return()` so that the
            // active queries map gets cleaned up and anyone waiting for an answer gets sent the
            // result.
            write_locked_active_queries.insert(question.clone(), sender.clone());
            drop(write_locked_active_queries);
        }
    }

    let mut cached_ns = Vec::new();
    let mut non_cached_ns = Vec::new();
    for name_server in name_servers {
        // TODO: this loop can probably be parallelized
        match get_cached_ns_addresses(client, joined_cache, question, name_server).await {
            Some(records) => cached_ns.push(async { QueryResponse::Records(records) }),
            None => non_cached_ns.push(get_network_ns_addresses(&client, &joined_cache, question, name_server)),
        }
    }

    // Query name servers.

    // #### FIRST USING CACHE ONLY ####

    // This is done to minimize the queries to external sources. This can be useful, for example if
    // the first name server in the list is not cached but the second name server has a cached IP
    // address. In that case, it would be better to query the cached IP. If that fails, we will fall
    // back on the other name servers. But we'll first need to query the network for its IP address,
    // which may take time.

    println!("Querying Cached Name Servers for '{question}'");
    if let QueryResponse::Records(records) = query_network_round_robin(client, joined_cache, &question, cached_ns).await {
        return sender_return::<CCache>(client, &sender, QueryResponse::Records(records), question).await;
    }

    // #### SECOND USING REGULAR QUERY (cache + network) ####

    println!("Querying Non-Cached Name Servers for '{question}'");

    let result = query_network_round_robin(client, joined_cache, &question, non_cached_ns).await;
    return sender_return::<CCache>(client, &sender, result, question).await;
}

async fn get_cached_ns_addresses<CCache>(_client: &Arc<DNSAsyncClient>, joined_cache: &Arc<CCache>, question: &Question, name_server: &CDomainName) -> Option<Vec<ResourceRecord>> where CCache: AsyncCache + Send + Sync {
    let a_question = question.with_new_qname_qtype(name_server.clone(), RType::A);
    let aaaa_question = question.with_new_qname_qtype(name_server.clone(), RType::AAAA);

    let a_search = query_cache(joined_cache, &a_question);
    let aaaa_search = query_cache(joined_cache, &aaaa_question);

    match join!(a_search, aaaa_search) {
        (QueryResponse::Records(mut a_records), QueryResponse::Records(aaaa_records)) => {
            a_records.extend(aaaa_records);
            Some(a_records)
        },
        (QueryResponse::Records(a_records), _) => Some(a_records),
        (_, QueryResponse::Records(aaaa_records)) => Some(aaaa_records),
        (_, _) => None,
    }
}

async fn get_network_ns_addresses<CCache>(client: &Arc<DNSAsyncClient>, joined_cache: &Arc<CCache>, question: &Question, name_server: &CDomainName) -> QueryResponse<ResourceRecord> where CCache: AsyncCache + Send + Sync {
    let a_question = question.with_new_qname_qtype(name_server.clone(), RType::A);
    let aaaa_question = question.with_new_qname_qtype(name_server.clone(), RType::AAAA);

    let a_search = recursive_query(client.clone(), joined_cache.clone(), &a_question);
    let aaaa_search = recursive_query(client.clone(), joined_cache.clone(), &aaaa_question);

    match join!(a_search, aaaa_search) {
        (QueryResponse::Records(mut a_records), QueryResponse::Records(aaaa_records)) => {
            a_records.extend(aaaa_records);
            QueryResponse::Records(a_records)
        },
        (QueryResponse::Records(a_records), _) => QueryResponse::Records(a_records),
        (_, QueryResponse::Records(aaaa_records)) => QueryResponse::Records(aaaa_records),
        (QueryResponse::NoRecords, QueryResponse::NoRecords) => QueryResponse::NoRecords,
        (QueryResponse::NoRecords, QueryResponse::Error(rcode)) => QueryResponse::Error(rcode),
        (QueryResponse::Error(rcode), QueryResponse::NoRecords) => QueryResponse::Error(rcode),
        (QueryResponse::Error(rcode1), QueryResponse::Error(_rcode2)) => QueryResponse::Error(rcode1),
    }
}

async fn query_network_round_robin<CCache>(client: &Arc<DNSAsyncClient>, joined_cache: &Arc<CCache>, question: &Question, name_server_addresses: Vec<impl Future<Output = QueryResponse<ResourceRecord>>>) -> QueryResponse<ResourceRecord> where CCache: AsyncCache + Send + Sync {
    println!("Querying Name Servers Round Robin for '{question}'");
    let mut last_error = RCode::ServFail
    ;

    for name_server_address_query in name_server_addresses {
        match name_server_address_query.await {
            QueryResponse::Error(rcode) => last_error = rcode,
            QueryResponse::NoRecords => (),
            QueryResponse::Records(records) => {
                for ns_address_record in records {
                    let ns_address = match ns_address_record {
                        ResourceRecord::A(_, a_rdata) => IpAddr::V4(*a_rdata.ipv4_addr()),
                        ResourceRecord::AAAA(_, aaaa_rdata) => IpAddr::V6(*aaaa_rdata.ipv6_addr()),
                        ResourceRecord::A6(_, _a6_rdata) => todo!("Add support for querying based on an A6 record."),
                        _ => {
                            println!("'{}' cannot be used as a name server address", ns_address_record.rtype());
                            continue;
                        },
                    };
                    let mut message = match query_network(client, joined_cache.clone(), question, &ns_address).await {
                        Ok(message) => message,
                        Err(_io_error) => {
                            last_error = RCode::ServFail;
                            continue;
                        },
                    };
                    match (message.rcode, message.answer.len() + message.authority.len() + message.additional.len()) {
                        (RCode::NoError, 0) => return QueryResponse::NoRecords,
                        (RCode::NoError, 1..) => return QueryResponse::Records({
                            message.answer.extend(message.authority);
                            message.answer
                        }),
                        (error, _) => {
                            last_error = error;
                            continue;
                        },
                    }
                }
            },
        }
    }

    return QueryResponse::Error(last_error);
}

// Note: Returns the result so that this function can be called as the return statement.
async fn sender_return<CCache>(client: &Arc<DNSAsyncClient>, sender: &Sender<QueryResponse<ResourceRecord>>, result: QueryResponse<ResourceRecord>, question: &Question) -> QueryResponse<ResourceRecord> where CCache: AsyncCache {
    // Cleanup.
    let mut write_locked_active_query_manager = client.active_query_manager.write().await;
    write_locked_active_query_manager.remove(question);
    drop(write_locked_active_query_manager);

    // Send out the answer to anyone waiting.
    if sender.receiver_count() != 0 {
        match sender.send(result.clone()) {
            Ok(_) => (),
            Err(error) => println!("Internal Send Error for '{question}': {error}"),
        }
    }

    // Return the result.
    return result;
}

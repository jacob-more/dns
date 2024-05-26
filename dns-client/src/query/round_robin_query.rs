use std::{net::IpAddr, sync::{atomic::{AtomicUsize, Ordering}, Arc}};

use dns_lib::{interface::cache::cache::AsyncCache, query::{message::Message, qr::QR, question::Question}, resource_record::{rcode::RCode, resource_record::ResourceRecord, rtype::RType}, types::c_domain_name::CDomainName};
use tokio::{join, sync::broadcast};

use crate::DNSAsyncClient;

use super::{network_query::query_network, recursive_query::{query_cache, recursive_query, QueryResponse}};


struct NSAddressMap<'b, 'c> {
    ns_domain: &'b CDomainName,
    index: AtomicUsize,
    addresses: &'c [IpAddr],
}

impl<'b, 'c> NSAddressMap<'b, 'c> {
    fn new(ns_domain: &'b CDomainName, addresses: &'c [IpAddr]) -> Self {
        Self {
            ns_domain,
            index: AtomicUsize::new(0),
            addresses,
        }
    }

    fn next(&self) -> Option<&'c IpAddr> {
        self.addresses.get(self.index.fetch_add(1, Ordering::SeqCst))
    }
}

struct RoundRobinIter<'a, 'b, 'c> {
    index: usize,
    some_found: bool,
    mappings: &'a [NSAddressMap<'b, 'c>],
}

impl<'a, 'b, 'c> RoundRobinIter<'a, 'b, 'c> {
    fn new(ns_address_maps: &'a [NSAddressMap<'b, 'c>]) -> Self {
        Self {
            index: 0,
            some_found: false,
            mappings: ns_address_maps,
        }
    }

    fn next(&mut self) -> Option<(&'b CDomainName, &'c IpAddr)> {
        let mut mappings_iter = self.mappings.iter().enumerate().skip(self.index);
        while let Some((index, ns_address_map)) = mappings_iter.next() {
            match ns_address_map.next() {
                Some(address) => {
                    // Next time `next` is called, start at the next index.
                    self.index = index + 1;
                    self.some_found = true;
                    return Some((ns_address_map.ns_domain, address));
                },
                // End of the list has been reached.
                None => continue,
            }
        }

        if self.some_found {
            // Start over.
            self.some_found = false;
            self.index = 0;
            return self.next();
        } else {
            // All internal iterators are drained. No more addresses.
            return None;
        }
    }
}

fn query_response(answer: Message) -> QueryResponse<ResourceRecord> {
    match answer {
        Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NoError, question: _, answer, authority, additional: _ } if answer.is_empty() && authority.is_empty() => QueryResponse::NoRecords,
        Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NoError, question: _, mut answer, authority, additional: _} => {
            answer.extend(authority);
            QueryResponse::Records(answer)
        },
        Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode, question: _, answer: _, authority: _, additional: _ } => QueryResponse::Error(rcode),
        Message { id: _, qr: _, opcode: _, authoritative_answer: _, truncation: _, recursion_desired: _, recursion_available: _, z: _, rcode: _, question: _, answer: _, authority: _, additional: _ } => QueryResponse::Error(RCode::FormErr),
    }
}

async fn get_cached_name_server_addresses<CCache>(_client: &Arc<DNSAsyncClient>, joined_cache: &Arc<CCache>, question: &Question, name_server: &CDomainName) -> Option<Vec<ResourceRecord>> where CCache: AsyncCache + Send + Sync {
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

async fn get_name_server_addresses<CCache>(client: &Arc<DNSAsyncClient>, joined_cache: &Arc<CCache>, question: &Question, name_server: &CDomainName) -> Option<Vec<ResourceRecord>> where CCache: AsyncCache + Send + Sync {
    let a_question = question.with_new_qname_qtype(name_server.clone(), RType::A);
    let aaaa_question = question.with_new_qname_qtype(name_server.clone(), RType::AAAA);

    let a_search = recursive_query(client.clone(), joined_cache.clone(), &a_question);
    let aaaa_search = recursive_query(client.clone(), joined_cache.clone(), &aaaa_question);

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

fn rr_to_ip_address(record: &ResourceRecord) -> Option<IpAddr> {
    match &record {
        ResourceRecord::A(_, a_record) => Some(IpAddr::V4(*a_record.ipv4_addr())),
        ResourceRecord::AAAA(_, aaaa_record) => Some(IpAddr::V6(*aaaa_record.ipv6_addr())),
        _ => None,
    }
}

pub async fn query_name_servers<CCache>(client: &Arc<DNSAsyncClient>, joined_cache: &Arc<CCache>, question: &Question, name_servers: &[CDomainName]) -> QueryResponse<ResourceRecord> where CCache: AsyncCache + Send + Sync {
    // Active Query Check: Check to see if a query has already been made for the record.
    let read_locked_active_queries = client.active_query_manager.read().await;
    match read_locked_active_queries.get(question) {
        Some(sender) => {
            println!("Already Forwarded (1): '{question}'");
            let mut receiver = sender.subscribe();
            drop(read_locked_active_queries);
            match receiver.recv().await {
                Ok(response) => return response,
                // In the event of an error passing messages between tasks, we can continue this query.
                Err(error) => println!("Recoverable Internal Error: {error}\nContinuing Query,"),
            }
        },
        None => drop(read_locked_active_queries),
    }

    let mut write_locked_active_queries = client.active_query_manager.write().await;
    // Since we switched our locks, we should check again that the query has not already been started.
    match write_locked_active_queries.get(question) {
        Some(sender) => {
            println!("Already Forwarded (2): '{question}'");
            let mut receiver = sender.subscribe();
            drop(write_locked_active_queries);
            match receiver.recv().await {
                Ok(response) => return response,
                // In the event of an error passing messages between tasks, we can continue this query.
                Err(error) => println!("Recoverable Internal Error: {error}\nRestarting Name Servers Query,"),
            }
        },
        None => {
            let (sender, _) = broadcast::channel(1);
            // IMPORTANT: Any return statements after this should use `sender_return()` so that the
            // active queries map gets cleaned up and anyone waiting for an answer gets sent the
            // result.
            write_locked_active_queries.insert(question.clone(), sender);
            drop(write_locked_active_queries);
        }
    }

    // Choose an order.
    //      1st, name servers who we already know addresses of (cached).
    //         we may want to experiment with selecting servers that have not been connected to yet
    //         to get an idea of how fast those name servers are.
    //      2nd, name servers who we don't know addresses of (need to be queried for).

    // TODO: still need to order based on which sockets exist and are connected.

    let mut cached_name_server_address_vec = Vec::with_capacity(name_servers.len());
    let mut non_cached_name_servers = Vec::with_capacity(name_servers.len());
    for name_server in name_servers {
        match get_cached_name_server_addresses(client, joined_cache, question, name_server).await {
            Some(address_records) => cached_name_server_address_vec.push((
                name_server,
                address_records.iter()
                    .filter_map(|record| rr_to_ip_address(record))
                    .collect::<Vec<IpAddr>>()
            )),
            None => non_cached_name_servers.push(name_server),
        }
    }

    println!("Querying Cached Name Servers for '{question}'");

    let cached_name_server_address_mappings = cached_name_server_address_vec.iter()
        .map(|(ns_domain, addresses)| NSAddressMap::new(ns_domain, addresses))
        .collect::<Vec<NSAddressMap>>();
    let mut cached_name_server_address_round_robin = RoundRobinIter::new(&cached_name_server_address_mappings);

    while let Some((ns_domain, address)) = cached_name_server_address_round_robin.next() {
        println!("Querying Name Server '{ns_domain}' for '{question}'");
        match query_network(client, joined_cache.clone(), question, &address).await {
            Ok(response) => return sender_return::<CCache>(client, query_response(response), question).await,
            Err(_) => continue,
        }
    }

    println!("Querying Non-Cached Name Servers for '{question}'");

    let mut non_cached_name_server_address_vec = Vec::with_capacity(non_cached_name_servers.len());
    for ns_domain in non_cached_name_servers {
        if let Some(address_records) = get_name_server_addresses(client, joined_cache, question, ns_domain).await {
            let mut addresses = address_records.iter()
                .filter_map(|record| rr_to_ip_address(record));

            println!("Querying Name Server '{ns_domain}' for '{question}'");
            if let Some(address) = addresses.next() {
                match query_network(client, joined_cache.clone(), question, &address).await {
                    Ok(response) => return sender_return::<CCache>(client, query_response(response), question).await,
                    Err(_) => (),
                }
            } else {
                // If this name server has no addresses, then we don't need to try to query it
                // again. We won't even bother adding the empty vector to the round-robin iterator.
                continue;
            }

            non_cached_name_server_address_vec.push((
                ns_domain,
                addresses.collect::<Vec<IpAddr>>()
            ));
        }
    }

    let non_cached_name_server_address_mappings = non_cached_name_server_address_vec.iter()
        .map(|(ns_domain, addresses)| NSAddressMap::new(ns_domain, addresses))
        .collect::<Vec<NSAddressMap>>();
    let mut non_cached_name_server_address_round_robin = RoundRobinIter::new(&non_cached_name_server_address_mappings);

    while let Some((ns_domain, address)) = non_cached_name_server_address_round_robin.next() {
        println!("Querying Name Server '{ns_domain}' for '{question}'");
        match query_network(client, joined_cache.clone(), question, &address).await {
            Ok(response) => return sender_return::<CCache>(client, query_response(response), question).await,
            Err(_) => continue,
        }
    }

    return sender_return::<CCache>(client, QueryResponse::Error(RCode::ServFail), question).await;
}

// Note: Returns the result so that this function can be called as the return statement.
async fn sender_return<CCache>(client: &Arc<DNSAsyncClient>, result: QueryResponse<ResourceRecord>, question: &Question) -> QueryResponse<ResourceRecord> where CCache: AsyncCache {
    // Cleanup.
    let mut write_locked_active_query_manager = client.active_query_manager.write().await;
    let sender = write_locked_active_query_manager.remove(question);
    drop(write_locked_active_query_manager);

    // Send out the answer to anyone waiting.
    if let Some(sender) = sender {
        let _ = sender.send(result.clone());
    }

    // Return the result.
    return result;
}

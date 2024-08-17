use std::{borrow::BorrowMut, future::Future, io, net::IpAddr, pin::Pin, sync::{atomic::{AtomicUsize, Ordering}, Arc}, task::Poll, time::Duration};

use async_lib::awake_token::AwakeToken;
use dns_lib::{interface::cache::cache::AsyncCache, query::{message::Message, qr::QR, question::Question}, resource_record::{rcode::RCode, resource_record::ResourceRecord, rtype::RType}, types::c_domain_name::CDomainName};
use futures::{stream::FuturesUnordered, StreamExt};
use tokio::{join, select, sync::broadcast};

use crate::DNSAsyncClient;

use super::{network_query::query_network, recursive_query::{query_cache, recursive_query, QueryResponse}};


enum NSQueryAddressState {
    Fresh,
    CacheHit(Vec<IpAddr>),
    CacheMiss,
    QueryingNetwork(Pin<Box<dyn Future<Output = QueryResponse<ResourceRecord>> + Send>>),
    QuerySuccess(Vec<IpAddr>),
    QueryFailed(RCode),
}

enum NSQueryResult {
    OutOfAddresses,
    NSAddressQueryErr(RCode),
    QueryResult(io::Result<Message>),
}

struct NSQuery<CCache> where CCache: AsyncCache + Send + Sync {
    ns_domain: CDomainName,
    question: Question,

    ns_address_rtype: RType,
    ns_addresses: NSQueryAddressState,
    query: Option<Pin<Box<dyn Future<Output = io::Result<Message>> + Send>>>,
    kill_token: Option<Arc<AwakeToken>>,

    client: Arc<DNSAsyncClient>,
    joined_cache: Arc<CCache>,
}

impl<CCache> NSQuery<CCache> where CCache: AsyncCache + Send + Sync + 'static {
    pub fn new(ns_domain: CDomainName, address_rtype: RType, question: Question, kill_token: Option<Arc<AwakeToken>>, client: Arc<DNSAsyncClient>, joined_cache: Arc<CCache>) -> Self {
        Self {
            ns_domain,
            question,
            ns_address_rtype: address_rtype,
            ns_addresses: NSQueryAddressState::Fresh,
            query: None,
            kill_token,
            client,
            joined_cache,
        }
    }

    pub async fn query_cache_for_ns_addresses(mut self: Pin<&mut Self>) {
        let question = Question::new(self.ns_domain.clone(), self.ns_address_rtype, self.question.qclass());

        fn rr_to_ip(record: ResourceRecord) -> Option<IpAddr> {
            match record {
                ResourceRecord::A(_, rdata) => Some(IpAddr::V4(*rdata.ipv4_addr())),
                ResourceRecord::AAAA(_, rdata) => Some(IpAddr::V6(*rdata.ipv6_addr())),
                _ => None,
            }
        }

        match query_cache(&self.joined_cache, &question).await {
            QueryResponse::Records(mut records) => self.ns_addresses = NSQueryAddressState::CacheHit(
                records.drain(..).filter_map(|record| rr_to_ip(record)).collect()
            ),
            _ => self.ns_addresses = NSQueryAddressState::CacheMiss,
        }
    }

    pub fn had_cache_hit(&self) -> bool {
        match &self.ns_addresses {
            NSQueryAddressState::CacheHit(_) => true,
            _ => false,
        }
    }

    pub fn is_querying(&self) -> bool {
        self.query.is_some()
    }
}

impl<CCache> Future for NSQuery<CCache> where CCache: AsyncCache + Send + Sync + 'static {
    type Output = NSQueryResult;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        async fn recursive_query_owned_args<CCache>(client: Arc<DNSAsyncClient>, joined_cache: Arc<CCache>, question: Question) -> QueryResponse<ResourceRecord> where CCache: AsyncCache + Send + Sync + 'static {
            recursive_query(client, joined_cache, &question).await
        }

        if let NSQueryAddressState::CacheMiss = self.ns_addresses {
            let client = self.client.clone();
            let cache = self.joined_cache.clone();
            let question = self.question.clone();
            self.ns_addresses = NSQueryAddressState::QueryingNetwork(Box::pin(recursive_query_owned_args(client, cache, question)));
        }

        fn rr_to_ip(record: ResourceRecord) -> Option<IpAddr> {
            match record {
                ResourceRecord::A(_, rdata) => Some(IpAddr::V4(*rdata.ipv4_addr())),
                ResourceRecord::AAAA(_, rdata) => Some(IpAddr::V6(*rdata.ipv6_addr())),
                _ => None,
            }
        }

        if let NSQueryAddressState::QueryingNetwork(ns_addresses) = &mut self.ns_addresses {
            match ns_addresses.as_mut().poll(cx) {
                Poll::Ready(QueryResponse::Records(mut records)) => self.ns_addresses = NSQueryAddressState::QuerySuccess(
                    records.drain(..).filter_map(|record| rr_to_ip(record)).collect()
                ),
                Poll::Ready(QueryResponse::NoRecords) => self.ns_addresses = NSQueryAddressState::QuerySuccess(vec![]),
                Poll::Ready(QueryResponse::Error(rcode)) => self.ns_addresses = NSQueryAddressState::QueryFailed(rcode),
                Poll::Pending => (),
            }
        }

        match &self.ns_addresses {
            NSQueryAddressState::QueryFailed(rcode) => {
                return Poll::Ready(NSQueryResult::NSAddressQueryErr(*rcode));
            },
            NSQueryAddressState::CacheHit(addresses) | NSQueryAddressState::QuerySuccess(addresses) => {
                if let (None, None) = (&self.query, addresses.last()) {
                    return Poll::Ready(NSQueryResult::OutOfAddresses);
                }
            },
            _ => (),
        }

        async fn query_network_owned_args<CCache>(client: Arc<DNSAsyncClient>, joined_cache: Arc<CCache>, question: Question, name_server_address: IpAddr, kill_token: Option<Arc<AwakeToken>>) -> io::Result<Message> where CCache: AsyncCache + Send + Sync {
            query_network(&client, joined_cache, &question, &name_server_address, kill_token).await
        }

        if self.query.is_none() {
            if let NSQueryAddressState::CacheHit(addresses) | NSQueryAddressState::QuerySuccess(addresses) = self.ns_addresses.borrow_mut() {
                if let Some(name_server_address) = addresses.pop() {
                    let client = self.client.clone();
                    let cache = self.joined_cache.clone();
                    let question = self.question.clone();
                    let kill_token = self.kill_token.clone();
                    self.query = Some(Box::pin(query_network_owned_args(client, cache, question, name_server_address, kill_token)));
                }
            }
        }

        // Polls existing query to move it forward if possible.
        // Or, polls the new query immediately to get it started after it was created.
        if let Some(query) = self.query.as_mut() {
            match query.as_mut().poll(cx) {
                Poll::Ready(result) => {
                    self.query = None;
                    return Poll::Ready(NSQueryResult::QueryResult(result))
                },
                Poll::Pending => (),
            }
        }

        return Poll::Pending;
    }
}

struct NSAddressMap<'b, 'c> {
    ns_domain: &'b CDomainName,
    index: AtomicUsize,
    addresses: &'c [IpAddr],
}

impl<'b, 'c> NSAddressMap<'b, 'c> {
    #[inline]
    fn new(ns_domain: &'b CDomainName, addresses: &'c [IpAddr]) -> Self {
        Self {
            ns_domain,
            index: AtomicUsize::new(0),
            addresses,
        }
    }

    #[inline]
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
    #[inline]
    fn new(ns_address_maps: &'a [NSAddressMap<'b, 'c>]) -> Self {
        Self {
            index: 0,
            some_found: false,
            mappings: ns_address_maps,
        }
    }

    #[inline]
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

#[inline]
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

#[inline]
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

// #[inline]
// async fn get_name_server_addresses<CCache>(client: &Arc<DNSAsyncClient>, joined_cache: &Arc<CCache>, question: &Question, name_server: &CDomainName) -> Option<Vec<ResourceRecord>> where CCache: AsyncCache + Send + Sync {
//     let a_question = question.with_new_qname_qtype(name_server.clone(), RType::A);
//     let aaaa_question = question.with_new_qname_qtype(name_server.clone(), RType::AAAA);

//     let a_search = recursive_query(client.clone(), joined_cache.clone(), &a_question);
//     let aaaa_search = recursive_query(client.clone(), joined_cache.clone(), &aaaa_question);

//     match join!(a_search, aaaa_search) {
//         (QueryResponse::Records(mut a_records), QueryResponse::Records(aaaa_records)) => {
//             a_records.extend(aaaa_records);
//             Some(a_records)
//         },
//         (QueryResponse::Records(a_records), _) => Some(a_records),
//         (_, QueryResponse::Records(aaaa_records)) => Some(aaaa_records),
//         (_, _) => None,
//     }
// }

#[inline]
fn rr_to_ip_address(record: &ResourceRecord) -> Option<IpAddr> {
    match &record {
        ResourceRecord::A(_, a_record) => Some(IpAddr::V4(*a_record.ipv4_addr())),
        ResourceRecord::AAAA(_, aaaa_record) => Some(IpAddr::V6(*aaaa_record.ipv6_addr())),
        _ => None,
    }
}

#[inline]
pub async fn query_name_servers<CCache>(client: &Arc<DNSAsyncClient>, joined_cache: &Arc<CCache>, question: &Question, name_servers: &[CDomainName]) -> QueryResponse<ResourceRecord> where CCache: AsyncCache + Send + Sync + 'static {
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

    // TODO: Create special-purpose collection to keep pinned values. This way, we don't need to
    //       use Box.
    let kill_token = Arc::new(AwakeToken::new());
    let mut name_server_queries = name_servers.iter()
        .flat_map(|ns_domain| [
            Box::pin(NSQuery::new(ns_domain.clone(), RType::A, question.clone(), Some(kill_token.clone()), client.clone(), joined_cache.clone())),
            Box::pin(NSQuery::new(ns_domain.clone(), RType::AAAA, question.clone(), Some(kill_token.clone()), client.clone(), joined_cache.clone())),
        ])
        .collect::<Vec<_>>();
    futures::stream::iter(name_server_queries.iter_mut())
        .for_each_concurrent(None, |ns_query| ns_query.as_mut().query_cache_for_ns_addresses())
        .await;

    let mut cached_queries = Vec::new();
    let mut non_cached_queries = Vec::new();
    for ns_query in name_server_queries {
        if ns_query.had_cache_hit() {
            cached_queries.push(ns_query);
        } else {
            non_cached_queries.push(ns_query)
        }
    }

    println!("Querying Cached Name Servers for '{question}'");

    let mut active_queries = FuturesUnordered::new();
    for ns_query in cached_queries.iter_mut() {
        active_queries.push(ns_query);
        match active_queries.len() {
            0..=2 => select! {
                biased;
                result = active_queries.select_next_some() => match result {
                    // No error. Valid response.
                    NSQueryResult::QueryResult(Ok(response @ Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NoError, question: _, answer: _, authority: _, additional: _ })) => return sender_return::<CCache>(client, query_response(response), question, Some(kill_token)).await,
                    // If a name server cannot interpret what we are sending it, asking other name servers probably will not help.
                    // Treat as a hard error.
                    NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::FormErr, question: _, answer: _, authority: _, additional: _ })) => return sender_return::<CCache>(client, QueryResponse::Error(RCode::ServFail), question, Some(kill_token)).await,
                    // Only authoritative servers can indicate that a name does not exist.
                    NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: true, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NXDomain, question: _, answer: _, authority: _, additional: _ })) => return sender_return::<CCache>(client, QueryResponse::Error(RCode::NXDomain), question, Some(kill_token)).await,
                    NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: false, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NXDomain, question: _, answer: _, authority: _, additional: _ })) => (),
                    // If a server does not support a query type, we can probably assume it is not in that zone.
                    // TODO: verify that this is a valid assumption. Should we return NotImpl?
                    NSQueryResult::QueryResult(Ok(response @ Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NotImp, question: _, answer: _, authority: _, additional: _ })) => return sender_return::<CCache>(client, query_response(response), question, Some(kill_token)).await,
                    // If a name server refuses to perform an operation, we should not keep asking the other servers.
                    // TODO: verify that this is a valid way of handling.
                    NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::Refused, question: _, answer: _, authority: _, additional: _ })) => return sender_return::<CCache>(client, QueryResponse::Error(RCode::ServFail), question, Some(kill_token)).await,
                    // We don't know how to handle unknown errors.
                    // Assume they are a hard failure.
                    NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: _, question: _, answer: _, authority: _, additional: _ })) => return sender_return::<CCache>(client, QueryResponse::Error(RCode::ServFail), question, Some(kill_token)).await,
                    // Malformed response.
                    NSQueryResult::QueryResult(Ok(Message { id: _, qr: _, opcode: _, authoritative_answer: _, truncation: _, recursion_desired: _, recursion_available: _, z: _, rcode: _, question: _, answer: _, authority: _, additional: _ })) => return sender_return::<CCache>(client, QueryResponse::Error(RCode::ServFail), question, Some(kill_token)).await,
                    NSQueryResult::QueryResult(Err(_)) => (),
                    NSQueryResult::OutOfAddresses => (),
                    NSQueryResult::NSAddressQueryErr(_) => (),
                },
                () = tokio::time::sleep(Duration::from_millis(250)) => ()
            },
            3.. => select! {
                biased;
                result = active_queries.select_next_some() => match result {
                    // No error. Valid response.
                    NSQueryResult::QueryResult(Ok(response @ Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NoError, question: _, answer: _, authority: _, additional: _ })) => return sender_return::<CCache>(client, query_response(response), question, Some(kill_token)).await,
                    // If a name server cannot interpret what we are sending it, asking other name servers probably will not help.
                    // Treat as a hard error.
                    NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::FormErr, question: _, answer: _, authority: _, additional: _ })) => return sender_return::<CCache>(client, QueryResponse::Error(RCode::ServFail), question, Some(kill_token)).await,
                    // Only authoritative servers can indicate that a name does not exist.
                    NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: true, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NXDomain, question: _, answer: _, authority: _, additional: _ })) => return sender_return::<CCache>(client, QueryResponse::Error(RCode::NXDomain), question, Some(kill_token)).await,
                    NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: false, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NXDomain, question: _, answer: _, authority: _, additional: _ })) => (),
                    // If a server does not support a query type, we can probably assume it is not in that zone.
                    // TODO: verify that this is a valid assumption. Should we return NotImpl?
                    NSQueryResult::QueryResult(Ok(response @ Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NotImp, question: _, answer: _, authority: _, additional: _ })) => return sender_return::<CCache>(client, query_response(response), question, Some(kill_token)).await,
                    // If a name server refuses to perform an operation, we should not keep asking the other servers.
                    // TODO: verify that this is a valid way of handling.
                    NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::Refused, question: _, answer: _, authority: _, additional: _ })) => return sender_return::<CCache>(client, QueryResponse::Error(RCode::ServFail), question, Some(kill_token)).await,
                    // We don't know how to handle unknown errors.
                    // Assume they are a hard failure.
                    NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: _, question: _, answer: _, authority: _, additional: _ })) => return sender_return::<CCache>(client, QueryResponse::Error(RCode::ServFail), question, Some(kill_token)).await,
                    // Malformed response.
                    NSQueryResult::QueryResult(Ok(Message { id: _, qr: _, opcode: _, authoritative_answer: _, truncation: _, recursion_desired: _, recursion_available: _, z: _, rcode: _, question: _, answer: _, authority: _, additional: _ })) => return sender_return::<CCache>(client, QueryResponse::Error(RCode::ServFail), question, Some(kill_token)).await,
                    NSQueryResult::QueryResult(Err(_)) => (),
                    NSQueryResult::OutOfAddresses => (),
                    NSQueryResult::NSAddressQueryErr(_) => (),
                },
            }
        }
    }
    drop(active_queries);

    println!("Querying Non-Cached Name Servers for '{question}'");

    let mut active_queries = FuturesUnordered::new();
    // Can include both cached and non-cached servers if the cached server didn't complete.
    for ns_query in cached_queries.iter_mut()
        .filter(|ns_query| ns_query.is_querying())
        .chain(non_cached_queries.iter_mut())
    {
        active_queries.push(ns_query);
        match active_queries.len() {
            0..=2 => select! {
                biased;
                result = &mut active_queries.select_next_some() => match result {
                    // No error. Valid response.
                    NSQueryResult::QueryResult(Ok(response @ Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NoError, question: _, answer: _, authority: _, additional: _ })) => return sender_return::<CCache>(client, query_response(response), question, Some(kill_token)).await,
                    // If a name server cannot interpret what we are sending it, asking other name servers probably will not help.
                    // Treat as a hard error.
                    NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::FormErr, question: _, answer: _, authority: _, additional: _ })) => return sender_return::<CCache>(client, QueryResponse::Error(RCode::ServFail), question, Some(kill_token)).await,
                    // Only authoritative servers can indicate that a name does not exist.
                    NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: true, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NXDomain, question: _, answer: _, authority: _, additional: _ })) => return sender_return::<CCache>(client, QueryResponse::Error(RCode::NXDomain), question, Some(kill_token)).await,
                    NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: false, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NXDomain, question: _, answer: _, authority: _, additional: _ })) => (),
                    // If a server does not support a query type, we can probably assume it is not in that zone.
                    // TODO: verify that this is a valid assumption. Should we return NotImpl?
                    NSQueryResult::QueryResult(Ok(response @ Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NotImp, question: _, answer: _, authority: _, additional: _ })) => return sender_return::<CCache>(client, query_response(response), question, Some(kill_token)).await,
                    // If a name server refuses to perform an operation, we should not keep asking the other servers.
                    // TODO: verify that this is a valid way of handling.
                    NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::Refused, question: _, answer: _, authority: _, additional: _ })) => return sender_return::<CCache>(client, QueryResponse::Error(RCode::ServFail), question, Some(kill_token)).await,
                    // We don't know how to handle unknown errors.
                    // Assume they are a hard failure.
                    NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: _, question: _, answer: _, authority: _, additional: _ })) => return sender_return::<CCache>(client, QueryResponse::Error(RCode::ServFail), question, Some(kill_token)).await,
                    // Malformed response.
                    NSQueryResult::QueryResult(Ok(Message { id: _, qr: _, opcode: _, authoritative_answer: _, truncation: _, recursion_desired: _, recursion_available: _, z: _, rcode: _, question: _, answer: _, authority: _, additional: _ })) => return sender_return::<CCache>(client, QueryResponse::Error(RCode::ServFail), question, Some(kill_token)).await,
                    NSQueryResult::QueryResult(Err(_)) => (),
                    NSQueryResult::OutOfAddresses => (),
                    NSQueryResult::NSAddressQueryErr(_) => (),
                },
                () = tokio::time::sleep(Duration::from_millis(450)) => ()
            },
            3.. => select! {
                biased;
                result = &mut active_queries.select_next_some() => match result {
                    // No error. Valid response.
                    NSQueryResult::QueryResult(Ok(response @ Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NoError, question: _, answer: _, authority: _, additional: _ })) => return sender_return::<CCache>(client, query_response(response), question, Some(kill_token)).await,
                    // If a name server cannot interpret what we are sending it, asking other name servers probably will not help.
                    // Treat as a hard error.
                    NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::FormErr, question: _, answer: _, authority: _, additional: _ })) => return sender_return::<CCache>(client, QueryResponse::Error(RCode::ServFail), question, Some(kill_token)).await,
                    // Only authoritative servers can indicate that a name does not exist.
                    NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: true, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NXDomain, question: _, answer: _, authority: _, additional: _ })) => return sender_return::<CCache>(client, QueryResponse::Error(RCode::NXDomain), question, Some(kill_token)).await,
                    NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: false, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NXDomain, question: _, answer: _, authority: _, additional: _ })) => (),
                    // If a server does not support a query type, we can probably assume it is not in that zone.
                    // TODO: verify that this is a valid assumption. Should we return NotImpl?
                    NSQueryResult::QueryResult(Ok(response @ Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NotImp, question: _, answer: _, authority: _, additional: _ })) => return sender_return::<CCache>(client, query_response(response), question, Some(kill_token)).await,
                    // If a name server refuses to perform an operation, we should not keep asking the other servers.
                    // TODO: verify that this is a valid way of handling.
                    NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::Refused, question: _, answer: _, authority: _, additional: _ })) => return sender_return::<CCache>(client, QueryResponse::Error(RCode::ServFail), question, Some(kill_token)).await,
                    // We don't know how to handle unknown errors.
                    // Assume they are a hard failure.
                    NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: _, question: _, answer: _, authority: _, additional: _ })) => return sender_return::<CCache>(client, QueryResponse::Error(RCode::ServFail), question, Some(kill_token)).await,
                    // Malformed response.
                    NSQueryResult::QueryResult(Ok(Message { id: _, qr: _, opcode: _, authoritative_answer: _, truncation: _, recursion_desired: _, recursion_available: _, z: _, rcode: _, question: _, answer: _, authority: _, additional: _ })) => return sender_return::<CCache>(client, QueryResponse::Error(RCode::ServFail), question, Some(kill_token)).await,
                    NSQueryResult::QueryResult(Err(_)) => (),
                    NSQueryResult::OutOfAddresses => (),
                    NSQueryResult::NSAddressQueryErr(_) => (),
                },
            }
        }
    }

    while !active_queries.is_empty() {
        select! {
            biased;
            result = &mut active_queries.select_next_some() => match result {
                // No error. Valid response.
                NSQueryResult::QueryResult(Ok(response @ Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NoError, question: _, answer: _, authority: _, additional: _ })) => return sender_return::<CCache>(client, query_response(response), question, Some(kill_token)).await,
                // If a name server cannot interpret what we are sending it, asking other name servers probably will not help.
                // Treat as a hard error.
                NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::FormErr, question: _, answer: _, authority: _, additional: _ })) => return sender_return::<CCache>(client, QueryResponse::Error(RCode::ServFail), question, Some(kill_token)).await,
                // Only authoritative servers can indicate that a name does not exist.
                NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: true, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NXDomain, question: _, answer: _, authority: _, additional: _ })) => return sender_return::<CCache>(client, QueryResponse::Error(RCode::NXDomain), question, Some(kill_token)).await,
                NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: false, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NXDomain, question: _, answer: _, authority: _, additional: _ })) => (),
                // If a server does not support a query type, we can probably assume it is not in that zone.
                // TODO: verify that this is a valid assumption. Should we return NotImpl?
                NSQueryResult::QueryResult(Ok(response @ Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NotImp, question: _, answer: _, authority: _, additional: _ })) => return sender_return::<CCache>(client, query_response(response), question, Some(kill_token)).await,
                // If a name server refuses to perform an operation, we should not keep asking the other servers.
                // TODO: verify that this is a valid way of handling.
                NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::Refused, question: _, answer: _, authority: _, additional: _ })) => return sender_return::<CCache>(client, QueryResponse::Error(RCode::ServFail), question, Some(kill_token)).await,
                // We don't know how to handle unknown errors.
                // Assume they are a hard failure.
                NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: _, question: _, answer: _, authority: _, additional: _ })) => return sender_return::<CCache>(client, QueryResponse::Error(RCode::ServFail), question, Some(kill_token)).await,
                // Malformed response.
                NSQueryResult::QueryResult(Ok(Message { id: _, qr: _, opcode: _, authoritative_answer: _, truncation: _, recursion_desired: _, recursion_available: _, z: _, rcode: _, question: _, answer: _, authority: _, additional: _ })) => return sender_return::<CCache>(client, QueryResponse::Error(RCode::ServFail), question, Some(kill_token)).await,
                NSQueryResult::QueryResult(Err(_)) => (),
                NSQueryResult::OutOfAddresses => (),
                NSQueryResult::NSAddressQueryErr(_) => (),
            },
        }
    }

    return sender_return::<CCache>(client, QueryResponse::Error(RCode::ServFail), question, Some(kill_token)).await;
}

#[inline]
// Note: Returns the result so that this function can be called as the return statement.
async fn sender_return<CCache>(client: &Arc<DNSAsyncClient>, result: QueryResponse<ResourceRecord>, question: &Question, kill_token: Option<Arc<AwakeToken>>) -> QueryResponse<ResourceRecord> where CCache: AsyncCache {
    // Cleanup.
    let mut write_locked_active_query_manager = client.active_query_manager.write().await;
    let sender = write_locked_active_query_manager.remove(question);
    drop(write_locked_active_query_manager);

    // Send out the answer to anyone waiting.
    if let Some(sender) = sender {
        let _ = sender.send(result.clone());
    }

    if let Some(kill_token) = kill_token {
        kill_token.awake();
    }

    // Return the result.
    return result;
}

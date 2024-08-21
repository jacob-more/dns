use std::{borrow::BorrowMut, collections::VecDeque, future::Future, io, net::IpAddr, pin::Pin, sync::Arc, task::Poll, time::Duration};

use async_lib::awake_token::AwakeToken;
use dns_lib::{interface::cache::cache::AsyncCache, query::{message::Message, qr::QR, question::Question}, resource_record::{rcode::RCode, resource_record::ResourceRecord, rtype::RType}, types::c_domain_name::CDomainName};
use futures::StreamExt;
use pin_project::pin_project;
use tokio::{pin, sync::broadcast};

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

#[pin_project]
struct NSSelectQuery<'a, Fut, Queries>
where
    Fut: Future<Output = NSQueryResult> + 'a,
    Queries: Iterator<Item = &'a mut Pin<Box<Fut>>>,
    
{
    ns_queries: Queries,
    ns_queries_empty: bool,
    running: Vec<&'a mut Pin<Box<Fut>>>,
    ready_results: VecDeque<NSQueryResult>,
    max_concurrency: usize,
    add_query_timeout: Duration,
    #[pin]
    add_query_timer: Option<tokio::time::Sleep>,
}

impl<'a, Fut, Queries> NSSelectQuery<'a, Fut, Queries>
where
    Fut: Future<Output = NSQueryResult> + 'a,
    Queries: Iterator<Item = &'a mut Pin<Box<Fut>>>,
{
    pub fn new(ns_queries: Queries, max_concurrency: usize, add_query_timeout: Duration) -> Self {
        Self {
            ns_queries,
            ns_queries_empty: false,
            running: Vec::new(),
            ready_results: VecDeque::new(),
            max_concurrency,
            add_query_timeout,
            add_query_timer: None,
        }
    }

    fn is_first_poll(&self) -> bool {
        // After the first poll, the running queue should never be left empty
        // between polls as long as there are more queries to try.
        self.running.is_empty()
        && !self.ns_queries_empty
        && self.ready_results.is_empty()
    }
}

impl<'a, Fut, Queries> Future for NSSelectQuery<'a, Fut, Queries>
where
    Fut: Future<Output = NSQueryResult> + 'a,
    Queries: Iterator<Item = &'a mut Pin<Box<Fut>>>,
{
    type Output = Option<NSQueryResult>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        if self.is_first_poll() {
            let mut this = self.as_mut().project();
            // Initialize the `running` queue with its first query.
            match this.ns_queries.next() {
                Some(ns_query) => this.running.push(ns_query),
                None => {
                    *this.ns_queries_empty = true;
                    // Don't want to be erroneously woken up if we are done.
                    // Although on the first poll, it is probably already None.
                    this.add_query_timer.set(None);
                    return Poll::Ready(None);
                },
            }

            // Initialize or refresh the `add_query_timer`
            match (this.add_query_timer.as_mut().as_pin_mut(), tokio::time::Instant::now().checked_add(*this.add_query_timeout)) {
                // The expected case is that a fresh NSSelectQuery has not yet
                // had the `add_query_timer` initialized. We should do that
                // here.
                (None, Some(deadline)) => this.add_query_timer.set(Some(tokio::time::sleep_until(deadline))),
                // If a time was created manually, we should refresh the
                // deadline since it may have become stale if this task has
                // been waiting to be run for a while.
                (Some(timer), Some(deadline)) => timer.reset(deadline),
                // If the timer cannot be reset, then this will only run 1 task
                // at a time since it is unable to schedule them to start later.
                // This should never really be an issue, but if it is, we could
                // have the system schedule `max_concurrency` many tasks
                // immediately. However, I would be concerned that this may
                // accidentally overwhelm ourself or the endpoints. Presumably,
                // if this fails, then this will probably fail for all other
                // queries in the system. If we had all of them run
                // `max_concurrency` many concurrent tasks, the system might
                // DOS itself.
                (_, None) => this.add_query_timer.set(None),
            }
        }

        loop {
            let mut poll_again = false;
            let mut this = self.as_mut().project();
            match (this.add_query_timer.as_mut().as_pin_mut(), tokio::time::Instant::now().checked_add(*this.add_query_timeout)) {
                (Some(mut timer), Some(new_deadline)) => match timer.as_mut().poll(cx) {
                    Poll::Ready(()) => match this.ns_queries.next() {
                        Some(ns_query) => {
                            this.running.push(ns_query);
                            // Keep setting the timer until the maximum number
                            // of allowed concurrent queries have been started
                            // for this group. Then, we will maintain that many
                            // concurrent queries until we run out of queued
                            // queries.
                            if this.running.len() < *this.max_concurrency {
                                // Poll again is set so that the new timer is
                                // polled (to get it started).
                                timer.reset(new_deadline);
                                poll_again = true;
                            } else {
                                this.add_query_timer.set(None);
                            }
                        },
                        None => {
                            *this.ns_queries_empty = true;
                            // Don't want to be erroneously woken up if there
                            // is nobody else to add.
                            this.add_query_timer.set(None);
                        },
                    },
                    Poll::Pending => (),
                },
                (Some(mut timer), None) => match timer.as_mut().poll(cx) {
                    Poll::Ready(()) => match this.ns_queries.next() {
                        Some(ns_query) => {
                            this.running.push(ns_query);
                            // Since a deadline could not be calculated, the
                            // timer cannot be reset.
                            // This could limit the number of concurrent
                            // processes that can run below `max_concurrency`.
                            // I go into more detail on why this is the
                            // preferred option earlier in this function.
                            this.add_query_timer.set(None);
                        },
                        None => {
                            *this.ns_queries_empty = true;
                            // Don't want to be erroneously woken up if there
                            // is nobody else to add.
                            this.add_query_timer.set(None);
                        },
                    },
                    Poll::Pending => (),
                },
                (None, _) => (),
            }

            // Poll all the tasks currently marked as running. Tasks that are
            // ready should be removed from this queue and their result should
            // be stored. They will be replaced (if possible) after this loop
            // is done to try to maintain the same number of running tasks.
            let mut removed_count: usize = 0;
            this.running.retain_mut(|ns_query| match ns_query.as_mut().poll(cx) {
                Poll::Ready(result) => {
                    this.ready_results.push_back(result);
                    removed_count += 1;
                    false
                },
                Poll::Pending => true,
            });
            // Add back as many tasks as were removed, unless the queue of
            // incoming tasks runs out.
            for _ in 0..removed_count {
                match this.ns_queries.next() {
                    Some(ns_query) => {
                        this.running.push(ns_query);
                        // Want to get newly added tasks polled so that they
                        // get started and can wake this task up.
                        poll_again = true;
                    },
                    None => {
                        *this.ns_queries_empty = true;
                        // Don't want to be erroneously woken up if there is
                        // nobody else to add.
                        this.add_query_timer.set(None);
                        break;
                    },
                }
            }

            if !poll_again {
                break;
            }
        }

        let this = self.as_mut().project();
        match (this.running.len(), this.ns_queries_empty, this.ready_results.pop_front()) {
            // All of the queued queries have been tried.
            (0, true, None) => Poll::Ready(None),
            // At least 1 query is still running.
            (1.., _, None) => Poll::Pending,
            (_, _, Some(result)) => Poll::Ready(Some(result)),
            // There is still a queued query but it was never added to the
            // running queue. This should never occur, even if
            // `max_concurrency` is less than 1 since it is not checked before
            // adding the initial query to the queue.
            (0, false, None) => panic!("There are still queries in the queue but the running queue is empty"),
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

    println!("Querying Name Servers for '{question}'");

    pin!(
        let active_queries = NSSelectQuery::new(cached_queries.iter_mut().chain(non_cached_queries.iter_mut()), 3, Duration::from_millis(200));
    );
    loop {
        match active_queries.as_mut().await {
            // No error. Valid response.
            Some(NSQueryResult::QueryResult(Ok(response @ Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NoError, question: _, answer: _, authority: _, additional: _ }))) => return sender_return::<CCache>(client, query_response(response), question, Some(kill_token)).await,
            // If a name server cannot interpret what we are sending it, asking other name servers probably will not help.
            // Treat as a hard error.
            Some(NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::FormErr, question: _, answer: _, authority: _, additional: _ }))) => return sender_return::<CCache>(client, QueryResponse::Error(RCode::ServFail), question, Some(kill_token)).await,
            // Only authoritative servers can indicate that a name does not exist.
            Some(NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: true, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NXDomain, question: _, answer: _, authority: _, additional: _ }))) => return sender_return::<CCache>(client, QueryResponse::Error(RCode::NXDomain), question, Some(kill_token)).await,
            // This server does not have the authority to say that the name
            // does not exist. Ask others.
            Some(NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: false, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NXDomain, question: _, answer: _, authority: _, additional: _ }))) => continue,
            // If a server does not support a query type, we can probably assume it is not in that zone.
            // TODO: verify that this is a valid assumption. Should we return NotImpl?
            Some(NSQueryResult::QueryResult(Ok(response @ Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NotImp, question: _, answer: _, authority: _, additional: _ }))) => return sender_return::<CCache>(client, query_response(response), question, Some(kill_token)).await,
            // If a name server refuses to perform an operation, we should not keep asking the other servers.
            // TODO: verify that this is a valid way of handling.
            Some(NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::Refused, question: _, answer: _, authority: _, additional: _ }))) => return sender_return::<CCache>(client, QueryResponse::Error(RCode::ServFail), question, Some(kill_token)).await,
            // We don't know how to handle unknown errors.
            // Assume they are a hard failure.
            Some(NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: _, question: _, answer: _, authority: _, additional: _ }))) => return sender_return::<CCache>(client, QueryResponse::Error(RCode::ServFail), question, Some(kill_token)).await,
            // Malformed response.
            Some(NSQueryResult::QueryResult(Ok(Message { id: _, qr: _, opcode: _, authoritative_answer: _, truncation: _, recursion_desired: _, recursion_available: _, z: _, rcode: _, question: _, answer: _, authority: _, additional: _ }))) => return sender_return::<CCache>(client, QueryResponse::Error(RCode::ServFail), question, Some(kill_token)).await,
            // If there is an IO error, try a different server.
            Some(NSQueryResult::QueryResult(Err(_))) => continue,
            // If a particular name server cannot be queried anymore, then keep
            // trying to query the others.
            Some(NSQueryResult::OutOfAddresses) => continue,
            // If there was an error looking up one of the name servers, keep
            // trying to look up the others.
            Some(NSQueryResult::NSAddressQueryErr(_)) => continue,
            None => break,
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

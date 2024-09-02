use std::{borrow::BorrowMut, future::Future, io, net::IpAddr, pin::Pin, sync::Arc, task::Poll, time::Duration};

use async_lib::awake_token::AwakeToken;
use dns_lib::{interface::{cache::cache::AsyncCache, client::Context}, query::{message::Message, qr::QR, question::Question}, resource_record::{rcode::RCode, resource_record::ResourceRecord, rtype::RType}, types::c_domain_name::CDomainName};
use futures::{future::BoxFuture, FutureExt};
use pin_project::{pin_project, pinned_drop};
use tokio::sync::{broadcast::{self, error::RecvError}, RwLockReadGuard, RwLockWriteGuard};

use crate::{query::recursive_query::recursive_query, DNSAsyncClient};

use super::{network_query::query_network, recursive_query::{query_cache, QueryResponse}};


async fn query_cache_for_ns_addresses<'a, 'b, CCache>(ns_domain: CDomainName, address_rtype: RType, context: Arc<Context>, kill_token: Option<Arc<AwakeToken>>, client: Arc<DNSAsyncClient>, joined_cache: Arc<CCache>) -> NSQuery<'a, 'b, CCache> where CCache: AsyncCache + Send + Sync {
    let ns_question = context.query().with_new_qname_qtype(ns_domain.clone(), address_rtype.clone());

    fn rr_to_ip(record: ResourceRecord) -> Option<IpAddr> {
        match record {
            ResourceRecord::A(_, rdata) => Some(IpAddr::V4(*rdata.ipv4_addr())),
            ResourceRecord::AAAA(_, rdata) => Some(IpAddr::V6(*rdata.ipv6_addr())),
            _ => None,
        }
    }

    let ns_init_state = match query_cache(&joined_cache, &ns_question).await {
        QueryResponse::Records(mut records) => NSQueryState::CacheHit {
            ns_addresses: records.drain(..).filter_map(|record| rr_to_ip(record)).collect()
        },
        _ => NSQueryState::CacheMiss,
    };

    NSQuery {
        ns_domain,
        ns_address_rtype: address_rtype,
        context,

        kill_token,
        client,
        joined_cache,

        state: ns_init_state,
    }
}

enum NSQueryState<'a, 'b> {
    CacheMiss,
    QueryingNetworkNSAddresses {
        ns_addresses_query: BoxFuture<'a, QueryResponse<ResourceRecord>>,
    },
    CacheHit {
        ns_addresses: Vec<IpAddr>,
    },
    QueryingNetwork {
        query: BoxFuture<'b, io::Result<Message>>,
        remaining_ns_addresses: Vec<IpAddr>,
    },
    OutOfAddresses,
}

enum NSQueryResult {
    OutOfAddresses,
    NSAddressQueryErr(RCode),
    QueryResult(io::Result<Message>),
}

#[pin_project]
struct NSQuery<'a, 'b, CCache> where CCache: AsyncCache + Send + Sync {
    ns_domain: CDomainName,
    ns_address_rtype: RType,
    context: Arc<Context>,

    kill_token: Option<Arc<AwakeToken>>,
    client: Arc<DNSAsyncClient>,
    joined_cache: Arc<CCache>,

    state: NSQueryState<'a, 'b>,
}

impl<'a, 'b, CCache> Future for NSQuery<'a, 'b, CCache> where CCache: AsyncCache + Send + Sync + 'static {
    type Output = NSQueryResult;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        async fn recursive_query_owned_args<CCache>(client: Arc<DNSAsyncClient>, joined_cache: Arc<CCache>, context: Context) -> QueryResponse<ResourceRecord> where CCache: AsyncCache + Send + Sync + 'static {
            recursive_query(client, joined_cache, context).await
        }

        async fn query_network_owned_args<CCache>(client: Arc<DNSAsyncClient>, joined_cache: Arc<CCache>, context: Arc<Context>, name_server_address: IpAddr, kill_token: Option<Arc<AwakeToken>>) -> io::Result<Message> where CCache: AsyncCache + Send + Sync {
            query_network(&client, joined_cache, context.query(), &name_server_address, kill_token).await
        }

        fn rr_to_ip(record: ResourceRecord) -> Option<IpAddr> {
            match record {
                ResourceRecord::A(_, rdata) => Some(IpAddr::V4(*rdata.ipv4_addr())),
                ResourceRecord::AAAA(_, rdata) => Some(IpAddr::V6(*rdata.ipv6_addr())),
                _ => None,
            }
        }

        loop {
            let this = self.as_mut().project();
            match this.state {
                NSQueryState::CacheMiss => {
                    let client = self.client.clone();
                    let cache = self.joined_cache.clone();
                    match self.context.clone().new_ns_address(self.context.query().with_new_qname_qtype(self.ns_domain.clone(), self.ns_address_rtype)) {
                        Ok(ns_address_context) => {
                            self.state = NSQueryState::QueryingNetworkNSAddresses { ns_addresses_query: Box::pin(recursive_query_owned_args(client, cache, ns_address_context)) };
                            // Next loop will poll the query for NS addresses
                            continue;
                        },
                        Err(error) => {
                            self.state = NSQueryState::OutOfAddresses;
                            println!("NS Lookup Error: {error}");
                            // Exit loop. The was an error trying to query for
                            // the addresses.
                            return Poll::Ready(NSQueryResult::NSAddressQueryErr(RCode::ServFail));
                        },
                    };
                },
                NSQueryState::QueryingNetworkNSAddresses { ns_addresses_query } => {
                    match ns_addresses_query.as_mut().poll(cx) {
                        Poll::Ready(QueryResponse::Records(mut records)) => {
                            let mut ns_addresses = records.drain(..)
                                .filter_map(|record| rr_to_ip(record))
                                .collect::<Vec<_>>();
                            match ns_addresses.pop() {
                                Some(first_ns_address) => {
                                    let client = this.client.clone();
                                    let cache = this.joined_cache.clone();
                                    let context = this.context.clone();
                                    let kill_token = self.kill_token.clone();
                                    let query = Box::pin(query_network_owned_args(client, cache, context, first_ns_address, kill_token));
                                    self.state = NSQueryState::QueryingNetwork { query, remaining_ns_addresses: ns_addresses };
                                    // Next loop will poll the query for the question.
                                    continue;
                                },
                                None => {
                                    self.state = NSQueryState::OutOfAddresses;
                                    // Exit loop. There are no addresses to query.
                                    return Poll::Ready(NSQueryResult::OutOfAddresses);
                                },
                            }
                        }
                        Poll::Ready(QueryResponse::NoRecords) => {
                            self.state = NSQueryState::OutOfAddresses;
                            // Exit loop. There are no addresses to query.
                            return Poll::Ready(NSQueryResult::OutOfAddresses);
                        },
                        Poll::Ready(QueryResponse::Error(rcode)) => {
                            self.state = NSQueryState::OutOfAddresses;
                            // Exit loop. The was an error trying to query for
                            // the addresses.
                            return Poll::Ready(NSQueryResult::NSAddressQueryErr(rcode));
                        },
                        // Exit loop. Will be woken up by the ns address query.
                        Poll::Pending => return Poll::Pending,
                    }
                },
                NSQueryState::CacheHit { ns_addresses } => {
                    match ns_addresses.pop() {
                        Some(first_ns_address) => {
                            let client = this.client.clone();
                            let cache = this.joined_cache.clone();
                            let context = this.context.clone();
                            let kill_token = this.kill_token.clone();
                            let remaining_ns_addresses = ns_addresses.drain(..).collect();
                            let query = Box::pin(query_network_owned_args(client, cache, context, first_ns_address, kill_token));
                            self.state = NSQueryState::QueryingNetwork { query, remaining_ns_addresses };
                            // Next loop will poll the query for the question.
                            continue;
                        },
                        None => {
                            self.state = NSQueryState::OutOfAddresses;
                            // Exit loop. There are no addresses to query.
                            return Poll::Ready(NSQueryResult::OutOfAddresses);
                        },
                    }
                },
                NSQueryState::QueryingNetwork { query, remaining_ns_addresses } => {
                    match query.as_mut().poll(cx) {
                        Poll::Ready(result) => {
                            match remaining_ns_addresses.pop() {
                                Some(next_ns_address) => {
                                    // Set up the next query. It will only be
                                    // polled if this struct is awaited again.
                                    let client = this.client.clone();
                                    let cache = this.joined_cache.clone();
                                    let context = this.context.clone();
                                    let kill_token = this.kill_token.clone();
                                    *query = Box::pin(query_network_owned_args(client, cache, context, next_ns_address, kill_token));
                                    // Exit loop. A result was found.
                                    return Poll::Ready(NSQueryResult::QueryResult(result));
                                },
                                None => {
                                    self.state = NSQueryState::OutOfAddresses;
                                    // Exit loop. A result was found.
                                    return Poll::Ready(NSQueryResult::QueryResult(result));
                                },
                            }
                        },
                        // Exit loop. Will be woken up by the query.
                        Poll::Pending => return Poll::Pending,
                    }
                },
                // Exit loop. All addresses have been queried.
                NSQueryState::OutOfAddresses => return Poll::Ready(NSQueryResult::OutOfAddresses),
            }
        }
    }
}

#[pin_project]
struct NSSelectQuery<Fut> where Fut: Future<Output = NSQueryResult> {
    // Note: the queries are read in reverse order (like a stack).
    ns_queries: Vec<Pin<Box<Fut>>>,
    running: Vec<Pin<Box<Fut>>>,
    max_concurrency: usize,
    add_query_timeout: Duration,
    #[pin]
    add_query_timer: Option<tokio::time::Sleep>,
}

impl<Fut> NSSelectQuery<Fut> where Fut: Future<Output = NSQueryResult> {
    pub fn new(ns_queries: Vec<Pin<Box<Fut>>>, max_concurrency: usize, add_query_timeout: Duration) -> Self {
        Self {
            ns_queries,
            running: Vec::new(),
            max_concurrency,
            add_query_timeout,
            add_query_timer: None,
        }
    }

    fn is_first_poll(&self) -> bool {
        // After the first poll, the running queue should never be left empty
        // between polls as long as there are more queries to try.
        self.running.is_empty()
        && !self.ns_queries.is_empty()
    }
}

impl<Fut> Future for NSSelectQuery<Fut> where Fut: Future<Output = NSQueryResult> {
    type Output = Option<NSQueryResult>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        if self.is_first_poll() {
            let mut this = self.as_mut().project();
            // Initialize the `running` queue with its first query.
            match this.ns_queries.pop() {
                Some(ns_query) => this.running.push(ns_query),
                None => {
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
                    Poll::Ready(()) => match this.ns_queries.pop() {
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
                                // If a result is found and this returns
                                // Poll::Ready(), then it won't get polled
                                // unless this struct is awaited again.
                                timer.reset(new_deadline);
                                poll_again = true;
                            } else {
                                // Once we have the maximum number of tasks
                                // running concurrently, we don't need to wake
                                // up to add new tasks. New tasks from
                                // `ns_query` will only be moved to `running`
                                // when a space opens up in `running`.
                                this.add_query_timer.set(None);
                            }
                        },
                        None => {
                            // Don't want to be erroneously woken up if there
                            // is nobody else to add.
                            this.add_query_timer.set(None);
                        },
                    },
                    Poll::Pending => (),
                },
                (Some(mut timer), None) => match timer.as_mut().poll(cx) {
                    Poll::Ready(()) => match this.ns_queries.pop() {
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
                            // Don't want to be erroneously woken up if there
                            // is nobody else to add.
                            this.add_query_timer.set(None);
                        },
                    },
                    Poll::Pending => (),
                },
                (None, _) => (),
            }

            let mut query_result = None;
            for (index, ns_query) in this.running.iter_mut().enumerate() {
                if let Poll::Ready(result) = ns_query.as_mut().poll(cx) {
                    query_result = Some(result);
                    match this.ns_queries.pop() {
                        Some(new_ns_query) => {
                            // We can re-use the spot in the `running` list for
                            // the new query since we don't care about the
                            // order of this list. They should all get polled
                            // eventually (as long as no result is found).
                            // Re-using the spot means the vector does not need
                            // to shift all the elements to the right of this
                            // index left just for us to append to the end.
                            *ns_query = new_ns_query;
                            // Want to get newly added tasks polled so that they
                            // get started and can wake this task up.
                            // If a result is found and this returns
                            // Poll::Ready(), then it won't get polled unless
                            // this struct is awaited again.
                            poll_again = true;
                        },
                        None => {
                            let _ = this.running.swap_remove(index);
                            // Don't want to be erroneously woken up if there is
                            // nobody else to add.
                            this.add_query_timer.set(None);
                        },
                    }
                    break;
                }
            }

            if let Some(result) = query_result {
                return Poll::Ready(Some(result));
            }

            if !poll_again {
                break;
            }
        }

        let this = self.as_mut().project();
        match (this.ns_queries.len(), this.running.len()) {
            // All of the queued queries have completed.
            (0, 0) => Poll::Ready(None),
            // At least 1 query is still running.
            (_, 1..) => Poll::Pending,
            (1.., 0) => panic!("There are still queries in the queue but the running queue is empty"),
        }
    }
}

#[pin_project(PinnedDrop)]
struct NSRoundRobin<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h, 'i, 'j, 'k, 'l, CCache>
where
    CCache: AsyncCache + Send + Sync + 'static,
    'a: 'e + 'f + 'g + 'h,
    'f: 'e,
    'h: 'g,
    'k: 'j,
    'l: 'j,
{
    client: &'a Arc<DNSAsyncClient>,
    joined_cache: &'b Arc<CCache>,
    question: &'c Arc<Context>,
    inner: InnerNSRoundRobin<'d, 'e, 'f, 'g, 'h, 'i, 'j, 'k, 'l, CCache>,
}

enum InnerNSRoundRobin<'d, 'e, 'f, 'g, 'h, 'i, 'j, 'k, 'l, CCache>
where
    CCache: AsyncCache + Send + Sync + 'static,
    'f: 'e,
    'h: 'g,
    'k: 'j,
    'l: 'j,
{
    Fresh {
        name_servers: &'d [CDomainName],
    },
    AwaitingReadLock {
        name_servers: &'d [CDomainName],
        read_lock_future: BoxFuture<'e, RwLockReadGuard<'f, std::collections::HashMap<Question, (Arc<Context>, broadcast::Sender<QueryResponse<ResourceRecord>>)>>>,
    },
    AwaitingWriteLock {
        name_servers: &'d [CDomainName],
        write_lock_future: BoxFuture<'g, RwLockWriteGuard<'h, std::collections::HashMap<Question, (Arc<Context>, broadcast::Sender<QueryResponse<ResourceRecord>>)>>>,
    },
    Forwarded {
        name_servers: &'d [CDomainName],
        receiver: BoxFuture<'i, Result<QueryResponse<ResourceRecord>, RecvError>>,
    },
    GetCachedNSAddresses {
        sender: broadcast::Sender<QueryResponse<ResourceRecord>>,
        kill_token: Arc<AwakeToken>,
        name_server_address_queries: Vec<BoxFuture<'j, NSQuery<'k, 'l, CCache>>>,
        name_server_non_cached_queries: Vec<Pin<Box<NSQuery<'k, 'l, CCache>>>>,
        name_server_cached_queries: Vec<Pin<Box<NSQuery<'k, 'l, CCache>>>>,
    },
    QueryNameServers {
        sender: broadcast::Sender<QueryResponse<ResourceRecord>>,
        kill_token: Arc<AwakeToken>,
        ns_query_select: Pin<Box<NSSelectQuery<NSQuery<'k, 'l, CCache>>>>,
    },
    Cleanup {
        write_lock_future: BoxFuture<'g, RwLockWriteGuard<'h, std::collections::HashMap<Question, (Arc<Context>, broadcast::Sender<QueryResponse<ResourceRecord>>)>>>,
        result: QueryResponse<ResourceRecord>,
    },
    Complete,
}

impl<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h, 'i, 'j, 'k, 'l, CCache> NSRoundRobin<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h, 'i, 'j, 'k, 'l, CCache> where CCache: AsyncCache + Send + Sync + 'static {
    fn new(client: &'a Arc<DNSAsyncClient>, joined_cache: &'b Arc<CCache>, question: &'c Arc<Context>, name_servers: &'d [CDomainName]) -> Self {
        Self { client, joined_cache, question, inner: InnerNSRoundRobin::Fresh { name_servers } }
    }
}

impl<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h, 'i, 'j, 'k, 'l, CCache> Future for NSRoundRobin<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h, 'i, 'j, 'k, 'l, CCache> where CCache: AsyncCache + Send + Sync + 'static {
    type Output = QueryResponse<ResourceRecord>;
    
    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        loop {
            let this = self.as_mut().project();
            match this.inner.borrow_mut() {
                InnerNSRoundRobin::Fresh { name_servers } => {
                    let read_lock_future = this.client.active_query_manager.read().boxed();
                    let name_servers = *name_servers;
    
                    *this.inner = InnerNSRoundRobin::AwaitingReadLock { name_servers, read_lock_future };
                    // Next loop will poll the read lock
                    continue;
                },
                InnerNSRoundRobin::AwaitingReadLock { name_servers, read_lock_future } => {
                    match read_lock_future.as_mut().poll(cx) {
                        Poll::Ready(r_active_queries) => match r_active_queries.get(this.question.query()) {
                            Some((sender_context, sender)) => {
                                match sender_context.is_ns_allowed(this.question.qname()) {
                                    Ok(()) => {
                                        let receiver = sender.subscribe();
                                        let receiver = async move {
                                            let mut receiver = receiver;
                                            receiver.recv().await
                                        }.boxed();
                                        drop(r_active_queries);
                                        println!("Already Forwarded (1): '{}'", this.question.query());
                                        let name_servers = *name_servers;
        
                                        *this.inner = InnerNSRoundRobin::Forwarded { name_servers, receiver };
                                        // Next loop will poll the receiver
                                        continue;
                                    },
                                    Err(error) => {
                                        drop(r_active_queries);
                                        println!("Cannot use Forwarded (1): {error}");

                                        *this.inner = InnerNSRoundRobin::Complete;
                                        // Exit forever. Query complete.
                                        return Poll::Ready(QueryResponse::Error(RCode::ServFail));
                                    },
                                }
                            },
                            None => {
                                drop(r_active_queries);
                                let write_lock_future = this.client.active_query_manager.write().boxed();
                                let name_servers = *name_servers;

                                *this.inner = InnerNSRoundRobin::AwaitingWriteLock { name_servers, write_lock_future };
                                // Next loop will poll the write lock
                                continue;
                            },
                        },
                        // Exit loop. Wait for read lock to wake us again.
                        Poll::Pending => return Poll::Pending,
                    }
                },
                InnerNSRoundRobin::AwaitingWriteLock { name_servers, write_lock_future } => {
                    match write_lock_future.as_mut().poll(cx) {
                        Poll::Ready(mut w_active_queries) => match w_active_queries.get(this.question.query()) {
                            Some((sender_context, sender)) => {
                                match sender_context.is_ns_allowed(this.question.qname()) {
                                    Ok(()) => {
                                        let receiver = sender.subscribe();
                                        let receiver = async move {
                                            let mut receiver = receiver;
                                            receiver.recv().await
                                        }.boxed();
                                        drop(w_active_queries);
                                        println!("Already Forwarded (2): '{}'", this.question.query());
                                        let name_servers = *name_servers;
        
                                        *this.inner = InnerNSRoundRobin::Forwarded { name_servers, receiver };
                                        // Next loop will poll the receiver
                                        continue;
                                    },
                                    Err(error) => {
                                        drop(w_active_queries);
                                        println!("Cannot use Forwarded (2): {error}");

                                        *this.inner = InnerNSRoundRobin::Complete;
                                        // Exit forever. Query complete.
                                        return Poll::Ready(QueryResponse::Error(RCode::ServFail));
                                    },
                                }
                            },
                            None => {
                                let (sender, _) = broadcast::channel(1);
                                w_active_queries.insert(this.question.query().clone(), (this.question.clone(), sender.clone()));
                                drop(w_active_queries);
                                let kill_token = Arc::new(AwakeToken::new());

                                let name_server_address_queries = name_servers.iter()
                                    .flat_map(|ns_domain| [
                                        query_cache_for_ns_addresses(ns_domain.clone(), RType::A, this.question.clone(), Some(kill_token.clone()), this.client.clone(), this.joined_cache.clone()).boxed(),
                                        query_cache_for_ns_addresses(ns_domain.clone(), RType::AAAA, this.question.clone(), Some(kill_token.clone()), this.client.clone(), this.joined_cache.clone()).boxed(),
                                    ])
                                    .collect::<Vec<_>>();
                                let capacity = name_server_address_queries.len();

                                *this.inner = InnerNSRoundRobin::GetCachedNSAddresses { sender, kill_token, name_server_address_queries, name_server_cached_queries: Vec::with_capacity(capacity), name_server_non_cached_queries: Vec::with_capacity(capacity) };
                                // Next loop will poll all the NS address queries
                                continue;
                            },
                        },
                        // Exit loop. Wait for read lock to wake us again.
                        Poll::Pending => return Poll::Pending,
                    }
                },
                InnerNSRoundRobin::Forwarded { name_servers, receiver } => {
                    match receiver.as_mut().poll(cx) {
                        // Exit. Answer returned.
                        Poll::Ready(Ok(query_response)) => {
                            println!("Forward Received: '{}'", this.question.query());

                            *this.inner = InnerNSRoundRobin::Complete;
                            // Exit forever. Query complete.
                            return Poll::Ready(query_response)
                        },
                        Poll::Ready(Err(RecvError::Closed)) => {
                            println!("Recoverable Internal Error: channel closed\nContinuing Query: '{}'", this.question.query());
                            let name_servers = *name_servers;

                            *this.inner = InnerNSRoundRobin::Fresh { name_servers };
                            // Next loop will restart the query
                            continue;
                        },
                        Poll::Ready(Err(RecvError::Lagged(skipped_message_count))) => {
                            println!("Recoverable Internal Error: channel lagged. Skipping {skipped_message_count} messages\nContinuing Query: '{}'", this.question.query());
                            // Next loop will re-poll the receiver to get a message
                            continue;
                        },
                        // Exit loop. Wait for receiver to wake us again.
                        Poll::Pending => return Poll::Pending,
                    }
                },
                InnerNSRoundRobin::GetCachedNSAddresses { sender, kill_token, name_server_address_queries, name_server_non_cached_queries, name_server_cached_queries } => {
                    name_server_address_queries.retain_mut(|ns_address_query| {
                        match ns_address_query.as_mut().poll(cx) {
                            Poll::Ready(ns_query @ NSQuery { ns_domain: _, ns_address_rtype: _, context: _, kill_token: _, client: _, joined_cache: _, state: NSQueryState::CacheHit { ns_addresses: _ } }) => {
                                name_server_cached_queries.push(Box::pin(ns_query));
                                false
                            },
                            Poll::Ready(ns_query) => {
                                name_server_non_cached_queries.push(Box::pin(ns_query));
                                false
                            },
                            Poll::Pending => true,
                        }
                    });
                    if name_server_address_queries.is_empty() {
                        // Join the two lists of queries. The queries that don't have cached
                        // addresses are at the front and the ones with cached addresses are at the
                        // back. This list will be read like a stack, so the cached queries will be
                        // run first.
                        let mut ns_queries = Vec::with_capacity(name_server_non_cached_queries.len() + name_server_cached_queries.len());
                        ns_queries.extend(name_server_non_cached_queries.drain(..));
                        ns_queries.extend(name_server_cached_queries.drain(..));
                        let ns_query_select = Box::pin(NSSelectQuery::new(ns_queries, 3, Duration::from_millis(200)));

                        let sender = sender.clone();
                        let kill_token = kill_token.clone();
                        *this.inner = InnerNSRoundRobin::QueryNameServers { sender, kill_token, ns_query_select };
                        // Next loop will select the first query from the list and start it
                        continue;
                    } else {
                        // Exit loop. Wait for one of the address queries to wake us again.
                        return Poll::Pending;
                    }
                },
                InnerNSRoundRobin::QueryNameServers { sender, kill_token, ns_query_select } => {
                    match ns_query_select.as_mut().poll(cx) {
                        // No error. Valid response.
                        Poll::Ready(Some(NSQueryResult::QueryResult(Ok(response @ Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NoError, question: _, answer: _, authority: _, additional: _ }))))
                        // If a server does not support a query type, we can probably assume it is not in that zone.
                        // TODO: verify that this is a valid assumption. Should we return NotImpl?
                      | Poll::Ready(Some(NSQueryResult::QueryResult(Ok(response @ Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NotImp, question: _, answer: _, authority: _, additional: _ })))) => {
                            let result = query_response(response);
                            let write_lock_future = this.client.active_query_manager.write().boxed();

                            // Send out the answer to anyone waiting.
                            let _ = sender.send(result.clone());
                            kill_token.awake();

                            *this.inner = InnerNSRoundRobin::Cleanup { write_lock_future, result };
                            // Next loop will start the cleanup process
                            continue;
                        },
                        // Only authoritative servers can indicate that a name does not exist.
                        Poll::Ready(Some(NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: true, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NXDomain, question: _, answer: _, authority: _, additional: _ })))) => {
                            let result = QueryResponse::Error(RCode::NXDomain);
                            let write_lock_future = this.client.active_query_manager.write().boxed();

                            // Send out the answer to anyone waiting.
                            let _ = sender.send(result.clone());
                            kill_token.awake();

                            *this.inner = InnerNSRoundRobin::Cleanup { write_lock_future, result };
                            // Next loop will start the cleanup process
                            continue;
                        },
                        // This server does not have the authority to say that the name
                        // does not exist. Ask others.
                        Poll::Ready(Some(NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: false, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NXDomain, question: _, answer: _, authority: _, additional: _ }))))
                        // If there is an IO error, try a different server.
                      | Poll::Ready(Some(NSQueryResult::QueryResult(Err(_))))
                        // If a particular name server cannot be queried anymore, then keep
                        // trying to query the others.
                      | Poll::Ready(Some(NSQueryResult::OutOfAddresses))
                        // If there was an error looking up one of the name servers, keep
                        // trying to look up the others.
                      | Poll::Ready(Some(NSQueryResult::NSAddressQueryErr(_))) => {
                            // Next loop will poll the other name servers.
                            continue;
                        },
                        // If a name server cannot interpret what we are sending it, asking other name servers probably will not help.
                        // Treat as a hard error.
                        Poll::Ready(Some(NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::FormErr, question: _, answer: _, authority: _, additional: _ }))))
                        // If a name server refuses to perform an operation, we should not keep asking the other servers.
                        // TODO: verify that this is a valid way of handling.
                      | Poll::Ready(Some(NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::Refused, question: _, answer: _, authority: _, additional: _ }))))
                        // We don't know how to handle unknown errors.
                        // Assume they are a fatal failure.
                      | Poll::Ready(Some(NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: _, question: _, answer: _, authority: _, additional: _ }))))
                        // Malformed response.
                      | Poll::Ready(Some(NSQueryResult::QueryResult(Ok(Message { id: _, qr: _, opcode: _, authoritative_answer: _, truncation: _, recursion_desired: _, recursion_available: _, z: _, rcode: _, question: _, answer: _, authority: _, additional: _ }))))
                        // No more servers to query.
                      | Poll::Ready(None) => {
                            let result = QueryResponse::Error(RCode::ServFail);
                            let write_lock_future = this.client.active_query_manager.write().boxed();

                            // Send out the answer to anyone waiting.
                            let _ = sender.send(result.clone());
                            kill_token.awake();

                            *this.inner = InnerNSRoundRobin::Cleanup { write_lock_future, result };
                            // Next loop will start the cleanup process
                            continue;
                        },
                        // Exit loop. Wait for one of the ns queries to wake us again.
                        Poll::Pending => return Poll::Pending,
                    }
                },
                InnerNSRoundRobin::Cleanup { write_lock_future, result } => {
                    match write_lock_future.as_mut().poll(cx) {
                        Poll::Ready(mut active_queries) => {
                            // Cleanup.
                            let _ = active_queries.remove(this.question.query());
                            drop(active_queries);

                            let result = result.clone();
                            *this.inner = InnerNSRoundRobin::Complete;

                            // Exit forever. Query complete.
                            return Poll::Ready(result);
                        },
                        // Exit loop. Wait for the write lock to wake us again.
                        Poll::Pending => return Poll::Pending,
                    }
                },
                InnerNSRoundRobin::Complete => {
                    panic!("NSRoundRobin query for '{}' was polled again after it already returned Poll::Ready", this.question.query());
                },
            }
        }
    }
}

#[pinned_drop]
impl<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h, 'i, 'j, 'k, 'l, CCache> PinnedDrop for NSRoundRobin<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h, 'i, 'j, 'k, 'l, CCache> where CCache: AsyncCache + Send + Sync + 'static {
    fn drop(mut self: Pin<&mut Self>) {
        let this = self.project();
        match this.inner {
            InnerNSRoundRobin::GetCachedNSAddresses { sender: _, kill_token, name_server_address_queries: _, name_server_non_cached_queries: _, name_server_cached_queries: _ }
          | InnerNSRoundRobin::QueryNameServers { sender: _, kill_token, ns_query_select: _ } => {
                kill_token.awake();

                let question = this.question.clone();
                let client = this.client.clone();
                tokio::spawn(async move {
                    let mut write_locked_active_query_manager = client.active_query_manager.write().await;
                    let _ = write_locked_active_query_manager.remove(&question.query());
                    drop(write_locked_active_query_manager);
                });
            },
            InnerNSRoundRobin::Cleanup { write_lock_future: _, result: _ } => {
                // Unfortunately, I don't think we can pass `write_lock_future` to the spawned task
                // because we only have a reference to it. I would prefer if we could re-use that
                // existing future.
                let question = this.question.clone();
                let client = this.client.clone();
                tokio::spawn(async move {
                    let mut write_locked_active_query_manager = client.active_query_manager.write().await;
                    let _ = write_locked_active_query_manager.remove(&question.query());
                    drop(write_locked_active_query_manager);
                });
            },
            _ => (),
        }
    }
}

// client: &Arc<DNSAsyncClient>, joined_cache: &Arc<CCache>, question: &Question, name_servers: &[CDomainName]

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
pub async fn query_name_servers<CCache>(client: &Arc<DNSAsyncClient>, joined_cache: &Arc<CCache>, question: Arc<Context>, name_servers: &[CDomainName]) -> QueryResponse<ResourceRecord> where CCache: AsyncCache + Send + Sync + 'static {
    println!("Querying Name Servers for '{}'", question.query());
    NSRoundRobin::new(client, joined_cache, &question, name_servers).await
}

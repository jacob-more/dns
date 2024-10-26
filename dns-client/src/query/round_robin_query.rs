use std::{borrow::BorrowMut, future::Future, io, net::IpAddr, pin::Pin, sync::Arc, task::Poll, time::Duration};

use dns_lib::{interface::{cache::cache::AsyncCache, client::Context}, query::{message::Message, qr::QR}, resource_record::{rcode::RCode, resource_record::ResourceRecord, rtype::RType}, types::c_domain_name::CDomainName};
use futures::{future::BoxFuture, FutureExt};
use log::{debug, info, trace};
use pin_project::{pin_project, pinned_drop};

use crate::{query::recursive_query::recursive_query, DNSAsyncClient};

use super::{network_query::query_network, recursive_query::{query_cache, QueryResponse}};


async fn query_cache_for_ns_addresses<'a, 'b, CCache>(ns_domain: CDomainName, address_rtype: RType, context: Arc<Context>, client: Arc<DNSAsyncClient>, joined_cache: Arc<CCache>) -> NSQuery<'a, 'b, CCache> where CCache: AsyncCache + Send + Sync {
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
        query: Option<BoxFuture<'b, io::Result<Message>>>,
        remaining_ns_addresses: Vec<IpAddr>,
    },
    OutOfAddresses,
}

#[derive(Debug)]
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

        async fn query_network_owned_args<CCache>(client: Arc<DNSAsyncClient>, joined_cache: Arc<CCache>, context: Arc<Context>, name_server_address: IpAddr) -> io::Result<Message> where CCache: AsyncCache + Send + Sync {
            query_network(&client, joined_cache, context.query(), &name_server_address).await
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
                            let context = self.context.as_ref();
                            trace!(context:?; "NSQuery::CacheMiss -> NSQuery::QueryingNetworkNSAddresses: querying for new ns addresses with context '{ns_address_context:?}'");
                            self.state = NSQueryState::QueryingNetworkNSAddresses { ns_addresses_query: recursive_query_owned_args(client, cache, ns_address_context).boxed() };
                            // Next loop will poll the query for NS addresses
                            continue;
                        },
                        Err(error) => {
                            self.state = NSQueryState::OutOfAddresses;
                            let context = self.context.as_ref();
                            debug!(context:?; "NSQuery::CacheMiss -> NSQuery::OutOfAddresses: new ns address error: {error}");
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
                                    let context = this.context.as_ref();
                                    trace!(context:?; "NSQuery::QueryingNetworkNSAddresses -> NSQuery::QueryingNetwork: querying ns address {first_ns_address}");
                                    let client = this.client.clone();
                                    let cache = this.joined_cache.clone();
                                    let context = this.context.clone();
                                    let query = query_network_owned_args(client, cache, context, first_ns_address).boxed();
                                    self.state = NSQueryState::QueryingNetwork { query: Some(query), remaining_ns_addresses: ns_addresses };
                                    // Next loop will poll the query for the question.
                                    continue;
                                },
                                None => {
                                    self.state = NSQueryState::OutOfAddresses;
                                    let context = &self.context;
                                    trace!(context:?; "NSQuery::QueryingNetworkNSAddresses -> NSQuery::OutOfAddresses: tried to query first ns address but out of addresses");
                                    // Exit loop. There are no addresses to query.
                                    return Poll::Ready(NSQueryResult::OutOfAddresses);
                                },
                            }
                        }
                        Poll::Ready(QueryResponse::NoRecords) => {
                            self.state = NSQueryState::OutOfAddresses;
                            let context = self.context.as_ref();
                            trace!(context:?; "NSQuery::QueryingNetworkNSAddresses -> NSQuery::OutOfAddresses: received response QueryResponse::NoRecords when querying network for ns addresses");
                            // Exit loop. There are no addresses to query.
                            return Poll::Ready(NSQueryResult::OutOfAddresses);
                        },
                        Poll::Ready(QueryResponse::Error(rcode)) => {
                            self.state = NSQueryState::OutOfAddresses;
                            let context = self.context.as_ref();
                            trace!(context:?; "NSQuery::QueryingNetworkNSAddresses -> NSQuery::OutOfAddresses: received response QueryResponse::Error({rcode}) when querying network for ns addresses");
                            // Exit loop. The was an error trying to query for
                            // the addresses.
                            return Poll::Ready(NSQueryResult::NSAddressQueryErr(rcode));
                        },
                        Poll::Pending => {
                            let context = self.context.as_ref();
                            trace!(context:?; "NSQuery::QueryingNetworkNSAddresses: waiting for network query response for ns addresses");
                            // Exit loop. Will be woken up by the ns address query.
                            return Poll::Pending
                        },
                    }
                },
                NSQueryState::CacheHit { ns_addresses } => {
                    match ns_addresses.pop() {
                        Some(first_ns_address) => {
                            let context = this.context.as_ref();
                            trace!(context:?; "NSQuery::CacheHit -> NSQuery::QueryingNetwork: querying ns address {first_ns_address}");
                            let client = this.client.clone();
                            let cache = this.joined_cache.clone();
                            let context = this.context.clone();
                            let remaining_ns_addresses = ns_addresses.drain(..).collect();
                            let query = query_network_owned_args(client, cache, context, first_ns_address).boxed();
                            self.state = NSQueryState::QueryingNetwork { query: Some(query), remaining_ns_addresses };
                            // Next loop will poll the query for the question.
                            continue;
                        },
                        None => {
                            self.state = NSQueryState::OutOfAddresses;
                            let context = self.context.as_ref();
                            trace!(context:?; "NSQuery::CacheHit -> NSQuery::OutOfAddresses: tried to query first ns address but out of addresses");
                            // Exit loop. There are no addresses to query.
                            return Poll::Ready(NSQueryResult::OutOfAddresses);
                        },
                    }
                },
                NSQueryState::QueryingNetwork { query: optional_query, remaining_ns_addresses } => {
                    match optional_query {
                        Some(query) => {
                            match query.as_mut().poll(cx) {
                                Poll::Ready(result) => {
                                    if remaining_ns_addresses.is_empty() {
                                        self.state = NSQueryState::OutOfAddresses;
                                        let context = self.context.as_ref();
                                        trace!(context:?; "NSQuery::QueryingNetwork -> NSQuery::OutOfAddresses: found result '{result:?}'");
                                        // Exit loop. A result was found.
                                        return Poll::Ready(NSQueryResult::QueryResult(result));
                                    } else {
                                        let context = this.context.as_ref();
                                        trace!(context:?; "NSQuery::QueryingNetwork: found result '{result:?}'");
                                        // Clear the query. If this object is
                                        // polled again, a new one will be set up
                                        // at that time.
                                        *optional_query = None;
                                        // Exit loop. A result was found.
                                        return Poll::Ready(NSQueryResult::QueryResult(result));
                                    }
                                },
                                Poll::Pending => {
                                    let context = self.context.as_ref();
                                    trace!(context:?; "NSQuery::QueryingNetwork: waiting for network query response for ns addresses");
                                    // Exit loop. Will be woken up by the query.
                                    return Poll::Pending
                                },
                            }
                        },
                        None => {
                            match remaining_ns_addresses.pop() {
                                Some(next_ns_address) => {
                                    let context = this.context.as_ref();
                                    trace!(context:?; "NSQuery::QueryingNetwork: setting up query to next ns {next_ns_address}");
                                    let client = this.client.clone();
                                    let cache = this.joined_cache.clone();
                                    let context = this.context.clone();
                                    let query = query_network_owned_args(client, cache, context, next_ns_address).boxed();
                                    *optional_query = Some(query);
                                    // Next loop will poll the query for the question.
                                    continue;
                                },
                                None => {
                                    let context = self.context.as_ref();
                                    trace!(context:?; "NSQuery::QueryingNetwork -> NSQuery::OutOfAddresses: tried to query next ns address but out of addresses");
                                    return Poll::Ready(NSQueryResult::OutOfAddresses)
                                },
                            }
                        },
                    }
                },
                // Exit loop. All addresses have been queried.
                NSQueryState::OutOfAddresses => {
                    let context = self.context.as_ref();
                    trace!(context:?; "NSQuery::OutOfAddresses");
                    return Poll::Ready(NSQueryResult::OutOfAddresses)
                },
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
struct NSRoundRobin<'a, 'b, 'c, 'd, 'e, 'f, 'g, CCache>
where
    CCache: AsyncCache + Send + Sync + 'static,
    'f: 'e,
    'g: 'e,
{
    client: &'a Arc<DNSAsyncClient>,
    joined_cache: &'b Arc<CCache>,
    context: &'c Arc<Context>,
    inner: InnerNSRoundRobin<'d, 'e, 'f, 'g, CCache>,
}

enum InnerNSRoundRobin<'a, 'b, 'c, 'd, CCache>
where
    CCache: AsyncCache + Send + Sync + 'static,
    'c: 'b,
    'd: 'b,
{
    Fresh {
        name_servers: &'a [CDomainName],
    },
    GetCachedNSAddresses {
        name_server_address_queries: Vec<BoxFuture<'b, NSQuery<'c, 'd, CCache>>>,
        name_server_non_cached_queries: Vec<Pin<Box<NSQuery<'c, 'd, CCache>>>>,
        name_server_cached_queries: Vec<Pin<Box<NSQuery<'c, 'd, CCache>>>>,
    },
    QueryNameServers {
        ns_query_select: Pin<Box<NSSelectQuery<NSQuery<'c, 'd, CCache>>>>,
    },
    Complete,
}

impl<'a, 'b, 'c, 'd, 'e, 'f, 'g, CCache> NSRoundRobin<'a, 'b, 'c, 'd, 'e, 'f, 'g, CCache> where CCache: AsyncCache + Send + Sync + 'static {
    fn new(client: &'a Arc<DNSAsyncClient>, joined_cache: &'b Arc<CCache>, question: &'c Arc<Context>, name_servers: &'d [CDomainName]) -> Self {
        Self { client, joined_cache, context: question, inner: InnerNSRoundRobin::Fresh { name_servers } }
    }
}

impl<'a, 'b, 'c, 'd, 'e, 'f, 'g, CCache> Future for NSRoundRobin<'a, 'b, 'c, 'd, 'e, 'f, 'g, CCache> where CCache: AsyncCache + Send + Sync + 'static {
    type Output = QueryResponse<ResourceRecord>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        loop {
            let this = self.as_mut().project();
            match this.inner.borrow_mut() {
                InnerNSRoundRobin::Fresh { name_servers } => {
                    let name_server_address_queries = name_servers.iter()
                        .flat_map(|ns_domain| [
                            query_cache_for_ns_addresses(ns_domain.clone(), RType::A, this.context.clone(), this.client.clone(), this.joined_cache.clone()).boxed(),
                            query_cache_for_ns_addresses(ns_domain.clone(), RType::AAAA, this.context.clone(), this.client.clone(), this.joined_cache.clone()).boxed(),
                        ])
                        .collect::<Vec<_>>();
                    let capacity = name_server_address_queries.len();

                    *this.inner = InnerNSRoundRobin::GetCachedNSAddresses { name_server_address_queries, name_server_cached_queries: Vec::with_capacity(capacity), name_server_non_cached_queries: Vec::with_capacity(capacity) };

                    let context = self.context.as_ref();
                    trace!(context:?; "NSRoundRobin::Fresh -> NSRoundRobin::GetCachedNSAddresses: Getting cached ns addresses");

                    // Next loop will poll all the NS address queries
                    continue;
                },
                InnerNSRoundRobin::GetCachedNSAddresses { name_server_address_queries, name_server_non_cached_queries, name_server_cached_queries } => {
                    name_server_address_queries.retain_mut(|ns_address_query| {
                        match ns_address_query.as_mut().poll(cx) {
                            Poll::Ready(ns_query @ NSQuery { ns_domain: _, ns_address_rtype: _, context: _, client: _, joined_cache: _, state: NSQueryState::CacheHit { ns_addresses: _ } }) => {
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
                        let context = this.context.as_ref();
                        trace!(context:?; "NSRoundRobin::GetCachedNSAddresses -> NSRoundRobin::QueryNameServers: Received all cache responses. {} queries are cached. {} queries are non-cached", name_server_non_cached_queries.len(), name_server_cached_queries.len());
                        // Join the two lists of queries. The queries that don't have cached
                        // addresses are at the front and the ones with cached addresses are at the
                        // back. This list will be read like a stack, so the cached queries will be
                        // run first.
                        let mut ns_queries = Vec::with_capacity(name_server_non_cached_queries.len() + name_server_cached_queries.len());
                        ns_queries.extend(name_server_non_cached_queries.drain(..));
                        ns_queries.extend(name_server_cached_queries.drain(..));
                        let ns_query_select = Box::pin(NSSelectQuery::new(ns_queries, 3, Duration::from_millis(200)));

                        *this.inner = InnerNSRoundRobin::QueryNameServers { ns_query_select };

                        // Next loop will select the first query from the list and start it
                        continue;
                    } else {
                        let context = this.context.as_ref();
                        trace!(context:?; "NSRoundRobin::GetCachedNSAddresses: Waiting for cache responses for {} queries. {} queries are cached. {} queries are non-cached", name_server_address_queries.len(), name_server_non_cached_queries.len(), name_server_cached_queries.len());

                        // Exit loop. Wait for one of the address queries to wake us again.
                        return Poll::Pending;
                    }
                },
                InnerNSRoundRobin::QueryNameServers { ns_query_select } => {
                    match ns_query_select.as_mut().poll(cx) {
                        // No error. Valid response.
                        Poll::Ready(Some(NSQueryResult::QueryResult(Ok(response @ Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NoError, question: _, answer: _, authority: _, additional: _ }))))
                        // If a server does not support a query type, we can probably assume it is not in that zone.
                        // TODO: verify that this is a valid assumption. Should we return NotImpl?
                      | Poll::Ready(Some(NSQueryResult::QueryResult(Ok(response @ Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NotImp, question: _, answer: _, authority: _, additional: _ })))) => {
                            let result = query_response(response);

                            let context = this.context.as_ref();
                            trace!(context:?; "NSRoundRobin::QueryNameServers -> NSRoundRobin::Complete: Received result {result:?}");

                            *this.inner = InnerNSRoundRobin::Complete;

                            // Exit forever. Query complete.
                            return Poll::Ready(result);
                        },
                        // Only authoritative servers can indicate that a name does not exist.
                        Poll::Ready(Some(NSQueryResult::QueryResult(Ok(response @ Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: true, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NXDomain, question: _, answer: _, authority: _, additional: _ })))) => {
                            let result = QueryResponse::Error(RCode::NXDomain);

                            let context = this.context.as_ref();
                            trace!(context:?; "NSRoundRobin::QueryNameServers -> NSRoundRobin::Cleanup: Received error NXDomain in message '{response:?}'");

                            *this.inner = InnerNSRoundRobin::Complete;

                            // Exit forever. Query complete.
                            return Poll::Ready(result);
                        },
                        // This server does not have the authority to say that the name
                        // does not exist. Ask others.
                        Poll::Ready(Some(response @ NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: false, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NXDomain, question: _, answer: _, authority: _, additional: _ }))))
                        // If there is an IO error, try a different server.
                      | Poll::Ready(Some(response @ NSQueryResult::QueryResult(Err(_))))
                        // If a particular name server cannot be queried anymore, then keep
                        // trying to query the others.
                      | Poll::Ready(Some(response @ NSQueryResult::OutOfAddresses))
                        // If there was an error looking up one of the name servers, keep
                        // trying to look up the others.
                      | Poll::Ready(Some(response @ NSQueryResult::NSAddressQueryErr(_))) => {
                            let context = this.context.as_ref();
                            trace!(context:?; "NSRoundRobin::QueryNameServers: Received error in message '{response:?}'");

                            // Next loop will poll the other name servers.
                            continue;
                        },
                        // If a name server cannot interpret what we are sending it, asking other name servers probably will not help.
                        // Treat as a hard error.
                        Poll::Ready(response @ Some(NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::FormErr, question: _, answer: _, authority: _, additional: _ }))))
                        // If a name server refuses to perform an operation, we should not keep asking the other servers.
                        // TODO: verify that this is a valid way of handling.
                      | Poll::Ready(response @ Some(NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::Refused, question: _, answer: _, authority: _, additional: _ }))))
                        // We don't know how to handle unknown errors.
                        // Assume they are a fatal failure.
                      | Poll::Ready(response @ Some(NSQueryResult::QueryResult(Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: _, question: _, answer: _, authority: _, additional: _ }))))
                        // Malformed response.
                      | Poll::Ready(response @ Some(NSQueryResult::QueryResult(Ok(Message { id: _, qr: _, opcode: _, authoritative_answer: _, truncation: _, recursion_desired: _, recursion_available: _, z: _, rcode: _, question: _, answer: _, authority: _, additional: _ }))))
                        // No more servers to query.
                      | Poll::Ready(response @ None) => {
                            let result = QueryResponse::Error(RCode::ServFail);

                            *this.inner = InnerNSRoundRobin::Complete;

                            let context = this.context.as_ref();
                            trace!(context:?; "NSRoundRobin::QueryNameServers -> NSRoundRobin::Complete: Result is ServFail. Received response '{response:?}'");

                            // Exit forever. Query complete.
                            return Poll::Ready(result);
                        },
                        // Exit loop. Wait for one of the ns queries to wake us again.
                        Poll::Pending => return Poll::Pending,
                    }
                },
                InnerNSRoundRobin::Complete => {
                    panic!("InnerNSRoundRobin::Complete: query for '{}' was polled again after it already returned Poll::Ready", this.context.query());
                },
            }
        }
    }
}

#[pinned_drop]
impl<'a, 'b, 'c, 'd, 'e, 'f, 'g, CCache> PinnedDrop for NSRoundRobin<'a, 'b, 'c, 'd, 'e, 'f, 'g, CCache> where CCache: AsyncCache + Send + Sync + 'static {
    fn drop(mut self: Pin<&mut Self>) {
        let this = self.project();
        match this.inner {
            InnerNSRoundRobin::Fresh { name_servers: _ } => (),
            InnerNSRoundRobin::GetCachedNSAddresses { name_server_address_queries: _, name_server_non_cached_queries: _, name_server_cached_queries: _ } => {
                let context = this.context.as_ref();
                trace!(context:?; "InnerNSRoundRobin::GetCachedNSAddresses -> NSRoundRobin::(drop): Cleaning up query {}", this.context.query());
            },
            InnerNSRoundRobin::QueryNameServers { ns_query_select: _ } => {
                let context = this.context.as_ref();
                trace!(context:?; "InnerNSRoundRobin::QueryNameServers -> NSRoundRobin::(drop): Cleaning up query {}", this.context.query());
            },
            InnerNSRoundRobin::Complete => (),
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
pub async fn query_name_servers<CCache>(client: &Arc<DNSAsyncClient>, joined_cache: &Arc<CCache>, context: Arc<Context>, name_servers: &[CDomainName]) -> QueryResponse<ResourceRecord> where CCache: AsyncCache + Send + Sync + 'static {
    info!(context:?; "Querying Name Servers for '{}'", context.query());
    NSRoundRobin::new(client, joined_cache, &context, name_servers).await
}

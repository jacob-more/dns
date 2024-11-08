use std::{borrow::BorrowMut, cmp::Reverse, collections::HashMap, future::Future, io, net::{IpAddr, SocketAddr}, pin::Pin, sync::Arc, task::Poll, time::Duration};

use dns_lib::{interface::{cache::cache::AsyncCache, client::Context}, query::{message::Message, qr::QR}, resource_record::{rcode::RCode, resource_record::ResourceRecord, rtype::RType}, types::c_domain_name::CDomainName};
use futures::{future::BoxFuture, FutureExt};
use log::{debug, info, trace};
use network::mixed_tcp_udp::{MixedSocket, errors::QueryError};
use pin_project::{pin_project, pinned_drop};

use crate::{query::recursive_query::recursive_query, DNSAsyncClient};

use super::{network_query::query_network, recursive_query::{query_cache, QueryResponse}};


async fn query_cache_for_ns_addresses<'a, 'b, 'c, CCache>(ns_domain: CDomainName, address_rtype: RType, context: Arc<Context>, client: Arc<DNSAsyncClient>, joined_cache: Arc<CCache>) -> NSQuery<'a, 'b, 'c, CCache> where CCache: AsyncCache + Send + Sync {
    let ns_question = context.query().with_new_qname_qtype(ns_domain.clone(), address_rtype.clone());

    fn rr_to_ip(record: ResourceRecord) -> Option<IpAddr> {
        match record {
            ResourceRecord::A(_, rdata) => Some(IpAddr::V4(*rdata.ipv4_addr())),
            ResourceRecord::AAAA(_, rdata) => Some(IpAddr::V6(*rdata.ipv6_addr())),
            _ => None,
        }
    }

    let ns_addresses;
    let cache_response;
    match query_cache(&joined_cache, &ns_question).await {
        QueryResponse::Records(records) => {
            ns_addresses = records.into_iter()
                .filter_map(|record| rr_to_ip(record))
                .collect();
            cache_response = NSQueryCacheResponse::Hit;
        },
        _ => {
            ns_addresses = vec![];
            cache_response = NSQueryCacheResponse::Miss;

        },
    };

    NSQuery {
        ns_domain,
        ns_address_rtype: address_rtype,
        context,

        client,
        joined_cache,

        ns_addresses,
        sockets: HashMap::new(),
        state: InnerNSQuery::Fresh(cache_response),
    }
}

#[derive(Debug)]
enum NSQueryResult {
    OutOfAddresses,
    NSAddressQueryErr(RCode),
    QueryResult(Result<Message, QueryError>),
}

#[pin_project]
struct NSQuery<'a, 'b, 'c, CCache> where CCache: AsyncCache + Send + Sync {
    ns_domain: CDomainName,
    ns_address_rtype: RType,
    context: Arc<Context>,

    client: Arc<DNSAsyncClient>,
    joined_cache: Arc<CCache>,

    ns_addresses: Vec<IpAddr>,
    sockets: HashMap<IpAddr, Arc<MixedSocket>>,
    state: InnerNSQuery<'a, 'b, 'c>,
}

enum InnerNSQuery<'a, 'b, 'c> {
    Fresh(NSQueryCacheResponse),
    QueryingNetworkNSAddresses {
        ns_addresses_query: BoxFuture<'a, QueryResponse<ResourceRecord>>,
    },
    GettingSocketStats(BoxFuture<'b, Vec<Arc<MixedSocket>>>),
    NetworkQueryStart,
    QueryingNetwork(BoxFuture<'c, Result<Message, QueryError>>),
    OutOfAddresses,
}

enum NSQueryCacheResponse {
    Hit,
    Miss,
}

impl<'a, 'b, 'c, CCache> NSQuery<'a, 'b, 'c, CCache> where CCache: AsyncCache + Send + Sync {
    pub fn best_address_stats(&self) -> Option<(u32, u32)> {
        self.ns_addresses.iter().map(|address| self.sockets.get(address)
                .map(|socket| (socket.average_dropped_udp_packets(), socket.average_udp_response_time()))
                .filter(|(average_dropped_udp_packets, average_udp_response_time)| (average_dropped_udp_packets.is_finite() && average_udp_response_time.is_finite()))
                .map(|(average_dropped_udp_packets, average_udp_response_time)| Reverse(((average_dropped_udp_packets * 100.0).ceil() as u32, average_udp_response_time.ceil() as u32))))
                .max()
                .flatten()
                .map(|val| val.0)
    }
}

fn take_best_address<'a, 'b, 'c, CCache>(ns_addresses: &mut Vec<IpAddr>, sockets: &HashMap<IpAddr, Arc<MixedSocket>>) -> Option<IpAddr> where CCache: AsyncCache + Send + Sync {
    match ns_addresses.iter()
        .enumerate()
        .max_by_key(|(_, address)| sockets.get(address)
            .map(|socket| (socket.average_dropped_udp_packets(), socket.average_udp_response_time()))
            .filter(|(average_dropped_udp_packets, average_udp_response_time)| (average_dropped_udp_packets.is_finite() && average_udp_response_time.is_finite()))
            .map(|(average_dropped_udp_packets, average_udp_response_time)| Reverse(((average_dropped_udp_packets * 100.0).ceil() as u32, average_udp_response_time.ceil() as u32))))
    {
        Some((index, _)) => Some(ns_addresses.swap_remove(index)),
        None => ns_addresses.pop(),
    }
}

impl<'a, 'b, 'c, CCache> Future for NSQuery<'a, 'b, 'c, CCache> where CCache: AsyncCache + Send + Sync + 'static {
    type Output = NSQueryResult;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        async fn recursive_query_owned_args<CCache>(client: Arc<DNSAsyncClient>, joined_cache: Arc<CCache>, context: Context) -> QueryResponse<ResourceRecord> where CCache: AsyncCache + Send + Sync + 'static {
            recursive_query(client, joined_cache, context).await
        }

        async fn query_network_owned_args<CCache>(client: Arc<DNSAsyncClient>, joined_cache: Arc<CCache>, context: Arc<Context>, name_server_address: IpAddr) -> Result<Message, QueryError> where CCache: AsyncCache + Send + Sync {
            query_network(&client, joined_cache, context.query(), &name_server_address).await
        }

        async fn query_for_sockets<CCache>(client: Arc<DNSAsyncClient>, sockets: Vec<SocketAddr>) -> Vec<Arc<MixedSocket>> where CCache: AsyncCache + Send {
            client.socket_manager.try_get_all(sockets.iter()).await
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
                InnerNSQuery::Fresh(NSQueryCacheResponse::Hit) => {
                    let sockets_addresses = this.ns_addresses.iter()
                        .map(|address| SocketAddr::new(*address, 53))
                        .collect::<Vec<_>>();
                    let client = this.client.clone();
                    let context = &self.context;
                    trace!(context:?; "NSQuery::QueryingNetworkNSAddresses -> NSQuery::GettingSocketStats");

                    self.state = InnerNSQuery::GettingSocketStats(query_for_sockets::<CCache>(client, sockets_addresses).boxed());

                    // TODO
                    continue;
                },
                InnerNSQuery::Fresh(NSQueryCacheResponse::Miss) => {
                    let client = self.client.clone();
                    let cache = self.joined_cache.clone();
                    match self.context.clone().new_ns_address(self.context.query().with_new_qname_qtype(self.ns_domain.clone(), self.ns_address_rtype)) {
                        Ok(ns_address_context) => {
                            let context = self.context.as_ref();
                            trace!(context:?; "NSQuery::Fresh(Miss) -> NSQuery::QueryingNetworkNSAddresses: querying for new ns addresses with context '{ns_address_context:?}'");

                            self.state = InnerNSQuery::QueryingNetworkNSAddresses { ns_addresses_query: recursive_query_owned_args(client, cache, ns_address_context).boxed() };

                            // Next loop will poll the query for NS addresses
                            continue;
                        },
                        Err(error) => {
                            let context = self.context.as_ref();
                            debug!(context:?; "NSQuery::Fresh(Miss) -> NSQuery::OutOfAddresses: new ns address error: {error}");

                            self.state = InnerNSQuery::OutOfAddresses;

                            // Exit loop. The was an error trying to query for the addresses.
                            return Poll::Ready(NSQueryResult::NSAddressQueryErr(RCode::ServFail));
                        },
                    };
                },
                InnerNSQuery::QueryingNetworkNSAddresses { ns_addresses_query } => {
                    match ns_addresses_query.as_mut().poll(cx) {
                        Poll::Ready(QueryResponse::Records(records)) => {
                            this.ns_addresses.extend(records.into_iter().filter_map(|record| rr_to_ip(record)));
                            if this.ns_addresses.is_empty() {
                                let context = &self.context;
                                trace!(context:?; "NSQuery::QueryingNetworkNSAddresses -> NSQuery::OutOfAddresses: tried to query first ns address but out of addresses");

                                self.state = InnerNSQuery::OutOfAddresses;

                                // Exit loop. There are no addresses to query.
                                return Poll::Ready(NSQueryResult::OutOfAddresses);
                            } else {
                                let sockets_addresses = this.ns_addresses.iter()
                                    .map(|address| SocketAddr::new(*address, 53))
                                    .collect::<Vec<_>>();
                                let client = this.client.clone();
                                let context = &self.context;
                                trace!(context:?; "NSQuery::QueryingNetworkNSAddresses -> NSQuery::GettingSocketStats");
            
                                self.state = InnerNSQuery::GettingSocketStats(query_for_sockets::<CCache>(client, sockets_addresses).boxed());

                                // TODO
                                continue;
                            }
                        },
                        Poll::Ready(QueryResponse::NoRecords) => {
                            let context = self.context.as_ref();
                            trace!(context:?; "NSQuery::QueryingNetworkNSAddresses -> NSQuery::OutOfAddresses: received response QueryResponse::NoRecords when querying network for ns addresses");

                            self.state = InnerNSQuery::OutOfAddresses;

                            // Exit loop. There are no addresses to query.
                            return Poll::Ready(NSQueryResult::OutOfAddresses);
                        },
                        Poll::Ready(QueryResponse::Error(rcode)) => {
                            let context = self.context.as_ref();
                            trace!(context:?; "NSQuery::QueryingNetworkNSAddresses -> NSQuery::OutOfAddresses: received response QueryResponse::Error({rcode}) when querying network for ns addresses");

                            self.state = InnerNSQuery::OutOfAddresses;

                            // Exit loop. The was an error trying to query for the addresses.
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
                InnerNSQuery::GettingSocketStats(sockets_future) => {
                    match sockets_future.as_mut().poll(cx) {
                        Poll::Ready(sockets) => {
                            self.sockets.extend(sockets.into_iter().map(|socket| (socket.socket_address().ip(), socket)));
                            let context = self.context.as_ref();
                            trace!(context:?; "NSQuery::GettingSocketStats -> InnerNSQuery::NetworkQueryStart: getting sockets to determine the fastest addresses");

                            self.state = InnerNSQuery::NetworkQueryStart;

                            // TODO
                            continue;
                        },
                        Poll::Pending => {
                            let context = self.context.as_ref();
                            trace!(context:?; "NSQuery::GettingSocketStats: getting sockets to determine the fastest addresses");

                            // Exit loop. Will be woken up by the ns address query.
                            return Poll::Pending
                        },
                    }
                },
                InnerNSQuery::NetworkQueryStart => {
                    match take_best_address::<CCache>(this.ns_addresses, &this.sockets) {
                        Some(next_ns_address) => {
                            let context = this.context.as_ref();
                            trace!(context:?; "NSQuery::NetworkQueryStart -> NSQuery::QueryingNetwork: setting up query to next ns {next_ns_address}");

                            let client = this.client.clone();
                            let cache = this.joined_cache.clone();
                            let context = this.context.clone();
                            let query = query_network_owned_args(client, cache, context, next_ns_address).boxed();

                            self.state = InnerNSQuery::QueryingNetwork(query);

                            // Next loop will poll the query for the question.
                            continue;
                        },
                        None => {
                            let context = self.context.as_ref();
                            trace!(context:?; "NSQuery::NetworkQueryStart -> NSQuery::OutOfAddresses: tried to query next ns address but out of addresses");

                            return Poll::Ready(NSQueryResult::OutOfAddresses)
                        },
                    }
                },
                InnerNSQuery::QueryingNetwork(query) => {
                    match query.as_mut().poll(cx) {
                        Poll::Ready(result) => {
                            let context = this.context.as_ref();
                            trace!(context:?; "NSQuery::QueryingNetwork -> NSQuery::NetworkQueryStart: found result '{result:?}'");

                            // Clear the query. If this object is polled again, a new one will be
                            // set up at that time.
                            self.state = InnerNSQuery::NetworkQueryStart;

                            // Exit loop. A result was found.
                            return Poll::Ready(NSQueryResult::QueryResult(result));
                        },
                        Poll::Pending => {
                            let context = self.context.as_ref();
                            trace!(context:?; "NSQuery::QueryingNetwork: waiting for network query response for ns addresses");

                            // Exit loop. Will be woken up by the query.
                            return Poll::Pending
                        },
                    }
                },
                InnerNSQuery::OutOfAddresses => {
                    let context = self.context.as_ref();
                    trace!(context:?; "NSQuery::OutOfAddresses");

                    // Exit loop. All addresses have been queried.
                    return Poll::Ready(NSQueryResult::OutOfAddresses)
                },
            }
        }
    }
}

#[pin_project]
struct NSSelectQuery<'a, 'b, 'c, CCache> where CCache: AsyncCache + Send + Sync {
    // Note: the queries are read in reverse order (like a stack).
    ns_queries: Vec<Pin<Box<NSQuery<'a, 'b, 'c, CCache>>>>,
    running: Vec<Pin<Box<NSQuery<'a, 'b, 'c, CCache>>>>,
    max_concurrency: usize,
    add_query_timeout: Duration,
    #[pin]
    add_query_timer: Option<tokio::time::Sleep>,
}

impl<'a, 'b, 'c, CCache> NSSelectQuery<'a, 'b, 'c, CCache> where CCache: AsyncCache + Send + Sync {
    pub fn new(ns_queries: Vec<Pin<Box<NSQuery<'a, 'b, 'c, CCache>>>>, max_concurrency: usize, add_query_timeout: Duration) -> Self {
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

fn take_best_ns_query<'a, 'b, 'c, CCache>(ns_queries: &mut Vec<Pin<Box<NSQuery<'a, 'b, 'c, CCache>>>>) -> Option<Pin<Box<NSQuery<'a, 'b, 'c, CCache>>>> where CCache: AsyncCache + Send + Sync {
    match ns_queries.iter()
        .enumerate()
        .max_by_key(|(_, ns_query)| ns_query.best_address_stats().map(|stats| Reverse(stats)))
    {
        Some((index, _)) => Some(ns_queries.swap_remove(index)),
        None => ns_queries.pop(),
    }
}

impl<'a, 'b, 'c, CCache> Future for NSSelectQuery<'a, 'b, 'c, CCache> where CCache: AsyncCache + Send + Sync + 'static {
    type Output = Option<NSQueryResult>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        if self.is_first_poll() {
            let mut this = self.as_mut().project();
            // Initialize the `running` queue with its first query.
            match take_best_ns_query(this.ns_queries) {
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
                // The expected case is that a fresh NSSelectQuery has not yet had the
                // `add_query_timer` initialized. We should do that here.
                (None, Some(deadline)) => this.add_query_timer.set(Some(tokio::time::sleep_until(deadline))),
                // If a time was created manually, we should refresh the deadline since it may have
                // become stale if this task has been waiting to be run for a while.
                (Some(timer), Some(deadline)) => timer.reset(deadline),
                // If the timer cannot be reset, then this will only run 1 task at a time since it
                // is unable to schedule them to start later. This should never really be an issue,
                // but if it is, we could have the system schedule `max_concurrency` many tasks
                // immediately. However, I would be concerned that this may accidentally overwhelm
                // ourself or the endpoints. Presumably, if this fails, then this will probably fail
                // for all other queries in the system. If we had all of them run `max_concurrency`
                // many concurrent tasks, the system might DOS itself.
                (_, None) => this.add_query_timer.set(None),
            }
        }

        let mut this = self.as_mut().project();
        if let Some(mut timer) = this.add_query_timer.as_mut().as_pin_mut() {
            if let Poll::Ready(()) = timer.as_mut().poll(cx) {
                match take_best_ns_query(this.ns_queries) {
                    Some(ns_query) => {
                        this.running.push(ns_query);
                        // Keep setting the timer until the maximum number of allowed concurrent
                        // queries have been started for this group. Then, we will maintain that
                        // many concurrent queries until we run out of queued queries.
                        if this.running.len() < *this.max_concurrency {
                            match tokio::time::Instant::now().checked_add(*this.add_query_timeout) {
                                Some(new_deadline) => timer.reset(new_deadline),
                                // If a deadline could not be calculated, the timer cannot be reset.
                                // This could limit the number of concurrent processes that can run
                                // below `max_concurrency`. I go into more detail on why this is the
                                // preferred option earlier in this function.
                                None => this.add_query_timer.set(None),
                            }
                        } else {
                            // Once we have the maximum number of tasks running concurrently, we
                            // don't need to wake up to add new tasks. New tasks from `ns_query`
                            // will only be moved to `running` when a space opens up in `running`.
                            this.add_query_timer.set(None);
                        }
                    },
                    None => {
                        // Don't want to be erroneously woken up if there is nobody else to add.
                        this.add_query_timer.set(None);
                    },
                }
            }
        }

        for (index, ns_query) in this.running.iter_mut().enumerate() {
            if let Poll::Ready(result) = ns_query.as_mut().poll(cx) {
                match (take_best_ns_query(this.ns_queries), &result) {
                    // We can re-use the spot in the `running` list for the new query since we don't
                    // care about the order of this list. They should all get polled eventually (as
                    // long as no result is found). Re-using the spot means the vector does not need
                    // to shift all the elements to the right of this index left just for us to
                    // append to the end.
                    (Some(new_ns_query), NSQueryResult::OutOfAddresses) => {
                        *ns_query = new_ns_query;
                    },
                    (Some(new_ns_query), _) => {
                        let ns_query = this.running.swap_remove(index);
                        this.running.push(new_ns_query);
                        this.ns_queries.push(ns_query);
                    },
                    (None, NSQueryResult::OutOfAddresses) => {
                        let _ = this.running.swap_remove(index);
                        // Don't want to be erroneously woken up if there is
                        // nobody else to add.
                        this.add_query_timer.set(None);
                    },
                    (None, _) => {
                        // Since we are down to the last ns_query, it can stay in the running queue
                        // until it runs out of addresses.

                        // Don't want to be erroneously woken up if there is
                        // nobody else to add.
                        this.add_query_timer.set(None);
                    },
                }

                return Poll::Ready(Some(result));
            }
        }

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
struct NSRoundRobin<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h, CCache>
where
    CCache: AsyncCache + Send + Sync + 'static,
    'f: 'e,
    'g: 'e,
    'h: 'e,
{
    client: &'a Arc<DNSAsyncClient>,
    joined_cache: &'b Arc<CCache>,
    context: &'c Arc<Context>,
    inner: InnerNSRoundRobin<'d, 'e, 'f, 'g, 'h, CCache>,
}

enum InnerNSRoundRobin<'a, 'b, 'c, 'd, 'e, CCache>
where
    CCache: AsyncCache + Send + Sync + 'static,
    'c: 'b,
    'd: 'b,
{
    Fresh {
        name_servers: &'a [CDomainName],
    },
    GetCachedNSAddresses {
        name_server_address_queries: Vec<BoxFuture<'b, NSQuery<'c, 'd, 'e, CCache>>>,
        name_server_non_cached_queries: Vec<Pin<Box<NSQuery<'c, 'd, 'e, CCache>>>>,
        name_server_cached_queries: Vec<Pin<Box<NSQuery<'c, 'd, 'e, CCache>>>>,
    },
    QueryNameServers {
        ns_query_select: Pin<Box<NSSelectQuery<'c, 'd, 'e, CCache>>>,
    },
    Complete,
}

impl<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h, CCache> NSRoundRobin<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h, CCache> where CCache: AsyncCache + Send + Sync + 'static {
    fn new(client: &'a Arc<DNSAsyncClient>, joined_cache: &'b Arc<CCache>, question: &'c Arc<Context>, name_servers: &'d [CDomainName]) -> Self {
        Self { client, joined_cache, context: question, inner: InnerNSRoundRobin::Fresh { name_servers } }
    }
}

impl<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h, CCache> Future for NSRoundRobin<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h, CCache> where CCache: AsyncCache + Send + Sync + 'static {
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
                            Poll::Ready(ns_query @ NSQuery { ns_domain: _, ns_address_rtype: _, context: _, client: _, joined_cache: _, ns_addresses: _, sockets: _, state: InnerNSQuery::Fresh(NSQueryCacheResponse::Hit) }) => {
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
impl<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h, CCache> PinnedDrop for NSRoundRobin<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h, CCache> where CCache: AsyncCache + Send + Sync + 'static {
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

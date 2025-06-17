use std::{
    cmp::Reverse,
    collections::{HashMap, hash_map::Entry},
    future::Future,
    net::IpAddr,
    pin::Pin,
    sync::Arc,
    task::Poll,
    time::Duration,
};

use async_lib::once_watch::{self, OnceWatchSend, OnceWatchSubscribe};
use dns_lib::{
    interface::{
        cache::{CacheQuery, CacheResponse, cache::AsyncCache},
        client::Context,
    },
    query::{message::Message, qr::QR},
    resource_record::{
        rcode::RCode,
        resource_record::{RecordData, ResourceRecord},
        rtype::RType,
    },
    types::c_domain_name::CDomainName,
};
use futures::{FutureExt, future::BoxFuture};
use log::{debug, info, trace};
use pin_project::{pin_project, pinned_drop};
use rand::{seq::IteratorRandom, thread_rng};

use crate::{
    DNSAsyncClient,
    network::{errors::QueryError, mixed_tcp_udp::MixedSocket},
    query::{network_query::query_network, recursive_query::recursive_query},
    result::{QError, QOk, QResult},
};

fn rr_to_ip(record: ResourceRecord) -> Option<IpAddr> {
    match record.into_rdata() {
        RecordData::A(rdata) => Some(rdata.into_ipv4_addr().into()),
        RecordData::AAAA(rdata) => Some(rdata.into_ipv6_addr().into()),
        _ => None,
    }
}

async fn query_cache_for_ns_addresses<'a, 'b, 'c, CCache>(
    ns_domain: CDomainName,
    address_rtype: RType,
    context: Arc<Context>,
    client: Arc<DNSAsyncClient>,
    joined_cache: Arc<CCache>,
) -> NSQuery<'a, 'b, 'c, CCache>
where
    CCache: AsyncCache + Send + Sync,
{
    let ns_question = context
        .query()
        .with_new_qname_qtype(ns_domain.clone(), address_rtype.clone());

    let ns_addresses;
    let cache_response;
    match joined_cache
        .get(&CacheQuery {
            authoritative: false,
            question: &ns_question,
        })
        .await
    {
        CacheResponse::Records(records) if !records.is_empty() => {
            ns_addresses = records
                .into_iter()
                .filter_map(|record| rr_to_ip(record.record))
                .collect();
            cache_response = NSQueryCacheResponse::Hit;
        }
        _ => {
            ns_addresses = vec![];
            cache_response = NSQueryCacheResponse::Miss;
        }
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
    Result(QResult<Message, QError>),
}

#[pin_project]
struct NSQuery<'a, 'b, 'c, CCache>
where
    CCache: AsyncCache + Send + Sync,
{
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
        ns_addresses_query: BoxFuture<'a, QResult>,
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

impl<'a, 'b, 'c, CCache> NSQuery<'a, 'b, 'c, CCache>
where
    CCache: AsyncCache + Send + Sync,
{
    pub fn best_address_stats(&self) -> Option<(u32, u32)> {
        self.ns_addresses
            .iter()
            .map(|address| {
                self.sockets
                    .get(address)
                    .map(|socket| {
                        (
                            socket.average_dropped_udp_packets(),
                            socket.average_udp_response_time(),
                        )
                    })
                    .filter(|(average_dropped_udp_packets, average_udp_response_time)| {
                        (average_dropped_udp_packets.is_finite()
                            && average_udp_response_time.is_finite())
                    })
                    // If more than 80% of UDP packets are being dropped, we'd rather explore new
                    // addresses. Otherwise, this address would still be technically better than one
                    // which had not yet been explored.
                    .filter(|(average_dropped_udp_packets, _)| *average_dropped_udp_packets < 0.80)
                    .map(|(average_dropped_udp_packets, average_udp_response_time)| {
                        Reverse((
                            (average_dropped_udp_packets * 100.0).ceil() as u32,
                            average_udp_response_time.ceil() as u32,
                        ))
                    })
            })
            .max()
            .flatten()
            .map(|val| val.0)
    }
}

fn take_random<T>(vec: &mut Vec<T>) -> Option<T> {
    let i = (0..vec.len()).choose(&mut thread_rng())?;
    Some(vec.swap_remove(i))
}

fn take_best_address<'a, 'b, 'c, CCache>(
    ns_addresses: &mut Vec<IpAddr>,
    sockets: &HashMap<IpAddr, Arc<MixedSocket>>,
) -> Option<IpAddr>
where
    CCache: AsyncCache + Send + Sync,
{
    match ns_addresses.iter().enumerate().max_by_key(|(_, address)| {
        sockets
            .get(address)
            .map(|socket| {
                (
                    socket.average_dropped_udp_packets(),
                    socket.average_udp_response_time(),
                )
            })
            .filter(|(average_dropped_udp_packets, average_udp_response_time)| {
                (average_dropped_udp_packets.is_finite() && average_udp_response_time.is_finite())
            })
            // If more than 80% of UDP packets are being dropped, we'd rather explore new
            // addresses. Otherwise, this address would still be technically better than one
            // which had not yet been explored.
            .filter(|(average_dropped_udp_packets, _)| *average_dropped_udp_packets < 0.80)
            .map(|(average_dropped_udp_packets, average_udp_response_time)| {
                Reverse((
                    (average_dropped_udp_packets * 100.0).ceil() as u32,
                    average_udp_response_time.ceil() as u32,
                ))
            })
    }) {
        Some((index, _)) => Some(ns_addresses.swap_remove(index)),
        None => take_random(ns_addresses),
    }
}

impl<'a, 'b, 'c, CCache> Future for NSQuery<'a, 'b, 'c, CCache>
where
    CCache: AsyncCache + Send + Sync + 'static,
{
    type Output = NSQueryResult;

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        async fn recursive_query_owned_args<CCache>(
            client: Arc<DNSAsyncClient>,
            joined_cache: Arc<CCache>,
            context: Context,
        ) -> QResult
        where
            CCache: AsyncCache + Send + Sync + 'static,
        {
            recursive_query(client, joined_cache, context).await
        }

        async fn query_network_owned_args<CCache>(
            client: Arc<DNSAsyncClient>,
            joined_cache: Arc<CCache>,
            context: Arc<Context>,
            name_server_address: IpAddr,
        ) -> Result<Message, QueryError>
        where
            CCache: AsyncCache + Send + Sync,
        {
            query_network(&client, joined_cache, context.query(), &name_server_address).await
        }

        async fn query_for_sockets<CCache>(
            client: Arc<DNSAsyncClient>,
            sockets: Vec<IpAddr>,
        ) -> Vec<Arc<MixedSocket>>
        where
            CCache: AsyncCache + Send,
        {
            client
                .socket_manager
                .try_get_all_udp_tcp(sockets.iter())
                .await
        }

        loop {
            let this = self.as_mut().project();
            match this.state {
                InnerNSQuery::Fresh(NSQueryCacheResponse::Hit) => {
                    let sockets_addresses = this
                        .ns_addresses
                        .iter()
                        .map(|address| *address)
                        .collect::<Vec<_>>();
                    let client = this.client.clone();
                    let context = &self.context;
                    trace!(context:?; "NSQuery::Fresh(Hit) -> NSQuery::GettingSocketStats for {:#?}", self.ns_addresses);

                    self.state = InnerNSQuery::GettingSocketStats(
                        query_for_sockets::<CCache>(client, sockets_addresses).boxed(),
                    );

                    // TODO
                    continue;
                }
                InnerNSQuery::Fresh(NSQueryCacheResponse::Miss) => {
                    let client = self.client.clone();
                    let cache = self.joined_cache.clone();
                    match self.context.clone().new_ns_address(
                        self.context
                            .query()
                            .with_new_qname_qtype(self.ns_domain.clone(), self.ns_address_rtype),
                    ) {
                        Ok(ns_address_context) => {
                            let context = self.context.as_ref();
                            trace!(context:?; "NSQuery::Fresh(Miss) -> NSQuery::QueryingNetworkNSAddresses: querying for new ns addresses with context '{ns_address_context:?}'");

                            self.state = InnerNSQuery::QueryingNetworkNSAddresses {
                                ns_addresses_query: recursive_query_owned_args(
                                    client,
                                    cache,
                                    ns_address_context,
                                )
                                .boxed(),
                            };

                            // Next loop will poll the query for NS addresses
                            continue;
                        }
                        Err(error) => {
                            let context = self.context.as_ref();
                            debug!(context:?; "NSQuery::Fresh(Miss) -> NSQuery::OutOfAddresses: new ns address error: {error}");

                            self.state = InnerNSQuery::OutOfAddresses;

                            // Exit loop. The was an error trying to query for the addresses.
                            return Poll::Ready(NSQueryResult::Result(
                                QError::ContextErr(error).into(),
                            ));
                        }
                    };
                }
                InnerNSQuery::QueryingNetworkNSAddresses { ns_addresses_query } => {
                    match ns_addresses_query.as_mut().poll(cx) {
                        Poll::Ready(QResult::Ok(QOk {
                            answer,
                            name_servers: _,
                            additional: _,
                        })) if answer.is_empty() => {
                            let context = self.context.as_ref();
                            trace!(context:?; "NSQuery::QueryingNetworkNSAddresses -> NSQuery::OutOfAddresses: received response QueryResponse::NoRecords when querying network for ns addresses");

                            self.state = InnerNSQuery::OutOfAddresses;

                            // Exit loop. There are no addresses to query.
                            return Poll::Ready(NSQueryResult::OutOfAddresses);
                        }
                        Poll::Ready(QResult::Ok(QOk {
                            answer,
                            name_servers: _,
                            additional: _,
                        })) => {
                            this.ns_addresses
                                .extend(answer.into_iter().filter_map(|record| rr_to_ip(record)));
                            if this.ns_addresses.is_empty() {
                                let context = &self.context;
                                trace!(context:?; "NSQuery::QueryingNetworkNSAddresses -> NSQuery::OutOfAddresses: tried to query first ns address but out of addresses");

                                self.state = InnerNSQuery::OutOfAddresses;

                                // Exit loop. There are no addresses to query.
                                return Poll::Ready(NSQueryResult::OutOfAddresses);
                            } else {
                                let sockets_addresses = this
                                    .ns_addresses
                                    .iter()
                                    .map(|address| *address)
                                    .collect::<Vec<_>>();
                                let client = this.client.clone();
                                let context = &self.context;
                                trace!(context:?; "NSQuery::QueryingNetworkNSAddresses -> NSQuery::GettingSocketStats");

                                self.state = InnerNSQuery::GettingSocketStats(
                                    query_for_sockets::<CCache>(client, sockets_addresses).boxed(),
                                );

                                // TODO
                                continue;
                            }
                        }
                        Poll::Ready(QResult::Err(error)) => {
                            let context = self.context.as_ref();
                            trace!(context:?; "NSQuery::QueryingNetworkNSAddresses -> NSQuery::OutOfAddresses: received response QueryResponse::Error({error}) when querying network for ns addresses");

                            self.state = InnerNSQuery::OutOfAddresses;

                            // Exit loop. The was an error trying to query for the addresses.
                            return Poll::Ready(NSQueryResult::Result(error.into()));
                        }
                        Poll::Ready(QResult::Fail(rcode)) => {
                            let context = self.context.as_ref();
                            trace!(context:?; "NSQuery::QueryingNetworkNSAddresses -> NSQuery::OutOfAddresses: received response QueryResponse::Error({rcode}) when querying network for ns addresses");

                            self.state = InnerNSQuery::OutOfAddresses;

                            // Exit loop. The was an error trying to query for the addresses.
                            return Poll::Ready(NSQueryResult::Result(rcode.into()));
                        }
                        Poll::Pending => {
                            let context = self.context.as_ref();
                            trace!(context:?; "NSQuery::QueryingNetworkNSAddresses: waiting for network query response for ns addresses");

                            // Exit loop. Will be woken up by the ns address query.
                            return Poll::Pending;
                        }
                    }
                }
                InnerNSQuery::GettingSocketStats(sockets_future) => {
                    match sockets_future.as_mut().poll(cx) {
                        Poll::Ready(sockets) => {
                            self.sockets.extend(
                                sockets
                                    .into_iter()
                                    .map(|socket| (socket.peer_addr(), socket)),
                            );
                            let context = self.context.as_ref();
                            trace!(context:?; "NSQuery::GettingSocketStats -> InnerNSQuery::NetworkQueryStart: getting sockets to determine the fastest addresses");

                            self.state = InnerNSQuery::NetworkQueryStart;

                            // TODO
                            continue;
                        }
                        Poll::Pending => {
                            let context = self.context.as_ref();
                            trace!(context:?; "NSQuery::GettingSocketStats: getting sockets to determine the fastest addresses");

                            // Exit loop. Will be woken up by the ns address query.
                            return Poll::Pending;
                        }
                    }
                }
                InnerNSQuery::NetworkQueryStart => {
                    match take_best_address::<CCache>(this.ns_addresses, &this.sockets) {
                        Some(next_ns_address) => {
                            let context = this.context.as_ref();
                            trace!(context:?; "NSQuery::NetworkQueryStart -> NSQuery::QueryingNetwork: setting up query to next ns {next_ns_address}");

                            let client = this.client.clone();
                            let cache = this.joined_cache.clone();
                            let context = this.context.clone();
                            let query =
                                query_network_owned_args(client, cache, context, next_ns_address)
                                    .boxed();

                            self.state = InnerNSQuery::QueryingNetwork(query);

                            // Next loop will poll the query for the question.
                            continue;
                        }
                        None => {
                            let context = self.context.as_ref();
                            trace!(context:?; "NSQuery::NetworkQueryStart -> NSQuery::OutOfAddresses: tried to query next ns address but out of addresses");

                            return Poll::Ready(NSQueryResult::OutOfAddresses);
                        }
                    }
                }
                InnerNSQuery::QueryingNetwork(query) => {
                    match query.as_mut().poll(cx) {
                        Poll::Ready(result) => {
                            let context = this.context.as_ref();
                            trace!(context:?; "NSQuery::QueryingNetwork -> NSQuery::NetworkQueryStart: found result '{result:?}'");

                            // Clear the query. If this object is polled again, a new one will be
                            // set up at that time.
                            self.state = InnerNSQuery::NetworkQueryStart;

                            // Exit loop. A result was found.
                            match result {
                                Ok(message) => {
                                    return Poll::Ready(NSQueryResult::Result(QResult::Ok(
                                        message,
                                    )));
                                }
                                Err(error) => {
                                    return Poll::Ready(NSQueryResult::Result(QResult::Err(
                                        error.into(),
                                    )));
                                }
                            }
                        }
                        Poll::Pending => {
                            let context = self.context.as_ref();
                            trace!(context:?; "NSQuery::QueryingNetwork: waiting for network query response for ns addresses");

                            // Exit loop. Will be woken up by the query.
                            return Poll::Pending;
                        }
                    }
                }
                InnerNSQuery::OutOfAddresses => {
                    let context = self.context.as_ref();
                    trace!(context:?; "NSQuery::OutOfAddresses");

                    // Exit loop. All addresses have been queried.
                    return Poll::Ready(NSQueryResult::OutOfAddresses);
                }
            }
        }
    }
}

#[pin_project]
struct NSSelectQuery<'a, 'b, 'c, CCache>
where
    CCache: AsyncCache + Send + Sync,
{
    // Note: the queries are read in reverse order (like a stack).
    ns_queries: Vec<Pin<Box<NSQuery<'a, 'b, 'c, CCache>>>>,
    running: Vec<Pin<Box<NSQuery<'a, 'b, 'c, CCache>>>>,
    max_concurrency: usize,
    add_query_timeout: Duration,
    #[pin]
    add_query_timer: Option<tokio::time::Sleep>,
}

impl<'a, 'b, 'c, CCache> NSSelectQuery<'a, 'b, 'c, CCache>
where
    CCache: AsyncCache + Send + Sync,
{
    pub fn new(
        ns_queries: Vec<Pin<Box<NSQuery<'a, 'b, 'c, CCache>>>>,
        max_concurrency: usize,
        add_query_timeout: Duration,
    ) -> Self {
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
        self.running.is_empty() && !self.ns_queries.is_empty()
    }
}

fn take_best_ns_query<'a, 'b, 'c, CCache>(
    ns_queries: &mut Vec<Pin<Box<NSQuery<'a, 'b, 'c, CCache>>>>,
) -> Option<Pin<Box<NSQuery<'a, 'b, 'c, CCache>>>>
where
    CCache: AsyncCache + Send + Sync,
{
    match ns_queries
        .iter()
        .enumerate()
        .max_by_key(|(_, ns_query)| ns_query.best_address_stats().map(|stats| Reverse(stats)))
    {
        Some((index, _)) => Some(ns_queries.swap_remove(index)),
        None => take_random(ns_queries),
    }
}

impl<'a, 'b, 'c, CCache> Future for NSSelectQuery<'a, 'b, 'c, CCache>
where
    CCache: AsyncCache + Send + Sync + 'static,
{
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
                }
            }

            // Initialize or refresh the `add_query_timer`
            match (
                this.add_query_timer.as_mut().as_pin_mut(),
                tokio::time::Instant::now().checked_add(*this.add_query_timeout),
            ) {
                // The expected case is that a fresh NSSelectQuery has not yet had the
                // `add_query_timer` initialized. We should do that here.
                (None, Some(deadline)) => this
                    .add_query_timer
                    .set(Some(tokio::time::sleep_until(deadline))),
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
                    }
                    None => {
                        // Don't want to be erroneously woken up if there is nobody else to add.
                        this.add_query_timer.set(None);
                    }
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
                    }
                    (Some(new_ns_query), _) => {
                        let ns_query = this.running.swap_remove(index);
                        this.running.push(new_ns_query);
                        this.ns_queries.push(ns_query);
                    }
                    (None, NSQueryResult::OutOfAddresses) => {
                        let _ = this.running.swap_remove(index);
                        // Don't want to be erroneously woken up if there is
                        // nobody else to add.
                        this.add_query_timer.set(None);
                    }
                    (None, _) => {
                        // Since we are down to the last ns_query, it can stay in the running queue
                        // until it runs out of addresses.

                        // Don't want to be erroneously woken up if there is
                        // nobody else to add.
                        this.add_query_timer.set(None);
                    }
                }

                return Poll::Ready(Some(result));
            }
        }

        match (this.ns_queries.len(), this.running.len()) {
            // All of the queued queries have completed.
            (0, 0) => Poll::Ready(None),
            // At least 1 query is still running.
            (_, 1..) => Poll::Pending,
            (1.., 0) => {
                panic!("There are still queries in the queue but the running queue is empty")
            }
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
    #[pin]
    inner: InnerNSRoundRobin<'d, 'e, 'f, 'g, 'h, CCache>,
}

#[pin_project(project = InnerNSRoundRobinProj)]
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
        #[pin]
        ns_query_select: NSSelectQuery<'c, 'd, 'e, CCache>,
    },
    Complete,
}

impl<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h, CCache> NSRoundRobin<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h, CCache>
where
    CCache: AsyncCache + Send + Sync + 'static,
{
    fn new(
        client: &'a Arc<DNSAsyncClient>,
        joined_cache: &'b Arc<CCache>,
        question: &'c Arc<Context>,
        name_servers: &'d [CDomainName],
    ) -> Self {
        Self {
            client,
            joined_cache,
            context: question,
            inner: InnerNSRoundRobin::Fresh { name_servers },
        }
    }
}

impl<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h, CCache> Future
    for NSRoundRobin<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h, CCache>
where
    CCache: AsyncCache + Send + Sync + 'static,
{
    type Output = QResult;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        loop {
            let mut this = self.as_mut().project();
            match this.inner.as_mut().project() {
                InnerNSRoundRobinProj::Fresh { name_servers } => {
                    let name_server_address_queries = name_servers
                        .iter()
                        .flat_map(|ns_domain| {
                            [
                                query_cache_for_ns_addresses(
                                    ns_domain.clone(),
                                    RType::A,
                                    this.context.clone(),
                                    this.client.clone(),
                                    this.joined_cache.clone(),
                                )
                                .boxed(),
                                query_cache_for_ns_addresses(
                                    ns_domain.clone(),
                                    RType::AAAA,
                                    this.context.clone(),
                                    this.client.clone(),
                                    this.joined_cache.clone(),
                                )
                                .boxed(),
                            ]
                        })
                        .collect::<Vec<_>>();
                    let capacity = name_server_address_queries.len();

                    this.inner.set(InnerNSRoundRobin::GetCachedNSAddresses {
                        name_server_address_queries,
                        name_server_cached_queries: Vec::with_capacity(capacity),
                        name_server_non_cached_queries: Vec::with_capacity(capacity),
                    });

                    let context = self.context.as_ref();
                    trace!(context:?; "NSRoundRobin::Fresh -> NSRoundRobin::GetCachedNSAddresses: Getting cached ns addresses");

                    // Next loop will poll all the NS address queries
                    continue;
                }
                InnerNSRoundRobinProj::GetCachedNSAddresses {
                    name_server_address_queries,
                    name_server_non_cached_queries,
                    name_server_cached_queries,
                } => {
                    name_server_address_queries.retain_mut(
                        |ns_address_query| match ns_address_query.as_mut().poll(cx) {
                            Poll::Ready(
                                ns_query @ NSQuery {
                                    ns_domain: _,
                                    ns_address_rtype: _,
                                    context: _,
                                    client: _,
                                    joined_cache: _,
                                    ns_addresses: _,
                                    sockets: _,
                                    state: InnerNSQuery::Fresh(NSQueryCacheResponse::Hit),
                                },
                            ) => {
                                name_server_cached_queries.push(Box::pin(ns_query));
                                false
                            }
                            Poll::Ready(ns_query) => {
                                name_server_non_cached_queries.push(Box::pin(ns_query));
                                false
                            }
                            Poll::Pending => true,
                        },
                    );
                    if name_server_address_queries.is_empty() {
                        let context = this.context.as_ref();
                        trace!(context:?; "NSRoundRobin::GetCachedNSAddresses -> NSRoundRobin::QueryNameServers: Received all cache responses. {} queries are cached. {} queries are non-cached", name_server_non_cached_queries.len(), name_server_cached_queries.len());
                        // Join the two lists of queries. The queries that don't have cached
                        // addresses are at the front and the ones with cached addresses are at the
                        // back. This list will be read like a stack, so the cached queries will be
                        // run first.
                        let mut ns_queries = Vec::with_capacity(
                            name_server_non_cached_queries.len() + name_server_cached_queries.len(),
                        );
                        ns_queries.extend(name_server_non_cached_queries.drain(..));
                        ns_queries.extend(name_server_cached_queries.drain(..));
                        let ns_query_select =
                            NSSelectQuery::new(ns_queries, 3, Duration::from_millis(200));

                        this.inner
                            .set(InnerNSRoundRobin::QueryNameServers { ns_query_select });

                        // Next loop will select the first query from the list and start it
                        continue;
                    } else {
                        let context = this.context.as_ref();
                        trace!(context:?; "NSRoundRobin::GetCachedNSAddresses: Waiting for cache responses for {} queries. {} queries are cached. {} queries are non-cached", name_server_address_queries.len(), name_server_non_cached_queries.len(), name_server_cached_queries.len());

                        // Exit loop. Wait for one of the address queries to wake us again.
                        return Poll::Pending;
                    }
                }
                InnerNSRoundRobinProj::QueryNameServers {
                    mut ns_query_select,
                } => {
                    match ns_query_select.as_mut().poll(cx) {
                        // No error. Valid response.
                        Poll::Ready(Some(NSQueryResult::Result(QResult::Ok(response @ Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NoError, question: _, answer: _, authority: _, additional: _ }))))
                        // If a server does not support a query type, we can probably assume it is not in that zone.
                        // TODO: verify that this is a valid assumption. Should we return NotImpl?
                      | Poll::Ready(Some(NSQueryResult::Result(QResult::Ok(response @ Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NotImp, question: _, answer: _, authority: _, additional: _ })))) => {
                            let result = query_response(response);

                            let context = this.context.as_ref();
                            trace!(context:?; "NSRoundRobin::QueryNameServers -> NSRoundRobin::Complete: Received result {result:?}");

                            this.inner.set(InnerNSRoundRobin::Complete);

                            // Exit forever. Query complete.
                            return Poll::Ready(result);
                        },
                        // Only authoritative servers can indicate that a name does not exist.
                        Poll::Ready(Some(NSQueryResult::Result(QResult::Ok(response @ Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: true, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NXDomain, question: _, answer: _, authority: _, additional: _ })))) => {
                            let result = QResult::Fail(RCode::NXDomain);

                            let context = this.context.as_ref();
                            trace!(context:?; "NSRoundRobin::QueryNameServers -> NSRoundRobin::Cleanup: Received error NXDomain in message '{response:?}'");

                            this.inner.set(InnerNSRoundRobin::Complete);

                            // Exit forever. Query complete.
                            return Poll::Ready(result);
                        },
                        // This server does not have the authority to say that the name
                        // does not exist. Ask others.
                        Poll::Ready(Some(response @ NSQueryResult::Result(QResult::Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: false, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::NXDomain, question: _, answer: _, authority: _, additional: _ }))))
                        // If there is an IO error, try a different server.
                      | Poll::Ready(Some(response @ NSQueryResult::Result(QResult::Err(_))))
                        // If a particular name server cannot be queried anymore, then keep
                        // trying to query the others.
                      | Poll::Ready(Some(response @ NSQueryResult::OutOfAddresses))
                        // If there was an error looking up one of the name servers, keep
                        // trying to look up the others.
                      | Poll::Ready(Some(response @ NSQueryResult::Result(QResult::Fail(_)))) => {
                            let context = this.context.as_ref();
                            trace!(context:?; "NSRoundRobin::QueryNameServers: Received error in message '{response:?}'");

                            // Next loop will poll the other name servers.
                            continue;
                        },
                        // If a name server cannot interpret what we are sending it, asking other name servers probably will not help.
                        // Treat as a hard error.
                        Poll::Ready(response @ Some(NSQueryResult::Result(QResult::Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::FormErr, question: _, answer: _, authority: _, additional: _ }))))
                        // If a name server refuses to perform an operation, we should not keep asking the other servers.
                        // TODO: verify that this is a valid way of handling.
                      | Poll::Ready(response @ Some(NSQueryResult::Result(QResult::Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: RCode::Refused, question: _, answer: _, authority: _, additional: _ }))))
                        // We don't know how to handle unknown errors.
                        // Assume they are a fatal failure.
                      | Poll::Ready(response @ Some(NSQueryResult::Result(QResult::Ok(Message { id: _, qr: QR::Response, opcode: _, authoritative_answer: _, truncation: false, recursion_desired: _, recursion_available: _, z: _, rcode: _, question: _, answer: _, authority: _, additional: _ }))))
                        // Malformed response.
                      | Poll::Ready(response @ Some(NSQueryResult::Result(QResult::Ok(Message { id: _, qr: _, opcode: _, authoritative_answer: _, truncation: _, recursion_desired: _, recursion_available: _, z: _, rcode: _, question: _, answer: _, authority: _, additional: _ }))))
                        // No more servers to query.
                      | Poll::Ready(response @ None) => {
                            let result = QResult::Fail(RCode::ServFail);

                            this.inner.set(InnerNSRoundRobin::Complete);

                            let context = this.context.as_ref();
                            trace!(context:?; "NSRoundRobin::QueryNameServers -> NSRoundRobin::Complete: Result is ServFail. Received response '{response:?}'");

                            // Exit forever. Query complete.
                            return Poll::Ready(result);
                        },
                        // Exit loop. Wait for one of the ns queries to wake us again.
                        Poll::Pending => return Poll::Pending,
                    }
                }
                InnerNSRoundRobinProj::Complete => {
                    panic!(
                        "InnerNSRoundRobin::Complete: query for '{}' was polled again after it already returned Poll::Ready",
                        this.context.query()
                    );
                }
            }
        }
    }
}

#[pinned_drop]
impl<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h, CCache> PinnedDrop
    for NSRoundRobin<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h, CCache>
where
    CCache: AsyncCache + Send + Sync + 'static,
{
    fn drop(mut self: Pin<&mut Self>) {
        let this = self.project();
        match this.inner.project() {
            InnerNSRoundRobinProj::Fresh { name_servers: _ } => (),
            InnerNSRoundRobinProj::GetCachedNSAddresses {
                name_server_address_queries: _,
                name_server_non_cached_queries: _,
                name_server_cached_queries: _,
            } => {
                let context = this.context.as_ref();
                trace!(context:?; "InnerNSRoundRobin::GetCachedNSAddresses -> NSRoundRobin::(drop): Cleaning up query {}", this.context.query());
            }
            InnerNSRoundRobinProj::QueryNameServers { ns_query_select: _ } => {
                let context = this.context.as_ref();
                trace!(context:?; "InnerNSRoundRobin::QueryNameServers -> NSRoundRobin::(drop): Cleaning up query {}", this.context.query());
            }
            InnerNSRoundRobinProj::Complete => (),
        }
    }
}

#[inline]
fn query_response(answer: Message) -> QResult {
    match answer {
        Message {
            id: _,
            qr: QR::Response,
            opcode: _,
            authoritative_answer: _,
            truncation: false,
            recursion_desired: _,
            recursion_available: _,
            z: _,
            rcode: RCode::NoError,
            question: _,
            answer,
            authority,
            additional,
        } => QResult::Ok(QOk {
            answer,
            name_servers: authority
                .into_iter()
                .filter_map(|record| record.try_into().ok())
                .collect(),
            additional,
        }),
        Message {
            id: _,
            qr: QR::Response,
            opcode: _,
            authoritative_answer: _,
            truncation: false,
            recursion_desired: _,
            recursion_available: _,
            z: _,
            rcode,
            question: _,
            answer: _,
            authority: _,
            additional: _,
        } => QResult::Fail(rcode),
        Message {
            id: _,
            qr: _,
            opcode: _,
            authoritative_answer: _,
            truncation: _,
            recursion_desired: _,
            recursion_available: _,
            z: _,
            rcode: _,
            question: _,
            answer: _,
            authority: _,
            additional: _,
        } => QResult::Fail(RCode::FormErr),
    }
}

#[pin_project(PinnedDrop)]
struct ActiveQuery<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h, CCache>
where
    CCache: AsyncCache + Send + Sync + 'static,
{
    #[pin]
    round_robin: NSRoundRobin<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h, CCache>,
    #[pin]
    inner: InnerActiveQuery,
}

#[pin_project(project = InnerActiveQueryProj)]
enum InnerActiveQuery {
    Fresh,
    WriteActiveQueries,
    Following(#[pin] once_watch::Receiver<QResult>),
    Cleanup(Option<QResult>),
    Complete,
}

impl<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h, CCache> ActiveQuery<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h, CCache>
where
    CCache: AsyncCache + Send + Sync + 'static,
{
    fn new(
        client: &'a Arc<DNSAsyncClient>,
        joined_cache: &'b Arc<CCache>,
        question: &'c Arc<Context>,
        name_servers: &'d [CDomainName],
    ) -> Self {
        Self {
            round_robin: NSRoundRobin::new(client, joined_cache, question, name_servers),
            inner: InnerActiveQuery::Fresh,
        }
    }
}

impl InnerActiveQuery {
    fn set_write_active_queries(mut self: std::pin::Pin<&mut Self>) {
        self.set(Self::WriteActiveQueries);
    }

    fn set_following(mut self: std::pin::Pin<&mut Self>, receiver: once_watch::Receiver<QResult>) {
        self.set(Self::Following(receiver));
    }

    fn set_cleanup(mut self: std::pin::Pin<&mut Self>, result: QResult) {
        self.set(Self::Cleanup(Some(result)));
    }

    fn set_complete(mut self: std::pin::Pin<&mut Self>) {
        self.set(Self::Complete);
    }
}

impl<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h, CCache> Future
    for ActiveQuery<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h, CCache>
where
    CCache: AsyncCache + Send + Sync + 'static,
{
    type Output = QResult;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        loop {
            let mut this = self.as_mut().project();
            match this.inner.as_mut().project() {
                InnerActiveQueryProj::Fresh => {
                    let r_active_queries = this.round_robin.client.active_queries.read().unwrap();
                    match r_active_queries.get(this.round_robin.context.query()) {
                        Some(result_sender) => {
                            let result_receiver = result_sender.subscribe();
                            drop(r_active_queries);

                            this.inner.set_following(result_receiver);

                            // TODO
                            continue;
                        }
                        None => {
                            drop(r_active_queries);

                            this.inner.set_write_active_queries();

                            // TODO
                            continue;
                        }
                    }
                }
                InnerActiveQueryProj::WriteActiveQueries => {
                    let mut w_active_queries =
                        this.round_robin.client.active_queries.write().unwrap();
                    match w_active_queries.entry(this.round_robin.context.query().clone()) {
                        Entry::Occupied(occupied_entry) => {
                            let result_receiver = occupied_entry.get().subscribe();
                            drop(w_active_queries);

                            this.inner.set_following(result_receiver);

                            // TODO
                            continue;
                        }
                        Entry::Vacant(vacant_entry) => {
                            let (send_response, result_receiver) = once_watch::channel();
                            vacant_entry.insert(send_response);
                            drop(w_active_queries);

                            this.inner.set_following(result_receiver);

                            // TODO
                            continue;
                        }
                    }
                }
                InnerActiveQueryProj::Following(mut result_receiver) => {
                    match result_receiver.as_mut().poll(cx) {
                        Poll::Ready(Ok(response)) => {
                            // The sender is responsible for removing the channel from the active
                            // queries map.
                            this.inner.set_complete();

                            // TODO
                            return Poll::Ready(response);
                        }
                        Poll::Ready(Err(once_watch::RecvError::Closed)) => {
                            // The sender is responsible for removing the channel from the active
                            // queries map.
                            this.inner.set_complete();

                            // TODO
                            return Poll::Ready(QResult::Fail(RCode::ServFail));
                        }
                        Poll::Pending => {
                            // Setup is done. Awaiting results. Now, poll the round_robin.
                        }
                    }

                    match this.round_robin.as_mut().poll(cx) {
                        Poll::Ready(result) => {
                            let _ = result_receiver.get_sender().send(result.clone());

                            this.inner.set_cleanup(result);

                            continue;
                        }
                        Poll::Pending => {
                            // Will be awoken if either the result_receiver or round_robin are.
                            return Poll::Pending;
                        }
                    }
                }
                InnerActiveQueryProj::Cleanup(result) => {
                    let mut w_active_queries =
                        this.round_robin.client.active_queries.write().unwrap();
                    if let Some(result_sender) =
                        w_active_queries.remove(&this.round_robin.context.query())
                    {
                        // Always make sure the channel is closed. This *should* never have an
                        // effect but will ensure that it is never left open.
                        result_sender.close();
                    }
                    drop(w_active_queries);

                    match result.take() {
                        Some(result) => {
                            this.inner.set_complete();

                            return Poll::Ready(result);
                        }
                        None => {
                            this.inner.set_complete();

                            panic!("The Option result is supposed to always be Some but was None")
                        }
                    }
                }
                InnerActiveQueryProj::Complete => {
                    panic!("ActiveQuery cannot be polled after completion");
                }
            }
        }
    }
}

#[pinned_drop]
impl<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h, CCache> PinnedDrop
    for ActiveQuery<'a, 'b, 'c, 'd, 'e, 'f, 'g, 'h, CCache>
where
    CCache: AsyncCache + Send + Sync + 'static,
{
    fn drop(mut self: Pin<&mut Self>) {
        match self.as_mut().project().inner.as_mut().project() {
            InnerActiveQueryProj::Fresh | InnerActiveQueryProj::WriteActiveQueries => {
                // Nothing to do
            }
            InnerActiveQueryProj::Following(_) | InnerActiveQueryProj::Cleanup(_) => {
                // An active query has been registered. Need to make sure it doesn't need to be
                // removed.
                let mut w_active_queries = self.round_robin.client.active_queries.write().unwrap();
                if let Some(sender) = w_active_queries.get(self.round_robin.context.query()) {
                    if (sender.sender_count() <= 1) && (sender.receiver_count() == 0) {
                        let _ = w_active_queries.remove(self.round_robin.context.query());
                    }
                }
                drop(w_active_queries);
            }
            InnerActiveQueryProj::Complete => {
                // Nothing to do
            }
        }
    }
}

#[inline]
pub async fn query_name_servers<CCache>(
    client: &Arc<DNSAsyncClient>,
    joined_cache: &Arc<CCache>,
    context: Arc<Context>,
    name_servers: &[CDomainName],
) -> QResult
where
    CCache: AsyncCache + Send + Sync + 'static,
{
    info!(context:?; "Querying Name Servers for '{}'", context.query());
    ActiveQuery::new(client, joined_cache, &context, name_servers).await
}

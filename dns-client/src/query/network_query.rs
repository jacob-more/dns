use std::{net::{IpAddr, SocketAddr}, sync::Arc};

use dns_lib::{interface::cache::cache::AsyncCache, query::{message::Message, question::Question}};
use log::trace;
use network::mixed_tcp_udp::{MixedSocket, errors::QueryError, QueryOptions};

use crate::DNSAsyncClient;

const UPSTREAM_PORT: u16 = 53;

pub async fn query_network<CCache>(client: &DNSAsyncClient, cache: Arc<CCache>, question: &Question, name_server_address: &IpAddr) -> Result<Message, QueryError> where CCache: AsyncCache + Sync {
    let upstream_dns_address = SocketAddr::new(
        *name_server_address,
        UPSTREAM_PORT,
    );
    let mut message_question = Message::from(question);
    trace!(question:?; "Querying network '{upstream_dns_address}' (UDP/TCP) with query '{message_question:?}'");

    let socket = client.socket_manager.get(&upstream_dns_address).await;
    let message = MixedSocket::query(&socket, &mut message_question, QueryOptions::Both).await?;

    // If the truncation flag is set, we need to try again with TCP
    if !message.truncation_flag() {
        trace!(question:?; "Querying network '{upstream_dns_address}', got response '{message:?}'");
        cache.insert_message(&message).await;
        return Ok(message);
    }
    trace!(question:?; "Querying network '{upstream_dns_address}', got truncation flag in response '{message:?}'");

    let message = MixedSocket::query(&socket, &mut message_question, QueryOptions::TcpOnly).await?;
    trace!(question:?; "Querying network '{upstream_dns_address}' (TCP Only), got response '{message:?}'");
    cache.insert_message(&message).await;
    return Ok(message);
}

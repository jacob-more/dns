use std::{net::IpAddr, sync::Arc};

use dns_lib::{interface::cache::cache::AsyncCache, query::{message::Message, question::Question}};
use log::trace;
use network::{async_query::QueryOpt, errors::QueryError, mixed_tcp_udp::MixedSocket};

use crate::DNSAsyncClient;

pub async fn query_network<CCache>(client: &DNSAsyncClient, cache: Arc<CCache>, question: &Question, name_server_address: &IpAddr) -> Result<Message, QueryError> where CCache: AsyncCache + Sync {
    let mut message_question = Message::from(question);
    trace!(question:?; "Querying network '{name_server_address}' (UDP/TCP) with query '{message_question:?}'");

    let socket = client.socket_manager.get_udp_tcp(*name_server_address).await;
    let message = MixedSocket::query(&socket, &mut message_question, QueryOpt::UdpTcp).await?;

    // If the truncation flag is set, we need to try again with TCP
    if !message.truncation_flag() {
        trace!(question:?; "Querying network '{name_server_address}', got response '{message:?}'");
        cache.insert_message(&message).await;
        return Ok(message);
    }
    trace!(question:?; "Querying network '{name_server_address}', got truncation flag in response '{message:?}'");

    let message = MixedSocket::query(&socket, &mut message_question, QueryOpt::Tcp).await?;
    trace!(question:?; "Querying network '{name_server_address}' (TCP Only), got response '{message:?}'");
    cache.insert_message(&message).await;
    return Ok(message);
}

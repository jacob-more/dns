use std::{net::{SocketAddr, IpAddr}, sync::Arc};

use dns_lib::{query::{question::Question, message::Message}, interface::cache::cache::AsyncCache};
use network::mixed_tcp_udp::{MixedSocket, QueryOptions};
use tokio::io;

use crate::DNSAsyncClient;

const UPSTREAM_PORT: u16 = 53;

pub async fn query_network<CCache>(client: &DNSAsyncClient, cache: Arc<CCache>, question: &Question, name_server_address: &IpAddr) -> io::Result<Message> where CCache: AsyncCache {
    let upstream_dns_address = SocketAddr::new(
        *name_server_address,
        UPSTREAM_PORT,
    );
    let message_question = Message::from(question);

    let socket = client.socket_manager.get(&upstream_dns_address).await;
    let message = MixedSocket::query(socket.clone(), message_question.clone(), QueryOptions::Both).await?;

    // If the truncation flag is set, we need to try again with TCP
    if !message.truncation_flag() {
        cache.insert(&message).await;
        return Ok(message);
    }

    println!("Truncation flag in message: {message:?}");

    let message = MixedSocket::query(socket, message_question, QueryOptions::TcpOnly).await?;
    cache.insert(&message).await;
    return Ok(message);
}

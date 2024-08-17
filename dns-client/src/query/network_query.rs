use std::{net::{IpAddr, SocketAddr}, sync::Arc, time::Duration};

use async_lib::awake_token::AwakeToken;
use dns_lib::{query::{question::Question, message::Message}, interface::cache::cache::AsyncCache};
use network::mixed_tcp_udp::{MixedSocket, QueryOptions};
use tokio::io;

use crate::DNSAsyncClient;

const UPSTREAM_PORT: u16 = 53;
const QUERY_TIMEOUT_MS: Duration = Duration::from_secs(30);

pub async fn query_network<CCache>(client: &DNSAsyncClient, cache: Arc<CCache>, question: &Question, name_server_address: &IpAddr, kill_token: Option<Arc<AwakeToken>>) -> io::Result<Message> where CCache: AsyncCache + Sync {
    let upstream_dns_address = SocketAddr::new(
        *name_server_address,
        UPSTREAM_PORT,
    );
    let message_question = Message::from(question);

    let socket = client.socket_manager.get(&upstream_dns_address).await;
    let message = MixedSocket::query(socket.clone(), message_question.clone(), QueryOptions::Both, Some(QUERY_TIMEOUT_MS), kill_token.clone()).await?;

    // If the truncation flag is set, we need to try again with TCP
    if !message.truncation_flag() {
        cache.insert_message(&message).await;
        return Ok(message);
    }

    println!("Truncation flag in message: {message:?}");

    let message = MixedSocket::query(socket, message_question, QueryOptions::TcpOnly, Some(QUERY_TIMEOUT_MS), kill_token).await?;
    cache.insert_message(&message).await;
    return Ok(message);
}

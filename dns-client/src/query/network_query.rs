use std::{net::{SocketAddr, IpAddr, Ipv4Addr, Ipv6Addr}, time::Instant, sync::Arc};

use dns_lib::{query::{question::Question, message::Message}, interface::cache::cache::AsyncCache};
use tokio::{io, net::UdpSocket};

use crate::{IPV4_ENABLED, IPV6_ENABLED, udp_handler::{write_udp_message, read_udp_message}, tcp_manager::TCPManager, DNSAsyncClient};

const UPSTREAM_PORT: u16 = 53;

pub async fn query_network<CCache>(client: &DNSAsyncClient, cache: Arc<CCache>, question: &Question, name_server_address: &IpAddr) -> io::Result<Message> where CCache: AsyncCache {
    let overall_start_time = Instant::now();

    let upstream_dns_address = SocketAddr::new(
        *name_server_address,
        UPSTREAM_PORT,
    );
    let message_question = Message::from(question);

    // TODO: Double check that the id is not already in use

    // Send Question Over UDP
    let udp_start_time = Instant::now();
    let message = query_over_udp(client, upstream_dns_address, &message_question).await?;
    let udp_end_time = Instant::now();

    // If the truncation flag is set, we need to try again with TCP
    if !message.truncation_flag() {
        let overall_end_time = Instant::now();
        println!("Network Query Time: {} ms", overall_end_time.duration_since(overall_start_time).as_millis());
        println!("\tUDP Query Time: {} ms", udp_end_time.duration_since(udp_start_time).as_millis());
        println!("\tTCP Query Time: N/A");
        println!();

        cache.insert(&message).await;
        return Ok(message);
    }

    let tcp_start_time = Instant::now();
    let message = query_over_tcp(client, upstream_dns_address, &message_question).await?;
    let tcp_end_time = Instant::now();

    let overall_end_time = Instant::now();
    println!("Network Query Time: {} ms", overall_end_time.duration_since(overall_start_time).as_millis());
    println!("\tUDP Query Time: {} ms", udp_end_time.duration_since(udp_start_time).as_millis());
    println!("\tTCP Query Time: {} ms", tcp_end_time.duration_since(tcp_start_time).as_millis());
    println!();

    cache.insert(&message).await;
    return Ok(message);
}

async fn query_over_udp(_client: &DNSAsyncClient, upstream_socket: SocketAddr, query: &Message) -> io::Result<Message> {
    let local_socket = match upstream_socket.ip() {
        IpAddr::V4(_) if !IPV4_ENABLED => return Err(io::Error::from(io::ErrorKind::Unsupported)),
        IpAddr::V6(_) if !IPV6_ENABLED => return Err(io::Error::from(io::ErrorKind::Unsupported)),
        IpAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
        IpAddr::V6(_) => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
    };

    // Connect to upstream dns
    println!("Connecting to {upstream_socket} via UDP...");
    let udp_socket = UdpSocket::bind(local_socket).await?;
    udp_socket.connect(upstream_socket).await?;
    println!("Local Address: {:?}", udp_socket.local_addr());
    println!("Foreign Address {:?}", udp_socket.peer_addr());

    // Send a DNS Query
    println!("Querying DNS Server...");
    println!("Sending: {query:#?}");
    if let Err(wire_error) = write_udp_message(&udp_socket, query).await {
        return Err(io::Error::new(io::ErrorKind::InvalidData, wire_error));
    }

    // Receive DNS Response
    // TODO: store the response time of the peer in the meta cache
    let (response, _peer) = match read_udp_message(&udp_socket).await {
        Ok((response, peer)) => (response, peer),
        Err(wire_error) => return Err(io::Error::new(io::ErrorKind::InvalidData, wire_error)),
    };
    println!("Response: {response:#?}\n");

    return Ok(response);
}

async fn query_over_tcp(client: &DNSAsyncClient, upstream_socket: SocketAddr, question: &Message) -> io::Result<Message> {
    TCPManager::query_tcp(client.tcp_manager.clone(), upstream_socket, question).await
}

type OnceWatchMessageSender =
    async_lib::once_watch::Sender<Result<dns_lib::query::message::Message, errors::QueryError>>;

pub mod async_query;
pub mod receive;
pub mod rolling_average;
pub mod socket;

pub mod errors;
pub mod socket_manager;

pub mod mixed_tcp_udp;
pub mod quic;
pub mod tls;

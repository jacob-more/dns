pub mod client;
pub mod server;

pub mod cache;


pub mod ports {
    pub const DNS_UDP_PORT: u16 = 53;
    pub const DNS_TCP_PORT: u16 = 53;
    pub const DOH_UDP_PORT: u16 = 443;
    pub const DOH_TCP_PORT: u16 = 443;
    pub const DOT_TCP_PORT: u16 = 853;
    pub const DOQ_UDP_PORT: u16 = 853;
}

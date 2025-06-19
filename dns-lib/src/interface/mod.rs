pub mod client;
pub mod server;

pub mod cache;

pub mod ports {
    use static_assertions::const_assert_ne;

    pub const DNS_UDP_PORT: u16 = 53;
    pub const DNS_TCP_PORT: u16 = 53;
    pub const DOH_UDP_PORT: u16 = 443;
    pub const DOH_TCP_PORT: u16 = 443;
    pub const DOT_TCP_PORT: u16 = 853;
    pub const DOQ_UDP_PORT: u16 = 853;

    // None of the default UDP ports should overlap
    const_assert_ne!(DNS_UDP_PORT, DOH_UDP_PORT);
    const_assert_ne!(DNS_UDP_PORT, DOQ_UDP_PORT);
    const_assert_ne!(DOH_UDP_PORT, DOQ_UDP_PORT);

    // None of the default TCP ports should overlap
    const_assert_ne!(DNS_TCP_PORT, DOH_TCP_PORT);
    const_assert_ne!(DNS_TCP_PORT, DOT_TCP_PORT);
    const_assert_ne!(DOH_TCP_PORT, DOT_TCP_PORT);
}

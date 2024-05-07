use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use tokio::sync::RwLock;

use crate::mixed_tcp_udp::MixedSocket;

pub struct SocketManager {
    sockets: RwLock<HashMap<SocketAddr, Arc<RwLock<MixedSocket>>>>
}

impl SocketManager {
    pub fn new() -> Self {
        Self { sockets: RwLock::new(HashMap::new()) }
    }

    pub async fn get(&self, address: &SocketAddr) -> Arc<RwLock<MixedSocket>> {
        let r_sockets = self.sockets.read().await;
        match r_sockets.get(address) {
            Some(socket) => return socket.clone(),
            None => (),
        }
        drop(r_sockets);

        let mut w_sockets = self.sockets.write().await;
        match w_sockets.get(address) {
            Some(socket) => return socket.clone(),
            None => {
                let socket = MixedSocket::new(address.clone());
                w_sockets.insert(address.clone(), socket.clone());
                return socket;
            },
        }
    }
}

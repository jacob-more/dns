use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};

use tokio::{sync::RwLock, task::JoinHandle};

use crate::mixed_tcp_udp::MixedSocket;

struct InternalSocketManager {
    sockets: HashMap<SocketAddr, Arc<MixedSocket>>,
    garbage_collection: Option<JoinHandle<()>>,
}

impl InternalSocketManager {
    pub fn new() -> Self {
        Self {
            sockets: HashMap::new(),
            garbage_collection: None,
        }
    }

    fn run_garbage_collection(internal_socket_manager: Arc<RwLock<Self>>) -> JoinHandle<()> {
        tokio::task::spawn(async move {
            let gc_interval = Duration::from_secs(60);
            loop {
                tokio::time::sleep(gc_interval).await;
                Self::drop_unused_sockets(&internal_socket_manager).await;
            }
        })
    }

    async fn drop_unused_sockets(internal_socket_manager: &Arc<RwLock<Self>>) {
        let mut w_socket_manager = internal_socket_manager.write().await;
        w_socket_manager.sockets.retain(|_, socket| {
            // If we are actively sending messages on a socket, we should never close it.
            if socket.recent_messages_sent() {
                socket.reset_recent_messages_sent_and_received();
                true
            } else {
                socket.reset_recent_messages_sent_and_received();
                false
            }
            // If we are actively receiving messages on a socket, that does not mean we should keep
            // it.
            // TODO: If access is ever given to get the number of active queries, that could be used
            // to determine if the socket should be closed too.
        });
        drop(w_socket_manager);
    }
}

#[derive(Clone)]
pub struct SocketManager {
    internal: Arc<RwLock<InternalSocketManager>>
}

impl SocketManager {
    pub async fn new() -> Self {
        let socket_manager = Self { internal: Arc::new(RwLock::new(InternalSocketManager::new())) };

        let join_handle = InternalSocketManager::run_garbage_collection(socket_manager.internal.clone());
        let mut w_isocket_manager = socket_manager.internal.write().await;
        w_isocket_manager.garbage_collection = Some(join_handle);
        drop(w_isocket_manager);

        socket_manager
    }

    pub async fn get(&self, address: &SocketAddr) -> Arc<MixedSocket> {
        let r_socket_manager = self.internal.read().await;
        match r_socket_manager.sockets.get(address) {
            Some(socket) => return socket.clone(),
            None => (),
        }
        drop(r_socket_manager);

        let mut w_socket_manager = self.internal.write().await;
        match w_socket_manager.sockets.get(address) {
            Some(socket) => return socket.clone(),
            None => {
                let socket = MixedSocket::new(address.clone());
                w_socket_manager.sockets.insert(address.clone(), socket.clone());
                return socket;
            },
        }
    }

    pub async fn for_each<F>(&self, f: F)
    where
        Self: Sized,
        F: FnMut((&SocketAddr, &Arc<MixedSocket>)),
    {
        let r_socket_manager = self.internal.read().await;
        r_socket_manager.sockets.iter().for_each(f);
        drop(r_socket_manager);
    }
}

impl Drop for SocketManager {
    fn drop(&mut self) {
        let imanager = self.internal.clone();
        tokio::task::spawn(async move {
            let r_imanager = imanager.read().await;
            if let Some(garbage_collection) = &r_imanager.garbage_collection {
                garbage_collection.abort()
            }
            drop(r_imanager);
        });
    }
}

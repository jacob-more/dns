use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};

use tokio::{select, sync::{watch, RwLock}, task::JoinHandle};

use crate::mixed_tcp_udp::MixedSocket;


const DEFAULT_KEEP_ALIVE: Duration = Duration::from_secs(120);


struct InternalSocketManager {
    sockets: HashMap<SocketAddr, Arc<MixedSocket>>,
    garbage_collection: Option<JoinHandle<()>>,
    keep_alive: watch::Sender<Duration>,
}

impl InternalSocketManager {
    #[inline]
    pub fn with_keep_alive(keep_alive: Duration) -> (Self, watch::Receiver<Duration>) {
        let (keep_alive_sender, keep_alive_receiver) = watch::channel(keep_alive);
        let manager = Self {
            sockets: HashMap::new(),
            garbage_collection: None,
            keep_alive: keep_alive_sender,
        };
        (manager, keep_alive_receiver)
    }

    #[inline]
    fn start_garbage_collection(internal_socket_manager: Arc<RwLock<Self>>, mut keep_alive_receiver: watch::Receiver<Duration>) -> JoinHandle<()> {
        tokio::task::spawn(async move {
            let mut gc_interval = *keep_alive_receiver.borrow_and_update();
            loop {
                select! {
                    biased;
                    _ = tokio::time::sleep(gc_interval) => Self::drop_unused_sockets(&internal_socket_manager).await,
                    change_notification = keep_alive_receiver.changed() => {
                        match change_notification {
                            // Modifies the interval at which garbage collection is performed.
                            Ok(()) => gc_interval = *keep_alive_receiver.borrow(),
                            // If the send channel is lost, that means that socket manager was
                            // dropped somehow.
                            Err(_) => break,
                        }
                    },
                }
            }
        })
    }

    #[inline]
    async fn drop_unused_sockets(internal_socket_manager: &Arc<RwLock<Self>>) {
        let mut w_socket_manager = internal_socket_manager.write().await;
        w_socket_manager.sockets.retain(|address, socket| {
            // If we are actively sending messages on a socket, we should never close it.
            if socket.recent_messages_sent() {
                socket.reset_recent_messages_sent_and_received();
                true
            } else {
                tokio::task::spawn(socket.clone().disable_both());
                socket.reset_recent_messages_sent_and_received();
                println!("GC: Removing {address} from socket manager");
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
    #[inline]
    pub async fn new() -> Self { Self::with_keep_alive(DEFAULT_KEEP_ALIVE).await }

    #[inline]
    pub async fn with_keep_alive(keep_alive: Duration) -> Self {
        let (socket_manager, keep_alive_receiver) = InternalSocketManager::with_keep_alive(keep_alive);
        let socket_manager = Self { internal: Arc::new(RwLock::new(socket_manager)) };

        let join_handle = InternalSocketManager::start_garbage_collection(socket_manager.internal.clone(), keep_alive_receiver);
        let mut w_isocket_manager = socket_manager.internal.write().await;
        w_isocket_manager.garbage_collection = Some(join_handle);
        drop(w_isocket_manager);

        socket_manager
    }

    #[inline]
    pub async fn set_keep_alive(&self, new_keep_alive: Duration) {
        let w_socket_manager = self.internal.write().await;
        w_socket_manager.keep_alive.send_if_modified(|current_keep_alive| {
            if *current_keep_alive == new_keep_alive {
                false
            } else {
                *current_keep_alive = new_keep_alive;
                true
            }
        });
        drop(w_socket_manager);
    }

    #[inline]
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

    #[inline]
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
            // Stop garbage collection.
            let r_imanager = imanager.read().await;
            if let Some(garbage_collection) = &r_imanager.garbage_collection {
                garbage_collection.abort();
            }

            // Shutdown all of the sockets still being managed.
            for (_, socket) in r_imanager.sockets.iter() {
                let _ = socket.clone().shutdown_both().await;
            }
            drop(r_imanager);
        });
    }
}

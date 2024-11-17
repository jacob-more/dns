use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};

use futures::StreamExt;
use tokio::{select, sync::{watch, RwLock}, task::JoinHandle};

use crate::mixed_tcp_udp::MixedSocket;


const DEFAULT_KEEP_ALIVE: Duration = Duration::from_secs(30);


struct InternalSocketManager {
    sockets: HashMap<SocketAddr, (Arc<MixedSocket>, u8)>,
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
            let mut start = tokio::time::Instant::now();
            let mut option_deadline = start.checked_add(gc_interval);
            loop {
                match option_deadline {
                    Some(deadline) => {
                        select! {
                            biased;
                            () = tokio::time::sleep_until(deadline) => {
                                Self::drop_unused_sockets(&internal_socket_manager).await;
                                start = tokio::time::Instant::now();
                                option_deadline = start.checked_add(gc_interval);
                            },
                            change_notification = keep_alive_receiver.changed() => {
                                match change_notification {
                                    // Modifies the interval at which garbage collection is performed.
                                    Ok(()) => {
                                        gc_interval = *keep_alive_receiver.borrow();
                                        // Still using the previous `start`. This way, we can run
                                        // the garbage collection if the new timeout is shorter or
                                        // hold off on running it if the timeout is longer.
                                        option_deadline = start.checked_add(gc_interval);
                                        
                                    },
                                    // If the send channel is lost, that means that socket manager was
                                    // dropped somehow.
                                    Err(_) => break,
                                }
                            },
                        }
                    },
                    None => {
                        // If we cannot add `gc_interval` to the current time, then we can't run the
                        // garbage collection unless a new `gc_interval` is provided.
                        match keep_alive_receiver.changed().await {
                            Ok(()) => {
                                gc_interval = *keep_alive_receiver.borrow();
                                start = tokio::time::Instant::now();
                                option_deadline = start.checked_add(gc_interval);
                            },
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
        w_socket_manager.sockets.retain(|address, (socket, nothing_received)| {
            // If we are actively sending messages on a socket, we should never close it.
            if socket.recent_messages_sent() {
                *nothing_received += 1;
            } else {
                *nothing_received = 0;
            }
            socket.reset_recent_messages_sent_and_received();

            if *nothing_received >= 10 {
                tokio::task::spawn(socket.clone().disable_both());
                println!("GC: Removing {address} from socket manager");
                false
            } else if *nothing_received >= 3 {
                tokio::task::spawn(socket.clone().shutdown_both());
                println!("GC: Shutdown {address} from socket manager");
                false
            } else {
                false
            }
            // If we are actively receiving messages on a socket, that does not mean we should keep
            // it.
            // TODO: If access is ever given to get the number of active queries, that could be used
            // to determine if the socket should be closed too.
        });
        drop(w_socket_manager);
    }

    #[inline]
    async fn drop_all_sockets(internal_socket_manager: &Arc<RwLock<Self>>) {
        let mut w_socket_manager = internal_socket_manager.write().await;
        futures::stream::iter(w_socket_manager.sockets.drain())
            .for_each_concurrent(None, |(address, (socket, _))| async move {
                println!("GC: Removing {address} from socket manager");
                let _ = socket.disable_both().await;
            }).await;
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

    /// # Cancel Safety
    /// 
    /// This function is cancel safe.
    #[inline]
    pub async fn get(&self, address: &SocketAddr) -> Arc<MixedSocket> {
        let r_socket_manager = self.internal.read().await;
        match r_socket_manager.sockets.get(address) {
            Some((socket, _)) => return socket.clone(),
            None => (),
        }
        drop(r_socket_manager);

        let mut w_socket_manager = self.internal.write().await;
        match w_socket_manager.sockets.get(address) {
            Some((socket, _)) => return socket.clone(),
            None => {
                let socket = MixedSocket::new(address.clone());
                w_socket_manager.sockets.insert(address.clone(), (socket.clone(), 0));
                return socket;
            },
        }
    }

    /// # Cancel Safety
    /// 
    /// This function is cancel safe.
    #[inline]
    pub async fn try_get(&self, address: &SocketAddr) -> Option<Arc<MixedSocket>> {
        let r_socket_manager = self.internal.read().await;
        let socket = r_socket_manager.sockets.get(address).cloned();
        drop(r_socket_manager);
        return socket.map(|(socket, _)| socket);
    }

    /// # Cancel Safety
    /// 
    /// This function is cancel safe.
    #[inline]
    pub async fn get_all(&self, addresses: impl Iterator<Item = &SocketAddr>) -> Vec<Arc<MixedSocket>> {
        let mut w_socket_manager = self.internal.write().await;
        let sockets = addresses
            .map(|address| match w_socket_manager.sockets.get(address) {
                Some((socket, _)) => socket.clone(),
                None => {
                    let socket = MixedSocket::new(address.clone());
                    w_socket_manager.sockets.insert(address.clone(), (socket.clone(), 0));
                    socket
                },
            })
            .collect::<Vec<_>>();
        drop(w_socket_manager);
        return sockets;
    }

    /// # Cancel Safety
    /// 
    /// This function is cancel safe.
    #[inline]
    pub async fn try_get_all(&self, addresses: impl Iterator<Item = &SocketAddr>) -> Vec<Arc<MixedSocket>> {
        let r_socket_manager = self.internal.read().await;
        let sockets = addresses
            .filter_map(|address| r_socket_manager.sockets.get(address).map(|(socket, _)| socket.clone()))
            .collect::<Vec<_>>();
        drop(r_socket_manager);
        return sockets;
    }

    #[inline]
    pub async fn for_each<F>(&self, f: F)
    where
        Self: Sized,
        F: FnMut((&SocketAddr, &Arc<MixedSocket>)),
    {
        let r_socket_manager = self.internal.read().await;
        r_socket_manager.sockets.iter().map(|(address, (socket, _))| (address, socket)).for_each(f);
        drop(r_socket_manager);
    }

    #[inline]
    pub async fn drop_all_sockets(&self) {
        InternalSocketManager::drop_all_sockets(&self.internal).await;
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
            for (_, (socket, _)) in r_imanager.sockets.iter() {
                let _ = socket.clone().shutdown_both().await;
            }
            drop(r_imanager);
        });
    }
}

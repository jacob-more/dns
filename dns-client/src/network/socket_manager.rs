use std::{
    collections::{HashMap, hash_map::Entry},
    net::IpAddr,
    sync::Arc,
    time::Duration,
};

use dns_lib::types::c_domain_name::CDomainName;
use futures::StreamExt;
use rustls::RootCertStore;
use tokio::{
    select,
    sync::{RwLock, watch},
    task::JoinHandle,
};

use crate::network::{mixed_tcp_udp::MixedSocket, quic::QuicSocket, tls::TlsSocket};

const DEFAULT_KEEP_ALIVE: Duration = Duration::from_secs(30);

struct InternalSocketManager {
    udp_tcp_sockets: HashMap<IpAddr, (Arc<MixedSocket>, u8)>,
    tls_sockets: HashMap<(IpAddr, CDomainName), (Arc<TlsSocket>, u8)>,
    quic_sockets: HashMap<(IpAddr, CDomainName), (Arc<QuicSocket>, u8)>,
    garbage_collection: Option<JoinHandle<()>>,
    keep_alive: watch::Sender<Duration>,
}

impl InternalSocketManager {
    #[inline]
    pub fn with_keep_alive(keep_alive: Duration) -> (Self, watch::Receiver<Duration>) {
        let (keep_alive_sender, keep_alive_receiver) = watch::channel(keep_alive);
        let manager = Self {
            udp_tcp_sockets: HashMap::new(),
            tls_sockets: HashMap::new(),
            quic_sockets: HashMap::new(),
            garbage_collection: None,
            keep_alive: keep_alive_sender,
        };
        (manager, keep_alive_receiver)
    }

    #[inline]
    fn start_garbage_collection(
        internal_socket_manager: Arc<RwLock<Self>>,
        mut keep_alive_receiver: watch::Receiver<Duration>,
    ) -> JoinHandle<()> {
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
                    }
                    None => {
                        // If we cannot add `gc_interval` to the current time, then we can't run the
                        // garbage collection unless a new `gc_interval` is provided.
                        match keep_alive_receiver.changed().await {
                            Ok(()) => {
                                gc_interval = *keep_alive_receiver.borrow();
                                start = tokio::time::Instant::now();
                                option_deadline = start.checked_add(gc_interval);
                            }
                            // If the send channel is lost, that means that socket manager was
                            // dropped somehow.
                            Err(_) => break,
                        }
                    }
                }
            }
        })
    }

    #[inline]
    async fn drop_unused_sockets(internal_socket_manager: &Arc<RwLock<Self>>) {
        let mut w_socket_manager = internal_socket_manager.write().await;
        w_socket_manager
            .udp_tcp_sockets
            .retain(|address, (socket, nothing_received)| {
                // If we are actively sending messages on a socket, we should never close it.
                if socket.recent_messages_sent() {
                    *nothing_received += 1;
                } else {
                    *nothing_received = 0;
                }
                socket.reset_recent_messages_sent_and_received();

                if *nothing_received >= 10 {
                    tokio::task::spawn(socket.clone().disable());
                    println!("GC: Removing {address} from socket manager");
                    false
                } else if *nothing_received >= 3 {
                    tokio::task::spawn(socket.clone().shutdown());
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
        futures::stream::iter(w_socket_manager.udp_tcp_sockets.drain())
            .for_each_concurrent(None, |(address, (socket, _))| async move {
                println!("GC: Removing {address} from socket manager");
                let _ = socket.disable().await;
            })
            .await;
        drop(w_socket_manager);
    }
}

#[derive(Clone)]
pub struct SocketManager {
    tls_client_config: Arc<rustls::ClientConfig>,
    quic_client_config: Arc<quinn::ClientConfig>,
    internal: Arc<RwLock<InternalSocketManager>>,
}

impl SocketManager {
    #[inline]
    pub async fn new() -> Self {
        Self::with_keep_alive(DEFAULT_KEEP_ALIVE).await
    }

    #[inline]
    pub async fn with_keep_alive(keep_alive: Duration) -> Self {
        let (socket_manager, keep_alive_receiver) =
            InternalSocketManager::with_keep_alive(keep_alive);

        // FIXME: the root cert store / client config should be provided by global config
        let root_cert_store = Arc::new(RootCertStore::from_iter(
            webpki_roots::TLS_SERVER_ROOTS.iter().cloned(),
        ));

        let tls_client_config = rustls::ClientConfig::builder()
            .with_root_certificates(root_cert_store.clone())
            .with_no_client_auth();
        let tls_client_config = Arc::new(tls_client_config);

        let quic_client_config = match quinn::ClientConfig::with_root_certificates(root_cert_store)
        {
            Ok(config) => Arc::new(config),
            Err(_) => todo!("handle config error"),
        };

        let socket_manager = Self {
            tls_client_config,
            quic_client_config,
            internal: Arc::new(RwLock::new(socket_manager)),
        };

        let join_handle = InternalSocketManager::start_garbage_collection(
            socket_manager.internal.clone(),
            keep_alive_receiver,
        );
        let mut w_isocket_manager = socket_manager.internal.write().await;
        w_isocket_manager.garbage_collection = Some(join_handle);
        drop(w_isocket_manager);

        socket_manager
    }

    #[inline]
    pub async fn set_keep_alive(&self, new_keep_alive: Duration) {
        let w_socket_manager = self.internal.write().await;
        w_socket_manager
            .keep_alive
            .send_if_modified(|current_keep_alive| {
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
    pub async fn get_udp_tcp(&self, address: IpAddr) -> Arc<MixedSocket> {
        let r_socket_manager = self.internal.read().await;
        if let Some((socket, _)) = r_socket_manager.udp_tcp_sockets.get(&address) {
            return socket.clone();
        }
        drop(r_socket_manager);

        let mut w_socket_manager = self.internal.write().await;
        match w_socket_manager.udp_tcp_sockets.entry(address) {
            Entry::Occupied(occupied_entry) => occupied_entry.get().0.clone(),
            Entry::Vacant(vacant_entry) => {
                let socket = MixedSocket::new(address);
                vacant_entry.insert((socket.clone(), 0));
                socket
            }
        }
    }

    /// # Cancel Safety
    ///
    /// This function is cancel safe.
    #[inline]
    pub async fn try_get_udp_tcp(&self, address: &IpAddr) -> Option<Arc<MixedSocket>> {
        let r_socket_manager = self.internal.read().await;
        let socket = r_socket_manager.udp_tcp_sockets.get(address).cloned();
        drop(r_socket_manager);
        socket.map(|(socket, _)| socket)
    }

    /// # Cancel Safety
    ///
    /// This function is cancel safe.
    #[inline]
    pub async fn get_all_udp_tcp(
        &self,
        addresses: impl Iterator<Item = IpAddr>,
    ) -> Vec<Arc<MixedSocket>> {
        let mut w_socket_manager = self.internal.write().await;
        let sockets = addresses
            .map(
                |address| match w_socket_manager.udp_tcp_sockets.entry(address) {
                    Entry::Occupied(occupied_entry) => occupied_entry.get().0.clone(),
                    Entry::Vacant(vacant_entry) => {
                        let socket = MixedSocket::new(address);
                        vacant_entry.insert((socket.clone(), 0));
                        socket
                    }
                },
            )
            .collect::<Vec<_>>();
        drop(w_socket_manager);
        sockets
    }

    /// # Cancel Safety
    ///
    /// This function is cancel safe.
    #[inline]
    pub async fn try_get_all_udp_tcp(
        &self,
        addresses: impl Iterator<Item = &IpAddr>,
    ) -> Vec<Arc<MixedSocket>> {
        let r_socket_manager = self.internal.read().await;
        let sockets = addresses
            .filter_map(|address| {
                r_socket_manager
                    .udp_tcp_sockets
                    .get(address)
                    .map(|(socket, _)| socket.clone())
            })
            .collect::<Vec<_>>();
        drop(r_socket_manager);
        sockets
    }

    #[inline]
    pub async fn for_each_udp_tcp<F>(&self, f: F)
    where
        Self: Sized,
        F: FnMut((&IpAddr, &Arc<MixedSocket>)),
    {
        let r_socket_manager = self.internal.read().await;
        r_socket_manager
            .udp_tcp_sockets
            .iter()
            .map(|(address, (socket, _))| (address, socket))
            .for_each(f);
        drop(r_socket_manager);
    }

    /// # Cancel Safety
    ///
    /// This function is cancel safe.
    #[inline]
    pub async fn get_tls(&self, address: (IpAddr, CDomainName)) -> Arc<TlsSocket> {
        let r_socket_manager = self.internal.read().await;
        if let Some((socket, _)) = r_socket_manager.tls_sockets.get(&address) {
            return socket.clone();
        }
        drop(r_socket_manager);

        let mut w_socket_manager = self.internal.write().await;
        match w_socket_manager.tls_sockets.entry(address) {
            Entry::Occupied(occupied_entry) => occupied_entry.get().0.clone(),
            Entry::Vacant(vacant_entry) => {
                let address = vacant_entry.key().0;
                let name = vacant_entry.key().1.clone();
                let socket = TlsSocket::new(address, name, self.tls_client_config.clone());
                vacant_entry.insert((socket.clone(), 0));
                socket
            }
        }
    }

    /// # Cancel Safety
    ///
    /// This function is cancel safe.
    #[inline]
    pub async fn try_get_tls(&self, address: &(IpAddr, CDomainName)) -> Option<Arc<TlsSocket>> {
        let r_socket_manager = self.internal.read().await;
        let socket = r_socket_manager.tls_sockets.get(address).cloned();
        drop(r_socket_manager);
        socket.map(|(socket, _)| socket)
    }

    /// # Cancel Safety
    ///
    /// This function is cancel safe.
    #[inline]
    pub async fn get_all_tls(
        &self,
        addresses: impl Iterator<Item = (IpAddr, CDomainName)>,
    ) -> Vec<Arc<TlsSocket>> {
        let mut w_socket_manager = self.internal.write().await;
        let sockets = addresses
            .map(
                |address| match w_socket_manager.tls_sockets.entry(address) {
                    Entry::Occupied(occupied_entry) => occupied_entry.get().0.clone(),
                    Entry::Vacant(vacant_entry) => {
                        let address = vacant_entry.key().0;
                        let name = vacant_entry.key().1.clone();
                        let socket = TlsSocket::new(address, name, self.tls_client_config.clone());
                        vacant_entry.insert((socket.clone(), 0));
                        socket
                    }
                },
            )
            .collect::<Vec<_>>();
        drop(w_socket_manager);
        sockets
    }

    /// # Cancel Safety
    ///
    /// This function is cancel safe.
    #[inline]
    pub async fn try_get_all_tls(
        &self,
        addresses: impl Iterator<Item = &(IpAddr, CDomainName)>,
    ) -> Vec<Arc<TlsSocket>> {
        let r_socket_manager = self.internal.read().await;
        let sockets = addresses
            .filter_map(|address| {
                r_socket_manager
                    .tls_sockets
                    .get(address)
                    .map(|(socket, _)| socket.clone())
            })
            .collect::<Vec<_>>();
        drop(r_socket_manager);
        sockets
    }

    #[inline]
    pub async fn for_each_tls<F>(&self, f: F)
    where
        Self: Sized,
        F: FnMut((&(IpAddr, CDomainName), &Arc<TlsSocket>)),
    {
        let r_socket_manager = self.internal.read().await;
        r_socket_manager
            .tls_sockets
            .iter()
            .map(|(address, (socket, _))| (address, socket))
            .for_each(f);
        drop(r_socket_manager);
    }

    /// # Cancel Safety
    ///
    /// This function is cancel safe.
    #[inline]
    pub async fn get_quic(&self, address: (IpAddr, CDomainName)) -> Arc<QuicSocket> {
        let r_socket_manager = self.internal.read().await;
        if let Some((socket, _)) = r_socket_manager.quic_sockets.get(&address) {
            return socket.clone();
        }
        drop(r_socket_manager);

        let mut w_socket_manager = self.internal.write().await;
        match w_socket_manager.quic_sockets.entry(address) {
            Entry::Occupied(occupied_entry) => occupied_entry.get().0.clone(),
            Entry::Vacant(vacant_entry) => {
                let address = vacant_entry.key().0;
                let name = vacant_entry.key().1.clone();
                let socket = QuicSocket::new(address, name, self.quic_client_config.clone());
                vacant_entry.insert((socket.clone(), 0));
                socket
            }
        }
    }

    /// # Cancel Safety
    ///
    /// This function is cancel safe.
    #[inline]
    pub async fn try_get_quic(&self, address: &(IpAddr, CDomainName)) -> Option<Arc<QuicSocket>> {
        let r_socket_manager = self.internal.read().await;
        let socket = r_socket_manager.quic_sockets.get(address).cloned();
        drop(r_socket_manager);
        socket.map(|(socket, _)| socket)
    }

    /// # Cancel Safety
    ///
    /// This function is cancel safe.
    #[inline]
    pub async fn get_all_quic(
        &self,
        addresses: impl Iterator<Item = (IpAddr, CDomainName)>,
    ) -> Vec<Arc<QuicSocket>> {
        let mut w_socket_manager = self.internal.write().await;
        let sockets = addresses
            .map(
                |address| match w_socket_manager.quic_sockets.entry(address) {
                    Entry::Occupied(occupied_entry) => occupied_entry.get().0.clone(),
                    Entry::Vacant(vacant_entry) => {
                        let address = vacant_entry.key().0;
                        let name = vacant_entry.key().1.clone();
                        let socket =
                            QuicSocket::new(address, name, self.quic_client_config.clone());
                        vacant_entry.insert((socket.clone(), 0));
                        socket
                    }
                },
            )
            .collect::<Vec<_>>();
        drop(w_socket_manager);
        sockets
    }

    /// # Cancel Safety
    ///
    /// This function is cancel safe.
    #[inline]
    pub async fn try_get_all_quic(
        &self,
        addresses: impl Iterator<Item = &(IpAddr, CDomainName)>,
    ) -> Vec<Arc<QuicSocket>> {
        let r_socket_manager = self.internal.read().await;
        let sockets = addresses
            .filter_map(|address| {
                r_socket_manager
                    .quic_sockets
                    .get(address)
                    .map(|(socket, _)| socket.clone())
            })
            .collect::<Vec<_>>();
        drop(r_socket_manager);
        sockets
    }

    #[inline]
    pub async fn for_each_quic<F>(&self, f: F)
    where
        Self: Sized,
        F: FnMut((&(IpAddr, CDomainName), &Arc<QuicSocket>)),
    {
        let r_socket_manager = self.internal.read().await;
        r_socket_manager
            .quic_sockets
            .iter()
            .map(|(address, (socket, _))| (address, socket))
            .for_each(f);
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
            let mut w_imanager = imanager.write().await;
            if let Some(garbage_collection) = &w_imanager.garbage_collection {
                garbage_collection.abort();
            }

            // Shutdown all of the sockets still being managed.
            futures::stream::iter(w_imanager.udp_tcp_sockets.drain())
                .for_each_concurrent(None, |(_, (socket, _))| async move {
                    let _ = socket.disable().await;
                })
                .await;
            futures::stream::iter(w_imanager.tls_sockets.drain())
                .for_each_concurrent(None, |(_, (socket, _))| async move {
                    let _ = socket.disable().await;
                })
                .await;
            drop(w_imanager);
        });
    }
}

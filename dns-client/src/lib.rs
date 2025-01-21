use std::{collections::HashMap, sync::Arc};

use async_lib::once_watch;
use async_trait::async_trait;
use dns_cache::asynchronous::{async_cache::AsyncTreeCache, async_main_cache::AsyncMainTreeCache};
use dns_lib::{interface::client::{Answer, AsyncClient, Context, Response}, query::question::Question, resource_record::rcode::RCode};
use log::info;
use network::socket_manager::SocketManager;
use query::recursive_query::recursive_query;
use result::{QOk, QResult};

mod qname_minimizer;
mod query;
mod network;
mod result;


pub struct DNSAsyncClient {
    cache: Arc<AsyncMainTreeCache>,
    socket_manager: SocketManager,
    active_queries: std::sync::RwLock<HashMap<Question, once_watch::Sender<QResult>>>,
}

impl DNSAsyncClient {
    #[inline]
    pub async fn new(cache: Arc<AsyncMainTreeCache>) -> Self {
        Self {
            cache,
            socket_manager: SocketManager::new().await,
            active_queries: std::sync::RwLock::new(HashMap::new()),
        }
    }

    #[inline]
    pub fn cache(&self) -> Arc<AsyncMainTreeCache> { self.cache.clone() }

    #[inline]
    pub async fn close(&self) {
        self.socket_manager.drop_all_sockets().await;
    }
}

#[async_trait]
impl AsyncClient for DNSAsyncClient {
    async fn query(client: Arc<Self>, context: Context) -> Response {
        info!("Start query '{}'", context.query());
        let joined_cache = Arc::new(AsyncTreeCache::new(client.cache.clone()));
        match recursive_query(client, joined_cache, context).await {
            QResult::Err(_) => Response::Error(RCode::ServFail),
            QResult::Fail(rcode) => Response::Error(rcode),
            QResult::Ok(QOk { answer, name_servers, additional }) => Response::Answer(Answer { answer, name_servers, additional, authoritative: false }),
        }
    }
}

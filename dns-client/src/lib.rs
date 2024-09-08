use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use dns_cache::asynchronous::{async_cache::AsyncTreeCache, async_main_cache::AsyncMainTreeCache};
use dns_lib::{interface::client::{Answer, AsyncClient, Context, Response}, query::question::Question, resource_record::resource_record::ResourceRecord};
use log::info;
use network::socket_manager::SocketManager;
use query::recursive_query::{recursive_query, QueryResponse};
use tokio::sync::{RwLock, broadcast::Sender};

mod query;

// Note: These should eventually be config options.
const IPV6_ENABLED: bool = false;
const IPV4_ENABLED: bool = true;

pub struct DNSAsyncClient {
    cache: Arc<AsyncMainTreeCache>,
    socket_manager: SocketManager,
    active_query_manager: RwLock<HashMap<Question, (Arc<Context>, Sender<QueryResponse<ResourceRecord>>)>>,
}

impl DNSAsyncClient {
    #[inline]
    pub async fn new(cache: Arc<AsyncMainTreeCache>) -> Self {
        Self {
            cache,
            socket_manager: SocketManager::new().await,
            active_query_manager: RwLock::new(HashMap::new())
        }
    }
}

#[async_trait]
impl AsyncClient for DNSAsyncClient {
    async fn query(client: Arc<Self>, context: Context) -> Response {
        info!("Start query '{}'", context.query());
        let joined_cache = Arc::new(AsyncTreeCache::new(client.cache.clone()));
        match recursive_query(client, joined_cache, context).await {
            QueryResponse::Error(rcode) => Response::Error(rcode),
            QueryResponse::NoRecords => Response::Answer(Answer { records: vec![], authoritative: false }),
            QueryResponse::Records(records) => Response::Answer(Answer { records, authoritative: false }),
        }
    }
}

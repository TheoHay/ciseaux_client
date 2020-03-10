use crate::{ConnectionsCount, ReconnectBehavior};

use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::{Mutex, MutexGuard, RwLock};

use redis::RedisError;
// MOVE => redirect_node and is_cluster_error

pub struct ClusterBuilder {
    clients: Vec<redis::Client>,
    conns_count: Option<ConnectionsCount>,
    reconnect_behavior: Option<ReconnectBehavior>,
}

impl ClusterBuilder {
    pub fn new(clients: Vec<redis::Client>) -> ClusterBuilder {
        ClusterBuilder {
            clients,
            conns_count: None,
            reconnect_behavior: None,
        }
    }

    /// Changes the number of connections in the pool.
    /// Default is 4 per threads
    pub fn set_conns_count(mut self, conns_count: ConnectionsCount) -> ClusterBuilder {
        self.conns_count = Some(conns_count);
        self
    }

    /// Change the reconnect behavior.
    /// Default is one ReconnectBehavior::InstantRetry
    pub fn set_reconnect_behavior(
        mut self,
        reconnect_behavior: ReconnectBehavior,
    ) -> ClusterBuilder {
        self.reconnect_behavior = Some(reconnect_behavior);
        self
    }

    // pub async fn build(self) -> Result<ClusterBuilder, RedisError> {
    //     for c in self.clients {
    //         let conn = match c.get_async_connection().await {
    //             Ok(v) => v,
    //             Err(_) => continue,
    //         };
    //     }
    //     unimplemented!()
    // }
}

pub struct CiseauxCluster<'a> {
    conns: Arc<RwLock<BTreeMap<&'a str, NodeConns>>>,
}

struct NodeConns {
    master: Vec<Mutex<redis::aio::Connection>>,
    replicas: Vec<Mutex<redis::aio::Connection>>,
}

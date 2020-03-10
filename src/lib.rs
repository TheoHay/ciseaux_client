//! This is a simple asynchronous Redis Pool, currently supporting only a connection pool to a Single Redis instance, and will probably provide Cluster support later.
//! If you want to understand how to use it, see examples and/or CiseauxSingle struct.
//!
//! The library currently support tokio only (Because of redis-rs), and require at least Rust 1.39

pub use redis;

mod cluster;
mod single;
pub use cluster::CiseauxCluster;
pub use cluster::ClusterBuilder;
pub use single::CiseauxSingle;
pub use single::SingleBuilder;

use std::time::Duration;

const DEFAULT_CONN_PER_THREAD: usize = 4;

/// To change the default pool size
pub enum ConnectionsCount {
    /// This will define the entire pool size
    Global(usize),
    /// This will define the pool size using the provided usize * cpu cores (Including virtual ones)
    PerThread(usize),
}

/// To change the default behavior of the pool on network/io error.
pub enum ReconnectBehavior {
    /// Will just return a RedisError, and the entire connection pool will not work after a disconnection, you should probably not use this
    NoReconnect,
    /// This is the default one, on error it will drop the previous connection and create a new one instantly, then retry the redis command, if it keeps failing, then it will just return a RedisError
    InstantRetry,
    /// Try to reconnect, but on fail, it will wait (2 seconds by default), then try again, if it keeps failing, then return a RedisError
    RetryWaitRetry(Option<Duration>),
}

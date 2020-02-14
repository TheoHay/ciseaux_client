//! This is a simple asynchronous Redis Pool, currently supporting only a connection pool to a Single Redis instance, and will probably provide Cluster support later.
//! If you want to understand how to use it, see examples and/or CiseauxSingle struct.
//!
//! The library currently support tokio only (Because of redis-rs), and require at least Rust 1.39

pub use redis;

mod single;
pub use single::CiseauxSingle;

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
    /// Use this when you need a full custom behavior, you can toggle off instant retry and change the number of retries, and delay between them
    Custom {
        instant_retry: bool,
        delays: Option<Vec<Duration>>,
    },
    /// Infinite will try to reconnect instantly, and on fail, try to reconnect after a delay (2 seconds by default)
    /// It's like an infinite loop, use this only if you can't fail a query and you can wait for it.
    /// But, for example, on a web server, you will accumulate requests/connections and may run out of memory.
    Infinite(Option<Duration>),
}

//! This is a simple asynchronous Redis Pool, currently only supports a connection pool to a Single Redis instance, and will probably provide Cluster support later.
//! If you want to understand how to use it, see examples and/or CiseauxSingle struct.
//!
//! The library currently supports tokio only (Because of redis-rs, async-std support is coming), and require at least Rust 1.39
//!
//! ```toml
//! [dependencies]
//! ciseaux_client = "0.3"
//! ```
//!
//! # Example
//!
//! Create a connection pool default settings and the provided redis::Client (from [redis-rs](https://crates.io/crates/redis))
//!
//! ```rust
//! use redis::{Client, Cmd};
//! use ciseaux_client::CiseauxSingle;
//!
//! #[tokio::main]
//! async fn main() {
//!     let redis_client = Client::open("redis://127.0.0.1:6379").unwrap();
//!     let db_pool = CiseauxSingle::new(redis_client).await.expect("Failed to create Pool");
//!     // You can safely share CiseauxSingle between threads
//!     // since it use Arcs and Mutexes under the hood)
//!     let res = match db_pool.query::<_, Option<String>>(&redis::Cmd::get("hello")).await {
//!         Ok(v) => v,
//!         Err(e) => return,// Handle Error
//!     };
//!     let hello = match res {
//!         Some(v) => v,
//!         None => return,// Handle Nil value
//!     };
//!     println!("{}", hello);
//! }
//! ```
//!

pub use redis;

mod cluster;
mod single;
#[cfg(test)]
mod tests;
pub use single::CiseauxSingle;
pub use single::SingleInit;

use std::time::Duration;

const DEFAULT_CONNS_COUNT: ConnectionsCount = ConnectionsCount::Global(4);
const DEFAULT_RECONNECT_BEHAVIOR: ReconnectBehavior = ReconnectBehavior::InstantRetry;
const DEFAULT_WAIT_RETRY_DUR: Duration = Duration::from_secs(2);

/// To change the default pool size
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ConnectionsCount {
    /// This will define the entire pool size
    Global(usize),
    /// This will define the pool size using the provided usize * cpu cores (Including virtual ones)
    PerThread(usize),
}

impl std::default::Default for ConnectionsCount {
    fn default() -> Self {
        DEFAULT_CONNS_COUNT
    }
}

impl ConnectionsCount {
    fn into_flat(self) -> usize {
        match self {
            ConnectionsCount::Global(c) => c,
            ConnectionsCount::PerThread(c) => num_cpus::get() * c,
        }
    }
}

/// To change the default behavior of the pool on network/io error.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ReconnectBehavior {
    /// Will just return a RedisError, and the entire connection pool will not work after a disconnection, you should probably not use this
    NoReconnect,
    /// This is the default one, on error it will drop the previous connection and create a new one instantly, then retry the redis command, if it keeps failing, then it will just return a RedisError
    InstantRetry,
    /// Try to reconnect, but on fail, it will wait (2 seconds by default), then try again, if it keeps failing, then return a RedisError
    RetryWaitRetry(Option<Duration>),
}

impl std::default::Default for ReconnectBehavior {
    fn default() -> Self {
        DEFAULT_RECONNECT_BEHAVIOR
    }
}

/// A trait that allow to have a single CiseauxSingle query, and not
/// a query_x per redis commands types (redis::Cmd and redis::Pipeline).
/// Implemented for redis::Cmd and redis::Pipeline (including &, and &mut)
#[async_trait::async_trait]
pub trait QueryAble {
    async fn query<T: redis::FromRedisValue>(
        &self,
        conn: &mut redis::aio::Connection,
    ) -> Result<T, redis::RedisError>;
}

#[async_trait::async_trait]
impl QueryAble for redis::Cmd {
    async fn query<T: redis::FromRedisValue>(
        &self,
        conn: &mut redis::aio::Connection,
    ) -> Result<T, redis::RedisError> {
        self.query_async::<redis::aio::Connection, T>(conn).await
    }
}

#[async_trait::async_trait]
impl QueryAble for &redis::Cmd {
    async fn query<T: redis::FromRedisValue>(
        &self,
        conn: &mut redis::aio::Connection,
    ) -> Result<T, redis::RedisError> {
        self.query_async::<redis::aio::Connection, T>(conn).await
    }
}

#[async_trait::async_trait]
impl QueryAble for &mut redis::Cmd {
    async fn query<T: redis::FromRedisValue>(
        &self,
        conn: &mut redis::aio::Connection,
    ) -> Result<T, redis::RedisError> {
        self.query_async::<redis::aio::Connection, T>(conn).await
    }
}

#[async_trait::async_trait]
impl QueryAble for redis::Pipeline {
    async fn query<T: redis::FromRedisValue>(
        &self,
        conn: &mut redis::aio::Connection,
    ) -> Result<T, redis::RedisError> {
        self.query_async::<redis::aio::Connection, T>(conn).await
    }
}

#[async_trait::async_trait]
impl QueryAble for &redis::Pipeline {
    async fn query<T: redis::FromRedisValue>(
        &self,
        conn: &mut redis::aio::Connection,
    ) -> Result<T, redis::RedisError> {
        self.query_async::<redis::aio::Connection, T>(conn).await
    }
}

#[async_trait::async_trait]
impl QueryAble for &mut redis::Pipeline {
    async fn query<T: redis::FromRedisValue>(
        &self,
        conn: &mut redis::aio::Connection,
    ) -> Result<T, redis::RedisError> {
        self.query_async::<redis::aio::Connection, T>(conn).await
    }
}

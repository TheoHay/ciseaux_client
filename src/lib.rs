//! This is a simple asynchronous Redis Pool, currently supporting only a connection pool to a Single Redis instance, and will probably provide Cluster support later.
//! If you want to understand how to use it, see examples and/or CiseauxSingle struct.
//!
//! The library currently support tokio only (Because of redis-rs), and require at least Rust 1.39

pub use redis;

mod single;
pub use single::CiseauxSingle;

const DEFAULT_CONN_PER_THREAD: usize = 4;

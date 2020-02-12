//! This is a simple Redis Pool, currently supporting only a connection pool to a Single Redis instance
//! If you want to understand how to use it, see examples and/or CiseauxSingle

pub use redis;

mod single;
pub use single::CiseauxSingle;

const DEFAULT_CONN_PER_THREAD: usize = 4;

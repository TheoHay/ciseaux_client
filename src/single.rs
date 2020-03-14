use crate::{ConnectionsCount, QueryAble, ReconnectBehavior};

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tokio::sync::{Mutex, MutexGuard};

use redis::RedisError;

/// An Init Struct to create a customized CiseauxSingle connections pool.
/// This is like a Builder, but using public fields instead of functions
#[derive(Debug)]
pub struct SingleInit {
    /// The redis-rs Client CiseauxSingle will use
    pub client: redis::Client,
    /// By default, 4 connections per Thread
    pub conns_count: ConnectionsCount,
    /// By default, Instant Retry
    pub reconnect_behavior: ReconnectBehavior,
}

impl SingleInit {
    /// This create a SingleInit with default settings and the provided redis::Client
    pub fn new(client: redis::Client) -> SingleInit {
        SingleInit {
            client,
            conns_count: ConnectionsCount::default(),
            reconnect_behavior: ReconnectBehavior::default(),
        }
    }

    /// Like SingleInit::new, but also opens a redis::Client on localhost (With redis default port : 6379)
    pub fn default_localhost() -> SingleInit {
        SingleInit {
            client: redis::Client::open("redis://127.0.0.1:6379").unwrap(), // Unwrap is OK since client open doesn't connect, but only checks URL Validity.
            conns_count: ConnectionsCount::default(),
            reconnect_behavior: ReconnectBehavior::default(),
        }
    }

    /// Asynchronously creates multiple connexions to a Redis instance
    pub async fn build(self) -> Result<CiseauxSingle, RedisError> {
        let conns_count = self.conns_count.into_flat();
        let mut conns_fut = Vec::with_capacity(conns_count);
        for _ in 0..conns_count {
            conns_fut.push(self.client.get_async_connection());
        }
        let mut conns = Vec::with_capacity(conns_count);
        for c in futures::future::join_all(conns_fut).await {
            conns.push(Mutex::new(c?));
        }
        Ok(CiseauxSingle {
            client: Arc::new(self.client),
            reconnect_behavior: self.reconnect_behavior,
            conns: Arc::new(conns),
            next: Arc::new(AtomicUsize::new(0)),
        })
    }
}

/// A connections pool to a single Redis instance
#[derive(Clone)]
pub struct CiseauxSingle {
    client: Arc<redis::Client>,
    reconnect_behavior: ReconnectBehavior,
    conns: Arc<Vec<Mutex<redis::aio::Connection>>>,
    next: Arc<AtomicUsize>,
}

impl CiseauxSingle {
    /// Shortcut to SingleInit::new
    pub fn builder(client: redis::Client) -> SingleInit {
        SingleInit::new(client)
    }

    /// Shortcut to SingleInit::new
    pub fn init(client: redis::Client) -> SingleInit {
        SingleInit::new(client)
    }

    /// Create a new pool using defaults settings
    pub async fn new(client: redis::Client) -> Result<CiseauxSingle, RedisError> {
        SingleInit::new(client).build().await
    }

    /// Asynchronously query QueryAble (trait, implemented for redis::Cmd and redis::Pipeline),
    /// but in case of network error, will try to reconnect once to the same database (by default),
    /// or follow the reconnect_behavior you provided
    pub async fn query<C: QueryAble, T: redis::FromRedisValue>(
        &self,
        cmd: C,
    ) -> Result<T, RedisError> {
        let mut conn = self.conns[self.next.fetch_add(1, Ordering::AcqRel) % self.conns.len()]
            .lock()
            .await;
        match cmd.query::<T>(&mut conn).await {
            Ok(v) => Ok(v),
            Err(e) => {
                if is_network_or_io_error(&e) {
                    if self.reconnect_behavior == ReconnectBehavior::NoReconnect {
                        return Err(e);
                    }
                    return self.retry_cmd(&mut conn, cmd).await;
                }
                return Err(e);
            }
        }
    }

    #[inline(always)]
    async fn retry_cmd<'a, C: QueryAble, T: redis::FromRedisValue>(
        &self,
        conn: &mut MutexGuard<'a, redis::aio::Connection>,
        cmd: C,
    ) -> Result<T, RedisError> {
        match self.try_reconnect(conn).await {
            Ok(()) => return cmd.query::<T>(conn).await,
            Err(e) => {
                if is_network_or_io_error(&e) {
                    match self.reconnect_behavior {
                        ReconnectBehavior::RetryWaitRetry(d) => {
                            tokio::time::delay_for(d.unwrap_or(crate::DEFAULT_WAIT_RETRY_DUR))
                                .await;
                            self.try_reconnect(conn).await?;
                            return cmd.query::<T>(conn).await;
                        }
                        _ => return Err(e),
                    }
                }
                return Err(e);
            }
        }
    }

    #[inline(always)]
    async fn try_reconnect<'a>(
        &self,
        conn: &mut MutexGuard<'a, redis::aio::Connection>,
    ) -> Result<(), RedisError> {
        match self.client.get_async_connection().await {
            Ok(c) => {
                **conn = c;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}

fn is_network_or_io_error(error: &RedisError) -> bool {
    if error.is_timeout() || error.is_connection_dropped() || error.is_io_error() {
        return true;
    }
    false
}

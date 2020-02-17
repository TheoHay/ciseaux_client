use crate::{ConnectionsCount, ReconnectBehavior};

use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::{Mutex, MutexGuard};

use redis::RedisError;

/// A builder to customize CiseauxSingle
pub struct SingleBuilder {
    client: redis::Client,
    conns_count: Option<ConnectionsCount>,
    reconnect_behavior: Option<ReconnectBehavior>,
}

impl SingleBuilder {
    /// Use this to create a CiseauxSingle struct, client must be a redis::Client (from redis-rs)
    pub fn new(client: redis::Client) -> SingleBuilder {
        SingleBuilder {
            client,
            conns_count: None,
            reconnect_behavior: None,
        }
    }

    /// Changes the number of connections in the pool.
    /// Default is 4 per threads
    pub fn set_conns_count(mut self, conns_count: ConnectionsCount) -> SingleBuilder {
        self.conns_count = Some(conns_count);
        self
    }

    /// Change the reconnect behavior.
    /// Default is one ReconnectBehavior::InstantRetry
    pub fn set_reconnect_behavior(
        mut self,
        reconnect_behavior: ReconnectBehavior,
    ) -> SingleBuilder {
        self.reconnect_behavior = Some(reconnect_behavior);
        self
    }

    pub async fn build(self) -> Result<CiseauxSingle, RedisError> {
        let conns_count = match self.conns_count {
            Some(v) => match v {
                ConnectionsCount::Global(v) => v,
                ConnectionsCount::PerThread(v) => num_cpus::get() * v,
            },
            None => num_cpus::get() * crate::DEFAULT_CONN_PER_THREAD,
        };
        let mut conns_fut = Vec::with_capacity(conns_count);
        for _ in 0..conns_count {
            conns_fut.push(self.client.get_async_connection());
        }
        let mut conns = Vec::with_capacity(conns_count);
        for c in futures::future::join_all(conns_fut).await {
            conns.push(Arc::new(Mutex::new(c?)));
        }
        Ok(CiseauxSingle {
            client: Arc::new(self.client),
            reconnect_behavior: Arc::new(match self.reconnect_behavior {
                Some(v) => v,
                None => ReconnectBehavior::InstantRetry,
            }),
            conns: Arc::new(conns),
            next: Arc::new(AtomicUsize::new(0)),
        })
    }
}

#[derive(Clone)]
pub struct CiseauxSingle {
    client: Arc<redis::Client>,
    reconnect_behavior: Arc<ReconnectBehavior>,
    conns: Arc<Vec<Arc<Mutex<redis::aio::Connection>>>>,
    next: Arc<AtomicUsize>,
}

impl CiseauxSingle {
    pub fn builder(client: redis::Client) -> SingleBuilder {
        SingleBuilder::new(client)
    }

    /// Create a new pool using the builder's defaults
    pub async fn new(client: redis::Client) -> Result<CiseauxSingle, RedisError> {
        SingleBuilder::new(client).build().await
    }

    /// Asynchronously query a redis::Cmd, but in case of network error, will try to reconnect once to the same database, then fail the command.
    pub async fn query_cmd<T: redis::FromRedisValue>(
        &self,
        cmd: &redis::Cmd,
    ) -> Result<T, RedisError> {
        let mut conn = self.conns[self.next.fetch_add(1, Ordering::AcqRel) % self.conns.len()]
            .lock()
            .await;
        match cmd
            .query_async::<redis::aio::Connection, T>(&mut conn)
            .await
        {
            Ok(v) => Ok(v),
            Err(e) => {
                if is_network_or_io_error(&e) {
                    match *self.reconnect_behavior {
                        ReconnectBehavior::NoReconnect => return Err(e),
                        ReconnectBehavior::InstantRetry => match self.try_reconnect(conn).await {
                            Ok(mut c) => {
                                return cmd.query_async::<redis::aio::Connection, T>(&mut c).await
                            }
                            Err((e, _)) => return Err(e),
                        },
                        ReconnectBehavior::RetryWaitRetry(d) => {
                            match self.try_reconnect(conn).await {
                                Ok(mut c) => {
                                    match cmd.query_async::<redis::aio::Connection, T>(&mut c).await
                                    {
                                        Ok(v) => return Ok(v),
                                        Err(e) => {
                                            if is_network_or_io_error(&e) {
                                                tokio::time::delay_for(
                                                    d.unwrap_or(Duration::from_secs(2)),
                                                )
                                                .await;
                                                match self.try_reconnect(c).await {
                                                    Ok(mut c) => return cmd
                                                        .query_async::<redis::aio::Connection, T>(
                                                            &mut c,
                                                        )
                                                        .await,
                                                    Err((e, _)) => return Err(e),
                                                }
                                            } else {
                                                return Err(e);
                                            }
                                        }
                                    }
                                }
                                Err((e, c)) => {
                                    if is_network_or_io_error(&e) {
                                        tokio::time::delay_for(d.unwrap_or(Duration::from_secs(2)))
                                            .await;
                                        match self.try_reconnect(c).await {
                                            Ok(mut c) => {
                                                return cmd
                                                    .query_async::<redis::aio::Connection, T>(
                                                        &mut c,
                                                    )
                                                    .await
                                            }
                                            Err((e, _)) => return Err(e),
                                        }
                                    } else {
                                        return Err(e);
                                    }
                                }
                            }
                        }
                    }
                }
                return Err(e);
            }
        }
    }

    /// Same that query_cmd, but with a redis::Pipeline
    pub async fn query_pipe<T: redis::FromRedisValue>(
        &self,
        pipe: &redis::Pipeline,
    ) -> Result<T, RedisError> {
        let mut conn = self.conns[self.next.fetch_add(1, Ordering::AcqRel) % self.conns.len()]
            .lock()
            .await;
        match pipe
            .query_async::<redis::aio::Connection, T>(&mut conn)
            .await
        {
            Ok(v) => Ok(v),
            Err(e) => {
                if is_network_or_io_error(&e) {
                    match *self.reconnect_behavior {
                        ReconnectBehavior::NoReconnect => return Err(e),
                        ReconnectBehavior::InstantRetry => match self.try_reconnect(conn).await {
                            Ok(mut c) => {
                                return pipe.query_async::<redis::aio::Connection, T>(&mut c).await
                            }
                            Err((e, _)) => return Err(e),
                        },
                        ReconnectBehavior::RetryWaitRetry(d) => {
                            match self.try_reconnect(conn).await {
                                Ok(mut c) => {
                                    match pipe
                                        .query_async::<redis::aio::Connection, T>(&mut c)
                                        .await
                                    {
                                        Ok(v) => return Ok(v),
                                        Err(e) => {
                                            if is_network_or_io_error(&e) {
                                                tokio::time::delay_for(
                                                    d.unwrap_or(Duration::from_secs(2)),
                                                )
                                                .await;
                                                match self.try_reconnect(c).await {
                                                    Ok(mut c) => return pipe
                                                        .query_async::<redis::aio::Connection, T>(
                                                            &mut c,
                                                        )
                                                        .await,
                                                    Err((e, _)) => return Err(e),
                                                }
                                            } else {
                                                return Err(e);
                                            }
                                        }
                                    }
                                }
                                Err((e, c)) => {
                                    if is_network_or_io_error(&e) {
                                        tokio::time::delay_for(d.unwrap_or(Duration::from_secs(2)))
                                            .await;
                                        match self.try_reconnect(c).await {
                                            Ok(mut c) => {
                                                return pipe
                                                    .query_async::<redis::aio::Connection, T>(
                                                        &mut c,
                                                    )
                                                    .await
                                            }
                                            Err((e, _)) => return Err(e),
                                        }
                                    } else {
                                        return Err(e);
                                    }
                                }
                            }
                        }
                    }
                }
                return Err(e);
            }
        }
    }

    async fn try_reconnect<'a>(
        &self,
        mut conn: MutexGuard<'a, redis::aio::Connection>,
    ) -> Result<
        MutexGuard<'a, redis::aio::Connection>,
        (RedisError, MutexGuard<'a, redis::aio::Connection>),
    > {
        match self.client.get_async_connection().await {
            Ok(c) => {
                *conn = c;
                Ok(conn)
            }
            Err(e) => Err((e, conn)),
        }
    }
}

fn is_network_or_io_error(error: &RedisError) -> bool {
    if error.is_timeout() || error.is_connection_dropped() || error.is_io_error() {
        return true;
    }
    false
}

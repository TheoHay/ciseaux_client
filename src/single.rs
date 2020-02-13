use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tokio::sync::{Mutex, MutexGuard};

use redis::RedisError;

#[derive(Clone)]
pub struct CiseauxSingle {
    client: Arc<redis::Client>,
    conns: Arc<Vec<Arc<Mutex<redis::aio::Connection>>>>,
    next: Arc<AtomicUsize>,
}

impl CiseauxSingle {
    /// Creates a new redis pool instance from redis-rs library Client struct (use ciseaux_client::redis or add redis to you depencies)
    /// Count change the number of connections for this pool, by default (When None is provided), it creates 4 connections per CPU cores
    pub async fn new(
        client: redis::Client,
        conns_count: Option<usize>,
    ) -> Result<CiseauxSingle, redis::RedisError> {
        let conns_count = match conns_count {
            Some(v) => v,
            None => num_cpus::get() * crate::DEFAULT_CONN_PER_THREAD,
        };
        let mut conns_fut = Vec::with_capacity(conns_count);
        for _ in 0..conns_count {
            conns_fut.push(client.get_async_connection());
        }
        let mut conns = Vec::with_capacity(conns_count);
        for c in futures::future::join_all(conns_fut).await {
            conns.push(Arc::new(Mutex::new(c?)));
        }
        Ok(CiseauxSingle {
            client: Arc::new(client),
            conns: Arc::new(conns),
            next: Arc::new(AtomicUsize::new(0)),
        })
    }

    /// This will query a redis::Cmd, but in case of network error, will try to reconnect once to the same database, then fail the command.
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
                if e.is_timeout() || e.is_connection_dropped() || e.is_io_error() {
                    match self.try_reconnect(conn).await {
                        Ok(mut c) => {
                            return cmd.query_async::<redis::aio::Connection, T>(&mut c).await
                        }
                        Err(e) => return Err(e),
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
                if e.is_timeout() || e.is_connection_dropped() || e.is_io_error() {
                    match self.try_reconnect(conn).await {
                        Ok(mut c) => {
                            return pipe.query_async::<redis::aio::Connection, T>(&mut c).await
                        }
                        Err(e) => return Err(e),
                    }
                }
                return Err(e);
            }
        }
    }

    async fn try_reconnect<'a>(
        &self,
        mut conn: MutexGuard<'a, redis::aio::Connection>,
    ) -> Result<MutexGuard<'a, redis::aio::Connection>, RedisError> {
        match self.client.get_async_connection().await {
            Ok(c) => {
                *conn = c;
                Ok(conn)
            }
            Err(e) => Err(e),
        }
    }
}

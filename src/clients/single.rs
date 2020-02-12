use crate::CiseauxError;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tokio::sync::{Mutex, MutexGuard};

use redis::RedisError;

pub struct CiseauxSingle {
    client: Arc<redis::Client>,
    conns: Arc<Vec<Arc<Mutex<redis::aio::Connection>>>>,
    next: Arc<AtomicUsize>,
}

impl CiseauxSingle {
    pub async fn new(
        client: redis::Client,
        count: Option<usize>,
    ) -> Result<CiseauxSingle, redis::RedisError> {
        let count = match count {
            Some(v) => v,
            None => num_cpus::get() * crate::DEFAULT_CONN_PER_THREAD,
        };
        let mut conns_fut = Vec::with_capacity(count);
        for _ in 0..count {
            conns_fut.push(client.get_async_connection());
        }
        let mut conns = Vec::with_capacity(count);
        for c in futures::future::join_all(conns_fut).await {
            conns.push(Arc::new(Mutex::new(c?)));
        }
        Ok(CiseauxSingle {
            client: Arc::new(client),
            conns: Arc::new(conns),
            next: Arc::new(AtomicUsize::new(0)),
        })
    }

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

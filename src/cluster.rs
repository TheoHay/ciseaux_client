use redis::RedisError;

#[derive(Debug)]
enum CiseauxError {
    Redis(RedisError),
    InvalidCmd,
}

fn is_read_only(cmd: &redis::Cmd) -> Result<bool, CiseauxError> {
    match cmd.args_iter().take(1).next() {
        None => Err(CiseauxError::InvalidCmd),
        Some(a) => match a {
            redis::Arg::Cursor => Err(CiseauxError::InvalidCmd),
            redis::Arg::Simple(bytes) => match &*String::from_utf8_lossy(bytes).to_uppercase() {
                "GET"
                | "LRANGE"
                | "LOLWUT"
                | "RANDOMKEY"
                | "SYNC"
                | "MEMORY"
                | "SSCAN"
                | "GEOHASH"
                | "EXISTS"
                | "OBJECT"
                | "GETRANGE"
                | "SUNION"
                | "XREVRANGE"
                | "GEODIST"
                | "GEORADIUSBYMEMBER_RO"
                | "HGETALL"
                | "HVALS"
                | "PSYNC"
                | "XREAD"
                | "ZRANGEBYSCORE"
                | "ZRANGEBYLEX"
                | "SISMEMBER"
                | "HLEN"
                | "XINFO"
                | "HEXISTS"
                | "ZREVRANGE"
                | "HSCAN"
                | "LLEN"
                | "GEOPOS"
                | "TOUCH"
                | "ZSCAN"
                | "SRANDMEMBER"
                | "SCARD"
                | "TYPE"
                | "LINDEX"
                | "ZREVRANGEBYSCORE"
                | "TTL"
                | "SUBSTR"
                | "HKEYS"
                | "KEYS"
                | "SINTER"
                | "STRLEN"
                | "XPENDING"
                | "ZSCORE"
                | "PFCOUNT"
                | "XLEN"
                | "GETBIT"
                | "ZLEXCOUNT"
                | "HGET"
                | "HSTRLEN"
                | "GEORADIUS_RO"
                | "SCAN"
                | "BITCOUNT"
                | "PTTL"
                | "ZRANK"
                | "DBSIZE"
                | "DUMP"
                | "BITPOS"
                | "HMGET"
                | "SMEMBERS"
                | "ZREVRANK"
                | "MGET"
                | "ZCARD" => Ok(true),
                _ => Ok(false),
                //"ECHO" => Ok(false), IDK why it isn't marked as read only ?? (Using COMMAND on redis 5)
            },
        },
    }
}

#[derive(Debug)]
pub struct ClusterInit {
    pub auto_redirect_read: bool,
}

#[derive(Clone)]
struct CiseauxCluster {
    auto_redirect_read: bool,
}

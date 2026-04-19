//! Redis List command builders.

use super::{
    command::CommandStatementBuilder,
    key::{IntoRedisKey, RedisKey},
    resp::RespCommand,
    value::ToRedisBytes,
};

/// Entry point for Redis List commands.
pub struct ListCommand;

impl ListCommand {
    /// Build an LPUSH command.
    pub fn lpush(key: impl IntoRedisKey) -> LPushBuilder {
        LPushBuilder { cmd: b"LPUSH", key: key.into_redis_key(), values: Vec::new() }
    }

    /// Build an RPUSH command.
    pub fn rpush(key: impl IntoRedisKey) -> LPushBuilder {
        LPushBuilder { cmd: b"RPUSH", key: key.into_redis_key(), values: Vec::new() }
    }

    /// Build an LPUSHX command (push only if list exists).
    pub fn lpushx(key: impl IntoRedisKey) -> LPushBuilder {
        LPushBuilder { cmd: b"LPUSHX", key: key.into_redis_key(), values: Vec::new() }
    }

    /// Build an RPUSHX command (push only if list exists).
    pub fn rpushx(key: impl IntoRedisKey) -> LPushBuilder {
        LPushBuilder { cmd: b"RPUSHX", key: key.into_redis_key(), values: Vec::new() }
    }

    /// Build an LPOP command.
    pub fn lpop(key: impl IntoRedisKey, count: Option<u64>) -> LPopStatement {
        LPopStatement { cmd: b"LPOP", key: key.into_redis_key(), count }
    }

    /// Build an RPOP command.
    pub fn rpop(key: impl IntoRedisKey, count: Option<u64>) -> LPopStatement {
        LPopStatement { cmd: b"RPOP", key: key.into_redis_key(), count }
    }

    /// Build an LRANGE command.
    pub fn lrange(key: impl IntoRedisKey, start: i64, stop: i64) -> LRangeStatement {
        LRangeStatement { key: key.into_redis_key(), start, stop }
    }

    /// Build an LLEN command.
    pub fn llen(key: impl IntoRedisKey) -> LLenStatement {
        LLenStatement { key: key.into_redis_key() }
    }

    /// Build an LINDEX command.
    pub fn lindex(key: impl IntoRedisKey, index: i64) -> LIndexStatement {
        LIndexStatement { key: key.into_redis_key(), index }
    }

    /// Build an LREM command.
    pub fn lrem(key: impl IntoRedisKey, count: i64, value: impl ToRedisBytes) -> LRemStatement {
        LRemStatement {
            key: key.into_redis_key(),
            count,
            value: value.to_redis_bytes(),
        }
    }
}

/// Builder for LPUSH/RPUSH/LPUSHX/RPUSHX commands.
#[derive(Debug)]
pub struct LPushBuilder {
    cmd: &'static [u8],
    key: RedisKey,
    values: Vec<Vec<u8>>,
}

impl LPushBuilder {
    /// Add a value to push.
    pub fn value(mut self, v: impl ToRedisBytes) -> Self {
        self.values.push(v.to_redis_bytes());
        self
    }
}

impl CommandStatementBuilder for LPushBuilder {
    fn build(&self) -> RespCommand {
        let mut args = vec![self.cmd.to_vec(), self.key.to_bytes()];
        args.extend(self.values.iter().cloned());
        RespCommand::new(args)
    }
}

/// Builder for LPOP/RPOP commands.
#[derive(Debug)]
pub struct LPopStatement {
    cmd: &'static [u8],
    key: RedisKey,
    count: Option<u64>,
}

impl CommandStatementBuilder for LPopStatement {
    fn build(&self) -> RespCommand {
        let mut args = vec![self.cmd.to_vec(), self.key.to_bytes()];
        if let Some(n) = self.count {
            args.push(n.to_string().into_bytes());
        }
        RespCommand::new(args)
    }
}

/// Builder for LRANGE command.
#[derive(Debug)]
pub struct LRangeStatement {
    key: RedisKey,
    start: i64,
    stop: i64,
}

impl CommandStatementBuilder for LRangeStatement {
    fn build(&self) -> RespCommand {
        RespCommand::new(vec![
            b"LRANGE".to_vec(),
            self.key.to_bytes(),
            self.start.to_string().into_bytes(),
            self.stop.to_string().into_bytes(),
        ])
    }
}

/// Builder for LLEN command.
#[derive(Debug)]
pub struct LLenStatement {
    key: RedisKey,
}

impl CommandStatementBuilder for LLenStatement {
    fn build(&self) -> RespCommand {
        RespCommand::new(vec![b"LLEN".to_vec(), self.key.to_bytes()])
    }
}

/// Builder for LINDEX command.
#[derive(Debug)]
pub struct LIndexStatement {
    key: RedisKey,
    index: i64,
}

impl CommandStatementBuilder for LIndexStatement {
    fn build(&self) -> RespCommand {
        RespCommand::new(vec![
            b"LINDEX".to_vec(),
            self.key.to_bytes(),
            self.index.to_string().into_bytes(),
        ])
    }
}

/// Builder for LREM command.
#[derive(Debug)]
pub struct LRemStatement {
    key: RedisKey,
    count: i64,
    value: Vec<u8>,
}

impl CommandStatementBuilder for LRemStatement {
    fn build(&self) -> RespCommand {
        RespCommand::new(vec![
            b"LREM".to_vec(),
            self.key.to_bytes(),
            self.count.to_string().into_bytes(),
            self.value.clone(),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;

    #[rstest]
    fn test_lpush() {
        let cmd = ListCommand::lpush("mylist").value("a").value("b").build();
        assert_eq!(cmd.args(), &[
            b"LPUSH".to_vec(), b"mylist".to_vec(),
            b"a".to_vec(), b"b".to_vec(),
        ]);
    }

    #[rstest]
    fn test_rpush() {
        let cmd = ListCommand::rpush("mylist").value("x").build();
        assert_eq!(cmd.args(), &[b"RPUSH".to_vec(), b"mylist".to_vec(), b"x".to_vec()]);
    }

    #[rstest]
    fn test_lrange() {
        let cmd = ListCommand::lrange("mylist", 0, -1).build();
        assert_eq!(cmd.args(), &[
            b"LRANGE".to_vec(), b"mylist".to_vec(),
            b"0".to_vec(), b"-1".to_vec(),
        ]);
    }

    #[rstest]
    fn test_lpop_no_count() {
        let cmd = ListCommand::lpop("mylist", None).build();
        assert_eq!(cmd.args(), &[b"LPOP".to_vec(), b"mylist".to_vec()]);
    }

    #[rstest]
    fn test_lpop_with_count() {
        let cmd = ListCommand::lpop("mylist", Some(3)).build();
        assert_eq!(cmd.args(), &[
            b"LPOP".to_vec(), b"mylist".to_vec(), b"3".to_vec(),
        ]);
    }

    #[rstest]
    fn test_llen() {
        let cmd = ListCommand::llen("mylist").build();
        assert_eq!(cmd.args(), &[b"LLEN".to_vec(), b"mylist".to_vec()]);
    }
}

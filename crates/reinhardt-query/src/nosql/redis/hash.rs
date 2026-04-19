//! Redis Hash command builders.

use super::{
    command::CommandStatementBuilder,
    key::{IntoRedisKey, RedisKey},
    resp::RespCommand,
    value::ToRedisBytes,
};

/// Entry point for Redis Hash commands.
pub struct HashCommand;

impl HashCommand {
    /// Build an HSET command (set one or more fields).
    pub fn hset(key: impl IntoRedisKey) -> HSetBuilder {
        HSetBuilder { key: key.into_redis_key(), fields: Vec::new() }
    }

    /// Build an HGET command.
    pub fn hget(key: impl IntoRedisKey, field: impl ToRedisBytes) -> HGetStatement {
        HGetStatement { key: key.into_redis_key(), field: field.to_redis_bytes() }
    }

    /// Build an HMGET command (get multiple fields).
    pub fn hmget(key: impl IntoRedisKey) -> HMGetBuilder {
        HMGetBuilder { key: key.into_redis_key(), fields: Vec::new() }
    }

    /// Build an HGETALL command.
    pub fn hgetall(key: impl IntoRedisKey) -> HGetAllStatement {
        HGetAllStatement { key: key.into_redis_key() }
    }

    /// Build an HDEL command.
    pub fn hdel(key: impl IntoRedisKey) -> HDelBuilder {
        HDelBuilder { key: key.into_redis_key(), fields: Vec::new() }
    }

    /// Build an HEXISTS command.
    pub fn hexists(key: impl IntoRedisKey, field: impl ToRedisBytes) -> HExistsStatement {
        HExistsStatement { key: key.into_redis_key(), field: field.to_redis_bytes() }
    }

    /// Build an HLEN command.
    pub fn hlen(key: impl IntoRedisKey) -> HLenStatement {
        HLenStatement { key: key.into_redis_key() }
    }

    /// Build an HKEYS command.
    pub fn hkeys(key: impl IntoRedisKey) -> HKeysStatement {
        HKeysStatement { key: key.into_redis_key() }
    }

    /// Build an HVALS command.
    pub fn hvals(key: impl IntoRedisKey) -> HValsStatement {
        HValsStatement { key: key.into_redis_key() }
    }

    /// Build an HINCRBY command.
    pub fn hincrby(key: impl IntoRedisKey, field: impl ToRedisBytes, by: i64) -> HIncrByStatement {
        HIncrByStatement {
            key: key.into_redis_key(),
            field: field.to_redis_bytes(),
            by,
        }
    }
}

/// Builder for HSET command.
#[derive(Debug)]
pub struct HSetBuilder {
    key: RedisKey,
    fields: Vec<(Vec<u8>, Vec<u8>)>,
}

impl HSetBuilder {
    /// Add a field-value pair.
    pub fn field(mut self, k: impl ToRedisBytes, v: impl ToRedisBytes) -> Self {
        self.fields.push((k.to_redis_bytes(), v.to_redis_bytes()));
        self
    }
}

impl CommandStatementBuilder for HSetBuilder {
    fn build(&self) -> RespCommand {
        let mut args = vec![b"HSET".to_vec(), self.key.to_bytes()];
        for (k, v) in &self.fields {
            args.push(k.clone());
            args.push(v.clone());
        }
        RespCommand::new(args)
    }
}

/// Builder for HMGET command.
#[derive(Debug)]
pub struct HMGetBuilder {
    key: RedisKey,
    fields: Vec<Vec<u8>>,
}

impl HMGetBuilder {
    /// Add a field to retrieve.
    pub fn field(mut self, f: impl ToRedisBytes) -> Self {
        self.fields.push(f.to_redis_bytes());
        self
    }
}

impl CommandStatementBuilder for HMGetBuilder {
    fn build(&self) -> RespCommand {
        let mut args = vec![b"HMGET".to_vec(), self.key.to_bytes()];
        args.extend(self.fields.iter().cloned());
        RespCommand::new(args)
    }
}

/// Builder for HDEL command.
#[derive(Debug)]
pub struct HDelBuilder {
    key: RedisKey,
    fields: Vec<Vec<u8>>,
}

impl HDelBuilder {
    /// Add a field to delete.
    pub fn field(mut self, f: impl ToRedisBytes) -> Self {
        self.fields.push(f.to_redis_bytes());
        self
    }
}

impl CommandStatementBuilder for HDelBuilder {
    fn build(&self) -> RespCommand {
        let mut args = vec![b"HDEL".to_vec(), self.key.to_bytes()];
        args.extend(self.fields.iter().cloned());
        RespCommand::new(args)
    }
}

macro_rules! simple_key_cmd {
    ($name:ident, $cmd:expr) => {
        #[doc = "Redis command builder (key only)."]
        #[derive(Debug)]
        pub struct $name {
            key: RedisKey,
        }
        impl CommandStatementBuilder for $name {
            fn build(&self) -> RespCommand {
                RespCommand::new(vec![$cmd.to_vec(), self.key.to_bytes()])
            }
        }
    };
}

macro_rules! key_field_cmd {
    ($name:ident, $cmd:expr) => {
        #[doc = "Redis command builder (key + field)."]
        #[derive(Debug)]
        pub struct $name {
            key: RedisKey,
            field: Vec<u8>,
        }
        impl CommandStatementBuilder for $name {
            fn build(&self) -> RespCommand {
                RespCommand::new(vec![
                    $cmd.to_vec(),
                    self.key.to_bytes(),
                    self.field.clone(),
                ])
            }
        }
    };
}

simple_key_cmd!(HGetAllStatement, b"HGETALL");
simple_key_cmd!(HLenStatement, b"HLEN");
simple_key_cmd!(HKeysStatement, b"HKEYS");
simple_key_cmd!(HValsStatement, b"HVALS");
key_field_cmd!(HGetStatement, b"HGET");
key_field_cmd!(HExistsStatement, b"HEXISTS");

/// Builder for HINCRBY command.
#[derive(Debug)]
pub struct HIncrByStatement {
    key: RedisKey,
    field: Vec<u8>,
    by: i64,
}

impl CommandStatementBuilder for HIncrByStatement {
    fn build(&self) -> RespCommand {
        RespCommand::new(vec![
            b"HINCRBY".to_vec(),
            self.key.to_bytes(),
            self.field.clone(),
            self.by.to_string().into_bytes(),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;

    #[rstest]
    fn test_hset_single_field() {
        let cmd = HashCommand::hset("myhash").field("name", "Alice").build();
        assert_eq!(cmd.args(), &[
            b"HSET".to_vec(), b"myhash".to_vec(),
            b"name".to_vec(), b"Alice".to_vec(),
        ]);
    }

    #[rstest]
    fn test_hset_multiple_fields() {
        let cmd = HashCommand::hset("h").field("a", "1").field("b", "2").build();
        assert_eq!(cmd.args(), &[
            b"HSET".to_vec(), b"h".to_vec(),
            b"a".to_vec(), b"1".to_vec(),
            b"b".to_vec(), b"2".to_vec(),
        ]);
    }

    #[rstest]
    fn test_hget() {
        let cmd = HashCommand::hget("h", "field1").build();
        assert_eq!(cmd.args(), &[
            b"HGET".to_vec(), b"h".to_vec(), b"field1".to_vec(),
        ]);
    }

    #[rstest]
    fn test_hgetall() {
        let cmd = HashCommand::hgetall("h").build();
        assert_eq!(cmd.args(), &[b"HGETALL".to_vec(), b"h".to_vec()]);
    }

    #[rstest]
    fn test_hincrby() {
        let cmd = HashCommand::hincrby("h", "count", 3).build();
        assert_eq!(cmd.args(), &[
            b"HINCRBY".to_vec(), b"h".to_vec(), b"count".to_vec(), b"3".to_vec(),
        ]);
    }

    #[rstest]
    fn test_hmget() {
        let cmd = HashCommand::hmget("h").field("f1").field("f2").build();
        assert_eq!(cmd.args(), &[
            b"HMGET".to_vec(), b"h".to_vec(), b"f1".to_vec(), b"f2".to_vec(),
        ]);
    }
}

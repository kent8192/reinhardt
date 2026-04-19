//! Redis Set command builders.

use super::{
    command::CommandStatementBuilder,
    key::{IntoRedisKey, RedisKey},
    resp::RespCommand,
    value::ToRedisBytes,
};

/// Entry point for Redis Set commands.
pub struct SetCommand;

impl SetCommand {
    /// Build an SADD command.
    pub fn sadd(key: impl IntoRedisKey) -> SAddBuilder {
        SAddBuilder { key: key.into_redis_key(), members: Vec::new() }
    }

    /// Build an SREM command.
    pub fn srem(key: impl IntoRedisKey) -> SRemBuilder {
        SRemBuilder { key: key.into_redis_key(), members: Vec::new() }
    }

    /// Build an SMEMBERS command.
    pub fn smembers(key: impl IntoRedisKey) -> SMembersStatement {
        SMembersStatement { key: key.into_redis_key() }
    }

    /// Build an SISMEMBER command.
    pub fn sismember(key: impl IntoRedisKey, member: impl ToRedisBytes) -> SIsMemberStatement {
        SIsMemberStatement { key: key.into_redis_key(), member: member.to_redis_bytes() }
    }

    /// Build an SCARD command.
    pub fn scard(key: impl IntoRedisKey) -> SCardStatement {
        SCardStatement { key: key.into_redis_key() }
    }

    /// Build an SRANDMEMBER command.
    pub fn srandmember(key: impl IntoRedisKey, count: Option<i64>) -> SRandMemberStatement {
        SRandMemberStatement { key: key.into_redis_key(), count }
    }

    /// Build an SPOP command.
    pub fn spop(key: impl IntoRedisKey, count: Option<u64>) -> SPopStatement {
        SPopStatement { key: key.into_redis_key(), count }
    }

    /// Build a SUNION command.
    pub fn sunion<K: IntoRedisKey>(keys: Vec<K>) -> SUnionStatement {
        SUnionStatement {
            cmd: b"SUNION",
            keys: keys.into_iter().map(|k| k.into_redis_key()).collect(),
        }
    }

    /// Build an SINTER command.
    pub fn sinter<K: IntoRedisKey>(keys: Vec<K>) -> SUnionStatement {
        SUnionStatement {
            cmd: b"SINTER",
            keys: keys.into_iter().map(|k| k.into_redis_key()).collect(),
        }
    }

    /// Build an SDIFF command.
    pub fn sdiff<K: IntoRedisKey>(keys: Vec<K>) -> SUnionStatement {
        SUnionStatement {
            cmd: b"SDIFF",
            keys: keys.into_iter().map(|k| k.into_redis_key()).collect(),
        }
    }
}

macro_rules! member_builder {
    ($name:ident, $cmd:expr) => {
        #[doc = "Redis Set member collection builder."]
        #[derive(Debug)]
        pub struct $name {
            key: RedisKey,
            members: Vec<Vec<u8>>,
        }
        impl $name {
            /// Add a member.
            pub fn member(mut self, m: impl ToRedisBytes) -> Self {
                self.members.push(m.to_redis_bytes());
                self
            }
        }
        impl CommandStatementBuilder for $name {
            fn build(&self) -> RespCommand {
                let mut args = vec![$cmd.to_vec(), self.key.to_bytes()];
                args.extend(self.members.iter().cloned());
                RespCommand::new(args)
            }
        }
    };
}

member_builder!(SAddBuilder, b"SADD");
member_builder!(SRemBuilder, b"SREM");

/// Builder for SMEMBERS command.
#[derive(Debug)]
pub struct SMembersStatement {
    key: RedisKey,
}

impl CommandStatementBuilder for SMembersStatement {
    fn build(&self) -> RespCommand {
        RespCommand::new(vec![b"SMEMBERS".to_vec(), self.key.to_bytes()])
    }
}

/// Builder for SISMEMBER command.
#[derive(Debug)]
pub struct SIsMemberStatement {
    key: RedisKey,
    member: Vec<u8>,
}

impl CommandStatementBuilder for SIsMemberStatement {
    fn build(&self) -> RespCommand {
        RespCommand::new(vec![
            b"SISMEMBER".to_vec(),
            self.key.to_bytes(),
            self.member.clone(),
        ])
    }
}

/// Builder for SCARD command.
#[derive(Debug)]
pub struct SCardStatement {
    key: RedisKey,
}

impl CommandStatementBuilder for SCardStatement {
    fn build(&self) -> RespCommand {
        RespCommand::new(vec![b"SCARD".to_vec(), self.key.to_bytes()])
    }
}

/// Builder for SRANDMEMBER command.
#[derive(Debug)]
pub struct SRandMemberStatement {
    key: RedisKey,
    count: Option<i64>,
}

impl CommandStatementBuilder for SRandMemberStatement {
    fn build(&self) -> RespCommand {
        let mut args = vec![b"SRANDMEMBER".to_vec(), self.key.to_bytes()];
        if let Some(n) = self.count {
            args.push(n.to_string().into_bytes());
        }
        RespCommand::new(args)
    }
}

/// Builder for SPOP command.
#[derive(Debug)]
pub struct SPopStatement {
    key: RedisKey,
    count: Option<u64>,
}

impl CommandStatementBuilder for SPopStatement {
    fn build(&self) -> RespCommand {
        let mut args = vec![b"SPOP".to_vec(), self.key.to_bytes()];
        if let Some(n) = self.count {
            args.push(n.to_string().into_bytes());
        }
        RespCommand::new(args)
    }
}

/// Builder for SUNION/SINTER/SDIFF commands.
#[derive(Debug)]
pub struct SUnionStatement {
    cmd: &'static [u8],
    keys: Vec<RedisKey>,
}

impl CommandStatementBuilder for SUnionStatement {
    fn build(&self) -> RespCommand {
        let mut args = vec![self.cmd.to_vec()];
        args.extend(self.keys.iter().map(|k| k.to_bytes()));
        RespCommand::new(args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;

    #[rstest]
    fn test_sadd() {
        let cmd = SetCommand::sadd("myset").member("a").member("b").build();
        assert_eq!(cmd.args(), &[
            b"SADD".to_vec(), b"myset".to_vec(),
            b"a".to_vec(), b"b".to_vec(),
        ]);
    }

    #[rstest]
    fn test_smembers() {
        let cmd = SetCommand::smembers("myset").build();
        assert_eq!(cmd.args(), &[b"SMEMBERS".to_vec(), b"myset".to_vec()]);
    }

    #[rstest]
    fn test_sismember() {
        let cmd = SetCommand::sismember("myset", "alice").build();
        assert_eq!(cmd.args(), &[
            b"SISMEMBER".to_vec(), b"myset".to_vec(), b"alice".to_vec(),
        ]);
    }

    #[rstest]
    fn test_scard() {
        let cmd = SetCommand::scard("myset").build();
        assert_eq!(cmd.args(), &[b"SCARD".to_vec(), b"myset".to_vec()]);
    }

    #[rstest]
    fn test_sunion() {
        let cmd = SetCommand::sunion(vec!["s1", "s2"]).build();
        assert_eq!(cmd.args(), &[
            b"SUNION".to_vec(), b"s1".to_vec(), b"s2".to_vec(),
        ]);
    }

    #[rstest]
    fn test_srem() {
        let cmd = SetCommand::srem("myset").member("x").build();
        assert_eq!(cmd.args(), &[
            b"SREM".to_vec(), b"myset".to_vec(), b"x".to_vec(),
        ]);
    }
}

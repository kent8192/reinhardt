//! Redis String command builders.

use std::fmt::Debug;
use std::marker::PhantomData;

use super::{
    command::CommandStatementBuilder,
    key::{IntoRedisKey, RedisKey},
    resp::RespCommand,
    value::ToRedisBytes,
};

// --- Typestate markers for SetStatement ---

/// Typestate marker: no NX/XX condition set yet.
#[derive(Debug)]
pub struct NoSetCondition;
/// Typestate marker: NX condition applied.
#[derive(Debug)]
pub struct SetNx;
/// Typestate marker: XX condition applied.
#[derive(Debug)]
pub struct SetXx;
/// Typestate marker: no expiry set yet.
#[derive(Debug)]
pub struct NoSetExpiry;
/// Typestate marker: expiry applied.
#[derive(Debug)]
pub struct HasSetExpiry;

#[derive(Debug)]
enum SetCondition {
    Nx,
    Xx,
}

#[derive(Debug)]
enum Expiry {
    Ex(u64),
    Px(u64),
    ExAt(u64),
    PxAt(u64),
    KeepTtl,
}

/// Builder for the Redis SET command.
///
/// Type parameters `Cond` and `Exp` are typestate markers that prevent
/// setting conflicting options at compile time (e.g., NX + XX or EX + PX).
#[derive(Debug)]
pub struct SetStatement<Cond = NoSetCondition, Exp = NoSetExpiry> {
    key: RedisKey,
    value: Vec<u8>,
    condition: Option<SetCondition>,
    expiry: Option<Expiry>,
    get: bool,
    _cond: PhantomData<Cond>,
    _exp: PhantomData<Exp>,
}

impl<Exp: Debug> SetStatement<NoSetCondition, Exp> {
    /// Set NX flag: only set if key does not exist.
    pub fn nx(self) -> SetStatement<SetNx, Exp> {
        SetStatement {
            condition: Some(SetCondition::Nx),
            key: self.key,
            value: self.value,
            expiry: self.expiry,
            get: self.get,
            _cond: PhantomData,
            _exp: self._exp,
        }
    }

    /// Set XX flag: only set if key already exists.
    pub fn xx(self) -> SetStatement<SetXx, Exp> {
        SetStatement {
            condition: Some(SetCondition::Xx),
            key: self.key,
            value: self.value,
            expiry: self.expiry,
            get: self.get,
            _cond: PhantomData,
            _exp: self._exp,
        }
    }
}

impl<Cond: Debug> SetStatement<Cond, NoSetExpiry> {
    /// Set expiry in seconds.
    pub fn ex(self, secs: u64) -> SetStatement<Cond, HasSetExpiry> {
        SetStatement {
            expiry: Some(Expiry::Ex(secs)),
            key: self.key,
            value: self.value,
            condition: self.condition,
            get: self.get,
            _cond: self._cond,
            _exp: PhantomData,
        }
    }

    /// Set expiry in milliseconds.
    pub fn px(self, ms: u64) -> SetStatement<Cond, HasSetExpiry> {
        SetStatement {
            expiry: Some(Expiry::Px(ms)),
            key: self.key,
            value: self.value,
            condition: self.condition,
            get: self.get,
            _cond: self._cond,
            _exp: PhantomData,
        }
    }

    /// Set expiry as Unix timestamp in seconds.
    pub fn exat(self, ts: u64) -> SetStatement<Cond, HasSetExpiry> {
        SetStatement {
            expiry: Some(Expiry::ExAt(ts)),
            key: self.key,
            value: self.value,
            condition: self.condition,
            get: self.get,
            _cond: self._cond,
            _exp: PhantomData,
        }
    }

    /// Set expiry as Unix timestamp in milliseconds.
    pub fn pxat(self, ts: u64) -> SetStatement<Cond, HasSetExpiry> {
        SetStatement {
            expiry: Some(Expiry::PxAt(ts)),
            key: self.key,
            value: self.value,
            condition: self.condition,
            get: self.get,
            _cond: self._cond,
            _exp: PhantomData,
        }
    }

    /// Retain the existing TTL.
    pub fn keepttl(self) -> SetStatement<Cond, HasSetExpiry> {
        SetStatement {
            expiry: Some(Expiry::KeepTtl),
            key: self.key,
            value: self.value,
            condition: self.condition,
            get: self.get,
            _cond: self._cond,
            _exp: PhantomData,
        }
    }
}

impl<Cond, Exp> SetStatement<Cond, Exp> {
    /// Return the old value before setting the new one (GET flag).
    pub fn get(mut self) -> Self {
        self.get = true;
        self
    }
}

impl<Cond: Debug, Exp: Debug> CommandStatementBuilder for SetStatement<Cond, Exp> {
    fn build(&self) -> RespCommand {
        let mut args = vec![b"SET".to_vec(), self.key.to_bytes(), self.value.clone()];

        if let Some(expiry) = &self.expiry {
            match expiry {
                Expiry::Ex(n) => {
                    args.push(b"EX".to_vec());
                    args.push(n.to_string().into_bytes());
                }
                Expiry::Px(n) => {
                    args.push(b"PX".to_vec());
                    args.push(n.to_string().into_bytes());
                }
                Expiry::ExAt(n) => {
                    args.push(b"EXAT".to_vec());
                    args.push(n.to_string().into_bytes());
                }
                Expiry::PxAt(n) => {
                    args.push(b"PXAT".to_vec());
                    args.push(n.to_string().into_bytes());
                }
                Expiry::KeepTtl => {
                    args.push(b"KEEPTTL".to_vec());
                }
            }
        }

        if let Some(cond) = &self.condition {
            match cond {
                SetCondition::Nx => args.push(b"NX".to_vec()),
                SetCondition::Xx => args.push(b"XX".to_vec()),
            }
        }

        if self.get {
            args.push(b"GET".to_vec());
        }

        RespCommand::new(args)
    }
}

// --- Simple statement types ---

/// Builder for GET command.
#[derive(Debug)]
pub struct GetStatement {
    key: RedisKey,
}

impl CommandStatementBuilder for GetStatement {
    fn build(&self) -> RespCommand {
        RespCommand::new(vec![b"GET".to_vec(), self.key.to_bytes()])
    }
}

/// Builder for MGET command.
#[derive(Debug)]
pub struct MGetStatement {
    keys: Vec<RedisKey>,
}

impl CommandStatementBuilder for MGetStatement {
    fn build(&self) -> RespCommand {
        let mut args = vec![b"MGET".to_vec()];
        args.extend(self.keys.iter().map(|k| k.to_bytes()));
        RespCommand::new(args)
    }
}

/// Builder for INCR/INCRBY commands.
#[derive(Debug)]
pub struct IncrStatement {
    key: RedisKey,
    by: Option<i64>,
}

impl CommandStatementBuilder for IncrStatement {
    fn build(&self) -> RespCommand {
        match self.by {
            None => RespCommand::new(vec![b"INCR".to_vec(), self.key.to_bytes()]),
            Some(n) => RespCommand::new(vec![
                b"INCRBY".to_vec(),
                self.key.to_bytes(),
                n.to_string().into_bytes(),
            ]),
        }
    }
}

/// Builder for DECR/DECRBY commands.
#[derive(Debug)]
pub struct DecrStatement {
    key: RedisKey,
    by: Option<i64>,
}

impl CommandStatementBuilder for DecrStatement {
    fn build(&self) -> RespCommand {
        match self.by {
            None => RespCommand::new(vec![b"DECR".to_vec(), self.key.to_bytes()]),
            Some(n) => RespCommand::new(vec![
                b"DECRBY".to_vec(),
                self.key.to_bytes(),
                n.to_string().into_bytes(),
            ]),
        }
    }
}

/// Builder for APPEND command.
#[derive(Debug)]
pub struct AppendStatement {
    key: RedisKey,
    value: Vec<u8>,
}

impl CommandStatementBuilder for AppendStatement {
    fn build(&self) -> RespCommand {
        RespCommand::new(vec![
            b"APPEND".to_vec(),
            self.key.to_bytes(),
            self.value.clone(),
        ])
    }
}

/// Builder for GETDEL command.
#[derive(Debug)]
pub struct GetDelStatement {
    key: RedisKey,
}

impl CommandStatementBuilder for GetDelStatement {
    fn build(&self) -> RespCommand {
        RespCommand::new(vec![b"GETDEL".to_vec(), self.key.to_bytes()])
    }
}

/// Builder for STRLEN command.
#[derive(Debug)]
pub struct StrLenStatement {
    key: RedisKey,
}

impl CommandStatementBuilder for StrLenStatement {
    fn build(&self) -> RespCommand {
        RespCommand::new(vec![b"STRLEN".to_vec(), self.key.to_bytes()])
    }
}

/// Entry point for Redis String commands.
pub struct StringCommand;

impl StringCommand {
    /// Build a SET command.
    pub fn set(key: impl IntoRedisKey, value: impl ToRedisBytes) -> SetStatement {
        SetStatement {
            key: key.into_redis_key(),
            value: value.to_redis_bytes(),
            condition: None,
            expiry: None,
            get: false,
            _cond: PhantomData,
            _exp: PhantomData,
        }
    }

    /// Build a GET command.
    pub fn get(key: impl IntoRedisKey) -> GetStatement {
        GetStatement { key: key.into_redis_key() }
    }

    /// Build a MGET command.
    pub fn mget<K: IntoRedisKey>(keys: Vec<K>) -> MGetStatement {
        MGetStatement {
            keys: keys.into_iter().map(|k| k.into_redis_key()).collect(),
        }
    }

    /// Build an INCR command.
    pub fn incr(key: impl IntoRedisKey) -> IncrStatement {
        IncrStatement { key: key.into_redis_key(), by: None }
    }

    /// Build an INCRBY command.
    pub fn incrby(key: impl IntoRedisKey, by: i64) -> IncrStatement {
        IncrStatement { key: key.into_redis_key(), by: Some(by) }
    }

    /// Build a DECR command.
    pub fn decr(key: impl IntoRedisKey) -> DecrStatement {
        DecrStatement { key: key.into_redis_key(), by: None }
    }

    /// Build a DECRBY command.
    pub fn decrby(key: impl IntoRedisKey, by: i64) -> DecrStatement {
        DecrStatement { key: key.into_redis_key(), by: Some(by) }
    }

    /// Build an APPEND command.
    pub fn append(key: impl IntoRedisKey, value: impl ToRedisBytes) -> AppendStatement {
        AppendStatement {
            key: key.into_redis_key(),
            value: value.to_redis_bytes(),
        }
    }

    /// Build a GETDEL command.
    pub fn getdel(key: impl IntoRedisKey) -> GetDelStatement {
        GetDelStatement { key: key.into_redis_key() }
    }

    /// Build a STRLEN command.
    pub fn strlen(key: impl IntoRedisKey) -> StrLenStatement {
        StrLenStatement { key: key.into_redis_key() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;

    // --- Simple commands ---

    #[rstest]
    fn test_get_command() {
        let cmd = StringCommand::get("mykey").build();
        assert_eq!(cmd.args(), &[b"GET".to_vec(), b"mykey".to_vec()]);
    }

    #[rstest]
    fn test_mget_command() {
        let cmd = StringCommand::mget(vec!["key1", "key2"]).build();
        assert_eq!(cmd.args(), &[
            b"MGET".to_vec(),
            b"key1".to_vec(),
            b"key2".to_vec(),
        ]);
    }

    #[rstest]
    fn test_incr_command() {
        let cmd = StringCommand::incr("counter").build();
        assert_eq!(cmd.args(), &[b"INCR".to_vec(), b"counter".to_vec()]);
    }

    #[rstest]
    fn test_incrby_command() {
        let cmd = StringCommand::incrby("counter", 5).build();
        assert_eq!(cmd.args(), &[
            b"INCRBY".to_vec(),
            b"counter".to_vec(),
            b"5".to_vec(),
        ]);
    }

    #[rstest]
    fn test_strlen_command() {
        let cmd = StringCommand::strlen("mykey").build();
        assert_eq!(cmd.args(), &[b"STRLEN".to_vec(), b"mykey".to_vec()]);
    }

    #[rstest]
    fn test_decr_command() {
        let cmd = StringCommand::decr("counter").build();
        assert_eq!(cmd.args(), &[b"DECR".to_vec(), b"counter".to_vec()]);
    }

    #[rstest]
    fn test_decrby_command() {
        let cmd = StringCommand::decrby("counter", 3).build();
        assert_eq!(cmd.args(), &[
            b"DECRBY".to_vec(),
            b"counter".to_vec(),
            b"3".to_vec(),
        ]);
    }

    // --- SetStatement typestate ---

    #[rstest]
    fn test_set_basic() {
        let cmd = StringCommand::set("mykey", "myvalue").build();
        assert_eq!(cmd.args(), &[
            b"SET".to_vec(),
            b"mykey".to_vec(),
            b"myvalue".to_vec(),
        ]);
    }

    #[rstest]
    fn test_set_with_ex() {
        let cmd = StringCommand::set("k", "v").ex(60).build();
        assert_eq!(cmd.args(), &[
            b"SET".to_vec(), b"k".to_vec(), b"v".to_vec(),
            b"EX".to_vec(), b"60".to_vec(),
        ]);
    }

    #[rstest]
    fn test_set_with_px() {
        let cmd = StringCommand::set("k", "v").px(1000).build();
        assert_eq!(cmd.args(), &[
            b"SET".to_vec(), b"k".to_vec(), b"v".to_vec(),
            b"PX".to_vec(), b"1000".to_vec(),
        ]);
    }

    #[rstest]
    fn test_set_with_nx() {
        let cmd = StringCommand::set("k", "v").nx().build();
        assert_eq!(cmd.args(), &[
            b"SET".to_vec(), b"k".to_vec(), b"v".to_vec(),
            b"NX".to_vec(),
        ]);
    }

    #[rstest]
    fn test_set_with_ex_and_nx() {
        let cmd = StringCommand::set("k", "v").ex(60).nx().build();
        assert_eq!(cmd.args(), &[
            b"SET".to_vec(), b"k".to_vec(), b"v".to_vec(),
            b"EX".to_vec(), b"60".to_vec(),
            b"NX".to_vec(),
        ]);
    }

    #[rstest]
    fn test_set_with_xx_and_get() {
        let cmd = StringCommand::set("k", "v").xx().get().build();
        assert_eq!(cmd.args(), &[
            b"SET".to_vec(), b"k".to_vec(), b"v".to_vec(),
            b"XX".to_vec(), b"GET".to_vec(),
        ]);
    }

    #[rstest]
    fn test_set_keepttl() {
        let cmd = StringCommand::set("k", "v").keepttl().build();
        assert_eq!(cmd.args(), &[
            b"SET".to_vec(), b"k".to_vec(), b"v".to_vec(),
            b"KEEPTTL".to_vec(),
        ]);
    }
}

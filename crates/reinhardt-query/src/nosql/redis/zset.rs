//! Redis Sorted Set command builders.

use std::fmt::Debug;
use std::marker::PhantomData;

use super::{
    command::CommandStatementBuilder,
    key::{IntoRedisKey, RedisKey},
    resp::RespCommand,
    value::ToRedisBytes,
};

// --- ZAddBuilder typestate markers ---

/// Typestate marker: no ZADD mode set.
#[derive(Debug)]
pub struct ZAddNoMode;
/// Typestate marker: NX mode (cannot add GT/LT).
#[derive(Debug)]
pub struct ZAddNxMode;
/// Typestate marker: XX mode (can transition to GT/LT).
#[derive(Debug)]
pub struct ZAddXxMode;
/// Typestate marker: GT mode (update existing members only if new score is greater).
#[derive(Debug)]
pub struct ZAddGtMode;
/// Typestate marker: LT mode (update existing members only if new score is less).
#[derive(Debug)]
pub struct ZAddLtMode;

#[derive(Debug)]
enum ZAddMode {
    Nx,
    Xx,
    XxGt,
    XxLt,
    Gt,
    Lt,
}

/// Builder for the Redis ZADD command.
///
/// The type parameter `M` is a typestate marker enforcing that GT, LT, and NX
/// are mutually exclusive at compile time.
#[derive(Debug)]
pub struct ZAddBuilder<M = ZAddNoMode> {
    key: RedisKey,
    members: Vec<(f64, Vec<u8>)>,
    mode: Option<ZAddMode>,
    changed: bool,
    _mode: PhantomData<M>,
}

impl ZAddBuilder<ZAddNoMode> {
    /// Set NX flag: only add new members.
    pub fn nx(self) -> ZAddBuilder<ZAddNxMode> {
        ZAddBuilder {
            mode: Some(ZAddMode::Nx),
            key: self.key,
            members: self.members,
            changed: self.changed,
            _mode: PhantomData,
        }
    }

    /// Set XX flag: only update existing members.
    pub fn xx(self) -> ZAddBuilder<ZAddXxMode> {
        ZAddBuilder {
            mode: Some(ZAddMode::Xx),
            key: self.key,
            members: self.members,
            changed: self.changed,
            _mode: PhantomData,
        }
    }

    /// Set GT flag: only update if new score is greater.
    ///
    /// Named `only_if_greater` (rather than `gt`) to avoid ambiguity with
    /// `Iterator::gt`, whose blanket impl would otherwise take precedence on
    /// typestates where GT is not a valid transition.
    pub fn only_if_greater(self) -> ZAddBuilder<ZAddGtMode> {
        ZAddBuilder {
            mode: Some(ZAddMode::Gt),
            key: self.key,
            members: self.members,
            changed: self.changed,
            _mode: PhantomData,
        }
    }

    /// Set LT flag: only update if new score is less.
    ///
    /// Named `only_if_less` (rather than `lt`) to avoid ambiguity with
    /// `Iterator::lt`.
    pub fn only_if_less(self) -> ZAddBuilder<ZAddLtMode> {
        ZAddBuilder {
            mode: Some(ZAddMode::Lt),
            key: self.key,
            members: self.members,
            changed: self.changed,
            _mode: PhantomData,
        }
    }
}

impl ZAddBuilder<ZAddXxMode> {
    /// Add GT flag alongside XX.
    pub fn only_if_greater(self) -> ZAddBuilder<ZAddGtMode> {
        ZAddBuilder {
            mode: Some(ZAddMode::XxGt),
            key: self.key,
            members: self.members,
            changed: self.changed,
            _mode: PhantomData,
        }
    }

    /// Add LT flag alongside XX.
    pub fn only_if_less(self) -> ZAddBuilder<ZAddLtMode> {
        ZAddBuilder {
            mode: Some(ZAddMode::XxLt),
            key: self.key,
            members: self.members,
            changed: self.changed,
            _mode: PhantomData,
        }
    }
}

impl<M> ZAddBuilder<M> {
    /// Add a score-member pair.
    pub fn member(mut self, score: f64, value: impl ToRedisBytes) -> Self {
        self.members.push((score, value.to_redis_bytes()));
        self
    }

    /// Return changed count instead of added count.
    pub fn ch(mut self) -> Self {
        self.changed = true;
        self
    }
}

fn score_bytes(score: f64) -> Vec<u8> {
    if score.is_infinite() && score.is_sign_positive() {
        b"+inf".to_vec()
    } else if score.is_infinite() && score.is_sign_negative() {
        b"-inf".to_vec()
    } else {
        format!("{}", score).into_bytes()
    }
}

impl<M: Debug> CommandStatementBuilder for ZAddBuilder<M> {
    fn build(&self) -> RespCommand {
        let mut args = vec![b"ZADD".to_vec(), self.key.to_bytes()];

        match &self.mode {
            Some(ZAddMode::Nx) => args.push(b"NX".to_vec()),
            Some(ZAddMode::Xx) => args.push(b"XX".to_vec()),
            Some(ZAddMode::XxGt) => {
                args.push(b"XX".to_vec());
                args.push(b"GT".to_vec());
            }
            Some(ZAddMode::XxLt) => {
                args.push(b"XX".to_vec());
                args.push(b"LT".to_vec());
            }
            Some(ZAddMode::Gt) => args.push(b"GT".to_vec()),
            Some(ZAddMode::Lt) => args.push(b"LT".to_vec()),
            None => {}
        }

        if self.changed {
            args.push(b"CH".to_vec());
        }

        for (score, member) in &self.members {
            args.push(score_bytes(*score));
            args.push(member.clone());
        }

        RespCommand::new(args)
    }
}

/// Range boundary for ZRANGE/ZCOUNT commands.
#[derive(Debug)]
enum ZRangeBy {
    Score,
    Lex,
}

/// Builder for ZRANGE command.
#[derive(Debug)]
pub struct ZRangeBuilder {
    key: RedisKey,
    start: i64,
    stop: i64,
    by: Option<ZRangeBy>,
    rev: bool,
    limit: Option<(i64, i64)>,
}

impl ZRangeBuilder {
    /// Use BYSCORE range mode.
    pub fn by_score(mut self) -> Self {
        self.by = Some(ZRangeBy::Score);
        self
    }

    /// Use BYLEX range mode.
    pub fn by_lex(mut self) -> Self {
        self.by = Some(ZRangeBy::Lex);
        self
    }

    /// Reverse the order.
    pub fn rev(mut self) -> Self {
        self.rev = true;
        self
    }

    /// Apply LIMIT offset and count.
    pub fn limit(mut self, offset: i64, count: i64) -> Self {
        self.limit = Some((offset, count));
        self
    }
}

impl CommandStatementBuilder for ZRangeBuilder {
    fn build(&self) -> RespCommand {
        let mut args = vec![
            b"ZRANGE".to_vec(),
            self.key.to_bytes(),
            self.start.to_string().into_bytes(),
            self.stop.to_string().into_bytes(),
        ];

        if let Some(by) = &self.by {
            match by {
                ZRangeBy::Score => args.push(b"BYSCORE".to_vec()),
                ZRangeBy::Lex => args.push(b"BYLEX".to_vec()),
            }
        }

        if self.rev {
            args.push(b"REV".to_vec());
        }

        if let Some((off, cnt)) = self.limit {
            args.push(b"LIMIT".to_vec());
            args.push(off.to_string().into_bytes());
            args.push(cnt.to_string().into_bytes());
        }

        RespCommand::new(args)
    }
}

/// Builder for ZREM command.
#[derive(Debug)]
pub struct ZRemBuilder {
    key: RedisKey,
    members: Vec<Vec<u8>>,
}

impl ZRemBuilder {
    /// Add a member to remove.
    pub fn member(mut self, m: impl ToRedisBytes) -> Self {
        self.members.push(m.to_redis_bytes());
        self
    }
}

impl CommandStatementBuilder for ZRemBuilder {
    fn build(&self) -> RespCommand {
        let mut args = vec![b"ZREM".to_vec(), self.key.to_bytes()];
        args.extend(self.members.iter().cloned());
        RespCommand::new(args)
    }
}

/// Builder for ZRANK command.
#[derive(Debug)]
pub struct ZRankStatement {
    key: RedisKey,
    member: Vec<u8>,
}

impl CommandStatementBuilder for ZRankStatement {
    fn build(&self) -> RespCommand {
        RespCommand::new(vec![
            b"ZRANK".to_vec(),
            self.key.to_bytes(),
            self.member.clone(),
        ])
    }
}

/// Builder for ZSCORE command.
#[derive(Debug)]
pub struct ZScoreStatement {
    key: RedisKey,
    member: Vec<u8>,
}

impl CommandStatementBuilder for ZScoreStatement {
    fn build(&self) -> RespCommand {
        RespCommand::new(vec![
            b"ZSCORE".to_vec(),
            self.key.to_bytes(),
            self.member.clone(),
        ])
    }
}

/// Builder for ZCARD command.
#[derive(Debug)]
pub struct ZCardStatement {
    key: RedisKey,
}

impl CommandStatementBuilder for ZCardStatement {
    fn build(&self) -> RespCommand {
        RespCommand::new(vec![b"ZCARD".to_vec(), self.key.to_bytes()])
    }
}

/// Builder for ZINCRBY command.
#[derive(Debug)]
pub struct ZIncrByStatement {
    key: RedisKey,
    increment: f64,
    member: Vec<u8>,
}

impl CommandStatementBuilder for ZIncrByStatement {
    fn build(&self) -> RespCommand {
        RespCommand::new(vec![
            b"ZINCRBY".to_vec(),
            self.key.to_bytes(),
            score_bytes(self.increment),
            self.member.clone(),
        ])
    }
}

/// Entry point for Redis Sorted Set commands.
pub struct ZSetCommand;

impl ZSetCommand {
    /// Build a ZADD command.
    pub fn zadd(key: impl IntoRedisKey) -> ZAddBuilder {
        ZAddBuilder {
            key: key.into_redis_key(),
            members: Vec::new(),
            mode: None,
            changed: false,
            _mode: PhantomData,
        }
    }

    /// Build a ZRANGE command.
    pub fn zrange(key: impl IntoRedisKey, start: i64, stop: i64) -> ZRangeBuilder {
        ZRangeBuilder {
            key: key.into_redis_key(),
            start,
            stop,
            by: None,
            rev: false,
            limit: None,
        }
    }

    /// Build a ZRANK command.
    pub fn zrank(key: impl IntoRedisKey, member: impl ToRedisBytes) -> ZRankStatement {
        ZRankStatement { key: key.into_redis_key(), member: member.to_redis_bytes() }
    }

    /// Build a ZREM command.
    pub fn zrem(key: impl IntoRedisKey) -> ZRemBuilder {
        ZRemBuilder { key: key.into_redis_key(), members: Vec::new() }
    }

    /// Build a ZSCORE command.
    pub fn zscore(key: impl IntoRedisKey, member: impl ToRedisBytes) -> ZScoreStatement {
        ZScoreStatement { key: key.into_redis_key(), member: member.to_redis_bytes() }
    }

    /// Build a ZCARD command.
    pub fn zcard(key: impl IntoRedisKey) -> ZCardStatement {
        ZCardStatement { key: key.into_redis_key() }
    }

    /// Build a ZINCRBY command.
    pub fn zincrby(key: impl IntoRedisKey, increment: f64, member: impl ToRedisBytes) -> ZIncrByStatement {
        ZIncrByStatement {
            key: key.into_redis_key(),
            increment,
            member: member.to_redis_bytes(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;

    #[rstest]
    fn test_zadd_no_flags() {
        let cmd = ZSetCommand::zadd("zset")
            .member(1.5, "a")
            .member(2.0, "b")
            .build();
        assert_eq!(cmd.args(), &[
            b"ZADD".to_vec(), b"zset".to_vec(),
            b"1.5".to_vec(), b"a".to_vec(),
            b"2".to_vec(), b"b".to_vec(),
        ]);
    }

    #[rstest]
    fn test_zadd_nx() {
        let cmd = ZSetCommand::zadd("z").member(1.0, "a").nx().build();
        assert_eq!(cmd.args(), &[
            b"ZADD".to_vec(), b"z".to_vec(), b"NX".to_vec(),
            b"1".to_vec(), b"a".to_vec(),
        ]);
    }

    #[rstest]
    fn test_zadd_xx_gt_ch() {
        let cmd = ZSetCommand::zadd("z").xx().only_if_greater().ch().member(5.0, "a").build();
        assert_eq!(cmd.args(), &[
            b"ZADD".to_vec(), b"z".to_vec(),
            b"XX".to_vec(), b"GT".to_vec(), b"CH".to_vec(),
            b"5".to_vec(), b"a".to_vec(),
        ]);
    }

    #[rstest]
    fn test_zadd_gt_without_xx() {
        let cmd = ZSetCommand::zadd("z").only_if_greater().member(3.0, "m").build();
        assert_eq!(cmd.args(), &[
            b"ZADD".to_vec(), b"z".to_vec(),
            b"GT".to_vec(), b"3".to_vec(), b"m".to_vec(),
        ]);
    }

    #[rstest]
    fn test_zrange_by_index() {
        let cmd = ZSetCommand::zrange("z", 0, -1).build();
        assert_eq!(cmd.args(), &[
            b"ZRANGE".to_vec(), b"z".to_vec(), b"0".to_vec(), b"-1".to_vec(),
        ]);
    }

    #[rstest]
    fn test_zrange_by_score_rev() {
        let cmd = ZSetCommand::zrange("z", 0, -1).by_score().rev().build();
        assert_eq!(cmd.args(), &[
            b"ZRANGE".to_vec(), b"z".to_vec(), b"0".to_vec(), b"-1".to_vec(),
            b"BYSCORE".to_vec(), b"REV".to_vec(),
        ]);
    }

    #[rstest]
    fn test_zscore() {
        let cmd = ZSetCommand::zscore("z", "member").build();
        assert_eq!(cmd.args(), &[
            b"ZSCORE".to_vec(), b"z".to_vec(), b"member".to_vec(),
        ]);
    }

    #[rstest]
    fn test_zadd_lt() {
        let cmd = ZSetCommand::zadd("z").only_if_less().member(1.0, "x").build();
        assert_eq!(cmd.args(), &[
            b"ZADD".to_vec(), b"z".to_vec(),
            b"LT".to_vec(), b"1".to_vec(), b"x".to_vec(),
        ]);
    }
}

//! Redis command builders.
//!
//! Provides type-safe builders for Redis commands that output
//! client-agnostic RESP3 byte sequences via [`RespCommand`].

/// Redis command output type.
pub mod resp;
/// Core builder trait.
pub mod command;
/// Key type for Redis keys.
pub mod key;
/// Value serialization to Redis bytes.
pub mod value;
/// String data structure commands.
pub mod string;
/// Hash data structure commands.
pub mod hash;
/// List data structure commands.
pub mod list;
/// Set data structure commands.
pub mod set;
/// Sorted set data structure commands.
pub mod zset;
/// Transaction and scripting commands.
pub mod transaction;

pub use command::CommandStatementBuilder;
pub use hash::HashCommand;
pub use key::{IntoRedisKey, RedisKey};
pub use list::ListCommand;
pub use resp::RespCommand;
pub use set::SetCommand;
pub use string::StringCommand;
pub use transaction::{ScriptCommand, Transaction};
pub use value::ToRedisBytes;
pub use zset::ZSetCommand;

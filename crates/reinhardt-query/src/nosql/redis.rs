//! Redis command builders.
//!
//! Provides type-safe builders for Redis commands that output
//! client-agnostic RESP3 byte sequences via [`RespCommand`].

/// Core builder trait.
pub mod command;
/// Hash data structure commands.
pub mod hash;
/// Key type for Redis keys.
pub mod key;
/// List data structure commands.
pub mod list;
/// Redis command output type.
pub mod resp;
/// Set data structure commands.
pub mod set;
/// String data structure commands.
pub mod string;
/// Transaction and scripting commands.
pub mod transaction;
/// Value serialization to Redis bytes.
pub mod value;
/// Sorted set data structure commands.
pub mod zset;

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

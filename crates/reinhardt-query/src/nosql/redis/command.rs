//! Core trait for Redis command builders.

use std::fmt::Debug;

use super::resp::RespCommand;

/// Trait for building Redis commands into RESP3 wire format.
///
/// Analogous to `QueryStatementBuilder` for SQL statements.
/// All Redis command builders implement this trait.
pub trait CommandStatementBuilder: Debug {
    /// Build this command into a [`RespCommand`] ready for RESP3 serialization.
    fn build(&self) -> RespCommand;
}

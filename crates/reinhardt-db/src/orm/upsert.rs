//! Typed assignment views and normalized plans for atomic upsert operations.

pub(crate) mod assignment;
pub(crate) mod plan;
pub(crate) mod sql;

pub use assignment::{UpsertCreate, UpsertWrite};

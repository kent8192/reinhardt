//! Typed assignment views and normalized plans for atomic upsert operations.

pub(crate) mod assignment;
pub(crate) mod plan;

pub use assignment::{UpsertCreate, UpsertWrite};

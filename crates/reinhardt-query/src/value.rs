//! Value types for representing SQL values.
//!
//! This module provides the core value types used throughout the query builder:
//!
//! - [`Value`]: The main enum representing all SQL value types
//! - [`ArrayType`]: Enum representing array element types
//! - [`ValueTuple`]: Enum for tuple values (used in IN clauses, etc.)
//! - [`Values`]: Wrapper struct for collected query parameters
//! - [`IntoValue`]: Trait for converting Rust types to `Value`

mod array;
mod core;
mod into_value;
mod tuple;
mod values;

pub use array::ArrayType;
pub use core::Value;
pub use into_value::IntoValue;
pub use tuple::ValueTuple;
pub use values::Values;

#[cfg(test)]
mod tests;

//! TBD DSL for auto-generating values in `.example.toml` files.
//!
//! This module provides a typed DSL that transforms `![expression]` markers
//! in template files and replaces them with valid TOML values.

pub mod error;

pub use error::{EvalErrorKind, Span, TbdError};

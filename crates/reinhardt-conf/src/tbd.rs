//! TBD DSL for auto-generating values in `.example.toml` files.
//!
//! This module provides a typed DSL that transforms `![expression]` markers
//! in template files and replaces them with valid TOML values.

pub mod ast;
pub mod error;

pub use ast::{BinOp, Expr, Literal, NumberValue, SpannedExpr};
pub use error::{EvalErrorKind, Span, TbdError};

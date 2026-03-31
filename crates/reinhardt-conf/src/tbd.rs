//! TBD DSL for auto-generating values in `.example.toml` files.
//!
//! This module provides a typed DSL that transforms `![expression]` markers
//! in template files and replaces them with valid TOML values.

pub mod ast;
pub mod error;
pub mod evaluator;
pub mod parser;
pub mod typechecker;
pub mod types;

pub use ast::{BinOp, Expr, Literal, NumberValue, SpannedExpr};
pub use error::{EvalErrorKind, Span, TbdError};
pub use evaluator::generate;
pub use parser::parse_expression;
pub use typechecker::typecheck;
pub use types::DslType;

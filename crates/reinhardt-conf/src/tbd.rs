//! TBD DSL for auto-generating values in `.example.toml` files.
//!
//! This module provides a typed DSL that transforms `![expression]` markers
//! in template files and replaces them with valid TOML values.

pub mod ast;
pub mod error;
pub mod evaluator;
pub mod parser;
pub mod processor;
pub mod typechecker;
pub mod types;

pub use ast::{BinOp, Expr, Literal, NumberValue, SpannedExpr};
pub use error::{EvalErrorKind, Span, TbdError};
pub use evaluator::generate;
pub use parser::parse_expression;
pub use processor::process_template;
pub use typechecker::typecheck;
pub use types::DslType;

/// Process a single DSL expression and return a [`toml::Value`].
///
/// This is a convenience function that runs the full pipeline:
/// parse -> typecheck -> generate.
///
/// # Errors
///
/// Returns a [`TbdError`] if parsing, type-checking, or evaluation fails.
pub fn evaluate_expression(expr: &str) -> Result<toml::Value, TbdError> {
	let ast = parser::parse_expression(expr)?;
	let _ty = typechecker::typecheck(&ast)?;
	evaluator::generate(&ast)
}

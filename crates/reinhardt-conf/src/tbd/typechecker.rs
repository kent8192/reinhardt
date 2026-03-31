//! Type checker for the TBD DSL.
//!
//! Performs type inference and validation on parsed AST expressions,
//! ensuring that pipe chains, function calls, and operators receive
//! operands of the correct types.

use crate::tbd::ast::{Expr, Literal, SpannedExpr};
use crate::tbd::error::TbdError;
use crate::tbd::types::DslType;

/// Type-checks a spanned expression and returns its inferred [`DslType`].
///
/// Returns a [`TbdError::TypeError`] when the expression violates the
/// DSL type rules (e.g. piping a primitive into `random`).
pub fn typecheck(expr: &SpannedExpr) -> Result<DslType, TbdError> {
	match &expr.expr {
		Expr::Literal(lit) => Ok(typecheck_literal(lit)),

		Expr::Identifier(name) => typecheck_identifier(name, expr),

		Expr::Pipe { left, right } => typecheck_pipe(left, right, expr),

		Expr::FunctionCall { name, args } => typecheck_function_call(name, args, expr),

		Expr::BinaryOp { left, right, .. } => typecheck_binary_op(left, right, expr),

		Expr::Tuple(elements) => typecheck_tuple(elements),

		Expr::Array(elements) => typecheck_array(elements, expr),

		Expr::Expansion(inner) => typecheck(inner),
	}
}

/// Infers the type of a literal value.
fn typecheck_literal(lit: &Literal) -> DslType {
	match lit {
		Literal::Number(_) => DslType::Number,
		Literal::String(_) => DslType::String,
		Literal::Boolean(_) => DslType::Boolean,
	}
}

/// Type-checks a bare identifier.
///
/// Only `ulid` is valid standalone; strategy keywords (`TBD`, `random`,
/// `sequential`, `fixed`) require pipe context and produce errors here.
fn typecheck_identifier(name: &str, expr: &SpannedExpr) -> Result<DslType, TbdError> {
	match name {
		"ulid" => Ok(DslType::Rule(Box::new(DslType::String))),
		"TBD" | "random" | "sequential" | "fixed" => Err(TbdError::TypeError {
			expected: "pipe context".into(),
			found: format!("standalone identifier `{name}`"),
			span: expr.span,
		}),
		_ => Err(TbdError::TypeError {
			expected: "known identifier".into(),
			found: format!("unknown identifier `{name}`"),
			span: expr.span,
		}),
	}
}

/// Type-checks a pipe expression `left | right`.
///
/// The right-hand side determines the transformation applied to the
/// left-hand type. See module-level documentation for the full rule set.
fn typecheck_pipe(
	left: &SpannedExpr,
	right: &SpannedExpr,
	_pipe_expr: &SpannedExpr,
) -> Result<DslType, TbdError> {
	let left_type = typecheck(left)?;

	match &right.expr {
		// `fixed` converts a primitive into a Rule<T>
		Expr::Identifier(name) if name == "fixed" => {
			if left_type.is_primitive() {
				Ok(DslType::Rule(Box::new(left_type)))
			} else {
				Err(TbdError::TypeError {
					expected: "primitive type (Number, String, or Boolean)".into(),
					found: left_type.to_string(),
					span: left.span,
				})
			}
		}

		// `random` and `sequential` require Rule<T> and return Rule<T>
		Expr::Identifier(name) if name == "random" || name == "sequential" => {
			if left_type.is_rule() {
				Ok(left_type)
			} else {
				Err(TbdError::TypeError {
					expected: "Rule<T>".into(),
					found: left_type.to_string(),
					span: left.span,
				})
			}
		}

		// `TBD` unwraps Rule<T> into T
		Expr::Identifier(name) if name == "TBD" => {
			if let Some(inner) = left_type.inner_rule_type() {
				Ok(inner.clone())
			} else {
				Err(TbdError::TypeError {
					expected: "Rule<T>".into(),
					found: left_type.to_string(),
					span: left.span,
				})
			}
		}

		// `range(n, m)` and `seed(n)` require Rule<T> and return Rule<T>
		Expr::FunctionCall { name, .. } if name == "range" || name == "seed" => {
			if left_type.is_rule() {
				Ok(left_type)
			} else {
				Err(TbdError::TypeError {
					expected: "Rule<T>".into(),
					found: left_type.to_string(),
					span: left.span,
				})
			}
		}

		// For any other right-hand expression, type-check it independently
		_ => {
			let right_type = typecheck(right)?;
			Ok(right_type)
		}
	}
}

/// Type-checks a standalone function call.
fn typecheck_function_call(
	name: &str,
	_args: &[SpannedExpr],
	expr: &SpannedExpr,
) -> Result<DslType, TbdError> {
	match name {
		"regex" => Ok(DslType::Rule(Box::new(DslType::String))),
		"range" => Ok(DslType::Rule(Box::new(DslType::Number))),
		"seed" => Ok(DslType::Rule(Box::new(DslType::Number))),
		_ => Err(TbdError::TypeError {
			expected: "known function".into(),
			found: format!("unknown function `{name}`"),
			span: expr.span,
		}),
	}
}

/// Type-checks a binary arithmetic operation.
///
/// Both operands must be `Number`; the result is always `Number`.
fn typecheck_binary_op(
	left: &SpannedExpr,
	right: &SpannedExpr,
	_expr: &SpannedExpr,
) -> Result<DslType, TbdError> {
	let left_type = typecheck(left)?;
	if left_type != DslType::Number {
		return Err(TbdError::TypeError {
			expected: "Number".into(),
			found: left_type.to_string(),
			span: left.span,
		});
	}

	let right_type = typecheck(right)?;
	if right_type != DslType::Number {
		return Err(TbdError::TypeError {
			expected: "Number".into(),
			found: right_type.to_string(),
			span: right.span,
		});
	}

	Ok(DslType::Number)
}

/// Type-checks a tuple by inferring the type of each element.
fn typecheck_tuple(elements: &[SpannedExpr]) -> Result<DslType, TbdError> {
	let types: Vec<DslType> = elements.iter().map(typecheck).collect::<Result<_, _>>()?;
	Ok(DslType::Tuple(types))
}

/// Type-checks an array, ensuring all elements share the same type.
fn typecheck_array(elements: &[SpannedExpr], expr: &SpannedExpr) -> Result<DslType, TbdError> {
	if elements.is_empty() {
		return Err(TbdError::TypeError {
			expected: "non-empty array".into(),
			found: "empty array".into(),
			span: expr.span,
		});
	}

	let first_type = typecheck(&elements[0])?;
	for elem in &elements[1..] {
		let elem_type = typecheck(elem)?;
		if elem_type != first_type {
			return Err(TbdError::TypeError {
				expected: first_type.to_string(),
				found: elem_type.to_string(),
				span: elem.span,
			});
		}
	}

	Ok(DslType::Array(Box::new(first_type)))
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::tbd::parser::parse_expression;
	use rstest::rstest;

	/// Helper that parses and type-checks an expression string.
	fn check(input: &str) -> Result<DslType, TbdError> {
		let expr = parse_expression(input).expect("parse should succeed");
		typecheck(&expr)
	}

	// Happy path: full pipeline produces expected output type
	#[rstest]
	#[case("false | fixed | TBD", DslType::Boolean)]
	#[case("true | fixed | TBD", DslType::Boolean)]
	#[case("42 | fixed | TBD", DslType::Number)]
	#[case(r#""hello" | fixed | TBD"#, DslType::String)]
	#[case("regex([a-z]*) | range(10, 20) | random | TBD", DslType::String)]
	#[case("ulid | TBD", DslType::String)]
	#[case("1 + 2", DslType::Number)]
	fn test_typecheck_ok(#[case] input: &str, #[case] expected: DslType) {
		// Arrange
		// (provided via #[case] parameters)

		// Act
		let result = check(input);

		// Assert
		assert_eq!(result.unwrap(), expected);
	}

	// Error path: type mismatches produce errors
	#[rstest]
	#[case("1 + 3 | TBD")]
	#[case("regex([a-z]*) | fixed")]
	#[case("true | range(1, 5)")]
	#[case("true | random")]
	fn test_typecheck_error(#[case] input: &str) {
		// Arrange
		// (provided via #[case] parameters)

		// Act
		let result = check(input);

		// Assert
		assert!(result.is_err(), "expected type error for: {input}");
	}

	// Decision table: `fixed` wraps primitives into Rule<T>
	#[rstest]
	#[case("false | fixed", DslType::Rule(Box::new(DslType::Boolean)))]
	#[case("42 | fixed", DslType::Rule(Box::new(DslType::Number)))]
	#[case(r#""s" | fixed"#, DslType::Rule(Box::new(DslType::String)))]
	fn test_fixed_produces_rule(#[case] input: &str, #[case] expected: DslType) {
		// Arrange
		// (provided via #[case] parameters)

		// Act
		let result = check(input);

		// Assert
		assert_eq!(result.unwrap(), expected);
	}

	// Decision table: `TBD` unwraps Rule<T> back to T
	#[rstest]
	#[case("false | fixed | TBD", DslType::Boolean)]
	#[case("42 | fixed | TBD", DslType::Number)]
	#[case(r#""s" | fixed | TBD"#, DslType::String)]
	#[case("regex([a-z]*) | random | TBD", DslType::String)]
	#[case("ulid | TBD", DslType::String)]
	fn test_tbd_type_inference(#[case] input: &str, #[case] expected: DslType) {
		// Arrange
		// (provided via #[case] parameters)

		// Act
		let result = check(input);

		// Assert
		assert_eq!(result.unwrap(), expected);
	}
}

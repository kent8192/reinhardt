//! Evaluator for TBD DSL expressions.
//!
//! Converts a parsed [`SpannedExpr`] AST into a concrete [`toml::Value`] by
//! evaluating literals, arithmetic, pipe-based rule construction, and
//! value generation via strategies (random, sequential, fixed).

use rand::Rng;
use rand::SeedableRng;

use crate::tbd::ast::{BinOp, Expr, Literal, NumberValue, SpannedExpr};
use crate::tbd::error::{EvalErrorKind, Span, TbdError};

/// Generation strategy applied when the `TBD` terminal is reached.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Strategy {
	/// No strategy has been set yet.
	None,
	/// Generate a random value.
	Random,
	/// Generate a sequential value.
	Sequential,
}

/// Internal representation of a rule being built through the pipe chain.
#[derive(Debug, Clone)]
enum RuleValue {
	/// A fixed literal value to be returned as-is.
	Fixed(toml::Value),
	/// A regex-based string generator.
	Regex {
		/// The regex pattern string.
		pattern: String,
		/// Minimum repetition length.
		min_len: usize,
		/// Maximum repetition length.
		max_len: usize,
		/// The generation strategy.
		strategy: Strategy,
		/// Optional seed for deterministic generation.
		seed: Option<u64>,
	},
	/// A ULID generator.
	Ulid,
	/// An integer range generator.
	/// Currently matched in `apply_range`, `apply_seed`, `set_strategy`, and
	/// `generate_from_rule` but not yet constructed through the DSL pipe chain.
	/// Construction will be added when the `int` generator function is implemented.
	#[allow(dead_code)]
	NumberRange {
		/// Minimum value (inclusive).
		min: i64,
		/// Maximum value (inclusive).
		max: i64,
		/// The generation strategy.
		strategy: Strategy,
		/// Optional seed for deterministic generation.
		seed: Option<u64>,
	},
}

/// Result of evaluating a single pipe stage.
#[derive(Debug)]
enum PipeResult {
	/// A rule still being constructed.
	Rule(RuleValue),
	/// A fully evaluated TOML value.
	Value(toml::Value),
}

/// Evaluates a TBD DSL expression and produces a concrete TOML value.
///
/// This is the main entry point for the evaluator. It takes a parsed AST and
/// recursively evaluates it, handling literals, arithmetic, pipe chains, and
/// value generation.
///
/// # Errors
///
/// Returns [`TbdError::EvalError`] for runtime evaluation failures such as
/// division by zero, invalid regex patterns, missing strategies, or invalid
/// ranges.
pub fn generate(expr: &SpannedExpr) -> Result<toml::Value, TbdError> {
	match evaluate_inner(expr)? {
		PipeResult::Value(v) => Ok(v),
		PipeResult::Rule(rule) => {
			// A rule without TBD terminal: generate with current state
			generate_from_rule(&rule, expr.span)
		}
	}
}

/// Recursively evaluates an expression, returning a `PipeResult`.
fn evaluate_inner(expr: &SpannedExpr) -> Result<PipeResult, TbdError> {
	match &expr.expr {
		Expr::Literal(lit) => Ok(PipeResult::Value(literal_to_toml(lit))),

		Expr::BinaryOp { op, left, right } => {
			let lv = evaluate_to_value(left)?;
			let rv = evaluate_to_value(right)?;
			let result = evaluate_binop(*op, &lv, &rv, expr.span)?;
			Ok(PipeResult::Value(result))
		}

		Expr::Pipe { left, right } => evaluate_pipe(left, right, expr.span),

		Expr::FunctionCall { name, args } => evaluate_function_call(name, args, expr.span),

		Expr::Identifier(ident) => evaluate_identifier(ident, expr.span),

		Expr::Array(elements) => {
			let mut values = Vec::with_capacity(elements.len());
			for elem in elements {
				values.push(evaluate_to_value(elem)?);
			}
			Ok(PipeResult::Value(toml::Value::Array(values)))
		}

		Expr::Expansion(inner) => evaluate_inner(inner),

		Expr::Tuple(_) => Err(TbdError::EvalError {
			kind: EvalErrorKind::UnknownFunction("tuple".into()),
			span: expr.span,
		}),
	}
}

/// Evaluates an expression and extracts the final `toml::Value`.
fn evaluate_to_value(expr: &SpannedExpr) -> Result<toml::Value, TbdError> {
	match evaluate_inner(expr)? {
		PipeResult::Value(v) => Ok(v),
		PipeResult::Rule(rule) => generate_from_rule(&rule, expr.span),
	}
}

/// Converts a literal AST node to a TOML value.
fn literal_to_toml(lit: &Literal) -> toml::Value {
	match lit {
		Literal::Number(NumberValue::Int(i)) => toml::Value::Integer(*i),
		Literal::Number(NumberValue::Float(f)) => toml::Value::Float(*f),
		Literal::String(s) => toml::Value::String(s.clone()),
		Literal::Boolean(b) => toml::Value::Boolean(*b),
	}
}

/// Evaluates a binary arithmetic operation on two TOML integer values.
fn evaluate_binop(
	op: BinOp,
	left: &toml::Value,
	right: &toml::Value,
	span: Span,
) -> Result<toml::Value, TbdError> {
	let lv = match left {
		toml::Value::Integer(i) => *i,
		_ => {
			return Err(TbdError::TypeError {
				expected: "integer".into(),
				found: format!("{left:?}"),
				span,
			});
		}
	};
	let rv = match right {
		toml::Value::Integer(i) => *i,
		_ => {
			return Err(TbdError::TypeError {
				expected: "integer".into(),
				found: format!("{right:?}"),
				span,
			});
		}
	};

	let result = match op {
		BinOp::Add => lv + rv,
		BinOp::Sub => lv - rv,
		BinOp::Mul => lv * rv,
		BinOp::Div => {
			if rv == 0 {
				return Err(TbdError::EvalError {
					kind: EvalErrorKind::DivisionByZero,
					span,
				});
			}
			lv / rv
		}
	};

	Ok(toml::Value::Integer(result))
}

/// Evaluates a pipe expression by applying the right-hand side to the
/// left-hand side result.
fn evaluate_pipe(
	left: &SpannedExpr,
	right: &SpannedExpr,
	_span: Span,
) -> Result<PipeResult, TbdError> {
	let left_result = evaluate_inner(left)?;

	// Check what the right side is
	match &right.expr {
		Expr::Identifier(ident) => apply_pipe_ident(&left_result, ident, right.span),
		Expr::FunctionCall { name, args } => {
			apply_pipe_function(&left_result, name, args, right.span)
		}
		_ => {
			// Nested pipe or other expression: evaluate right side independently
			evaluate_inner(right)
		}
	}
}

/// Applies a bare identifier as a pipe stage.
fn apply_pipe_ident(left: &PipeResult, ident: &str, span: Span) -> Result<PipeResult, TbdError> {
	match ident {
		"fixed" => {
			// Convert left value to Fixed rule
			let value = match left {
				PipeResult::Value(v) => v.clone(),
				PipeResult::Rule(rule) => generate_from_rule(rule, span)?,
			};
			Ok(PipeResult::Rule(RuleValue::Fixed(value)))
		}
		"random" => {
			// Set strategy to Random on the current rule
			let rule = match left {
				PipeResult::Rule(rule) => {
					let mut rule = rule.clone();
					set_strategy(&mut rule, Strategy::Random);
					rule
				}
				PipeResult::Value(_) => {
					return Err(TbdError::EvalError {
						kind: EvalErrorKind::UnknownFunction("random on value".into()),
						span,
					});
				}
			};
			Ok(PipeResult::Rule(rule))
		}
		"sequential" => {
			// Set strategy to Sequential on the current rule
			let rule = match left {
				PipeResult::Rule(rule) => {
					let mut rule = rule.clone();
					set_strategy(&mut rule, Strategy::Sequential);
					rule
				}
				PipeResult::Value(_) => {
					return Err(TbdError::EvalError {
						kind: EvalErrorKind::UnknownFunction("sequential on value".into()),
						span,
					});
				}
			};
			Ok(PipeResult::Rule(rule))
		}
		"TBD" => {
			// Terminal: generate the final value
			match left {
				PipeResult::Rule(rule) => {
					let value = generate_from_rule(rule, span)?;
					Ok(PipeResult::Value(value))
				}
				PipeResult::Value(v) => Ok(PipeResult::Value(v.clone())),
			}
		}
		_ => Err(TbdError::EvalError {
			kind: EvalErrorKind::UnknownFunction(ident.into()),
			span,
		}),
	}
}

/// Applies a function call as a pipe stage.
fn apply_pipe_function(
	left: &PipeResult,
	name: &str,
	args: &[SpannedExpr],
	span: Span,
) -> Result<PipeResult, TbdError> {
	match name {
		"range" => {
			if args.len() != 2 {
				return Err(TbdError::EvalError {
					kind: EvalErrorKind::UnknownFunction(
						"range requires exactly 2 arguments".into(),
					),
					span,
				});
			}
			let min_val = evaluate_to_value(&args[0])?;
			let max_val = evaluate_to_value(&args[1])?;

			let min_i = match &min_val {
				toml::Value::Integer(i) => *i,
				_ => {
					return Err(TbdError::TypeError {
						expected: "integer".into(),
						found: format!("{min_val:?}"),
						span,
					});
				}
			};
			let max_i = match &max_val {
				toml::Value::Integer(i) => *i,
				_ => {
					return Err(TbdError::TypeError {
						expected: "integer".into(),
						found: format!("{max_val:?}"),
						span,
					});
				}
			};

			let rule = match left {
				PipeResult::Rule(rule) => {
					let mut rule = rule.clone();
					apply_range(&mut rule, min_i, max_i);
					rule
				}
				PipeResult::Value(_) => {
					return Err(TbdError::EvalError {
						kind: EvalErrorKind::UnknownFunction("range on value".into()),
						span,
					});
				}
			};
			Ok(PipeResult::Rule(rule))
		}
		"seed" => {
			if args.len() != 1 {
				return Err(TbdError::EvalError {
					kind: EvalErrorKind::UnknownFunction("seed requires exactly 1 argument".into()),
					span,
				});
			}
			let seed_val = evaluate_to_value(&args[0])?;
			let seed = match &seed_val {
				toml::Value::Integer(i) => *i as u64,
				_ => {
					return Err(TbdError::TypeError {
						expected: "integer".into(),
						found: format!("{seed_val:?}"),
						span,
					});
				}
			};

			let rule = match left {
				PipeResult::Rule(rule) => {
					let mut rule = rule.clone();
					apply_seed(&mut rule, seed);
					rule
				}
				PipeResult::Value(_) => {
					return Err(TbdError::EvalError {
						kind: EvalErrorKind::UnknownFunction("seed on value".into()),
						span,
					});
				}
			};
			Ok(PipeResult::Rule(rule))
		}
		_ => Err(TbdError::EvalError {
			kind: EvalErrorKind::UnknownFunction(name.into()),
			span,
		}),
	}
}

/// Evaluates a standalone function call (not in pipe context).
fn evaluate_function_call(
	name: &str,
	args: &[SpannedExpr],
	span: Span,
) -> Result<PipeResult, TbdError> {
	match name {
		"regex" => {
			if args.len() != 1 {
				return Err(TbdError::EvalError {
					kind: EvalErrorKind::UnknownFunction(
						"regex requires exactly 1 argument".into(),
					),
					span,
				});
			}
			let pattern = match &args[0].expr {
				Expr::Literal(Literal::String(s)) => s.clone(),
				_ => {
					return Err(TbdError::TypeError {
						expected: "string".into(),
						found: format!("{:?}", args[0].expr),
						span,
					});
				}
			};
			Ok(PipeResult::Rule(RuleValue::Regex {
				pattern,
				min_len: 1,
				max_len: 1,
				strategy: Strategy::None,
				seed: None,
			}))
		}
		_ => Err(TbdError::EvalError {
			kind: EvalErrorKind::UnknownFunction(name.into()),
			span,
		}),
	}
}

/// Evaluates a bare identifier.
fn evaluate_identifier(ident: &str, span: Span) -> Result<PipeResult, TbdError> {
	match ident {
		"ulid" => Ok(PipeResult::Rule(RuleValue::Ulid)),
		"TBD" | "fixed" | "random" | "sequential" => Err(TbdError::EvalError {
			kind: EvalErrorKind::MissingStrategy,
			span,
		}),
		_ => Err(TbdError::EvalError {
			kind: EvalErrorKind::UnknownFunction(ident.into()),
			span,
		}),
	}
}

/// Sets the generation strategy on a rule value.
fn set_strategy(rule: &mut RuleValue, strategy: Strategy) {
	match rule {
		RuleValue::Regex { strategy: s, .. } => *s = strategy,
		RuleValue::NumberRange { strategy: s, .. } => *s = strategy,
		RuleValue::Fixed(_) | RuleValue::Ulid => {
			// Fixed and Ulid don't use strategies; silently ignore
		}
	}
}

/// Applies a range modifier to a rule value.
fn apply_range(rule: &mut RuleValue, min: i64, max: i64) {
	match rule {
		RuleValue::Regex {
			min_len, max_len, ..
		} => {
			*min_len = min as usize;
			*max_len = max as usize;
		}
		RuleValue::NumberRange {
			min: rule_min,
			max: rule_max,
			..
		} => {
			*rule_min = min;
			*rule_max = max;
		}
		RuleValue::Fixed(_) | RuleValue::Ulid => {
			// Silently ignore range on fixed/ulid
		}
	}
}

/// Applies a seed to a rule value.
fn apply_seed(rule: &mut RuleValue, seed_val: u64) {
	match rule {
		RuleValue::Regex { seed, .. } => *seed = Some(seed_val),
		RuleValue::NumberRange { seed, .. } => *seed = Some(seed_val),
		RuleValue::Fixed(_) | RuleValue::Ulid => {
			// Silently ignore seed on fixed/ulid
		}
	}
}

/// Builds a regex pattern with explicit repetition bounds.
///
/// If the source pattern ends with an unbounded quantifier (`*` or `+`),
/// it is replaced with `{min_len,max_len}`. Otherwise the entire pattern
/// is wrapped in a non-capturing group and the range is appended.
fn build_regex_pattern(pattern: &str, min_len: usize, max_len: usize) -> String {
	if let Some(base) = pattern.strip_suffix('*') {
		// Replace `*` (0 or more) with explicit range
		format!("{base}{{{min_len},{max_len}}}")
	} else if let Some(base) = pattern.strip_suffix('+') {
		// Replace `+` (1 or more) with explicit range
		format!("{base}{{{min_len},{max_len}}}")
	} else {
		// No trailing quantifier: wrap and repeat the whole pattern
		format!("(?:{pattern}){{{min_len},{max_len}}}")
	}
}

/// Generates a concrete TOML value from a fully-constructed rule.
fn generate_from_rule(rule: &RuleValue, span: Span) -> Result<toml::Value, TbdError> {
	match rule {
		RuleValue::Fixed(v) => Ok(v.clone()),

		RuleValue::Ulid => {
			let id = ulid::Ulid::new().to_string();
			Ok(toml::Value::String(id))
		}

		RuleValue::Regex {
			pattern,
			min_len,
			max_len,
			strategy,
			seed,
		} => {
			if *strategy == Strategy::None {
				return Err(TbdError::EvalError {
					kind: EvalErrorKind::MissingStrategy,
					span,
				});
			}
			if min_len > max_len {
				return Err(TbdError::EvalError {
					kind: EvalErrorKind::RangeInvalid {
						min: *min_len as i64,
						max: *max_len as i64,
					},
					span,
				});
			}

			// Build the full regex pattern by replacing trailing unbounded
			// quantifiers (`*`, `+`) with an explicit `{min,max}` range.
			// If the pattern has no trailing quantifier, wrap the whole
			// pattern in a group and apply the repetition count.
			let full_pattern = build_regex_pattern(pattern, *min_len, *max_len);
			let regex = rand_regex::Regex::compile(&full_pattern, (*max_len as u32).max(1))
				.map_err(|e| TbdError::EvalError {
					kind: EvalErrorKind::InvalidRegexPattern(e.to_string()),
					span,
				})?;

			let generated: String = if let Some(s) = seed {
				let mut rng = rand::rngs::StdRng::seed_from_u64(*s);
				rng.sample(&regex)
			} else {
				let mut rng = rand::rng();
				rng.sample(&regex)
			};

			Ok(toml::Value::String(generated))
		}

		RuleValue::NumberRange {
			min,
			max,
			strategy,
			seed,
		} => {
			if *strategy == Strategy::None {
				return Err(TbdError::EvalError {
					kind: EvalErrorKind::MissingStrategy,
					span,
				});
			}
			if min > max {
				return Err(TbdError::EvalError {
					kind: EvalErrorKind::RangeInvalid {
						min: *min,
						max: *max,
					},
					span,
				});
			}

			let value = if let Some(s) = seed {
				let mut rng = rand::rngs::StdRng::seed_from_u64(*s);
				rng.random_range(*min..=*max)
			} else {
				let mut rng = rand::rng();
				rng.random_range(*min..=*max)
			};

			Ok(toml::Value::Integer(value))
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::tbd::parser::parse_expression;
	use rstest::rstest;

	/// Helper to parse and generate a value from a DSL expression string.
	fn run_generate(input: &str) -> Result<toml::Value, TbdError> {
		let expr = parse_expression(input).expect("parse should succeed");
		generate(&expr)
	}

	// Happy path: fixed values
	#[rstest]
	#[case("false | fixed | TBD", toml::Value::Boolean(false))]
	#[case("true | fixed | TBD", toml::Value::Boolean(true))]
	#[case("8080 | fixed | TBD", toml::Value::Integer(8080))]
	#[case("3.14 | fixed | TBD", toml::Value::Float(3.14))]
	#[case(r#""hello" | fixed | TBD"#, toml::Value::String("hello".into()))]
	fn test_generate_fixed(#[case] input: &str, #[case] expected: toml::Value) {
		// Act
		let result = run_generate(input).expect("should succeed");
		// Assert
		assert_eq!(result, expected);
	}

	// Arithmetic operations
	#[rstest]
	#[case("1 + 2", 3)]
	#[case("10 - 3", 7)]
	#[case("4 * 5", 20)]
	#[case("10 / 3", 3)]
	fn test_generate_arithmetic(#[case] input: &str, #[case] expected: i64) {
		// Act
		let result = run_generate(input).expect("should succeed");
		// Assert
		assert_eq!(result, toml::Value::Integer(expected));
	}

	// Seeded random produces deterministic results
	#[rstest]
	#[case("regex([a-z]*) | range(10, 10) | random | seed(42) | TBD")]
	#[case("regex([0-9]*) | range(5, 5) | random | seed(0) | TBD")]
	fn test_seeded_random_deterministic(#[case] input: &str) {
		// Act
		let result1 = run_generate(input).expect("first generation should succeed");
		let result2 = run_generate(input).expect("second generation should succeed");
		// Assert
		assert_eq!(
			result1, result2,
			"seeded random should produce identical results"
		);
	}

	// ULID format: 26 alphanumeric characters (Crockford Base32)
	#[rstest]
	fn test_generate_ulid_format() {
		// Arrange
		let input = "ulid | random | TBD";
		// Act
		let result = run_generate(input).expect("should succeed");
		// Assert
		let s = result.as_str().expect("should be string");
		assert_eq!(s.len(), 26, "ULID should be 26 characters");
		assert!(
			s.chars().all(|c| c.is_ascii_alphanumeric()),
			"ULID should be alphanumeric, got: {s}"
		);
	}

	// Error: division by zero
	#[rstest]
	fn test_generate_division_by_zero() {
		// Act
		let result = run_generate("10 / 0");
		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(
			matches!(
				err,
				TbdError::EvalError {
					kind: EvalErrorKind::DivisionByZero,
					..
				}
			),
			"expected DivisionByZero, got: {err:?}"
		);
	}

	// Error: range invalid (min > max)
	#[rstest]
	fn test_generate_range_invalid() {
		// Act
		let result = run_generate("regex([a-z]*) | range(10, 5) | random | TBD");
		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(
			matches!(
				err,
				TbdError::EvalError {
					kind: EvalErrorKind::RangeInvalid { .. },
					..
				}
			),
			"expected RangeInvalid, got: {err:?}"
		);
	}

	// Boundary: range(1, 1) should produce exactly 1 character
	#[rstest]
	fn test_generate_range_boundary() {
		// Arrange
		let input = "regex([a-z]*) | range(1, 1) | random | seed(99) | TBD";
		// Act
		let result = run_generate(input).expect("should succeed");
		// Assert
		let s = result.as_str().expect("should be string");
		assert_eq!(s.len(), 1, "range(1,1) should produce exactly 1 character");
	}

	mod proptests {
		use super::*;
		use proptest::prelude::*;

		proptest! {
			// fixed always preserves the exact input integer value
			#[test]
			fn prop_fixed_preserves_int(n in -10000i64..10000) {
				let expr = format!("{n} | fixed | TBD");
				let result = run_generate(&expr).unwrap();
				prop_assert_eq!(result, toml::Value::Integer(n));
			}

			// same seed always produces identical output
			#[test]
			fn prop_same_seed_same_result(seed in 0u64..10000) {
				let expr = format!(
					"regex([a-z]*) | range(10, 10) | random | seed({seed}) | TBD"
				);
				let r1 = run_generate(&expr).unwrap();
				let r2 = run_generate(&expr).unwrap();
				prop_assert_eq!(r1, r2);
			}
		}
	}
}

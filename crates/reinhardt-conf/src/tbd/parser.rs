//! Winnow-based parser for TBD DSL expressions.
//!
//! Converts source text into [`SpannedExpr`] AST nodes with byte-offset spans.
//!
//! # Grammar
//!
//! ```text
//! expression     = pipe_expr EOF
//! pipe_expr      = additive ("|" additive)*
//! additive       = multiplicative (("+"|"-") multiplicative)*
//! multiplicative = unary (("*"|"/") unary)*
//! unary          = "*" unary | atom
//! atom           = "(" expr_list ")" | "[" expr_list "]" | string | number | boolean | func_or_ident
//! func_or_ident  = identifier ("(" args ")")?
//! ```

use winnow::ascii::{digit1, multispace0};
use winnow::combinator::{alt, opt, preceded};
use winnow::error::{ContextError, ErrMode, ModalResult};
use winnow::prelude::*;
use winnow::token::take_while;

use crate::tbd::ast::{BinOp, Expr, Literal, NumberValue, SpannedExpr};
use crate::tbd::error::{Span, TbdError};

/// Helper to create a backtrack error.
fn backtrack() -> ErrMode<ContextError> {
	ErrMode::Backtrack(ContextError::new())
}

/// Parses a complete TBD DSL expression from the given input string.
///
/// Returns a [`SpannedExpr`] on success or a [`TbdError::ParseError`] if the
/// input cannot be parsed. All input must be consumed; trailing text causes an
/// error.
pub fn parse_expression(input: &str) -> Result<SpannedExpr, TbdError> {
	let mut remaining = input;
	let result = parse_pipe_expr
		.parse_next(&mut remaining)
		.map_err(|_| TbdError::ParseError {
			message: "failed to parse expression".into(),
			span: Span {
				start: 0,
				end: input.len(),
			},
		})?;

	// Skip trailing whitespace and ensure all input is consumed
	let trimmed = remaining.trim();
	if !trimmed.is_empty() {
		let trailing_start = offset_in(input, remaining);
		return Err(TbdError::ParseError {
			message: format!("unexpected trailing text: {trimmed}"),
			span: Span {
				start: trailing_start,
				end: input.len(),
			},
		});
	}

	Ok(result)
}

/// Parses a pipe expression: `additive ("|" additive)*`.
///
/// Left-associative: `A | B | C` becomes `Pipe(Pipe(A, B), C)`.
fn parse_pipe_expr(input: &mut &str) -> ModalResult<SpannedExpr> {
	let mut left = parse_additive(input)?;

	loop {
		multispace0.parse_next(input)?;
		if input.starts_with('|') {
			*input = &input[1..];
			multispace0.parse_next(input)?;
			let right = parse_additive(input)?;
			let start = left.span.start;
			let end = right.span.end;
			left = SpannedExpr::new(
				Expr::Pipe {
					left: Box::new(left),
					right: Box::new(right),
				},
				Span { start, end },
			);
		} else {
			break;
		}
	}

	Ok(left)
}

/// Parses an additive expression: `multiplicative (("+"|"-") multiplicative)*`.
fn parse_additive(input: &mut &str) -> ModalResult<SpannedExpr> {
	let mut left = parse_multiplicative(input)?;

	loop {
		multispace0.parse_next(input)?;
		let op = if input.starts_with('+') {
			*input = &input[1..];
			BinOp::Add
		} else if input.starts_with('-') {
			// Disambiguate: `-` followed by a digit is a negative number in
			// primary, not a subtraction operator. However at this level we
			// already parsed the left operand, so `-` is always subtraction.
			// But we need to be careful: `-3` after whitespace could be a
			// negative literal. We treat it as subtraction here since we have
			// a left operand.
			*input = &input[1..];
			BinOp::Sub
		} else {
			break;
		};
		multispace0.parse_next(input)?;
		let right = parse_multiplicative(input)?;
		let start = left.span.start;
		let end = right.span.end;
		left = SpannedExpr::new(
			Expr::BinaryOp {
				op,
				left: Box::new(left),
				right: Box::new(right),
			},
			Span { start, end },
		);
	}

	Ok(left)
}

/// Parses a multiplicative expression: `unary (("*"|"/") unary)*`.
fn parse_multiplicative(input: &mut &str) -> ModalResult<SpannedExpr> {
	let mut left = parse_unary(input)?;

	loop {
		multispace0.parse_next(input)?;
		let op = if input.starts_with('*') {
			*input = &input[1..];
			BinOp::Mul
		} else if input.starts_with('/') {
			*input = &input[1..];
			BinOp::Div
		} else {
			break;
		};
		multispace0.parse_next(input)?;
		let right = parse_unary(input)?;
		let start = left.span.start;
		let end = right.span.end;
		left = SpannedExpr::new(
			Expr::BinaryOp {
				op,
				left: Box::new(left),
				right: Box::new(right),
			},
			Span { start, end },
		);
	}

	Ok(left)
}

/// Parses a unary expression: `"*" unary | atom`.
///
/// The prefix `*` is the expansion operator, wrapping the inner expression in
/// [`Expr::Expansion`].
fn parse_unary(input: &mut &str) -> ModalResult<SpannedExpr> {
	let full = *input;
	multispace0.parse_next(input)?;
	let start = offset_in(full, input);

	if input.starts_with('*') {
		*input = &input[1..];
		let inner = parse_unary(input)?;
		let end = inner.span.end;
		Ok(SpannedExpr::new(
			Expr::Expansion(Box::new(inner)),
			Span { start, end },
		))
	} else {
		parse_atom(input)
	}
}

/// Parses an atom: parenthesized expression/tuple, array, or primary literal /
/// function / identifier.
fn parse_atom(input: &mut &str) -> ModalResult<SpannedExpr> {
	multispace0.parse_next(input)?;

	if input.starts_with('(') {
		parse_paren_expr(input)
	} else if input.starts_with('[') {
		parse_array(input)
	} else {
		alt((
			parse_boolean,
			parse_number,
			parse_string_literal,
			parse_func_or_ident,
		))
		.parse_next(input)
	}
}

/// Parses a parenthesized expression, grouping, or tuple.
///
/// - `(expr)` is grouping (returns the inner expression as-is).
/// - `(expr, expr, ...)` is a tuple.
fn parse_paren_expr(input: &mut &str) -> ModalResult<SpannedExpr> {
	let full = *input;
	let start = offset_in(full, input);

	// Consume opening `(`
	"(".parse_next(input)?;
	multispace0.parse_next(input)?;

	let mut elements: Vec<SpannedExpr> = Vec::new();
	let mut has_comma = false;

	// Handle empty parens
	if !input.starts_with(')') {
		// Parse first element
		let first = parse_pipe_expr(input)?;
		elements.push(first);

		// Parse subsequent comma-separated elements
		loop {
			multispace0.parse_next(input)?;
			if input.starts_with(',') {
				has_comma = true;
				*input = &input[1..];
				multispace0.parse_next(input)?;
				// Allow trailing comma before `)`
				if input.starts_with(')') {
					break;
				}
				let elem = parse_pipe_expr(input)?;
				elements.push(elem);
			} else {
				break;
			}
		}
	}

	multispace0.parse_next(input)?;
	")".parse_next(input)?;
	let end = offset_in(full, input);

	if elements.len() == 1 && !has_comma {
		// Grouping: return the inner expression, preserving its span
		Ok(elements.into_iter().next().unwrap())
	} else {
		// Tuple
		Ok(SpannedExpr::new(Expr::Tuple(elements), Span { start, end }))
	}
}

/// Parses an array literal: `[expr, expr, ...]`.
fn parse_array(input: &mut &str) -> ModalResult<SpannedExpr> {
	let full = *input;
	let start = offset_in(full, input);

	// Consume opening `[`
	"[".parse_next(input)?;
	multispace0.parse_next(input)?;

	let mut elements: Vec<SpannedExpr> = Vec::new();

	if !input.starts_with(']') {
		let first = parse_pipe_expr(input)?;
		elements.push(first);

		loop {
			multispace0.parse_next(input)?;
			if input.starts_with(',') {
				*input = &input[1..];
				multispace0.parse_next(input)?;
				// Allow trailing comma before `]`
				if input.starts_with(']') {
					break;
				}
				let elem = parse_pipe_expr(input)?;
				elements.push(elem);
			} else {
				break;
			}
		}
	}

	multispace0.parse_next(input)?;
	"]".parse_next(input)?;
	let end = offset_in(full, input);

	Ok(SpannedExpr::new(Expr::Array(elements), Span { start, end }))
}

/// Parses a function call or bare identifier.
///
/// If an identifier is followed by `(`, it is treated as a function call.
/// Special case: `regex(...)` consumes the raw content between balanced
/// parentheses as a string literal argument.
fn parse_func_or_ident(input: &mut &str) -> ModalResult<SpannedExpr> {
	let full = *input;
	let start = offset_in(full, input);

	// Parse the identifier name
	let first: char = winnow::token::any
		.verify(|c: &char| c.is_alphabetic())
		.parse_next(input)?;

	let rest: &str =
		take_while(0.., |c: char| c.is_alphanumeric() || c == '_').parse_next(input)?;

	let mut name = String::with_capacity(1 + rest.len());
	name.push(first);
	name.push_str(rest);

	// Reject bare `true` / `false` so they are handled by the boolean parser
	if name == "true" || name == "false" {
		*input = &full[start..];
		return Err(backtrack());
	}

	// Check for function call
	multispace0.parse_next(input)?;
	if input.starts_with('(') {
		if name == "regex" {
			// Special case: consume raw pattern between balanced parentheses
			let args = parse_regex_raw_arg(input)?;
			let end = offset_in(full, input);
			Ok(SpannedExpr::new(
				Expr::FunctionCall { name, args },
				Span { start, end },
			))
		} else {
			let args = parse_call_args(input)?;
			let end = offset_in(full, input);
			Ok(SpannedExpr::new(
				Expr::FunctionCall { name, args },
				Span { start, end },
			))
		}
	} else {
		let end = offset_in(full, input);
		Ok(SpannedExpr::new(
			Expr::Identifier(name),
			Span { start, end },
		))
	}
}

/// Parses the raw argument for `regex(...)`.
///
/// Consumes everything between balanced parentheses as a single string literal.
/// Nested parentheses are tracked so that patterns like `regex((a|b))` work.
fn parse_regex_raw_arg(input: &mut &str) -> ModalResult<Vec<SpannedExpr>> {
	let full = *input;

	// Consume opening `(`
	"(".parse_next(input)?;
	let content_start = offset_in(full, input);

	let mut depth: usize = 1;
	let mut end_pos = 0;

	for (i, ch) in input.char_indices() {
		match ch {
			'(' => depth += 1,
			')' => {
				depth -= 1;
				if depth == 0 {
					end_pos = i;
					break;
				}
			}
			_ => {}
		}
	}

	if depth != 0 {
		return Err(backtrack());
	}

	let pattern = &input[..end_pos];
	*input = &input[end_pos..];

	// Consume closing `)`
	")".parse_next(input)?;

	let content_end = offset_in(full, input) - 1; // exclude closing paren

	let arg = SpannedExpr::new(
		Expr::Literal(Literal::String(pattern.to_string())),
		Span {
			start: content_start,
			end: content_end,
		},
	);

	Ok(vec![arg])
}

/// Parses comma-separated function call arguments: `"(" expr ("," expr)* ")"`.
fn parse_call_args(input: &mut &str) -> ModalResult<Vec<SpannedExpr>> {
	// Consume opening `(`
	"(".parse_next(input)?;
	multispace0.parse_next(input)?;

	let mut args: Vec<SpannedExpr> = Vec::new();

	if !input.starts_with(')') {
		let first = parse_pipe_expr(input)?;
		args.push(first);

		loop {
			multispace0.parse_next(input)?;
			if input.starts_with(',') {
				*input = &input[1..];
				multispace0.parse_next(input)?;
				if input.starts_with(')') {
					break;
				}
				let arg = parse_pipe_expr(input)?;
				args.push(arg);
			} else {
				break;
			}
		}
	}

	multispace0.parse_next(input)?;
	")".parse_next(input)?;

	Ok(args)
}

/// Parses `"true"` or `"false"` as a boolean literal.
///
/// Uses a word-boundary check so that identifiers like `truevalue` are not
/// mistakenly consumed as booleans.
fn parse_boolean(input: &mut &str) -> ModalResult<SpannedExpr> {
	let full = *input;
	let start = offset_in(full, input);

	let value = alt(("true", "false")).parse_next(input)?;

	// Word boundary check: the next character must not be alphanumeric or underscore
	if let Some(next_ch) = input.chars().next() {
		if next_ch.is_alphanumeric() || next_ch == '_' {
			// Restore input and fail so that the identifier parser can handle it
			*input = &full[start..];
			return Err(backtrack());
		}
	}

	let end = offset_in(full, input);
	let lit = Literal::Boolean(value == "true");
	Ok(SpannedExpr::new(Expr::Literal(lit), Span { start, end }))
}

/// Parses an integer or floating-point number with an optional leading `-`.
fn parse_number(input: &mut &str) -> ModalResult<SpannedExpr> {
	let full = *input;
	let start = offset_in(full, input);

	// Optional negative sign
	let neg: Option<&str> = opt("-").parse_next(input)?;

	// Integer part (one or more digits)
	let int_part: &str = digit1.parse_next(input)?;

	// Optional fractional part
	let frac: Option<&str> = opt(preceded(".", digit1)).parse_next(input)?;

	let end = offset_in(full, input);

	let expr = if let Some(frac_digits) = frac {
		// Build the float string and parse it
		let mut s = String::new();
		if neg.is_some() {
			s.push('-');
		}
		s.push_str(int_part);
		s.push('.');
		s.push_str(frac_digits);
		let f: f64 = s.parse().map_err(|_| backtrack())?;
		Expr::Literal(Literal::Number(NumberValue::Float(f)))
	} else {
		let mut s = String::new();
		if neg.is_some() {
			s.push('-');
		}
		s.push_str(int_part);
		let i: i64 = s.parse().map_err(|_| backtrack())?;
		Expr::Literal(Literal::Number(NumberValue::Int(i)))
	};

	Ok(SpannedExpr::new(expr, Span { start, end }))
}

/// Parses a double-quoted string literal with basic escape handling.
fn parse_string_literal(input: &mut &str) -> ModalResult<SpannedExpr> {
	let full = *input;
	let start = offset_in(full, input);

	// Opening quote
	"\"".parse_next(input)?;

	let mut value = String::new();
	loop {
		if input.is_empty() {
			return Err(backtrack());
		}
		let ch = input.chars().next().unwrap();
		*input = &input[ch.len_utf8()..];

		match ch {
			'"' => break,
			'\\' => {
				// Handle escape sequences
				if input.is_empty() {
					return Err(backtrack());
				}
				let escaped = input.chars().next().unwrap();
				*input = &input[escaped.len_utf8()..];
				match escaped {
					'n' => value.push('\n'),
					't' => value.push('\t'),
					'\\' => value.push('\\'),
					'"' => value.push('"'),
					_ => {
						value.push('\\');
						value.push(escaped);
					}
				}
			}
			other => value.push(other),
		}
	}

	let end = offset_in(full, input);
	Ok(SpannedExpr::new(
		Expr::Literal(Literal::String(value)),
		Span { start, end },
	))
}

/// Computes the byte offset of `current` within `original`.
fn offset_in(original: &str, current: &str) -> usize {
	original.len() - current.len()
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	#[case("true", Literal::Boolean(true))]
	#[case("false", Literal::Boolean(false))]
	fn test_parse_boolean(#[case] input: &str, #[case] expected: Literal) {
		// Act
		let result = parse_expression(input).unwrap();
		// Assert
		assert_eq!(result.expr, Expr::Literal(expected));
	}

	#[rstest]
	#[case("42", Literal::Number(NumberValue::Int(42)))]
	#[case("-7", Literal::Number(NumberValue::Int(-7)))]
	#[case("0", Literal::Number(NumberValue::Int(0)))]
	#[case("3.14", Literal::Number(NumberValue::Float(3.14)))]
	#[case("-2.5", Literal::Number(NumberValue::Float(-2.5)))]
	fn test_parse_number(#[case] input: &str, #[case] expected: Literal) {
		// Act
		let result = parse_expression(input).unwrap();
		// Assert
		assert_eq!(result.expr, Expr::Literal(expected));
	}

	#[rstest]
	#[case(r#""hello""#, Literal::String("hello".into()))]
	#[case(r#""""#, Literal::String(String::new()))]
	fn test_parse_string(#[case] input: &str, #[case] expected: Literal) {
		// Act
		let result = parse_expression(input).unwrap();
		// Assert
		assert_eq!(result.expr, Expr::Literal(expected));
	}

	#[rstest]
	#[case("TBD", "TBD")]
	#[case("random", "random")]
	#[case("fixed", "fixed")]
	#[case("ulid", "ulid")]
	#[case("sequential", "sequential")]
	fn test_parse_identifier(#[case] input: &str, #[case] expected_name: &str) {
		// Act
		let result = parse_expression(input).unwrap();
		// Assert
		assert_eq!(result.expr, Expr::Identifier(expected_name.into()));
	}

	#[rstest]
	fn test_parse_boolean_word_boundary() {
		// Arrange
		// `truevalue` should parse as an identifier, not as boolean `true`
		// Act
		let result = parse_expression("truevalue").unwrap();
		// Assert
		assert_eq!(result.expr, Expr::Identifier("truevalue".into()));
	}

	#[rstest]
	fn test_parse_span_tracking() {
		// Act
		let result = parse_expression("42").unwrap();
		// Assert
		assert_eq!(result.span, Span { start: 0, end: 2 });
	}

	// Function calls
	#[rstest]
	#[case("range(10, 20)")]
	#[case("seed(42)")]
	#[case("regex([a-z]*)")]
	fn test_parse_function_call(#[case] input: &str) {
		// Act
		let result = parse_expression(input).unwrap();
		// Assert
		assert!(matches!(result.expr, Expr::FunctionCall { .. }));
	}

	// Pipe
	#[rstest]
	#[case("false | fixed | TBD")]
	#[case("regex([a-z]*) | range(10, 20) | random | TBD")]
	#[case("8080 | fixed | TBD")]
	fn test_parse_pipe(#[case] input: &str) {
		// Act
		let result = parse_expression(input).unwrap();
		// Assert
		assert!(matches!(result.expr, Expr::Pipe { .. }));
	}

	// Arithmetic
	#[rstest]
	#[case("1 + 2")]
	#[case("2 + 3 * 4")]
	fn test_parse_arithmetic(#[case] input: &str) {
		// Act
		let result = parse_expression(input).unwrap();
		// Assert
		assert!(matches!(result.expr, Expr::BinaryOp { .. }));
	}

	// Tuple
	#[rstest]
	fn test_parse_tuple() {
		// Act
		let result = parse_expression("(5, 10)").unwrap();
		// Assert
		assert!(matches!(result.expr, Expr::Tuple(_)));
	}

	// Array
	#[rstest]
	fn test_parse_array() {
		// Act
		let result = parse_expression("[1, 2, 3]").unwrap();
		// Assert
		assert!(matches!(result.expr, Expr::Array(_)));
	}

	// Expansion
	#[rstest]
	fn test_parse_expansion_in_pipe() {
		// Act
		let result = parse_expression("*(5, 10) | range").unwrap();
		// Assert
		if let Expr::Pipe { left, .. } = &result.expr {
			assert!(matches!(left.expr, Expr::Expansion(_)));
		} else {
			panic!("expected Pipe");
		}
	}

	// Whitespace handling
	#[rstest]
	#[case("  false  |  fixed  |  TBD  ")]
	#[case("false|fixed|TBD")]
	fn test_parse_whitespace(#[case] input: &str) {
		// Act & Assert
		assert!(parse_expression(input).is_ok());
	}

	// Combinations
	#[rstest]
	#[case("regex([a-z]*)", "range(5, 10)", "random", "TBD")]
	#[case("regex([0-9]*)", "range(1, 1)", "sequential", "TBD")]
	fn test_parse_pipe_combinations(
		#[case] rule_builder: &str,
		#[case] modifier: &str,
		#[case] strategy: &str,
		#[case] terminal: &str,
	) {
		// Arrange
		let input = format!("{rule_builder} | {modifier} | {strategy} | {terminal}");
		// Act & Assert
		assert!(parse_expression(&input).is_ok());
	}

	// Error cases
	#[rstest]
	fn test_parse_empty_input_error() {
		// Act & Assert
		assert!(parse_expression("").is_err());
	}

	// Boundary
	#[rstest]
	#[case("range(0, 0)")]
	#[case("seed(0)")]
	fn test_parse_boundary(#[case] input: &str) {
		// Act & Assert
		assert!(parse_expression(input).is_ok());
	}
}

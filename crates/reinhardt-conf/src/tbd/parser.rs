//! Winnow-based parser for TBD DSL expressions.
//!
//! Converts source text into [`SpannedExpr`] AST nodes with byte-offset spans.

use winnow::ascii::{digit1, multispace0};
use winnow::combinator::{alt, opt, preceded};
use winnow::error::{ContextError, ErrMode, ModalResult};
use winnow::prelude::*;
use winnow::token::take_while;

use crate::tbd::ast::{Expr, Literal, NumberValue, SpannedExpr};
use crate::tbd::error::{Span, TbdError};

/// Helper to create a backtrack error.
fn backtrack() -> ErrMode<ContextError> {
	ErrMode::Backtrack(ContextError::new())
}

/// Parses a complete TBD DSL expression from the given input string.
///
/// Returns a [`SpannedExpr`] on success or a [`TbdError::ParseError`] if the
/// input cannot be parsed.
pub fn parse_expression(input: &str) -> Result<SpannedExpr, TbdError> {
	let mut remaining = input;
	parse_primary.parse_next(&mut remaining).map_err(|_| TbdError::ParseError {
		message: "failed to parse expression".into(),
		span: Span {
			start: 0,
			end: input.len(),
		},
	})
}

/// Skips leading whitespace, then dispatches to one of the literal or
/// identifier parsers.
fn parse_primary(input: &mut &str) -> ModalResult<SpannedExpr> {
	multispace0.parse_next(input)?;

	alt((
		parse_boolean,
		parse_number,
		parse_string_literal,
		parse_identifier,
	))
	.parse_next(input)
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
	Ok(SpannedExpr::new(
		Expr::Literal(lit),
		Span { start, end },
	))
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

/// Parses an identifier: starts with an alphabetic character, followed by
/// zero or more alphanumeric characters or underscores.
///
/// Keywords `true` and `false` are excluded (handled by the boolean parser).
fn parse_identifier(input: &mut &str) -> ModalResult<SpannedExpr> {
	let full = *input;
	let start = offset_in(full, input);

	// First character must be alphabetic
	let first: char =
		winnow::token::any.verify(|c: &char| c.is_alphabetic()).parse_next(input)?;

	// Remaining characters: alphanumeric or underscore
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

	let end = offset_in(full, input);
	Ok(SpannedExpr::new(
		Expr::Identifier(name),
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
}

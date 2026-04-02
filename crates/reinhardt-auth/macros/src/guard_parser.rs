//! winnow-based parser for guard expression syntax.
//!
//! Grammar:
//! ```text
//! GuardExpr := OrExpr
//! OrExpr    := AndExpr ('|' AndExpr)*
//! AndExpr   := UnaryExpr ('&' UnaryExpr)*
//! UnaryExpr := '!' UnaryExpr | Atom
//! Atom      := '(' OrExpr ')' | 'HasPerm' '(' STRING_LIT ')' | TypePath
//! TypePath  := Ident ('::' Ident)*
//! ```
//!
//! Operator precedence: `!` > `&` > `|`

use winnow::combinator::{alt, delimited, preceded, repeat};
use winnow::token::take_while;
use winnow::{ModalResult, Parser};

/// AST node for guard expressions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GuardExpr {
	/// A type path like `IsAdminUser` or `my_mod::MyPerm`.
	TypePath(Vec<String>),
	/// A string-based permission check: `HasPerm("blog.add")`.
	HasPerm(String),
	/// AND combinator: all sub-expressions must pass.
	And(Vec<GuardExpr>),
	/// OR combinator: at least one sub-expression must pass.
	Or(Vec<GuardExpr>),
	/// NOT combinator: inverts the sub-expression.
	Not(Box<GuardExpr>),
}

/// Parses the full guard expression from a token stream string.
pub(crate) fn parse_guard_expr(input: &str) -> Result<GuardExpr, String> {
	let trimmed = input.trim();
	if trimmed.is_empty() {
		return Err("empty guard expression".to_owned());
	}

	let mut stream = trimmed;
	match or_expr(&mut stream) {
		Ok(expr) => {
			let remaining = stream.trim();
			if remaining.is_empty() {
				Ok(expr)
			} else {
				Err(format!("unexpected trailing input: `{remaining}`"))
			}
		}
		Err(_) => Err(format!("failed to parse guard expression: `{trimmed}`")),
	}
}

// ── Parsers ──────────────────────────────────────────────────────────

fn ws<'i>(input: &mut &'i str) -> ModalResult<&'i str> {
	take_while(0.., |c: char| c.is_ascii_whitespace()).parse_next(input)
}

fn or_expr(input: &mut &str) -> ModalResult<GuardExpr> {
	let first = and_expr(input)?;
	let rest: Vec<GuardExpr> = repeat(0.., preceded((ws, '|', ws), and_expr)).parse_next(input)?;
	if rest.is_empty() {
		Ok(first)
	} else {
		let mut all = vec![first];
		all.extend(rest);
		Ok(GuardExpr::Or(all))
	}
}

fn and_expr(input: &mut &str) -> ModalResult<GuardExpr> {
	let first = unary_expr(input)?;
	let rest: Vec<GuardExpr> =
		repeat(0.., preceded((ws, '&', ws), unary_expr)).parse_next(input)?;
	if rest.is_empty() {
		Ok(first)
	} else {
		let mut all = vec![first];
		all.extend(rest);
		Ok(GuardExpr::And(all))
	}
}

fn unary_expr(input: &mut &str) -> ModalResult<GuardExpr> {
	alt((not_expr, atom)).parse_next(input)
}

fn not_expr(input: &mut &str) -> ModalResult<GuardExpr> {
	let _ = '!'.parse_next(input)?;
	ws(input)?;
	let inner = unary_expr(input)?;
	Ok(GuardExpr::Not(Box::new(inner)))
}

fn atom(input: &mut &str) -> ModalResult<GuardExpr> {
	ws(input)?;
	alt((paren_expr, has_perm, type_path)).parse_next(input)
}

fn paren_expr(input: &mut &str) -> ModalResult<GuardExpr> {
	delimited(('(', ws), or_expr, (ws, ')')).parse_next(input)
}

fn string_lit(input: &mut &str) -> ModalResult<String> {
	let _ = '"'.parse_next(input)?;
	// Proc-macro TokenStream stringifies string literals with escaped quotes,
	// so we just take until the next unescaped `"`
	let mut result = String::new();
	loop {
		let chunk: &str = take_while(0.., |c: char| c != '"' && c != '\\').parse_next(input)?;
		result.push_str(chunk);
		if input.starts_with('\\') {
			let _ = '\\'.parse_next(input)?;
			if let Some(c) = input.chars().next() {
				let _ = take_while(1, |ch: char| ch == c).parse_next(input)?;
				result.push(c);
			}
		} else {
			break;
		}
	}
	let _ = '"'.parse_next(input)?;
	Ok(result)
}

fn has_perm(input: &mut &str) -> ModalResult<GuardExpr> {
	let _ = "HasPerm".parse_next(input)?;
	ws(input)?;
	let _ = '('.parse_next(input)?;
	ws(input)?;
	let perm = string_lit(input)?;
	ws(input)?;
	let _ = ')'.parse_next(input)?;
	Ok(GuardExpr::HasPerm(perm))
}

fn ident(input: &mut &str) -> ModalResult<String> {
	// Rust identifier: starts with letter or underscore, followed by alphanumerics or underscores
	let first: &str =
		take_while(1, |c: char| c.is_ascii_alphabetic() || c == '_').parse_next(input)?;
	let rest: &str =
		take_while(0.., |c: char| c.is_ascii_alphanumeric() || c == '_').parse_next(input)?;
	let mut s = String::from(first);
	s.push_str(rest);
	Ok(s)
}

fn type_path(input: &mut &str) -> ModalResult<GuardExpr> {
	let first = ident(input)?;
	let rest: Vec<String> = repeat(0.., preceded("::", ident)).parse_next(input)?;
	let mut segments = vec![first];
	segments.extend(rest);
	Ok(GuardExpr::TypePath(segments))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parse_single_type() {
		// Arrange
		let input = "IsAdminUser";

		// Act
		let result = parse_guard_expr(input).unwrap();

		// Assert
		assert_eq!(result, GuardExpr::TypePath(vec!["IsAdminUser".to_owned()]));
	}

	#[test]
	fn parse_type_path_with_module() {
		// Arrange
		let input = "my_mod::MyPerm";

		// Act
		let result = parse_guard_expr(input).unwrap();

		// Assert
		assert_eq!(
			result,
			GuardExpr::TypePath(vec!["my_mod".to_owned(), "MyPerm".to_owned()])
		);
	}

	#[test]
	fn parse_and_expr() {
		// Arrange
		let input = "IsAdminUser & IsActiveUser";

		// Act
		let result = parse_guard_expr(input).unwrap();

		// Assert
		assert_eq!(
			result,
			GuardExpr::And(vec![
				GuardExpr::TypePath(vec!["IsAdminUser".to_owned()]),
				GuardExpr::TypePath(vec!["IsActiveUser".to_owned()]),
			])
		);
	}

	#[test]
	fn parse_or_expr() {
		// Arrange
		let input = "IsAdminUser | IsActiveUser";

		// Act
		let result = parse_guard_expr(input).unwrap();

		// Assert
		assert_eq!(
			result,
			GuardExpr::Or(vec![
				GuardExpr::TypePath(vec!["IsAdminUser".to_owned()]),
				GuardExpr::TypePath(vec!["IsActiveUser".to_owned()]),
			])
		);
	}

	#[test]
	fn parse_not_expr() {
		// Arrange
		let input = "! IsAdminUser";

		// Act
		let result = parse_guard_expr(input).unwrap();

		// Assert
		assert_eq!(
			result,
			GuardExpr::Not(Box::new(GuardExpr::TypePath(vec![
				"IsAdminUser".to_owned()
			])))
		);
	}

	#[test]
	fn parse_has_perm() {
		// Arrange
		let input = "HasPerm(\"blog.add\")";

		// Act
		let result = parse_guard_expr(input).unwrap();

		// Assert
		assert_eq!(result, GuardExpr::HasPerm("blog.add".to_owned()));
	}

	#[test]
	fn parse_parenthesized() {
		// Arrange
		let input = "(IsAdminUser | IsActiveUser) & IsAuthenticated";

		// Act
		let result = parse_guard_expr(input).unwrap();

		// Assert
		assert_eq!(
			result,
			GuardExpr::And(vec![
				GuardExpr::Or(vec![
					GuardExpr::TypePath(vec!["IsAdminUser".to_owned()]),
					GuardExpr::TypePath(vec!["IsActiveUser".to_owned()]),
				]),
				GuardExpr::TypePath(vec!["IsAuthenticated".to_owned()]),
			])
		);
	}

	#[test]
	fn parse_precedence_and_before_or() {
		// Arrange: A & B | C should parse as (A & B) | C
		let input = "A & B | C";

		// Act
		let result = parse_guard_expr(input).unwrap();

		// Assert
		assert_eq!(
			result,
			GuardExpr::Or(vec![
				GuardExpr::And(vec![
					GuardExpr::TypePath(vec!["A".to_owned()]),
					GuardExpr::TypePath(vec!["B".to_owned()]),
				]),
				GuardExpr::TypePath(vec!["C".to_owned()]),
			])
		);
	}

	#[test]
	fn parse_not_binds_tighter_than_and() {
		// Arrange: !A & B should parse as (!A) & B
		let input = "! A & B";

		// Act
		let result = parse_guard_expr(input).unwrap();

		// Assert
		assert_eq!(
			result,
			GuardExpr::And(vec![
				GuardExpr::Not(Box::new(GuardExpr::TypePath(vec!["A".to_owned()]))),
				GuardExpr::TypePath(vec!["B".to_owned()]),
			])
		);
	}

	#[test]
	fn parse_complex_expr() {
		// Arrange
		let input = "(IsAdminUser | IsActiveUser) & ! HasPerm(\"blog.delete\")";

		// Act
		let result = parse_guard_expr(input).unwrap();

		// Assert
		assert_eq!(
			result,
			GuardExpr::And(vec![
				GuardExpr::Or(vec![
					GuardExpr::TypePath(vec!["IsAdminUser".to_owned()]),
					GuardExpr::TypePath(vec!["IsActiveUser".to_owned()]),
				]),
				GuardExpr::Not(Box::new(GuardExpr::HasPerm("blog.delete".to_owned()))),
			])
		);
	}

	#[test]
	fn parse_triple_and() {
		// Arrange
		let input = "A & B & C";

		// Act
		let result = parse_guard_expr(input).unwrap();

		// Assert
		assert_eq!(
			result,
			GuardExpr::And(vec![
				GuardExpr::TypePath(vec!["A".to_owned()]),
				GuardExpr::TypePath(vec!["B".to_owned()]),
				GuardExpr::TypePath(vec!["C".to_owned()]),
			])
		);
	}

	#[test]
	fn parse_empty_input_fails() {
		// Arrange & Act
		let result = parse_guard_expr("");

		// Assert
		assert!(result.is_err());
	}

	#[test]
	fn parse_whitespace_only_fails() {
		// Arrange & Act
		let result = parse_guard_expr("   ");

		// Assert
		assert!(result.is_err());
	}

	#[test]
	fn parse_trailing_input_fails() {
		// Arrange & Act
		let result = parse_guard_expr("A B");

		// Assert
		assert!(result.is_err());
	}

	#[test]
	fn parse_double_not() {
		// Arrange
		let input = "!! A";

		// Act
		let result = parse_guard_expr(input).unwrap();

		// Assert
		assert_eq!(
			result,
			GuardExpr::Not(Box::new(GuardExpr::Not(Box::new(GuardExpr::TypePath(
				vec!["A".to_owned()]
			)))))
		);
	}

	#[test]
	fn parse_nested_parens() {
		// Arrange
		let input = "((A))";

		// Act
		let result = parse_guard_expr(input).unwrap();

		// Assert
		assert_eq!(result, GuardExpr::TypePath(vec!["A".to_owned()]));
	}

	#[test]
	fn parse_deep_path() {
		// Arrange
		let input = "crate::auth::perms::IsSpecial";

		// Act
		let result = parse_guard_expr(input).unwrap();

		// Assert
		assert_eq!(
			result,
			GuardExpr::TypePath(vec![
				"crate".to_owned(),
				"auth".to_owned(),
				"perms".to_owned(),
				"IsSpecial".to_owned(),
			])
		);
	}
}

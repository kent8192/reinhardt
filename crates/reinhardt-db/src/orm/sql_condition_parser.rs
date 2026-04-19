//! SQL condition parser for converting raw SQL WHERE clauses to Q objects
//!
//! This module provides a hand-written parser with regex support for common SQL patterns.

use crate::orm::expressions::Q;
use regex::Regex;
use std::sync::OnceLock;

#[cfg(test)]
use crate::orm::expressions::QOperator;

/// Compiled regex patterns for SQL parsing
struct Patterns {
	is_null: Regex,
	is_not_null: Regex,
	between: Regex,
	in_clause: Regex,
	like: Regex,
	comparison: Regex,
}

impl Patterns {
	fn new() -> Self {
		Self {
			// IS NULL: "field IS NULL"
			is_null: Regex::new(r"(?i)^\s*(\w+)\s+IS\s+NULL\s*$").unwrap(),
			// IS NOT NULL: "field IS NOT NULL"
			is_not_null: Regex::new(r"(?i)^\s*(\w+)\s+IS\s+NOT\s+NULL\s*$").unwrap(),
			// BETWEEN: "field BETWEEN val1 AND val2"
			between: Regex::new(r"(?i)^\s*(\w+)\s+BETWEEN\s+(.+?)\s+AND\s+(.+?)\s*$").unwrap(),
			// IN: "field IN (val1, val2, ...)"
			in_clause: Regex::new(r"(?i)^\s*(\w+)\s+IN\s*\((.+?)\)\s*$").unwrap(),
			// LIKE: "field LIKE 'pattern'"
			like: Regex::new(r"(?i)^\s*(\w+)\s+LIKE\s+(.+?)\s*$").unwrap(),
			// Comparison: "field op value" where op is =, !=, <, >, <=, >=, <>
			comparison: Regex::new(r"(?i)^\s*(\w+)\s*(=|!=|<>|<=|>=|<|>)\s*(.+?)\s*$").unwrap(),
		}
	}
}

static PATTERNS: OnceLock<Patterns> = OnceLock::new();

fn patterns() -> &'static Patterns {
	PATTERNS.get_or_init(Patterns::new)
}

/// SQL condition parser
pub struct SqlConditionParser;

impl SqlConditionParser {
	/// Parse a SQL condition string into a Q object
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::orm::sql_condition_parser::SqlConditionParser;
	///
	/// let q = SqlConditionParser::parse("age > 18");
	/// let q = SqlConditionParser::parse("name LIKE '%John%'");
	/// let q = SqlConditionParser::parse("email IS NOT NULL");
	/// let q = SqlConditionParser::parse("status IN ('active', 'pending')");
	/// let q = SqlConditionParser::parse("age BETWEEN 18 AND 65");
	/// let q = SqlConditionParser::parse("active = true AND verified = true");
	/// ```
	pub fn parse(sql: &str) -> Q {
		let trimmed = sql.trim();

		// Try compound conditions first (AND/OR)
		if let Some(q) = Self::try_parse_compound(trimmed) {
			return q;
		}

		// Try IS NULL
		if let Some(q) = Self::try_parse_is_null(trimmed) {
			return q;
		}

		// Try IS NOT NULL
		if let Some(q) = Self::try_parse_is_not_null(trimmed) {
			return q;
		}

		// Try BETWEEN
		if let Some(q) = Self::try_parse_between(trimmed) {
			return q;
		}

		// Try IN
		if let Some(q) = Self::try_parse_in(trimmed) {
			return q;
		}

		// Try LIKE
		if let Some(q) = Self::try_parse_like(trimmed) {
			return q;
		}

		// Try comparison
		if let Some(q) = Self::try_parse_comparison(trimmed) {
			return q;
		}

		// Fallback: store as raw SQL in value field
		Q::Condition {
			field: String::new(),
			operator: String::new(),
			value: trimmed.to_string(),
		}
	}

	/// Try to parse IS NULL pattern
	fn try_parse_is_null(sql: &str) -> Option<Q> {
		let caps = patterns().is_null.captures(sql)?;
		let field = caps.get(1)?.as_str().to_string();

		Some(Q::Condition {
			field,
			operator: "IS NULL".to_string(),
			value: String::new(),
		})
	}

	/// Try to parse IS NOT NULL pattern
	fn try_parse_is_not_null(sql: &str) -> Option<Q> {
		let caps = patterns().is_not_null.captures(sql)?;
		let field = caps.get(1)?.as_str().to_string();

		Some(Q::Condition {
			field,
			operator: "IS NOT NULL".to_string(),
			value: String::new(),
		})
	}

	/// Try to parse BETWEEN pattern
	fn try_parse_between(sql: &str) -> Option<Q> {
		let caps = patterns().between.captures(sql)?;
		let field = caps.get(1)?.as_str().to_string();
		let val1 = caps.get(2)?.as_str().trim();
		let val2 = caps.get(3)?.as_str().trim();

		// BETWEEN is equivalent to: field >= val1 AND field <= val2
		let q1 = Q::Condition {
			field: field.clone(),
			operator: ">=".to_string(),
			value: val1.to_string(),
		};
		let q2 = Q::Condition {
			field,
			operator: "<=".to_string(),
			value: val2.to_string(),
		};

		Some(q1.and(q2))
	}

	/// Try to parse IN pattern
	fn try_parse_in(sql: &str) -> Option<Q> {
		let caps = patterns().in_clause.captures(sql)?;
		let field = caps.get(1)?.as_str().to_string();
		let values_str = caps.get(2)?.as_str();

		// Parse comma-separated values
		let values: Vec<String> = values_str
			.split(',')
			.map(|v| v.trim().to_string())
			.collect();

		// IN is equivalent to: field = val1 OR field = val2 OR ...
		let conditions: Vec<Q> = values
			.into_iter()
			.map(|value| Q::Condition {
				field: field.clone(),
				operator: "=".to_string(),
				value,
			})
			.collect();

		if conditions.is_empty() {
			return None;
		}

		// Combine with OR
		Some(conditions.into_iter().reduce(|acc, q| acc.or(q)).unwrap())
	}

	/// Try to parse LIKE pattern
	fn try_parse_like(sql: &str) -> Option<Q> {
		let caps = patterns().like.captures(sql)?;
		let field = caps.get(1)?.as_str().to_string();
		let pattern = caps.get(2)?.as_str().trim().to_string();

		Some(Q::Condition {
			field,
			operator: "LIKE".to_string(),
			value: pattern,
		})
	}

	/// Try to parse comparison operators (=, !=, <, >, <=, >=, <>)
	fn try_parse_comparison(sql: &str) -> Option<Q> {
		let caps = patterns().comparison.captures(sql)?;
		let field = caps.get(1)?.as_str().to_string();
		let operator = caps.get(2)?.as_str().to_string();
		let value = caps.get(3)?.as_str().trim().to_string();

		Some(Q::Condition {
			field,
			operator,
			value,
		})
	}

	/// Try to parse compound conditions (AND/OR)
	fn try_parse_compound(sql: &str) -> Option<Q> {
		// Simple split-based parsing for AND/OR
		// Note: This is a simplified implementation that doesn't handle nested parentheses
		// For production use, a proper recursive descent parser would be better

		// Try to split by AND (case-insensitive)
		if let Some(and_pos) = Self::find_operator(sql, " AND ") {
			let left = sql[..and_pos].trim();
			let right = sql[and_pos + 5..].trim(); // " AND " is 5 chars
			let left_q = Self::parse(left);
			let right_q = Self::parse(right);
			return Some(left_q.and(right_q));
		}

		// Try to split by OR (case-insensitive)
		if let Some(or_pos) = Self::find_operator(sql, " OR ") {
			let left = sql[..or_pos].trim();
			let right = sql[or_pos + 4..].trim(); // " OR " is 4 chars
			let left_q = Self::parse(left);
			let right_q = Self::parse(right);
			return Some(left_q.or(right_q));
		}

		None
	}

	/// Find position of operator (case-insensitive, not inside quotes)
	fn find_operator(sql: &str, op: &str) -> Option<usize> {
		let upper_sql = sql.to_uppercase();
		let upper_op = op.to_uppercase();

		// Simple implementation: find first occurrence not inside quotes
		let mut in_quote = false;
		let mut quote_char = ' ';

		for (i, ch) in sql.chars().enumerate() {
			if ch == '\'' || ch == '"' {
				if in_quote && ch == quote_char {
					in_quote = false;
				} else if !in_quote {
					in_quote = true;
					quote_char = ch;
				}
			}

			if !in_quote
				&& i + upper_op.len() <= upper_sql.len()
				&& upper_sql[i..i + upper_op.len()] == *upper_op
			{
				return Some(i);
			}
		}

		None
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_parse_comparison() {
		let q = SqlConditionParser::parse("age > 18");
		match q {
			Q::Condition {
				field,
				operator,
				value,
			} => {
				assert_eq!(field, "age");
				assert_eq!(operator, ">");
				assert_eq!(value, "18");
			}
			_ => panic!("Expected Condition"),
		}
	}

	#[test]
	fn test_parse_is_null() {
		let q = SqlConditionParser::parse("email IS NULL");
		match q {
			Q::Condition {
				field,
				operator,
				value,
			} => {
				assert_eq!(field, "email");
				assert_eq!(operator, "IS NULL");
				assert_eq!(value, "");
			}
			_ => panic!("Expected Condition"),
		}
	}

	#[test]
	fn test_parse_is_not_null() {
		let q = SqlConditionParser::parse("email IS NOT NULL");
		match q {
			Q::Condition {
				field,
				operator,
				value,
			} => {
				assert_eq!(field, "email");
				assert_eq!(operator, "IS NOT NULL");
				assert_eq!(value, "");
			}
			_ => panic!("Expected Condition"),
		}
	}

	#[test]
	fn test_parse_like() {
		let q = SqlConditionParser::parse("name LIKE '%John%'");
		match q {
			Q::Condition {
				field,
				operator,
				value,
			} => {
				assert_eq!(field, "name");
				assert_eq!(operator, "LIKE");
				assert_eq!(value, "'%John%'");
			}
			_ => panic!("Expected Condition"),
		}
	}

	#[test]
	fn test_parse_between() {
		let q = SqlConditionParser::parse("age BETWEEN 18 AND 65");
		match q {
			Q::Combined {
				operator: QOperator::And,
				conditions,
			} => {
				assert_eq!(conditions.len(), 2);
			}
			_ => panic!("Expected Combined with AND"),
		}
	}

	#[test]
	fn test_parse_in() {
		let q = SqlConditionParser::parse("status IN ('active', 'pending')");
		match q {
			Q::Combined {
				operator: QOperator::Or,
				conditions,
			} => {
				assert_eq!(conditions.len(), 2);
			}
			_ => panic!("Expected Combined with OR"),
		}
	}

	#[test]
	fn test_parse_compound_and() {
		let q = SqlConditionParser::parse("active = true AND verified = true");
		match q {
			Q::Combined {
				operator: QOperator::And,
				conditions,
			} => {
				assert_eq!(conditions.len(), 2);
			}
			_ => panic!("Expected Combined with AND"),
		}
	}

	#[test]
	fn test_parse_compound_or() {
		let q = SqlConditionParser::parse("status = 'draft' OR status = 'pending'");
		match q {
			Q::Combined {
				operator: QOperator::Or,
				conditions,
			} => {
				assert_eq!(conditions.len(), 2);
			}
			_ => panic!("Expected Combined with OR"),
		}
	}
}

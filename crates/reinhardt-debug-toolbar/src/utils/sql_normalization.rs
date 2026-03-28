//! SQL query normalization for duplicate detection

use regex::Regex;
use std::sync::LazyLock;

/// Normalize SQL query for duplicate detection
///
/// This function normalizes SQL queries by:
/// - Converting to uppercase
/// - Replacing numeric literals with `?`
/// - Replacing string literals with `?`
/// - Normalizing whitespace
/// - Removing comments
///
/// # Examples
///
/// ```
/// use reinhardt_debug_toolbar::utils::sql_normalization::normalize_sql;
///
/// let sql1 = "SELECT * FROM users WHERE id = 123";
/// let sql2 = "SELECT * FROM users WHERE id = 456";
/// assert_eq!(normalize_sql(sql1), normalize_sql(sql2));
/// ```
pub fn normalize_sql(sql: &str) -> String {
	static NUMERIC_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b\d+\b").unwrap());
	static STRING_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"'([^'\\]|\\.)*'"#).unwrap());
	static COMMENT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)--.*$").unwrap());
	static WHITESPACE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());

	// Remove comments
	let sql = COMMENT_RE.replace_all(sql, "");

	// Replace string literals
	let sql = STRING_RE.replace_all(&sql, "?");

	// Replace numeric literals
	let sql = NUMERIC_RE.replace_all(&sql, "?");

	// Normalize whitespace
	let sql = WHITESPACE_RE.replace_all(&sql, " ");

	// Convert to uppercase and trim
	sql.to_uppercase().trim().to_string()
}

/// Detect N+1 query patterns
///
/// Returns true if the queries likely represent an N+1 pattern:
/// - Similar normalized queries with high frequency
/// - Queries executed in succession
pub fn detect_n_plus_one(queries: &[crate::context::SqlQuery]) -> Vec<String> {
	use std::collections::HashMap;

	let mut normalized_counts: HashMap<String, usize> = HashMap::new();
	let mut n_plus_one_patterns = Vec::new();

	// Count normalized queries
	for query in queries {
		let normalized = normalize_sql(&query.sql);
		*normalized_counts.entry(normalized.clone()).or_insert(0) += 1;

		// If a normalized query appears more than 3 times, it's likely N+1
		if normalized_counts[&normalized] > 3 && !n_plus_one_patterns.contains(&normalized) {
			n_plus_one_patterns.push(normalized);
		}
	}

	n_plus_one_patterns
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_normalize_sql_numbers() {
		let sql1 = "SELECT * FROM users WHERE id = 123";
		let sql2 = "SELECT * FROM users WHERE id = 456";
		assert_eq!(normalize_sql(sql1), normalize_sql(sql2));
		assert_eq!(normalize_sql(sql1), "SELECT * FROM USERS WHERE ID = ?");
	}

	#[test]
	fn test_normalize_sql_strings() {
		let sql1 = "SELECT * FROM users WHERE name = 'Alice'";
		let sql2 = "SELECT * FROM users WHERE name = 'Bob'";
		assert_eq!(normalize_sql(sql1), normalize_sql(sql2));
		assert_eq!(normalize_sql(sql1), "SELECT * FROM USERS WHERE NAME = ?");
	}

	#[test]
	fn test_normalize_sql_whitespace() {
		let sql1 = "SELECT   *   FROM\n  users\tWHERE  id = 1";
		let sql2 = "SELECT * FROM users WHERE id = 2";
		assert_eq!(normalize_sql(sql1), normalize_sql(sql2));
	}

	#[test]
	fn test_normalize_sql_comments() {
		let sql = "SELECT * FROM users -- this is a comment\nWHERE id = 1";
		let normalized = normalize_sql(sql);
		assert!(!normalized.contains("COMMENT"));
		assert_eq!(normalized, "SELECT * FROM USERS WHERE ID = ?");
	}

	#[test]
	fn test_detect_n_plus_one() {
		use crate::context::SqlQuery;
		use chrono::Utc;
		use std::time::Duration;

		let queries = vec![
			SqlQuery {
				sql: "SELECT * FROM users WHERE id = 1".to_string(),
				params: vec![],
				duration: Duration::from_millis(10),
				stack_trace: String::new(),
				timestamp: Utc::now(),
				connection: None,
			},
			SqlQuery {
				sql: "SELECT * FROM posts WHERE user_id = 1".to_string(),
				params: vec![],
				duration: Duration::from_millis(5),
				stack_trace: String::new(),
				timestamp: Utc::now(),
				connection: None,
			},
			SqlQuery {
				sql: "SELECT * FROM posts WHERE user_id = 2".to_string(),
				params: vec![],
				duration: Duration::from_millis(5),
				stack_trace: String::new(),
				timestamp: Utc::now(),
				connection: None,
			},
			SqlQuery {
				sql: "SELECT * FROM posts WHERE user_id = 3".to_string(),
				params: vec![],
				duration: Duration::from_millis(5),
				stack_trace: String::new(),
				timestamp: Utc::now(),
				connection: None,
			},
			SqlQuery {
				sql: "SELECT * FROM posts WHERE user_id = 4".to_string(),
				params: vec![],
				duration: Duration::from_millis(5),
				stack_trace: String::new(),
				timestamp: Utc::now(),
				connection: None,
			},
			SqlQuery {
				sql: "SELECT * FROM posts WHERE user_id = 5".to_string(),
				params: vec![],
				duration: Duration::from_millis(5),
				stack_trace: String::new(),
				timestamp: Utc::now(),
				connection: None,
			},
		];

		let patterns = detect_n_plus_one(&queries);
		assert_eq!(patterns.len(), 1);
		assert!(patterns[0].contains("SELECT * FROM POSTS WHERE USER_ID = ?"));
	}

	#[test]
	fn test_detect_n_plus_one_no_pattern() {
		use crate::context::SqlQuery;
		use chrono::Utc;
		use std::time::Duration;

		let queries = vec![
			SqlQuery {
				sql: "SELECT * FROM users WHERE id = 1".to_string(),
				params: vec![],
				duration: Duration::from_millis(10),
				stack_trace: String::new(),
				timestamp: Utc::now(),
				connection: None,
			},
			SqlQuery {
				sql: "SELECT * FROM posts WHERE id = 1".to_string(),
				params: vec![],
				duration: Duration::from_millis(5),
				stack_trace: String::new(),
				timestamp: Utc::now(),
				connection: None,
			},
		];

		let patterns = detect_n_plus_one(&queries);
		assert_eq!(patterns.len(), 0);
	}
}

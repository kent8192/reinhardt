//! Query statement traits
//!
//! This module defines the core traits for building and executing SQL queries.

use std::{any::Any, fmt::Debug};

use crate::value::Values;

/// Replace parameter placeholders in SQL with inline value literals.
///
/// Supports both numbered (`$1, $2, ...`) and positional (`?`) placeholders.
/// This enables `to_string()` to produce complete SQL with values inlined,
/// matching reinhardt-query behavior for debugging and non-parameterized execution.
pub fn inline_params(sql: &str, values: &Values) -> String {
	if values.is_empty() {
		return sql.to_string();
	}

	let vals = values.iter().collect::<Vec<_>>();

	// Detect placeholder style: if SQL contains `$1`, it's numbered (PostgreSQL)
	if sql.contains("$1") {
		// Replace from highest index down to avoid `$1` matching inside `$10`
		let mut result = sql.to_string();
		for i in (0..vals.len()).rev() {
			let placeholder = format!("${}", i + 1);
			result = result.replacen(&placeholder, &vals[i].to_sql_literal(), 1);
		}
		result
	} else {
		// Positional `?` placeholders (MySQL/SQLite)
		let mut result = String::with_capacity(sql.len());
		let mut val_idx = 0;
		let chars = sql.chars().peekable();
		let mut in_single_quote = false;

		for ch in chars {
			// Track single-quoted strings to avoid replacing `?` inside them
			if ch == '\'' {
				in_single_quote = !in_single_quote;
				result.push(ch);
			} else if ch == '?' && !in_single_quote && val_idx < vals.len() {
				result.push_str(&vals[val_idx].to_sql_literal());
				val_idx += 1;
			} else {
				result.push(ch);
			}
		}
		result
	}
}

/// Trait for building query statements
///
/// This trait provides methods to build SQL statements for different database backends
/// and collect query parameters.
pub trait QueryStatementBuilder: Debug {
	/// Build SQL statement for a database backend and collect query parameters
	///
	/// This is the primary method for generating parameterized SQL queries.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::select()
	///     .column(Expr::col("name"))
	///     .from("users");
	///
	/// // Build for PostgreSQL
	/// let (sql, values) = query.build(PostgresQueryBuilder);
	/// // sql = "SELECT \"name\" FROM \"users\""
	/// // values = Values(vec![])
	/// ```
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, Values);

	/// Build SQL statement for a database backend and return SQL string
	/// with values inlined as SQL literals.
	///
	/// This produces a complete SQL string with parameter values embedded
	/// directly, suitable for debugging, inspection, or execution against
	/// databases that do not support parameterized queries.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::select()
	///     .column(Expr::col("name"))
	///     .from("users")
	///     .and_where(Expr::col("active").eq(true));
	///
	/// let sql = query.to_string(MysqlQueryBuilder);
	/// // sql = "SELECT `name` FROM `users` WHERE `active` = TRUE"
	/// ```
	fn to_string<T: QueryBuilderTrait>(&self, query_builder: T) -> String {
		let (sql, values) = self.build(query_builder);
		inline_params(&sql, &values)
	}

	/// Build SQL statement with parameter collection
	///
	/// This is a convenience method that wraps `build_any()` with a concrete
	/// query builder type.
	fn build<T: QueryBuilderTrait>(&self, query_builder: T) -> (String, Values) {
		self.build_any(&query_builder)
	}
}

/// Trait for query statement writers
///
/// This trait extends [`QueryStatementBuilder`] with additional methods for
/// writing SQL statements.
pub trait QueryStatementWriter: QueryStatementBuilder {}

/// Placeholder trait for query builders (will be implemented in Phase 5)
///
/// This trait defines the interface for database-specific query builders
/// that generate SQL syntax for different backends (PostgreSQL, MySQL, SQLite).
pub trait QueryBuilderTrait: Debug + Any {
	/// Get placeholder format for this backend
	///
	/// Returns a tuple of (placeholder_format, is_numbered):
	/// - PostgreSQL: ("$", true) -> $1, $2, $3...
	/// - MySQL: ("?", false) -> ?, ?, ?...
	/// - SQLite: ("?", false) -> ?, ?, ?...
	fn placeholder(&self) -> (&str, bool);

	/// Get quote character for this backend
	///
	/// - PostgreSQL: " (double quote)
	/// - MySQL: ` (backtick)
	/// - SQLite: " (double quote)
	fn quote_char(&self) -> char;
}

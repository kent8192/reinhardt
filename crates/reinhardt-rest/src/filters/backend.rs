//! Custom filter backend system
//!
//! Provides a pluggable filter backend architecture for specialized filtering needs.
//!
//! # Examples
//!
//! ```
//! use reinhardt_rest::filters::{FilterBackend, FilterResult, CustomFilterBackend};
//! use std::collections::HashMap;
//! use async_trait::async_trait;
//!
//! // Define a custom filter backend
//! struct MyCustomFilter;
//!
//! #[async_trait]
//! impl FilterBackend for MyCustomFilter {
//!     async fn filter_queryset(
//!         &self,
//!         query_params: &HashMap<String, String>,
//!         sql: String,
//!     ) -> FilterResult<String> {
//!         // Custom filtering logic
//!         Ok(sql)
//!     }
//! }
//!
//! # async fn example() {
//! // Use the custom filter
//! let filter = MyCustomFilter;
//! let mut backend = CustomFilterBackend::new();
//! backend.add_filter(Box::new(filter));
//! // Verify the filter is added successfully
//! assert_eq!(backend.filter_count(), 1);
//! # }
//! ```

use super::{DatabaseDialect, FilterBackend, FilterError, FilterResult};
use async_trait::async_trait;
use reinhardt_query::SimpleExpr;
use reinhardt_query::prelude::{
	Alias, Cond, Expr, ExprTrait, MySqlQueryBuilder, Order, PostgresQueryBuilder, Query,
	QueryStatementBuilder,
};
use std::collections::HashMap;
use std::sync::Arc;

/// Find a SQL keyword in `sql` using ASCII case-insensitive matching followed by whitespace.
///
/// Returns the byte position of the keyword start if found.
/// Uses byte-level ASCII comparison to avoid allocation and Unicode case-folding issues
/// (e.g., "ß" uppercasing to "SS" which would shift byte indices).
///
/// NOTE: This naive scan can match keywords inside string literals, identifiers,
/// comments, or subqueries. A full SQL parser is out of scope for this filter layer;
/// callers should be aware of this limitation.
fn find_sql_keyword(sql: &str, keyword: &str) -> Option<usize> {
	let sql_bytes = sql.as_bytes();
	let kw_bytes = keyword.as_bytes();
	let kw_len = kw_bytes.len();

	if sql_bytes.len() < kw_len {
		return None;
	}

	for i in 0..=(sql_bytes.len() - kw_len) {
		// Check that the keyword matches (ASCII case-insensitive)
		let matched = sql_bytes[i..i + kw_len]
			.iter()
			.zip(kw_bytes.iter())
			.all(|(s, k)| s.eq_ignore_ascii_case(k));

		if !matched {
			continue;
		}

		// Keyword must be followed by whitespace or end of string
		let after_ok = if i + kw_len >= sql_bytes.len() {
			true
		} else {
			sql_bytes[i + kw_len].is_ascii_whitespace()
		};

		// Keyword must be preceded by whitespace, start of string, or ')'
		let before_ok = if i == 0 {
			true
		} else {
			let prev = sql_bytes[i - 1];
			prev.is_ascii_whitespace() || prev == b')'
		};

		if after_ok && before_ok {
			return Some(i);
		}
	}

	None
}

/// Find the end position of a SQL clause by locating the next top-level keyword.
///
/// Searches for each keyword in `end_keywords` after `start_pos` in `sql`,
/// returning the earliest match position. If no keyword is found, returns `sql.len()`.
///
/// NOTE: This naive scan can match keywords inside string literals, identifiers,
/// comments, or subqueries. A full SQL parser is out of scope for this filter layer.
fn find_clause_end(sql: &str, start_pos: usize, end_keywords: &[&str]) -> usize {
	end_keywords
		.iter()
		.filter_map(|kw| find_sql_keyword(&sql[start_pos..], kw).map(|pos| start_pos + pos))
		.min()
		.unwrap_or(sql.len())
}

/// A composable filter backend that chains multiple filters
///
/// # Examples
///
/// ```
/// use reinhardt_rest::filters::{FilterBackend, CustomFilterBackend};
/// use std::collections::HashMap;
/// use async_trait::async_trait;
///
/// struct SearchFilter;
/// struct OrderingFilter;
///
/// #[async_trait]
/// impl FilterBackend for SearchFilter {
///     async fn filter_queryset(
///         &self,
///         query_params: &HashMap<String, String>,
///         sql: String,
///     ) -> reinhardt_rest::filters::FilterResult<String> {
///         Ok(sql)
///     }
/// }
///
/// #[async_trait]
/// impl FilterBackend for OrderingFilter {
///     async fn filter_queryset(
///         &self,
///         query_params: &HashMap<String, String>,
///         sql: String,
///     ) -> reinhardt_rest::filters::FilterResult<String> {
///         Ok(sql)
///     }
/// }
///
/// # async fn example() {
/// let mut backend = CustomFilterBackend::new();
/// backend.add_filter(Box::new(SearchFilter));
/// backend.add_filter(Box::new(OrderingFilter));
///
/// let params = HashMap::new();
/// let result = backend.filter_queryset(&params, "SELECT * FROM users".to_string()).await;
/// // Verify the filter chain processes the query successfully
/// assert!(result.is_ok());
/// # }
/// ```
#[derive(Default)]
pub struct CustomFilterBackend {
	filters: Vec<Arc<dyn FilterBackend>>,
}

impl CustomFilterBackend {
	/// Create a new custom filter backend
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::CustomFilterBackend;
	///
	/// let backend = CustomFilterBackend::new();
	/// // Verify backend is created with no filters
	/// assert_eq!(backend.filter_count(), 0);
	/// ```
	pub fn new() -> Self {
		Self {
			filters: Vec::new(),
		}
	}

	/// Add a filter to the backend chain
	///
	/// Filters are applied in the order they are added.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::{FilterBackend, CustomFilterBackend};
	/// use std::collections::HashMap;
	/// use async_trait::async_trait;
	///
	/// struct MyFilter;
	///
	/// #[async_trait]
	/// impl FilterBackend for MyFilter {
	///     async fn filter_queryset(
	///         &self,
	///         query_params: &HashMap<String, String>,
	///         sql: String,
	///     ) -> reinhardt_rest::filters::FilterResult<String> {
	///         Ok(sql)
	///     }
	/// }
	///
	/// let mut backend = CustomFilterBackend::new();
	/// backend.add_filter(Box::new(MyFilter));
	/// // Verify the filter is added successfully
	/// assert_eq!(backend.filter_count(), 1);
	/// ```
	pub fn add_filter(&mut self, filter: Box<dyn FilterBackend>) {
		self.filters.push(Arc::from(filter));
	}

	/// Get the number of filters in the chain
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::{FilterBackend, CustomFilterBackend};
	/// use std::collections::HashMap;
	/// use async_trait::async_trait;
	///
	/// struct MyFilter;
	///
	/// #[async_trait]
	/// impl FilterBackend for MyFilter {
	///     async fn filter_queryset(
	///         &self,
	///         query_params: &HashMap<String, String>,
	///         sql: String,
	///     ) -> reinhardt_rest::filters::FilterResult<String> {
	///         Ok(sql)
	///     }
	/// }
	///
	/// let mut backend = CustomFilterBackend::new();
	/// // Verify initial count is 0
	/// assert_eq!(backend.filter_count(), 0);
	/// backend.add_filter(Box::new(MyFilter));
	/// // Verify count increases to 1
	/// assert_eq!(backend.filter_count(), 1);
	/// ```
	pub fn filter_count(&self) -> usize {
		self.filters.len()
	}
}

#[async_trait]
impl FilterBackend for CustomFilterBackend {
	async fn filter_queryset(
		&self,
		query_params: &HashMap<String, String>,
		mut sql: String,
	) -> FilterResult<String> {
		for filter in &self.filters {
			sql = filter.filter_queryset(query_params, sql).await?;
		}
		Ok(sql)
	}
}

/// A simple search filter backend implementation
///
/// # Examples
///
/// ```
/// use reinhardt_rest::filters::{FilterBackend, SimpleSearchBackend};
/// use std::collections::HashMap;
///
/// # async fn example() {
/// let backend = SimpleSearchBackend::new("search")
///     .with_field("title");
/// let mut params = HashMap::new();
/// params.insert("search".to_string(), "rust".to_string());
///
/// let sql = "SELECT * FROM articles".to_string();
/// let result = backend.filter_queryset(&params, sql).await.unwrap();
/// // Verify WHERE clause is added
/// assert!(result.contains("WHERE"));
/// # }
/// ```
pub struct SimpleSearchBackend {
	param_name: String,
	fields: Vec<String>,
	dialect: DatabaseDialect,
}

impl SimpleSearchBackend {
	/// Create a new simple search backend
	///
	/// # Arguments
	///
	/// * `param_name` - The query parameter name to look for (e.g., "search", "q")
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::SimpleSearchBackend;
	///
	/// let backend = SimpleSearchBackend::new("search");
	/// // Verify backend is created with the correct parameter name
	/// let _: SimpleSearchBackend = backend;
	/// ```
	pub fn new(param_name: impl Into<String>) -> Self {
		Self {
			param_name: param_name.into(),
			fields: Vec::new(),
			dialect: DatabaseDialect::default(),
		}
	}

	/// Add a field to search in
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::SimpleSearchBackend;
	///
	/// let backend = SimpleSearchBackend::new("search")
	///     .with_field("title")
	///     .with_field("content");
	/// // Verify fields are added successfully
	/// let _: SimpleSearchBackend = backend;
	/// ```
	pub fn with_field(mut self, field: impl Into<String>) -> Self {
		self.fields.push(field.into());
		self
	}

	/// Set the database dialect for query generation
	///
	/// Different databases use different identifier quoting styles.
	/// MySQL uses backticks (`column`) while PostgreSQL uses double quotes ("column").
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::{SimpleSearchBackend, DatabaseDialect};
	///
	/// let backend = SimpleSearchBackend::new("search")
	///     .with_field("title")
	///     .with_dialect(DatabaseDialect::PostgreSQL);
	/// // Verify backend is configured for PostgreSQL
	/// let _: SimpleSearchBackend = backend;
	/// ```
	pub fn with_dialect(mut self, dialect: DatabaseDialect) -> Self {
		self.dialect = dialect;
		self
	}

	/// Escape special characters in LIKE patterns to prevent SQL injection
	///
	/// Escapes `%`, `_`, and `\` which have special meanings in SQL LIKE patterns.
	fn escape_like_pattern(pattern: &str) -> String {
		pattern
			.replace('\\', "\\\\")
			.replace('%', "\\%")
			.replace('_', "\\_")
	}

	/// Build the search condition using reinhardt-query
	///
	/// Returns a vector of SimpleExpr for the search condition.
	/// Uses SeaQuery's parameterized query building to prevent SQL injection.
	fn build_search_conditions(&self, search_query: &str) -> Vec<SimpleExpr> {
		let escaped = Self::escape_like_pattern(search_query);

		self.fields
			.iter()
			.map(|field| {
				// Use SeaQuery's .contains() which properly parameterizes the value
				// This prevents SQL injection by using parameterized queries
				Expr::col(Alias::new(field)).contains(escaped.as_str())
			})
			.collect()
	}
}

#[async_trait]
impl FilterBackend for SimpleSearchBackend {
	async fn filter_queryset(
		&self,
		query_params: &HashMap<String, String>,
		sql: String,
	) -> FilterResult<String> {
		if let Some(search_query) = query_params.get(&self.param_name) {
			if self.fields.is_empty() {
				return Err(FilterError::InvalidParameter(
					"No search fields configured".to_string(),
				));
			}

			// Build search conditions using SeaQuery's parameterized queries
			let conditions = self.build_search_conditions(search_query);

			// Combine all conditions with OR logic
			let mut condition = Cond::any();
			for cond in conditions {
				condition = condition.add(cond);
			}

			// Build a minimal SELECT query to extract the WHERE clause
			// SeaQuery properly escapes values when generating SQL
			// Use the appropriate QueryBuilder based on dialect
			let query = match self.dialect {
				DatabaseDialect::MySQL => Query::select()
					.expr(Expr::val(1))
					.cond_where(condition)
					.to_string(MySqlQueryBuilder),
				DatabaseDialect::PostgreSQL => Query::select()
					.expr(Expr::val(1))
					.cond_where(condition)
					.to_string(PostgresQueryBuilder),
			};

			// Extract just the WHERE condition portion (after "WHERE ")
			let condition_str = if let Some(idx) = query.find("WHERE ") {
				query[idx + 6..].to_string()
			} else {
				String::new()
			};

			let where_clause = format!("WHERE ({})", condition_str);

			// Append search condition using proper SQL composition
			// instead of string replacement which can corrupt complex WHERE clauses.
			// Uses ASCII case-insensitive keyword scanning to avoid allocation and
			// Unicode case-folding byte-length divergence from to_uppercase().
			if let Some(where_pos) = find_sql_keyword(&sql, "WHERE") {
				// Skip past "WHERE" keyword and any trailing whitespace
				let after_keyword = where_pos + "WHERE".len();
				let content_start = sql[after_keyword..]
					.bytes()
					.position(|b| !b.is_ascii_whitespace())
					.map(|p| after_keyword + p)
					.unwrap_or(after_keyword);

				// Find the end of the existing WHERE clause by locating
				// the next top-level SQL keyword (GROUP BY, ORDER BY, LIMIT, etc.)
				let clause_end_keywords = ["GROUP BY", "ORDER BY", "LIMIT", "OFFSET", "HAVING"];
				let end_pos = find_clause_end(&sql, content_start, &clause_end_keywords);

				let existing_where = sql[content_start..end_pos].trim();
				let remainder = &sql[end_pos..];
				let prefix = &sql[..where_pos];

				Ok(format!(
					"{}WHERE ({}) AND ({}) {}",
					prefix,
					existing_where,
					condition_str,
					remainder.trim()
				))
			} else {
				Ok(format!("{} {}", sql, where_clause))
			}
		} else {
			Ok(sql)
		}
	}
}

/// A simple ordering filter backend implementation
///
/// # Examples
///
/// ```
/// use reinhardt_rest::filters::{FilterBackend, SimpleOrderingBackend};
/// use std::collections::HashMap;
///
/// # async fn example() {
/// let backend = SimpleOrderingBackend::new("ordering")
///     .allow_field("created_at")
///     .allow_field("title");
///
/// let mut params = HashMap::new();
/// params.insert("ordering".to_string(), "-created_at".to_string());
///
/// let sql = "SELECT * FROM articles".to_string();
/// let result = backend.filter_queryset(&params, sql).await.unwrap();
/// // Verify ORDER BY clause is added with DESC direction
/// assert!(result.contains("ORDER BY created_at DESC"));
/// # }
/// ```
pub struct SimpleOrderingBackend {
	param_name: String,
	allowed_fields: Vec<String>,
}

impl SimpleOrderingBackend {
	/// Create a new simple ordering backend
	///
	/// # Arguments
	///
	/// * `param_name` - The query parameter name to look for (e.g., "ordering", "sort")
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::SimpleOrderingBackend;
	///
	/// let backend = SimpleOrderingBackend::new("ordering");
	/// // Verify backend is created with the correct parameter name
	/// let _: SimpleOrderingBackend = backend;
	/// ```
	pub fn new(param_name: impl Into<String>) -> Self {
		Self {
			param_name: param_name.into(),
			allowed_fields: Vec::new(),
		}
	}

	/// Add an allowed field for ordering
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::SimpleOrderingBackend;
	///
	/// let backend = SimpleOrderingBackend::new("ordering")
	///     .allow_field("created_at")
	///     .allow_field("title");
	/// // Verify fields are allowed successfully
	/// let _: SimpleOrderingBackend = backend;
	/// ```
	pub fn allow_field(mut self, field: impl Into<String>) -> Self {
		self.allowed_fields.push(field.into());
		self
	}

	/// Build the ORDER BY clause
	///
	/// Returns the ORDER BY clause string (without the "ORDER BY" keywords).
	/// Generates database-agnostic SQL without identifier quoting.
	fn build_order_clause(&self, field: &str, order: Order) -> String {
		let order_str = match order {
			Order::Asc => "ASC",
			Order::Desc => "DESC",
		};
		format!("{} {}", field, order_str)
	}
}

#[async_trait]
impl FilterBackend for SimpleOrderingBackend {
	async fn filter_queryset(
		&self,
		query_params: &HashMap<String, String>,
		sql: String,
	) -> FilterResult<String> {
		if let Some(ordering) = query_params.get(&self.param_name) {
			// Parse the ordering parameter: "-field" for DESC, "field" for ASC
			let (field, order) = if let Some(field_name) = ordering.strip_prefix('-') {
				(field_name, Order::Desc)
			} else {
				(ordering.as_str(), Order::Asc)
			};

			if !self.allowed_fields.contains(&field.to_string()) {
				return Err(FilterError::InvalidParameter(format!(
					"Field '{}' is not allowed for ordering",
					field
				)));
			}

			// Use reinhardt-query to build type-safe ORDER BY clause
			let order_expr = self.build_order_clause(field, order);
			let order_clause = format!("ORDER BY {}", order_expr);

			// Append new ordering criteria to existing ORDER BY clause
			// instead of replacing it, which would destroy previous orderings.
			// Uses ASCII case-insensitive keyword scanning to avoid allocation and
			// Unicode case-folding byte-length divergence from to_uppercase().
			if let Some(order_pos) = find_sql_keyword(&sql, "ORDER BY") {
				// Skip past "ORDER BY" keyword and any trailing whitespace
				let after_keyword = order_pos + "ORDER BY".len();
				let content_start = sql[after_keyword..]
					.bytes()
					.position(|b| !b.is_ascii_whitespace())
					.map(|p| after_keyword + p)
					.unwrap_or(after_keyword);

				// Find end of existing ORDER BY clause (next top-level keyword)
				let clause_end_keywords = ["LIMIT", "OFFSET"];
				let end_pos = find_clause_end(&sql, content_start, &clause_end_keywords);

				let existing_order = sql[content_start..end_pos].trim_end();
				let remainder = &sql[end_pos..];
				let prefix = &sql[..order_pos];

				Ok(format!(
					"{}ORDER BY {}, {} {}",
					prefix,
					existing_order,
					order_expr,
					remainder.trim()
				)
				.trim_end()
				.to_string())
			} else {
				Ok(format!("{} {}", sql, order_clause))
			}
		} else {
			Ok(sql)
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	#[tokio::test]
	async fn test_custom_filter_backend_empty() {
		// Arrange
		let backend = CustomFilterBackend::new();
		let params = HashMap::new();
		let sql = "SELECT * FROM users".to_string();

		// Act
		let count = backend.filter_count();
		let result = backend.filter_queryset(&params, sql.clone()).await.unwrap();

		// Assert
		assert_eq!(count, 0);
		assert_eq!(result, sql);
	}

	#[rstest]
	#[tokio::test]
	async fn test_custom_filter_backend_with_filters() {
		// Arrange
		let mut backend = CustomFilterBackend::new();
		backend.add_filter(Box::new(
			SimpleSearchBackend::new("search").with_field("name"),
		));
		let mut params = HashMap::new();
		params.insert("search".to_string(), "john".to_string());
		let sql = "SELECT * FROM users".to_string();

		// Act
		let count = backend.filter_count();
		let result = backend.filter_queryset(&params, sql).await.unwrap();

		// Assert
		assert_eq!(count, 1);
		assert!(result.contains("WHERE"));
		assert!(result.contains("`name` LIKE '%john%'"));
	}

	#[rstest]
	#[tokio::test]
	async fn test_simple_search_backend() {
		// Arrange
		let backend = SimpleSearchBackend::new("search")
			.with_field("title")
			.with_field("content");
		let mut params = HashMap::new();
		params.insert("search".to_string(), "rust".to_string());
		let sql = "SELECT * FROM articles".to_string();

		// Act
		let result = backend.filter_queryset(&params, sql).await.unwrap();

		// Assert
		assert!(result.contains("WHERE"));
		assert!(result.contains("`title` LIKE '%rust%'"));
		assert!(result.contains("`content` LIKE '%rust%'"));
		assert!(result.contains("OR"));
	}

	#[rstest]
	#[tokio::test]
	async fn test_simple_search_backend_no_query() {
		// Arrange
		let backend = SimpleSearchBackend::new("search").with_field("title");
		let params = HashMap::new();
		let sql = "SELECT * FROM articles".to_string();

		// Act
		let result = backend.filter_queryset(&params, sql.clone()).await.unwrap();

		// Assert
		assert_eq!(result, sql);
	}

	#[rstest]
	#[tokio::test]
	async fn test_simple_search_backend_no_fields() {
		// Arrange
		let backend = SimpleSearchBackend::new("search");
		let mut params = HashMap::new();
		params.insert("search".to_string(), "rust".to_string());
		let sql = "SELECT * FROM articles".to_string();

		// Act
		let result = backend.filter_queryset(&params, sql).await;

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	#[tokio::test]
	async fn test_simple_search_backend_postgres() {
		// Arrange
		let backend = SimpleSearchBackend::new("search")
			.with_field("title")
			.with_dialect(DatabaseDialect::PostgreSQL);
		let mut params = HashMap::new();
		params.insert("search".to_string(), "rust".to_string());
		let sql = "SELECT * FROM articles".to_string();

		// Act
		let result = backend.filter_queryset(&params, sql).await.unwrap();

		// Assert
		assert!(result.contains("WHERE"));
		assert!(result.contains("\"title\" LIKE '%rust%'"));
	}

	#[rstest]
	#[case("' OR '1'='1")]
	#[case("'; DROP TABLE articles; --")]
	#[case("' UNION SELECT * FROM users --")]
	#[tokio::test]
	async fn test_simple_search_backend_sql_injection_prevention(#[case] payload: &str) {
		// Arrange
		let backend = SimpleSearchBackend::new("search").with_field("title");
		let mut params = HashMap::new();
		params.insert("search".to_string(), payload.to_string());
		let sql = "SELECT * FROM articles".to_string();

		// Act
		let result = backend.filter_queryset(&params, sql).await.unwrap();

		// Assert
		assert!(
			result.contains("LIKE"),
			"Result should contain LIKE clause for payload: {}",
			payload
		);
		// Single quotes must be balanced (even count) to prevent SQL injection
		let single_quote_count = result.matches('\'').count();
		assert!(
			single_quote_count % 2 == 0,
			"SQL injection vulnerability: unbalanced single quotes in result for payload: {}. Result: {}",
			payload,
			result
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_simple_ordering_backend_asc() {
		// Arrange
		let backend = SimpleOrderingBackend::new("ordering")
			.allow_field("created_at")
			.allow_field("title");
		let mut params = HashMap::new();
		params.insert("ordering".to_string(), "created_at".to_string());
		let sql = "SELECT * FROM articles".to_string();

		// Act
		let result = backend.filter_queryset(&params, sql).await.unwrap();

		// Assert
		assert!(result.contains("ORDER BY created_at ASC"));
	}

	#[rstest]
	#[tokio::test]
	async fn test_simple_ordering_backend_desc() {
		// Arrange
		let backend = SimpleOrderingBackend::new("ordering")
			.allow_field("created_at")
			.allow_field("title");
		let mut params = HashMap::new();
		params.insert("ordering".to_string(), "-created_at".to_string());
		let sql = "SELECT * FROM articles".to_string();

		// Act
		let result = backend.filter_queryset(&params, sql).await.unwrap();

		// Assert
		assert!(result.contains("ORDER BY created_at DESC"));
	}

	#[rstest]
	#[tokio::test]
	async fn test_simple_ordering_backend_invalid_field() {
		// Arrange
		let backend = SimpleOrderingBackend::new("ordering").allow_field("created_at");
		let mut params = HashMap::new();
		params.insert("ordering".to_string(), "invalid_field".to_string());
		let sql = "SELECT * FROM articles".to_string();

		// Act
		let result = backend.filter_queryset(&params, sql).await;

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	#[tokio::test]
	async fn test_simple_ordering_backend_no_query() {
		// Arrange
		let backend = SimpleOrderingBackend::new("ordering").allow_field("created_at");
		let params = HashMap::new();
		let sql = "SELECT * FROM articles".to_string();

		// Act
		let result = backend.filter_queryset(&params, sql.clone()).await.unwrap();

		// Assert
		assert_eq!(result, sql);
	}

	#[rstest]
	#[tokio::test]
	async fn test_chained_filters() {
		// Arrange
		let mut backend = CustomFilterBackend::new();
		backend.add_filter(Box::new(
			SimpleSearchBackend::new("search").with_field("title"),
		));
		backend.add_filter(Box::new(
			SimpleOrderingBackend::new("ordering").allow_field("created_at"),
		));
		let mut params = HashMap::new();
		params.insert("search".to_string(), "rust".to_string());
		params.insert("ordering".to_string(), "-created_at".to_string());
		let sql = "SELECT * FROM articles".to_string();

		// Act
		let result = backend.filter_queryset(&params, sql).await.unwrap();

		// Assert
		assert!(result.contains("WHERE"));
		assert!(result.contains("`title` LIKE '%rust%'"));
		assert!(result.contains("ORDER BY created_at DESC"));
	}

	// Regression test for #2685: SimpleOrderingBackend must preserve existing ORDER BY
	#[rstest]
	#[tokio::test]
	async fn test_ordering_preserves_existing_order_by() {
		// Arrange
		let backend = SimpleOrderingBackend::new("ordering").allow_field("title");
		let mut params = HashMap::new();
		params.insert("ordering".to_string(), "title".to_string());
		let sql = "SELECT * FROM articles ORDER BY created_at ASC".to_string();

		// Act
		let result = backend.filter_queryset(&params, sql).await.unwrap();

		// Assert - both ordering criteria must be present
		assert!(
			result.contains("ORDER BY created_at ASC, title ASC"),
			"Expected existing ORDER BY to be preserved with new criteria appended, got: {}",
			result
		);
	}

	// Regression test for #2685: ORDER BY with LIMIT must be preserved correctly
	#[rstest]
	#[tokio::test]
	async fn test_ordering_preserves_existing_order_by_with_limit() {
		// Arrange
		let backend = SimpleOrderingBackend::new("ordering").allow_field("title");
		let mut params = HashMap::new();
		params.insert("ordering".to_string(), "-title".to_string());
		let sql = "SELECT * FROM articles ORDER BY created_at ASC LIMIT 10".to_string();

		// Act
		let result = backend.filter_queryset(&params, sql).await.unwrap();

		// Assert - both ordering criteria and LIMIT must be present
		assert!(
			result.contains("ORDER BY created_at ASC, title DESC"),
			"Expected existing ORDER BY to be preserved, got: {}",
			result
		);
		assert!(
			result.contains("LIMIT 10"),
			"Expected LIMIT clause to be preserved, got: {}",
			result
		);
	}

	// Regression test for #2687: SimpleSearchBackend must preserve existing WHERE clause
	#[rstest]
	#[tokio::test]
	async fn test_search_preserves_existing_where_clause() {
		// Arrange
		let backend = SimpleSearchBackend::new("search").with_field("title");
		let mut params = HashMap::new();
		params.insert("search".to_string(), "rust".to_string());
		let sql = "SELECT * FROM articles WHERE status = 'published'".to_string();

		// Act
		let result = backend.filter_queryset(&params, sql).await.unwrap();

		// Assert - both the original WHERE condition and search condition must be present
		assert!(
			result.contains("status = 'published'"),
			"Expected original WHERE condition to be preserved, got: {}",
			result
		);
		assert!(
			result.contains("`title` LIKE '%rust%'"),
			"Expected search condition to be added, got: {}",
			result
		);
		assert!(
			result.contains("AND"),
			"Expected AND joining original and search conditions, got: {}",
			result
		);
	}

	// Regression test for #2687: Complex WHERE clause must not be corrupted
	#[rstest]
	#[tokio::test]
	async fn test_search_preserves_complex_where_clause() {
		// Arrange
		let backend = SimpleSearchBackend::new("search").with_field("name");
		let mut params = HashMap::new();
		params.insert("search".to_string(), "john".to_string());
		let sql = "SELECT * FROM users WHERE (age > 18 AND active = true) ORDER BY id".to_string();

		// Act
		let result = backend.filter_queryset(&params, sql).await.unwrap();

		// Assert - original complex condition, search condition, and ORDER BY must all be present
		assert!(
			result.contains("age > 18 AND active = true"),
			"Expected complex WHERE condition to be preserved, got: {}",
			result
		);
		assert!(
			result.contains("`name` LIKE '%john%'"),
			"Expected search condition to be added, got: {}",
			result
		);
		assert!(
			result.contains("ORDER BY"),
			"Expected ORDER BY clause to be preserved, got: {}",
			result
		);
	}

	// Regression test for #2687: Empty query (no WHERE) should add WHERE normally
	#[rstest]
	#[tokio::test]
	async fn test_search_adds_where_to_empty_query() {
		// Arrange
		let backend = SimpleSearchBackend::new("search").with_field("title");
		let mut params = HashMap::new();
		params.insert("search".to_string(), "rust".to_string());
		let sql = "SELECT * FROM articles".to_string();

		// Act
		let result = backend.filter_queryset(&params, sql).await.unwrap();

		// Assert
		assert!(
			result.contains("WHERE"),
			"Expected WHERE clause to be added, got: {}",
			result
		);
		assert!(
			result.contains("`title` LIKE '%rust%'"),
			"Expected search condition, got: {}",
			result
		);
	}

	// Regression test for #2685: Empty query (no ORDER BY) should add ORDER BY normally
	#[rstest]
	#[tokio::test]
	async fn test_ordering_adds_order_by_to_empty_query() {
		// Arrange
		let backend = SimpleOrderingBackend::new("ordering").allow_field("created_at");
		let mut params = HashMap::new();
		params.insert("ordering".to_string(), "-created_at".to_string());
		let sql = "SELECT * FROM articles".to_string();

		// Act
		let result = backend.filter_queryset(&params, sql).await.unwrap();

		// Assert
		assert_eq!(
			result, "SELECT * FROM articles ORDER BY created_at DESC",
			"Expected ORDER BY to be appended to query without existing ORDER BY"
		);
	}
}

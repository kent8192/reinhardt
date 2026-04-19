//! Batch operations for improved performance
//!
//! Provides efficient batch insert, update, and delete operations:
//! - Bulk insert with COPY or multi-value INSERT
//! - Batch updates with optimized queries
//! - Transaction batching

use crate::backends::error::Result;
use crate::backends::types::DatabaseType;
use async_trait::async_trait;

/// Batch operations trait
#[async_trait]
pub trait BatchOperations {
	/// Execute batch insert
	async fn batch_insert(
		&self,
		table: &str,
		columns: &[&str],
		rows: Vec<Vec<String>>,
	) -> Result<u64>;

	/// Execute batch update
	async fn batch_update(
		&self,
		table: &str,
		updates: Vec<(String, Vec<(String, String)>)>, // (where_clause, [(column, value)])
	) -> Result<u64>;

	/// Execute batch delete
	async fn batch_delete(&self, table: &str, ids: Vec<i64>) -> Result<u64>;
}

/// Identifier quoting style for different database backends
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuoteStyle {
	/// ANSI SQL: double quotes (`"identifier"`)
	Ansi,
	/// MySQL: backticks (`` `identifier` ``)
	Backtick,
}

impl QuoteStyle {
	/// Quote an identifier using this style, escaping embedded quote characters
	fn quote_identifier(&self, ident: &str) -> String {
		match self {
			QuoteStyle::Ansi => format!("\"{}\"", ident.replace('"', "\"\"")),
			QuoteStyle::Backtick => format!("`{}`", ident.replace('`', "``")),
		}
	}
}

impl From<DatabaseType> for QuoteStyle {
	/// Convert a [`DatabaseType`] to the appropriate [`QuoteStyle`]
	///
	/// - MySQL uses backtick quoting
	/// - PostgreSQL and SQLite use ANSI double-quote quoting
	fn from(db_type: DatabaseType) -> Self {
		match db_type {
			DatabaseType::Mysql => QuoteStyle::Backtick,
			DatabaseType::Postgres | DatabaseType::Sqlite => QuoteStyle::Ansi,
		}
	}
}

/// Builder for batch insert operations
pub struct BatchInsertBuilder {
	table: String,
	columns: Vec<String>,
	rows: Vec<Vec<String>>,
	batch_size: usize,
	quote_style: QuoteStyle,
}

impl BatchInsertBuilder {
	/// Create a new batch insert builder
	pub fn new(table: impl Into<String>) -> Self {
		Self {
			table: table.into(),
			columns: Vec::new(),
			rows: Vec::new(),
			batch_size: 1000,
			quote_style: QuoteStyle::Ansi,
		}
	}

	/// Set columns for insert
	pub fn columns(mut self, columns: Vec<String>) -> Self {
		self.columns = columns;
		self
	}

	/// Add a row of values
	pub fn add_row(mut self, row: Vec<String>) -> Self {
		self.rows.push(row);
		self
	}

	/// Set batch size (number of rows per INSERT statement)
	pub fn batch_size(mut self, size: usize) -> Self {
		self.batch_size = size;
		self
	}

	/// Set the identifier quoting style for the target database backend
	///
	/// Defaults to [`QuoteStyle::Ansi`] (double quotes). Use
	/// [`QuoteStyle::Backtick`] for MySQL.
	pub fn quote_style(mut self, style: QuoteStyle) -> Self {
		self.quote_style = style;
		self
	}

	/// Build SQL statements for batch insert
	///
	/// Identifiers (table name and column names) are quoted using the
	/// configured [`QuoteStyle`] (defaults to ANSI double quotes).
	/// Embedded quote characters are escaped to prevent SQL injection.
	pub fn build_sql(&self) -> Vec<String> {
		let mut statements = Vec::new();

		let quoted_table = self.quote_style.quote_identifier(&self.table);
		let quoted_columns = self
			.columns
			.iter()
			.map(|c| self.quote_style.quote_identifier(c))
			.collect::<Vec<_>>()
			.join(", ");

		for chunk in self.rows.chunks(self.batch_size) {
			let values_list: Vec<String> = chunk
				.iter()
				.map(|row| {
					let values = row
						.iter()
						.map(|v| format!("'{}'", v.replace('\'', "''")))
						.collect::<Vec<_>>()
						.join(", ");
					format!("({})", values)
				})
				.collect();

			let sql = format!(
				"INSERT INTO {} ({}) VALUES {}",
				quoted_table,
				quoted_columns,
				values_list.join(", ")
			);

			statements.push(sql);
		}

		statements
	}

	/// Get total number of rows
	pub fn row_count(&self) -> usize {
		self.rows.len()
	}
}

/// Internal representation for an update entry with parameterized WHERE clause
struct UpdateEntry {
	/// Column name used in the WHERE equality condition
	where_column: String,
	/// Value bound to the WHERE parameter
	where_value: String,
	/// Column-value pairs to SET
	columns_values: Vec<(String, String)>,
}

/// Builder for batch update operations
///
/// Uses parameterized queries to prevent SQL injection. Add updates with
/// [`add_update_parameterized`](Self::add_update_parameterized) and build
/// statements with [`build_sql_parameterized`](Self::build_sql_parameterized).
pub struct BatchUpdateBuilder {
	table: String,
	updates: Vec<UpdateEntry>,
}

impl BatchUpdateBuilder {
	/// Create a new batch update builder
	pub fn new(table: impl Into<String>) -> Self {
		Self {
			table: table.into(),
			updates: Vec::new(),
		}
	}

	/// Add an update operation with a parameterized WHERE clause
	///
	/// Uses column equality condition (`where_column = $N`) with bind parameter
	/// to prevent SQL injection.
	pub fn add_update_parameterized(
		mut self,
		where_column: String,
		where_value: String,
		columns_values: Vec<(String, String)>,
	) -> Self {
		self.updates.push(UpdateEntry {
			where_column,
			where_value,
			columns_values,
		});
		self
	}

	/// Build parameterized SQL statements for batch update
	///
	/// Returns a list of `(sql, params)` tuples where `sql` contains `$N`
	/// placeholders and `params` contains the corresponding bind values.
	pub fn build_sql_parameterized(&self) -> Vec<(String, Vec<String>)> {
		self.updates
			.iter()
			.map(|entry| {
				let mut params = Vec::with_capacity(entry.columns_values.len() + 1);
				let mut param_idx = 1usize;

				let set_clause = entry
					.columns_values
					.iter()
					.map(|(col, val)| {
						let placeholder = format!("{} = ${}", col, param_idx);
						params.push(val.clone());
						param_idx += 1;
						placeholder
					})
					.collect::<Vec<_>>()
					.join(", ");

				let sql = format!(
					"UPDATE {} SET {} WHERE {} = ${}",
					self.table, set_clause, entry.where_column, param_idx
				);
				params.push(entry.where_value.clone());

				(sql, params)
			})
			.collect()
	}

	/// Get total number of updates
	pub fn update_count(&self) -> usize {
		self.updates.len()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_batch_insert_builder() {
		// Arrange
		let builder = BatchInsertBuilder::new("users")
			.columns(vec!["name".to_string(), "email".to_string()])
			.add_row(vec!["Alice".to_string(), "alice@example.com".to_string()])
			.add_row(vec!["Bob".to_string(), "bob@example.com".to_string()])
			.batch_size(2);

		// Act
		let sql_statements = builder.build_sql();

		// Assert
		assert_eq!(sql_statements.len(), 1);
		assert!(sql_statements[0].contains("INSERT INTO \"users\""));
		assert!(sql_statements[0].contains("\"name\""));
		assert!(sql_statements[0].contains("\"email\""));
		assert!(sql_statements[0].contains("Alice"));
		assert!(sql_statements[0].contains("Bob"));
	}

	#[rstest]
	fn test_batch_insert_chunking() {
		// Arrange
		let mut builder = BatchInsertBuilder::new("users")
			.columns(vec!["name".to_string()])
			.batch_size(2);

		for i in 0..5 {
			builder = builder.add_row(vec![format!("User{}", i)]);
		}

		// Act
		let sql_statements = builder.build_sql();

		// Assert - 5 rows with batch size 2 = 3 SQL statements (2 + 2 + 1)
		assert_eq!(sql_statements.len(), 3);
	}

	#[rstest]
	fn test_batch_update_builder_uses_parameterized_queries() {
		// Arrange
		let builder = BatchUpdateBuilder::new("users")
			.add_update_parameterized(
				"id".to_string(),
				"1".to_string(),
				vec![("name".to_string(), "Alice Updated".to_string())],
			)
			.add_update_parameterized(
				"id".to_string(),
				"2".to_string(),
				vec![("name".to_string(), "Bob Updated".to_string())],
			);

		// Act
		let statements = builder.build_sql_parameterized();

		// Assert
		assert_eq!(statements.len(), 2);
		let (sql, params) = &statements[0];
		assert_eq!(sql, "UPDATE users SET name = $1 WHERE id = $2");
		assert_eq!(params, &["Alice Updated", "1"]);

		let (sql, params) = &statements[1];
		assert_eq!(sql, "UPDATE users SET name = $1 WHERE id = $2");
		assert_eq!(params, &["Bob Updated", "2"]);
	}

	#[rstest]
	fn test_batch_update_sql_injection_in_where_value_is_parameterized() {
		// Arrange - attempt SQL injection via where_value
		let builder = BatchUpdateBuilder::new("users").add_update_parameterized(
			"id".to_string(),
			"1 OR 1=1; DROP TABLE users; --".to_string(),
			vec![("name".to_string(), "hacked".to_string())],
		);

		// Act
		let statements = builder.build_sql_parameterized();

		// Assert - the malicious value is a bind parameter, not in the SQL string
		let (sql, params) = &statements[0];
		assert_eq!(sql, "UPDATE users SET name = $1 WHERE id = $2");
		assert!(!sql.contains("DROP TABLE"));
		assert_eq!(params[1], "1 OR 1=1; DROP TABLE users; --");
	}

	#[rstest]
	fn test_batch_update_sql_injection_in_set_value_is_parameterized() {
		// Arrange - attempt SQL injection via column value
		let builder = BatchUpdateBuilder::new("users").add_update_parameterized(
			"id".to_string(),
			"1".to_string(),
			vec![("name".to_string(), "'; DROP TABLE users; --".to_string())],
		);

		// Act
		let statements = builder.build_sql_parameterized();

		// Assert - the malicious value is a bind parameter, not in the SQL string
		let (sql, params) = &statements[0];
		assert_eq!(sql, "UPDATE users SET name = $1 WHERE id = $2");
		assert!(!sql.contains("DROP TABLE"));
		assert_eq!(params[0], "'; DROP TABLE users; --");
	}

	#[rstest]
	fn test_batch_update_multiple_columns() {
		// Arrange
		let builder = BatchUpdateBuilder::new("users").add_update_parameterized(
			"id".to_string(),
			"42".to_string(),
			vec![
				("name".to_string(), "Alice".to_string()),
				("email".to_string(), "alice@example.com".to_string()),
			],
		);

		// Act
		let statements = builder.build_sql_parameterized();

		// Assert
		let (sql, params) = &statements[0];
		assert_eq!(sql, "UPDATE users SET name = $1, email = $2 WHERE id = $3");
		assert_eq!(params, &["Alice", "alice@example.com", "42"]);
	}

	#[rstest]
	fn test_sql_injection_protection_in_insert() {
		// Arrange
		let builder = BatchInsertBuilder::new("users")
			.columns(vec!["name".to_string()])
			.add_row(vec!["Alice'; DROP TABLE users; --".to_string()]);

		// Act
		let sql_statements = builder.build_sql();

		// Assert - single quotes should be escaped
		assert!(sql_statements[0].contains("Alice''; DROP TABLE users; --"));
	}

	#[rstest]
	fn test_batch_insert_quotes_table_name() {
		// Arrange - table name with double quote injection attempt
		let builder = BatchInsertBuilder::new("users\"; DROP TABLE data; --")
			.columns(vec!["name".to_string()])
			.add_row(vec!["Alice".to_string()]);

		// Act
		let sql_statements = builder.build_sql();

		// Assert - table name must be properly quoted and escaped
		assert!(sql_statements[0].starts_with("INSERT INTO \"users\"\"; DROP TABLE data; --\""));
	}

	#[rstest]
	fn test_batch_insert_quotes_column_names() {
		// Arrange - column name with double quote injection attempt
		let builder = BatchInsertBuilder::new("users")
			.columns(vec![
				"name".to_string(),
				"col\"; DROP TABLE users; --".to_string(),
			])
			.add_row(vec!["Alice".to_string(), "value".to_string()]);

		// Act
		let sql_statements = builder.build_sql();

		// Assert - column names must be properly quoted
		assert!(sql_statements[0].contains("\"name\""));
		assert!(sql_statements[0].contains("\"col\"\"; DROP TABLE users; --\""));
	}

	#[rstest]
	fn test_batch_insert_mysql_backtick_quoting() {
		// Arrange - use backtick quoting for MySQL
		let builder = BatchInsertBuilder::new("users")
			.columns(vec!["name".to_string(), "email".to_string()])
			.add_row(vec!["Alice".to_string(), "alice@example.com".to_string()])
			.quote_style(QuoteStyle::Backtick);

		// Act
		let sql_statements = builder.build_sql();

		// Assert - identifiers should use backticks, not double quotes
		assert_eq!(sql_statements.len(), 1);
		assert!(sql_statements[0].contains("INSERT INTO `users`"));
		assert!(sql_statements[0].contains("`name`"));
		assert!(sql_statements[0].contains("`email`"));
	}

	#[rstest]
	fn test_batch_insert_mysql_backtick_escaping() {
		// Arrange - backtick in identifier should be escaped by doubling
		let builder = BatchInsertBuilder::new("my`table")
			.columns(vec!["col`name".to_string()])
			.add_row(vec!["value".to_string()])
			.quote_style(QuoteStyle::Backtick);

		// Act
		let sql_statements = builder.build_sql();

		// Assert - embedded backticks must be doubled
		assert!(sql_statements[0].contains("INSERT INTO `my``table`"));
		assert!(sql_statements[0].contains("`col``name`"));
	}

	#[rstest]
	fn test_quote_style_from_database_type() {
		// Arrange & Act & Assert
		assert_eq!(QuoteStyle::from(DatabaseType::Mysql), QuoteStyle::Backtick);
		assert_eq!(QuoteStyle::from(DatabaseType::Postgres), QuoteStyle::Ansi);
		assert_eq!(QuoteStyle::from(DatabaseType::Sqlite), QuoteStyle::Ansi);
	}

	#[rstest]
	fn test_batch_insert_with_database_type() {
		// Arrange - use DatabaseType to select quoting style
		let builder = BatchInsertBuilder::new("users")
			.columns(vec!["name".to_string()])
			.add_row(vec!["Alice".to_string()])
			.quote_style(QuoteStyle::from(DatabaseType::Mysql));

		// Act
		let sql_statements = builder.build_sql();

		// Assert - MySQL should use backticks
		assert!(sql_statements[0].contains("INSERT INTO `users`"));
		assert!(sql_statements[0].contains("`name`"));
	}
}

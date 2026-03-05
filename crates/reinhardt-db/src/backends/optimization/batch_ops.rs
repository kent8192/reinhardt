//! Batch operations for improved performance
//!
//! Provides efficient batch insert, update, and delete operations:
//! - Bulk insert with COPY or multi-value INSERT
//! - Batch updates with optimized queries
//! - Transaction batching

use crate::backends::error::Result;
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

/// Builder for batch insert operations
pub struct BatchInsertBuilder {
	table: String,
	columns: Vec<String>,
	rows: Vec<Vec<String>>,
	batch_size: usize,
}

impl BatchInsertBuilder {
	/// Create a new batch insert builder
	pub fn new(table: impl Into<String>) -> Self {
		Self {
			table: table.into(),
			columns: Vec::new(),
			rows: Vec::new(),
			batch_size: 1000,
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

	/// Build SQL statements for batch insert
	pub fn build_sql(&self) -> Vec<String> {
		let mut statements = Vec::new();

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
				self.table,
				self.columns.join(", "),
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

/// Internal representation for update entries supporting both legacy and
/// parameterized formats.
enum UpdateEntry {
	/// Legacy format: raw WHERE clause string (not SQL-injection safe)
	Legacy(String, Vec<(String, String)>),
	/// Parameterized format: (where_column, where_value, column_value_pairs)
	Parameterized(String, String, Vec<(String, String)>),
}

/// Builder for batch update operations
///
/// Supports both legacy raw WHERE clause API (deprecated) and parameterized
/// query API for SQL injection prevention. New code should use
/// [`add_update_parameterized`](Self::add_update_parameterized) and
/// [`build_sql_parameterized`](Self::build_sql_parameterized).
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

	/// Add an update operation with a raw WHERE clause
	///
	/// # Deprecated
	///
	/// This method is vulnerable to SQL injection through the `where_clause`
	/// parameter. Use [`add_update_parameterized`](Self::add_update_parameterized)
	/// instead.
	#[deprecated(
		since = "0.1.0",
		note = "vulnerable to SQL injection; use add_update_parameterized instead"
	)]
	pub fn add_update(
		mut self,
		where_clause: String,
		columns_values: Vec<(String, String)>,
	) -> Self {
		self.updates
			.push(UpdateEntry::Legacy(where_clause, columns_values));
		self
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
		self.updates.push(UpdateEntry::Parameterized(
			where_column,
			where_value,
			columns_values,
		));
		self
	}

	/// Build SQL statements for batch update (legacy format)
	///
	/// # Deprecated
	///
	/// Returns raw SQL strings without parameterization. Use
	/// [`build_sql_parameterized`](Self::build_sql_parameterized) instead.
	#[deprecated(
		since = "0.1.0",
		note = "returns unparameterized SQL; use build_sql_parameterized instead"
	)]
	pub fn build_sql(&self) -> Vec<String> {
		self.updates
			.iter()
			.map(|entry| match entry {
				UpdateEntry::Legacy(where_clause, columns_values) => {
					let set_clause = columns_values
						.iter()
						.map(|(col, val)| format!("{} = '{}'", col, val.replace('\'', "''")))
						.collect::<Vec<_>>()
						.join(", ");

					format!(
						"UPDATE {} SET {} WHERE {}",
						self.table, set_clause, where_clause
					)
				}
				UpdateEntry::Parameterized(where_column, where_value, columns_values) => {
					let set_clause = columns_values
						.iter()
						.map(|(col, val)| format!("{} = '{}'", col, val.replace('\'', "''")))
						.collect::<Vec<_>>()
						.join(", ");

					format!(
						"UPDATE {} SET {} WHERE {} = '{}'",
						self.table,
						set_clause,
						where_column,
						where_value.replace('\'', "''")
					)
				}
			})
			.collect()
	}

	/// Build parameterized SQL statements for batch update
	///
	/// Returns a list of `(sql, params)` tuples where `sql` contains `$N`
	/// placeholders and `params` contains the corresponding bind values.
	///
	/// Legacy entries added via the deprecated `add_update` method are rendered
	/// with inline values (no parameterization) and return an empty params vec.
	pub fn build_sql_parameterized(&self) -> Vec<(String, Vec<String>)> {
		self.updates
			.iter()
			.map(|entry| match entry {
				UpdateEntry::Legacy(where_clause, columns_values) => {
					let set_clause = columns_values
						.iter()
						.map(|(col, val)| format!("{} = '{}'", col, val.replace('\'', "''")))
						.collect::<Vec<_>>()
						.join(", ");

					let sql = format!(
						"UPDATE {} SET {} WHERE {}",
						self.table, set_clause, where_clause
					);
					(sql, Vec::new())
				}
				UpdateEntry::Parameterized(where_column, where_value, columns_values) => {
					let mut params = Vec::with_capacity(columns_values.len() + 1);
					let mut param_idx = 1usize;

					let set_clause = columns_values
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
						self.table, set_clause, where_column, param_idx
					);
					params.push(where_value.clone());

					(sql, params)
				}
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
		assert!(sql_statements[0].contains("INSERT INTO users"));
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
	#[allow(deprecated)] // Testing deprecated API for backward compatibility
	fn test_batch_update_builder_legacy() {
		// Arrange
		let builder = BatchUpdateBuilder::new("users")
			.add_update(
				"id = 1".to_string(),
				vec![("name".to_string(), "Alice Updated".to_string())],
			)
			.add_update(
				"id = 2".to_string(),
				vec![("name".to_string(), "Bob Updated".to_string())],
			);

		// Act
		let sql_statements = builder.build_sql();

		// Assert
		assert_eq!(sql_statements.len(), 2);
		assert!(sql_statements[0].contains("UPDATE users"));
		assert!(sql_statements[0].contains("WHERE id = 1"));
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
}

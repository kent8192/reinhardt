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

/// Builder for batch update operations
pub struct BatchUpdateBuilder {
	table: String,
	updates: Vec<(String, Vec<(String, String)>)>, // (where_clause, [(column, value)])
}

impl BatchUpdateBuilder {
	/// Create a new batch update builder
	pub fn new(table: impl Into<String>) -> Self {
		Self {
			table: table.into(),
			updates: Vec::new(),
		}
	}

	/// Add an update operation
	pub fn add_update(
		mut self,
		where_clause: String,
		columns_values: Vec<(String, String)>,
	) -> Self {
		self.updates.push((where_clause, columns_values));
		self
	}

	/// Build SQL statements for batch update
	pub fn build_sql(&self) -> Vec<String> {
		self.updates
			.iter()
			.map(|(where_clause, columns_values)| {
				let set_clause = columns_values
					.iter()
					.map(|(col, val)| format!("{} = '{}'", col, val.replace('\'', "''")))
					.collect::<Vec<_>>()
					.join(", ");

				format!(
					"UPDATE {} SET {} WHERE {}",
					self.table, set_clause, where_clause
				)
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

	#[test]
	fn test_batch_insert_builder() {
		let builder = BatchInsertBuilder::new("users")
			.columns(vec!["name".to_string(), "email".to_string()])
			.add_row(vec!["Alice".to_string(), "alice@example.com".to_string()])
			.add_row(vec!["Bob".to_string(), "bob@example.com".to_string()])
			.batch_size(2);

		let sql_statements = builder.build_sql();
		assert_eq!(sql_statements.len(), 1);
		assert!(sql_statements[0].contains("INSERT INTO users"));
		assert!(sql_statements[0].contains("Alice"));
		assert!(sql_statements[0].contains("Bob"));
	}

	#[test]
	fn test_batch_insert_chunking() {
		let mut builder = BatchInsertBuilder::new("users")
			.columns(vec!["name".to_string()])
			.batch_size(2);

		// Add 5 rows with batch size 2 = 3 SQL statements
		for i in 0..5 {
			builder = builder.add_row(vec![format!("User{}", i)]);
		}

		let sql_statements = builder.build_sql();
		assert_eq!(sql_statements.len(), 3); // 2 + 2 + 1
	}

	#[test]
	fn test_batch_update_builder() {
		let builder = BatchUpdateBuilder::new("users")
			.add_update(
				"id = 1".to_string(),
				vec![("name".to_string(), "Alice Updated".to_string())],
			)
			.add_update(
				"id = 2".to_string(),
				vec![("name".to_string(), "Bob Updated".to_string())],
			);

		let sql_statements = builder.build_sql();
		assert_eq!(sql_statements.len(), 2);
		assert!(sql_statements[0].contains("UPDATE users"));
		assert!(sql_statements[0].contains("WHERE id = 1"));
	}

	#[test]
	fn test_sql_injection_protection() {
		let builder = BatchInsertBuilder::new("users")
			.columns(vec!["name".to_string()])
			.add_row(vec!["Alice'; DROP TABLE users; --".to_string()]);

		let sql_statements = builder.build_sql();
		// Single quotes should be escaped
		assert!(sql_statements[0].contains("Alice''; DROP TABLE users; --"));
	}
}

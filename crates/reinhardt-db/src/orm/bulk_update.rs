//! Bulk Update Operations with Hybrid Property Support
//!
//! This module provides bulk update operations that integrate with hybrid properties.
//! Based on SQLAlchemy's bulk update functionality.

use crate::hybrid::HybridProperty;
use std::collections::HashMap;

/// Strategy for synchronizing session state after bulk update
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SynchronizeStrategy {
	/// Evaluate changes using Python/Rust code (instance-level)
	Evaluate,
	/// Fetch fresh data from database (SQL-level)
	Fetch,
	/// Don't synchronize (fastest but potentially stale)
	False,
}

/// Bulk update builder for ORM operations
pub struct BulkUpdateBuilder {
	table_name: String,
	updates: HashMap<String, String>,
	where_clause: Option<(String, String)>,
	synchronize: SynchronizeStrategy,
}

impl BulkUpdateBuilder {
	/// Create a new bulk update builder for efficient batch updates
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::bulk_update::BulkUpdateBuilder;
	///
	/// let builder = BulkUpdateBuilder::new("users");
	/// // Can chain: .set().where_clause().build()
	/// ```
	pub fn new(table_name: &str) -> Self {
		Self {
			table_name: table_name.to_string(),
			updates: HashMap::new(),
			where_clause: None,
			synchronize: SynchronizeStrategy::Evaluate,
		}
	}
	/// Set a regular column value
	///
	pub fn set(mut self, column: &str, value: &str) -> Self {
		self.updates.insert(column.to_string(), value.to_string());
		self
	}
	/// Set a hybrid property value (direct property, e.g., fname -> first_name)
	///
	pub fn set_hybrid<T, R>(
		mut self,
		column: &str,
		_property: &HybridProperty<T, R>,
		value: &str,
	) -> Self {
		// For direct hybrid properties, just set the underlying column
		self.updates.insert(column.to_string(), value.to_string());
		self
	}
	/// Set a hybrid property with update expression (e.g., name -> first_name, last_name)
	///
	pub fn set_hybrid_expanded(mut self, updates: Vec<(&str, &str)>) -> Self {
		for (col, val) in updates {
			self.updates.insert(col.to_string(), val.to_string());
		}
		self
	}
	/// Set a parameterized WHERE clause using column equality.
	///
	/// Uses `column = ?` with a bind parameter to prevent SQL injection.
	pub fn where_clause(mut self, column: &str, value: &str) -> Self {
		self.where_clause = Some((column.to_string(), value.to_string()));
		self
	}
	/// Set synchronization strategy
	///
	pub fn synchronize(mut self, strategy: SynchronizeStrategy) -> Self {
		self.synchronize = strategy;
		self
	}
	/// Build the SQL UPDATE statement
	///
	pub fn build(&self) -> (String, Vec<String>, SynchronizeStrategy) {
		let mut set_clauses = Vec::new();
		let mut params = Vec::new();

		// Build SET clauses with quoted column names
		for (col, val) in &self.updates {
			set_clauses.push(format!("\"{}\"=?", col));
			params.push(val.clone());
		}

		let mut sql = format!(
			"UPDATE \"{}\" SET {}",
			self.table_name,
			set_clauses.join(", ")
		);

		if let Some((column, value)) = &self.where_clause {
			sql.push_str(&format!(" WHERE \"{}\"=?", column));
			params.push(value.clone());
		}

		(sql, params, self.synchronize)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_bulk_update_plain() {
		let builder = BulkUpdateBuilder::new("person").set("first_name", "Dr.");

		let (sql, params, _) = builder.build();
		assert!(sql.contains("UPDATE \"person\""));
		assert!(sql.contains("\"first_name\"=?"));
		assert_eq!(params, vec!["Dr."]);
	}

	#[test]
	fn test_bulk_update_with_where() {
		let builder = BulkUpdateBuilder::new("person")
			.set("first_name", "Dr.")
			.where_clause("id", "3");

		let (sql, params, _) = builder.build();
		assert!(sql.contains("UPDATE \"person\""));
		assert!(sql.contains("\"first_name\"=?"));
		assert!(sql.contains("WHERE \"id\"=?"));
		assert_eq!(params, vec!["Dr.", "3"]);
	}

	#[test]
	fn test_bulk_update_expanded() {
		let builder = BulkUpdateBuilder::new("person")
			.set_hybrid_expanded(vec![("first_name", "Dr."), ("last_name", "No")]);

		let (sql, params, _) = builder.build();
		assert!(sql.contains("UPDATE \"person\" SET"));
		assert!(sql.contains("\"first_name\"=?"));
		assert!(sql.contains("\"last_name\"=?"));
		assert_eq!(params.len(), 2);
	}

	#[test]
	fn test_synchronize_strategy() {
		let builder = BulkUpdateBuilder::new("person")
			.set("first_name", "Dr.")
			.synchronize(SynchronizeStrategy::Fetch);

		let (_, _, strategy) = builder.build();
		assert_eq!(strategy, SynchronizeStrategy::Fetch);
	}
}

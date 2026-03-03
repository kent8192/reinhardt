//! Hybrid Property Integration with DML Operations
//!
//! This module provides integration between reinhardt-hybrid properties
//! and DML (Data Manipulation Language) operations like INSERT and UPDATE.
//!
//! Based on SQLAlchemy's hybrid property DML support.

use crate::backends::backend::DatabaseBackend;
use crate::hybrid::HybridProperty;
use std::collections::HashMap;
use std::sync::Arc;

/// Internal marker key for expanded hybrid property values.
///
/// When a hybrid property expands to multiple columns (e.g., Point(x, y) -> x, y),
/// we store them under this special key to distinguish from regular columns.
///
/// Uses a single underscore prefix following Rust conventions for internal identifiers.
const EXPANDED_MARKER: &str = "_expanded";

/// A value that can be inserted/updated, either direct or from a hybrid property
#[derive(Debug, Clone)]
pub enum DmlValue {
	/// Direct value
	Direct(String),
	/// Value from hybrid property expression
	HybridExpression(String),
	/// Multiple columns from hybrid property (e.g., Point(x, y) -> x, y)
	Expanded(Vec<(String, String)>),
}

/// Builder for INSERT statements with hybrid property support
pub struct InsertBuilder {
	table_name: String,
	values: HashMap<String, DmlValue>,
	backend: Option<Arc<dyn DatabaseBackend>>,
}

impl InsertBuilder {
	/// Create a new INSERT builder with hybrid property support
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::hybrid_dml::InsertBuilder;
	///
	/// let builder = InsertBuilder::new("users");
	/// // Can chain: .value().hybrid_value().build()
	/// ```
	pub fn new(table_name: &str) -> Self {
		Self {
			table_name: table_name.to_string(),
			values: HashMap::new(),
			backend: None,
		}
	}

	/// Set the database backend for placeholder generation
	pub fn with_backend(mut self, backend: Arc<dyn DatabaseBackend>) -> Self {
		self.backend = Some(backend);
		self
	}
	/// Add a direct column value
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::hybrid_dml::InsertBuilder;
	///
	/// let builder = InsertBuilder::new("users")
	///     .value("name", "Alice")
	///     .value("email", "alice@example.com");
	///
	/// let (sql, params) = builder.build();
	/// // SQL output order may vary, verify components separately
	/// assert!(sql.starts_with("INSERT INTO users ("));
	/// assert!(sql.contains("name"));
	/// assert!(sql.contains("email"));
	/// assert!(sql.contains(") VALUES (?, ?)"));
	/// assert_eq!(params.len(), 2);
	/// // HashMap doesn't guarantee order, so check both values are present
	/// assert!(params.contains(&"Alice".to_string()));
	/// assert!(params.contains(&"alice@example.com".to_string()));
	/// ```
	pub fn value(mut self, column: &str, value: &str) -> Self {
		self.values
			.insert(column.to_string(), DmlValue::Direct(value.to_string()));
		self
	}
	/// Add a hybrid property value
	///
	/// This method integrates hybrid properties with DML operations.
	/// If the hybrid property has an SQL expression, it will be used;
	/// otherwise, the value is treated as a direct parameter.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::hybrid_dml::InsertBuilder;
	/// use reinhardt_db::hybrid::HybridProperty;
	///
	/// struct User { email: String }
	///
	/// let lower_email = HybridProperty::new(|user: &User| user.email.to_lowercase())
	///     .with_expression(|| "LOWER(email)".to_string());
	///
	/// let builder = InsertBuilder::new("users")
	///     .hybrid_value("email", &lower_email, "TEST@EXAMPLE.COM");
	///
	/// let (sql, _) = builder.build();
	/// assert_eq!(
	///     sql,
	///     "INSERT INTO users (email) VALUES (LOWER('TEST@EXAMPLE.COM'))",
	///     "Expected INSERT with LOWER expression, got: {}",
	///     sql
	/// );
	/// ```
	pub fn hybrid_value<T, R>(
		mut self,
		column: &str,
		property: &HybridProperty<T, R>,
		value: &str,
	) -> Self {
		// If the property has an expression, use it; otherwise treat as direct value
		if let Some(expr) = property.expression() {
			// Replace the column reference in the expression with the actual value
			// For example: "LOWER(email)" -> "LOWER('value')"
			let value_expr = format!("'{}'", value);
			let expanded_expr =
				expr.replace(&format!("({})", column), &format!("({})", value_expr));
			self.values.insert(
				column.to_string(),
				DmlValue::HybridExpression(expanded_expr),
			);
		} else {
			// No expression, use direct value
			self.values
				.insert(column.to_string(), DmlValue::Direct(value.to_string()));
		}
		self
	}
	/// Add an expanded hybrid property (e.g., Point -> x, y)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::hybrid_dml::InsertBuilder;
	///
	/// let builder = InsertBuilder::new("points")
	///     .expanded_hybrid(vec![("x", "10"), ("y", "20")]);
	///
	/// let (sql, params) = builder.build();
	/// assert_eq!(
	///     sql,
	///     "INSERT INTO points (x, y) VALUES (?, ?)",
	///     "Expected INSERT with expanded columns x and y, got: {}",
	///     sql
	/// );
	/// assert_eq!(params.len(), 2);
	/// assert_eq!(params[0], "10");
	/// assert_eq!(params[1], "20");
	/// ```
	pub fn expanded_hybrid(mut self, columns: Vec<(&str, &str)>) -> Self {
		let expanded = columns
			.into_iter()
			.map(|(col, val)| (col.to_string(), val.to_string()))
			.collect();

		// Add a special marker for expanded values
		self.values
			.insert(EXPANDED_MARKER.to_string(), DmlValue::Expanded(expanded));
		self
	}
	/// Build the SQL INSERT statement
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::hybrid_dml::InsertBuilder;
	///
	/// let builder = InsertBuilder::new("users")
	///     .value("name", "Bob")
	///     .value("age", "25");
	///
	/// let (sql, params) = builder.build();
	/// // SQL output order may vary, verify components separately
	/// assert!(sql.starts_with("INSERT INTO users ("));
	/// assert!(sql.contains("name"));
	/// assert!(sql.contains("age"));
	/// assert!(sql.contains(") VALUES (?, ?)"));
	/// assert_eq!(params.len(), 2);
	/// assert!(params.contains(&"Bob".to_string()));
	/// assert!(params.contains(&"25".to_string()));
	/// ```
	pub fn build(&self) -> (String, Vec<String>) {
		let mut columns = Vec::new();
		let mut placeholders = Vec::new();
		let mut params = Vec::new();
		let mut param_index = 1;

		// Get placeholder function
		let get_placeholder = |index: usize| -> String {
			if let Some(ref backend) = self.backend {
				backend.placeholder(index)
			} else {
				// Fallback to ? for backward compatibility
				"?".to_string()
			}
		};

		// Handle expanded values first
		if let Some(DmlValue::Expanded(expanded)) = self.values.get(EXPANDED_MARKER) {
			for (col, val) in expanded {
				columns.push(col.clone());
				placeholders.push(get_placeholder(param_index));
				param_index += 1;
				params.push(val.clone());
			}
		}

		// Handle regular values
		for (col, val) in &self.values {
			if col == EXPANDED_MARKER {
				continue;
			}
			match val {
				DmlValue::Direct(v) => {
					columns.push(col.clone());
					placeholders.push(get_placeholder(param_index));
					param_index += 1;
					params.push(v.clone());
				}
				DmlValue::HybridExpression(expr) => {
					columns.push(col.clone());
					placeholders.push(expr.clone());
				}
				DmlValue::Expanded(_) => {
					// Already handled above
				}
			}
		}

		let sql = format!(
			"INSERT INTO {} ({}) VALUES ({})",
			self.table_name,
			columns.join(", "),
			placeholders.join(", ")
		);

		(sql, params)
	}
}

/// Builder for UPDATE statements with hybrid property support
pub struct UpdateBuilder {
	table_name: String,
	values: HashMap<String, DmlValue>,
	where_clause: Option<(String, String)>,
	backend: Option<Arc<dyn DatabaseBackend>>,
}

impl UpdateBuilder {
	/// Create a new UPDATE builder with hybrid property support
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::hybrid_dml::UpdateBuilder;
	///
	/// let builder = UpdateBuilder::new("users");
	/// // Can chain: .set().where_clause().build()
	/// ```
	pub fn new(table_name: &str) -> Self {
		Self {
			table_name: table_name.to_string(),
			values: HashMap::new(),
			where_clause: None,
			backend: None,
		}
	}

	/// Set the database backend for placeholder generation
	pub fn with_backend(mut self, backend: Arc<dyn DatabaseBackend>) -> Self {
		self.backend = Some(backend);
		self
	}
	/// Add a direct column value
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::hybrid_dml::UpdateBuilder;
	///
	/// let builder = UpdateBuilder::new("users")
	///     .set("name", "Charlie")
	///     .set("age", "30")
	///     .where_clause("id", "1");
	///
	/// let (sql, params) = builder.build();
	/// // SQL output order may vary, verify components separately
	/// assert!(sql.starts_with("UPDATE users SET "));
	/// assert!(sql.contains("name=?"));
	/// assert!(sql.contains("age=?"));
	/// assert!(sql.contains(" WHERE \"id\"=?"));
	/// assert_eq!(params.len(), 3);
	/// assert!(params.contains(&"Charlie".to_string()));
	/// assert!(params.contains(&"30".to_string()));
	/// assert!(params.contains(&"1".to_string()));
	/// ```
	pub fn set(mut self, column: &str, value: &str) -> Self {
		self.values
			.insert(column.to_string(), DmlValue::Direct(value.to_string()));
		self
	}
	/// Add a hybrid property value
	///
	/// This method integrates hybrid properties with UPDATE operations.
	/// If the hybrid property has an SQL expression, it will be used;
	/// otherwise, the value is treated as a direct parameter.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::hybrid_dml::UpdateBuilder;
	/// use reinhardt_db::hybrid::HybridProperty;
	///
	/// struct User { email: String }
	///
	/// let lower_email = HybridProperty::new(|user: &User| user.email.to_lowercase())
	///     .with_expression(|| "LOWER(email)".to_string());
	///
	/// let builder = UpdateBuilder::new("users")
	///     .set_hybrid("email", &lower_email, "UPDATED@EXAMPLE.COM")
	///     .where_clause("id", "1");
	///
	/// let (sql, params) = builder.build();
	/// assert!(sql.contains("email=LOWER('UPDATED@EXAMPLE.COM')"));
	/// assert!(sql.contains("WHERE \"id\"=?"));
	/// assert_eq!(params, vec!["1"]);
	/// ```
	pub fn set_hybrid<T, R>(
		mut self,
		column: &str,
		property: &HybridProperty<T, R>,
		value: &str,
	) -> Self {
		// If the property has an expression, use it; otherwise treat as direct value
		if let Some(expr) = property.expression() {
			// Replace the column reference in the expression with the actual value
			let value_expr = format!("'{}'", value);
			let expanded_expr =
				expr.replace(&format!("({})", column), &format!("({})", value_expr));
			self.values.insert(
				column.to_string(),
				DmlValue::HybridExpression(expanded_expr),
			);
		} else {
			// No expression, use direct value
			self.values
				.insert(column.to_string(), DmlValue::Direct(value.to_string()));
		}
		self
	}
	/// Add an expanded hybrid property (e.g., Point -> x, y)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::hybrid_dml::UpdateBuilder;
	///
	/// let builder = UpdateBuilder::new("points")
	///     .set_expanded(vec![("x", "100"), ("y", "200")])
	///     .where_clause("id", "5");
	///
	/// let (sql, params) = builder.build();
	/// assert!(sql.contains("UPDATE points SET x=?, y=?"));
	/// assert!(sql.contains("WHERE \"id\"=?"));
	/// assert_eq!(params.len(), 3);
	/// assert_eq!(params[0], "100");
	/// assert_eq!(params[1], "200");
	/// assert_eq!(params[2], "5");
	/// ```
	pub fn set_expanded(mut self, columns: Vec<(&str, &str)>) -> Self {
		let expanded = columns
			.into_iter()
			.map(|(col, val)| (col.to_string(), val.to_string()))
			.collect();

		self.values
			.insert(EXPANDED_MARKER.to_string(), DmlValue::Expanded(expanded));
		self
	}
	/// Add a parameterized WHERE clause using column equality.
	///
	/// Uses `"column" = ?` with a bind parameter to prevent SQL injection.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::hybrid_dml::UpdateBuilder;
	///
	/// let builder = UpdateBuilder::new("users")
	///     .set("status", "active")
	///     .where_clause("id", "42");
	///
	/// let (sql, params) = builder.build();
	/// assert!(sql.contains("UPDATE users SET status=?"));
	/// assert!(sql.contains("WHERE \"id\"=?"));
	/// assert_eq!(params.len(), 2);
	/// assert_eq!(params[0], "active");
	/// assert_eq!(params[1], "42");
	/// ```
	pub fn where_clause(mut self, column: &str, value: &str) -> Self {
		self.where_clause = Some((column.to_string(), value.to_string()));
		self
	}
	/// Build the SQL UPDATE statement
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::hybrid_dml::UpdateBuilder;
	///
	/// let builder = UpdateBuilder::new("users")
	///     .set("name", "David")
	///     .where_clause("id", "10");
	///
	/// let (sql, params) = builder.build();
	/// assert!(sql.contains("UPDATE users SET name=?"));
	/// assert!(sql.contains("WHERE \"id\"=?"));
	/// assert_eq!(params.len(), 2);
	/// assert_eq!(params[0], "David");
	/// assert_eq!(params[1], "10");
	/// ```
	pub fn build(&self) -> (String, Vec<String>) {
		let mut set_clauses = Vec::new();
		let mut params = Vec::new();
		let mut param_index = 1;

		// Get placeholder function
		let get_placeholder = |index: usize| -> String {
			if let Some(ref backend) = self.backend {
				backend.placeholder(index)
			} else {
				// Fallback to ? for backward compatibility
				"?".to_string()
			}
		};

		// Handle expanded values first
		if let Some(DmlValue::Expanded(expanded)) = self.values.get(EXPANDED_MARKER) {
			for (col, val) in expanded {
				set_clauses.push(format!("{}={}", col, get_placeholder(param_index)));
				param_index += 1;
				params.push(val.clone());
			}
		}

		// Handle regular values
		for (col, val) in &self.values {
			if col == EXPANDED_MARKER {
				continue;
			}
			match val {
				DmlValue::Direct(v) => {
					set_clauses.push(format!("{}={}", col, get_placeholder(param_index)));
					param_index += 1;
					params.push(v.clone());
				}
				DmlValue::HybridExpression(expr) => {
					set_clauses.push(format!("{}={}", col, expr));
				}
				DmlValue::Expanded(_) => {
					// Already handled above
				}
			}
		}

		let mut sql = format!("UPDATE {} SET {}", self.table_name, set_clauses.join(", "));

		if let Some((column, value)) = &self.where_clause {
			sql.push_str(&format!(" WHERE \"{}\"=?", column));
			params.push(value.clone());
		}

		(sql, params)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_insert_builder_simple() {
		let builder = InsertBuilder::new("person")
			.value("first_name", "John")
			.value("last_name", "Doe");

		let (sql, params) = builder.build();
		// SQL output order may vary, verify components separately
		assert!(
			sql.starts_with("INSERT INTO person ("),
			"Expected INSERT INTO person, got: {}",
			sql
		);
		assert!(
			sql.contains("first_name"),
			"Expected first_name in SQL: {}",
			sql
		);
		assert!(
			sql.contains("last_name"),
			"Expected last_name in SQL: {}",
			sql
		);
		assert!(
			sql.contains(") VALUES (?, ?)"),
			"Expected VALUES (?, ?), got: {}",
			sql
		);
		assert_eq!(
			params.len(),
			2,
			"Expected 2 parameters, got: {}",
			params.len()
		);
		assert!(
			params.contains(&"John".to_string()),
			"Expected 'John' in params: {:?}",
			params
		);
		assert!(
			params.contains(&"Doe".to_string()),
			"Expected 'Doe' in params: {:?}",
			params
		);
	}

	#[test]
	fn test_update_builder_simple() {
		let builder = UpdateBuilder::new("person")
			.set("first_name", "Jane")
			.where_clause("id", "1");

		let (sql, params) = builder.build();
		assert!(
			sql.contains("UPDATE person SET first_name=?"),
			"Expected UPDATE person SET, got: {}",
			sql
		);
		assert!(
			sql.contains("WHERE \"id\"=?"),
			"Expected parameterized WHERE, got: {}",
			sql
		);
		assert_eq!(
			params.len(),
			2,
			"Expected 2 parameters, got: {}",
			params.len()
		);
		assert_eq!(params[0], "Jane", "Expected 'Jane', got: {}", params[0]);
		assert_eq!(params[1], "1", "Expected '1', got: {}", params[1]);
	}

	#[test]
	fn test_insert_builder_expanded() {
		let builder = InsertBuilder::new("points").expanded_hybrid(vec![("x", "10"), ("y", "20")]);

		let (sql, params) = builder.build();
		assert_eq!(
			sql, "INSERT INTO points (x, y) VALUES (?, ?)",
			"Expected INSERT with expanded columns, got: {}",
			sql
		);
		assert_eq!(
			params.len(),
			2,
			"Expected 2 parameters, got: {}",
			params.len()
		);
		assert_eq!(
			params[0], "10",
			"Expected '10' as first param, got: {}",
			params[0]
		);
		assert_eq!(
			params[1], "20",
			"Expected '20' as second param, got: {}",
			params[1]
		);
	}

	#[test]
	fn test_update_builder_expanded() {
		let builder = UpdateBuilder::new("points")
			.set_expanded(vec![("x", "30"), ("y", "40")])
			.where_clause("id", "1");

		let (sql, params) = builder.build();
		assert!(
			sql.contains("UPDATE points SET x=?, y=?"),
			"Expected UPDATE with expanded columns, got: {}",
			sql
		);
		assert!(
			sql.contains("WHERE \"id\"=?"),
			"Expected parameterized WHERE, got: {}",
			sql
		);
		assert_eq!(
			params.len(),
			3,
			"Expected 3 parameters, got: {}",
			params.len()
		);
		assert_eq!(params[0], "30");
		assert_eq!(params[1], "40");
		assert_eq!(params[2], "1");
	}

	#[test]
	fn test_insert_builder_with_hybrid_expression() {
		struct User {
			email: String,
		}

		let lower_email = HybridProperty::new(|user: &User| user.email.to_lowercase())
			.with_expression(|| "LOWER(email)".to_string());

		let builder = InsertBuilder::new("users")
			.value("name", "John")
			.hybrid_value("email", &lower_email, "TEST@EXAMPLE.COM");

		let (sql, params) = builder.build();
		// SQL output order may vary, verify components separately
		assert!(
			sql.starts_with("INSERT INTO users ("),
			"Expected INSERT INTO users, got: {}",
			sql
		);
		assert!(
			sql.contains("name"),
			"Expected 'name' column in SQL: {}",
			sql
		);
		assert!(
			sql.contains("email"),
			"Expected 'email' column in SQL: {}",
			sql
		);
		assert!(
			sql.contains("LOWER('TEST@EXAMPLE.COM')"),
			"Expected LOWER expression in SQL: {}",
			sql
		);
		assert_eq!(
			params.len(),
			1,
			"Expected 1 parameter for name, got: {}",
			params.len()
		);
		assert_eq!(
			params[0], "John",
			"Expected 'John' as parameter, got: {}",
			params[0]
		);
	}

	#[test]
	fn test_insert_builder_with_hybrid_no_expression() {
		struct User {
			email: String,
		}

		let simple_prop = HybridProperty::new(|user: &User| user.email.clone());

		let builder = InsertBuilder::new("users")
			.value("name", "John")
			.hybrid_value("email", &simple_prop, "test@example.com");

		let (sql, params) = builder.build();
		// SQL output order may vary, verify components separately
		assert!(
			sql.starts_with("INSERT INTO users ("),
			"Expected INSERT INTO users, got: {}",
			sql
		);
		assert!(
			sql.contains("name"),
			"Expected 'name' column in SQL: {}",
			sql
		);
		assert!(
			sql.contains("email"),
			"Expected 'email' column in SQL: {}",
			sql
		);
		assert!(
			sql.contains(") VALUES (?, ?)"),
			"Expected VALUES (?, ?), got: {}",
			sql
		);
		assert_eq!(
			params.len(),
			2,
			"Expected 2 parameters, got: {}",
			params.len()
		);
		assert!(
			params.contains(&"John".to_string()),
			"Expected 'John' in params: {:?}",
			params
		);
		assert!(
			params.contains(&"test@example.com".to_string()),
			"Expected 'test@example.com' in params: {:?}",
			params
		);
	}

	#[test]
	fn test_update_builder_with_hybrid_expression() {
		struct User {
			email: String,
		}

		let lower_email = HybridProperty::new(|user: &User| user.email.to_lowercase())
			.with_expression(|| "LOWER(email)".to_string());

		let builder = UpdateBuilder::new("users")
			.set_hybrid("email", &lower_email, "UPDATED@EXAMPLE.COM")
			.where_clause("id", "1");

		let (sql, params) = builder.build();
		assert!(
			sql.contains("email=LOWER('UPDATED@EXAMPLE.COM')"),
			"Expected LOWER expression, got: {}",
			sql
		);
		assert!(
			sql.contains("WHERE \"id\"=?"),
			"Expected parameterized WHERE, got: {}",
			sql
		);
		assert_eq!(
			params.len(),
			1,
			"Expected 1 parameter (WHERE value), got: {}",
			params.len()
		);
		assert_eq!(params[0], "1");
	}

	#[test]
	fn test_update_builder_with_hybrid_no_expression() {
		struct User {
			email: String,
		}

		let simple_prop = HybridProperty::new(|user: &User| user.email.clone());

		let builder = UpdateBuilder::new("users")
			.set_hybrid("email", &simple_prop, "updated@example.com")
			.where_clause("id", "1");

		let (sql, params) = builder.build();
		assert!(
			sql.contains("UPDATE users SET email=?"),
			"Expected UPDATE with direct value, got: {}",
			sql
		);
		assert!(
			sql.contains("WHERE \"id\"=?"),
			"Expected parameterized WHERE, got: {}",
			sql
		);
		assert_eq!(
			params.len(),
			2,
			"Expected 2 parameters, got: {}",
			params.len()
		);
		assert_eq!(params[0], "updated@example.com");
		assert_eq!(params[1], "1");
	}
}

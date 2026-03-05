/// Set operations (UNION, INTERSECT, EXCEPT) similar to Django's QuerySet combinators
use serde::{Deserialize, Serialize};

/// Set operation type
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum SetOperation {
	Union,
	UnionAll,
	Intersect,
	IntersectAll,
	Except,
	ExceptAll,
}

impl SetOperation {
	/// Documentation for `to_sql`
	///
	pub fn to_sql(&self) -> &'static str {
		match self {
			SetOperation::Union => "UNION",
			SetOperation::UnionAll => "UNION ALL",
			SetOperation::Intersect => "INTERSECT",
			SetOperation::IntersectAll => "INTERSECT ALL",
			SetOperation::Except => "EXCEPT",
			SetOperation::ExceptAll => "EXCEPT ALL",
		}
	}
}

/// Combined query using set operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombinedQuery {
	pub queries: Vec<String>,
	pub operations: Vec<SetOperation>,
	pub order_by: Vec<String>,
	pub limit: Option<usize>,
	pub offset: Option<usize>,
}

impl CombinedQuery {
	/// Create a new combined query for set operations (UNION, INTERSECT, EXCEPT)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::set_operations::CombinedQuery;
	///
	/// let query = CombinedQuery::new("SELECT * FROM users WHERE active = true");
	/// assert_eq!(query.queries.len(), 1);
	/// assert!(query.operations.is_empty());
	/// ```
	pub fn new(first_query: impl Into<String>) -> Self {
		Self {
			queries: vec![first_query.into()],
			operations: Vec::new(),
			order_by: Vec::new(),
			limit: None,
			offset: None,
		}
	}
	/// Documentation for `union`
	///
	pub fn union(mut self, query: impl Into<String>) -> Self {
		self.queries.push(query.into());
		self.operations.push(SetOperation::Union);
		self
	}
	/// Documentation for `union_all`
	///
	pub fn union_all(mut self, query: impl Into<String>) -> Self {
		self.queries.push(query.into());
		self.operations.push(SetOperation::UnionAll);
		self
	}
	/// Documentation for `intersect`
	///
	pub fn intersect(mut self, query: impl Into<String>) -> Self {
		self.queries.push(query.into());
		self.operations.push(SetOperation::Intersect);
		self
	}
	/// Documentation for `intersect_all`
	///
	pub fn intersect_all(mut self, query: impl Into<String>) -> Self {
		self.queries.push(query.into());
		self.operations.push(SetOperation::IntersectAll);
		self
	}
	/// Documentation for `except`
	///
	pub fn except(mut self, query: impl Into<String>) -> Self {
		self.queries.push(query.into());
		self.operations.push(SetOperation::Except);
		self
	}
	/// Documentation for `except_all`
	///
	pub fn except_all(mut self, query: impl Into<String>) -> Self {
		self.queries.push(query.into());
		self.operations.push(SetOperation::ExceptAll);
		self
	}
	/// Documentation for `order_by`
	///
	pub fn order_by(mut self, field: impl Into<String>) -> Self {
		self.order_by.push(field.into());
		self
	}
	/// Documentation for `limit`
	///
	pub fn limit(mut self, limit: usize) -> Self {
		self.limit = Some(limit);
		self
	}
	/// Documentation for `offset`
	///
	pub fn offset(mut self, offset: usize) -> Self {
		self.offset = Some(offset);
		self
	}
	/// Documentation for `to_sql`
	///
	pub fn to_sql(&self) -> String {
		if self.queries.is_empty() {
			return String::new();
		}

		if self.queries.len() == 1 {
			return self.queries[0].clone();
		}

		let mut sql = String::new();

		// Add first query in parentheses
		sql.push('(');
		sql.push_str(&self.queries[0]);
		sql.push(')');

		// Add remaining queries with operations
		for (i, query) in self.queries.iter().enumerate().skip(1) {
			if let Some(operation) = self.operations.get(i - 1) {
				sql.push_str(&format!("\n{}\n", operation.to_sql()));
			}
			sql.push('(');
			sql.push_str(query);
			sql.push(')');
		}

		// Add ORDER BY
		if !self.order_by.is_empty() {
			sql.push_str(&format!("\nORDER BY {}", self.order_by.join(", ")));
		}

		// Add LIMIT
		if let Some(limit) = self.limit {
			sql.push_str(&format!("\nLIMIT {}", limit));
		}

		// Add OFFSET
		if let Some(offset) = self.offset {
			sql.push_str(&format!("\nOFFSET {}", offset));
		}

		sql
	}
}

/// Builder for set operations on QuerySets
pub struct SetOperationBuilder {
	base_query: String,
}

impl SetOperationBuilder {
	/// Create a new builder for set operations starting with a base query
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::set_operations::SetOperationBuilder;
	///
	/// let builder = SetOperationBuilder::new("SELECT * FROM users");
	/// // Can chain: .union().intersect().except()
	/// ```
	pub fn new(base_query: impl Into<String>) -> Self {
		Self {
			base_query: base_query.into(),
		}
	}
	/// Documentation for `union`
	///
	pub fn union(self, other_query: impl Into<String>) -> CombinedQuery {
		CombinedQuery::new(self.base_query).union(other_query)
	}
	/// Documentation for `union_all`
	///
	pub fn union_all(self, other_query: impl Into<String>) -> CombinedQuery {
		CombinedQuery::new(self.base_query).union_all(other_query)
	}
	/// Documentation for `intersect`
	///
	pub fn intersect(self, other_query: impl Into<String>) -> CombinedQuery {
		CombinedQuery::new(self.base_query).intersect(other_query)
	}
	/// Documentation for `except`
	///
	pub fn except(self, other_query: impl Into<String>) -> CombinedQuery {
		CombinedQuery::new(self.base_query).except(other_query)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_union() {
		let combined = CombinedQuery::new("SELECT * FROM users WHERE active = true")
			.union("SELECT * FROM users WHERE admin = true");

		let sql = combined.to_sql();
		assert!(sql.contains("SELECT * FROM users WHERE active = true"));
		assert!(sql.contains("UNION"));
		assert!(sql.contains("SELECT * FROM users WHERE admin = true"));
	}

	#[test]
	fn test_union_all() {
		let combined = CombinedQuery::new("SELECT name FROM employees")
			.union_all("SELECT name FROM contractors");

		let sql = combined.to_sql();
		assert!(sql.contains("UNION ALL"));
	}

	#[test]
	fn test_intersect() {
		let combined = CombinedQuery::new("SELECT id FROM customers")
			.intersect("SELECT customer_id FROM orders");

		let sql = combined.to_sql();
		assert!(sql.contains("INTERSECT"));
	}

	#[test]
	fn test_except() {
		let combined =
			CombinedQuery::new("SELECT id FROM all_users").except("SELECT id FROM deleted_users");

		let sql = combined.to_sql();
		assert!(sql.contains("EXCEPT"));
	}

	#[test]
	fn test_multiple_operations() {
		let combined = CombinedQuery::new("SELECT * FROM table1")
			.union("SELECT * FROM table2")
			.union("SELECT * FROM table3");

		let sql = combined.to_sql();
		assert_eq!(sql.matches("UNION").count(), 2);
	}

	#[test]
	fn test_with_order_by() {
		let combined = CombinedQuery::new("SELECT name FROM users WHERE role = 'admin'")
			.union("SELECT name FROM users WHERE role = 'moderator'")
			.order_by("name ASC");

		let sql = combined.to_sql();
		assert!(sql.contains("ORDER BY name ASC"));
	}

	#[test]
	fn test_with_limit() {
		let combined = CombinedQuery::new("SELECT * FROM table1")
			.union("SELECT * FROM table2")
			.limit(10);

		let sql = combined.to_sql();
		assert!(sql.contains("LIMIT 10"));
	}

	#[test]
	fn test_with_offset() {
		let combined = CombinedQuery::new("SELECT * FROM table1")
			.union("SELECT * FROM table2")
			.offset(5);

		let sql = combined.to_sql();
		assert!(sql.contains("OFFSET 5"));
	}

	#[test]
	fn test_with_limit_and_offset() {
		let combined = CombinedQuery::new("SELECT * FROM table1")
			.union("SELECT * FROM table2")
			.order_by("created_at DESC")
			.limit(20)
			.offset(10);

		let sql = combined.to_sql();
		assert!(sql.contains("ORDER BY created_at DESC"));
		assert!(sql.contains("LIMIT 20"));
		assert!(sql.contains("OFFSET 10"));
	}

	#[test]
	fn test_mixed_operations() {
		let combined = CombinedQuery::new("SELECT id FROM table1")
			.union("SELECT id FROM table2")
			.intersect("SELECT id FROM table3");

		let sql = combined.to_sql();
		assert!(sql.contains("UNION"));
		assert!(sql.contains("INTERSECT"));
	}

	#[test]
	fn test_parenthesized_queries() {
		let combined = CombinedQuery::new("SELECT * FROM users").union("SELECT * FROM admins");

		let sql = combined.to_sql();
		// Each query should be in parentheses
		assert!(sql.starts_with("("));
		assert!(sql.contains(")\nUNION\n("));
		assert!(sql.ends_with(")"));
	}

	#[test]
	fn test_set_operation_builder() {
		let builder = SetOperationBuilder::new("SELECT * FROM users");
		let combined = builder.union("SELECT * FROM admins");

		let sql = combined.to_sql();
		assert!(sql.contains("UNION"));
	}
}

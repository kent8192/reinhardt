//! # Common Table Expressions (CTEs)
//!
//! SQL Common Table Expressions (WITH clauses) support.
//!
//! This module is inspired by SQLAlchemy's CTE implementation
//! Copyright 2005-2025 SQLAlchemy authors and contributors
//! Licensed under MIT License. See THIRD-PARTY-NOTICES for details.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Represents a Common Table Expression (WITH clause)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CTE {
	pub name: String,
	pub query: String,
	pub columns: Vec<String>,
	pub recursive: bool,
	pub materialized: Option<bool>,
}

impl CTE {
	/// Create a Common Table Expression (WITH clause)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::cte::CTE;
	///
	/// let cte = CTE::new("recent_users", "SELECT * FROM users WHERE created_at > NOW() - INTERVAL '7 days'");
	/// assert_eq!(cte.name, "recent_users");
	/// assert!(!cte.recursive); // Not recursive by default
	/// ```
	pub fn new(name: impl Into<String>, query: impl Into<String>) -> Self {
		Self {
			name: name.into(),
			query: query.into(),
			columns: Vec::new(),
			recursive: false,
			materialized: None,
		}
	}
	/// Documentation for `with_columns`
	pub fn with_columns(mut self, columns: Vec<String>) -> Self {
		self.columns = columns;
		self
	}
	/// Documentation for `recursive`
	///
	pub fn recursive(mut self) -> Self {
		self.recursive = true;
		self
	}
	/// Documentation for `materialized`
	///
	pub fn materialized(mut self, materialized: bool) -> Self {
		self.materialized = Some(materialized);
		self
	}
	/// Generate SQL for this CTE
	///
	pub fn to_sql(&self) -> String {
		let mut parts = vec![self.name.clone()];

		// Add column list if specified
		if !self.columns.is_empty() {
			parts[0] = format!("{} ({})", parts[0], self.columns.join(", "));
		}

		parts.push("AS".to_string());

		// Add materialized hint if specified (PostgreSQL)
		if let Some(mat) = self.materialized {
			if mat {
				parts.push("MATERIALIZED".to_string());
			} else {
				parts.push("NOT MATERIALIZED".to_string());
			}
		}

		parts.push(format!("({})", self.query));

		parts.join(" ")
	}
}

impl fmt::Display for CTE {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.to_sql())
	}
}

/// Collection of CTEs for building WITH clauses
#[derive(Debug, Clone, Default)]
pub struct CTECollection {
	ctes: Vec<CTE>,
	recursive: bool,
}

impl CTECollection {
	/// Create a new collection of CTEs for building WITH clauses
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::cte::CTECollection;
	///
	/// let collection = CTECollection::new();
	/// assert!(collection.is_empty()); // New collection is empty
	/// ```
	pub fn new() -> Self {
		Self {
			ctes: Vec::new(),
			recursive: false,
		}
	}
	/// Documentation for `add`
	///
	pub fn add(&mut self, cte: CTE) {
		if cte.recursive {
			self.recursive = true;
		}
		self.ctes.push(cte);
	}
	/// Documentation for `get`
	///
	pub fn get(&self, name: &str) -> Option<&CTE> {
		self.ctes.iter().find(|c| c.name == name)
	}
	/// Documentation for `is_empty`
	///
	pub fn is_empty(&self) -> bool {
		self.ctes.is_empty()
	}
	/// Documentation for `len`
	///
	pub fn len(&self) -> usize {
		self.ctes.len()
	}
	/// Generate complete WITH clause
	///
	pub fn to_sql(&self) -> Option<String> {
		if self.ctes.is_empty() {
			return None;
		}

		let with_keyword = if self.recursive {
			"WITH RECURSIVE"
		} else {
			"WITH"
		};

		let cte_sql: Vec<String> = self.ctes.iter().map(|c| c.to_sql()).collect();

		Some(format!("{} {}", with_keyword, cte_sql.join(", ")))
	}
}

/// Builder for creating CTEs
pub struct CTEBuilder {
	name: String,
	query: String,
	columns: Vec<String>,
	recursive: bool,
	materialized: Option<bool>,
}

impl CTEBuilder {
	/// Create a new CTE builder with a name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::cte::CTEBuilder;
	///
	/// let builder = CTEBuilder::new("user_stats");
	/// // Use builder to construct a CTE step by step
	/// ```
	pub fn new(name: impl Into<String>) -> Self {
		Self {
			name: name.into(),
			query: String::new(),
			columns: Vec::new(),
			recursive: false,
			materialized: None,
		}
	}
	/// Documentation for `query`
	///
	pub fn query(mut self, query: impl Into<String>) -> Self {
		self.query = query.into();
		self
	}
	/// Documentation for `columns`
	///
	pub fn columns(mut self, columns: Vec<String>) -> Self {
		self.columns = columns;
		self
	}
	/// Documentation for `column`
	///
	pub fn column(mut self, column: impl Into<String>) -> Self {
		self.columns.push(column.into());
		self
	}
	/// Documentation for `recursive`
	///
	pub fn recursive(mut self) -> Self {
		self.recursive = true;
		self
	}
	/// Documentation for `materialized`
	///
	pub fn materialized(mut self, materialized: bool) -> Self {
		self.materialized = Some(materialized);
		self
	}
	/// Documentation for `build`
	///
	pub fn build(self) -> CTE {
		CTE {
			name: self.name,
			query: self.query,
			columns: self.columns,
			recursive: self.recursive,
			materialized: self.materialized,
		}
	}
}

/// Common CTE patterns
pub struct CTEPatterns;

impl CTEPatterns {
	/// Hierarchical data traversal (organization tree, categories, etc.)
	///
	pub fn recursive_hierarchy(
		cte_name: &str,
		table: &str,
		id_col: &str,
		parent_col: &str,
		root_condition: &str,
	) -> CTE {
		let query = format!(
			r#"
            SELECT {id}, {parent}, 1 as level, CAST({id} AS TEXT) as path
            FROM {table}
            WHERE {root_condition}

            UNION ALL

            SELECT t.{id}, t.{parent}, cte.level + 1, cte.path || '/' || CAST(t.{id} AS TEXT)
            FROM {table} t
            INNER JOIN {cte} cte ON t.{parent} = cte.{id}
            "#,
			id = id_col,
			parent = parent_col,
			table = table,
			root_condition = root_condition,
			cte = cte_name
		);

		CTE::new(cte_name, query.trim()).recursive()
	}
	/// Aggregation with intermediate results
	///
	pub fn aggregation_cte(cte_name: &str, table: &str, group_by: &str, agg_expr: &str) -> CTE {
		let query = format!(
			"SELECT {}, {} FROM {} GROUP BY {}",
			group_by, agg_expr, table, group_by
		);

		CTE::new(cte_name, query)
	}
	/// Date series generation
	///
	pub fn date_series(cte_name: &str, start_date: &str, end_date: &str) -> CTE {
		let query = format!(
			r#"
            SELECT DATE('{}') as date
            UNION ALL
            SELECT DATE(date, '+1 day')
            FROM {}
            WHERE date < DATE('{}')
            "#,
			start_date, cte_name, end_date
		);

		CTE::new(cte_name, query.trim()).recursive()
	}
	/// Number series generation
	///
	pub fn number_series(cte_name: &str, start: i64, end: i64) -> CTE {
		let query = format!(
			r#"
            SELECT {} as n
            UNION ALL
            SELECT n + 1
            FROM {}
            WHERE n < {}
            "#,
			start, cte_name, end
		);

		CTE::new(cte_name, query.trim()).recursive()
	}
	/// Moving average calculation
	///
	pub fn moving_average(
		cte_name: &str,
		table: &str,
		value_col: &str,
		date_col: &str,
		window_size: i32,
	) -> CTE {
		let query = format!(
			r#"
            SELECT
                {},
                {},
                AVG({}) OVER (
                    ORDER BY {}
                    ROWS BETWEEN {} PRECEDING AND CURRENT ROW
                ) as moving_avg
            FROM {}
            ORDER BY {}
            "#,
			date_col,
			value_col,
			value_col,
			date_col,
			window_size - 1,
			table,
			date_col
		);

		CTE::new(cte_name, query.trim())
	}
	/// Deduplication
	///
	pub fn deduplicate(cte_name: &str, table: &str, partition_by: &str, order_by: &str) -> CTE {
		let query = format!(
			r#"
            SELECT *,
                ROW_NUMBER() OVER (PARTITION BY {} ORDER BY {}) as rn
            FROM {}
            "#,
			partition_by, order_by, table
		);

		CTE::new(cte_name, query.trim())
	}
	/// Graph traversal (follows relationships)
	///
	pub fn graph_traversal(
		cte_name: &str,
		table: &str,
		id_col: &str,
		relation_col: &str,
		start_id: i64,
	) -> CTE {
		let query = format!(
			r#"
            SELECT {id}, {relation}, 1 as depth
            FROM {table}
            WHERE {id} = {start_id}

            UNION ALL

            SELECT t.{id}, t.{relation}, cte.depth + 1
            FROM {table} t
            INNER JOIN {cte} cte ON t.{id} = cte.{relation}
            WHERE cte.depth < 100
            "#,
			id = id_col,
			relation = relation_col,
			table = table,
			start_id = start_id,
			cte = cte_name
		);

		CTE::new(cte_name, query.trim()).recursive()
	}
	/// Running total calculation
	///
	pub fn running_total(cte_name: &str, table: &str, amount_col: &str, date_col: &str) -> CTE {
		let query = format!(
			r#"
            SELECT
                {},
                {},
                SUM({}) OVER (ORDER BY {}) as running_total
            FROM {}
            ORDER BY {}
            "#,
			date_col, amount_col, amount_col, date_col, table, date_col
		);

		CTE::new(cte_name, query.trim())
	}
	/// Pivot table simulation
	///
	pub fn pivot(
		cte_name: &str,
		table: &str,
		row_col: &str,
		col_col: &str,
		value_col: &str,
	) -> CTE {
		let query = format!(
			r#"
            SELECT
                {},
                SUM(CASE WHEN {} = 'A' THEN {} ELSE 0 END) as a_value,
                SUM(CASE WHEN {} = 'B' THEN {} ELSE 0 END) as b_value,
                SUM(CASE WHEN {} = 'C' THEN {} ELSE 0 END) as c_value
            FROM {}
            GROUP BY {}
            "#,
			row_col, col_col, value_col, col_col, value_col, col_col, value_col, table, row_col
		);

		CTE::new(cte_name, query.trim())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_cte_creation() {
		let cte = CTE::new(
			"regional_sales",
			"SELECT region, SUM(amount) as total FROM orders GROUP BY region",
		);
		assert_eq!(cte.name, "regional_sales");
		assert!(!cte.recursive);
	}

	#[test]
	fn test_cte_with_columns() {
		let cte = CTE::new("sales", "SELECT * FROM orders")
			.with_columns(vec!["region".to_string(), "total".to_string()]);

		let sql = cte.to_sql();
		assert!(sql.contains("sales (region, total)"));
	}

	#[test]
	fn test_cte_recursive_unit() {
		let cte = CTE::new("tree", "SELECT * FROM nodes").recursive();
		assert!(cte.recursive);
	}

	#[test]
	fn test_materialized_cte() {
		let cte = CTE::new("expensive_query", "SELECT * FROM large_table").materialized(true);
		let sql = cte.to_sql();
		assert!(sql.contains("MATERIALIZED"));
	}

	#[test]
	fn test_not_materialized_cte() {
		let cte = CTE::new("simple_query", "SELECT * FROM small_table").materialized(false);
		let sql = cte.to_sql();
		assert!(sql.contains("NOT MATERIALIZED"));
	}

	#[test]
	fn test_cte_builder() {
		let cte = CTEBuilder::new("user_stats")
			.query("SELECT user_id, COUNT(*) as count FROM posts GROUP BY user_id")
			.column("user_id")
			.column("post_count")
			.build();

		assert_eq!(cte.name, "user_stats");
		assert_eq!(cte.columns.len(), 2);
	}

	#[test]
	fn test_cte_collection() {
		let mut collection = CTECollection::new();

		collection.add(CTE::new("cte1", "SELECT * FROM table1"));
		collection.add(CTE::new("cte2", "SELECT * FROM table2"));

		assert_eq!(collection.len(), 2);
		assert!(collection.get("cte1").is_some());

		let sql = collection.to_sql().unwrap();
		assert!(sql.starts_with("WITH"));
		assert!(sql.contains("cte1"));
		assert!(sql.contains("cte2"));
	}

	#[test]
	fn test_recursive_collection() {
		let mut collection = CTECollection::new();
		collection.add(CTE::new("tree", "SELECT * FROM nodes").recursive());

		let sql = collection.to_sql().unwrap();
		assert!(sql.starts_with("WITH RECURSIVE"));
	}

	#[test]
	fn test_recursive_hierarchy_pattern() {
		let cte = CTEPatterns::recursive_hierarchy(
			"org_tree",
			"employees",
			"id",
			"manager_id",
			"manager_id IS NULL",
		);

		assert!(cte.recursive);
		assert!(cte.query.contains("UNION ALL"));
		assert!(cte.query.contains("level"));
		assert!(cte.query.contains("path"));
	}

	#[test]
	fn test_date_series_pattern() {
		let cte = CTEPatterns::date_series("dates", "2024-01-01", "2024-01-31");

		assert!(cte.recursive);
		assert!(cte.query.contains("DATE"));
		assert!(cte.query.contains("+1 day"));
	}

	#[test]
	fn test_number_series_pattern() {
		let cte = CTEPatterns::number_series("numbers", 1, 100);

		assert!(cte.recursive);
		assert!(cte.query.contains("n + 1"));
	}

	#[test]
	fn test_moving_average_pattern() {
		let cte = CTEPatterns::moving_average("ma", "sales", "amount", "date", 7);

		assert!(cte.query.contains("AVG"));
		assert!(cte.query.contains("OVER"));
		assert!(cte.query.contains("PRECEDING"));
	}

	#[test]
	fn test_deduplicate_pattern() {
		let cte = CTEPatterns::deduplicate("deduped", "users", "email", "created_at DESC");

		assert!(cte.query.contains("ROW_NUMBER()"));
		assert!(cte.query.contains("PARTITION BY"));
	}

	#[test]
	fn test_graph_traversal_pattern() {
		let cte = CTEPatterns::graph_traversal("graph", "relationships", "id", "related_id", 1);

		assert!(cte.recursive);
		assert!(cte.query.contains("depth"));
		assert!(cte.query.contains("UNION ALL"));
	}

	#[test]
	fn test_running_total_pattern() {
		let cte = CTEPatterns::running_total("totals", "transactions", "amount", "date");

		assert!(cte.query.contains("SUM"));
		assert!(cte.query.contains("OVER"));
		assert!(cte.query.contains("running_total"));
	}

	#[test]
	fn test_aggregation_cte_pattern() {
		let cte = CTEPatterns::aggregation_cte(
			"region_totals",
			"sales",
			"region",
			"SUM(amount) as total",
		);

		assert!(cte.query.contains("GROUP BY"));
	}

	#[test]
	fn test_empty_collection_sql() {
		let collection = CTECollection::new();
		assert!(collection.to_sql().is_none());
	}
}

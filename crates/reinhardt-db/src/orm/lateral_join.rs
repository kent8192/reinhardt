/// LATERAL JOIN support for correlated subqueries as JOINs
/// Available in PostgreSQL 9.3+, MySQL 8.0.14+, SQL Server 2017+
use serde::{Deserialize, Serialize};

/// Represents a LATERAL JOIN clause
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LateralJoin {
	pub alias: String,
	pub subquery: String,
	pub join_type: LateralJoinType,
	pub on_condition: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LateralJoinType {
	Inner,
	Left,
	Right,
	Full,
}

impl LateralJoin {
	/// Create a new LATERAL JOIN for correlated subqueries
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::lateral_join::{LateralJoin, LateralJoinType};
	///
	/// let join = LateralJoin::new("sub", "SELECT * FROM orders WHERE user_id = users.id");
	/// assert_eq!(join.alias, "sub");
	/// assert_eq!(join.join_type, LateralJoinType::Left); // Default is LEFT JOIN
	/// assert!(join.on_condition.is_none());
	/// ```
	pub fn new(alias: impl Into<String>, subquery: impl Into<String>) -> Self {
		Self {
			alias: alias.into(),
			subquery: subquery.into(),
			join_type: LateralJoinType::Left,
			on_condition: None,
		}
	}
	/// Documentation for `inner`
	///
	pub fn inner(mut self) -> Self {
		self.join_type = LateralJoinType::Inner;
		self
	}
	/// Documentation for `left`
	///
	pub fn left(mut self) -> Self {
		self.join_type = LateralJoinType::Left;
		self
	}
	/// Documentation for `on`
	///
	pub fn on(mut self, condition: impl Into<String>) -> Self {
		self.on_condition = Some(condition.into());
		self
	}
	/// Generate SQL for LATERAL JOIN
	///
	pub fn to_sql(&self) -> String {
		let join_keyword = match self.join_type {
			LateralJoinType::Inner => "INNER JOIN",
			LateralJoinType::Left => "LEFT JOIN",
			LateralJoinType::Right => "RIGHT JOIN",
			LateralJoinType::Full => "FULL JOIN",
		};

		let on_clause = self
			.on_condition
			.as_ref()
			.map(|c| format!(" ON {}", c))
			.unwrap_or_else(|| " ON true".to_string());

		format!(
			"{} LATERAL ({}) AS {}{}",
			join_keyword, self.subquery, self.alias, on_clause
		)
	}
	/// Generate SQL for MySQL (uses different syntax)
	///
	pub fn to_mysql_sql(&self) -> String {
		// MySQL doesn't use LATERAL keyword but supports similar functionality
		let join_keyword = match self.join_type {
			LateralJoinType::Inner => "INNER JOIN",
			LateralJoinType::Left => "LEFT JOIN",
			LateralJoinType::Right => "RIGHT JOIN",
			LateralJoinType::Full => "FULL JOIN",
		};

		let on_clause = self
			.on_condition
			.as_ref()
			.map(|c| format!(" ON {}", c))
			.unwrap_or_else(|| " ON true".to_string());

		format!(
			"{} ({}) AS {}{}",
			join_keyword, self.subquery, self.alias, on_clause
		)
	}
}

/// Builder for LATERAL JOINs
pub struct LateralJoinBuilder {
	alias: String,
	subquery: String,
	join_type: LateralJoinType,
	on_condition: Option<String>,
}

impl LateralJoinBuilder {
	/// Create a new builder for constructing LATERAL JOINs
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::lateral_join::LateralJoinBuilder;
	///
	/// let builder = LateralJoinBuilder::new("latest");
	/// // Builder methods can be chained: .subquery().left().on().build()
	/// ```
	pub fn new(alias: impl Into<String>) -> Self {
		Self {
			alias: alias.into(),
			subquery: String::new(),
			join_type: LateralJoinType::Left,
			on_condition: None,
		}
	}
	/// Documentation for `subquery`
	///
	pub fn subquery(mut self, query: impl Into<String>) -> Self {
		self.subquery = query.into();
		self
	}
	/// Documentation for `inner`
	///
	pub fn inner(mut self) -> Self {
		self.join_type = LateralJoinType::Inner;
		self
	}
	/// Documentation for `left`
	///
	pub fn left(mut self) -> Self {
		self.join_type = LateralJoinType::Left;
		self
	}
	/// Documentation for `on`
	///
	pub fn on(mut self, condition: impl Into<String>) -> Self {
		self.on_condition = Some(condition.into());
		self
	}
	/// Documentation for `build`
	///
	pub fn build(self) -> LateralJoin {
		LateralJoin {
			alias: self.alias,
			subquery: self.subquery,
			join_type: self.join_type,
			on_condition: self.on_condition,
		}
	}
}

/// Common LATERAL JOIN patterns
pub struct LateralJoinPatterns;

impl LateralJoinPatterns {
	/// Get top N related records per parent
	///
	pub fn top_n_per_group(
		alias: &str,
		table: &str,
		foreign_key: &str,
		parent_key: &str,
		order_by: &str,
		limit: usize,
	) -> LateralJoin {
		let subquery = format!(
			"SELECT * FROM {} WHERE {} = {}.{} ORDER BY {} LIMIT {}",
			table, foreign_key, parent_key, "id", order_by, limit
		);

		LateralJoin::new(alias, subquery).left()
	}

	/// Get latest record per parent
	pub fn latest_per_parent(
		alias: &str,
		table: &str,
		foreign_key: &str,
		parent_table: &str,
		date_field: &str,
	) -> LateralJoin {
		let subquery = format!(
			"SELECT * FROM {} WHERE {} = {}.id ORDER BY {} DESC LIMIT 1",
			table, foreign_key, parent_table, date_field
		);

		LateralJoin::new(alias, subquery).left()
	}
	/// Aggregate calculation per parent
	///
	pub fn aggregate_per_parent(
		alias: &str,
		table: &str,
		foreign_key: &str,
		parent_table: &str,
		aggregate_expr: &str,
	) -> LateralJoin {
		let subquery = format!(
			"SELECT {} FROM {} WHERE {} = {}.id",
			aggregate_expr, table, foreign_key, parent_table
		);

		LateralJoin::new(alias, subquery).left()
	}
	/// Ranked results per parent
	///
	pub fn ranked_per_parent(
		alias: &str,
		table: &str,
		foreign_key: &str,
		parent_table: &str,
		rank_expr: &str,
		limit: usize,
	) -> LateralJoin {
		let subquery = format!(
			r#"
            SELECT *, ROW_NUMBER() OVER (ORDER BY {}) as rank
            FROM {}
            WHERE {} = {}.id
            ORDER BY {}
            LIMIT {}
            "#,
			rank_expr, table, foreign_key, parent_table, rank_expr, limit
		);

		LateralJoin::new(alias, subquery.trim()).left()
	}
	/// Conditional aggregation per parent
	///
	pub fn conditional_aggregate(
		alias: &str,
		table: &str,
		foreign_key: &str,
		parent_table: &str,
		condition: &str,
		aggregate_expr: &str,
	) -> LateralJoin {
		let subquery = format!(
			"SELECT {} FROM {} WHERE {} = {}.id AND {}",
			aggregate_expr, table, foreign_key, parent_table, condition
		);

		LateralJoin::new(alias, subquery).left()
	}
	/// Cross-apply style (get first match)
	///
	pub fn first_match(
		alias: &str,
		table: &str,
		join_condition: &str,
		order_by: &str,
	) -> LateralJoin {
		let subquery = format!(
			"SELECT * FROM {} WHERE {} ORDER BY {} LIMIT 1",
			table, join_condition, order_by
		);

		LateralJoin::new(alias, subquery).inner()
	}
	/// Window function with LATERAL
	///
	pub fn window_aggregate(
		alias: &str,
		table: &str,
		foreign_key: &str,
		parent_table: &str,
		window_expr: &str,
	) -> LateralJoin {
		let subquery = format!(
			"SELECT *, {} FROM {} WHERE {} = {}.id",
			window_expr, table, foreign_key, parent_table
		);

		LateralJoin::new(alias, subquery).left()
	}
}

/// Collection of LATERAL JOINs
#[derive(Debug, Clone, Default)]
pub struct LateralJoins {
	joins: Vec<LateralJoin>,
}

impl LateralJoins {
	/// Create a new collection of LATERAL JOINs
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::lateral_join::LateralJoins;
	///
	/// let joins = LateralJoins::new();
	/// assert!(joins.is_empty());
	/// assert_eq!(joins.len(), 0);
	/// ```
	pub fn new() -> Self {
		Self { joins: Vec::new() }
	}
	/// Documentation for `add`
	///
	pub fn add(&mut self, join: LateralJoin) {
		self.joins.push(join);
	}
	/// Documentation for `is_empty`
	///
	pub fn is_empty(&self) -> bool {
		self.joins.is_empty()
	}
	/// Documentation for `len`
	///
	pub fn len(&self) -> usize {
		self.joins.len()
	}
	/// Documentation for `to_sql`
	///
	pub fn to_sql(&self) -> Vec<String> {
		self.joins.iter().map(|j| j.to_sql()).collect()
	}
	/// Documentation for `to_mysql_sql`
	///
	pub fn to_mysql_sql(&self) -> Vec<String> {
		self.joins.iter().map(|j| j.to_mysql_sql()).collect()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_lateral_join_creation() {
		let join = LateralJoin::new("sub", "SELECT * FROM orders WHERE user_id = users.id");
		assert_eq!(join.alias, "sub");
		assert_eq!(join.join_type, LateralJoinType::Left);
	}

	#[test]
	fn test_lateral_join_types() {
		let inner = LateralJoin::new("sub", "SELECT 1").inner();
		assert_eq!(inner.join_type, LateralJoinType::Inner);

		let left = LateralJoin::new("sub", "SELECT 1").left();
		assert_eq!(left.join_type, LateralJoinType::Left);
	}

	#[test]
	fn test_lateral_join_sql() {
		let join = LateralJoin::new(
			"recent_orders",
			"SELECT * FROM orders WHERE user_id = users.id LIMIT 5",
		)
		.left();

		let sql = join.to_sql();
		assert!(sql.contains("LEFT JOIN LATERAL"));
		assert!(sql.contains("recent_orders"));
		assert!(sql.contains("ON true"));
	}

	#[test]
	fn test_lateral_join_with_on_condition() {
		let join =
			LateralJoin::new("sub", "SELECT * FROM items").on("sub.category_id = categories.id");

		let sql = join.to_sql();
		assert!(sql.contains("ON sub.category_id = categories.id"));
	}

	#[test]
	fn test_lateral_join_builder_pattern() {
		let join = LateralJoinBuilder::new("latest")
			.subquery(
				"SELECT * FROM events WHERE user_id = users.id ORDER BY created_at DESC LIMIT 1",
			)
			.left()
			.build();

		assert_eq!(join.alias, "latest");
		assert!(join.subquery.contains("ORDER BY"));
	}

	#[test]
	fn test_top_n_pattern() {
		let join = LateralJoinPatterns::top_n_per_group(
			"top_products",
			"products",
			"category_id",
			"categories",
			"sales DESC",
			3,
		);

		let sql = join.to_sql();
		assert!(sql.contains("LIMIT 3"));
		assert!(sql.contains("ORDER BY sales DESC"));
	}

	#[test]
	fn test_latest_per_parent_pattern() {
		let join = LateralJoinPatterns::latest_per_parent(
			"latest_order",
			"orders",
			"customer_id",
			"customers",
			"created_at",
		);

		let sql = join.to_sql();
		assert!(sql.contains("LIMIT 1"));
		assert!(sql.contains("ORDER BY created_at DESC"));
	}

	#[test]
	fn test_aggregate_per_parent_pattern() {
		let join = LateralJoinPatterns::aggregate_per_parent(
			"order_stats",
			"orders",
			"customer_id",
			"customers",
			"COUNT(*) as order_count, SUM(total) as total_spent",
		);

		let sql = join.to_sql();
		assert!(sql.contains("COUNT(*)"));
		assert!(sql.contains("SUM(total)"));
	}

	#[test]
	fn test_ranked_per_parent_pattern() {
		let join = LateralJoinPatterns::ranked_per_parent(
			"ranked_reviews",
			"reviews",
			"product_id",
			"products",
			"rating DESC, helpful_count DESC",
			5,
		);

		let sql = join.to_sql();
		assert!(sql.contains("ROW_NUMBER()"));
		assert!(sql.contains("LIMIT 5"));
	}

	#[test]
	fn test_conditional_aggregate_pattern() {
		let join = LateralJoinPatterns::conditional_aggregate(
			"high_value_orders",
			"orders",
			"customer_id",
			"customers",
			"total > 1000",
			"COUNT(*) as high_value_count, SUM(total) as high_value_total",
		);

		let sql = join.to_sql();
		assert!(sql.contains("total > 1000"));
		assert!(sql.contains("COUNT(*)"));
	}

	#[test]
	fn test_first_match_pattern() {
		let join = LateralJoinPatterns::first_match(
			"matching_promo",
			"promotions",
			"promotions.category = products.category",
			"priority DESC",
		);

		assert_eq!(join.join_type, LateralJoinType::Inner);
		let sql = join.to_sql();
		assert!(sql.contains("LIMIT 1"));
	}

	#[test]
	fn test_lateral_join_mysql_sql() {
		let join = LateralJoin::new("sub", "SELECT * FROM orders LIMIT 5");
		let sql = join.to_mysql_sql();

		// MySQL doesn't use LATERAL keyword
		assert!(!sql.contains("LATERAL"));
		assert!(sql.contains("LEFT JOIN"));
	}

	#[test]
	fn test_lateral_joins_collection() {
		let mut joins = LateralJoins::new();

		joins.add(LateralJoin::new("j1", "SELECT 1"));
		joins.add(LateralJoin::new("j2", "SELECT 2"));

		assert_eq!(joins.len(), 2);

		let sqls = joins.to_sql();
		assert_eq!(sqls.len(), 2);
	}

	#[test]
	fn test_lateral_join_empty_collection() {
		let joins = LateralJoins::new();
		assert!(joins.is_empty());
		assert_eq!(joins.len(), 0);
	}

	#[test]
	fn test_window_aggregate_pattern() {
		let join = LateralJoinPatterns::window_aggregate(
			"windowed",
			"sales",
			"product_id",
			"products",
			"AVG(amount) OVER (ORDER BY date ROWS 7 PRECEDING) as moving_avg",
		);

		let sql = join.to_sql();
		assert!(sql.contains("AVG(amount) OVER"));
		assert!(sql.contains("ROWS 7 PRECEDING"));
	}
}

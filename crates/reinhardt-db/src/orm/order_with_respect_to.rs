//! Order with respect to functionality
//!
//! Django's order_with_respect_to allows automatic ordering of model instances
//! relative to a parent model or set of fields.

use crate::orm::query_types::DbBackend;
use reinhardt_query::prelude::{
	Alias, Expr, ExprTrait, Func, MySqlQueryBuilder, Order, PostgresQueryBuilder, Query,
	QueryStatementBuilder, SqliteQueryBuilder,
};
use serde::{Deserialize, Serialize};
use sqlx::{AnyPool, Row};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

/// Error type for ordering operations
#[non_exhaustive]
#[derive(Debug)]
pub enum OrderError {
	InvalidOrder(String),
	OrderFieldNotFound(String),
	UpdateFailed(String),
}

impl fmt::Display for OrderError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			OrderError::InvalidOrder(msg) => write!(f, "Invalid order value: {}", msg),
			OrderError::OrderFieldNotFound(msg) => write!(f, "Order field not found: {}", msg),
			OrderError::UpdateFailed(msg) => write!(f, "Failed to update order: {}", msg),
		}
	}
}

impl std::error::Error for OrderError {}

/// Value type for filter conditions
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrderValue {
	Integer(i64),
	String(String),
	Boolean(bool),
}

/// Manages ordering for models with order_with_respect_to
///
/// # Examples
///
/// ```
/// use reinhardt_db::orm::order_with_respect_to::OrderedModel;
/// use reinhardt_db::orm::query_types::DbBackend;
/// use std::collections::HashMap;
/// use std::sync::Arc;
/// use sqlx::AnyPool;
///
/// # async fn example() {
/// # let pool = Arc::new(AnyPool::connect("sqlite::memory:").await.unwrap());
/// let ordered = OrderedModel::new(
///     "order".to_string(),
///     vec!["parent_id".to_string()],
///     "items".to_string(),
///     pool,
///     DbBackend::Sqlite,
/// );
///
/// assert_eq!(ordered.order_field(), "order");
/// assert_eq!(ordered.order_with_respect_to(), &["parent_id"]);
/// # }
/// ```
pub struct OrderedModel {
	/// Field name for storing the order
	order_field: String,
	/// Fields that define the ordering scope
	order_with_respect_to: Vec<String>,
	/// Table name
	table_name: String,
	/// Database connection pool
	pool: Arc<AnyPool>,
	/// Database backend type
	db_backend: DbBackend,
}

impl OrderedModel {
	/// Creates a new OrderedModel
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::order_with_respect_to::OrderedModel;
	/// use reinhardt_db::orm::query_types::DbBackend;
	/// use std::sync::Arc;
	/// use sqlx::AnyPool;
	///
	/// # async fn example() {
	/// # let pool = Arc::new(AnyPool::connect("sqlite::memory:").await.unwrap());
	/// let ordered = OrderedModel::new(
	///     "_order".to_string(),
	///     vec!["category_id".to_string()],
	///     "products".to_string(),
	///     pool,
	///     DbBackend::Sqlite,
	/// );
	///
	/// assert_eq!(ordered.order_field(), "_order");
	/// # }
	/// ```
	pub fn new(
		order_field: String,
		order_with_respect_to: Vec<String>,
		table_name: String,
		pool: Arc<AnyPool>,
		db_backend: DbBackend,
	) -> Self {
		Self {
			order_field,
			order_with_respect_to,
			table_name,
			pool,
			db_backend,
		}
	}

	/// Gets the order field name
	pub fn order_field(&self) -> &str {
		&self.order_field
	}

	/// Gets the fields that define the ordering scope
	pub fn order_with_respect_to(&self) -> &[String] {
		&self.order_with_respect_to
	}

	/// Gets the next order value for a given filter scope
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::order_with_respect_to::{OrderedModel, OrderValue};
	/// use reinhardt_db::orm::query_types::DbBackend;
	/// use std::collections::HashMap;
	/// use std::sync::Arc;
	/// use sqlx::AnyPool;
	///
	/// # async fn example() {
	/// # let pool = Arc::new(AnyPool::connect("sqlite::memory:").await.unwrap());
	/// let ordered = OrderedModel::new(
	///     "order".to_string(),
	///     vec!["parent_id".to_string()],
	///     "items".to_string(),
	///     pool,
	///     DbBackend::Sqlite,
	/// );
	///
	/// let mut filters = HashMap::new();
	/// filters.insert("parent_id".to_string(), OrderValue::Integer(1));
	///
	/// let next_order = ordered.get_next_order(filters).await.unwrap();
	/// // Returns the next available order number in the scope
	/// # }
	/// ```
	pub async fn get_next_order(
		&self,
		filters: HashMap<String, OrderValue>,
	) -> Result<i32, OrderError> {
		let backend = self.get_backend();

		// Build SELECT MAX(order_field) FROM table WHERE filters
		let mut select_stmt = Query::select()
			.from(Alias::new(&self.table_name))
			.expr(Func::max(
				Expr::col(Alias::new(&self.order_field)).into_simple_expr(),
			))
			.to_owned();

		// Add WHERE clauses for filters
		for (col_name, col_value) in &filters {
			select_stmt.and_where(
				Expr::col(Alias::new(col_name)).eq(Expr::val(order_value_to_sea_value(col_value))),
			);
		}

		// Build SQL based on backend
		let (sql, values) = match backend {
			DbBackend::Postgres => select_stmt.build(PostgresQueryBuilder),
			DbBackend::Mysql => select_stmt.build(MySqlQueryBuilder),
			DbBackend::Sqlite => select_stmt.build(SqliteQueryBuilder),
		};

		// Execute query
		let mut query = sqlx::query(&sql);
		for value in &values.0 {
			query = bind_sea_value(query, value);
		}

		let row = query
			.fetch_optional(&*self.pool)
			.await
			.map_err(|e| OrderError::UpdateFailed(format!("Failed to query max order: {}", e)))?;

		if let Some(row) = row {
			// Extract max value - it could be NULL if no rows exist
			let max_order: Option<i32> = row.try_get(0).map_err(|e| {
				OrderError::UpdateFailed(format!("Failed to get max order value: {}", e))
			})?;

			Ok(max_order.map(|v| v + 1).unwrap_or(0))
		} else {
			// No rows in this scope, start from 0
			Ok(0)
		}
	}

	/// Get database backend type
	fn get_backend(&self) -> DbBackend {
		// Return the backend type that was provided during OrderedModel creation
		self.db_backend
	}

	/// Moves an object up in the ordering (decreases order value)
	pub async fn move_up(&self, current_order: i32) -> Result<i32, OrderError> {
		if current_order <= 0 {
			return Err(OrderError::InvalidOrder(
				"Cannot move up from position 0".to_string(),
			));
		}
		Ok(current_order - 1)
	}

	/// Moves an object down in the ordering (increases order value)
	pub async fn move_down(&self, current_order: i32, max_order: i32) -> Result<i32, OrderError> {
		if current_order >= max_order {
			return Err(OrderError::InvalidOrder(format!(
				"Cannot move down from max position {}",
				max_order
			)));
		}
		Ok(current_order + 1)
	}

	/// Moves an object to a specific position in the ordering
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::orm::order_with_respect_to::OrderedModel;
	/// use reinhardt_db::orm::query_types::DbBackend;
	/// use sqlx::AnyPool;
	/// use std::sync::Arc;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let pool = AnyPool::connect("sqlite::memory:").await?;
	/// let ordered = OrderedModel::new(
	///     "order".to_string(),
	///     vec!["parent_id".to_string()],
	///     "items".to_string(),
	///     Arc::new(pool),
	///     DbBackend::Sqlite,
	/// );
	///
	/// let new_order = ordered.move_to_position(5, 10, 3).await?;
	/// assert_eq!(new_order, 3);
	/// # Ok(())
	/// # }
	/// ```
	pub async fn move_to_position(
		&self,
		_current_order: i32,
		max_order: i32,
		new_position: i32,
	) -> Result<i32, OrderError> {
		if new_position < 0 || new_position > max_order {
			return Err(OrderError::InvalidOrder(format!(
				"Invalid position: {} (max: {})",
				new_position, max_order
			)));
		}
		Ok(new_position)
	}

	/// Swaps the order of two objects
	pub async fn swap_order(&self, order1: i32, order2: i32) -> Result<(i32, i32), OrderError> {
		Ok((order2, order1))
	}

	/// Reorders all objects in a scope sequentially (0, 1, 2, ...)
	///
	/// This fetches all records in the scope, orders them by the order field,
	/// and updates each one with sequential order values starting from 0.
	///
	/// Returns the list of new order values assigned.
	pub async fn reorder_all(
		&self,
		filters: HashMap<String, OrderValue>,
	) -> Result<Vec<i32>, OrderError> {
		let backend = self.get_backend();

		// Step 1: Query all IDs in the scope, ordered by current order field
		let mut select_stmt = Query::select()
			.from(Alias::new(&self.table_name))
			.column(Alias::new("id"))
			.column(Alias::new(&self.order_field))
			.order_by(Alias::new(&self.order_field), Order::Asc)
			.to_owned();

		// Add WHERE clauses for filters
		for (col_name, col_value) in &filters {
			select_stmt.and_where(
				Expr::col(Alias::new(col_name)).eq(Expr::val(order_value_to_sea_value(col_value))),
			);
		}

		// Build and execute SELECT query
		let (sql, values) = match backend {
			DbBackend::Postgres => select_stmt.build(PostgresQueryBuilder),
			DbBackend::Mysql => select_stmt.build(MySqlQueryBuilder),
			DbBackend::Sqlite => select_stmt.build(SqliteQueryBuilder),
		};

		let mut query = sqlx::query(&sql);
		for value in &values.0 {
			query = bind_sea_value(query, value);
		}

		let rows = query.fetch_all(&*self.pool).await.map_err(|e| {
			OrderError::UpdateFailed(format!("Failed to fetch records for reordering: {}", e))
		})?;

		let mut new_orders = Vec::new();

		// Step 2: Update each record with sequential order values
		for (idx, row) in rows.iter().enumerate() {
			let id: i64 = row.try_get("id").map_err(|e| {
				OrderError::UpdateFailed(format!("Failed to get id from row: {}", e))
			})?;

			let new_order = idx as i32;
			new_orders.push(new_order);

			// Build UPDATE statement for this record
			let update_stmt = Query::update()
				.table(Alias::new(&self.table_name))
				.value(Alias::new(&self.order_field), new_order)
				.and_where(Expr::col(Alias::new("id")).eq(Expr::val(id)))
				.to_owned();

			// Build and execute UPDATE query
			let (update_sql, update_values) = match backend {
				DbBackend::Postgres => update_stmt.build(PostgresQueryBuilder),
				DbBackend::Mysql => update_stmt.build(MySqlQueryBuilder),
				DbBackend::Sqlite => update_stmt.build(SqliteQueryBuilder),
			};

			let mut update_query = sqlx::query(&update_sql);
			for value in &update_values.0 {
				update_query = bind_sea_value(update_query, value);
			}

			update_query.execute(&*self.pool).await.map_err(|e| {
				OrderError::UpdateFailed(format!("Failed to update order for id {}: {}", id, e))
			})?;
		}

		Ok(new_orders)
	}

	/// Validates an order value
	pub fn validate_order(&self, order: i32, max_order: i32) -> Result<(), OrderError> {
		if order < 0 {
			return Err(OrderError::InvalidOrder(format!(
				"Order must be non-negative, got {}",
				order
			)));
		}
		if order > max_order {
			return Err(OrderError::InvalidOrder(format!(
				"Order {} exceeds maximum {}",
				order, max_order
			)));
		}
		Ok(())
	}
}

/// Convert OrderValue to reinhardt-query Value
fn order_value_to_sea_value(value: &OrderValue) -> reinhardt_query::value::Value {
	match value {
		OrderValue::Integer(i) => reinhardt_query::value::Value::BigInt(Some(*i)),
		OrderValue::String(s) => reinhardt_query::value::Value::String(Some(Box::new(s.clone()))),
		OrderValue::Boolean(b) => reinhardt_query::value::Value::Bool(Some(*b)),
	}
}

/// Bind reinhardt-query Value to sqlx query
fn bind_sea_value<'a>(
	query: sqlx::query::Query<'a, sqlx::Any, sqlx::any::AnyArguments<'a>>,
	value: &reinhardt_query::value::Value,
) -> sqlx::query::Query<'a, sqlx::Any, sqlx::any::AnyArguments<'a>> {
	match value {
		reinhardt_query::value::Value::Bool(Some(b)) => query.bind(*b),
		reinhardt_query::value::Value::TinyInt(Some(i)) => query.bind(*i as i32),
		reinhardt_query::value::Value::SmallInt(Some(i)) => query.bind(*i as i32),
		reinhardt_query::value::Value::Int(Some(i)) => query.bind(*i),
		reinhardt_query::value::Value::BigInt(Some(i)) => query.bind(*i),
		reinhardt_query::value::Value::TinyUnsigned(Some(i)) => query.bind(*i as i32),
		reinhardt_query::value::Value::SmallUnsigned(Some(i)) => query.bind(*i as i32),
		reinhardt_query::value::Value::Unsigned(Some(i)) => query.bind(*i as i64),
		reinhardt_query::value::Value::BigUnsigned(Some(i)) => query.bind(*i as i64),
		reinhardt_query::value::Value::Float(Some(f)) => query.bind(*f),
		reinhardt_query::value::Value::Double(Some(f)) => query.bind(*f),
		reinhardt_query::value::Value::String(Some(s)) => query.bind(s.as_ref().clone()),
		reinhardt_query::value::Value::Bytes(Some(b)) => query.bind(b.as_ref().clone()),
		_ => query.bind(None::<i32>), // NULL values
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;
	use serial_test::serial;

	// Helper function to create a test pool
	async fn create_test_pool() -> Arc<AnyPool> {
		use sqlx::pool::PoolOptions;

		// Initialize SQLx drivers (idempotent operation)
		sqlx::any::install_default_drivers();

		// Use shared in-memory database so all connections see the same data
		// The "mode=memory" and "cache=shared" ensure the database persists across connections
		let pool = PoolOptions::new()
			.min_connections(1)
			.max_connections(5)
			.connect("sqlite:file:test_order_db?mode=memory&cache=shared")
			.await
			.expect("Failed to create test pool");

		// Create the test_table for testing
		// Schema matches what order_with_respect_to tests expect
		sqlx::query(
			r#"CREATE TABLE IF NOT EXISTS test_table (
				id INTEGER PRIMARY KEY,
				"order" INTEGER NOT NULL DEFAULT 0,
				parent_id INTEGER NOT NULL,
				category_id INTEGER
			)"#,
		)
		.execute(&pool)
		.await
		.expect("Failed to create test_table");

		Arc::new(pool)
	}

	/// Initialize SQLx drivers (required for AnyPool)
	#[fixture]
	fn init_drivers() {
		sqlx::any::install_default_drivers();
	}

	#[rstest]
	#[serial(sqlx_drivers)]
	#[tokio::test]
	async fn test_ordered_model_creation(_init_drivers: ()) {
		let pool = create_test_pool().await;
		let ordered = OrderedModel::new(
			"_order".to_string(),
			vec!["category_id".to_string()],
			"test_table".to_string(),
			pool,
			DbBackend::Sqlite,
		);

		assert_eq!(ordered.order_field(), "_order");
		assert_eq!(ordered.order_with_respect_to().len(), 1);
		assert_eq!(ordered.order_with_respect_to()[0], "category_id");
	}

	#[rstest]
	#[serial(sqlx_drivers)]
	#[tokio::test]
	async fn test_ordered_model_with_multiple_fields(_init_drivers: ()) {
		let pool = create_test_pool().await;
		let ordered = OrderedModel::new(
			"order".to_string(),
			vec!["parent_id".to_string(), "category_id".to_string()],
			"test_table".to_string(),
			pool,
			DbBackend::Sqlite,
		);

		assert_eq!(ordered.order_with_respect_to().len(), 2);
	}

	#[rstest]
	#[serial(sqlx_drivers)]
	#[tokio::test]
	async fn test_get_next_order(_init_drivers: ()) {
		let pool = create_test_pool().await;
		let ordered = OrderedModel::new(
			"order".to_string(),
			vec!["parent_id".to_string()],
			"test_table".to_string(),
			pool,
			DbBackend::Sqlite,
		);

		let mut filters = HashMap::new();
		filters.insert("parent_id".to_string(), OrderValue::Integer(1));

		let next_order = ordered.get_next_order(filters).await.unwrap();
		assert_eq!(next_order, 0);
	}

	#[rstest]
	#[serial(sqlx_drivers)]
	#[tokio::test]
	async fn test_move_up(_init_drivers: ()) {
		let pool = create_test_pool().await;
		let ordered = OrderedModel::new(
			"order".to_string(),
			vec!["parent_id".to_string()],
			"test_table".to_string(),
			pool,
			DbBackend::Sqlite,
		);

		let new_order = ordered.move_up(5).await.unwrap();
		assert_eq!(new_order, 4);

		let result = ordered.move_up(0).await;
		assert!(result.is_err());
	}

	#[rstest]
	#[serial(sqlx_drivers)]
	#[tokio::test]
	async fn test_move_down(_init_drivers: ()) {
		let pool = create_test_pool().await;
		let ordered = OrderedModel::new(
			"order".to_string(),
			vec!["parent_id".to_string()],
			"test_table".to_string(),
			pool,
			DbBackend::Sqlite,
		);

		let new_order = ordered.move_down(3, 10).await.unwrap();
		assert_eq!(new_order, 4);

		let result = ordered.move_down(10, 10).await;
		assert!(result.is_err());
	}

	#[rstest]
	#[serial(sqlx_drivers)]
	#[tokio::test]
	async fn test_move_to_position(_init_drivers: ()) {
		let pool = create_test_pool().await;
		let ordered = OrderedModel::new(
			"order".to_string(),
			vec!["parent_id".to_string()],
			"test_table".to_string(),
			pool,
			DbBackend::Sqlite,
		);

		let new_order = ordered.move_to_position(5, 10, 7).await.unwrap();
		assert_eq!(new_order, 7);

		let result = ordered.move_to_position(5, 10, 15).await;
		assert!(result.is_err());

		let result = ordered.move_to_position(5, 10, -1).await;
		assert!(result.is_err());
	}

	#[rstest]
	#[serial(sqlx_drivers)]
	#[tokio::test]
	async fn test_swap_order(_init_drivers: ()) {
		let pool = create_test_pool().await;
		let ordered = OrderedModel::new(
			"order".to_string(),
			vec!["parent_id".to_string()],
			"test_table".to_string(),
			pool,
			DbBackend::Sqlite,
		);

		let (new_order1, new_order2) = ordered.swap_order(3, 7).await.unwrap();
		assert_eq!(new_order1, 7);
		assert_eq!(new_order2, 3);
	}

	#[rstest]
	#[serial(sqlx_drivers)]
	#[tokio::test]
	async fn test_validate_order(_init_drivers: ()) {
		let pool = create_test_pool().await;
		let ordered = OrderedModel::new(
			"order".to_string(),
			vec!["parent_id".to_string()],
			"test_table".to_string(),
			pool,
			DbBackend::Sqlite,
		);

		assert!(ordered.validate_order(5, 10).is_ok());
		assert!(ordered.validate_order(0, 10).is_ok());
		assert!(ordered.validate_order(10, 10).is_ok());

		assert!(ordered.validate_order(-1, 10).is_err());
		assert!(ordered.validate_order(11, 10).is_err());
	}

	#[rstest]
	#[serial(sqlx_drivers)]
	#[tokio::test]
	async fn test_reorder_all(_init_drivers: ()) {
		let pool = create_test_pool().await;
		let ordered = OrderedModel::new(
			"order".to_string(),
			vec!["parent_id".to_string()],
			"test_table".to_string(),
			pool,
			DbBackend::Sqlite,
		);

		let mut filters = HashMap::new();
		filters.insert("parent_id".to_string(), OrderValue::Integer(1));

		let result = ordered.reorder_all(filters).await.unwrap();
		assert_eq!(result.len(), 0);
	}

	#[test]
	fn test_order_value_variants() {
		let int_value = OrderValue::Integer(42);
		let str_value = OrderValue::String("test".to_string());
		let bool_value = OrderValue::Boolean(true);

		match int_value {
			OrderValue::Integer(v) => assert_eq!(v, 42),
			_ => panic!("Expected Integer variant"),
		}

		match str_value {
			OrderValue::String(v) => assert_eq!(v, "test"),
			_ => panic!("Expected String variant"),
		}

		match bool_value {
			OrderValue::Boolean(v) => assert!(v),
			_ => panic!("Expected Boolean variant"),
		}
	}
}

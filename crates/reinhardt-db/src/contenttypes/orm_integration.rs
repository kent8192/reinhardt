//! ORM Integration Module
//!
//! This module provides integration between ContentTypes and reinhardt-orm.
//! Through integration with Session, Query, and Transaction interfaces,
//! ContentType operations through the ORM are made possible.

#[cfg(feature = "database")]
use reinhardt_query::prelude::{
	Alias, BinOper, Cond, Expr, Func, Order, Query, QueryStatementBuilder, SqliteQueryBuilder,
};
#[cfg(feature = "database")]
use reinhardt_query::value::Values;
#[cfg(feature = "database")]
use sqlx::{AnyPool, Row};
#[cfg(feature = "database")]
use std::sync::Arc;

#[cfg(feature = "database")]
use super::ContentType;
#[cfg(feature = "database")]
use super::persistence::{PersistenceError, bind_query_values};

/// ORM-compatible ContentType query builder
///
/// Provides an API similar to reinhardt-orm's Query interface,
/// building type-safe queries for ContentType.
///
/// ## Example
///
/// ```rust,no_run
/// use reinhardt_db::contenttypes::orm_integration::ContentTypeQuery;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let pool = sqlx::AnyPool::connect("sqlite::memory:").await?;
///
/// let query = ContentTypeQuery::new(pool.into());
/// let results = query
///     .filter_app_label("auth")
///     .order_by_model()
///     .all()
///     .await?;
///
/// for ct in results {
///     println!("{}.{}", ct.app_label, ct.model);
/// }
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "database")]
#[derive(Clone)]
pub struct ContentTypeQuery {
	pool: Arc<AnyPool>,
	filters: Vec<ContentTypeFilter>,
	order_by: Vec<OrderBy>,
	limit: Option<u64>,
	offset: Option<u64>,
}

#[cfg(feature = "database")]
#[derive(Clone)]
enum ContentTypeFilter {
	AppLabel(String),
	Model(String),
	Id(i64),
}

#[cfg(feature = "database")]
#[derive(Clone)]
enum OrderBy {
	AppLabel(OrderDirection),
	Model(OrderDirection),
	Id(OrderDirection),
}

#[cfg(feature = "database")]
#[derive(Clone)]
enum OrderDirection {
	Asc,
	Desc,
}

#[cfg(feature = "database")]
impl ContentTypeQuery {
	/// Create a new query builder
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_db::contenttypes::orm_integration::ContentTypeQuery;
	/// use std::sync::Arc;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let pool = sqlx::AnyPool::connect("sqlite::memory:").await?;
	/// let query = ContentTypeQuery::new(Arc::new(pool));
	/// # Ok(())
	/// # }
	/// ```
	pub fn new(pool: Arc<AnyPool>) -> Self {
		Self {
			pool,
			filters: Vec::new(),
			order_by: Vec::new(),
			limit: None,
			offset: None,
		}
	}

	/// Filter by app_label
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_db::contenttypes::orm_integration::ContentTypeQuery;
	/// use std::sync::Arc;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let pool = sqlx::AnyPool::connect("sqlite::memory:").await?;
	/// let query = ContentTypeQuery::new(Arc::new(pool))
	///     .filter_app_label("auth");
	/// # Ok(())
	/// # }
	/// ```
	pub fn filter_app_label(mut self, app_label: impl Into<String>) -> Self {
		self.filters
			.push(ContentTypeFilter::AppLabel(app_label.into()));
		self
	}

	/// Filter by model
	pub fn filter_model(mut self, model: impl Into<String>) -> Self {
		self.filters.push(ContentTypeFilter::Model(model.into()));
		self
	}

	/// Filter by ID
	pub fn filter_id(mut self, id: i64) -> Self {
		self.filters.push(ContentTypeFilter::Id(id));
		self
	}

	/// Sort by app_label in ascending order
	pub fn order_by_app_label(mut self) -> Self {
		self.order_by.push(OrderBy::AppLabel(OrderDirection::Asc));
		self
	}

	/// Sort by model in ascending order
	pub fn order_by_model(mut self) -> Self {
		self.order_by.push(OrderBy::Model(OrderDirection::Asc));
		self
	}

	/// Sort by ID in ascending order
	pub fn order_by_id(mut self) -> Self {
		self.order_by.push(OrderBy::Id(OrderDirection::Asc));
		self
	}

	/// Sort by app_label in descending order
	pub fn order_by_app_label_desc(mut self) -> Self {
		self.order_by.push(OrderBy::AppLabel(OrderDirection::Desc));
		self
	}

	/// Sort by model in descending order
	pub fn order_by_model_desc(mut self) -> Self {
		self.order_by.push(OrderBy::Model(OrderDirection::Desc));
		self
	}

	/// Sort by ID in descending order
	pub fn order_by_id_desc(mut self) -> Self {
		self.order_by.push(OrderBy::Id(OrderDirection::Desc));
		self
	}

	/// Limit the number of results
	pub fn limit(mut self, limit: u64) -> Self {
		self.limit = Some(limit);
		self
	}

	/// Set result offset
	pub fn offset(mut self, offset: u64) -> Self {
		self.offset = Some(offset);
		self
	}

	/// Build the query, returning parameterized SQL and bound values
	fn build_query(&self) -> (String, Values) {
		let mut query = Query::select()
			.columns([
				Alias::new("id"),
				Alias::new("app_label"),
				Alias::new("model"),
			])
			.from(Alias::new("django_content_type"))
			.to_owned();

		// Apply filters
		for filter in &self.filters {
			let condition = match filter {
				ContentTypeFilter::AppLabel(app_label) => Cond::all().add(
					Expr::col(Alias::new("app_label"))
						.binary(BinOper::Equal, Expr::val(app_label.clone())),
				),
				ContentTypeFilter::Model(model) => Cond::all().add(
					Expr::col(Alias::new("model")).binary(BinOper::Equal, Expr::val(model.clone())),
				),
				ContentTypeFilter::Id(id) => Cond::all()
					.add(Expr::col(Alias::new("id")).binary(BinOper::Equal, Expr::val(*id))),
			};
			query.cond_where(condition);
		}

		// Apply sort order
		for order in &self.order_by {
			match order {
				OrderBy::AppLabel(direction) => {
					query.order_by(
						Alias::new("app_label"),
						match direction {
							OrderDirection::Asc => Order::Asc,
							OrderDirection::Desc => Order::Desc,
						},
					);
				}
				OrderBy::Model(direction) => {
					query.order_by(
						Alias::new("model"),
						match direction {
							OrderDirection::Asc => Order::Asc,
							OrderDirection::Desc => Order::Desc,
						},
					);
				}
				OrderBy::Id(direction) => {
					query.order_by(
						Alias::new("id"),
						match direction {
							OrderDirection::Asc => Order::Asc,
							OrderDirection::Desc => Order::Desc,
						},
					);
				}
			}
		}

		// Apply limit/offset
		if let Some(limit) = self.limit {
			query.limit(limit);
		}
		if let Some(offset) = self.offset {
			query.offset(offset);
		}

		query.build(SqliteQueryBuilder)
	}

	/// Retrieve all results
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_db::contenttypes::orm_integration::ContentTypeQuery;
	/// use std::sync::Arc;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let pool = sqlx::AnyPool::connect("sqlite::memory:").await?;
	/// let results = ContentTypeQuery::new(Arc::new(pool))
	///     .filter_app_label("auth")
	///     .all()
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn all(&self) -> Result<Vec<ContentType>, PersistenceError> {
		let (sql, values) = self.build_query();
		let sql_leaked: &'static str = Box::leak(sql.into_boxed_str());
		let rows = bind_query_values(sqlx::query(sql_leaked), &values)
			.fetch_all(&*self.pool)
			.await
			.map_err(|e| {
				PersistenceError::DatabaseError(format!("Failed to execute query: {}", e))
			})?;

		let mut results = Vec::new();
		for row in rows {
			let id: i64 = row
				.try_get("id")
				.map_err(|e| PersistenceError::DatabaseError(format!("Invalid id: {}", e)))?;
			let app_label: String = row.try_get("app_label").map_err(|e| {
				PersistenceError::DatabaseError(format!("Invalid app_label: {}", e))
			})?;
			let model: String = row
				.try_get("model")
				.map_err(|e| PersistenceError::DatabaseError(format!("Invalid model: {}", e)))?;

			results.push(ContentType {
				id: Some(id),
				app_label,
				model,
			});
		}

		Ok(results)
	}

	/// Retrieve the first result
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_db::contenttypes::orm_integration::ContentTypeQuery;
	/// use std::sync::Arc;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let pool = sqlx::AnyPool::connect("sqlite::memory:").await?;
	/// let result = ContentTypeQuery::new(Arc::new(pool))
	///     .filter_app_label("auth")
	///     .filter_model("User")
	///     .first()
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn first(&self) -> Result<Option<ContentType>, PersistenceError> {
		let mut query = self.clone();
		query.limit = Some(1);

		let results = query.all().await?;
		Ok(results.into_iter().next())
	}

	/// Get the count of results
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_db::contenttypes::orm_integration::ContentTypeQuery;
	/// use std::sync::Arc;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let pool = sqlx::AnyPool::connect("sqlite::memory:").await?;
	/// let count = ContentTypeQuery::new(Arc::new(pool))
	///     .filter_app_label("auth")
	///     .count()
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn count(&self) -> Result<i64, PersistenceError> {
		let mut count_query = Query::select()
			.expr(Func::count(Expr::col(Alias::new("id")).into()))
			.from(Alias::new("django_content_type"))
			.to_owned();

		// Apply filters only (order and limit are unnecessary)
		for filter in &self.filters {
			let condition = match filter {
				ContentTypeFilter::AppLabel(app_label) => Cond::all().add(
					Expr::col(Alias::new("app_label"))
						.binary(BinOper::Equal, Expr::val(app_label.clone())),
				),
				ContentTypeFilter::Model(model) => Cond::all().add(
					Expr::col(Alias::new("model")).binary(BinOper::Equal, Expr::val(model.clone())),
				),
				ContentTypeFilter::Id(id) => Cond::all()
					.add(Expr::col(Alias::new("id")).binary(BinOper::Equal, Expr::val(*id))),
			};
			count_query.cond_where(condition);
		}

		let (sql, values) = count_query.build(SqliteQueryBuilder);
		let sql_leaked: &'static str = Box::leak(sql.into_boxed_str());
		let row = bind_query_values(sqlx::query(sql_leaked), &values)
			.fetch_one(&*self.pool)
			.await
			.map_err(|e| PersistenceError::DatabaseError(format!("Failed to count: {}", e)))?;

		let count: i64 = row
			.try_get(0)
			.map_err(|e| PersistenceError::DatabaseError(format!("Invalid count: {}", e)))?;

		Ok(count)
	}

	/// Check if results exist
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_db::contenttypes::orm_integration::ContentTypeQuery;
	/// use std::sync::Arc;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let pool = sqlx::AnyPool::connect("sqlite::memory:").await?;
	/// let exists = ContentTypeQuery::new(Arc::new(pool))
	///     .filter_app_label("auth")
	///     .exists()
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn exists(&self) -> Result<bool, PersistenceError> {
		let count = self.count().await?;
		Ok(count > 0)
	}
}

/// ContentType operations with transaction support
///
/// Enables executing ContentType operations within transactions
/// through integration with ORM Transaction.
#[cfg(feature = "database")]
pub struct ContentTypeTransaction {
	pool: Arc<AnyPool>,
}

#[cfg(feature = "database")]
impl ContentTypeTransaction {
	/// Create a new transaction context
	pub fn new(pool: Arc<AnyPool>) -> Self {
		Self { pool }
	}

	/// Get query builder
	pub fn query(&self) -> ContentTypeQuery {
		ContentTypeQuery::new(self.pool.clone())
	}

	/// Create ContentType (within transaction)
	pub async fn create(
		&self,
		app_label: impl Into<String>,
		model: impl Into<String>,
	) -> Result<ContentType, PersistenceError> {
		let app_label = app_label.into();
		let model = model.into();

		let stmt = Query::insert()
			.into_table(Alias::new("django_content_type"))
			.columns([Alias::new("app_label"), Alias::new("model")])
			.values(vec![app_label.clone().into(), model.clone().into()])
			.expect("Failed to build insert statement")
			.to_owned();
		let (sql, values) = stmt.build(SqliteQueryBuilder);
		let sql_leaked: &'static str = Box::leak(sql.into_boxed_str());

		bind_query_values(sqlx::query(sql_leaked), &values)
			.execute(&*self.pool)
			.await
			.map_err(|e| {
				PersistenceError::DatabaseError(format!("Failed to create content type: {}", e))
			})?;

		// Get the last inserted ID using SQLite's last_insert_rowid()
		let id_row = sqlx::query("SELECT last_insert_rowid() as id")
			.fetch_one(&*self.pool)
			.await
			.map_err(|e| {
				PersistenceError::DatabaseError(format!("Failed to get last insert ID: {}", e))
			})?;

		let id: i64 = id_row
			.try_get("id")
			.map_err(|e| PersistenceError::DatabaseError(format!("Failed to extract ID: {}", e)))?;

		Ok(ContentType {
			id: Some(id),
			app_label,
			model,
		})
	}

	/// Delete ContentType (within transaction)
	pub async fn delete(&self, id: i64) -> Result<(), PersistenceError> {
		let stmt = Query::delete()
			.from_table(Alias::new("django_content_type"))
			.cond_where(
				Cond::all().add(Expr::col(Alias::new("id")).binary(BinOper::Equal, Expr::val(id))),
			)
			.to_owned();
		let (sql, values) = stmt.build(SqliteQueryBuilder);
		let sql_leaked: &'static str = Box::leak(sql.into_boxed_str());

		bind_query_values(sqlx::query(sql_leaked), &values)
			.execute(&*self.pool)
			.await
			.map_err(|e| {
				PersistenceError::DatabaseError(format!("Failed to delete content type: {}", e))
			})?;

		Ok(())
	}
}

#[cfg(all(test, feature = "database"))]
mod tests {
	use super::*;
	use crate::contenttypes::persistence::ContentTypePersistence;
	use std::sync::Once;

	static INIT_DRIVERS: Once = Once::new();

	fn init_drivers() {
		INIT_DRIVERS.call_once(|| {
			sqlx::any::install_default_drivers();
		});
	}

	async fn setup_test_db() -> Arc<AnyPool> {
		init_drivers();

		// Use in-memory SQLite with shared cache mode and single connection
		let db_url = "sqlite::memory:?mode=rwc&cache=shared";

		// Create pool with single connection
		use sqlx::pool::PoolOptions;
		let pool = PoolOptions::new()
			.min_connections(1)
			.max_connections(1)
			.connect(db_url)
			.await
			.expect("Failed to connect");

		// Create table
		let persistence = ContentTypePersistence::from_pool(pool.clone().into(), db_url);
		persistence
			.create_table()
			.await
			.expect("Failed to create table");

		pool.into()
	}

	#[tokio::test]
	async fn test_content_type_query_all() {
		let pool = setup_test_db().await;

		// Create test data
		let tx = ContentTypeTransaction::new(pool.clone());
		tx.create("auth", "User").await.expect("Failed to create");
		tx.create("auth", "Group").await.expect("Failed to create");

		// Execute query
		let query = ContentTypeQuery::new(pool);
		let results = query.all().await.expect("Failed to execute query");

		assert_eq!(results.len(), 2);
	}

	#[tokio::test]
	async fn test_content_type_query_filter() {
		let pool = setup_test_db().await;

		// Create test data
		let tx = ContentTypeTransaction::new(pool.clone());
		tx.create("auth", "User").await.expect("Failed to create");
		tx.create("blog", "Post").await.expect("Failed to create");

		// Filter query
		let query = ContentTypeQuery::new(pool);
		let results = query
			.filter_app_label("auth")
			.all()
			.await
			.expect("Failed to execute query");

		assert_eq!(results.len(), 1);
		assert_eq!(results[0].app_label, "auth");
	}

	#[tokio::test]
	async fn test_content_type_query_order_by() {
		let pool = setup_test_db().await;

		// Create test data
		let tx = ContentTypeTransaction::new(pool.clone());
		tx.create("blog", "Post").await.expect("Failed to create");
		tx.create("auth", "User").await.expect("Failed to create");

		// Query with sorting
		let query = ContentTypeQuery::new(pool);
		let results = query
			.order_by_app_label()
			.all()
			.await
			.expect("Failed to execute query");

		assert_eq!(results.len(), 2);
		assert_eq!(results[0].app_label, "auth");
		assert_eq!(results[1].app_label, "blog");
	}

	#[tokio::test]
	async fn test_content_type_query_limit_offset() {
		let pool = setup_test_db().await;

		// Create test data
		let tx = ContentTypeTransaction::new(pool.clone());
		tx.create("app1", "Model1").await.expect("Failed to create");
		tx.create("app2", "Model2").await.expect("Failed to create");
		tx.create("app3", "Model3").await.expect("Failed to create");

		// Query with limit/offset
		let query = ContentTypeQuery::new(pool);
		let results = query
			.order_by_id()
			.limit(2)
			.offset(1)
			.all()
			.await
			.expect("Failed to execute query");

		assert_eq!(results.len(), 2);
	}

	#[tokio::test]
	async fn test_content_type_query_first() {
		let pool = setup_test_db().await;

		// Create test data
		let tx = ContentTypeTransaction::new(pool.clone());
		tx.create("auth", "User").await.expect("Failed to create");

		// first()
		let query = ContentTypeQuery::new(pool);
		let result = query
			.filter_app_label("auth")
			.first()
			.await
			.expect("Failed to execute query");

		assert!(result.is_some());
		assert_eq!(result.unwrap().model, "User");
	}

	#[tokio::test]
	async fn test_content_type_query_count() {
		let pool = setup_test_db().await;

		// Create test data
		let tx = ContentTypeTransaction::new(pool.clone());
		tx.create("auth", "User").await.expect("Failed to create");
		tx.create("auth", "Group").await.expect("Failed to create");
		tx.create("blog", "Post").await.expect("Failed to create");

		// count()
		let query = ContentTypeQuery::new(pool);
		let count = query
			.filter_app_label("auth")
			.count()
			.await
			.expect("Failed to count");

		assert_eq!(count, 2);
	}

	#[tokio::test]
	async fn test_content_type_query_exists() {
		let pool = setup_test_db().await;

		// Create test data
		let tx = ContentTypeTransaction::new(pool.clone());
		tx.create("auth", "User").await.expect("Failed to create");

		// exists()
		let query = ContentTypeQuery::new(pool.clone());
		let exists = query
			.filter_app_label("auth")
			.exists()
			.await
			.expect("Failed to check existence");

		assert!(exists);

		// Non-existent case
		let query2 = ContentTypeQuery::new(pool);
		let not_exists = query2
			.filter_app_label("nonexistent")
			.exists()
			.await
			.expect("Failed to check existence");

		assert!(!not_exists);
	}

	#[tokio::test]
	async fn test_content_type_transaction_create() {
		let pool = setup_test_db().await;

		let tx = ContentTypeTransaction::new(pool.clone());
		let ct = tx
			.create("shop", "Product")
			.await
			.expect("Failed to create");

		assert_eq!(ct.app_label, "shop");
		assert_eq!(ct.model, "Product");
		assert!(ct.id.is_some());
	}

	#[tokio::test]
	async fn test_content_type_transaction_delete() {
		let pool = setup_test_db().await;

		let tx = ContentTypeTransaction::new(pool.clone());
		let ct = tx.create("temp", "Model").await.expect("Failed to create");
		let id = ct.id.unwrap();

		// Delete
		tx.delete(id).await.expect("Failed to delete");

		// Verify deletion
		let query = ContentTypeQuery::new(pool);
		let result = query.filter_id(id).first().await.expect("Failed to query");

		assert!(result.is_none());
	}

	#[tokio::test]
	async fn test_content_type_query_multiple_filters() {
		let pool = setup_test_db().await;

		let tx = ContentTypeTransaction::new(pool.clone());
		tx.create("auth", "User").await.expect("Failed to create");
		tx.create("auth", "Group").await.expect("Failed to create");

		// Multiple filters
		let query = ContentTypeQuery::new(pool);
		let results = query
			.filter_app_label("auth")
			.filter_model("User")
			.all()
			.await
			.expect("Failed to execute query");

		assert_eq!(results.len(), 1);
		assert_eq!(results[0].model, "User");
	}

	#[tokio::test]
	async fn test_content_type_query_order_desc() {
		let pool = setup_test_db().await;

		let tx = ContentTypeTransaction::new(pool.clone());
		tx.create("app1", "Model1").await.expect("Failed to create");
		tx.create("app2", "Model2").await.expect("Failed to create");

		// Descending sort
		let query = ContentTypeQuery::new(pool);
		let results = query
			.order_by_app_label_desc()
			.all()
			.await
			.expect("Failed to execute query");

		assert_eq!(results[0].app_label, "app2");
		assert_eq!(results[1].app_label, "app1");
	}
}

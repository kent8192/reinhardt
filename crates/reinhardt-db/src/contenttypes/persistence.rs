//! Database persistence for ContentType
//!
//! This module provides database-backed persistence for ContentType instances,
//! similar to Django's contenttypes framework.
//!
//! ## Features
//!
//! - Persistent ContentType storage in database
//! - Automatic table creation and migration
//! - Database-backed content type lookups
//! - Synchronization between in-memory registry and database
//!
//! ## Example
//!
//! ```rust,ignore
//! // This example requires the "database" feature
//! use reinhardt_db::contenttypes::persistence::ContentTypePersistence;
//! use reinhardt_db::contenttypes::ContentType;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Initialize persistence backend
//! let persistence = ContentTypePersistence::new("sqlite::memory:").await?;
//!
//! // Create table
//! persistence.create_table().await?;
//!
//! // Get or create content type
//! let ct = persistence.get_or_create("blog", "Post").await?;
//! println!("ContentType ID: {:?}", ct.id);
//!
//! // Load all content types
//! let all_cts = persistence.load_all().await?;
//! println!("Total content types: {}", all_cts.len());
//! # Ok(())
//! # }
//! ```

use serde::{Deserialize, Serialize};

#[cfg(feature = "database")]
use async_trait::async_trait;
#[cfg(feature = "database")]
use reinhardt_query::prelude::{
	Alias, BinOper, ColumnDef, Cond, Expr, Order, PostgresQueryBuilder, Query,
	QueryStatementBuilder, SqliteQueryBuilder,
};
#[cfg(feature = "database")]
use reinhardt_query::value::{Value, Values};
#[cfg(feature = "database")]
use sqlx::{AnyPool, Row};
#[cfg(feature = "database")]
use std::sync::Arc;

use crate::contenttypes::ContentType;

/// Error type for persistence operations
#[non_exhaustive]
#[cfg(feature = "database")]
#[derive(Debug, thiserror::Error)]
pub enum PersistenceError {
	#[error("Database error: {0}")]
	DatabaseError(String),

	#[error("Serialization error: {0}")]
	SerializationError(String),

	#[error("Not found: {0}")]
	NotFound(String),
}

#[non_exhaustive]
#[cfg(not(feature = "database"))]
#[derive(Debug)]
pub enum PersistenceError {
	DatabaseError(String),
	SerializationError(String),
	NotFound(String),
}

#[cfg(not(feature = "database"))]
impl std::fmt::Display for PersistenceError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			PersistenceError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
			PersistenceError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
			PersistenceError::NotFound(msg) => write!(f, "Not found: {}", msg),
		}
	}
}

#[cfg(not(feature = "database"))]
impl std::error::Error for PersistenceError {}

/// Database model for ContentType
///
/// Represents a content type stored in the database.
///
/// ## Database Schema
///
/// ```sql
/// CREATE TABLE django_content_type (
///     id INTEGER PRIMARY KEY AUTOINCREMENT,
///     app_label VARCHAR(100) NOT NULL,
///     model VARCHAR(100) NOT NULL,
///     UNIQUE(app_label, model)
/// );
/// CREATE INDEX idx_content_type_app_label ON django_content_type(app_label);
/// CREATE INDEX idx_content_type_model ON django_content_type(model);
/// ```
///
/// ## Example
///
/// ```rust
/// use reinhardt_db::contenttypes::persistence::ContentTypeModel;
///
/// let ct_model = ContentTypeModel {
///     id: Some(1),
///     app_label: "blog".to_string(),
///     model: "Post".to_string(),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContentTypeModel {
	pub id: Option<i64>,
	pub app_label: String,
	pub model: String,
}

impl From<ContentType> for ContentTypeModel {
	fn from(ct: ContentType) -> Self {
		Self {
			id: ct.id,
			app_label: ct.app_label,
			model: ct.model,
		}
	}
}

impl From<ContentTypeModel> for ContentType {
	fn from(model: ContentTypeModel) -> Self {
		ContentType {
			id: model.id,
			app_label: model.app_label,
			model: model.model,
		}
	}
}

/// Trait for ContentType persistence operations
#[cfg(feature = "database")]
#[async_trait]
pub trait ContentTypePersistenceBackend: Send + Sync {
	/// Get a content type by app label and model name
	async fn get(
		&self,
		app_label: &str,
		model: &str,
	) -> Result<Option<ContentType>, PersistenceError>;

	/// Get a content type by ID
	async fn get_by_id(&self, id: i64) -> Result<Option<ContentType>, PersistenceError>;

	/// Get or create a content type
	async fn get_or_create(
		&self,
		app_label: &str,
		model: &str,
	) -> Result<ContentType, PersistenceError>;

	/// Load all content types from database
	async fn load_all(&self) -> Result<Vec<ContentType>, PersistenceError>;

	/// Save a content type
	async fn save(&self, ct: &ContentType) -> Result<ContentType, PersistenceError>;

	/// Delete a content type
	async fn delete(&self, id: i64) -> Result<(), PersistenceError>;

	/// Check if a content type exists
	async fn exists(&self, app_label: &str, model: &str) -> Result<bool, PersistenceError>;
}

/// Database-backed ContentType persistence
///
/// Provides persistent storage for ContentType instances using a database backend.
/// Supports PostgreSQL, MySQL, and SQLite through sqlx's `Any` driver.
///
/// ## Example
///
/// ```rust,no_run
/// use reinhardt_db::contenttypes::persistence::{ContentTypePersistence, ContentTypePersistenceBackend};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Initialize with database URL
/// let persistence = ContentTypePersistence::new("sqlite::memory:").await?;
///
/// // Create the django_content_type table
/// persistence.create_table().await?;
///
/// // Get or create content type
/// let ct = persistence.get_or_create("auth", "User").await?;
/// assert_eq!(ct.app_label, "auth");
/// assert_eq!(ct.model, "User");
/// assert!(ct.id.is_some());
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "database")]
#[derive(Clone)]
pub struct ContentTypePersistence {
	pool: Arc<AnyPool>,
	database_url: String,
}

#[cfg(feature = "database")]
impl ContentTypePersistence {
	/// Create a new ContentType persistence backend
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_db::contenttypes::persistence::ContentTypePersistence;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // PostgreSQL
	/// let persistence = ContentTypePersistence::new("postgres://localhost/mydb").await?;
	///
	/// // MySQL
	/// let persistence = ContentTypePersistence::new("mysql://localhost/mydb").await?;
	///
	/// // SQLite
	/// let persistence = ContentTypePersistence::new("sqlite::memory:").await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn new(database_url: &str) -> Result<Self, PersistenceError> {
		use sqlx::pool::PoolOptions;

		// For in-memory SQLite with shared cache, use single connection pool
		// to ensure all operations see the same database
		let (min_conn, max_conn) =
			if database_url.contains(":memory:") && database_url.contains("cache=shared") {
				(1, 1)
			} else {
				(0, 5)
			};

		let pool = PoolOptions::new()
			.min_connections(min_conn)
			.max_connections(max_conn)
			.connect(database_url)
			.await
			.map_err(|e| {
				PersistenceError::DatabaseError(format!("Database connection error: {}", e))
			})?;

		Ok(Self {
			pool: Arc::new(pool),
			database_url: database_url.to_string(),
		})
	}

	/// Create a new backend from an existing pool
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_db::contenttypes::persistence::ContentTypePersistence;
	/// use sqlx::AnyPool;
	/// use std::sync::Arc;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let database_url = "sqlite::memory:";
	/// let pool = AnyPool::connect(database_url).await?;
	/// let persistence = ContentTypePersistence::from_pool(Arc::new(pool), database_url);
	/// # Ok(())
	/// # }
	/// ```
	pub fn from_pool(pool: Arc<AnyPool>, database_url: &str) -> Self {
		Self {
			pool,
			database_url: database_url.to_string(),
		}
	}

	/// Create the django_content_type table if it doesn't exist
	///
	/// Creates the required database table for ContentType storage following
	/// Django's schema conventions.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_db::contenttypes::persistence::ContentTypePersistence;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let persistence = ContentTypePersistence::new("sqlite::memory:").await?;
	/// persistence.create_table().await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn create_table(&self) -> Result<(), PersistenceError> {
		// Get a single connection from the pool to ensure all operations use the same connection
		let mut conn = self.pool.acquire().await.map_err(|e| {
			PersistenceError::DatabaseError(format!("Failed to acquire connection: {}", e))
		})?;

		let stmt = Query::create_table()
			.table(Alias::new("django_content_type"))
			.if_not_exists()
			.col(
				ColumnDef::new("id")
					.integer()
					.not_null(true)
					.auto_increment(true)
					.primary_key(true),
			)
			.col(ColumnDef::new("app_label").string_len(100).not_null(true))
			.col(ColumnDef::new("model").string_len(100).not_null(true))
			.to_owned();

		// Select appropriate QueryBuilder based on database URL
		let sql = if self.database_url.starts_with("postgres") {
			stmt.to_string(PostgresQueryBuilder)
		} else {
			stmt.to_string(SqliteQueryBuilder)
		};
		let sql_leaked: &'static str = Box::leak(sql.into_boxed_str());

		sqlx::query(sql_leaked)
			.execute(&mut *conn)
			.await
			.map_err(|e| {
				PersistenceError::DatabaseError(format!("Failed to create table: {}", e))
			})?;

		// Create unique index on (app_label, model)
		let idx = Query::create_index()
			.if_not_exists()
			.unique()
			.name("django_content_type_app_label_model_unique")
			.table(Alias::new("django_content_type"))
			.col(Alias::new("app_label"))
			.col(Alias::new("model"))
			.to_owned();
		// Use same QueryBuilder as table creation
		let sql = if self.database_url.starts_with("postgres") {
			idx.to_string(PostgresQueryBuilder)
		} else {
			idx.to_string(SqliteQueryBuilder)
		};
		let sql_leaked: &'static str = Box::leak(sql.into_boxed_str());

		sqlx::query(sql_leaked)
			.execute(&mut *conn)
			.await
			.map_err(|e| {
				PersistenceError::DatabaseError(format!("Failed to create unique index: {}", e))
			})?;

		// Create index on app_label
		let idx = Query::create_index()
			.if_not_exists()
			.name("idx_content_type_app_label")
			.table(Alias::new("django_content_type"))
			.col(Alias::new("app_label"))
			.to_owned();
		// Use same QueryBuilder as table creation
		let sql = if self.database_url.starts_with("postgres") {
			idx.to_string(PostgresQueryBuilder)
		} else {
			idx.to_string(SqliteQueryBuilder)
		};
		let sql_leaked: &'static str = Box::leak(sql.into_boxed_str());

		sqlx::query(sql_leaked)
			.execute(&mut *conn)
			.await
			.map_err(|e| {
				PersistenceError::DatabaseError(format!("Failed to create app_label index: {}", e))
			})?;

		// Create index on model
		let idx = Query::create_index()
			.if_not_exists()
			.name("idx_content_type_model")
			.table(Alias::new("django_content_type"))
			.col(Alias::new("model"))
			.to_owned();
		// Use same QueryBuilder as table creation
		let sql = if self.database_url.starts_with("postgres") {
			idx.to_string(PostgresQueryBuilder)
		} else {
			idx.to_string(SqliteQueryBuilder)
		};
		let sql_leaked: &'static str = Box::leak(sql.into_boxed_str());

		sqlx::query(sql_leaked)
			.execute(&mut *conn)
			.await
			.map_err(|e| {
				PersistenceError::DatabaseError(format!("Failed to create model index: {}", e))
			})?;

		Ok(())
	}

	/// Helper method to check if database is PostgreSQL
	fn is_postgres(&self) -> bool {
		self.database_url.starts_with("postgres")
	}

	/// Helper method to build parameterized SQL with appropriate QueryBuilder
	///
	/// Returns a tuple of (SQL string with placeholders, bound values).
	fn build_sql_with_values<T>(&self, builder: T) -> (String, Values)
	where
		T: QueryStatementBuilder,
	{
		if self.is_postgres() {
			builder.build(PostgresQueryBuilder)
		} else {
			builder.build(SqliteQueryBuilder)
		}
	}
}

/// Bind a reinhardt-query Value to a sqlx Any query
#[cfg(feature = "database")]
pub(crate) fn bind_query_value<'a>(
	query: sqlx::query::Query<'a, sqlx::Any, sqlx::any::AnyArguments<'a>>,
	value: &Value,
) -> sqlx::query::Query<'a, sqlx::Any, sqlx::any::AnyArguments<'a>> {
	match value {
		Value::Bool(Some(b)) => query.bind(*b),
		Value::TinyInt(Some(i)) => query.bind(*i as i32),
		Value::SmallInt(Some(i)) => query.bind(*i as i32),
		Value::Int(Some(i)) => query.bind(*i),
		Value::BigInt(Some(i)) => query.bind(*i),
		Value::TinyUnsigned(Some(i)) => query.bind(*i as i32),
		Value::SmallUnsigned(Some(i)) => query.bind(*i as i32),
		Value::Unsigned(Some(i)) => query.bind(*i as i64),
		Value::BigUnsigned(Some(i)) => query.bind(*i as i64),
		Value::Float(Some(f)) => query.bind(*f),
		Value::Double(Some(f)) => query.bind(*f),
		Value::String(Some(s)) => query.bind(s.as_ref().clone()),
		Value::Bytes(Some(b)) => query.bind(b.as_ref().clone()),
		_ => query.bind(None::<i32>), // NULL values
	}
}

/// Bind all values from a reinhardt-query Values collection to a sqlx Any query
#[cfg(feature = "database")]
pub(crate) fn bind_query_values<'a>(
	mut query: sqlx::query::Query<'a, sqlx::Any, sqlx::any::AnyArguments<'a>>,
	values: &Values,
) -> sqlx::query::Query<'a, sqlx::Any, sqlx::any::AnyArguments<'a>> {
	for value in values.iter() {
		query = bind_query_value(query, value);
	}
	query
}

#[cfg(feature = "database")]
#[async_trait]
impl ContentTypePersistenceBackend for ContentTypePersistence {
	async fn get(
		&self,
		app_label: &str,
		model: &str,
	) -> Result<Option<ContentType>, PersistenceError> {
		let stmt = Query::select()
			.columns([
				Alias::new("id"),
				Alias::new("app_label"),
				Alias::new("model"),
			])
			.from(Alias::new("django_content_type"))
			.cond_where(Cond::all().add(
				Expr::col(Alias::new("app_label")).binary(BinOper::Equal, Expr::val(app_label)),
			))
			.cond_where(
				Cond::all()
					.add(Expr::col(Alias::new("model")).binary(BinOper::Equal, Expr::val(model))),
			)
			.to_owned();
		let (sql, values) = self.build_sql_with_values(stmt);
		let sql_leaked: &'static str = Box::leak(sql.into_boxed_str());

		let row = bind_query_values(sqlx::query(sql_leaked), &values)
			.fetch_optional(&*self.pool)
			.await
			.map_err(|e| {
				PersistenceError::DatabaseError(format!("Failed to get content type: {}", e))
			})?;

		match row {
			Some(row) => {
				let id: i64 = row
					.try_get("id")
					.map_err(|e| PersistenceError::DatabaseError(format!("Invalid id: {}", e)))?;
				let app_label: String = row.try_get("app_label").map_err(|e| {
					PersistenceError::DatabaseError(format!("Invalid app_label: {}", e))
				})?;
				let model: String = row.try_get("model").map_err(|e| {
					PersistenceError::DatabaseError(format!("Invalid model: {}", e))
				})?;

				Ok(Some(ContentType {
					id: Some(id),
					app_label,
					model,
				}))
			}
			None => Ok(None),
		}
	}

	async fn get_by_id(&self, id: i64) -> Result<Option<ContentType>, PersistenceError> {
		let stmt = Query::select()
			.columns([
				Alias::new("id"),
				Alias::new("app_label"),
				Alias::new("model"),
			])
			.from(Alias::new("django_content_type"))
			.cond_where(
				Cond::all().add(Expr::col(Alias::new("id")).binary(BinOper::Equal, Expr::val(id))),
			)
			.to_owned();
		let (sql, values) = self.build_sql_with_values(stmt);
		let sql_leaked: &'static str = Box::leak(sql.into_boxed_str());

		let row = bind_query_values(sqlx::query(sql_leaked), &values)
			.fetch_optional(&*self.pool)
			.await
			.map_err(|e| {
				PersistenceError::DatabaseError(format!("Failed to get content type by id: {}", e))
			})?;

		match row {
			Some(row) => {
				let id: i64 = row
					.try_get("id")
					.map_err(|e| PersistenceError::DatabaseError(format!("Invalid id: {}", e)))?;
				let app_label: String = row.try_get("app_label").map_err(|e| {
					PersistenceError::DatabaseError(format!("Invalid app_label: {}", e))
				})?;
				let model: String = row.try_get("model").map_err(|e| {
					PersistenceError::DatabaseError(format!("Invalid model: {}", e))
				})?;

				Ok(Some(ContentType {
					id: Some(id),
					app_label,
					model,
				}))
			}
			None => Ok(None),
		}
	}

	async fn get_or_create(
		&self,
		app_label: &str,
		model: &str,
	) -> Result<ContentType, PersistenceError> {
		// Try to get existing
		if let Some(ct) = self.get(app_label, model).await? {
			return Ok(ct);
		}

		// Create new
		let ct = ContentType::new(app_label, model);
		self.save(&ct).await
	}

	async fn load_all(&self) -> Result<Vec<ContentType>, PersistenceError> {
		let stmt = Query::select()
			.columns([
				Alias::new("id"),
				Alias::new("app_label"),
				Alias::new("model"),
			])
			.from(Alias::new("django_content_type"))
			.order_by(Alias::new("app_label"), Order::Asc)
			.order_by(Alias::new("model"), Order::Asc)
			.to_owned();
		let (sql, values) = self.build_sql_with_values(stmt);
		let sql_leaked: &'static str = Box::leak(sql.into_boxed_str());

		let rows = bind_query_values(sqlx::query(sql_leaked), &values)
			.fetch_all(&*self.pool)
			.await
			.map_err(|e| {
				PersistenceError::DatabaseError(format!("Failed to load all content types: {}", e))
			})?;

		let mut content_types = Vec::new();
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

			content_types.push(ContentType {
				id: Some(id),
				app_label,
				model,
			});
		}

		Ok(content_types)
	}

	async fn save(&self, ct: &ContentType) -> Result<ContentType, PersistenceError> {
		if let Some(id) = ct.id {
			// Update existing
			let stmt = Query::update()
				.table(Alias::new("django_content_type"))
				.value(Alias::new("app_label"), ct.app_label.clone())
				.value(Alias::new("model"), ct.model.clone())
				.cond_where(
					Cond::all()
						.add(Expr::col(Alias::new("id")).binary(BinOper::Equal, Expr::val(id))),
				)
				.to_owned();
			let (sql, values) = self.build_sql_with_values(stmt);
			let sql_leaked: &'static str = Box::leak(sql.into_boxed_str());

			bind_query_values(sqlx::query(sql_leaked), &values)
				.execute(&*self.pool)
				.await
				.map_err(|e| {
					PersistenceError::DatabaseError(format!("Failed to update content type: {}", e))
				})?;

			Ok(ct.clone())
		} else {
			// Insert new - handle PostgreSQL RETURNING vs SQLite last_insert_rowid()
			if self.is_postgres() {
				// PostgreSQL: Use RETURNING clause
				let stmt = Query::insert()
					.into_table(Alias::new("django_content_type"))
					.columns([Alias::new("app_label"), Alias::new("model")])
					.values(vec![ct.app_label.clone().into(), ct.model.clone().into()])
					.expect("Failed to build insert statement")
					.returning([Alias::new("id")])
					.to_owned();
				let (sql, values) = self.build_sql_with_values(stmt);
				let sql_leaked: &'static str = Box::leak(sql.into_boxed_str());

				let id_row = bind_query_values(sqlx::query(sql_leaked), &values)
					.fetch_one(&*self.pool)
					.await
					.map_err(|e| {
						PersistenceError::DatabaseError(format!(
							"Failed to insert content type: {}",
							e
						))
					})?;

				let id: i64 = id_row.try_get("id").map_err(|e| {
					PersistenceError::DatabaseError(format!("Failed to extract ID: {}", e))
				})?;

				Ok(ContentType {
					id: Some(id),
					app_label: ct.app_label.clone(),
					model: ct.model.clone(),
				})
			} else {
				// SQLite: Use last_insert_rowid()
				let stmt = Query::insert()
					.into_table(Alias::new("django_content_type"))
					.columns([Alias::new("app_label"), Alias::new("model")])
					.values(vec![ct.app_label.clone().into(), ct.model.clone().into()])
					.expect("Failed to build insert statement")
					.to_owned();
				let (sql, values) = self.build_sql_with_values(stmt);
				let sql_leaked: &'static str = Box::leak(sql.into_boxed_str());

				bind_query_values(sqlx::query(sql_leaked), &values)
					.execute(&*self.pool)
					.await
					.map_err(|e| {
						PersistenceError::DatabaseError(format!(
							"Failed to insert content type: {}",
							e
						))
					})?;

				// Get the last inserted ID using SQLite's last_insert_rowid()
				let id_row = sqlx::query("SELECT last_insert_rowid() as id")
					.fetch_one(&*self.pool)
					.await
					.map_err(|e| {
						PersistenceError::DatabaseError(format!(
							"Failed to get last insert ID: {}",
							e
						))
					})?;

				let id: i64 = id_row.try_get("id").map_err(|e| {
					PersistenceError::DatabaseError(format!("Failed to extract ID: {}", e))
				})?;

				Ok(ContentType {
					id: Some(id),
					app_label: ct.app_label.clone(),
					model: ct.model.clone(),
				})
			}
		}
	}

	async fn delete(&self, id: i64) -> Result<(), PersistenceError> {
		let stmt = Query::delete()
			.from_table(Alias::new("django_content_type"))
			.cond_where(
				Cond::all().add(Expr::col(Alias::new("id")).binary(BinOper::Equal, Expr::val(id))),
			)
			.to_owned();
		let (sql, values) = self.build_sql_with_values(stmt);
		let sql_leaked: &'static str = Box::leak(sql.into_boxed_str());

		bind_query_values(sqlx::query(sql_leaked), &values)
			.execute(&*self.pool)
			.await
			.map_err(|e| {
				PersistenceError::DatabaseError(format!("Failed to delete content type: {}", e))
			})?;

		Ok(())
	}

	async fn exists(&self, app_label: &str, model: &str) -> Result<bool, PersistenceError> {
		let stmt = Query::select()
			.expr(Expr::val(1))
			.from(Alias::new("django_content_type"))
			.cond_where(Cond::all().add(
				Expr::col(Alias::new("app_label")).binary(BinOper::Equal, Expr::val(app_label)),
			))
			.cond_where(
				Cond::all()
					.add(Expr::col(Alias::new("model")).binary(BinOper::Equal, Expr::val(model))),
			)
			.to_owned();
		let (sql, values) = self.build_sql_with_values(stmt);
		let sql_leaked: &'static str = Box::leak(sql.into_boxed_str());

		let row = bind_query_values(sqlx::query(sql_leaked), &values)
			.fetch_optional(&*self.pool)
			.await
			.map_err(|e| {
				PersistenceError::DatabaseError(format!(
					"Failed to check content type existence: {}",
					e
				))
			})?;

		Ok(row.is_some())
	}
}

#[cfg(all(test, feature = "database"))]
mod tests {
	use super::*;
	use std::sync::Once;

	static INIT_DRIVERS: Once = Once::new();

	fn init_drivers() {
		INIT_DRIVERS.call_once(|| {
			sqlx::any::install_default_drivers();
		});
	}

	async fn create_test_persistence() -> ContentTypePersistence {
		init_drivers();

		// Use in-memory SQLite with shared cache mode
		// This allows multiple connections from the pool to share the same in-memory database
		let db_url = "sqlite::memory:?mode=rwc&cache=shared";

		// Create persistence with minimal connection pool for tests
		use sqlx::pool::PoolOptions;
		let pool = PoolOptions::new()
			.min_connections(1)
			.max_connections(1)
			.connect(db_url)
			.await
			.expect("Failed to connect to test database");

		let persistence = ContentTypePersistence::from_pool(Arc::new(pool), db_url);

		persistence
			.create_table()
			.await
			.expect("Failed to create table");
		persistence
	}

	#[tokio::test]
	async fn test_create_table() {
		init_drivers();
		let persistence = ContentTypePersistence::new("sqlite::memory:?cache=shared")
			.await
			.expect("Failed to create persistence");

		// Should not fail
		persistence
			.create_table()
			.await
			.expect("Failed to create table");

		// Should not fail on second call (IF NOT EXISTS)
		persistence
			.create_table()
			.await
			.expect("Failed to create table second time");
	}

	#[tokio::test]
	async fn test_save_and_get() {
		let persistence = create_test_persistence().await;

		let ct = ContentType::new("blog", "Post");
		let saved = persistence.save(&ct).await.expect("Failed to save");

		assert!(saved.id.is_some());
		assert_eq!(saved.app_label, "blog");
		assert_eq!(saved.model, "Post");

		let loaded = persistence
			.get("blog", "Post")
			.await
			.expect("Failed to get")
			.expect("ContentType not found");

		assert_eq!(loaded.id, saved.id);
		assert_eq!(loaded.app_label, "blog");
		assert_eq!(loaded.model, "Post");
	}

	#[tokio::test]
	async fn test_get_by_id() {
		let persistence = create_test_persistence().await;

		let ct = ContentType::new("auth", "User");
		let saved = persistence.save(&ct).await.expect("Failed to save");
		let id = saved.id.unwrap();

		let loaded = persistence
			.get_by_id(id)
			.await
			.expect("Failed to get by id")
			.expect("ContentType not found");

		assert_eq!(loaded.id, Some(id));
		assert_eq!(loaded.app_label, "auth");
		assert_eq!(loaded.model, "User");
	}

	#[tokio::test]
	async fn test_get_or_create() {
		let persistence = create_test_persistence().await;

		// First call should create
		let ct1 = persistence
			.get_or_create("app1", "Model1")
			.await
			.expect("Failed to get_or_create");
		assert!(ct1.id.is_some());

		// Second call should return existing
		let ct2 = persistence
			.get_or_create("app1", "Model1")
			.await
			.expect("Failed to get_or_create");
		assert_eq!(ct1.id, ct2.id);
	}

	#[tokio::test]
	async fn test_load_all() {
		let persistence = create_test_persistence().await;

		// Create multiple content types
		persistence
			.save(&ContentType::new("app1", "Model1"))
			.await
			.expect("Failed to save");
		persistence
			.save(&ContentType::new("app2", "Model2"))
			.await
			.expect("Failed to save");
		persistence
			.save(&ContentType::new("app3", "Model3"))
			.await
			.expect("Failed to save");

		let all = persistence.load_all().await.expect("Failed to load all");
		assert_eq!(all.len(), 3);

		// Should be ordered by app_label, model
		assert_eq!(all[0].app_label, "app1");
		assert_eq!(all[1].app_label, "app2");
		assert_eq!(all[2].app_label, "app3");
	}

	#[tokio::test]
	async fn test_exists() {
		let persistence = create_test_persistence().await;

		// Should not exist initially
		let exists = persistence
			.exists("test", "Model")
			.await
			.expect("Failed to check existence");
		assert!(!exists);

		// Create content type
		persistence
			.save(&ContentType::new("test", "Model"))
			.await
			.expect("Failed to save");

		// Should now exist
		let exists = persistence
			.exists("test", "Model")
			.await
			.expect("Failed to check existence");
		assert!(exists);
	}

	#[tokio::test]
	async fn test_delete() {
		let persistence = create_test_persistence().await;

		let ct = persistence
			.save(&ContentType::new("deleteme", "Model"))
			.await
			.expect("Failed to save");
		let id = ct.id.unwrap();

		// Verify it exists
		assert!(
			persistence
				.get_by_id(id)
				.await
				.expect("Failed to get")
				.is_some()
		);

		// Delete it
		persistence.delete(id).await.expect("Failed to delete");

		// Verify it's gone
		assert!(
			persistence
				.get_by_id(id)
				.await
				.expect("Failed to get")
				.is_none()
		);
	}

	#[tokio::test]
	async fn test_unique_constraint() {
		let persistence = create_test_persistence().await;

		// Create first content type
		persistence
			.save(&ContentType::new("unique", "Model"))
			.await
			.expect("Failed to save first");

		// Try to create duplicate (should fail due to unique constraint)
		let result = persistence.save(&ContentType::new("unique", "Model")).await;

		// Should fail with database error
		assert!(result.is_err());
	}

	#[tokio::test]
	async fn test_update_existing() {
		let persistence = create_test_persistence().await;

		let ct = persistence
			.save(&ContentType::new("original", "Model"))
			.await
			.expect("Failed to save");
		let id = ct.id.unwrap();

		// Update
		let updated = ContentType {
			id: Some(id),
			app_label: "updated".to_string(),
			model: "UpdatedModel".to_string(),
		};

		persistence.save(&updated).await.expect("Failed to update");

		// Verify update
		let loaded = persistence
			.get_by_id(id)
			.await
			.expect("Failed to get")
			.expect("Not found");

		assert_eq!(loaded.app_label, "updated");
		assert_eq!(loaded.model, "UpdatedModel");
	}
}

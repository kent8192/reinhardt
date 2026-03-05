//! Database backend for dynamic settings
//!
//! This backend provides runtime configuration storage using SQL databases via sqlx.
//!
//! ## Features
//!
//! This module is only available when the `dynamic-database` feature is enabled.
//!
//! ## Database Schema
//!
//! The backend expects a table with the following structure:
//!
//! ```sql
//! CREATE TABLE settings (
//!     key VARCHAR(255) PRIMARY KEY,
//!     value TEXT NOT NULL,
//!     expire_date TIMESTAMP
//! );
//! CREATE INDEX idx_settings_expire_date ON settings(expire_date);
//! ```
//!
//! ## Example
//!
//! ```rust,no_run
//! # #[cfg(feature = "dynamic-database")]
//! # async fn example() -> Result<(), String> {
//! use reinhardt_conf::settings::backends::DatabaseBackend;
//! use serde_json::json;
//!
//! // Create backend
//! let backend = DatabaseBackend::new("postgres://user:pass@localhost/db").await?;
//!
//! // Create table
//! backend.create_table().await?;
//!
//! // Set a value with TTL
//! let value = json!({"debug": true, "port": 8080});
//! backend.set("app_config", &value, Some(3600)).await?;
//!
//! // Get the value
//! let retrieved = backend.get("app_config").await?;
//! assert!(retrieved.is_some());
//!
//! // Delete the value
//! backend.delete("app_config").await?;
//! # Ok(())
//! # }
//! ```

#[cfg(feature = "dynamic-database")]
use chrono::{DateTime, Duration, Utc};
#[cfg(feature = "dynamic-database")]
use reinhardt_query::prelude::{
	MySqlQueryBuilder, PostgresQueryBuilder, QueryStatementBuilder, SqliteQueryBuilder,
};
#[cfg(feature = "dynamic-database")]
use sqlx::{AnyPool, Row};
#[cfg(feature = "dynamic-database")]
use std::sync::Arc;

#[cfg(feature = "dynamic-database")]
use crate::settings::dynamic::{DynamicBackend, DynamicError, DynamicResult};
#[cfg(feature = "dynamic-database")]
use async_trait::async_trait;

use crate::settings::database_config::validate_database_url_scheme;

/// Database backend for runtime configuration changes
///
/// This backend allows dynamic settings to be stored in and retrieved from SQL databases,
/// enabling runtime configuration changes without application restarts.
///
/// Supports PostgreSQL, MySQL, and SQLite through sqlx.
///
/// ## Example
///
/// ```rust,no_run
/// # #[cfg(feature = "dynamic-database")]
/// # async fn example() -> Result<(), String> {
/// use reinhardt_conf::settings::backends::DatabaseBackend;
/// use serde_json::json;
///
/// let backend = DatabaseBackend::new("postgres://localhost/settings").await?;
/// backend.create_table().await?;
///
/// // Store configuration
/// backend.set("feature_flags", &json!({"new_ui": true}), None).await?;
///
/// // Retrieve configuration
/// let config = backend.get("feature_flags").await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct DatabaseBackend {
	#[cfg(feature = "dynamic-database")]
	pool: Arc<AnyPool>,
	#[cfg(feature = "dynamic-database")]
	database_url: String,
	#[cfg(not(feature = "dynamic-database"))]
	_phantom: std::marker::PhantomData<()>,
}

impl DatabaseBackend {
	/// Create a new database backend
	///
	/// Initializes a connection pool to the specified database URL.
	///
	/// ## Arguments
	///
	/// * `connection_url` - Database connection URL
	///   - PostgreSQL: `postgres://user:pass@localhost/dbname`
	///   - MySQL: `mysql://user:pass@localhost/dbname`
	///   - SQLite: `sqlite://path/to/db.sqlite`
	///
	/// ## Example
	///
	/// ```rust,no_run
	/// # #[cfg(feature = "dynamic-database")]
	/// # async fn example() -> Result<(), String> {
	/// use reinhardt_conf::settings::backends::DatabaseBackend;
	///
	/// // PostgreSQL
	/// let backend = DatabaseBackend::new("postgres://user:pass@localhost/db").await?;
	///
	/// // SQLite
	/// let backend = DatabaseBackend::new("sqlite://settings.db").await?;
	/// # Ok(())
	/// # }
	/// ```
	#[cfg(feature = "dynamic-database")]
	pub async fn new(connection_url: &str) -> Result<Self, String> {
		validate_database_url_scheme(connection_url)?;

		let pool = AnyPool::connect(connection_url)
			.await
			.map_err(|e| format!("Database connection error: {}", e))?;

		Ok(Self {
			pool: Arc::new(pool),
			database_url: connection_url.to_string(),
		})
	}

	#[cfg(not(feature = "dynamic-database"))]
	pub async fn new(_connection_url: &str) -> Result<Self, String> {
		Err("Database backend not enabled. Enable the 'dynamic-database' feature.".to_string())
	}

	/// Create a new backend from an existing connection pool
	///
	/// ## Example
	///
	/// ```rust,no_run
	/// # #[cfg(feature = "dynamic-database")]
	/// # async fn example() -> Result<(), String> {
	/// use reinhardt_conf::settings::backends::DatabaseBackend;
	/// use sqlx::AnyPool;
	/// use std::sync::Arc;
	///
	/// let pool = AnyPool::connect("sqlite::memory:").await.map_err(|e| e.to_string())?;
	/// let backend = DatabaseBackend::from_pool(Arc::new(pool), "sqlite::memory:");
	/// # Ok(())
	/// # }
	/// ```
	#[cfg(feature = "dynamic-database")]
	pub fn from_pool(pool: Arc<AnyPool>, database_url: &str) -> Self {
		Self {
			pool,
			database_url: database_url.to_string(),
		}
	}

	/// Detect database backend type from URL
	///
	/// Returns the database backend type based on the connection URL format.
	#[cfg(feature = "dynamic-database")]
	fn detect_backend(&self) -> &'static str {
		if self.database_url.starts_with("postgres://")
			|| self.database_url.starts_with("postgresql://")
		{
			"postgres"
		} else if self.database_url.starts_with("mysql://") {
			"mysql"
		} else {
			"sqlite"
		}
	}

	/// Build SQL string for the current database backend
	///
	/// Uses reinhardt-query to generate database-specific SQL syntax for queries.
	#[cfg(feature = "dynamic-database")]
	fn build_sql<T>(&self, statement: T) -> String
	where
		T: QueryStatementBuilder,
	{
		match self.detect_backend() {
			"postgres" => statement.to_string(PostgresQueryBuilder),
			"mysql" => statement.to_string(MySqlQueryBuilder),
			_ => statement.to_string(SqliteQueryBuilder),
		}
	}

	/// Build table SQL string for the current database backend
	///
	/// Uses reinhardt-query to generate database-specific SQL syntax for DDL operations.
	#[cfg(feature = "dynamic-database")]
	fn build_table_sql<T>(&self, statement: T) -> String
	where
		T: QueryStatementBuilder,
	{
		match self.detect_backend() {
			"postgres" => statement.to_string(PostgresQueryBuilder),
			"mysql" => statement.to_string(MySqlQueryBuilder),
			_ => statement.to_string(SqliteQueryBuilder),
		}
	}

	/// Build index SQL string for the current database backend
	///
	/// Uses reinhardt-query to generate database-specific SQL syntax for index creation.
	#[cfg(feature = "dynamic-database")]
	fn build_index_sql(
		&self,
		statement: &reinhardt_query::prelude::CreateIndexStatement,
	) -> String {
		match self.detect_backend() {
			"postgres" => statement.to_string(PostgresQueryBuilder),
			"mysql" => statement.to_string(MySqlQueryBuilder),
			_ => statement.to_string(SqliteQueryBuilder),
		}
	}

	/// Create the settings table if it doesn't exist
	///
	/// Creates the required database table for settings storage.
	///
	/// ## Example
	///
	/// ```rust,no_run
	/// # #[cfg(feature = "dynamic-database")]
	/// # async fn example() -> Result<(), String> {
	/// use reinhardt_conf::settings::backends::DatabaseBackend;
	///
	/// let backend = DatabaseBackend::new("sqlite::memory:").await?;
	/// backend.create_table().await?;
	/// # Ok(())
	/// # }
	/// ```
	#[cfg(feature = "dynamic-database")]
	pub async fn create_table(&self) -> Result<(), String> {
		use reinhardt_query::prelude::{Alias, ColumnDef, Query};

		// Build CREATE TABLE statement using reinhardt-query
		let stmt = Query::create_table()
			.table(Alias::new("settings"))
			.if_not_exists()
			.col(
				ColumnDef::new(Alias::new("key"))
					.string_len(255)
					.not_null(true)
					.primary_key(true),
			)
			.col(ColumnDef::new(Alias::new("value")).text().not_null(true))
			.col(ColumnDef::new(Alias::new("expire_date")).text())
			.to_owned();

		let sql = self.build_table_sql(stmt);

		sqlx::query(&sql)
			.execute(self.pool.as_ref())
			.await
			.map_err(|e| format!("Failed to create table: {}", e))?;

		// Create index on expire_date for efficient cleanup using reinhardt-query
		let index_stmt = Query::create_index()
			.if_not_exists()
			.name("idx_settings_expire_date")
			.table(Alias::new("settings"))
			.col(Alias::new("expire_date"))
			.to_owned();

		let index_sql = self.build_index_sql(&index_stmt);

		sqlx::query(&index_sql)
			.execute(self.pool.as_ref())
			.await
			.map_err(|e| format!("Failed to create index: {}", e))?;

		Ok(())
	}

	/// Get a setting value by key
	///
	/// Returns `None` if the key doesn't exist or has expired.
	///
	/// ## Example
	///
	/// ```rust,no_run
	/// # #[cfg(feature = "dynamic-database")]
	/// # async fn example() -> Result<(), String> {
	/// use reinhardt_conf::settings::backends::DatabaseBackend;
	/// use serde_json::json;
	///
	/// let backend = DatabaseBackend::new("sqlite::memory:").await?;
	/// backend.create_table().await?;
	///
	/// backend.set("key", &json!("value"), None).await?;
	/// let value = backend.get("key").await?;
	/// assert_eq!(value, Some(json!("value")));
	/// # Ok(())
	/// # }
	/// ```
	#[cfg(feature = "dynamic-database")]
	pub async fn get(&self, key: &str) -> Result<Option<serde_json::Value>, String> {
		use reinhardt_query::prelude::{Alias, Expr, ExprTrait, Query};

		// Build SELECT query using reinhardt-query
		let stmt = Query::select()
			.columns([Alias::new("value"), Alias::new("expire_date")])
			.from(Alias::new("settings"))
			.and_where(Expr::col(Alias::new("key")).eq(key))
			.to_owned();

		let sql = self.build_sql(stmt);

		let row = sqlx::query(&sql)
			.fetch_optional(self.pool.as_ref())
			.await
			.map_err(|e| format!("Failed to get setting: {}", e))?;

		match row {
			Some(row) => {
				// Check if setting has expired
				// Use index-based access for MySQL compatibility
				// MySQL TEXT columns may be returned as BLOB, so handle both String and Vec<u8>
				let expire_date_str: Option<String> = {
					if let Ok(s) = row.try_get::<String, _>(1) {
						Some(s)
					} else if let Ok(bytes) = row.try_get::<Vec<u8>, _>(1) {
						String::from_utf8(bytes).ok()
					} else {
						None
					}
				};

				if let Some(expire_date_str) = expire_date_str
					&& let Ok(expire_date) = DateTime::parse_from_rfc3339(&expire_date_str)
					&& expire_date.with_timezone(&Utc) < Utc::now()
				{
					// Setting expired, delete it
					let _ = self.delete(key).await;
					return Ok(None);
				}

				// Use index-based access for MySQL compatibility (value is first column, index 0)
				// MySQL TEXT columns may be returned as BLOB, so try Vec<u8> first
				let value: String = if let Ok(s) = row.try_get::<String, _>(0) {
					s
				} else if let Ok(bytes) = row.try_get::<Vec<u8>, _>(0) {
					String::from_utf8(bytes)
						.map_err(|e| format!("Invalid UTF-8 in value column: {}", e))?
				} else {
					return Err("Missing value column".to_string());
				};

				let data: serde_json::Value = serde_json::from_str(&value)
					.map_err(|e| format!("Deserialization error: {}", e))?;

				Ok(Some(data))
			}
			None => Ok(None),
		}
	}

	/// Set a setting value with optional TTL
	///
	/// ## Arguments
	///
	/// * `key` - Setting key
	/// * `value` - Setting value (must be JSON-serializable)
	/// * `ttl` - Optional time-to-live in seconds
	///
	/// ## Example
	///
	/// ```rust,no_run
	/// # #[cfg(feature = "dynamic-database")]
	/// # async fn example() -> Result<(), String> {
	/// use reinhardt_conf::settings::backends::DatabaseBackend;
	/// use serde_json::json;
	///
	/// let backend = DatabaseBackend::new("sqlite::memory:").await?;
	/// backend.create_table().await?;
	///
	/// // Set with 1 hour TTL
	/// backend.set("temp_config", &json!({"enabled": true}), Some(3600)).await?;
	///
	/// // Set without TTL (permanent)
	/// backend.set("permanent_config", &json!({"version": "1.0"}), None).await?;
	/// # Ok(())
	/// # }
	/// ```
	#[cfg(feature = "dynamic-database")]
	pub async fn set(
		&self,
		key: &str,
		value: &serde_json::Value,
		ttl: Option<u64>,
	) -> Result<(), String> {
		let value_str =
			serde_json::to_string(value).map_err(|e| format!("Serialization error: {}", e))?;

		let expire_date_str = ttl.map(|seconds| {
			let expire_date = Utc::now() + Duration::seconds(seconds as i64);
			expire_date.to_rfc3339()
		});

		// Delete existing key first for simplicity (works across all databases)
		let _ = self.delete(key).await;

		use reinhardt_query::prelude::{Alias, IntoValue, Query};

		// Build INSERT query using reinhardt-query
		let mut stmt = Query::insert()
			.into_table(Alias::new("settings"))
			.columns([
				Alias::new("key"),
				Alias::new("value"),
				Alias::new("expire_date"),
			])
			.to_owned();

		// Add values based on whether expire_date is set
		if let Some(expire_str) = &expire_date_str {
			stmt.values_panic(vec![
				key.into_value(),
				value_str.into_value(),
				expire_str.clone().into_value(),
			]);
		} else {
			stmt.values_panic(vec![
				key.into_value(),
				value_str.into_value(),
				Option::<String>::None.into_value(),
			]);
		}

		let sql = self.build_sql(stmt);

		sqlx::query(&sql)
			.execute(self.pool.as_ref())
			.await
			.map_err(|e| format!("Failed to set setting: {}", e))?;

		Ok(())
	}

	/// Delete a setting by key
	///
	/// ## Example
	///
	/// ```rust,no_run
	/// # #[cfg(feature = "dynamic-database")]
	/// # async fn example() -> Result<(), String> {
	/// use reinhardt_conf::settings::backends::DatabaseBackend;
	/// use serde_json::json;
	///
	/// let backend = DatabaseBackend::new("sqlite::memory:").await?;
	/// backend.create_table().await?;
	///
	/// backend.set("key", &json!("value"), None).await?;
	/// backend.delete("key").await?;
	/// assert!(!backend.exists("key").await?);
	/// # Ok(())
	/// # }
	/// ```
	#[cfg(feature = "dynamic-database")]
	pub async fn delete(&self, key: &str) -> Result<(), String> {
		use reinhardt_query::prelude::{Alias, Expr, ExprTrait, Query};

		// Build DELETE query using reinhardt-query
		let stmt = Query::delete()
			.from_table(Alias::new("settings"))
			.and_where(Expr::col(Alias::new("key")).eq(key))
			.to_owned();

		let sql = self.build_sql(stmt);

		sqlx::query(&sql)
			.execute(self.pool.as_ref())
			.await
			.map_err(|e| format!("Failed to delete setting: {}", e))?;

		Ok(())
	}

	/// Check if a setting exists and is not expired
	///
	/// ## Example
	///
	/// ```rust,no_run
	/// # #[cfg(feature = "dynamic-database")]
	/// # async fn example() -> Result<(), String> {
	/// use reinhardt_conf::settings::backends::DatabaseBackend;
	/// use serde_json::json;
	///
	/// let backend = DatabaseBackend::new("sqlite::memory:").await?;
	/// backend.create_table().await?;
	///
	/// assert!(!backend.exists("nonexistent").await?);
	///
	/// backend.set("key", &json!("value"), None).await?;
	/// assert!(backend.exists("key").await?);
	/// # Ok(())
	/// # }
	/// ```
	#[cfg(feature = "dynamic-database")]
	pub async fn exists(&self, key: &str) -> Result<bool, String> {
		use reinhardt_query::prelude::{Alias, Cond, Expr, ExprTrait, Query};

		let now = Utc::now().to_rfc3339();

		// Build SELECT query using reinhardt-query
		// WHERE key = ? AND (expire_date IS NULL OR expire_date > ?)
		let stmt = Query::select()
			.expr(Expr::value(1))
			.from(Alias::new("settings"))
			.and_where(Expr::col(Alias::new("key")).eq(key))
			.and_where(
				Cond::any()
					.add(Expr::col(Alias::new("expire_date")).is_null())
					.add(Expr::col(Alias::new("expire_date")).gt(Expr::value(&now))),
			)
			.to_owned();

		let sql = self.build_sql(stmt);

		let row = sqlx::query(&sql)
			.fetch_optional(self.pool.as_ref())
			.await
			.map_err(|e| format!("Failed to check setting existence: {}", e))?;

		Ok(row.is_some())
	}

	/// Clean up expired settings
	///
	/// Deletes all settings that have passed their expiration time.
	/// This should be called periodically to prevent database bloat.
	///
	/// Returns the number of deleted settings.
	///
	/// ## Example
	///
	/// ```rust,no_run
	/// # #[cfg(feature = "dynamic-database")]
	/// # async fn example() -> Result<(), String> {
	/// use reinhardt_conf::settings::backends::DatabaseBackend;
	///
	/// let backend = DatabaseBackend::new("sqlite::memory:").await?;
	/// backend.create_table().await?;
	///
	/// let deleted_count = backend.cleanup_expired().await?;
	/// println!("Deleted {} expired settings", deleted_count);
	/// # Ok(())
	/// # }
	/// ```
	#[cfg(feature = "dynamic-database")]
	pub async fn cleanup_expired(&self) -> Result<u64, String> {
		use reinhardt_query::prelude::{Alias, Expr, ExprTrait, Query};

		let now = Utc::now().to_rfc3339();

		// Build DELETE query using reinhardt-query
		let stmt = Query::delete()
			.from_table(Alias::new("settings"))
			.and_where(Expr::col(Alias::new("expire_date")).lt(Expr::value(&now)))
			.to_owned();

		let sql = self.build_sql(stmt);

		let rows_affected = sqlx::query(&sql)
			.execute(self.pool.as_ref())
			.await
			.map_err(|e| format!("Failed to cleanup settings: {}", e))?
			.rows_affected();

		Ok(rows_affected)
	}

	/// Get all non-expired setting keys
	///
	/// ## Example
	///
	/// ```rust,no_run
	/// # #[cfg(feature = "dynamic-database")]
	/// # async fn example() -> Result<(), String> {
	/// use reinhardt_conf::settings::backends::DatabaseBackend;
	/// use serde_json::json;
	///
	/// let backend = DatabaseBackend::new("sqlite::memory:").await?;
	/// backend.create_table().await?;
	///
	/// backend.set("key1", &json!("value1"), None).await?;
	/// backend.set("key2", &json!("value2"), None).await?;
	///
	/// let keys = backend.keys().await?;
	/// assert!(keys.contains(&"key1".to_string()));
	/// assert!(keys.contains(&"key2".to_string()));
	/// # Ok(())
	/// # }
	/// ```
	#[cfg(feature = "dynamic-database")]
	pub async fn keys(&self) -> Result<Vec<String>, String> {
		use reinhardt_query::prelude::{Alias, Cond, Expr, ExprTrait, Query};

		let now = Utc::now().to_rfc3339();

		// Build SELECT query using reinhardt-query
		// WHERE expire_date IS NULL OR expire_date > ?
		let stmt = Query::select()
			.column(Alias::new("key"))
			.from(Alias::new("settings"))
			.and_where(
				Cond::any()
					.add(Expr::col(Alias::new("expire_date")).is_null())
					.add(Expr::col(Alias::new("expire_date")).gt(Expr::value(&now))),
			)
			.to_owned();

		let sql = self.build_sql(stmt);

		let rows = sqlx::query(&sql)
			.fetch_all(self.pool.as_ref())
			.await
			.map_err(|e| format!("Failed to fetch keys: {}", e))?;

		// Use index-based access for MySQL compatibility (key is first column, index 0)
		// MySQL TEXT columns may be returned as BLOB, so handle both String and Vec<u8>
		let keys: Result<Vec<String>, String> = rows
			.iter()
			.map(|row| {
				if let Ok(s) = row.try_get::<String, _>(0) {
					Ok(s)
				} else if let Ok(bytes) = row.try_get::<Vec<u8>, _>(0) {
					String::from_utf8(bytes)
						.map_err(|e| format!("Invalid UTF-8 in key column: {}", e))
				} else {
					Err("Missing key column".to_string())
				}
			})
			.collect();

		keys
	}
}

/// Convert String errors to DynamicError
#[cfg(feature = "dynamic-database")]
impl From<String> for DynamicError {
	fn from(error: String) -> Self {
		DynamicError::Backend(error)
	}
}

/// DynamicBackend trait implementation for DatabaseBackend
#[cfg(feature = "dynamic-database")]
#[async_trait]
impl DynamicBackend for DatabaseBackend {
	async fn get(&self, key: &str) -> DynamicResult<Option<serde_json::Value>> {
		self.get(key).await.map_err(DynamicError::from)
	}

	async fn set(
		&self,
		key: &str,
		value: &serde_json::Value,
		ttl: Option<u64>,
	) -> DynamicResult<()> {
		self.set(key, value, ttl).await.map_err(DynamicError::from)
	}

	async fn delete(&self, key: &str) -> DynamicResult<()> {
		self.delete(key).await.map_err(DynamicError::from)
	}

	async fn exists(&self, key: &str) -> DynamicResult<bool> {
		self.exists(key).await.map_err(DynamicError::from)
	}

	async fn keys(&self) -> DynamicResult<Vec<String>> {
		self.keys().await.map_err(DynamicError::from)
	}
}

#[cfg(all(test, not(feature = "dynamic-database")))]
mod tests_no_feature {
	use super::*;

	#[tokio::test]
	async fn test_database_backend_disabled() {
		let result = DatabaseBackend::new("sqlite::memory:?mode=rwc&cache=shared").await;
		assert!(result.is_err());
		assert!(result.unwrap_err().contains("Database backend not enabled"));
	}
}

#[cfg(all(test, feature = "dynamic-database"))]
mod tests {
	use super::*;
	use serde_json::json;
	use std::sync::Once;

	static INIT_DRIVERS: Once = Once::new();

	fn init_drivers() {
		INIT_DRIVERS.call_once(|| {
			sqlx::any::install_default_drivers();
		});
	}

	async fn create_test_backend() -> DatabaseBackend {
		init_drivers();
		// Use in-memory SQLite with shared cache mode
		// This allows multiple connections from the pool to share the same in-memory database
		let db_url = "sqlite::memory:?mode=rwc&cache=shared";

		// Create backend with minimal connection pool for tests
		use sqlx::any::AnyPoolOptions;
		let pool = AnyPoolOptions::new()
			.min_connections(1)
			.max_connections(1)
			.connect(db_url)
			.await
			.expect("Failed to connect to test database");

		let backend = DatabaseBackend::from_pool(Arc::new(pool), db_url);
		backend
			.create_table()
			.await
			.expect("Failed to create table");
		backend
	}

	#[tokio::test]
	async fn test_set_and_get_setting() {
		let backend = create_test_backend().await;
		let key = "test_setting_1";
		let value = json!({
			"debug": true,
			"port": 8080,
		});

		// Set setting
		backend
			.set(key, &value, Some(3600))
			.await
			.expect("Failed to set setting");

		// Get setting
		let retrieved = backend.get(key).await.expect("Failed to get setting");

		assert_eq!(retrieved, Some(value));
	}

	#[tokio::test]
	async fn test_setting_exists() {
		let backend = create_test_backend().await;
		let key = "test_setting_2";
		let value = json!({"test": "data"});

		// Setting should not exist initially
		let exists = backend
			.exists(key)
			.await
			.expect("Failed to check existence");
		assert!(!exists);

		// Set setting
		backend
			.set(key, &value, Some(3600))
			.await
			.expect("Failed to set setting");

		// Setting should now exist
		let exists = backend
			.exists(key)
			.await
			.expect("Failed to check existence");
		assert!(exists);
	}

	#[tokio::test]
	async fn test_delete_setting() {
		let backend = create_test_backend().await;
		let key = "test_setting_3";
		let value = json!({"test": "data"});

		// Set setting
		backend
			.set(key, &value, Some(3600))
			.await
			.expect("Failed to set setting");

		// Verify setting exists
		assert!(
			backend
				.exists(key)
				.await
				.expect("Failed to check existence")
		);

		// Delete setting
		backend.delete(key).await.expect("Failed to delete setting");

		// Verify setting no longer exists
		assert!(
			!backend
				.exists(key)
				.await
				.expect("Failed to check existence")
		);
	}

	#[tokio::test]
	async fn test_expired_setting() {
		let backend = create_test_backend().await;
		let key = "test_setting_4";
		let value = json!({"test": "data"});

		// Set setting with 0 second TTL (immediately expired)
		backend
			.set(key, &value, Some(0))
			.await
			.expect("Failed to set setting");

		// Try to get expired setting
		let retrieved = backend.get(key).await.expect("Failed to get setting");

		assert_eq!(retrieved, None);
	}

	#[tokio::test]
	async fn test_cleanup_expired() {
		let backend = create_test_backend().await;

		// Set some expired settings
		for i in 0..5 {
			let key = format!("expired_{}", i);
			backend
				.set(&key, &json!({ "test": i }), Some(0))
				.await
				.expect("Failed to set setting");
		}

		// Set some active settings
		for i in 0..3 {
			let key = format!("active_{}", i);
			backend
				.set(&key, &json!({ "test": i }), Some(3600))
				.await
				.expect("Failed to set setting");
		}

		// Clean up expired settings
		let deleted = backend.cleanup_expired().await.expect("Failed to cleanup");

		assert_eq!(deleted, 5);

		// Verify active settings still exist
		for i in 0..3 {
			let key = format!("active_{}", i);
			assert!(
				backend
					.exists(&key)
					.await
					.expect("Failed to check existence")
			);
		}
	}

	#[tokio::test]
	async fn test_setting_without_ttl() {
		let backend = create_test_backend().await;
		let key = "permanent_setting";
		let value = json!({"permanent": true});

		// Set setting without TTL
		backend
			.set(key, &value, None)
			.await
			.expect("Failed to set setting");

		// Get setting
		let retrieved = backend.get(key).await.expect("Failed to get setting");
		assert_eq!(retrieved, Some(value));

		// Verify it exists
		assert!(
			backend
				.exists(key)
				.await
				.expect("Failed to check existence")
		);
	}

	#[tokio::test]
	async fn test_overwrite_existing_setting() {
		let backend = create_test_backend().await;
		let key = "overwrite_test";

		// Set initial value
		backend
			.set(key, &json!({"value": 1}), None)
			.await
			.expect("Failed to set setting");

		// Overwrite with new value
		backend
			.set(key, &json!({"value": 2}), None)
			.await
			.expect("Failed to set setting");

		// Get updated value
		let retrieved = backend.get(key).await.expect("Failed to get setting");
		assert_eq!(retrieved, Some(json!({"value": 2})));
	}

	#[tokio::test]
	async fn test_keys_retrieval() {
		let backend = create_test_backend().await;

		// Set multiple settings
		backend
			.set("key1", &json!("value1"), None)
			.await
			.expect("Failed to set key1");
		backend
			.set("key2", &json!("value2"), None)
			.await
			.expect("Failed to set key2");
		backend
			.set("key3", &json!("value3"), None)
			.await
			.expect("Failed to set key3");

		// Get all keys
		let keys = backend.keys().await.expect("Failed to get keys");

		// Verify all keys are present
		assert_eq!(keys.len(), 3);
		assert!(keys.contains(&"key1".to_string()));
		assert!(keys.contains(&"key2".to_string()));
		assert!(keys.contains(&"key3".to_string()));
	}

	#[tokio::test]
	async fn test_keys_excludes_expired() {
		let backend = create_test_backend().await;

		// Set active settings
		backend
			.set("active1", &json!("value1"), Some(3600))
			.await
			.expect("Failed to set active1");
		backend
			.set("active2", &json!("value2"), None)
			.await
			.expect("Failed to set active2");

		// Set expired settings
		backend
			.set("expired1", &json!("value3"), Some(0))
			.await
			.expect("Failed to set expired1");
		backend
			.set("expired2", &json!("value4"), Some(0))
			.await
			.expect("Failed to set expired2");

		// Get keys (should only include active settings)
		let keys = backend.keys().await.expect("Failed to get keys");

		assert_eq!(keys.len(), 2);
		assert!(keys.contains(&"active1".to_string()));
		assert!(keys.contains(&"active2".to_string()));
		assert!(!keys.contains(&"expired1".to_string()));
		assert!(!keys.contains(&"expired2".to_string()));
	}
}

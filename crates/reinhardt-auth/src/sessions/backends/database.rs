//! Database-backed session storage
//!
//! This module provides session storage using a database backend (PostgreSQL, MySQL, SQLite).
//! Sessions are persisted to a database table, making them survive application restarts.
//!
//! ## Features
//!
//! - Persistent session storage
//! - Automatic session expiration cleanup
//! - Support for multiple database backends
//!
//! ## Example
//!
//! ```rust,no_run
//! use reinhardt_auth::sessions::backends::{DatabaseSessionBackend, SessionBackend};
//! use serde_json::json;
//!
//! # async fn example() {
//! // Create a database session backend
//! // Note: For actual usage, any database URL is supported (postgres://, mysql://, sqlite:)
//! let backend = DatabaseSessionBackend::new("sqlite::memory:").await.unwrap();
//! backend.create_table().await.unwrap();
//!
//! // Store user session
//! let session_data = json!({
//!     "user_id": 42,
//!     "username": "alice",
//!     "authenticated": true,
//! });
//!
//! backend.save("session_key_123", &session_data, Some(3600)).await.unwrap();
//!
//! // Retrieve session
//! let retrieved: Option<serde_json::Value> = backend.load("session_key_123").await.unwrap();
//! assert!(retrieved.is_some());
//! assert_eq!(retrieved.unwrap()["user_id"], 42);
//!
//! // Clean up expired sessions
//! let deleted_count = backend.cleanup_expired().await.unwrap();
//! assert_eq!(deleted_count, 0); // No expired sessions
//! # }
//! # tokio::runtime::Runtime::new().unwrap().block_on(example());
//! ```

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use reinhardt_core::macros::model;
use reinhardt_db::DatabaseConnection;
use reinhardt_db::orm::{DatabaseBackend, Filter, FilterOperator, FilterValue, Model};
use reinhardt_query::prelude::{
	Alias, ColumnDef, CreateIndexStatement, Expr, ExprTrait, IntoValue, MySqlQueryBuilder,
	OnConflict, PostgresQueryBuilder, Query, QueryStatementBuilder, SqliteQueryBuilder,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::sessions::cleanup::{CleanupableBackend, SessionMetadata};

use super::cache::{SessionBackend, SessionError};

/// Database session model
///
/// Represents a session stored in the database.
/// Uses Unix timestamps (milliseconds) for date fields for database compatibility.
///
/// ## Example
///
/// ```rust
/// use reinhardt_auth::sessions::backends::database::Session;
/// use chrono::Utc;
///
/// let now_ms = Utc::now().timestamp_millis();
/// let session = Session {
///     session_key: "abc123".to_string(),
///     session_data: "{\"user_id\": 42}".to_string(),
///     expire_date: now_ms + 3600000, // 1 hour
///     created_at: now_ms,
///     last_accessed: Some(now_ms),
/// };
/// ```
#[model(table_name = "sessions")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
	/// Unique session key (primary key)
	#[field(primary_key = true, max_length = 255)]
	pub session_key: String,
	/// Session data stored as JSON string
	#[field(max_length = 65535)]
	pub session_data: String,
	/// Session expiration timestamp (Unix timestamp in milliseconds)
	#[field]
	pub expire_date: i64,
	/// Session creation timestamp (Unix timestamp in milliseconds)
	#[field]
	pub created_at: i64,
	/// Last accessed timestamp (Unix timestamp in milliseconds)
	#[field]
	pub last_accessed: Option<i64>,
}

/// Database-backed session storage
///
/// Stores sessions in a database table with automatic expiration handling.
/// Supports PostgreSQL, MySQL, and SQLite.
///
/// ## Database Schema
///
/// The backend expects a table with the following structure (created via migrations):
///
/// ```sql
/// CREATE TABLE sessions (
///     session_key VARCHAR(255) PRIMARY KEY,
///     session_data TEXT NOT NULL,
///     expire_date BIGINT NOT NULL,
///     created_at BIGINT NOT NULL,
///     last_accessed BIGINT
/// );
/// CREATE INDEX idx_sessions_expire_date ON sessions(expire_date);
/// ```
///
/// Note: Timestamps are stored as Unix timestamps (milliseconds since epoch) in BIGINT columns.
///
/// ## Example
///
/// ```rust,no_run
/// use reinhardt_auth::sessions::backends::{DatabaseSessionBackend, SessionBackend};
/// use serde_json::json;
/// use reinhardt_db::DatabaseConnection;
/// use std::sync::Arc;
///
/// # async fn example() {
/// // Initialize backend with database connection
/// let connection = DatabaseConnection::connect("sqlite::memory:").await.unwrap();
/// let backend = DatabaseSessionBackend::from_connection(Arc::new(connection));
///
/// // Note: Table should be created via migrations
///
/// // Store session with 1 hour TTL
/// let data = json!({"cart_total": 99.99});
/// backend.save("cart_xyz", &data, Some(3600)).await.unwrap();
///
/// // Check if session exists
/// let exists = backend.exists("cart_xyz").await.unwrap();
/// assert!(exists);
/// # }
/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
/// ```
#[derive(Clone)]
pub struct DatabaseSessionBackend {
	connection: Arc<DatabaseConnection>,
}

impl DatabaseSessionBackend {
	/// Create a new database session backend
	///
	/// Initializes a connection to the specified database URL.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_auth::sessions::backends::DatabaseSessionBackend;
	///
	/// # async fn example() {
	/// // Supports multiple database backends:
	/// // - PostgreSQL: "postgres://localhost/mydb"
	/// // - MySQL: "mysql://localhost/mydb"
	/// // - SQLite (in-memory): "sqlite::memory:"
	/// // - SQLite (file): "sqlite://sessions.db"
	///
	/// let backend = DatabaseSessionBackend::new("sqlite::memory:").await.unwrap();
	/// // Backend created successfully
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	pub async fn new(database_url: &str) -> Result<Self, SessionError> {
		let connection = DatabaseConnection::connect(database_url)
			.await
			.map_err(|e| SessionError::CacheError(format!("Database connection error: {}", e)))?;

		Ok(Self {
			connection: Arc::new(connection),
		})
	}

	/// Create a new backend from an existing database connection
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_auth::sessions::backends::DatabaseSessionBackend;
	/// use reinhardt_db::DatabaseConnection;
	/// use std::sync::Arc;
	///
	/// # async fn example() {
	/// let connection = DatabaseConnection::connect("sqlite::memory:").await.unwrap();
	/// let backend = DatabaseSessionBackend::from_connection(Arc::new(connection));
	/// // Backend created from existing connection
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	pub fn from_connection(connection: Arc<DatabaseConnection>) -> Self {
		Self { connection }
	}

	/// Build SQL string for the current database backend
	///
	/// Uses reinhardt-query to generate database-specific SQL syntax.
	fn build_sql<T>(&self, statement: T) -> String
	where
		T: QueryStatementBuilder,
	{
		match self.connection.backend() {
			DatabaseBackend::Postgres => statement.to_string(PostgresQueryBuilder),
			DatabaseBackend::MySql => statement.to_string(MySqlQueryBuilder),
			DatabaseBackend::Sqlite => statement.to_string(SqliteQueryBuilder),
		}
	}

	/// Build table SQL string for the current database backend
	fn build_table_sql<T>(&self, statement: T) -> String
	where
		T: QueryStatementBuilder,
	{
		match self.connection.backend() {
			DatabaseBackend::Postgres => statement.to_string(PostgresQueryBuilder),
			DatabaseBackend::MySql => statement.to_string(MySqlQueryBuilder),
			DatabaseBackend::Sqlite => statement.to_string(SqliteQueryBuilder),
		}
	}

	/// Build index SQL string for the current database backend
	fn build_index_sql(&self, statement: &CreateIndexStatement) -> String {
		match self.connection.backend() {
			DatabaseBackend::Postgres => statement.to_string(PostgresQueryBuilder),
			DatabaseBackend::MySql => statement.to_string(MySqlQueryBuilder),
			DatabaseBackend::Sqlite => statement.to_string(SqliteQueryBuilder),
		}
	}

	/// Clean up expired sessions
	///
	/// Deletes all sessions that have passed their expiration time.
	/// This should be called periodically to prevent database bloat.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_auth::sessions::backends::DatabaseSessionBackend;
	///
	/// # async fn example() {
	/// let backend = DatabaseSessionBackend::new("sqlite::memory:").await.unwrap();
	///
	/// // Note: Table should be created via migrations
	///
	/// // Clean up expired sessions
	/// let deleted_count = backend.cleanup_expired().await.unwrap();
	/// assert!(deleted_count >= 0); // Returns number of deleted sessions
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	pub async fn cleanup_expired(&self) -> Result<u64, SessionError> {
		let now_timestamp = Utc::now().timestamp_millis();

		// Build DELETE query using reinhardt-query
		let stmt = Query::delete()
			.from_table(Alias::new("sessions"))
			.and_where(Expr::col(Alias::new("expire_date")).lt(now_timestamp))
			.to_owned();

		let sql = self.build_sql(stmt);
		let rows_affected =
			self.connection.execute(&sql, vec![]).await.map_err(|e| {
				SessionError::CacheError(format!("Failed to cleanup sessions: {}", e))
			})?;

		Ok(rows_affected)
	}

	/// Create the sessions table
	///
	/// Creates the sessions table in the database. This is primarily intended for testing.
	/// In production, migrations should be used to create the table.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_auth::sessions::backends::DatabaseSessionBackend;
	///
	/// # async fn example() {
	/// let backend = DatabaseSessionBackend::new("sqlite::memory:").await.unwrap();
	/// backend.create_table().await.unwrap();
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	pub async fn create_table(&self) -> Result<(), SessionError> {
		// Build CREATE TABLE statement using reinhardt-query
		let stmt = Query::create_table()
			.table(Alias::new("sessions"))
			.if_not_exists()
			.col(
				ColumnDef::new(Alias::new("session_key"))
					.string_len(255)
					.not_null(true)
					.primary_key(true),
			)
			.col(
				ColumnDef::new(Alias::new("session_data"))
					.text()
					.not_null(true),
			)
			.col(
				ColumnDef::new(Alias::new("expire_date"))
					.big_integer()
					.not_null(true),
			)
			.col(
				ColumnDef::new(Alias::new("created_at"))
					.big_integer()
					.not_null(true),
			)
			.col(ColumnDef::new(Alias::new("last_accessed")).big_integer())
			.to_owned();

		let sql = self.build_table_sql(stmt);
		self.connection.execute(&sql, vec![]).await.map_err(|e| {
			SessionError::CacheError(format!("Failed to create sessions table: {}", e))
		})?;

		// Create index for expire_date using reinhardt-query
		let index_stmt = Query::create_index()
			.if_not_exists()
			.name("idx_sessions_expire_date")
			.table(Alias::new("sessions"))
			.col(Alias::new("expire_date"))
			.to_owned();

		let index_sql = self.build_index_sql(&index_stmt);
		let _ = self.connection.execute(&index_sql, vec![]).await;

		Ok(())
	}
}

#[async_trait]
impl SessionBackend for DatabaseSessionBackend {
	async fn load<T>(&self, session_key: &str) -> Result<Option<T>, SessionError>
	where
		T: for<'de> Deserialize<'de> + Send,
	{
		// Use ORM to load session
		let session = Session::objects()
			.filter_by(Filter::new(
				"session_key".to_string(),
				FilterOperator::Eq,
				FilterValue::String(session_key.to_string()),
			))
			.first()
			.await
			.ok()
			.flatten();

		match session {
			Some(session) => {
				// Check if session has expired
				let expire_date =
					DateTime::from_timestamp_millis(session.expire_date).unwrap_or_else(Utc::now);

				if expire_date < Utc::now() {
					// Session expired, delete it
					let _ = self.delete(session_key).await;
					return Ok(None);
				}

				let data: T = serde_json::from_str(&session.session_data).map_err(|e| {
					SessionError::SerializationError(format!("Deserialization error: {}", e))
				})?;

				Ok(Some(data))
			}
			None => Ok(None),
		}
	}

	async fn save<T>(
		&self,
		session_key: &str,
		data: &T,
		ttl: Option<u64>,
	) -> Result<(), SessionError>
	where
		T: Serialize + Send + Sync,
	{
		let session_data = serde_json::to_string(data)
			.map_err(|e| SessionError::SerializationError(format!("Serialization error: {}", e)))?;

		let now = Utc::now();
		let expire_date = match ttl {
			Some(seconds) => now + Duration::seconds(seconds as i64),
			None => now + Duration::days(14), // Default 14 days
		};

		let now_timestamp = now.timestamp_millis();
		let expire_timestamp = expire_date.timestamp_millis();

		// Build UPSERT statement using reinhardt-query
		// reinhardt-query handles database-specific UPSERT syntax (ON CONFLICT vs ON DUPLICATE KEY)
		let stmt = Query::insert()
			.into_table(Alias::new("sessions"))
			.columns([
				Alias::new("session_key"),
				Alias::new("session_data"),
				Alias::new("expire_date"),
				Alias::new("created_at"),
				Alias::new("last_accessed"),
			])
			.values_panic(vec![
				session_key.into_value(),
				session_data.into_value(),
				expire_timestamp.into_value(),
				now_timestamp.into_value(),
				now_timestamp.into_value(),
			])
			.on_conflict(
				OnConflict::column(Alias::new("session_key"))
					.update_columns([
						Alias::new("session_data"),
						Alias::new("expire_date"),
						Alias::new("last_accessed"),
					])
					.to_owned(),
			)
			.to_owned();

		let sql = self.build_sql(stmt);
		self.connection
			.execute(&sql, vec![])
			.await
			.map_err(|e| SessionError::CacheError(format!("Failed to save session: {}", e)))?;

		Ok(())
	}

	async fn delete(&self, session_key: &str) -> Result<(), SessionError> {
		// Build DELETE query using reinhardt-query
		let stmt = Query::delete()
			.from_table(Alias::new("sessions"))
			.and_where(Expr::col(Alias::new("session_key")).eq(session_key))
			.to_owned();

		let sql = self.build_sql(stmt);
		self.connection
			.execute(&sql, vec![])
			.await
			.map_err(|e| SessionError::CacheError(format!("Failed to delete session: {}", e)))?;

		Ok(())
	}

	async fn exists(&self, session_key: &str) -> Result<bool, SessionError> {
		let now_timestamp = Utc::now().timestamp_millis();

		// Use ORM to check if session exists and is not expired
		let session = Session::objects()
			.filter_by(Filter::new(
				"session_key".to_string(),
				FilterOperator::Eq,
				FilterValue::String(session_key.to_string()),
			))
			.filter(Filter::new(
				"expire_date".to_string(),
				FilterOperator::Gt,
				FilterValue::Integer(now_timestamp),
			))
			.first()
			.await
			.ok()
			.flatten();

		Ok(session.is_some())
	}
}

#[async_trait]
impl CleanupableBackend for DatabaseSessionBackend {
	async fn get_all_keys(&self) -> Result<Vec<String>, SessionError> {
		// Use ORM to get all session keys
		// Manager::all() returns QuerySet, QuerySet::all() executes and returns Vec<T>
		let sessions = Session::objects()
			.all()
			.all()
			.await
			.map_err(|e| SessionError::CacheError(format!("Failed to get all keys: {}", e)))?;

		let keys: Vec<String> = sessions.into_iter().map(|s| s.session_key).collect();

		Ok(keys)
	}

	async fn get_metadata(
		&self,
		session_key: &str,
	) -> Result<Option<SessionMetadata>, SessionError> {
		// Use ORM to get session metadata
		let session = Session::objects()
			.filter_by(Filter::new(
				"session_key".to_string(),
				FilterOperator::Eq,
				FilterValue::String(session_key.to_string()),
			))
			.first()
			.await
			.ok()
			.flatten();

		match session {
			Some(session) => {
				let created_at =
					DateTime::from_timestamp_millis(session.created_at).unwrap_or_else(Utc::now);

				let last_accessed = session
					.last_accessed
					.and_then(DateTime::from_timestamp_millis);

				Ok(Some(SessionMetadata {
					created_at,
					last_accessed,
				}))
			}
			None => Ok(None),
		}
	}

	async fn list_keys_with_prefix(&self, prefix: &str) -> Result<Vec<String>, SessionError> {
		// Use ORM to list session keys with prefix
		let sessions = Session::objects()
			.filter_by(Filter::new(
				"session_key".to_string(),
				FilterOperator::StartsWith,
				FilterValue::String(prefix.to_string()),
			))
			.all()
			.await
			.map_err(|e| SessionError::CacheError(format!("Failed to list session keys: {}", e)))?;

		let keys: Vec<String> = sessions.into_iter().map(|s| s.session_key).collect();

		Ok(keys)
	}

	async fn count_keys_with_prefix(&self, prefix: &str) -> Result<usize, SessionError> {
		// Use ORM to count session keys with prefix
		let count = Session::objects()
			.filter_by(Filter::new(
				"session_key".to_string(),
				FilterOperator::StartsWith,
				FilterValue::String(prefix.to_string()),
			))
			.count()
			.await
			.map_err(|e| {
				SessionError::CacheError(format!("Failed to count session keys: {}", e))
			})?;

		Ok(count)
	}

	async fn delete_keys_with_prefix(&self, prefix: &str) -> Result<usize, SessionError> {
		// Build DELETE query with LIKE condition using reinhardt-query
		let pattern = format!("{}%", prefix);
		let stmt = Query::delete()
			.from_table(Alias::new("sessions"))
			.and_where(Expr::col(Alias::new("session_key")).like(pattern.as_str()))
			.to_owned();

		let sql = self.build_sql(stmt);
		let rows_affected = self.connection.execute(&sql, vec![]).await.map_err(|e| {
			SessionError::CacheError(format!("Failed to delete session keys: {}", e))
		})?;

		Ok(rows_affected as usize)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_session_struct_fields() {
		let now_ms = Utc::now().timestamp_millis();
		let session = Session::new(
			"test_key".to_string(),
			r#"{"user_id": 42}"#.to_string(),
			now_ms + 3600000, // 1 hour from now
			now_ms,
			Some(now_ms),
		);

		assert_eq!(session.session_key, "test_key");
		assert_eq!(session.session_data, r#"{"user_id": 42}"#);
		assert_eq!(session.expire_date, now_ms + 3600000);
		assert_eq!(session.created_at, now_ms);
		assert_eq!(session.last_accessed, Some(now_ms));
	}

	#[test]
	fn test_session_struct_without_last_accessed() {
		let now_ms = Utc::now().timestamp_millis();
		let session = Session::new(
			"key".to_string(),
			"{}".to_string(),
			now_ms + 1000,
			now_ms,
			None,
		);

		assert!(session.last_accessed.is_none());
	}

	#[test]
	fn test_session_clone() {
		let now_ms = Utc::now().timestamp_millis();
		let session = Session::new(
			"clone_test".to_string(),
			r#"{"data": "value"}"#.to_string(),
			now_ms + 3600000,
			now_ms,
			Some(now_ms),
		);

		let cloned = session.clone();

		assert_eq!(cloned.session_key, session.session_key);
		assert_eq!(cloned.session_data, session.session_data);
		assert_eq!(cloned.expire_date, session.expire_date);
		assert_eq!(cloned.created_at, session.created_at);
		assert_eq!(cloned.last_accessed, session.last_accessed);
	}

	#[test]
	fn test_session_debug() {
		let now_ms = Utc::now().timestamp_millis();
		let session = Session::new(
			"debug_key".to_string(),
			"{}".to_string(),
			now_ms,
			now_ms,
			None,
		);

		let debug_str = format!("{:?}", session);

		assert!(debug_str.contains("Session"));
		assert!(debug_str.contains("debug_key"));
	}

	#[test]
	fn test_session_serialize() {
		let now_ms = Utc::now().timestamp_millis();
		let session = Session::new(
			"serialize_key".to_string(),
			r#"{"count": 10}"#.to_string(),
			now_ms + 3600000,
			now_ms,
			Some(now_ms),
		);

		let json = serde_json::to_string(&session).unwrap();

		assert!(json.contains("serialize_key"));
		// session_data is serialized as a JSON string, so internal quotes are escaped
		assert!(json.contains(r#"{\"count\": 10}"#));
	}

	#[test]
	fn test_session_deserialize() {
		let now_ms = 1700000000000_i64; // Fixed timestamp for test
		let json = format!(
			r#"{{
				"session_key": "deserialize_key",
				"session_data": "{{\"user\": \"test\"}}",
				"expire_date": {},
				"created_at": {},
				"last_accessed": {}
			}}"#,
			now_ms + 3600000,
			now_ms,
			now_ms
		);

		let session: Session = serde_json::from_str(&json).unwrap();

		assert_eq!(session.session_key, "deserialize_key");
		assert_eq!(session.session_data, r#"{"user": "test"}"#);
		assert_eq!(session.expire_date, now_ms + 3600000);
		assert_eq!(session.created_at, now_ms);
		assert_eq!(session.last_accessed, Some(now_ms));
	}

	#[test]
	fn test_session_deserialize_without_last_accessed() {
		let now_ms = 1700000000000_i64;
		let json = format!(
			r#"{{
				"session_key": "no_access",
				"session_data": "{{}}",
				"expire_date": {},
				"created_at": {},
				"last_accessed": null
			}}"#,
			now_ms + 3600000,
			now_ms
		);

		let session: Session = serde_json::from_str(&json).unwrap();

		assert_eq!(session.session_key, "no_access");
		assert!(session.last_accessed.is_none());
	}

	#[test]
	fn test_database_session_backend_clone() {
		// DatabaseSessionBackend implements Clone via Arc
		// We can't test this without a real connection, but we can verify the trait is implemented
		fn assert_clone<T: Clone>() {}
		assert_clone::<DatabaseSessionBackend>();
	}
}

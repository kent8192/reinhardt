//! Database audit backend
//!
//! This backend stores audit logs in a SQL database.

use crate::settings::audit::{AuditBackend, AuditEvent, ChangeRecord, EventFilter, EventType};
use crate::settings::database_config::validate_database_url_scheme;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reinhardt_query::prelude::{
	Alias, ColumnDef, CreateIndexStatement, Expr, ExprTrait, IntoValue, MySqlQueryBuilder, Order,
	PostgresQueryBuilder, Query, QueryStatementBuilder, SqliteQueryBuilder,
};
use serde_json;
use sqlx::{AnyPool, Row};
use std::collections::HashMap;

/// Database audit backend
///
/// Stores audit logs in a SQL database.
/// Supports PostgreSQL, MySQL, and SQLite.
pub struct DatabaseAuditBackend {
	pool: std::sync::Arc<AnyPool>,
	database_url: String,
}

impl DatabaseAuditBackend {
	/// Create a new database audit backend
	///
	/// # Arguments
	///
	/// * `database_url` - Database connection URL
	///
	/// # Example
	///
	/// ```no_run
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// use reinhardt_conf::settings::audit::backends::DatabaseAuditBackend;
	///
	/// let backend = DatabaseAuditBackend::new("sqlite::memory:").await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn new(database_url: &str) -> Result<Self, String> {
		validate_database_url_scheme(database_url)?;

		let pool = AnyPool::connect(database_url)
			.await
			.map_err(|e| format!("Database connection failed: {}", e))?;

		let backend = Self {
			pool: std::sync::Arc::new(pool),
			database_url: database_url.to_string(),
		};
		backend.init_tables().await?;

		Ok(backend)
	}

	/// Detect database backend type from URL
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

	/// Build SQL string from a query statement
	///
	/// Selects the appropriate QueryBuilder based on the database backend type.
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

	/// Build SQL string from a DDL (table) statement
	///
	/// Selects the appropriate QueryBuilder based on the database backend type.
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

	/// Build SQL string from an index statement
	///
	/// Selects the appropriate QueryBuilder based on the database backend type.
	fn build_index_sql(&self, statement: &CreateIndexStatement) -> String {
		match self.detect_backend() {
			"postgres" => statement.to_string(PostgresQueryBuilder),
			"mysql" => statement.to_string(MySqlQueryBuilder),
			_ => statement.to_string(SqliteQueryBuilder),
		}
	}

	/// Initialize audit tables if they don't exist
	async fn init_tables(&self) -> Result<(), String> {
		// Create audit_events table
		let stmt = Query::create_table()
			.table(Alias::new("audit_events"))
			.if_not_exists()
			.col(
				ColumnDef::new(Alias::new("id"))
					.integer()
					.not_null(true)
					.auto_increment(true)
					.primary_key(true),
			)
			.col(
				ColumnDef::new(Alias::new("timestamp"))
					.text()
					.not_null(true),
			)
			.col(
				ColumnDef::new(Alias::new("event_type"))
					.text()
					.not_null(true),
			)
			.col(ColumnDef::new(Alias::new("user")).text())
			.col(ColumnDef::new(Alias::new("changes")).text().not_null(true))
			.to_owned();
		let sql = self.build_table_sql(stmt);
		sqlx::query(&sql)
			.execute(self.pool.as_ref())
			.await
			.map_err(|e| format!("Failed to create audit_events table: {}", e))?;

		// Create indexes for common queries
		let idx = Query::create_index()
			.if_not_exists()
			.name("idx_events_timestamp")
			.table(Alias::new("audit_events"))
			.col(Alias::new("timestamp"))
			.to_owned();
		let idx_sql = self.build_index_sql(&idx);
		let _ = sqlx::query(&idx_sql).execute(self.pool.as_ref()).await;

		let idx = Query::create_index()
			.if_not_exists()
			.name("idx_events_type")
			.table(Alias::new("audit_events"))
			.col(Alias::new("event_type"))
			.to_owned();
		let idx_sql = self.build_index_sql(&idx);
		let _ = sqlx::query(&idx_sql).execute(self.pool.as_ref()).await;

		let idx = Query::create_index()
			.if_not_exists()
			.name("idx_events_user")
			.table(Alias::new("audit_events"))
			.col(Alias::new("user"))
			.to_owned();
		let idx_sql = self.build_index_sql(&idx);
		let _ = sqlx::query(&idx_sql).execute(self.pool.as_ref()).await;

		Ok(())
	}
}

#[async_trait]
impl AuditBackend for DatabaseAuditBackend {
	async fn log_event(&self, event: AuditEvent) -> Result<(), String> {
		// Serialize changes to JSON
		let changes_json = serde_json::to_string(&event.changes)
			.map_err(|e| format!("Failed to serialize changes: {}", e))?;

		let stmt = Query::insert()
			.into_table(Alias::new("audit_events"))
			.columns([
				Alias::new("timestamp"),
				Alias::new("event_type"),
				Alias::new("user"),
				Alias::new("changes"),
			])
			.values(vec![
				event.timestamp.to_rfc3339().into_value(),
				event.event_type.as_str().into_value(),
				event.user.unwrap_or_default().into_value(),
				changes_json.into_value(),
			])
			.unwrap()
			.to_owned();
		let sql = self.build_sql(stmt);

		sqlx::query(&sql)
			.execute(self.pool.as_ref())
			.await
			.map_err(|e| format!("Failed to log event: {}", e))?;

		Ok(())
	}

	async fn get_events(&self, filter: Option<EventFilter>) -> Result<Vec<AuditEvent>, String> {
		let mut query = Query::select()
			.columns([
				Alias::new("timestamp"),
				Alias::new("event_type"),
				Alias::new("user"),
				Alias::new("changes"),
			])
			.from(Alias::new("audit_events"))
			.to_owned();

		// Apply filters
		if let Some(ref f) = filter {
			if let Some(ref event_type) = f.event_type {
				query.and_where(Expr::col(Alias::new("event_type")).eq(event_type.as_str()));
			}
			if let Some(ref user) = f.user {
				query.and_where(Expr::col(Alias::new("user")).eq(user.as_str()));
			}
			if let Some(start_time) = f.start_time {
				query.and_where(Expr::col(Alias::new("timestamp")).gte(start_time.to_rfc3339()));
			}
			if let Some(end_time) = f.end_time {
				query.and_where(Expr::col(Alias::new("timestamp")).lte(end_time.to_rfc3339()));
			}
		}

		query.order_by(Alias::new("timestamp"), Order::Desc);

		let sql = self.build_sql(query);

		let rows = sqlx::query(&sql)
			.fetch_all(self.pool.as_ref())
			.await
			.map_err(|e| format!("Failed to fetch events: {}", e))?;

		let mut events = Vec::new();
		for row in rows {
			// Use index-based access for MySQL compatibility
			// Column order: timestamp(0), event_type(1), user(2), changes(3)
			// MySQL TEXT columns may be returned as BLOB, so handle both String and Vec<u8>

			let timestamp_str: String = if let Ok(s) = row.try_get::<String, _>(0) {
				s
			} else if let Ok(bytes) = row.try_get::<Vec<u8>, _>(0) {
				String::from_utf8(bytes)
					.map_err(|e| format!("Invalid UTF-8 in timestamp: {}", e))?
			} else {
				return Err("Failed to get timestamp".to_string());
			};
			let timestamp = DateTime::parse_from_rfc3339(&timestamp_str)
				.map(|dt| dt.with_timezone(&Utc))
				.unwrap_or_else(|_| Utc::now());

			let event_type_str: String = if let Ok(s) = row.try_get::<String, _>(1) {
				s
			} else if let Ok(bytes) = row.try_get::<Vec<u8>, _>(1) {
				String::from_utf8(bytes)
					.map_err(|e| format!("Invalid UTF-8 in event_type: {}", e))?
			} else {
				return Err("Failed to get event_type".to_string());
			};
			let event_type = match event_type_str.as_str() {
				"config_update" => EventType::ConfigUpdate,
				"config_delete" => EventType::ConfigDelete,
				"config_create" => EventType::ConfigCreate,
				"secret_access" => EventType::SecretAccess,
				"secret_rotation" => EventType::SecretRotation,
				_ => EventType::ConfigUpdate, // default
			};

			let user_str: String = if let Ok(s) = row.try_get::<String, _>(2) {
				s
			} else if let Ok(bytes) = row.try_get::<Vec<u8>, _>(2) {
				String::from_utf8(bytes).map_err(|e| format!("Invalid UTF-8 in user: {}", e))?
			} else {
				return Err("Failed to get user".to_string());
			};
			let user = if user_str.is_empty() {
				None
			} else {
				Some(user_str)
			};

			let changes_json: String = if let Ok(s) = row.try_get::<String, _>(3) {
				s
			} else if let Ok(bytes) = row.try_get::<Vec<u8>, _>(3) {
				String::from_utf8(bytes).map_err(|e| format!("Invalid UTF-8 in changes: {}", e))?
			} else {
				return Err("Failed to get changes".to_string());
			};
			let changes: HashMap<String, ChangeRecord> = serde_json::from_str(&changes_json)
				.map_err(|e| format!("Failed to deserialize changes: {}", e))?;

			events.push(AuditEvent {
				timestamp,
				event_type,
				user,
				changes,
			});
		}

		// Reverse to get chronological order
		events.reverse();
		Ok(events)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use reinhardt_query::Func;
	use serde_json::json;
	use std::sync::Once;

	static INIT_DRIVERS: Once = Once::new();

	fn init_drivers() {
		INIT_DRIVERS.call_once(|| {
			sqlx::any::install_default_drivers();
		});
	}

	async fn create_test_backend() -> DatabaseAuditBackend {
		init_drivers();
		// Use named in-memory SQLite database with shared cache
		// The "file:" prefix with "mode=memory" and "cache=shared" ensures
		// all connections in the pool share the same in-memory database
		let db_url = "sqlite:file:memdb1?mode=memory&cache=shared";

		// Create backend with AnyPool
		use sqlx::any::AnyPoolOptions;

		let pool = AnyPoolOptions::new()
			.max_connections(5)
			.connect(db_url)
			.await
			.expect("Failed to connect to test database");

		let backend = DatabaseAuditBackend {
			pool: std::sync::Arc::new(pool),
			database_url: db_url.to_string(),
		};
		backend
			.init_tables()
			.await
			.expect("Failed to initialize tables");
		backend
	}

	#[tokio::test]
	async fn test_database_backend_init() {
		let backend = create_test_backend().await;

		// Verify tables were created
		let stmt = Query::select()
			.expr_as(
				Func::count(Expr::asterisk().into_simple_expr()),
				Alias::new("count"),
			)
			.from(Alias::new("audit_events"))
			.to_owned();
		let sql = stmt.to_string(SqliteQueryBuilder);

		let rows = sqlx::query(&sql)
			.fetch_all(backend.pool.as_ref())
			.await
			.unwrap();
		let count: i64 = rows[0].try_get("count").unwrap();

		assert_eq!(count, 0);
	}

	#[tokio::test]
	async fn test_database_backend_log_event() {
		let backend = create_test_backend().await;

		let mut changes = HashMap::new();
		changes.insert(
			"test_key".to_string(),
			ChangeRecord {
				old_value: Some(json!("old")),
				new_value: Some(json!("new")),
			},
		);

		let event = AuditEvent::new(
			EventType::ConfigUpdate,
			Some("test_user".to_string()),
			changes,
		);

		backend.log_event(event).await.unwrap();

		let events = backend.get_events(None).await.unwrap();
		assert_eq!(events.len(), 1);
		assert_eq!(events[0].event_type, EventType::ConfigUpdate);
	}

	#[tokio::test]
	async fn test_database_backend_filter_by_type() {
		let backend = create_test_backend().await;

		for i in 0..5 {
			let event_type = if i % 2 == 0 {
				EventType::ConfigUpdate
			} else {
				EventType::ConfigCreate
			};

			let mut changes = HashMap::new();
			changes.insert(
				format!("key_{}", i),
				ChangeRecord {
					old_value: None,
					new_value: Some(json!(format!("value_{}", i))),
				},
			);

			let event = AuditEvent::new(event_type, Some("user".to_string()), changes);
			backend.log_event(event).await.unwrap();
		}

		let filter = EventFilter {
			event_type: Some(EventType::ConfigUpdate),
			user: None,
			start_time: None,
			end_time: None,
		};

		let update_events = backend.get_events(Some(filter)).await.unwrap();
		assert_eq!(update_events.len(), 3);
	}

	#[tokio::test]
	async fn test_database_backend_filter_by_user() {
		let backend = create_test_backend().await;

		for i in 0..5 {
			let user = if i % 2 == 0 { "alice" } else { "bob" };

			let mut changes = HashMap::new();
			changes.insert(
				format!("key_{}", i),
				ChangeRecord {
					old_value: None,
					new_value: Some(json!(i)),
				},
			);

			let event = AuditEvent::new(EventType::ConfigUpdate, Some(user.to_string()), changes);
			backend.log_event(event).await.unwrap();
		}

		let filter = EventFilter {
			event_type: None,
			user: Some("alice".to_string()),
			start_time: None,
			end_time: None,
		};

		let alice_events = backend.get_events(Some(filter)).await.unwrap();
		assert_eq!(alice_events.len(), 3);
	}
}

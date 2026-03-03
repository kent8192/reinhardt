//! Schema Editor for migration execution
//!
//! Provides atomic transaction support for DDL operations,
//! similar to Django's schema_editor.
//!
//! # Overview
//!
//! The `SchemaEditor` wraps database connections and optionally manages
//! transactions for atomic migration execution. It follows Django's pattern
//! where migrations can be wrapped in transactions for databases that support
//! transactional DDL.
//!
//! # Database Support
//!
//! | Database | Transactional DDL | Notes |
//! |----------|-------------------|-------|
//! | PostgreSQL | Yes | DDL can be rolled back |
//! | SQLite | Yes | DDL can be rolled back |
//! | MySQL | No | DDL causes implicit commit |
//!
//! # Example
//!
//! ```ignore
//! use reinhardt_db::migrations::schema_editor::SchemaEditor;
//! use reinhardt_db::backends::{DatabaseConnection, DatabaseType};
//!
//! let connection = DatabaseConnection::connect_postgres("postgres://...").await?;
//! let mut editor = SchemaEditor::new(connection.clone(), true, DatabaseType::Postgres).await?;
//!
//! editor.execute("CREATE TABLE users (id SERIAL PRIMARY KEY)").await?;
//! editor.execute("ALTER TABLE users ADD COLUMN name TEXT").await?;
//!
//! // Commit all changes atomically
//! editor.finish().await?;
//! ```

use super::Result;
use crate::backends::{
	connection::DatabaseConnection,
	types::{DatabaseType, TransactionExecutor},
};

/// Schema editor for executing DDL statements with optional transaction support
///
/// This struct wraps a database connection and optionally manages a transaction
/// for atomic migration execution. It follows Django's schema_editor pattern.
///
/// When `atomic` is `true` and the database supports transactional DDL,
/// all DDL operations are wrapped in a transaction that can be committed
/// or rolled back as a unit.
pub struct SchemaEditor {
	/// Database connection
	connection: DatabaseConnection,
	/// Transaction executor (if using atomic mode)
	executor: Option<Box<dyn TransactionExecutor>>,
	/// Whether this editor is using atomic transactions
	atomic: bool,
	/// Database type
	db_type: DatabaseType,
	/// Deferred SQL statements to execute at finish
	deferred_sql: Vec<String>,
}

impl SchemaEditor {
	/// Create a new schema editor
	///
	/// If `atomic` is true and the database supports transactional DDL,
	/// a transaction will be started automatically.
	///
	/// # Arguments
	///
	/// * `connection` - Database connection to use
	/// * `atomic` - Whether to wrap operations in a transaction
	/// * `db_type` - Database type for dialect-specific handling
	///
	/// # Returns
	///
	/// A new SchemaEditor instance
	///
	/// # Notes
	///
	/// If `atomic` is `true` but the database doesn't support transactional DDL
	/// (e.g., MySQL), a warning is logged and operations proceed without
	/// transaction wrapping.
	pub async fn new(
		connection: DatabaseConnection,
		atomic: bool,
		db_type: DatabaseType,
	) -> Result<Self> {
		let effective_atomic = atomic && db_type.supports_transactional_ddl();

		let executor = if effective_atomic {
			Some(connection.begin().await?)
		} else {
			if atomic && !db_type.supports_transactional_ddl() {
				tracing::warn!(
					"atomic=true requested but {:?} doesn't support transactional DDL. \
					 Proceeding without transaction wrapper.",
					db_type
				);
			}
			None
		};

		Ok(Self {
			connection,
			executor,
			atomic: effective_atomic,
			db_type,
			deferred_sql: Vec::new(),
		})
	}

	/// Execute a DDL statement
	///
	/// If in atomic mode, executes within the transaction.
	/// Otherwise, executes directly on the connection.
	///
	/// # Arguments
	///
	/// * `sql` - SQL statement to execute
	pub async fn execute(&mut self, sql: &str) -> Result<()> {
		if let Some(ref mut tx) = self.executor {
			tx.execute(sql, vec![]).await?;
			// SQLite requires a schema cache refresh after DDL within a transaction
			// to prevent SQLITE_SCHEMA (code 262) errors on subsequent DDL statements.
			if self.db_type == DatabaseType::Sqlite {
				tx.execute("SELECT 1", vec![]).await?;
			}
		} else {
			self.connection.execute(sql, vec![]).await?;
		}

		Ok(())
	}

	/// Defer SQL execution until finish()
	///
	/// Some operations need to be executed after all other operations
	/// in the migration (e.g., creating indexes on newly created columns).
	///
	/// # Arguments
	///
	/// * `sql` - SQL statement to defer
	pub fn defer(&mut self, sql: String) {
		self.deferred_sql.push(sql);
	}

	/// Finish the schema editing session
	///
	/// Executes any deferred SQL and commits the transaction if atomic.
	///
	/// # Returns
	///
	/// Ok(()) on success
	pub async fn finish(mut self) -> Result<()> {
		// Execute deferred SQL
		for sql in self.deferred_sql.drain(..) {
			if let Some(ref mut tx) = self.executor {
				tx.execute(&sql, vec![]).await?;
			} else {
				self.connection.execute(&sql, vec![]).await?;
			}
		}

		// Commit if in transaction
		if let Some(tx) = self.executor.take() {
			tx.commit().await?;
		}

		Ok(())
	}

	/// Rollback any changes (only effective for transactional DDL databases)
	///
	/// For databases that don't support transactional DDL (MySQL),
	/// this is a no-op as DDL statements have already been implicitly committed.
	pub async fn rollback(mut self) -> Result<()> {
		if let Some(tx) = self.executor.take() {
			tx.rollback().await?;
		}
		Ok(())
	}

	/// Check if this editor is using atomic transactions
	pub fn is_atomic(&self) -> bool {
		self.atomic
	}

	/// Get the database type
	pub fn database_type(&self) -> DatabaseType {
		self.db_type
	}

	/// Get a reference to the underlying connection
	///
	/// This can be used for operations that need direct connection access
	/// outside of the transaction (e.g., checking table existence).
	pub fn connection(&self) -> &DatabaseConnection {
		&self.connection
	}

	/// Disable foreign key checks (SQLite only)
	///
	/// This must be called BEFORE any table recreation operations that might
	/// temporarily break foreign key relationships. Remember to re-enable
	/// foreign keys after the operation completes.
	///
	/// # SQLite Foreign Key Handling
	///
	/// SQLite table recreation temporarily drops the original table, which
	/// can cause foreign key violations. This method disables foreign key
	/// enforcement during the recreation process.
	///
	/// # Returns
	///
	/// Ok(()) if successful, or an error if the operation fails.
	/// Returns Ok(()) immediately for non-SQLite databases.
	#[cfg(feature = "sqlite")]
	pub async fn disable_foreign_keys(&mut self) -> Result<()> {
		if !matches!(self.db_type, DatabaseType::Sqlite) {
			return Ok(());
		}

		tracing::debug!("Disabling SQLite foreign key checks");
		self.execute("PRAGMA foreign_keys = OFF").await?;
		Ok(())
	}

	/// Enable foreign key checks (SQLite only)
	///
	/// This should be called AFTER table recreation operations complete
	/// to restore foreign key enforcement.
	///
	/// # Returns
	///
	/// Ok(()) if successful, or an error if the operation fails.
	/// Returns Ok(()) immediately for non-SQLite databases.
	#[cfg(feature = "sqlite")]
	pub async fn enable_foreign_keys(&mut self) -> Result<()> {
		if !matches!(self.db_type, DatabaseType::Sqlite) {
			return Ok(());
		}

		tracing::debug!("Enabling SQLite foreign key checks");
		self.execute("PRAGMA foreign_keys = ON").await?;
		Ok(())
	}

	/// Check foreign key integrity (SQLite only)
	///
	/// This should be called after table recreation to verify that all
	/// foreign key relationships are valid. If violations are found,
	/// they will be returned as a vector of violation descriptions.
	///
	/// # Returns
	///
	/// A vector of foreign key violation descriptions (empty if no violations).
	/// Returns an empty vector immediately for non-SQLite databases.
	#[cfg(feature = "sqlite")]
	pub async fn check_foreign_key_integrity(&mut self) -> Result<Vec<String>> {
		if !matches!(self.db_type, DatabaseType::Sqlite) {
			return Ok(Vec::new());
		}

		tracing::debug!("Checking SQLite foreign key integrity");

		// PRAGMA foreign_key_check returns rows with:
		// table, rowid, parent_table, fkid
		let sql = "PRAGMA foreign_key_check";
		let rows = if let Some(ref mut tx) = self.executor {
			tx.fetch_all(sql, vec![]).await?
		} else {
			self.connection.fetch_all(sql, vec![]).await?
		};

		let violations: Vec<String> = rows
			.into_iter()
			.map(|row| {
				// PRAGMA foreign_key_check returns: table, rowid, parent, fkid
				let table: String = row.get("table").unwrap_or_default();
				let rowid: i64 = row.get("rowid").unwrap_or_default();
				let parent_table: String = row.get("parent").unwrap_or_default();
				format!(
					"FK violation in '{}' row {} referencing '{}'",
					table, rowid, parent_table
				)
			})
			.collect();

		if !violations.is_empty() {
			tracing::warn!("Foreign key violations found: {:?}", violations);
		}

		Ok(violations)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_database_type_transactional_ddl() {
		assert!(DatabaseType::Postgres.supports_transactional_ddl());
		assert!(DatabaseType::Sqlite.supports_transactional_ddl());
		assert!(!DatabaseType::Mysql.supports_transactional_ddl());
	}
}

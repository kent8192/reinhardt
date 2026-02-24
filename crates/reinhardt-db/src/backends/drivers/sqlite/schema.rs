//! SQLite schema editor implementation
//!
//! This module provides SQLite-specific DDL operations through the `SQLiteSchemaEditor`.
//!
//! Note: SQLite has limited ALTER TABLE support. Some operations require table recreation.

use crate::backends::schema::{BaseDatabaseSchemaEditor, SchemaEditorError, SchemaEditorResult};

/// Quote SQLite identifier (double-quote escaping)
fn quote_sqlite_identifier(name: &str) -> String {
	format!("\"{}\"", name.replace('"', "\"\""))
}

/// SQLite schema editor for DDL operations
///
/// Implements the `BaseDatabaseSchemaEditor` trait with SQLite-specific SQL syntax.
///
/// # SQLite Limitations
///
/// SQLite has limited ALTER TABLE support:
/// - Cannot ALTER COLUMN (requires table recreation)
/// - Cannot DROP CONSTRAINT (requires table recreation)
/// - Limited RENAME COLUMN support (added in SQLite 3.25.0)
///
/// # Example
///
/// ```rust
/// # use reinhardt_db::backends::drivers::sqlite::schema::SQLiteSchemaEditor;
/// let editor = SQLiteSchemaEditor::new();
/// let sql = editor.rename_table_sql("users", "people");
/// assert!(sql.contains("ALTER TABLE"));
/// assert!(sql.contains("\"users\""));
/// assert!(sql.contains("\"people\""));
/// ```
#[derive(Debug, Default, Clone)]
pub struct SQLiteSchemaEditor;

impl SQLiteSchemaEditor {
	/// Create a new SQLite schema editor
	///
	/// # Example
	///
	/// ```rust
	/// # use reinhardt_db::backends::drivers::sqlite::schema::SQLiteSchemaEditor;
	/// let editor = SQLiteSchemaEditor::new();
	/// # drop(editor); // Verify it's creatable
	/// ```
	pub fn new() -> Self {
		Self
	}

	/// Generate ALTER COLUMN SQL (SQLite-specific note)
	///
	/// SQLite does not support ALTER COLUMN directly. Table recreation is required.
	pub fn alter_column_note(&self, table_name: &str) -> String {
		format!(
			"-- SQLite does not support ALTER COLUMN, table recreation required for {}",
			quote_sqlite_identifier(table_name)
		)
	}

	/// Generate RENAME TABLE SQL (SQLite-specific)
	pub fn rename_table_sql(&self, old_name: &str, new_name: &str) -> String {
		format!(
			"ALTER TABLE {} RENAME TO {}",
			quote_sqlite_identifier(old_name),
			quote_sqlite_identifier(new_name)
		)
	}

	/// Generate ADD CONSTRAINT note (SQLite-specific)
	///
	/// SQLite does not support ADD CONSTRAINT. Table recreation is required.
	pub fn add_constraint_note(&self, table_name: &str) -> String {
		format!(
			"-- SQLite does not support ADD CONSTRAINT, table recreation required for {}",
			quote_sqlite_identifier(table_name)
		)
	}

	/// Generate DROP CONSTRAINT note (SQLite-specific)
	///
	/// SQLite does not support DROP CONSTRAINT. Table recreation is required.
	pub fn drop_constraint_note(&self, table_name: &str) -> String {
		format!(
			"-- SQLite does not support DROP CONSTRAINT, table recreation required for {}",
			quote_sqlite_identifier(table_name)
		)
	}
}

#[async_trait::async_trait]
impl BaseDatabaseSchemaEditor for SQLiteSchemaEditor {
	fn database_type(&self) -> crate::backends::types::DatabaseType {
		crate::backends::types::DatabaseType::Sqlite
	}

	async fn execute(&mut self, _sql: &str) -> SchemaEditorResult<()> {
		Err(SchemaEditorError::ExecutionError(
			"Execution not supported in schema editor".to_string(),
		))
	}

	/// Override ALTER COLUMN statement for SQLite
	///
	/// SQLite does not support `ALTER COLUMN TYPE` or `ALTER COLUMN SET NOT NULL` directly.
	/// This is a fundamental limitation of SQLite's architecture.
	///
	/// ## Required Steps for Column Modification
	///
	/// Column type changes require a 12-step table recreation process:
	///
	/// 1. Begin a transaction
	/// 2. Disable foreign key checks: `PRAGMA foreign_keys = OFF`
	/// 3. Create new table with desired schema: `CREATE TABLE "new_table" (...)`
	/// 4. Copy data: `INSERT INTO "new_table" SELECT ... FROM "old_table"`
	/// 5. Drop old table: `DROP TABLE "old_table"`
	/// 6. Rename new table: `ALTER TABLE "new_table" RENAME TO "old_table"`
	/// 7. Recreate indexes
	/// 8. Recreate triggers
	/// 9. Re-enable foreign key checks: `PRAGMA foreign_keys = ON`
	/// 10. Verify foreign key integrity: `PRAGMA foreign_key_check`
	/// 11. Commit transaction
	/// 12. Vacuum database (optional, reclaims space)
	///
	/// ## Recommendation
	///
	/// Use the Reinhardt migration system (`reinhardt-migrations` crate) which handles
	/// this process automatically when column modifications are detected. The migration
	/// system generates the appropriate operations and handles data preservation.
	///
	/// ## Return Value
	///
	/// This method returns a SQL comment indicating that table recreation is required,
	/// serving as a clear indicator that this operation needs special handling.
	fn alter_column_statement(&self, table: &str, _column: &str, _new_type: &str) -> String {
		self.alter_column_note(table)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_alter_column_note() {
		let editor = SQLiteSchemaEditor::new();
		let sql = editor.alter_column_note("users");
		assert!(sql.contains("SQLite does not support ALTER COLUMN"));
		assert!(sql.contains("\"users\""));
	}

	#[test]
	fn test_alter_column_statement() {
		use crate::backends::schema::BaseDatabaseSchemaEditor;

		let editor = SQLiteSchemaEditor::new();
		// Test trait method override (returns SQL comment for SQLite)
		let sql = editor.alter_column_statement("users", "email", "TEXT");
		assert!(sql.contains("SQLite does not support ALTER COLUMN"));
		assert!(sql.contains("table recreation required"));
		assert!(sql.contains("\"users\""));
	}

	#[test]
	fn test_rename_table_sql() {
		let editor = SQLiteSchemaEditor::new();
		let sql = editor.rename_table_sql("users", "people");
		assert_eq!(sql, "ALTER TABLE \"users\" RENAME TO \"people\"");
	}

	#[test]
	fn test_add_constraint_note() {
		let editor = SQLiteSchemaEditor::new();
		let sql = editor.add_constraint_note("users");
		assert!(sql.contains("SQLite does not support ADD CONSTRAINT"));
		assert!(sql.contains("\"users\""));
	}

	#[test]
	fn test_drop_constraint_note() {
		let editor = SQLiteSchemaEditor::new();
		let sql = editor.drop_constraint_note("users");
		assert!(sql.contains("SQLite does not support DROP CONSTRAINT"));
		assert!(sql.contains("\"users\""));
	}
}

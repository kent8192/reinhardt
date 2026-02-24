//! MySQL schema editor implementation
//!
//! This module provides MySQL-specific DDL operations through the `MySQLSchemaEditor`.

use crate::backends::schema::{BaseDatabaseSchemaEditor, SchemaEditorError, SchemaEditorResult};

/// Quote MySQL identifier (backtick escaping)
fn quote_mysql_identifier(name: &str) -> String {
	format!("`{}`", name.replace('`', "``"))
}

/// MySQL schema editor for DDL operations
///
/// Implements the `BaseDatabaseSchemaEditor` trait with MySQL-specific SQL syntax.
///
/// # Example
///
/// ```rust
/// # use reinhardt_db::backends::drivers::mysql::schema::MySQLSchemaEditor;
/// let editor = MySQLSchemaEditor::new();
/// let sql = editor.rename_table_sql("users", "people");
/// assert!(sql.contains("ALTER TABLE"));
/// assert!(sql.contains("`users`"));
/// assert!(sql.contains("`people`"));
/// ```
#[derive(Debug, Default, Clone)]
pub struct MySQLSchemaEditor;

impl MySQLSchemaEditor {
	/// Create a new MySQL schema editor
	///
	/// # Example
	///
	/// ```rust
	/// # use reinhardt_db::backends::drivers::mysql::schema::MySQLSchemaEditor;
	/// let editor = MySQLSchemaEditor::new();
	/// # drop(editor); // Verify it's creatable
	/// ```
	pub fn new() -> Self {
		Self
	}

	/// Generate ALTER COLUMN SQL (MySQL-specific)
	///
	/// MySQL uses MODIFY COLUMN for altering columns
	pub fn alter_column_sql(&self, table_name: &str, column_name: &str, new_type: &str) -> String {
		format!(
			"ALTER TABLE {} MODIFY COLUMN {} {}",
			quote_mysql_identifier(table_name),
			quote_mysql_identifier(column_name),
			new_type
		)
	}

	/// Generate RENAME TABLE SQL (MySQL-specific)
	pub fn rename_table_sql(&self, old_name: &str, new_name: &str) -> String {
		format!(
			"ALTER TABLE {} RENAME TO {}",
			quote_mysql_identifier(old_name),
			quote_mysql_identifier(new_name)
		)
	}

	/// Generate ADD CONSTRAINT SQL (MySQL-specific)
	pub fn add_constraint_sql(&self, table_name: &str, constraint_sql: &str) -> String {
		format!(
			"ALTER TABLE {} ADD {}",
			quote_mysql_identifier(table_name),
			constraint_sql
		)
	}

	/// Generate DROP CONSTRAINT SQL (MySQL-specific)
	pub fn drop_constraint_sql(&self, table_name: &str, constraint_name: &str) -> String {
		format!(
			"ALTER TABLE {} DROP CONSTRAINT {}",
			quote_mysql_identifier(table_name),
			quote_mysql_identifier(constraint_name)
		)
	}
}

#[async_trait::async_trait]
impl BaseDatabaseSchemaEditor for MySQLSchemaEditor {
	fn database_type(&self) -> crate::backends::types::DatabaseType {
		crate::backends::types::DatabaseType::Mysql
	}

	async fn execute(&mut self, _sql: &str) -> SchemaEditorResult<()> {
		Err(SchemaEditorError::ExecutionError(
			"Execution not supported in schema editor".to_string(),
		))
	}

	/// Override ALTER COLUMN statement for MySQL syntax
	///
	/// MySQL uses `ALTER TABLE table MODIFY COLUMN column type` instead of
	/// PostgreSQL's `ALTER TABLE table ALTER COLUMN column TYPE type`.
	///
	/// Uses backtick (`) for identifier quoting instead of double quotes.
	fn alter_column_statement(&self, table: &str, column: &str, new_type: &str) -> String {
		self.alter_column_sql(table, column, new_type)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_alter_column_sql() {
		let editor = MySQLSchemaEditor::new();
		let sql = editor.alter_column_sql("users", "email", "TEXT");
		assert_eq!(sql, "ALTER TABLE `users` MODIFY COLUMN `email` TEXT");
	}

	#[test]
	fn test_alter_column_statement() {
		use crate::backends::schema::BaseDatabaseSchemaEditor;

		let editor = MySQLSchemaEditor::new();
		// Test trait method override
		let sql = editor.alter_column_statement("users", "email", "TEXT");
		assert_eq!(sql, "ALTER TABLE `users` MODIFY COLUMN `email` TEXT");

		// Verify MySQL-specific syntax (MODIFY COLUMN, not ALTER COLUMN)
		assert!(sql.contains("MODIFY COLUMN"));
		assert!(!sql.contains("ALTER COLUMN"));
	}

	#[test]
	fn test_rename_table_sql() {
		let editor = MySQLSchemaEditor::new();
		let sql = editor.rename_table_sql("users", "people");
		assert_eq!(sql, "ALTER TABLE `users` RENAME TO `people`");
	}

	#[test]
	fn test_add_constraint_sql() {
		let editor = MySQLSchemaEditor::new();
		let sql = editor.add_constraint_sql("users", "UNIQUE (email)");
		assert_eq!(sql, "ALTER TABLE `users` ADD UNIQUE (email)");
	}

	#[test]
	fn test_drop_constraint_sql() {
		let editor = MySQLSchemaEditor::new();
		let sql = editor.drop_constraint_sql("users", "unique_email");
		assert_eq!(sql, "ALTER TABLE `users` DROP CONSTRAINT `unique_email`");
	}
}

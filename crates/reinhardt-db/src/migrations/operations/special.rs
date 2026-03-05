//! Special operations for migrations
//!
//! This module provides special operations like RunSQL and RunRust,
//! inspired by Django's `django/db/migrations/operations/special.py`.
//!
//! # Example
//!
//! ```rust
//! use reinhardt_db::migrations::operations::special::RunSQL;
//!
//! // Create a RunSQL operation
//! let run_sql = RunSQL::new(
//!     "INSERT INTO users (name, email) VALUES ('admin', 'admin@example.com')",
//! ).with_reverse_sql("DELETE FROM users WHERE email = 'admin@example.com'");
//!
//! // Get forward SQL
//! assert_eq!(run_sql.sql, "INSERT INTO users (name, email) VALUES ('admin', 'admin@example.com')");
//! ```

use crate::backends::connection::DatabaseConnection;
use crate::backends::schema::BaseDatabaseSchemaEditor;
use crate::migrations::ProjectState;
use serde::{Deserialize, Serialize};

/// Execute raw SQL
///
/// This operation allows you to execute arbitrary SQL statements during migration.
/// It's useful for data migrations, custom schema modifications, or database-specific operations.
///
/// # Example
///
/// ```rust
/// use reinhardt_db::migrations::operations::special::RunSQL;
///
/// // Simple SQL execution
/// let sql = RunSQL::new("CREATE INDEX idx_email ON users(email)");
///
/// // With reverse SQL for rollback
/// let sql_reversible = RunSQL::new("CREATE INDEX idx_email ON users(email)")
///     .with_reverse_sql("DROP INDEX idx_email");
///
/// // Multiple statements
/// let multi_sql = RunSQL::new_multi(vec![
///     "INSERT INTO roles (name) VALUES ('admin')".to_string(),
///     "INSERT INTO roles (name) VALUES ('user')".to_string(),
/// ]);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSQL {
	pub sql: String,
	pub reverse_sql: Option<String>,
	pub state_operations: Vec<StateOperation>,
}

impl RunSQL {
	/// Create a new RunSQL operation
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::operations::special::RunSQL;
	///
	/// let sql = RunSQL::new("INSERT INTO config (key, value) VALUES ('version', '1.0')");
	/// assert!(sql.reverse_sql.is_none());
	/// ```
	pub fn new(sql: impl Into<String>) -> Self {
		Self {
			sql: sql.into(),
			reverse_sql: None,
			state_operations: vec![],
		}
	}

	/// Create a RunSQL operation with multiple statements
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::operations::special::RunSQL;
	///
	/// let sql = RunSQL::new_multi(vec![
	///     "UPDATE users SET active = true WHERE id = 1".to_string(),
	///     "UPDATE users SET active = false WHERE id = 2".to_string(),
	/// ]);
	///
	/// assert!(sql.sql.contains("UPDATE users SET active = true"));
	/// assert!(sql.sql.contains("UPDATE users SET active = false"));
	/// ```
	pub fn new_multi(statements: Vec<String>) -> Self {
		Self {
			sql: statements.join(";\n"),
			reverse_sql: None,
			state_operations: vec![],
		}
	}

	/// Set reverse SQL for rollback
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::operations::special::RunSQL;
	///
	/// let sql = RunSQL::new("CREATE INDEX idx_email ON users(email)")
	///     .with_reverse_sql("DROP INDEX idx_email");
	///
	/// assert!(sql.reverse_sql.is_some());
	/// assert_eq!(sql.reverse_sql.unwrap(), "DROP INDEX idx_email");
	/// ```
	pub fn with_reverse_sql(mut self, reverse_sql: impl Into<String>) -> Self {
		self.reverse_sql = Some(reverse_sql.into());
		self
	}

	/// Add state operations to be applied along with the SQL
	///
	/// This allows you to keep the project state in sync when running custom SQL.
	pub fn with_state_operations(mut self, operations: Vec<StateOperation>) -> Self {
		self.state_operations = operations;
		self
	}

	/// Apply to project state (forward)
	///
	/// RunSQL doesn't modify state by default unless state_operations are specified
	pub fn state_forwards(&self, app_label: &str, state: &mut ProjectState) {
		for op in &self.state_operations {
			op.apply(app_label, state);
		}
	}

	/// Generate SQL
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_db::migrations::operations::special::RunSQL;
	/// use reinhardt_db::backends::schema::factory::{SchemaEditorFactory, DatabaseType};
	///
	/// let sql = RunSQL::new("SELECT 1");
	/// let factory = SchemaEditorFactory::new();
	/// let editor = factory.create_for_database(DatabaseType::PostgreSQL);
	///
	/// let statements = sql.database_forwards(editor.as_ref());
	/// assert_eq!(statements.len(), 1);
	/// assert_eq!(statements[0], "SELECT 1");
	/// ```
	pub fn database_forwards(&self, _schema_editor: &dyn BaseDatabaseSchemaEditor) -> Vec<String> {
		vec![self.sql.clone()]
	}

	/// Get reverse SQL for rollback
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::operations::special::RunSQL;
	///
	/// let sql = RunSQL::new("CREATE TABLE temp (id INT)")
	///     .with_reverse_sql("DROP TABLE temp");
	///
	/// assert_eq!(sql.get_reverse_sql(), Some("DROP TABLE temp"));
	///
	/// let irreversible = RunSQL::new("DROP TABLE important_data");
	/// assert_eq!(irreversible.get_reverse_sql(), None);
	/// ```
	pub fn get_reverse_sql(&self) -> Option<&str> {
		self.reverse_sql.as_deref()
	}
}

/// State operation to apply alongside SQL
///
/// This allows RunSQL to update the project state appropriately
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StateOperation {
	AddModel { name: String },
	RemoveModel { name: String },
	AddField { model: String, field: String },
	RemoveField { model: String, field: String },
}

impl StateOperation {
	fn apply(&self, app_label: &str, state: &mut ProjectState) {
		match self {
			StateOperation::AddModel { .. } => {
				// Would need model definition
			}
			StateOperation::RemoveModel { name } => {
				state.remove_model(app_label, name);
			}
			StateOperation::AddField { .. } => {
				// Would need field definition
			}
			StateOperation::RemoveField { model, field } => {
				if let Some(model_state) = state.get_model_mut(app_label, model) {
					model_state.remove_field(field);
				}
			}
		}
	}
}

/// Execute Rust code during migration
///
/// This is the Rust equivalent of Django's RunPython operation. It allows you to execute
/// arbitrary Rust code during migration, useful for data transformations.
///
/// # Example
///
/// ```rust
/// use reinhardt_db::migrations::operations::special::RunCode;
/// use reinhardt_db::backends::connection::DatabaseConnection;
///
/// // Create a code operation with description
/// let code = RunCode::new("Update user emails", |connection| {
///     // Access database through connection
///     println!("Updating emails...");
///     Ok(())
/// });
/// ```
pub struct RunCode {
	pub description: String,
	#[allow(clippy::type_complexity)]
	pub code: Box<dyn Fn(&DatabaseConnection) -> Result<(), String> + Send + Sync>,
	#[allow(clippy::type_complexity)]
	pub reverse_code: Option<Box<dyn Fn(&DatabaseConnection) -> Result<(), String> + Send + Sync>>,
}

impl std::fmt::Debug for RunCode {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("RunCode")
			.field("description", &self.description)
			.field("has_reverse", &self.reverse_code.is_some())
			.finish()
	}
}

impl RunCode {
	/// Create a new RunCode operation
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::operations::special::RunCode;
	///
	/// let code = RunCode::new("Update user emails", |connection| {
	///     // Database operations using connection
	///     Ok(())
	/// });
	/// ```
	pub fn new<F>(description: impl Into<String>, code: F) -> Self
	where
		F: Fn(&DatabaseConnection) -> Result<(), String> + Send + Sync + 'static,
	{
		Self {
			description: description.into(),
			code: Box::new(code),
			reverse_code: None,
		}
	}

	/// Set reverse code for rollback
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::operations::special::RunCode;
	///
	/// let code = RunCode::new("Update emails", |connection| Ok(()))
	///     .with_reverse_code(|connection| {
	///         // Rollback logic
	///         Ok(())
	///     });
	/// ```
	pub fn with_reverse_code<F>(mut self, reverse: F) -> Self
	where
		F: Fn(&DatabaseConnection) -> Result<(), String> + Send + Sync + 'static,
	{
		self.reverse_code = Some(Box::new(reverse));
		self
	}

	/// Execute the code
	///
	/// # Example
	///
	/// ```no_run
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// use reinhardt_db::migrations::operations::special::RunCode;
	/// use reinhardt_db::backends::connection::DatabaseConnection;
	///
	/// let code = RunCode::new("Migrate data", |connection| Ok(()));
	/// let connection = DatabaseConnection::connect_postgres("postgres://localhost/db").await?;
	/// code.execute(&connection)?;
	/// # Ok(())
	/// # }
	/// ```
	pub fn execute(&self, connection: &DatabaseConnection) -> Result<(), String> {
		(self.code)(connection)
	}

	/// Execute reverse code
	///
	/// # Example
	///
	/// ```no_run
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// use reinhardt_db::migrations::operations::special::RunCode;
	/// use reinhardt_db::backends::connection::DatabaseConnection;
	///
	/// let code = RunCode::new("Migrate data", |_| Ok(()))
	///     .with_reverse_code(|_| Ok(()));
	///
	/// let connection = DatabaseConnection::connect_postgres("postgres://localhost/db").await?;
	/// code.execute_reverse(&connection)?;
	/// # Ok(())
	/// # }
	/// ```
	pub fn execute_reverse(&self, connection: &DatabaseConnection) -> Result<(), String> {
		if let Some(reverse) = &self.reverse_code {
			reverse(connection)
		} else {
			Err("This operation is not reversible".to_string())
		}
	}

	/// Apply to project state (forward)
	///
	/// RunCode doesn't modify state by default
	pub fn state_forwards(&self, _app_label: &str, _state: &mut ProjectState) {
		// Custom code operations don't modify state
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::migrations::FieldType;

	#[test]
	fn test_run_sql_basic() {
		let sql = RunSQL::new("CREATE INDEX idx_email ON users(email)");
		assert_eq!(sql.sql, "CREATE INDEX idx_email ON users(email)");
		assert!(sql.reverse_sql.is_none());
	}

	#[test]
	fn test_run_sql_with_reverse() {
		let sql = RunSQL::new("CREATE INDEX idx_email ON users(email)")
			.with_reverse_sql("DROP INDEX idx_email");

		assert_eq!(sql.sql, "CREATE INDEX idx_email ON users(email)");
		assert_eq!(sql.reverse_sql, Some("DROP INDEX idx_email".to_string()));
		assert_eq!(sql.get_reverse_sql(), Some("DROP INDEX idx_email"));
	}

	#[test]
	fn test_run_sql_multi() {
		let sql = RunSQL::new_multi(vec![
			"INSERT INTO roles (name) VALUES ('admin')".to_string(),
			"INSERT INTO roles (name) VALUES ('user')".to_string(),
		]);

		assert!(
			sql.sql
				.contains("INSERT INTO roles (name) VALUES ('admin')")
		);
		assert!(sql.sql.contains("INSERT INTO roles (name) VALUES ('user')"));
	}

	#[cfg(feature = "postgres")]
	#[test]
	fn test_run_sql_database_forwards() {
		use crate::backends::schema::test_utils::MockSchemaEditor;

		let sql = RunSQL::new("SELECT COUNT(*) FROM users");
		let editor = MockSchemaEditor::new();

		let statements = sql.database_forwards(&editor);
		assert_eq!(statements.len(), 1);
		assert_eq!(statements[0], "SELECT COUNT(*) FROM users");
	}

	#[test]
	fn test_run_code_basic() {
		let code = RunCode::new("Test migration", |_connection| Ok(()));
		assert_eq!(code.description, "Test migration");
		assert!(code.reverse_code.is_none());
	}

	#[test]
	fn test_run_code_with_reverse() {
		let code = RunCode::new("Test migration", |_connection| Ok(()))
			.with_reverse_code(|_connection| Ok(()));
		assert!(code.reverse_code.is_some());
	}

	#[test]
	fn test_state_operation_remove_model() {
		use crate::migrations::operations::FieldDefinition;
		use crate::migrations::operations::models::CreateModel;

		let mut state = ProjectState::new();

		// Create a model first
		let create = CreateModel::new(
			"User",
			vec![FieldDefinition::new(
				"id",
				FieldType::Integer,
				true,
				false,
				None::<String>,
			)],
		);
		create.state_forwards("myapp", &mut state);
		assert!(state.get_model("myapp", "User").is_some());

		// Remove it via state operation
		let op = StateOperation::RemoveModel {
			name: "User".to_string(),
		};
		op.apply("myapp", &mut state);
		assert!(state.get_model("myapp", "User").is_none());
	}
}

/// Complex data migration builder
///
/// Provides a fluent API for building complex data migrations with batching,
/// progress tracking, and error handling.
///
/// # Example
///
/// ```rust
/// use reinhardt_db::migrations::operations::special::DataMigration;
///
/// let migration = DataMigration::new("users", "Migrate user data")
///     .batch_size(1000)
///     .add_transformation("UPDATE users SET status = 'active' WHERE status IS NULL")
///     .add_transformation("UPDATE users SET created_at = NOW() WHERE created_at IS NULL");
/// ```
#[derive(Debug, Clone)]
pub struct DataMigration {
	/// Table name
	pub table: String,
	/// Migration description
	pub description: String,
	/// Batch size for processing
	pub batch_size: usize,
	/// SQL transformations to apply
	pub transformations: Vec<String>,
	/// Whether to use transactions
	pub use_transactions: bool,
}

impl DataMigration {
	/// Create a new data migration
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::operations::special::DataMigration;
	///
	/// let migration = DataMigration::new("users", "Update user statuses");
	/// assert_eq!(migration.table, "users");
	/// assert_eq!(migration.batch_size, 1000);
	/// ```
	pub fn new(table: impl Into<String>, description: impl Into<String>) -> Self {
		Self {
			table: table.into(),
			description: description.into(),
			batch_size: 1000,
			transformations: Vec::new(),
			use_transactions: true,
		}
	}

	/// Set batch size for processing
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::operations::special::DataMigration;
	///
	/// let migration = DataMigration::new("users", "Migrate data")
	///     .batch_size(500);
	/// assert_eq!(migration.batch_size, 500);
	/// ```
	pub fn batch_size(mut self, size: usize) -> Self {
		self.batch_size = size;
		self
	}

	/// Add a SQL transformation
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::operations::special::DataMigration;
	///
	/// let migration = DataMigration::new("users", "Clean data")
	///     .add_transformation("UPDATE users SET email = LOWER(email)");
	/// assert_eq!(migration.transformations.len(), 1);
	/// ```
	pub fn add_transformation(mut self, sql: impl Into<String>) -> Self {
		self.transformations.push(sql.into());
		self
	}

	/// Set whether to use transactions
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::operations::special::DataMigration;
	///
	/// let migration = DataMigration::new("users", "Migrate")
	///     .use_transactions(false);
	/// assert!(!migration.use_transactions);
	/// ```
	pub fn use_transactions(mut self, use_tx: bool) -> Self {
		self.use_transactions = use_tx;
		self
	}

	/// Generate batched SQL statements
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::operations::special::DataMigration;
	///
	/// let migration = DataMigration::new("users", "Update emails")
	///     .batch_size(100)
	///     .add_transformation("UPDATE users SET email = LOWER(email) WHERE id >= {start} AND id < {end}");
	///
	/// let statements = migration.generate_batched_sql(1000);
	/// assert_eq!(statements.len(), 10); // 1000 / 100 = 10 batches
	/// ```
	pub fn generate_batched_sql(&self, total_rows: usize) -> Vec<String> {
		let mut statements = Vec::new();
		let num_batches = total_rows.div_ceil(self.batch_size);

		for batch in 0..num_batches {
			let start = batch * self.batch_size;
			let end = ((batch + 1) * self.batch_size).min(total_rows);

			for transformation in &self.transformations {
				let sql = transformation
					.replace("{start}", &start.to_string())
					.replace("{end}", &end.to_string())
					.replace("{batch_size}", &self.batch_size.to_string())
					.replace("{table}", &self.table);

				statements.push(sql);
			}
		}

		statements
	}

	/// Convert to RunSQL operation
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::operations::special::DataMigration;
	///
	/// let migration = DataMigration::new("users", "Migrate")
	///     .add_transformation("UPDATE users SET status = 'active'");
	///
	/// let run_sql = migration.to_run_sql();
	/// assert!(run_sql.sql.contains("UPDATE users"));
	/// ```
	pub fn to_run_sql(&self) -> RunSQL {
		let sql = if self.use_transactions {
			format!("BEGIN;\n{}\nCOMMIT;", self.transformations.join(";\n"))
		} else {
			self.transformations.join(";\n")
		};

		RunSQL::new(sql)
	}
}

// MigrationOperation trait implementation for Django-style naming
use crate::migrations::operation_trait::MigrationOperation;

impl MigrationOperation for RunSQL {
	fn migration_name_fragment(&self) -> Option<String> {
		// Return "run_sql" to indicate this is a custom SQL operation
		Some("run_sql".to_string())
	}

	fn describe(&self) -> String {
		let preview = if self.sql.len() > 50 {
			format!("{}...", &self.sql[..50])
		} else {
			self.sql.clone()
		};
		format!("RunSQL: {}", preview)
	}
}

impl MigrationOperation for RunCode {
	fn migration_name_fragment(&self) -> Option<String> {
		// Return "run_code" to indicate this is a custom code operation
		Some("run_code".to_string())
	}

	fn describe(&self) -> String {
		"RunCode: Custom code execution".to_string()
	}
}

impl MigrationOperation for DataMigration {
	fn migration_name_fragment(&self) -> Option<String> {
		// Return "data_migration" to indicate this is a data transformation operation
		Some("data_migration".to_string())
	}

	fn describe(&self) -> String {
		format!("DataMigration: {}", self.description)
	}
}

#[cfg(test)]
mod data_migration_tests {
	use super::*;

	#[test]
	fn test_data_migration_creation() {
		let migration = DataMigration::new("users", "Migrate user data");
		assert_eq!(migration.table, "users");
		assert_eq!(migration.description, "Migrate user data");
		assert_eq!(migration.batch_size, 1000);
		assert!(migration.use_transactions);
	}

	#[test]
	fn test_data_migration_batch_size() {
		let migration = DataMigration::new("users", "Migrate").batch_size(500);
		assert_eq!(migration.batch_size, 500);
	}

	#[test]
	fn test_data_migration_add_transformation() {
		let migration = DataMigration::new("users", "Clean")
			.add_transformation("UPDATE users SET email = LOWER(email)")
			.add_transformation("UPDATE users SET name = TRIM(name)");

		assert_eq!(migration.transformations.len(), 2);
	}

	#[test]
	fn test_data_migration_use_transactions() {
		let migration = DataMigration::new("users", "Migrate").use_transactions(false);
		assert!(!migration.use_transactions);
	}

	#[test]
	fn test_generate_batched_sql() {
		let migration = DataMigration::new("users", "Update")
			.batch_size(100)
			.add_transformation(
				"UPDATE users SET processed = true WHERE id >= {start} AND id < {end}",
			);

		let statements = migration.generate_batched_sql(250);
		assert_eq!(statements.len(), 3); // 250 / 100 = 3 batches

		assert!(statements[0].contains("id >= 0 AND id < 100"));
		assert!(statements[1].contains("id >= 100 AND id < 200"));
		assert!(statements[2].contains("id >= 200 AND id < 250"));
	}

	#[test]
	fn test_to_run_sql_with_transactions() {
		let migration = DataMigration::new("users", "Migrate")
			.add_transformation("UPDATE users SET status = 'active'")
			.add_transformation("UPDATE users SET verified = true");

		let run_sql = migration.to_run_sql();
		assert!(run_sql.sql.contains("BEGIN"));
		assert!(run_sql.sql.contains("COMMIT"));
		assert!(run_sql.sql.contains("UPDATE users"));
	}

	#[test]
	fn test_to_run_sql_without_transactions() {
		let migration = DataMigration::new("users", "Migrate")
			.use_transactions(false)
			.add_transformation("UPDATE users SET status = 'active'");

		let run_sql = migration.to_run_sql();
		assert!(!run_sql.sql.contains("BEGIN"));
		assert!(!run_sql.sql.contains("COMMIT"));
	}
}

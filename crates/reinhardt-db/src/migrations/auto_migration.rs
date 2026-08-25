//! Auto-migration generation
//!
//! Automatically generates migration files from model definitions:
//! - Detects schema changes
//! - Generates migration operations
//! - Creates rollback scripts

use super::operations::Operation;
use super::operations::named_index_has_target;
use super::repository::MigrationRepository;
use super::schema_diff::{DatabaseSchema, SchemaDiff};
use std::path::PathBuf;
use std::sync::Arc;

/// Auto-migration generator
pub struct AutoMigrationGenerator {
	/// Target schema from models
	target_schema: DatabaseSchema,
	/// Repository for checking existing migrations
	repository: Arc<tokio::sync::Mutex<dyn MigrationRepository>>,
}

/// Auto-migration result
///
/// This structure contains the result of auto-migration generation.
/// Note that `migration_file` and `rollback_file` are placeholders and will be
/// populated by the caller (typically `MakeMigrationsCommand`) based on the
/// application's migration naming and numbering scheme.
///
/// # Design Rationale
///
/// The separation of concerns is intentional:
/// - `AutoMigrationGenerator::generate()` is responsible for detecting schema changes
///   and generating migration operations
/// - The caller (e.g., `MakeMigrationsCommand` or `MigrationService`) is responsible
///   for determining the actual file paths based on the application's naming conventions,
///   existing migration numbers, and file system structure
#[derive(Debug)]
pub struct AutoMigrationResult {
	/// Generated migration file path (placeholder)
	///
	/// This field is a placeholder and will be set by the caller.
	/// The caller determines the actual path based on:
	/// - Application name
	/// - Migration number (sequential)
	/// - Migration naming conventions
	pub migration_file: PathBuf,
	/// Rollback file path (placeholder)
	///
	/// This field is a placeholder and will be set by the caller if rollback
	/// file generation is enabled. The caller determines:
	/// - Whether to generate a rollback file
	/// - The rollback file path and naming convention
	pub rollback_file: Option<PathBuf>,
	/// Generated operations
	///
	/// List of migration operations detected from schema differences.
	/// These operations represent the changes needed to transform the current
	/// database schema to the target schema defined in models.
	pub operations: Vec<Operation>,
	/// Number of operations generated
	///
	/// Total count of migration operations. Used for reporting and validation.
	pub operation_count: usize,
	/// Has destructive changes
	///
	/// Indicates whether the migration contains potentially destructive operations
	/// such as table drops, column drops, or type changes that may result in data loss.
	/// The caller can use this flag to prompt for user confirmation before applying.
	pub has_destructive_changes: bool,
}

impl AutoMigrationGenerator {
	/// Create a new auto-migration generator
	///
	/// # Arguments
	///
	/// * `target_schema` - The target schema from model definitions
	/// * `repository` - Repository for checking existing migrations
	pub fn new(
		target_schema: DatabaseSchema,
		repository: Arc<tokio::sync::Mutex<dyn MigrationRepository>>,
	) -> Self {
		Self {
			target_schema,
			repository,
		}
	}

	/// Generate migration from current database state
	///
	/// This method is responsible for detecting schema differences between the current
	/// database state and the target schema defined in models, and generating the
	/// necessary migration operations to transform the schema.
	///
	/// # Design Responsibility
	///
	/// This method follows the single responsibility principle:
	/// - **This method's responsibility**: Detect schema changes and generate migration operations
	/// - **Caller's responsibility**: Determine file paths, migration numbering, and persist files
	///
	/// The returned `AutoMigrationResult` contains placeholder values for `migration_file` and
	/// `rollback_file`. The caller (typically `MakeMigrationsCommand` or `MigrationService`) must:
	/// 1. Determine the next migration number by examining existing migrations
	/// 2. Construct the migration file path following the naming convention (e.g., `0001_initial.rs`)
	/// 3. Decide whether to generate a rollback file based on configuration
	/// 4. Write the actual migration files to disk
	///
	/// # Arguments
	///
	/// * `app_label` - The app label for the migration (used for grouping and dependency resolution)
	/// * `current_schema` - Current database schema (obtained from database introspection)
	///
	/// # Returns
	///
	/// Returns `AutoMigrationResult` containing:
	/// - Generated migration operations
	/// - Placeholder file paths (to be set by caller)
	/// - Metadata (operation count, destructive changes flag)
	///
	/// # Errors
	///
	/// Returns `AutoMigrationError::NoChangesDetected` if no schema changes are detected
	/// between the current and target schemas.
	///
	/// Returns `AutoMigrationError::DuplicateMigration` if the generated operations are
	/// semantically identical to the last migration in the repository. This prevents
	/// accidental duplicate migration generation.
	///
	/// Returns `AutoMigrationError::WriteError` if repository access fails.
	///
	/// # Example Workflow
	///
	/// ```ignore
	/// // Caller (e.g., MakeMigrationsCommand)
	/// let result = generator.generate(app_label, current_schema).await?;
	///
	/// // Caller determines file paths
	/// let migration_number = determine_next_migration_number(app_label).await?;
	/// let migration_file = format!("migrations/{:04}_auto.rs", migration_number);
	/// let rollback_file = if config.enable_rollback {
	///     Some(format!("migrations/{:04}_auto_rollback.rs", migration_number))
	/// } else {
	///     None
	/// };
	///
	/// // Caller persists the migration
	/// write_migration_file(&migration_file, &result.operations).await?;
	/// ```
	pub async fn generate(
		&self,
		app_label: &str,
		current_schema: DatabaseSchema,
	) -> Result<AutoMigrationResult, AutoMigrationError> {
		// Detect schema differences
		let diff = SchemaDiff::new(current_schema.clone(), self.target_schema.clone());
		let operations = diff.generate_operations();

		if operations.is_empty() {
			return Err(AutoMigrationError::NoChangesDetected);
		}

		// Get existing migrations from repository
		let repo = self.repository.lock().await;
		let existing_migrations = repo
			.list(app_label)
			.await
			.map_err(|e| AutoMigrationError::WriteError(e.to_string()))?;

		// Check for duplicate operations (compare with last migration)
		if let Some(last_migration) = existing_migrations.last()
			&& self.is_duplicate_operations(&operations, &last_migration.operations)
		{
			return Err(AutoMigrationError::DuplicateMigration);
		}

		drop(repo);

		let has_destructive = diff.has_destructive_changes();

		Ok(AutoMigrationResult {
			// Placeholder - caller (e.g., MakeMigrationsCommand) determines actual path
			// based on app_label, migration number, and naming conventions
			migration_file: std::path::PathBuf::new(),
			// Placeholder - caller determines if rollback file is needed based on
			// configuration and decides the rollback file path
			rollback_file: None,
			operations: operations.clone(),
			operation_count: operations.len(),
			has_destructive_changes: has_destructive,
		})
	}

	/// Check if operations are duplicates
	///
	/// Compares operations using semantic equality, which normalizes order-independent
	/// elements before comparison. This allows detection of duplicate migrations even
	/// when the order of columns, constraints, or other elements differs.
	fn is_duplicate_operations(&self, ops1: &[Operation], ops2: &[Operation]) -> bool {
		use super::operation_trait::MigrationOperation;

		if ops1.len() != ops2.len() {
			return false;
		}

		// Semantic equality check using normalize() and comparison
		ops1.iter()
			.zip(ops2.iter())
			.all(|(op1, op2)| op1.semantically_equal(op2))
	}

	/// Generate rollback operations
	// Allow dead_code: rollback operation generation planned for future migration reversal feature
	#[allow(dead_code)]
	fn generate_rollback(&self, operations: &[Operation]) -> Vec<Operation> {
		operations
			.iter()
			.rev()
			.filter_map(|op| match op {
				// Table operations
				Operation::CreateTable { name, .. } => {
					Some(Operation::DropTable { name: name.clone() })
				}
				Operation::DropTable { .. } => None, // Cannot rollback - data is lost
				Operation::RenameTable { old_name, new_name } => Some(Operation::RenameTable {
					old_name: new_name.clone(),
					new_name: old_name.clone(),
				}),

				// Column operations
				Operation::AddColumn { table, column, .. } => Some(Operation::DropColumn {
					table: table.clone(),
					column: column.name.clone(),
					old_definition: None,
				}),
				Operation::DropColumn { .. } => None, // Cannot rollback - data is lost
				Operation::RenameColumn {
					table,
					old_name,
					new_name,
				} => Some(Operation::RenameColumn {
					table: table.clone(),
					old_name: new_name.clone(),
					new_name: old_name.clone(),
				}),
				Operation::AlterColumn { .. } => None, // Cannot safely rollback without old definition

				// Constraint operations
				Operation::AddConstraint { .. } => None, // Cannot rollback without constraint name
				Operation::AddConstraintDefinition { table, constraint } => {
					Some(Operation::DropConstraintDefinition {
						table: table.clone(),
						constraint: constraint.clone(),
					})
				}
				Operation::AddConstraintRepair { .. } => None,
				Operation::RestoreConstraintOnRollback {
					table,
					constraint_sql,
				} => Some(Operation::AddConstraint {
					table: table.clone(),
					constraint_sql: constraint_sql.clone(),
				}),
				Operation::DropConstraint { .. } => None, // Cannot rollback without constraint SQL
				Operation::DropConstraintDefinition { table, constraint } => {
					Some(Operation::AddConstraintDefinition {
						table: table.clone(),
						constraint: constraint.clone(),
					})
				}

				// Index operations
				Operation::CreateIndex {
					table,
					columns,
					unique,
					index_type,
					where_clause,
					concurrently,
					expressions,
					mysql_options,
					operator_class,
				} => Some(Operation::DropNamedIndex {
					table: table.clone(),
					name: super::operations::generated_index_name(
						table,
						columns,
						expressions.as_deref(),
					),
					columns: columns.clone(),
					unique: *unique,
					index_type: *index_type,
					where_clause: where_clause.clone(),
					concurrently: *concurrently,
					expressions: expressions.clone(),
					mysql_options: *mysql_options,
					operator_class: operator_class.clone(),
				}),
				#[cfg(feature = "pgvector")]
				Operation::CreateNamedIndex {
					table,
					name,
					columns,
					unique,
					index_type,
					where_clause,
					concurrently,
					expressions,
					mysql_options,
					operator_class,
				} => Some(Operation::DropNamedIndex {
					table: table.clone(),
					name: name.clone(),
					columns: columns.clone(),
					unique: *unique,
					index_type: *index_type,
					where_clause: where_clause.clone(),
					concurrently: *concurrently,
					expressions: expressions.clone(),
					mysql_options: *mysql_options,
					operator_class: operator_class.clone(),
				}),
				Operation::CreateIndexRepair { .. } => None,
				Operation::RestoreIndexOnRollback {
					table,
					name,
					columns,
					unique,
					index_type,
					where_clause,
					concurrently,
					expressions,
					mysql_options,
					operator_class,
				} => Some(Operation::CreateIndexRepair {
					table: table.clone(),
					name: name.clone(),
					columns: columns.clone(),
					unique: *unique,
					index_type: *index_type,
					where_clause: where_clause.clone(),
					concurrently: *concurrently,
					expressions: expressions.clone(),
					mysql_options: *mysql_options,
					operator_class: operator_class.clone(),
				}),
				Operation::DropIndex { .. } => None, // Cannot rollback without index definition
				#[cfg(feature = "pgvector")]
				Operation::DropNamedIndex {
					table,
					name,
					columns,
					unique,
					index_type,
					where_clause,
					concurrently,
					expressions,
					mysql_options,
					operator_class,
				} => named_index_has_target(columns, expressions.as_deref()).then(|| {
					Operation::CreateNamedIndex {
						table: table.clone(),
						name: name.clone(),
						columns: columns.clone(),
						unique: *unique,
						index_type: *index_type,
						where_clause: where_clause.clone(),
						concurrently: *concurrently,
						expressions: expressions.clone(),
						mysql_options: *mysql_options,
						operator_class: operator_class.clone(),
					}
				}),
				#[cfg(not(feature = "pgvector"))]
				Operation::DropNamedIndex {
					table,
					name,
					columns,
					unique,
					index_type,
					where_clause,
					concurrently,
					expressions,
					mysql_options,
					operator_class,
				} => named_index_has_target(columns, expressions.as_deref()).then(|| {
					Operation::CreateIndexRepair {
						table: table.clone(),
						name: Some(name.clone()),
						columns: columns.clone(),
						unique: *unique,
						index_type: *index_type,
						where_clause: where_clause.clone(),
						concurrently: *concurrently,
						expressions: expressions.clone(),
						mysql_options: *mysql_options,
						operator_class: operator_class.clone(),
					}
				}),

				// Special operations
				Operation::RunSQL { reverse_sql, .. } => {
					reverse_sql.as_ref().map(|sql| Operation::RunSQL {
						sql: sql.clone(),
						reverse_sql: None,
					})
				}
				Operation::RunRust { reverse_code, .. } => {
					reverse_code.as_ref().map(|code| Operation::RunRust {
						code: code.clone(),
						reverse_code: None,
					})
				}

				// Other operations - no rollback
				Operation::AlterTableComment { .. }
				| Operation::AlterUniqueTogether { .. }
				| Operation::AlterModelOptions { .. }
				| Operation::CreateInheritedTable { .. }
				| Operation::AddDiscriminatorColumn { .. }
				| Operation::MoveModel { .. }
				| Operation::CreateSchema { .. }
				| Operation::DropSchema { .. }
				| Operation::CreateExtension { .. }
				| Operation::BulkLoad { .. }
				| Operation::SetAutoIncrementValue { .. }
				| Operation::CreateCompositePrimaryKey { .. } => None, // Cannot rollback - data loading / counter / constraint ops are not auto-reversible
			})
			.collect()
	}

	/// Validate migration before generation
	pub fn validate(&self, current_schema: DatabaseSchema) -> ValidationResult {
		let diff = SchemaDiff::new(current_schema, self.target_schema.clone());
		let diff_result = diff.detect();

		let mut warnings = Vec::new();
		let mut errors = Vec::new();

		// Check for data loss risks
		if !diff_result.tables_to_remove.is_empty() {
			warnings.push(format!(
				"Removing tables will cause data loss: {:?}",
				diff_result.tables_to_remove
			));
		}

		if !diff_result.columns_to_remove.is_empty() {
			warnings.push(format!(
				"Removing columns will cause data loss: {:?}",
				diff_result.columns_to_remove
			));
		}

		// Check for column type changes
		for (table, col, old, new) in &diff_result.columns_to_modify {
			if old.data_type != new.data_type {
				warnings.push(format!(
					"Column type change in {}.{}: {} -> {} (may cause data loss or conversion errors)",
					table, col, old.data_type, new.data_type
				));
			}

			if old.nullable && !new.nullable {
				errors.push(format!(
                    "Column {}.{} changed from nullable to non-nullable (requires default value or data migration)",
                    table, col
                ));
			}
		}

		ValidationResult {
			is_valid: errors.is_empty(),
			warnings,
			errors,
		}
	}
}

/// Validation result
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationResult {
	/// Is migration valid
	pub is_valid: bool,
	/// Warnings (non-blocking)
	pub warnings: Vec<String>,
	/// Errors (blocking)
	pub errors: Vec<String>,
}

/// Auto-migration error
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AutoMigrationError {
	#[error("No schema changes detected")]
	/// NoChangesDetected variant.
	NoChangesDetected,

	#[error("Failed to write migration file: {0}")]
	/// WriteError variant.
	WriteError(String),

	#[error("Migration validation failed: {0}")]
	/// ValidationError variant.
	ValidationError(String),

	#[error(
		"Duplicate migration detected: generated operations are identical to the last migration.\nThis usually means you're trying to generate the same migration twice.\nIf you need to modify the previous migration, delete it first and run makemigrations again."
	)]
	/// DuplicateMigration variant.
	DuplicateMigration,

	#[error(
		"Conflicting migrations detected for '{app_label}': {formatted_migrations}. Run makemigrations --merge to resolve."
	)]
	/// ConflictingMigrations variant.
	ConflictingMigrations {
		/// The app label with conflicting migrations.
		app_label: String,
		/// The conflicting migration names (comma-separated for display).
		formatted_migrations: String,
	},
}

impl From<std::io::Error> for AutoMigrationError {
	fn from(err: std::io::Error) -> Self {
		AutoMigrationError::WriteError(err.to_string())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::migrations::repository::MigrationRepository;
	use crate::migrations::{
		FieldState, FieldType, IndexDefinition, IndexType, Migration, MigrationAutodetector,
		MigrationError, ModelState, ProjectState, Result, SqlDialect,
	};
	use async_trait::async_trait;
	use std::collections::{BTreeMap, HashMap};
	use tokio::sync::Mutex;

	/// Test repository implementation
	struct TestRepository {
		migrations: HashMap<(String, String), Migration>,
	}

	impl TestRepository {
		fn new() -> Self {
			Self {
				migrations: HashMap::new(),
			}
		}
	}

	#[cfg(feature = "pgvector")]
	fn vector_index(index_type: IndexType, operator_class: &str) -> IndexDefinition {
		IndexDefinition {
			name: "documents_embedding_ann".to_string(),
			fields: vec!["embedding".to_string()],
			unique: false,
			where_clause: None,
			index_type: Some(index_type),
			operator_class: Some(operator_class.to_string()),
			expressions: None,
		}
	}

	#[cfg(feature = "pgvector")]
	fn vector_state(dimensions: usize, indexes: Vec<IndexDefinition>) -> ProjectState {
		let mut model = ModelState::new("search", "Document");
		model.table_name = "search_document".to_string();
		model.fields.insert(
			"embedding".to_string(),
			FieldState::new("embedding", FieldType::Vector { dimensions }, false),
		);
		model.indexes = indexes;
		let mut state = ProjectState::new();
		state.add_model(model);
		state
	}

	fn rollback_generator() -> AutoMigrationGenerator {
		AutoMigrationGenerator::new(
			DatabaseSchema::default(),
			Arc::new(Mutex::new(TestRepository::new())),
		)
	}

	#[async_trait]
	impl MigrationRepository for TestRepository {
		async fn save(&mut self, migration: &Migration) -> Result<()> {
			let key = (migration.app_label.to_string(), migration.name.to_string());
			self.migrations.insert(key, migration.clone());
			Ok(())
		}

		async fn get(&self, app_label: &str, name: &str) -> Result<Migration> {
			let key = (app_label.to_string(), name.to_string());
			self.migrations
				.get(&key)
				.cloned()
				.ok_or_else(|| MigrationError::NotFound(format!("{}.{}", app_label, name)))
		}

		async fn list(&self, app_label: &str) -> Result<Vec<Migration>> {
			Ok(self
				.migrations
				.values()
				.filter(|m| m.app_label == app_label)
				.cloned()
				.collect())
		}

		async fn exists(&self, app_label: &str, name: &str) -> Result<bool> {
			Ok(self
				.get(app_label, name)
				.await
				.map(|_| true)
				.unwrap_or(false))
		}
	}

	#[test]
	fn test_rollback_generation() {
		let target_schema = DatabaseSchema::default();
		let repo = Arc::new(Mutex::new(TestRepository::new()));
		let generator = AutoMigrationGenerator::new(target_schema, repo);

		let operations = vec![Operation::CreateTable {
			name: "users".to_string(),
			columns: Vec::new(),
			constraints: Vec::new(),
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		}];

		let rollback = generator.generate_rollback(&operations);
		assert_eq!(rollback.len(), 1);
		assert!(matches!(rollback[0], Operation::DropTable { .. }));
	}

	#[test]
	#[cfg(feature = "pgvector")]
	fn vector_index_column_rollback_retains_column_identity() {
		// Arrange
		let generator = AutoMigrationGenerator::new(
			DatabaseSchema::default(),
			Arc::new(Mutex::new(TestRepository::new())),
		);
		let operation = Operation::CreateIndex {
			table: "source".to_string(),
			columns: vec!["embedding".to_string()],
			unique: false,
			index_type: None,
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: None,
		};

		// Act
		let rollback = generator.generate_rollback(&[operation]);

		// Assert
		assert_eq!(
			rollback[0].to_sql(&SqlDialect::Postgres),
			"DROP INDEX idx_source_embedding;"
		);
	}

	#[test]
	#[cfg(feature = "pgvector")]
	fn vector_index_expression_rollback_retains_expression_identity() {
		// Arrange
		let generator = AutoMigrationGenerator::new(
			DatabaseSchema::default(),
			Arc::new(Mutex::new(TestRepository::new())),
		);
		let operation = Operation::CreateIndex {
			table: "source".to_string(),
			columns: vec![],
			unique: false,
			index_type: None,
			where_clause: None,
			concurrently: false,
			expressions: Some(vec!["normalize(embedding)".to_string()]),
			mysql_options: None,
			operator_class: None,
		};

		// Act
		let rollback = generator.generate_rollback(&[operation]);

		// Assert
		assert_eq!(
			rollback[0].to_sql(&SqlDialect::Postgres),
			"DROP INDEX idx_source_expr;"
		);
	}

	#[test]
	#[cfg(feature = "pgvector")]
	fn named_vector_index_removal_rollback_restores_old_definition() {
		// Arrange
		let old_index = vector_index(
			IndexType::Hnsw {
				m: Some(16),
				ef_construction: Some(64),
			},
			"vector_cosine_ops",
		);
		let from_state = vector_state(1536, vec![old_index.clone()]);
		let mut to_state = vector_state(1536, Vec::new());
		let operations =
			MigrationAutodetector::new(from_state, to_state.clone()).generate_operations();

		// Act
		let reverse_sql = operations[0]
			.to_reverse_sql(&SqlDialect::Postgres, &to_state)
			.expect("reverse SQL");
		let rollback = rollback_generator().generate_rollback(&operations);
		operations[0].state_backwards("search", &mut to_state);

		// Assert
		assert_eq!(
			reverse_sql,
			Some(vec![
				"CREATE INDEX documents_embedding_ann ON search_document USING hnsw (embedding vector_cosine_ops) WITH (m = 16, ef_construction = 64);".to_string()
			])
		);
		assert!(matches!(
			&rollback[..],
			[Operation::CreateNamedIndex {
				name,
				index_type: Some(IndexType::Hnsw {
					m: Some(16),
					ef_construction: Some(64),
				}),
				operator_class: Some(operator_class),
				..
			}] if name == "documents_embedding_ann"
				&& operator_class == "vector_cosine_ops"
		));
		assert_eq!(
			to_state
				.get_model("search", "Document")
				.expect("replayed model")
				.indexes,
			vec![old_index]
		);
	}

	#[test]
	#[cfg(feature = "pgvector")]
	fn named_vector_index_property_change_rollback_restores_old_definition() {
		// Arrange
		let old_index = vector_index(
			IndexType::Hnsw {
				m: Some(16),
				ef_construction: Some(64),
			},
			"vector_cosine_ops",
		);
		let new_index = vector_index(IndexType::Ivfflat { lists: Some(100) }, "vector_l2_ops");
		let from_state = vector_state(1536, vec![old_index.clone()]);
		let mut to_state = vector_state(1536, vec![new_index]);
		let operations =
			MigrationAutodetector::new(from_state, to_state.clone()).generate_operations();

		// Act
		let reverse_sql = operations[0]
			.to_reverse_sql(&SqlDialect::Postgres, &to_state)
			.expect("reverse SQL");
		let rollback = rollback_generator().generate_rollback(&operations);
		for operation in operations.iter().rev() {
			operation.state_backwards("search", &mut to_state);
		}

		// Assert
		assert_eq!(
			reverse_sql,
			Some(vec![
				"CREATE INDEX documents_embedding_ann ON search_document USING hnsw (embedding vector_cosine_ops) WITH (m = 16, ef_construction = 64);".to_string()
			])
		);
		assert!(matches!(
			&rollback[..],
			[
				Operation::DropNamedIndex { name: dropped, .. },
				Operation::CreateNamedIndex {
					name: created,
					index_type: Some(IndexType::Hnsw {
						m: Some(16),
						ef_construction: Some(64),
					}),
					operator_class: Some(operator_class),
					..
				}
			] if dropped == "documents_embedding_ann"
				&& created == "documents_embedding_ann"
				&& operator_class == "vector_cosine_ops"
		));
		assert_eq!(
			to_state
				.get_model("search", "Document")
				.expect("replayed model")
				.indexes,
			vec![old_index]
		);
	}

	#[test]
	#[cfg(feature = "pgvector")]
	fn named_vector_index_dimension_change_rollback_restores_old_definition() {
		// Arrange
		let old_index = vector_index(
			IndexType::Hnsw {
				m: Some(24),
				ef_construction: Some(96),
			},
			"vector_ip_ops",
		);
		let from_state = vector_state(768, vec![old_index.clone()]);
		let mut to_state = vector_state(1536, vec![old_index.clone()]);
		let operations =
			MigrationAutodetector::new(from_state, to_state.clone()).generate_operations();

		// Act
		let reverse_sql = operations[0]
			.to_reverse_sql(&SqlDialect::Postgres, &to_state)
			.expect("reverse SQL");
		let rollback = rollback_generator().generate_rollback(&operations);
		for operation in operations.iter().rev() {
			operation.state_backwards("search", &mut to_state);
		}

		// Assert
		assert_eq!(
			reverse_sql,
			Some(vec![
				"CREATE INDEX documents_embedding_ann ON search_document USING hnsw (embedding vector_ip_ops) WITH (m = 24, ef_construction = 96);".to_string()
			])
		);
		assert!(matches!(
			&rollback[..],
			[
				Operation::DropNamedIndex { name: dropped, .. },
				Operation::CreateNamedIndex {
					name: created,
					index_type: Some(IndexType::Hnsw {
						m: Some(24),
						ef_construction: Some(96),
					}),
					operator_class: Some(operator_class),
					..
				}
			] if dropped == "documents_embedding_ann"
				&& created == "documents_embedding_ann"
				&& operator_class == "vector_ip_ops"
		));
		assert_eq!(
			to_state
				.get_model("search", "Document")
				.expect("replayed model")
				.indexes,
			vec![old_index]
		);
	}

	#[test]
	fn legacy_named_index_drop_without_definition_does_not_fabricate_rollback() {
		// Arrange
		let generator = rollback_generator();
		let operation: Operation = serde_json::from_str(
			r#"{
				"type": "DropNamedIndex",
				"table": "search_document",
				"name": "documents_embedding_ann"
			}"#,
		)
		.expect("legacy DropNamedIndex JSON");

		// Act
		let rollback = generator.generate_rollback(&[operation]);

		// Assert
		assert!(rollback.is_empty());
	}

	#[test]
	fn test_validation_nullable_change() {
		use crate::migrations::schema_diff::{ColumnSchema, TableSchema};

		let mut current = DatabaseSchema::default();
		let mut current_table = TableSchema {
			name: "users".to_string(),
			columns: BTreeMap::new(),
			indexes: Vec::new(),
			constraints: Vec::new(),
		};
		current_table.columns.insert(
			"email".to_string(),
			ColumnSchema {
				name: "email".to_string(),
				data_type: FieldType::VarChar(255),
				nullable: true,
				default: None,
				primary_key: false,
				auto_increment: false,
				generated: None,
			},
		);
		current.tables.insert("users".to_string(), current_table);

		let mut target = DatabaseSchema::default();
		let mut target_table = TableSchema {
			name: "users".to_string(),
			columns: BTreeMap::new(),
			indexes: Vec::new(),
			constraints: Vec::new(),
		};
		target_table.columns.insert(
			"email".to_string(),
			ColumnSchema {
				name: "email".to_string(),
				data_type: FieldType::VarChar(255),
				nullable: false, // Changed to non-nullable
				default: None,
				primary_key: false,
				auto_increment: false,
				generated: None,
			},
		);
		target.tables.insert("users".to_string(), target_table);

		let repo = Arc::new(Mutex::new(TestRepository::new()));
		let generator = AutoMigrationGenerator::new(target, repo);
		let validation = generator.validate(current);

		assert!(!validation.is_valid);
		assert!(!validation.errors.is_empty());
	}
}

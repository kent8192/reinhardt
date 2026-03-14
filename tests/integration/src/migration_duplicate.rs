//! Integration tests for migration generation to prevent duplicate migrations
//!
//! Tests that ensure makemigrations doesn't create duplicate migration files
//! when run multiple times without model changes.

#[cfg(test)]
mod tests {
	use async_trait::async_trait;
	use reinhardt_db::migrations::{
		AutoMigrationGenerator, DatabaseSchema, FieldType, Migration, MigrationRepository,
		schema_diff::SchemaDiff,
	};
	use std::collections::{BTreeMap, HashMap};
	use std::sync::Arc;
	use tempfile::TempDir;

	/// Test repository implementation for integration tests
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

	#[async_trait]
	impl MigrationRepository for TestRepository {
		async fn save(&mut self, migration: &Migration) -> reinhardt_db::migrations::Result<()> {
			let key = (migration.app_label.to_string(), migration.name.to_string());
			self.migrations.insert(key, migration.clone());
			Ok(())
		}

		async fn get(
			&self,
			app_label: &str,
			name: &str,
		) -> reinhardt_db::migrations::Result<Migration> {
			let key = (app_label.to_string(), name.to_string());
			self.migrations.get(&key).cloned().ok_or_else(|| {
				reinhardt_db::migrations::MigrationError::NotFound(format!(
					"{}.{}",
					app_label, name
				))
			})
		}

		async fn list(&self, app_label: &str) -> reinhardt_db::migrations::Result<Vec<Migration>> {
			Ok(self
				.migrations
				.values()
				.filter(|m| m.app_label == app_label)
				.cloned()
				.collect())
		}

		async fn exists(
			&self,
			app_label: &str,
			name: &str,
		) -> reinhardt_db::migrations::Result<bool> {
			Ok(self
				.get(app_label, name)
				.await
				.map(|_| true)
				.unwrap_or(false))
		}
	}

	/// Test that system tables (like reinhardt_migrations) are excluded from schema diff
	#[test]
	fn test_system_tables_excluded_from_diff() {
		// Create a schema with user tables and system table
		let mut current_schema = DatabaseSchema {
			tables: BTreeMap::new(),
		};

		// Add system table (reinhardt_migrations)
		current_schema.tables.insert(
			"reinhardt_migrations".to_string(),
			reinhardt_db::migrations::schema_diff::TableSchema {
				name: "reinhardt_migrations".to_string(),
				columns: BTreeMap::new(),
				indexes: Vec::new(),
				constraints: Vec::new(),
			},
		);

		// Add user table
		current_schema.tables.insert(
			"users".to_string(),
			reinhardt_db::migrations::schema_diff::TableSchema {
				name: "users".to_string(),
				columns: BTreeMap::new(),
				indexes: Vec::new(),
				constraints: Vec::new(),
			},
		);

		// Target schema has only the user table (no system tables)
		let mut target_schema = DatabaseSchema {
			tables: BTreeMap::new(),
		};
		target_schema.tables.insert(
			"users".to_string(),
			reinhardt_db::migrations::schema_diff::TableSchema {
				name: "users".to_string(),
				columns: BTreeMap::new(),
				indexes: Vec::new(),
				constraints: Vec::new(),
			},
		);

		// Create diff
		let diff = SchemaDiff::new(current_schema, target_schema);
		let diff_result = diff.detect();

		// System table should NOT appear in tables_to_remove
		assert!(
			!diff_result
				.tables_to_remove
				.iter()
				.any(|t| t == "reinhardt_migrations"),
			"System table 'reinhardt_migrations' should not be included in tables to remove"
		);

		// Generate operations
		let operations = diff.generate_operations();

		// Verify no DropTable operation for reinhardt_migrations
		for op in &operations {
			if let reinhardt_db::migrations::Operation::DropTable { name } = op {
				assert_ne!(
					*name, "reinhardt_migrations",
					"Should not generate DropTable operation for system table"
				);
			}
		}
	}

	/// Test that no changes are detected when schema hasn't changed
	#[test]
	fn test_no_changes_when_schema_unchanged() {
		// Create identical schemas
		let mut schema = DatabaseSchema {
			tables: BTreeMap::new(),
		};

		schema.tables.insert(
			"users".to_string(),
			reinhardt_db::migrations::schema_diff::TableSchema {
				name: "users".to_string(),
				columns: BTreeMap::new(),
				indexes: Vec::new(),
				constraints: Vec::new(),
			},
		);

		let diff = SchemaDiff::new(schema.clone(), schema.clone());
		let diff_result = diff.detect();

		// No changes should be detected
		assert!(diff_result.tables_to_add.is_empty());
		assert!(diff_result.tables_to_remove.is_empty());
		assert!(diff_result.columns_to_add.is_empty());
		assert!(diff_result.columns_to_remove.is_empty());

		// Generate operations should return empty
		let operations = diff.generate_operations();
		assert!(
			operations.is_empty(),
			"No operations should be generated when schemas are identical"
		);
	}

	/// Test auto-migration generator with no changes
	#[tokio::test]
	async fn test_auto_migration_no_changes_detected() {
		let _temp_dir = TempDir::new().unwrap();

		// Create identical schemas
		let schema = DatabaseSchema {
			tables: BTreeMap::new(),
		};

		let repository: Arc<tokio::sync::Mutex<dyn MigrationRepository>> =
			Arc::new(tokio::sync::Mutex::new(TestRepository::new()));

		let generator = AutoMigrationGenerator::new(schema.clone(), repository);

		// Generate should return NoChangesDetected error
		let result = generator.generate("testapp", schema).await;

		assert!(
			matches!(
				result,
				Err(reinhardt_db::migrations::AutoMigrationError::NoChangesDetected)
			),
			"Should return NoChangesDetected error when schemas are identical"
		);
	}

	/// Integration test: Simulate makemigrations workflow
	///
	/// This test verifies that:
	/// 1. First makemigrations creates a migration
	/// 2. After applying migration, second makemigrations detects no changes
	///
	/// Note: This is a unit-level test. A full integration test would require:
	/// - Actually running migrate command to apply migrations to a test database
	/// - Running makemigrations command via CLI
	/// - Verifying filesystem state
	///
	/// Note: FilesystemSource now parses operations via ast_parser module.
	/// Full CLI integration test can be implemented when needed.
	#[tokio::test]
	async fn test_makemigrations_workflow_unit() {
		let _temp_dir = TempDir::new().unwrap();

		// Step 1: Empty current schema, target has tables
		let current_schema = DatabaseSchema {
			tables: BTreeMap::new(),
		};

		let mut target_schema = DatabaseSchema {
			tables: BTreeMap::new(),
		};

		// Add a table to target schema
		let mut columns = BTreeMap::new();
		columns.insert(
			"id".to_string(),
			reinhardt_db::migrations::schema_diff::ColumnSchema {
				name: "id".to_string(),
				data_type: FieldType::Integer,
				nullable: false,
				default: None,
				primary_key: true,
				auto_increment: true,
			},
		);

		target_schema.tables.insert(
			"users".to_string(),
			reinhardt_db::migrations::schema_diff::TableSchema {
				name: "users".to_string(),
				columns,
				indexes: Vec::new(),
				constraints: Vec::new(),
			},
		);

		// Generate first migration
		let repository: Arc<tokio::sync::Mutex<dyn MigrationRepository>> =
			Arc::new(tokio::sync::Mutex::new(TestRepository::new()));

		let generator = AutoMigrationGenerator::new(target_schema.clone(), repository.clone());
		let result = generator.generate("testapp", current_schema.clone()).await;

		assert!(result.is_ok(), "First migration generation should succeed");
		let migration_result = result.unwrap();
		assert_eq!(
			migration_result.operation_count, 1,
			"Should have 1 operation (CreateTable)"
		);

		// Step 2: After "applying" migration, schemas should be identical
		// Simulate post-migration state: current schema now has the table
		let generator2 = AutoMigrationGenerator::new(target_schema.clone(), repository);
		let result2 = generator2.generate("testapp", target_schema).await;

		// Should return NoChangesDetected
		assert!(
			matches!(
				result2,
				Err(reinhardt_db::migrations::AutoMigrationError::NoChangesDetected)
			),
			"Second migration generation should detect no changes"
		);
	}
}

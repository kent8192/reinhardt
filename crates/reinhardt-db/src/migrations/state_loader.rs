//! Migration State Loader
//!
//! This module provides functionality to build a `ProjectState` by replaying
//! applied migrations, following Django's approach to schema state management.
//!
//! Instead of directly querying the database schema (introspection-based approach),
//! this loader reconstructs the schema state by sequentially applying all migration
//! operations from the migration history.

use super::recorder::MigrationRecord;
use super::{
	DatabaseMigrationRecorder, Migration, MigrationGraph, MigrationKey, MigrationSource,
	ProjectState, Result,
};

/// Loader for building ProjectState from migration history.
///
/// This is the Django-style approach where `from_state` is computed by replaying
/// past migrations to reconstruct a virtual schema (ProjectState), rather than
/// directly querying the database.
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_db::migrations::{MigrationStateLoader, DatabaseMigrationRecorder, FilesystemSource};
/// async fn example() -> reinhardt_db::migrations::Result<()> {
///     let connection = reinhardt_db::backends::DatabaseConnection::connect_postgres("postgres://localhost/mydb").await.unwrap();
///     let recorder = DatabaseMigrationRecorder::new(connection);
///     let source = FilesystemSource::new("./migrations");
///     let loader = MigrationStateLoader::new(recorder, source);
///
///     // Build current state by replaying applied migrations
///     let current_state = loader.build_current_state().await?;
///     Ok(())
/// }
/// ```
pub struct MigrationStateLoader<S: MigrationSource> {
	recorder: DatabaseMigrationRecorder,
	source: S,
}

impl<S: MigrationSource> MigrationStateLoader<S> {
	/// Create a new MigrationStateLoader.
	///
	/// # Arguments
	///
	/// * `recorder` - The migration recorder to get applied migrations from
	/// * `source` - The migration source to load migration definitions from
	pub fn new(recorder: DatabaseMigrationRecorder, source: S) -> Self {
		Self { recorder, source }
	}

	/// Build the current ProjectState by replaying all applied migrations.
	///
	/// This method:
	/// 1. Gets the list of applied migrations from the database
	/// 2. Loads all available migration definitions from the source
	/// 3. Builds a migration graph for dependency resolution
	/// 4. Replays migrations in topological order to build the ProjectState
	///
	/// # Returns
	///
	/// The ProjectState representing the current database schema as understood
	/// through migration history.
	///
	/// # Errors
	///
	/// Returns an error if:
	/// - Failed to get applied migrations from the recorder
	/// - Failed to load migrations from the source
	/// - A migration dependency cannot be resolved
	/// - Circular dependency is detected
	pub async fn build_current_state(&self) -> Result<ProjectState> {
		// 1. Get applied migrations from database
		let applied_records = self.recorder.get_applied_migrations().await?;

		// If no migrations have been applied, return empty state
		if applied_records.is_empty() {
			return Ok(ProjectState::default());
		}

		// 2. Load all available migrations from source
		let all_migrations = self.source.all_migrations().await?;

		// 3. Build a graph of applied migrations
		let graph = self.build_applied_migration_graph(&applied_records, &all_migrations)?;

		// 4. Get topologically sorted order
		let sorted_keys = graph.topological_sort()?;

		// 5. Build ProjectState by replaying migrations in order
		let mut state = ProjectState::default();

		for key in sorted_keys {
			// Find the migration with this key
			if let Some(migration) = all_migrations
				.iter()
				.find(|m| m.app_label == key.app_label && m.name == key.name)
			{
				eprintln!(
					"[DEBUG] Applying migration: {}/{}",
					migration.app_label, migration.name
				);
				eprintln!("[DEBUG]   Operations count: {}", migration.operations.len());
				state.apply_migration_operations(&migration.operations, &migration.app_label);
				eprintln!(
					"[DEBUG]   State after applying - models count: {}",
					state.models.len()
				);
				for (app, model_name) in state.models.keys() {
					eprintln!("[DEBUG]     - {}/{}", app, model_name);
				}
			}
		}

		Ok(state)
	}

	/// Build a migration graph from applied migrations.
	///
	/// Only includes migrations that have been applied to the database.
	fn build_applied_migration_graph(
		&self,
		applied_records: &[MigrationRecord],
		all_migrations: &[Migration],
	) -> Result<MigrationGraph> {
		let mut graph = MigrationGraph::new();

		// Create a set of applied migration keys for quick lookup
		let applied_set: std::collections::HashSet<(String, String)> = applied_records
			.iter()
			.map(|r| (r.app.clone(), r.name.clone()))
			.collect();

		// Add only applied migrations to the graph
		for migration in all_migrations {
			let key_tuple = (migration.app_label.to_string(), migration.name.to_string());
			if !applied_set.contains(&key_tuple) {
				continue;
			}

			let key = MigrationKey::new(migration.app_label.clone(), migration.name.clone());

			// Convert dependencies to MigrationKey
			let dependencies: Vec<MigrationKey> = migration
				.dependencies
				.iter()
				.map(|(app, name)| MigrationKey::new(app.clone(), name.clone()))
				.collect();

			// Convert replaces to MigrationKey
			let replaces: Vec<MigrationKey> = migration
				.replaces
				.iter()
				.map(|(app, name)| MigrationKey::new(app.clone(), name.clone()))
				.collect();

			graph.add_migration_with_replaces(key, dependencies, replaces);
		}

		Ok(graph)
	}

	/// Get the list of applied migrations.
	///
	/// This is a convenience method that delegates to the recorder.
	pub async fn get_applied_migrations(&self) -> Result<Vec<MigrationRecord>> {
		self.recorder.get_applied_migrations().await
	}

	/// Check if a specific migration has been applied.
	///
	/// # Arguments
	///
	/// * `app_label` - The app label of the migration
	/// * `name` - The name of the migration
	pub async fn is_migration_applied(&self, app_label: &str, name: &str) -> Result<bool> {
		self.recorder.is_applied(app_label, name).await
	}

	/// Build ProjectState up to a specific migration.
	///
	/// This is useful for understanding the schema state at a specific point
	/// in migration history.
	///
	/// # Arguments
	///
	/// * `target_app` - The app label of the target migration
	/// * `target_name` - The name of the target migration
	pub async fn build_state_up_to(
		&self,
		target_app: &str,
		target_name: &str,
	) -> Result<ProjectState> {
		// Load all migrations
		let all_migrations = self.source.all_migrations().await?;

		// Build full graph
		let mut graph = MigrationGraph::new();
		for migration in &all_migrations {
			let key = MigrationKey::new(migration.app_label.clone(), migration.name.clone());

			let dependencies: Vec<MigrationKey> = migration
				.dependencies
				.iter()
				.map(|(app, name)| MigrationKey::new(app.clone(), name.clone()))
				.collect();

			let replaces: Vec<MigrationKey> = migration
				.replaces
				.iter()
				.map(|(app, name)| MigrationKey::new(app.clone(), name.clone()))
				.collect();

			graph.add_migration_with_replaces(key, dependencies, replaces);
		}

		// Find root nodes and build path to target
		let root_nodes = graph.get_root_nodes();

		let target_key = MigrationKey::new(target_app, target_name);

		// If no root nodes, just start from target
		let path = if root_nodes.is_empty() {
			vec![target_key]
		} else {
			// Find path from first root to target
			graph.find_migration_path(root_nodes[0], &target_key)?
		};

		// Build state by applying migrations in path order
		let mut state = ProjectState::default();
		for key in path {
			if let Some(migration) = all_migrations
				.iter()
				.find(|m| m.app_label == key.app_label && m.name == key.name)
			{
				state.apply_migration_operations(&migration.operations, &migration.app_label);
			}
		}

		Ok(state)
	}
}

#[cfg(test)]
#[cfg(feature = "sqlite")]
mod tests {
	use super::*;
	use crate::migrations::FieldType;
	use crate::migrations::operations::{ColumnDefinition, Operation};
	use chrono::Utc;

	/// Helper function to create a MigrationRecord for testing
	fn create_migration_record(app: &str, name: &str) -> MigrationRecord {
		MigrationRecord {
			app: app.to_string(),
			name: name.to_string(),
			applied: Utc::now(),
		}
	}

	/// Helper function to create a Migration for testing
	fn create_migration(
		app_label: &str,
		name: &str,
		operations: Vec<Operation>,
		dependencies: Vec<(&str, &str)>,
	) -> Migration {
		Migration {
			app_label: app_label.to_string(),
			name: name.to_string(),
			operations,
			dependencies: dependencies
				.into_iter()
				.map(|(a, n)| (a.to_string(), n.to_string()))
				.collect(),
			replaces: vec![],
			atomic: true,
			initial: None,
			state_only: false,
			database_only: false,
			swappable_dependencies: vec![],
			optional_dependencies: vec![],
		}
	}

	/// Helper function to create a CreateTable operation
	fn create_table_operation(table_name: &str, columns: Vec<&str>) -> Operation {
		Operation::CreateTable {
			name: table_name.to_string(),
			columns: columns
				.into_iter()
				.map(|col_name| ColumnDefinition {
					name: col_name.to_string(),
					type_definition: FieldType::VarChar(255),
					not_null: false,
					primary_key: col_name == "id",
					unique: false,
					auto_increment: col_name == "id",
					default: None,
				})
				.collect(),
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		}
	}

	/// Helper function to create an AddColumn operation
	fn add_column_operation(table_name: &str, column_name: &str) -> Operation {
		Operation::AddColumn {
			table: table_name.to_string(),
			column: ColumnDefinition {
				name: column_name.to_string(),
				type_definition: FieldType::VarChar(255),
				not_null: false,
				primary_key: false,
				unique: false,
				auto_increment: false,
				default: None,
			},
			mysql_options: None,
		}
	}

	mod build_applied_migration_graph {
		use super::*;
		use crate::backends::DatabaseConnection;

		/// Mock migration source for testing
		#[derive(Clone)]
		struct MockMigrationSource {
			migrations: Vec<Migration>,
		}

		#[async_trait::async_trait]
		impl MigrationSource for MockMigrationSource {
			async fn all_migrations(&self) -> Result<Vec<Migration>> {
				Ok(self.migrations.clone())
			}
		}

		/// Test that only applied migrations are included in the graph
		#[tokio::test]
		async fn test_filters_unapplied_migrations() {
			let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
				.await
				.expect("Failed to connect");
			let recorder = crate::migrations::DatabaseMigrationRecorder::new(connection);

			let source = MockMigrationSource { migrations: vec![] };
			let loader = MigrationStateLoader::new(recorder, source);

			// Create applied records (only migration 1 is applied)
			let applied_records = vec![create_migration_record("myapp", "0001_initial")];

			// Create all migrations (both migration 1 and 2 exist)
			let all_migrations = vec![
				create_migration(
					"myapp",
					"0001_initial",
					vec![create_table_operation("users", vec!["id", "name"])],
					vec![],
				),
				create_migration(
					"myapp",
					"0002_add_email",
					vec![add_column_operation("users", "email")],
					vec![("myapp", "0001_initial")],
				),
			];

			let graph = loader
				.build_applied_migration_graph(&applied_records, &all_migrations)
				.expect("Failed to build graph");

			// Graph should only contain one migration (the applied one)
			let sorted = graph.topological_sort().expect("Failed to sort");
			assert_eq!(sorted.len(), 1);
			assert_eq!(sorted[0].app_label, "myapp");
			assert_eq!(sorted[0].name, "0001_initial");
		}

		/// Test that multiple applied migrations are included with correct dependencies
		#[tokio::test]
		async fn test_includes_all_applied_migrations() {
			let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
				.await
				.expect("Failed to connect");
			let recorder = crate::migrations::DatabaseMigrationRecorder::new(connection);

			let source = MockMigrationSource { migrations: vec![] };
			let loader = MigrationStateLoader::new(recorder, source);

			// Both migrations are applied
			let applied_records = vec![
				create_migration_record("myapp", "0001_initial"),
				create_migration_record("myapp", "0002_add_email"),
			];

			let all_migrations = vec![
				create_migration(
					"myapp",
					"0001_initial",
					vec![create_table_operation("users", vec!["id", "name"])],
					vec![],
				),
				create_migration(
					"myapp",
					"0002_add_email",
					vec![add_column_operation("users", "email")],
					vec![("myapp", "0001_initial")],
				),
			];

			let graph = loader
				.build_applied_migration_graph(&applied_records, &all_migrations)
				.expect("Failed to build graph");

			let sorted = graph.topological_sort().expect("Failed to sort");
			assert_eq!(sorted.len(), 2);
			// First should be initial (no dependencies)
			assert_eq!(sorted[0].name, "0001_initial");
			// Second should be dependent one
			assert_eq!(sorted[1].name, "0002_add_email");
		}

		/// Test with migrations from multiple apps
		#[tokio::test]
		async fn test_multiple_apps() {
			let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
				.await
				.expect("Failed to connect");
			let recorder = crate::migrations::DatabaseMigrationRecorder::new(connection);

			let source = MockMigrationSource { migrations: vec![] };
			let loader = MigrationStateLoader::new(recorder, source);

			let applied_records = vec![
				create_migration_record("users", "0001_initial"),
				create_migration_record("posts", "0001_initial"),
			];

			let all_migrations = vec![
				create_migration(
					"users",
					"0001_initial",
					vec![create_table_operation("users", vec!["id", "name"])],
					vec![],
				),
				create_migration(
					"posts",
					"0001_initial",
					vec![create_table_operation("posts", vec!["id", "title"])],
					vec![],
				),
			];

			let graph = loader
				.build_applied_migration_graph(&applied_records, &all_migrations)
				.expect("Failed to build graph");

			let sorted = graph.topological_sort().expect("Failed to sort");
			assert_eq!(sorted.len(), 2);

			// Both apps should be represented
			let apps: std::collections::HashSet<_> =
				sorted.iter().map(|k| k.app_label.as_str()).collect();
			assert!(apps.contains("users"));
			assert!(apps.contains("posts"));
		}

		/// Test with empty applied records
		#[tokio::test]
		async fn test_empty_applied_records() {
			let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
				.await
				.expect("Failed to connect");
			let recorder = crate::migrations::DatabaseMigrationRecorder::new(connection);

			let source = MockMigrationSource { migrations: vec![] };
			let loader = MigrationStateLoader::new(recorder, source);

			let applied_records: Vec<MigrationRecord> = vec![];

			let all_migrations = vec![create_migration(
				"myapp",
				"0001_initial",
				vec![create_table_operation("users", vec!["id"])],
				vec![],
			)];

			let graph = loader
				.build_applied_migration_graph(&applied_records, &all_migrations)
				.expect("Failed to build graph");

			let sorted = graph.topological_sort().expect("Failed to sort");
			assert!(sorted.is_empty());
		}

		/// Test with empty all_migrations
		#[tokio::test]
		async fn test_empty_all_migrations() {
			let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
				.await
				.expect("Failed to connect");
			let recorder = crate::migrations::DatabaseMigrationRecorder::new(connection);

			let source = MockMigrationSource { migrations: vec![] };
			let loader = MigrationStateLoader::new(recorder, source);

			let applied_records = vec![create_migration_record("myapp", "0001_initial")];

			let all_migrations: Vec<Migration> = vec![];

			let graph = loader
				.build_applied_migration_graph(&applied_records, &all_migrations)
				.expect("Failed to build graph");

			// Applied record exists but no matching migration definition
			let sorted = graph.topological_sort().expect("Failed to sort");
			assert!(sorted.is_empty());
		}

		/// Test cross-app dependencies
		#[tokio::test]
		async fn test_cross_app_dependencies() {
			let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
				.await
				.expect("Failed to connect");
			let recorder = crate::migrations::DatabaseMigrationRecorder::new(connection);

			let source = MockMigrationSource { migrations: vec![] };
			let loader = MigrationStateLoader::new(recorder, source);

			let applied_records = vec![
				create_migration_record("users", "0001_initial"),
				create_migration_record("posts", "0001_initial"),
			];

			let all_migrations = vec![
				create_migration(
					"users",
					"0001_initial",
					vec![create_table_operation("users", vec!["id", "name"])],
					vec![],
				),
				create_migration(
					"posts",
					"0001_initial",
					vec![create_table_operation("posts", vec!["id", "user_id"])],
					vec![("users", "0001_initial")], // posts depends on users
				),
			];

			let graph = loader
				.build_applied_migration_graph(&applied_records, &all_migrations)
				.expect("Failed to build graph");

			let sorted = graph.topological_sort().expect("Failed to sort");
			assert_eq!(sorted.len(), 2);

			// users should come before posts due to dependency
			let users_pos = sorted.iter().position(|k| k.app_label == "users").unwrap();
			let posts_pos = sorted.iter().position(|k| k.app_label == "posts").unwrap();
			assert!(
				users_pos < posts_pos,
				"users should come before posts in topological order"
			);
		}
	}

	mod project_state_replay {
		use super::*;

		/// Test that CreateTable operation creates model with correct fields
		#[test]
		fn test_create_table_creates_model() {
			let mut state = ProjectState::default();

			let operations = vec![create_table_operation("users", vec!["id", "name", "email"])];

			state.apply_migration_operations(&operations, "testapp");

			assert_eq!(state.models.len(), 1);
			let model = state.models.values().next().unwrap();
			assert_eq!(model.table_name, "users");
			assert!(model.fields.contains_key("id"));
			assert!(model.fields.contains_key("name"));
			assert!(model.fields.contains_key("email"));
		}

		/// Test that AddColumn adds field to existing model
		#[test]
		fn test_add_column_adds_field() {
			let mut state = ProjectState::default();

			// First create the table
			let create_ops = vec![create_table_operation("users", vec!["id", "name"])];
			state.apply_migration_operations(&create_ops, "testapp");

			// Then add a column
			let add_ops = vec![add_column_operation("users", "email")];
			state.apply_migration_operations(&add_ops, "testapp");

			let model = state.models.values().next().unwrap();
			assert_eq!(model.fields.len(), 3);
			assert!(model.fields.contains_key("id"));
			assert!(model.fields.contains_key("name"));
			assert!(model.fields.contains_key("email"));
		}

		/// Test that DropColumn removes field from model
		#[test]
		fn test_drop_column_removes_field() {
			let mut state = ProjectState::default();

			// Create table with multiple columns
			let create_ops = vec![create_table_operation("users", vec!["id", "name", "email"])];
			state.apply_migration_operations(&create_ops, "testapp");

			// Drop a column
			let drop_ops = vec![Operation::DropColumn {
				table: "users".to_string(),
				column: "email".to_string(),
			}];
			state.apply_migration_operations(&drop_ops, "testapp");

			let model = state.models.values().next().unwrap();
			assert_eq!(model.fields.len(), 2);
			assert!(model.fields.contains_key("id"));
			assert!(model.fields.contains_key("name"));
			assert!(!model.fields.contains_key("email"));
		}

		/// Test that DropTable removes model
		#[test]
		fn test_drop_table_removes_model() {
			let mut state = ProjectState::default();

			// Create two tables
			let create_ops = vec![
				create_table_operation("users", vec!["id"]),
				create_table_operation("posts", vec!["id"]),
			];
			state.apply_migration_operations(&create_ops, "testapp");

			assert_eq!(state.models.len(), 2);

			// Drop one table
			let drop_ops = vec![Operation::DropTable {
				name: "users".to_string(),
			}];
			state.apply_migration_operations(&drop_ops, "testapp");

			assert_eq!(state.models.len(), 1);
			let model = state.models.values().next().unwrap();
			assert_eq!(model.table_name, "posts");
		}

		/// Test that RenameTable updates table name
		#[test]
		fn test_rename_table_updates_name() {
			let mut state = ProjectState::default();

			let create_ops = vec![create_table_operation("old_users", vec!["id"])];
			state.apply_migration_operations(&create_ops, "testapp");

			let rename_ops = vec![Operation::RenameTable {
				old_name: "old_users".to_string(),
				new_name: "users".to_string(),
			}];
			state.apply_migration_operations(&rename_ops, "testapp");

			let model = state.models.values().next().unwrap();
			assert_eq!(model.table_name, "users");
		}

		/// Test that RenameColumn updates field name
		#[test]
		fn test_rename_column_updates_field_name() {
			let mut state = ProjectState::default();

			let create_ops = vec![create_table_operation("users", vec!["id", "user_name"])];
			state.apply_migration_operations(&create_ops, "testapp");

			let rename_ops = vec![Operation::RenameColumn {
				table: "users".to_string(),
				old_name: "user_name".to_string(),
				new_name: "name".to_string(),
			}];
			state.apply_migration_operations(&rename_ops, "testapp");

			let model = state.models.values().next().unwrap();
			assert!(!model.fields.contains_key("user_name"));
			assert!(model.fields.contains_key("name"));
		}

		/// Test sequential operations from multiple migrations
		#[test]
		fn test_sequential_migrations() {
			let mut state = ProjectState::default();

			// Migration 1: Create initial table
			let migration1_ops = vec![create_table_operation("users", vec!["id", "name"])];
			state.apply_migration_operations(&migration1_ops, "testapp");

			// Migration 2: Add email column
			let migration2_ops = vec![add_column_operation("users", "email")];
			state.apply_migration_operations(&migration2_ops, "testapp");

			// Migration 3: Add created_at column
			let migration3_ops = vec![add_column_operation("users", "created_at")];
			state.apply_migration_operations(&migration3_ops, "testapp");

			let model = state.models.values().next().unwrap();
			assert_eq!(model.fields.len(), 4);
			assert!(model.fields.contains_key("id"));
			assert!(model.fields.contains_key("name"));
			assert!(model.fields.contains_key("email"));
			assert!(model.fields.contains_key("created_at"));
		}
	}
}

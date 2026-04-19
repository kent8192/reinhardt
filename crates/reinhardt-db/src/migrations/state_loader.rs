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

/// Build `ProjectState` from all migration files without a database connection.
///
/// This function loads all migrations from the given source, builds a
/// dependency graph from ALL of them (not just applied ones), topologically
/// sorts them, and replays their operations to reconstruct the full schema state.
///
/// This is the offline fallback for `makemigrations` when neither a
/// database nor TestContainers is available. It assumes all migration files
/// on disk represent the current schema state.
///
/// # Arguments
///
/// * `source` - A migration source (e.g., `FilesystemSource`) to load migrations from
///
/// # Returns
///
/// The `ProjectState` representing the schema as defined by all migration files.
///
/// # Errors
///
/// Returns an error if:
/// - Failed to load migrations from the source
/// - Circular dependency is detected in the migration graph
pub async fn build_state_from_files<S: MigrationSource>(source: &S) -> Result<ProjectState> {
	// 1. Load all available migrations from source
	let all_migrations = source.all_migrations().await?;

	// If no migration files exist, return empty state (genuinely initial migration)
	if all_migrations.is_empty() {
		return Ok(ProjectState::default());
	}

	// 2. Build a graph from ALL migrations (not filtered by applied status)
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

	// 3. Get topologically sorted order
	let sorted_keys = graph.topological_sort()?;

	// 4. Build ProjectState by replaying all migrations in order
	let mut state = ProjectState::default();

	for key in sorted_keys {
		if let Some(migration) = all_migrations
			.iter()
			.find(|m| m.app_label == key.app_label && m.name == key.name)
		{
			state.apply_migration_operations(&migration.operations, &migration.app_label);
		}
	}

	Ok(state)
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

#[cfg(test)]
mod build_state_from_files_tests {
	use super::*;
	use crate::migrations::FieldType;
	use crate::migrations::operations::{ColumnDefinition, Operation};
	use rstest::rstest;

	/// Mock migration source for testing (no database required)
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

	/// Empty source returns empty ProjectState
	#[rstest]
	#[tokio::test]
	async fn test_empty_source_returns_empty_state() {
		// Arrange
		let source = MockMigrationSource { migrations: vec![] };

		// Act
		let state = build_state_from_files(&source).await.unwrap();

		// Assert
		assert!(state.models.is_empty());
	}

	/// Single CreateTable migration produces one model in state
	#[rstest]
	#[tokio::test]
	async fn test_single_create_table() {
		// Arrange
		let source = MockMigrationSource {
			migrations: vec![create_migration(
				"auth",
				"0001_initial",
				vec![create_table_operation("auth_users", vec!["id", "username"])],
				vec![],
			)],
		};

		// Act
		let state = build_state_from_files(&source).await.unwrap();

		// Assert
		assert_eq!(state.models.len(), 1);
		let model = state.find_model_by_table("auth_users").unwrap();
		assert_eq!(model.fields.len(), 2);
		assert!(model.fields.contains_key("id"));
		assert!(model.fields.contains_key("username"));
	}

	/// Chained migrations: CreateTable then AddColumn produces correct state
	#[rstest]
	#[tokio::test]
	async fn test_chained_create_and_add_column() {
		// Arrange
		let source = MockMigrationSource {
			migrations: vec![
				create_migration(
					"auth",
					"0001_initial",
					vec![create_table_operation("auth_users", vec!["id", "username"])],
					vec![],
				),
				create_migration(
					"auth",
					"0002_add_email",
					vec![add_column_operation("auth_users", "email")],
					vec![("auth", "0001_initial")],
				),
			],
		};

		// Act
		let state = build_state_from_files(&source).await.unwrap();

		// Assert
		assert_eq!(state.models.len(), 1);
		let model = state.find_model_by_table("auth_users").unwrap();
		assert_eq!(model.fields.len(), 3);
		assert!(model.fields.contains_key("id"));
		assert!(model.fields.contains_key("username"));
		assert!(model.fields.contains_key("email"));
	}

	/// Cross-app dependencies are resolved correctly via topological sort
	#[rstest]
	#[tokio::test]
	async fn test_cross_app_dependencies() {
		// Arrange
		let source = MockMigrationSource {
			migrations: vec![
				create_migration(
					"auth",
					"0001_initial",
					vec![create_table_operation("auth_users", vec!["id", "username"])],
					vec![],
				),
				create_migration(
					"posts",
					"0001_initial",
					vec![create_table_operation(
						"posts_post",
						vec!["id", "title", "author_id"],
					)],
					vec![("auth", "0001_initial")],
				),
			],
		};

		// Act
		let state = build_state_from_files(&source).await.unwrap();

		// Assert
		assert_eq!(state.models.len(), 2);
		assert!(state.find_model_by_table("auth_users").is_some());
		assert!(state.find_model_by_table("posts_post").is_some());
	}

	/// CreateTable followed by DropTable results in empty state
	#[rstest]
	#[tokio::test]
	async fn test_create_then_drop_results_in_empty() {
		// Arrange
		let source = MockMigrationSource {
			migrations: vec![
				create_migration(
					"temp",
					"0001_initial",
					vec![create_table_operation("temp_data", vec!["id", "value"])],
					vec![],
				),
				create_migration(
					"temp",
					"0002_drop",
					vec![Operation::DropTable {
						name: "temp_data".to_string(),
					}],
					vec![("temp", "0001_initial")],
				),
			],
		};

		// Act
		let state = build_state_from_files(&source).await.unwrap();

		// Assert
		assert!(state.models.is_empty());
	}

	/// Multiple apps with no dependencies are all present in state
	#[rstest]
	#[tokio::test]
	async fn test_multiple_independent_apps() {
		// Arrange
		let source = MockMigrationSource {
			migrations: vec![
				create_migration(
					"auth",
					"0001_initial",
					vec![create_table_operation("auth_users", vec!["id", "username"])],
					vec![],
				),
				create_migration(
					"clusters",
					"0001_initial",
					vec![create_table_operation(
						"clusters_cluster",
						vec!["id", "name"],
					)],
					vec![],
				),
				create_migration(
					"deployments",
					"0001_initial",
					vec![create_table_operation(
						"deployments_deployment",
						vec!["id", "status"],
					)],
					vec![],
				),
			],
		};

		// Act
		let state = build_state_from_files(&source).await.unwrap();

		// Assert
		assert_eq!(state.models.len(), 3);
		assert!(state.find_model_by_table("auth_users").is_some());
		assert!(state.find_model_by_table("clusters_cluster").is_some());
		assert!(
			state
				.find_model_by_table("deployments_deployment")
				.is_some()
		);
	}

	/// RenameColumn is correctly reflected in state
	#[rstest]
	#[tokio::test]
	async fn test_rename_column_reflected() {
		// Arrange
		let source = MockMigrationSource {
			migrations: vec![
				create_migration(
					"auth",
					"0001_initial",
					vec![create_table_operation("auth_users", vec!["id", "name"])],
					vec![],
				),
				create_migration(
					"auth",
					"0002_rename",
					vec![Operation::RenameColumn {
						table: "auth_users".to_string(),
						old_name: "name".to_string(),
						new_name: "full_name".to_string(),
					}],
					vec![("auth", "0001_initial")],
				),
			],
		};

		// Act
		let state = build_state_from_files(&source).await.unwrap();

		// Assert
		let model = state.find_model_by_table("auth_users").unwrap();
		assert_eq!(model.fields.len(), 2);
		assert!(model.fields.contains_key("id"));
		assert!(!model.fields.contains_key("name"));
		assert!(model.fields.contains_key("full_name"));
	}

	/// RenameTable is correctly reflected in state
	#[rstest]
	#[tokio::test]
	async fn test_rename_table_reflected() {
		// Arrange
		let source = MockMigrationSource {
			migrations: vec![
				create_migration(
					"auth",
					"0001_initial",
					vec![create_table_operation("auth_users", vec!["id", "name"])],
					vec![],
				),
				create_migration(
					"auth",
					"0002_rename_table",
					vec![Operation::RenameTable {
						old_name: "auth_users".to_string(),
						new_name: "auth_accounts".to_string(),
					}],
					vec![("auth", "0001_initial")],
				),
			],
		};

		// Act
		let state = build_state_from_files(&source).await.unwrap();

		// Assert
		assert_eq!(state.models.len(), 1);
		assert!(state.find_model_by_table("auth_users").is_none());
		assert!(state.find_model_by_table("auth_accounts").is_some());
	}

	/// DropColumn removes the field from state
	#[rstest]
	#[tokio::test]
	async fn test_drop_column_reflected() {
		// Arrange
		let source = MockMigrationSource {
			migrations: vec![
				create_migration(
					"auth",
					"0001_initial",
					vec![create_table_operation(
						"auth_users",
						vec!["id", "username", "legacy_field"],
					)],
					vec![],
				),
				create_migration(
					"auth",
					"0002_drop_legacy",
					vec![Operation::DropColumn {
						table: "auth_users".to_string(),
						column: "legacy_field".to_string(),
					}],
					vec![("auth", "0001_initial")],
				),
			],
		};

		// Act
		let state = build_state_from_files(&source).await.unwrap();

		// Assert
		let model = state.find_model_by_table("auth_users").unwrap();
		assert_eq!(model.fields.len(), 2);
		assert!(model.fields.contains_key("id"));
		assert!(model.fields.contains_key("username"));
		assert!(!model.fields.contains_key("legacy_field"));
	}

	/// AlterColumn changes the field type in state
	#[rstest]
	#[tokio::test]
	async fn test_alter_column_reflected() {
		// Arrange
		let source = MockMigrationSource {
			migrations: vec![
				create_migration(
					"auth",
					"0001_initial",
					vec![create_table_operation("auth_users", vec!["id", "email"])],
					vec![],
				),
				create_migration(
					"auth",
					"0002_alter_email",
					vec![Operation::AlterColumn {
						table: "auth_users".to_string(),
						column: "email".to_string(),
						old_definition: Some(ColumnDefinition {
							name: "email".to_string(),
							type_definition: FieldType::VarChar(255),
							not_null: false,
							primary_key: false,
							unique: false,
							auto_increment: false,
							default: None,
						}),
						new_definition: ColumnDefinition {
							name: "email".to_string(),
							type_definition: FieldType::Text,
							not_null: true,
							primary_key: false,
							unique: true,
							auto_increment: false,
							default: None,
						},
						mysql_options: None,
					}],
					vec![("auth", "0001_initial")],
				),
			],
		};

		// Act
		let state = build_state_from_files(&source).await.unwrap();

		// Assert
		let model = state.find_model_by_table("auth_users").unwrap();
		let email_field = model.fields.get("email").unwrap();
		assert_eq!(email_field.field_type, FieldType::Text);
	}

	/// Complex multi-app multi-migration scenario matching Issue #3199
	///
	/// Reproduces the exact scenario from Issue #3199: auth, clusters, and
	/// deployments apps with existing migrations, then a new field is added
	/// to auth. The reconstructed state should contain all three tables
	/// with correct columns, proving that the autodetector would generate
	/// AddColumn instead of CreateTable.
	#[rstest]
	#[tokio::test]
	async fn test_issue_3199_scenario_reconstruction() {
		// Arrange
		let source = MockMigrationSource {
			migrations: vec![
				// auth app: initial + add field
				create_migration(
					"auth",
					"0001_initial",
					vec![create_table_operation(
						"auth_users",
						vec!["id", "username", "password"],
					)],
					vec![],
				),
				create_migration(
					"auth",
					"0002_add_names",
					vec![
						add_column_operation("auth_users", "first_name"),
						add_column_operation("auth_users", "last_name"),
					],
					vec![("auth", "0001_initial")],
				),
				// clusters app: initial only
				create_migration(
					"clusters",
					"0001_initial",
					vec![create_table_operation(
						"clusters_cluster",
						vec!["id", "name", "region"],
					)],
					vec![],
				),
				// deployments app: initial only, depends on clusters
				create_migration(
					"deployments",
					"0001_initial",
					vec![create_table_operation(
						"deployments_deployment",
						vec!["id", "cluster_id", "status"],
					)],
					vec![("clusters", "0001_initial")],
				),
			],
		};

		// Act
		let state = build_state_from_files(&source).await.unwrap();

		// Assert: all 3 tables exist with correct columns
		assert_eq!(state.models.len(), 3);

		// auth_users: id, username, password, first_name, last_name
		let auth_model = state.find_model_by_table("auth_users").unwrap();
		assert_eq!(auth_model.fields.len(), 5);
		assert!(auth_model.fields.contains_key("first_name"));
		assert!(auth_model.fields.contains_key("last_name"));

		// clusters_cluster: id, name, region (unchanged)
		let clusters_model = state.find_model_by_table("clusters_cluster").unwrap();
		assert_eq!(clusters_model.fields.len(), 3);

		// deployments_deployment: id, cluster_id, status (unchanged)
		let deployments_model = state.find_model_by_table("deployments_deployment").unwrap();
		assert_eq!(deployments_model.fields.len(), 3);
	}

	/// Long migration chain with multiple operations per migration
	#[rstest]
	#[tokio::test]
	async fn test_long_migration_chain() {
		// Arrange
		let source = MockMigrationSource {
			migrations: vec![
				create_migration(
					"blog",
					"0001_initial",
					vec![create_table_operation("blog_post", vec!["id", "title"])],
					vec![],
				),
				create_migration(
					"blog",
					"0002_add_body",
					vec![add_column_operation("blog_post", "body")],
					vec![("blog", "0001_initial")],
				),
				create_migration(
					"blog",
					"0003_add_author",
					vec![add_column_operation("blog_post", "author_id")],
					vec![("blog", "0002_add_body")],
				),
				create_migration(
					"blog",
					"0004_add_timestamps",
					vec![
						add_column_operation("blog_post", "created_at"),
						add_column_operation("blog_post", "updated_at"),
					],
					vec![("blog", "0003_add_author")],
				),
				create_migration(
					"blog",
					"0005_add_category",
					vec![
						create_table_operation("blog_category", vec!["id", "name"]),
						add_column_operation("blog_post", "category_id"),
					],
					vec![("blog", "0004_add_timestamps")],
				),
			],
		};

		// Act
		let state = build_state_from_files(&source).await.unwrap();

		// Assert
		assert_eq!(state.models.len(), 2);

		let post = state.find_model_by_table("blog_post").unwrap();
		assert_eq!(post.fields.len(), 7); // id, title, body, author_id, created_at, updated_at, category_id

		let category = state.find_model_by_table("blog_category").unwrap();
		assert_eq!(category.fields.len(), 2); // id, name
	}

	/// Merge migration (empty operations, multiple dependencies) does not affect state
	///
	/// After a migration conflict (two branches adding different columns),
	/// a merge migration resolves the conflict. The merge migration has empty
	/// operations and depends on both conflicting leaves. The final state
	/// should contain all columns from both branches.
	#[rstest]
	#[tokio::test]
	async fn test_merge_migration_preserves_state() {
		// Arrange: simulate branch conflict + merge
		// 0001_initial: CreateTable
		// 0002_add_email (branch A): AddColumn email
		// 0002_add_phone (branch B): AddColumn phone
		// 0003_merge: empty operations, depends on both 0002s
		let source = MockMigrationSource {
			migrations: vec![
				create_migration(
					"contacts",
					"0001_initial",
					vec![create_table_operation(
						"contacts_person",
						vec!["id", "name"],
					)],
					vec![],
				),
				create_migration(
					"contacts",
					"0002_add_email",
					vec![add_column_operation("contacts_person", "email")],
					vec![("contacts", "0001_initial")],
				),
				create_migration(
					"contacts",
					"0002_add_phone",
					vec![add_column_operation("contacts_person", "phone")],
					vec![("contacts", "0001_initial")],
				),
				// Merge migration: empty operations, depends on both branches
				create_migration(
					"contacts",
					"0003_merge_0002_add_email_0002_add_phone",
					vec![],
					vec![
						("contacts", "0002_add_email"),
						("contacts", "0002_add_phone"),
					],
				),
			],
		};

		// Act
		let state = build_state_from_files(&source).await.unwrap();

		// Assert: table has all columns from both branches
		assert_eq!(state.models.len(), 1);
		let model = state.find_model_by_table("contacts_person").unwrap();
		assert_eq!(model.fields.len(), 4); // id, name, email, phone
		assert!(model.fields.contains_key("id"));
		assert!(model.fields.contains_key("name"));
		assert!(model.fields.contains_key("email"));
		assert!(model.fields.contains_key("phone"));
	}

	/// Merge migration with subsequent migrations after the merge point
	#[rstest]
	#[tokio::test]
	async fn test_merge_then_continue_adding_columns() {
		// Arrange
		let source = MockMigrationSource {
			migrations: vec![
				create_migration(
					"auth",
					"0001_initial",
					vec![create_table_operation("auth_users", vec!["id", "username"])],
					vec![],
				),
				create_migration(
					"auth",
					"0002_add_email",
					vec![add_column_operation("auth_users", "email")],
					vec![("auth", "0001_initial")],
				),
				create_migration(
					"auth",
					"0002_add_avatar",
					vec![add_column_operation("auth_users", "avatar_url")],
					vec![("auth", "0001_initial")],
				),
				// Merge migration
				create_migration(
					"auth",
					"0003_merge",
					vec![],
					vec![("auth", "0002_add_email"), ("auth", "0002_add_avatar")],
				),
				// Post-merge migration
				create_migration(
					"auth",
					"0004_add_bio",
					vec![add_column_operation("auth_users", "bio")],
					vec![("auth", "0003_merge")],
				),
			],
		};

		// Act
		let state = build_state_from_files(&source).await.unwrap();

		// Assert: all columns present including post-merge addition
		let model = state.find_model_by_table("auth_users").unwrap();
		assert_eq!(model.fields.len(), 5); // id, username, email, avatar_url, bio
		assert!(model.fields.contains_key("bio"));
	}

	/// Cross-app merge: two apps each have a branch conflict, resolved by merge
	#[rstest]
	#[tokio::test]
	async fn test_cross_app_merge_migrations() {
		// Arrange
		let source = MockMigrationSource {
			migrations: vec![
				// auth app: initial + branch conflict + merge
				create_migration(
					"auth",
					"0001_initial",
					vec![create_table_operation("auth_users", vec!["id", "name"])],
					vec![],
				),
				create_migration(
					"auth",
					"0002_add_email",
					vec![add_column_operation("auth_users", "email")],
					vec![("auth", "0001_initial")],
				),
				create_migration(
					"auth",
					"0002_add_role",
					vec![add_column_operation("auth_users", "role")],
					vec![("auth", "0001_initial")],
				),
				create_migration(
					"auth",
					"0003_merge",
					vec![],
					vec![("auth", "0002_add_email"), ("auth", "0002_add_role")],
				),
				// posts app: depends on auth, has its own branch conflict + merge
				create_migration(
					"posts",
					"0001_initial",
					vec![create_table_operation(
						"posts_post",
						vec!["id", "title", "author_id"],
					)],
					vec![("auth", "0001_initial")],
				),
				create_migration(
					"posts",
					"0002_add_body",
					vec![add_column_operation("posts_post", "body")],
					vec![("posts", "0001_initial")],
				),
				create_migration(
					"posts",
					"0002_add_slug",
					vec![add_column_operation("posts_post", "slug")],
					vec![("posts", "0001_initial")],
				),
				create_migration(
					"posts",
					"0003_merge",
					vec![],
					vec![("posts", "0002_add_body"), ("posts", "0002_add_slug")],
				),
			],
		};

		// Act
		let state = build_state_from_files(&source).await.unwrap();

		// Assert
		assert_eq!(state.models.len(), 2);

		let users = state.find_model_by_table("auth_users").unwrap();
		assert_eq!(users.fields.len(), 4); // id, name, email, role

		let posts = state.find_model_by_table("posts_post").unwrap();
		assert_eq!(posts.fields.len(), 5); // id, title, author_id, body, slug
	}
}

/// Full-path integration tests using `FilesystemSource` with real migration files on disk.
///
/// These tests verify the complete pipeline: write `.rs` migration files to a temp directory,
/// load them via `FilesystemSource`, and reconstruct `ProjectState` via `build_state_from_files`.
#[cfg(test)]
mod filesystem_integration_tests {
	use super::*;
	use crate::migrations::FilesystemSource;
	use rstest::rstest;
	use tempfile::TempDir;

	/// Helper to write a migration file to the expected directory structure
	fn write_migration_file(base: &std::path::Path, app: &str, name: &str, content: &str) {
		let dir = base.join(app);
		std::fs::create_dir_all(&dir).unwrap();
		let file_path = dir.join(format!("{}.rs", name));
		std::fs::write(file_path, content).unwrap();
	}

	/// Single app with one initial migration file on disk
	#[rstest]
	#[tokio::test]
	async fn test_single_app_initial_migration_from_files() {
		// Arrange
		let tmp = TempDir::new().unwrap();
		write_migration_file(
			tmp.path(),
			"todos",
			"0001_initial",
			r#"
use reinhardt_db::migrations::prelude::*;

pub fn migration() -> Migration {
	Migration {
		name: "0001_initial".to_string(),
		app_label: "todos".to_string(),
		operations: vec![
			Operation::CreateTable {
				name: "todos_task".to_string(),
				columns: vec![
					ColumnDefinition {
						name: "id".to_string(),
						type_definition: FieldType::Serial,
						not_null: true,
						primary_key: true,
						unique: false,
						auto_increment: true,
						default: None,
					},
					ColumnDefinition {
						name: "title".to_string(),
						type_definition: FieldType::VarChar(255),
						not_null: true,
						primary_key: false,
						unique: false,
						auto_increment: false,
						default: None,
					},
				],
				constraints: vec![],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
		],
		dependencies: vec![],
		replaces: vec![],
		atomic: true,
		initial: Some(true),
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	}
}

pub fn dependencies() -> Vec<(String, String)> {
	vec![]
}

pub fn atomic() -> bool {
	true
}

pub fn replaces() -> Vec<(String, String)> {
	vec![]
}
"#,
		);

		let source = FilesystemSource::new(tmp.path());

		// Act
		let state = build_state_from_files(&source).await.unwrap();

		// Assert
		assert_eq!(state.models.len(), 1);
		let model = state.find_model_by_table("todos_task").unwrap();
		assert!(model.fields.contains_key("id"));
		assert!(model.fields.contains_key("title"));
	}

	/// Two apps with cross-app dependency, loaded from disk
	#[rstest]
	#[tokio::test]
	async fn test_cross_app_migrations_from_files() {
		// Arrange
		let tmp = TempDir::new().unwrap();

		// auth app: 0001_initial
		write_migration_file(
			tmp.path(),
			"auth",
			"0001_initial",
			r#"
use reinhardt_db::migrations::prelude::*;

pub fn migration() -> Migration {
	Migration {
		name: "0001_initial".to_string(),
		app_label: "auth".to_string(),
		operations: vec![
			Operation::CreateTable {
				name: "auth_users".to_string(),
				columns: vec![
					ColumnDefinition {
						name: "id".to_string(),
						type_definition: FieldType::Serial,
						not_null: true,
						primary_key: true,
						unique: false,
						auto_increment: true,
						default: None,
					},
					ColumnDefinition {
						name: "username".to_string(),
						type_definition: FieldType::VarChar(150),
						not_null: true,
						primary_key: false,
						unique: true,
						auto_increment: false,
						default: None,
					},
				],
				constraints: vec![],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
		],
		dependencies: vec![],
		replaces: vec![],
		atomic: true,
		initial: Some(true),
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	}
}

pub fn dependencies() -> Vec<(String, String)> {
	vec![]
}

pub fn atomic() -> bool {
	true
}

pub fn replaces() -> Vec<(String, String)> {
	vec![]
}
"#,
		);

		// posts app: 0001_initial (depends on auth/0001_initial)
		write_migration_file(
			tmp.path(),
			"posts",
			"0001_initial",
			r#"
use reinhardt_db::migrations::prelude::*;

pub fn migration() -> Migration {
	Migration {
		name: "0001_initial".to_string(),
		app_label: "posts".to_string(),
		operations: vec![
			Operation::CreateTable {
				name: "posts_post".to_string(),
				columns: vec![
					ColumnDefinition {
						name: "id".to_string(),
						type_definition: FieldType::Serial,
						not_null: true,
						primary_key: true,
						unique: false,
						auto_increment: true,
						default: None,
					},
					ColumnDefinition {
						name: "title".to_string(),
						type_definition: FieldType::VarChar(200),
						not_null: true,
						primary_key: false,
						unique: false,
						auto_increment: false,
						default: None,
					},
					ColumnDefinition {
						name: "author_id".to_string(),
						type_definition: FieldType::Integer,
						not_null: true,
						primary_key: false,
						unique: false,
						auto_increment: false,
						default: None,
					},
				],
				constraints: vec![],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
		],
		dependencies: vec![
			("auth".to_string(), "0001_initial".to_string()),
		],
		replaces: vec![],
		atomic: true,
		initial: Some(true),
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	}
}

pub fn dependencies() -> Vec<(String, String)> {
	vec![
		("auth".to_string(), "0001_initial".to_string()),
	]
}

pub fn atomic() -> bool {
	true
}

pub fn replaces() -> Vec<(String, String)> {
	vec![]
}
"#,
		);

		let source = FilesystemSource::new(tmp.path());

		// Act
		let state = build_state_from_files(&source).await.unwrap();

		// Assert
		assert_eq!(state.models.len(), 2);
		assert!(state.find_model_by_table("auth_users").is_some());
		assert!(state.find_model_by_table("posts_post").is_some());
		let posts_model = state.find_model_by_table("posts_post").unwrap();
		assert_eq!(posts_model.fields.len(), 3);
	}

	/// Issue #3199 reproduction: chained migrations with AddColumn from disk
	#[rstest]
	#[tokio::test]
	async fn test_issue_3199_add_column_from_files() {
		// Arrange
		let tmp = TempDir::new().unwrap();

		// auth/0001_initial: CreateTable
		write_migration_file(
			tmp.path(),
			"auth",
			"0001_initial",
			r#"
use reinhardt_db::migrations::prelude::*;

pub fn migration() -> Migration {
	Migration {
		name: "0001_initial".to_string(),
		app_label: "auth".to_string(),
		operations: vec![
			Operation::CreateTable {
				name: "auth_users".to_string(),
				columns: vec![
					ColumnDefinition {
						name: "id".to_string(),
						type_definition: FieldType::Serial,
						not_null: true,
						primary_key: true,
						unique: false,
						auto_increment: true,
						default: None,
					},
					ColumnDefinition {
						name: "username".to_string(),
						type_definition: FieldType::VarChar(150),
						not_null: true,
						primary_key: false,
						unique: true,
						auto_increment: false,
						default: None,
					},
				],
				constraints: vec![],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
		],
		dependencies: vec![],
		replaces: vec![],
		atomic: true,
		initial: Some(true),
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	}
}

pub fn dependencies() -> Vec<(String, String)> {
	vec![]
}

pub fn atomic() -> bool {
	true
}

pub fn replaces() -> Vec<(String, String)> {
	vec![]
}
"#,
		);

		// auth/0002_add_names: AddColumn (first_name, last_name)
		write_migration_file(
			tmp.path(),
			"auth",
			"0002_add_names",
			r#"
use reinhardt_db::migrations::prelude::*;

pub fn migration() -> Migration {
	Migration {
		name: "0002_add_names".to_string(),
		app_label: "auth".to_string(),
		operations: vec![
			Operation::AddColumn {
				table: "auth_users".to_string(),
				column: ColumnDefinition {
					name: "first_name".to_string(),
					type_definition: FieldType::VarChar(100),
					not_null: false,
					primary_key: false,
					unique: false,
					auto_increment: false,
					default: None,
				},
				mysql_options: None,
			},
			Operation::AddColumn {
				table: "auth_users".to_string(),
				column: ColumnDefinition {
					name: "last_name".to_string(),
					type_definition: FieldType::VarChar(100),
					not_null: false,
					primary_key: false,
					unique: false,
					auto_increment: false,
					default: None,
				},
				mysql_options: None,
			},
		],
		dependencies: vec![
			("auth".to_string(), "0001_initial".to_string()),
		],
		replaces: vec![],
		atomic: true,
		initial: None,
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	}
}

pub fn dependencies() -> Vec<(String, String)> {
	vec![
		("auth".to_string(), "0001_initial".to_string()),
	]
}

pub fn atomic() -> bool {
	true
}

pub fn replaces() -> Vec<(String, String)> {
	vec![]
}
"#,
		);

		let source = FilesystemSource::new(tmp.path());

		// Act
		let state = build_state_from_files(&source).await.unwrap();

		// Assert: auth_users has all 4 columns (not a duplicate CreateTable)
		assert_eq!(state.models.len(), 1);
		let model = state.find_model_by_table("auth_users").unwrap();
		assert_eq!(model.fields.len(), 4);
		assert!(model.fields.contains_key("id"));
		assert!(model.fields.contains_key("username"));
		assert!(model.fields.contains_key("first_name"));
		assert!(model.fields.contains_key("last_name"));
	}

	/// Empty migrations directory returns empty state
	#[rstest]
	#[tokio::test]
	async fn test_empty_directory_returns_empty_state() {
		// Arrange
		let tmp = TempDir::new().unwrap();
		let source = FilesystemSource::new(tmp.path());

		// Act
		let state = build_state_from_files(&source).await.unwrap();

		// Assert
		assert!(state.models.is_empty());
	}

	/// Merge migration scenario: branch conflict resolved by a merge migration,
	/// all loaded from real `.rs` files on disk via FilesystemSource.
	///
	/// Simulates the --merge workflow:
	/// 1. 0001_initial: CreateTable with id, username
	/// 2. 0002_add_email (branch A): AddColumn email
	/// 3. 0002_add_phone (branch B): AddColumn phone
	/// 4. 0003_merge: empty operations, depends on both 0002s
	#[rstest]
	#[tokio::test]
	async fn test_merge_migration_from_files() {
		// Arrange
		let tmp = TempDir::new().unwrap();

		// 0001_initial
		write_migration_file(
			tmp.path(),
			"contacts",
			"0001_initial",
			r#"
use reinhardt_db::migrations::prelude::*;

pub fn migration() -> Migration {
	Migration {
		name: "0001_initial".to_string(),
		app_label: "contacts".to_string(),
		operations: vec![
			Operation::CreateTable {
				name: "contacts_person".to_string(),
				columns: vec![
					ColumnDefinition {
						name: "id".to_string(),
						type_definition: FieldType::Serial,
						not_null: true,
						primary_key: true,
						unique: false,
						auto_increment: true,
						default: None,
					},
					ColumnDefinition {
						name: "username".to_string(),
						type_definition: FieldType::VarChar(150),
						not_null: true,
						primary_key: false,
						unique: true,
						auto_increment: false,
						default: None,
					},
				],
				constraints: vec![],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
		],
		dependencies: vec![],
		replaces: vec![],
		atomic: true,
		initial: Some(true),
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	}
}

pub fn dependencies() -> Vec<(String, String)> {
	vec![]
}

pub fn atomic() -> bool {
	true
}

pub fn replaces() -> Vec<(String, String)> {
	vec![]
}
"#,
		);

		// 0002_add_email (branch A)
		write_migration_file(
			tmp.path(),
			"contacts",
			"0002_add_email",
			r#"
use reinhardt_db::migrations::prelude::*;

pub fn migration() -> Migration {
	Migration {
		name: "0002_add_email".to_string(),
		app_label: "contacts".to_string(),
		operations: vec![
			Operation::AddColumn {
				table: "contacts_person".to_string(),
				column: ColumnDefinition {
					name: "email".to_string(),
					type_definition: FieldType::VarChar(255),
					not_null: false,
					primary_key: false,
					unique: false,
					auto_increment: false,
					default: None,
				},
				mysql_options: None,
			},
		],
		dependencies: vec![
			("contacts".to_string(), "0001_initial".to_string()),
		],
		replaces: vec![],
		atomic: true,
		initial: None,
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	}
}

pub fn dependencies() -> Vec<(String, String)> {
	vec![
		("contacts".to_string(), "0001_initial".to_string()),
	]
}

pub fn atomic() -> bool {
	true
}

pub fn replaces() -> Vec<(String, String)> {
	vec![]
}
"#,
		);

		// 0002_add_phone (branch B)
		write_migration_file(
			tmp.path(),
			"contacts",
			"0002_add_phone",
			r#"
use reinhardt_db::migrations::prelude::*;

pub fn migration() -> Migration {
	Migration {
		name: "0002_add_phone".to_string(),
		app_label: "contacts".to_string(),
		operations: vec![
			Operation::AddColumn {
				table: "contacts_person".to_string(),
				column: ColumnDefinition {
					name: "phone".to_string(),
					type_definition: FieldType::VarChar(20),
					not_null: false,
					primary_key: false,
					unique: false,
					auto_increment: false,
					default: None,
				},
				mysql_options: None,
			},
		],
		dependencies: vec![
			("contacts".to_string(), "0001_initial".to_string()),
		],
		replaces: vec![],
		atomic: true,
		initial: None,
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	}
}

pub fn dependencies() -> Vec<(String, String)> {
	vec![
		("contacts".to_string(), "0001_initial".to_string()),
	]
}

pub fn atomic() -> bool {
	true
}

pub fn replaces() -> Vec<(String, String)> {
	vec![]
}
"#,
		);

		// 0003_merge: empty operations, depends on both branches
		write_migration_file(
			tmp.path(),
			"contacts",
			"0003_merge_0002_add_email_0002_add_phone",
			r#"
use reinhardt_db::migrations::prelude::*;

pub fn migration() -> Migration {
	Migration {
		name: "0003_merge_0002_add_email_0002_add_phone".to_string(),
		app_label: "contacts".to_string(),
		operations: vec![],
		dependencies: vec![
			("contacts".to_string(), "0002_add_email".to_string()),
			("contacts".to_string(), "0002_add_phone".to_string()),
		],
		replaces: vec![],
		atomic: true,
		initial: None,
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	}
}

pub fn dependencies() -> Vec<(String, String)> {
	vec![
		("contacts".to_string(), "0002_add_email".to_string()),
		("contacts".to_string(), "0002_add_phone".to_string()),
	]
}

pub fn atomic() -> bool {
	true
}

pub fn replaces() -> Vec<(String, String)> {
	vec![]
}
"#,
		);

		let source = FilesystemSource::new(tmp.path());

		// Act
		let state = build_state_from_files(&source).await.unwrap();

		// Assert: table has all columns from both branches
		assert_eq!(state.models.len(), 1);
		let model = state.find_model_by_table("contacts_person").unwrap();
		assert_eq!(model.fields.len(), 4); // id, username, email, phone
		assert!(model.fields.contains_key("id"));
		assert!(model.fields.contains_key("username"));
		assert!(model.fields.contains_key("email"));
		assert!(model.fields.contains_key("phone"));
	}

	/// Post-merge workflow: merge migration followed by additional migrations,
	/// loaded from real `.rs` files on disk.
	#[rstest]
	#[tokio::test]
	async fn test_post_merge_migration_from_files() {
		// Arrange
		let tmp = TempDir::new().unwrap();

		// 0001_initial
		write_migration_file(
			tmp.path(),
			"auth",
			"0001_initial",
			r#"
use reinhardt_db::migrations::prelude::*;

pub fn migration() -> Migration {
	Migration {
		name: "0001_initial".to_string(),
		app_label: "auth".to_string(),
		operations: vec![
			Operation::CreateTable {
				name: "auth_users".to_string(),
				columns: vec![
					ColumnDefinition {
						name: "id".to_string(),
						type_definition: FieldType::Serial,
						not_null: true,
						primary_key: true,
						unique: false,
						auto_increment: true,
						default: None,
					},
					ColumnDefinition {
						name: "username".to_string(),
						type_definition: FieldType::VarChar(150),
						not_null: true,
						primary_key: false,
						unique: true,
						auto_increment: false,
						default: None,
					},
				],
				constraints: vec![],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
		],
		dependencies: vec![],
		replaces: vec![],
		atomic: true,
		initial: Some(true),
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	}
}

pub fn dependencies() -> Vec<(String, String)> {
	vec![]
}

pub fn atomic() -> bool {
	true
}

pub fn replaces() -> Vec<(String, String)> {
	vec![]
}
"#,
		);

		// 0002_add_email (branch A)
		write_migration_file(
			tmp.path(),
			"auth",
			"0002_add_email",
			r#"
use reinhardt_db::migrations::prelude::*;

pub fn migration() -> Migration {
	Migration {
		name: "0002_add_email".to_string(),
		app_label: "auth".to_string(),
		operations: vec![
			Operation::AddColumn {
				table: "auth_users".to_string(),
				column: ColumnDefinition {
					name: "email".to_string(),
					type_definition: FieldType::VarChar(255),
					not_null: false,
					primary_key: false,
					unique: false,
					auto_increment: false,
					default: None,
				},
				mysql_options: None,
			},
		],
		dependencies: vec![
			("auth".to_string(), "0001_initial".to_string()),
		],
		replaces: vec![],
		atomic: true,
		initial: None,
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	}
}

pub fn dependencies() -> Vec<(String, String)> {
	vec![("auth".to_string(), "0001_initial".to_string())]
}

pub fn atomic() -> bool {
	true
}

pub fn replaces() -> Vec<(String, String)> {
	vec![]
}
"#,
		);

		// 0002_add_avatar (branch B)
		write_migration_file(
			tmp.path(),
			"auth",
			"0002_add_avatar",
			r#"
use reinhardt_db::migrations::prelude::*;

pub fn migration() -> Migration {
	Migration {
		name: "0002_add_avatar".to_string(),
		app_label: "auth".to_string(),
		operations: vec![
			Operation::AddColumn {
				table: "auth_users".to_string(),
				column: ColumnDefinition {
					name: "avatar_url".to_string(),
					type_definition: FieldType::VarChar(500),
					not_null: false,
					primary_key: false,
					unique: false,
					auto_increment: false,
					default: None,
				},
				mysql_options: None,
			},
		],
		dependencies: vec![
			("auth".to_string(), "0001_initial".to_string()),
		],
		replaces: vec![],
		atomic: true,
		initial: None,
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	}
}

pub fn dependencies() -> Vec<(String, String)> {
	vec![("auth".to_string(), "0001_initial".to_string())]
}

pub fn atomic() -> bool {
	true
}

pub fn replaces() -> Vec<(String, String)> {
	vec![]
}
"#,
		);

		// 0003_merge
		write_migration_file(
			tmp.path(),
			"auth",
			"0003_merge",
			r#"
use reinhardt_db::migrations::prelude::*;

pub fn migration() -> Migration {
	Migration {
		name: "0003_merge".to_string(),
		app_label: "auth".to_string(),
		operations: vec![],
		dependencies: vec![
			("auth".to_string(), "0002_add_email".to_string()),
			("auth".to_string(), "0002_add_avatar".to_string()),
		],
		replaces: vec![],
		atomic: true,
		initial: None,
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	}
}

pub fn dependencies() -> Vec<(String, String)> {
	vec![
		("auth".to_string(), "0002_add_email".to_string()),
		("auth".to_string(), "0002_add_avatar".to_string()),
	]
}

pub fn atomic() -> bool {
	true
}

pub fn replaces() -> Vec<(String, String)> {
	vec![]
}
"#,
		);

		// 0004_add_bio (post-merge)
		write_migration_file(
			tmp.path(),
			"auth",
			"0004_add_bio",
			r#"
use reinhardt_db::migrations::prelude::*;

pub fn migration() -> Migration {
	Migration {
		name: "0004_add_bio".to_string(),
		app_label: "auth".to_string(),
		operations: vec![
			Operation::AddColumn {
				table: "auth_users".to_string(),
				column: ColumnDefinition {
					name: "bio".to_string(),
					type_definition: FieldType::Text,
					not_null: false,
					primary_key: false,
					unique: false,
					auto_increment: false,
					default: None,
				},
				mysql_options: None,
			},
		],
		dependencies: vec![
			("auth".to_string(), "0003_merge".to_string()),
		],
		replaces: vec![],
		atomic: true,
		initial: None,
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	}
}

pub fn dependencies() -> Vec<(String, String)> {
	vec![("auth".to_string(), "0003_merge".to_string())]
}

pub fn atomic() -> bool {
	true
}

pub fn replaces() -> Vec<(String, String)> {
	vec![]
}
"#,
		);

		let source = FilesystemSource::new(tmp.path());

		// Act
		let state = build_state_from_files(&source).await.unwrap();

		// Assert: all columns present including post-merge addition
		assert_eq!(state.models.len(), 1);
		let model = state.find_model_by_table("auth_users").unwrap();
		assert_eq!(model.fields.len(), 5); // id, username, email, avatar_url, bio
		assert!(model.fields.contains_key("id"));
		assert!(model.fields.contains_key("username"));
		assert!(model.fields.contains_key("email"));
		assert!(model.fields.contains_key("avatar_url"));
		assert!(model.fields.contains_key("bio"));
	}
}

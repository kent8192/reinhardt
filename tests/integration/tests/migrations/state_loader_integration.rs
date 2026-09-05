//! Integration tests for MigrationStateLoader
//!
//! These tests verify that MigrationStateLoader correctly builds ProjectState
//! by replaying migration history, following the Django-style approach.

use reinhardt_db::backends::DatabaseConnection;
use reinhardt_db::migrations::{
	ColumnDefinition, DatabaseMigrationRecorder, FieldType, Migration, MigrationSource,
	MigrationStateLoader, Operation,
};
use serial_test::serial;

/// Helper struct for in-memory migration source (for testing)
#[derive(Clone)]
struct InMemoryMigrationSource {
	migrations: Vec<Migration>,
}

impl InMemoryMigrationSource {
	fn new(migrations: Vec<Migration>) -> Self {
		Self { migrations }
	}
}

#[async_trait::async_trait]
impl MigrationSource for InMemoryMigrationSource {
	async fn all_migrations(&self) -> reinhardt_db::migrations::Result<Vec<Migration>> {
		Ok(self.migrations.clone())
	}
}

/// Test that empty migration history returns empty ProjectState
#[tokio::test]
#[serial(state_loader_migrations)]
async fn test_empty_migration_history_returns_empty_state() {
	// Use unique in-memory database for each test
	let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
		.await
		.expect("Failed to connect to SQLite");

	let recorder = DatabaseMigrationRecorder::new(connection);

	// Ensure migration table exists
	recorder
		.ensure_schema_table()
		.await
		.expect("Failed to create schema table");

	let source = InMemoryMigrationSource::new(vec![]);
	let loader = MigrationStateLoader::new(recorder, source);

	let state = loader
		.build_current_state()
		.await
		.expect("Failed to build state");

	assert!(state.models.is_empty(), "Expected empty ProjectState");
}

/// Test that a single CreateTable migration is correctly replayed
#[tokio::test]
#[serial(state_loader_migrations)]
async fn test_single_create_table_migration() {
	// Use unique in-memory database for each test
	let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
		.await
		.expect("Failed to connect to SQLite");

	let recorder = DatabaseMigrationRecorder::new(connection.clone());

	// Ensure migration table exists
	recorder
		.ensure_schema_table()
		.await
		.expect("Failed to create schema table");

	// Create and record a migration
	let migration = Migration::from_parts(
		"0001_initial".to_string(),
		"testapp".to_string(),
		vec![Operation::CreateTable {
			name: "test_model".to_string(),
			columns: vec![ColumnDefinition::from_parts(
				"id".to_string(),
				FieldType::BigInteger,
				true,
				false,
				true,
				true,
				None,
				None,
				None,
			)],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
		vec![],
		vec![],
		true,
		Some(true),
		false,
		false,
		vec![],
		vec![],
	);

	// Record the migration as applied
	recorder
		.record_applied("testapp", "0001_initial")
		.await
		.expect("Failed to record migration");

	let source = InMemoryMigrationSource::new(vec![migration]);
	let loader = MigrationStateLoader::new(recorder, source);

	let state = loader
		.build_current_state()
		.await
		.expect("Failed to build state");

	// Verify the model was created in ProjectState
	assert_eq!(state.models.len(), 1, "Expected 1 model in ProjectState");

	// Find the model (key is (app_label, model_name))
	let model = state
		.models
		.values()
		.next()
		.expect("Expected at least one model");

	assert_eq!(model.table_name, "test_model");
	assert!(model.fields.contains_key("id"), "Expected 'id' field");
}

/// Test that multiple migrations are replayed in correct order
#[tokio::test]
#[serial(state_loader_migrations)]
async fn test_multiple_migrations_in_order() {
	// Use unique in-memory database for each test
	let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
		.await
		.expect("Failed to connect to SQLite");

	let recorder = DatabaseMigrationRecorder::new(connection.clone());

	// Ensure migration table exists
	recorder
		.ensure_schema_table()
		.await
		.expect("Failed to create schema table");

	// First migration: Create table
	let migration1 = Migration::from_parts(
		"0001_initial".to_string(),
		"testapp".to_string(),
		vec![Operation::CreateTable {
			name: "users".to_string(),
			columns: vec![ColumnDefinition::from_parts(
				"id".to_string(),
				FieldType::BigInteger,
				true,
				false,
				true,
				true,
				None,
				None,
				None,
			)],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
		vec![],
		vec![],
		true,
		Some(true),
		false,
		false,
		vec![],
		vec![],
	);

	// Second migration: Add column
	let migration2 = Migration::from_parts(
		"0002_add_email".to_string(),
		"testapp".to_string(),
		vec![Operation::AddColumn {
			table: "users".to_string(),
			column: ColumnDefinition::from_parts(
				"email".to_string(),
				FieldType::VarChar(255),
				false,
				false,
				false,
				false,
				None,
				None,
				None,
			),
			mysql_options: None,
		}],
		vec![("testapp".to_string(), "0001_initial".to_string())],
		vec![],
		true,
		Some(false),
		false,
		false,
		vec![],
		vec![],
	);

	// Record both migrations as applied
	recorder
		.record_applied("testapp", "0001_initial")
		.await
		.expect("Failed to record migration 1");

	recorder
		.record_applied("testapp", "0002_add_email")
		.await
		.expect("Failed to record migration 2");

	let source = InMemoryMigrationSource::new(vec![migration1, migration2]);
	let loader = MigrationStateLoader::new(recorder, source);

	let state = loader
		.build_current_state()
		.await
		.expect("Failed to build state");

	// Verify the model has both fields
	let model = state
		.models
		.values()
		.next()
		.expect("Expected at least one model");

	assert_eq!(model.table_name, "users");
	assert!(model.fields.contains_key("id"), "Expected 'id' field");
	assert!(model.fields.contains_key("email"), "Expected 'email' field");
}

/// Test that unapplied migrations are not included in state
#[tokio::test]
#[serial(state_loader_migrations)]
async fn test_unapplied_migrations_not_included() {
	// Use unique in-memory database for each test
	let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
		.await
		.expect("Failed to connect to SQLite");

	let recorder = DatabaseMigrationRecorder::new(connection.clone());

	// Ensure migration table exists
	recorder
		.ensure_schema_table()
		.await
		.expect("Failed to create schema table");

	// First migration: Create table (applied)
	let migration1 = Migration::from_parts(
		"0001_initial".to_string(),
		"testapp".to_string(),
		vec![Operation::CreateTable {
			name: "users".to_string(),
			columns: vec![ColumnDefinition::from_parts(
				"id".to_string(),
				FieldType::BigInteger,
				true,
				false,
				true,
				true,
				None,
				None,
				None,
			)],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
		vec![],
		vec![],
		true,
		Some(true),
		false,
		false,
		vec![],
		vec![],
	);

	// Second migration: Add column (NOT applied)
	let migration2 = Migration::from_parts(
		"0002_add_email".to_string(),
		"testapp".to_string(),
		vec![Operation::AddColumn {
			table: "users".to_string(),
			column: ColumnDefinition::from_parts(
				"email".to_string(),
				FieldType::VarChar(255),
				false,
				false,
				false,
				false,
				None,
				None,
				None,
			),
			mysql_options: None,
		}],
		vec![("testapp".to_string(), "0001_initial".to_string())],
		vec![],
		true,
		Some(false),
		false,
		false,
		vec![],
		vec![],
	);

	// Only record first migration as applied
	recorder
		.record_applied("testapp", "0001_initial")
		.await
		.expect("Failed to record migration 1");

	// Second migration is NOT recorded (simulating unapplied state)

	let source = InMemoryMigrationSource::new(vec![migration1, migration2]);
	let loader = MigrationStateLoader::new(recorder, source);

	let state = loader
		.build_current_state()
		.await
		.expect("Failed to build state");

	// Verify the model only has 'id' field (no 'email' because migration2 wasn't applied)
	let model = state
		.models
		.values()
		.next()
		.expect("Expected at least one model");

	assert_eq!(model.table_name, "users");
	assert!(model.fields.contains_key("id"), "Expected 'id' field");
	assert!(
		!model.fields.contains_key("email"),
		"Email field should NOT be present (migration not applied)"
	);
}

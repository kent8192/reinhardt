#![cfg(all(feature = "migrations", feature = "sqlite"))]

use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use reinhardt_commands::{
	BaseCommand, CommandContext, MigrationVisibilityWriter, ShowMigrationsCommand,
	ShowMigrationsMode, SqlMigrateCommand, format_migration_snapshot, render_migration_sql,
};
use reinhardt_db::backends::DatabaseConnection;
use reinhardt_db::migrations::{
	ColumnDefinition, FieldType, Migration, MigrationCatalog, MigrationDirection, MigrationKey,
	MigrationRenderOptions, MigrationSnapshot, MigrationSource, Operation, Result,
};
use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex};

struct TestSource {
	migrations: Vec<Migration>,
}

#[derive(Default)]
struct RecordingWriter {
	writes: Mutex<Vec<String>>,
}

impl MigrationVisibilityWriter for RecordingWriter {
	fn write_stdout(&self, content: &str) -> io::Result<()> {
		self.writes.lock().unwrap().push(content.to_string());
		Ok(())
	}
}

impl RecordingWriter {
	fn outputs(&self) -> Vec<String> {
		self.writes.lock().unwrap().clone()
	}
}

#[async_trait]
impl MigrationSource for TestSource {
	async fn all_migrations(&self) -> Result<Vec<Migration>> {
		Ok(self.migrations.clone())
	}
}

fn migration(app: &str, name: &str, dependencies: &[(&str, &str)]) -> Migration {
	let mut migration = Migration::new(name, app);
	migration.dependencies = dependencies
		.iter()
		.map(|(app, name)| ((*app).to_string(), (*name).to_string()))
		.collect();
	migration
}

#[test]
fn list_output_is_grouped_with_exact_markers_and_level_two_timestamps() {
	let auth = migration("auth", "0001_initial", &[]);
	let polls = migration("polls", "0001_initial", &[("auth", "0001_initial")]);
	let applied_at = Utc
		.with_ymd_and_hms(2026, 7, 29, 12, 34, 56)
		.single()
		.unwrap();
	let snapshot = MigrationSnapshot {
		ordered: vec![auth, polls],
		applied: HashMap::from([(MigrationKey::new("auth", "0001_initial"), applied_at)]),
	};

	let normal = format_migration_snapshot(&snapshot, ShowMigrationsMode::List, 1);
	let verbose = format_migration_snapshot(&snapshot, ShowMigrationsMode::List, 2);

	assert_eq!(
		normal,
		"auth\n [X] 0001_initial\npolls\n [ ] 0001_initial\n"
	);
	assert_eq!(
		verbose,
		"auth\n [X] 0001_initial (applied at 2026-07-29T12:34:56+00:00)\npolls\n [ ] 0001_initial\n"
	);
}

#[test]
fn plan_output_uses_complete_dependency_order() {
	let snapshot = MigrationSnapshot {
		ordered: vec![
			migration("auth", "0001_initial", &[]),
			migration("polls", "0001_initial", &[("auth", "0001_initial")]),
			migration("polls", "0002_question", &[("polls", "0001_initial")]),
		],
		applied: HashMap::from([(MigrationKey::new("auth", "0001_initial"), Utc::now())]),
	};

	let output = format_migration_snapshot(&snapshot, ShowMigrationsMode::Plan, 0);

	assert_eq!(
		output,
		"[X] auth.0001_initial\n[ ] polls.0001_initial\n[ ] polls.0002_question\n"
	);
}

fn create_books_migration() -> Migration {
	Migration::new("0001_initial", "catalog").add_operation(Operation::CreateTable {
		name: "books".to_string(),
		columns: vec![ColumnDefinition::new("id", FieldType::Integer)],
		constraints: Vec::new(),
		without_rowid: None,
		interleave_in_parent: None,
		partition: None,
	})
}

#[tokio::test]
async fn sql_rendering_uses_two_catalog_states_for_both_directions() {
	let migration = create_books_migration();
	let catalog = MigrationCatalog::load_strict(&TestSource {
		migrations: vec![migration.clone()],
	})
	.await
	.unwrap();
	let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
		.await
		.unwrap();
	let key = MigrationKey::new("catalog", "0001_initial");

	let forward = render_migration_sql(
		&connection,
		&catalog,
		&migration,
		&key,
		MigrationDirection::Forward,
	)
	.await
	.unwrap();
	let backward = render_migration_sql(
		&connection,
		&catalog,
		&migration,
		&key,
		MigrationDirection::Backward,
	)
	.await
	.unwrap();

	assert_eq!(
		forward,
		"BEGIN;\nCREATE TABLE books (\n  id INTEGER\n);\nCOMMIT;\n"
	);
	assert_eq!(backward, "BEGIN;\nDROP TABLE books;\nCOMMIT;\n");
}

#[tokio::test]
async fn unique_prefix_resolves_before_sql_rendering() {
	let migration = create_books_migration();
	let catalog = MigrationCatalog::load_strict(&TestSource {
		migrations: vec![migration],
	})
	.await
	.unwrap();

	let key = catalog.resolve_unique_prefix("catalog", "0001").unwrap();

	assert_eq!(key, MigrationKey::new("catalog", "0001_initial"));
	assert_eq!(catalog.migration(&key).unwrap().name, "0001_initial");
}

#[tokio::test]
async fn showmigrations_writes_one_complete_snapshot_without_creating_history() {
	let migrations = tempfile::tempdir().unwrap();
	let database = tempfile::NamedTempFile::new().unwrap();
	let writer = Arc::new(RecordingWriter::default());
	let command = ShowMigrationsCommand::with_writer(writer.clone());
	let mut context = CommandContext::default();
	context.set_option(
		"database-url".to_string(),
		format!("sqlite:///{}", database.path().display()),
	);
	context.set_option(
		"migrations-dir".to_string(),
		migrations.path().to_string_lossy().into_owned(),
	);

	command.execute(&context).await.unwrap();

	assert_eq!(writer.outputs(), [String::new()]);
	let connection =
		DatabaseConnection::connect_sqlite(&format!("sqlite:///{}", database.path().display()))
			.await
			.unwrap();
	let tables = connection
		.fetch_all(
			"SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name",
			vec![],
		)
		.await
		.unwrap();
	assert!(tables.is_empty());
}

#[tokio::test]
async fn sqlmigrate_resolves_prefix_and_writes_one_complete_script() {
	let migrations = tempfile::tempdir().unwrap();
	let repository = reinhardt_db::migrations::FilesystemRepository::new(migrations.path());
	let migration = create_books_migration();
	let source = repository
		.render(
			&migration,
			MigrationRenderOptions {
				include_header: false,
			},
		)
		.unwrap();
	repository
		.create_new_source("catalog", "0001_initial", &source)
		.unwrap();
	let writer = Arc::new(RecordingWriter::default());
	let command = SqlMigrateCommand::with_writer(writer.clone());
	let mut context = CommandContext::new(vec!["catalog".to_string(), "0001".to_string()]);
	context.set_option("database-url".to_string(), "sqlite::memory:".to_string());
	context.set_option(
		"migrations-dir".to_string(),
		migrations.path().to_string_lossy().into_owned(),
	);

	command.execute(&context).await.unwrap();

	assert_eq!(
		writer.outputs(),
		["BEGIN;\nCREATE TABLE books (\n  id INTEGER\n);\nCOMMIT;\n"]
	);
}

#[tokio::test]
async fn database_selection_errors_redact_credentials_before_output() {
	let migrations = tempfile::tempdir().unwrap();
	let writer = Arc::new(RecordingWriter::default());
	let command = ShowMigrationsCommand::with_writer(writer.clone());
	let mut context = CommandContext::default();
	context.set_option(
		"database-url".to_string(),
		"oracle://admin:redaction-secret@db.example/catalog".to_string(),
	);
	context.set_option(
		"migrations-dir".to_string(),
		migrations.path().to_string_lossy().into_owned(),
	);

	let error = command.execute(&context).await.unwrap_err();
	let diagnostic = error.to_string();

	assert!(diagnostic.contains("oracle"));
	assert!(!diagnostic.contains("redaction-secret"));
	assert!(!diagnostic.contains("db.example"));
	assert!(writer.outputs().is_empty());
}

#[tokio::test]
async fn migration_errors_identify_command_and_redacted_database_alias() {
	let migrations = tempfile::tempdir().unwrap();
	let writer = Arc::new(RecordingWriter::default());
	let command = ShowMigrationsCommand::with_writer(writer);
	let alias = "postgresql://admin:alias-secret@db.example/catalog";
	let mut context = CommandContext::new(vec!["missing_app".to_string()]);
	context.set_option("database".to_string(), alias.to_string());
	context.set_option("database-url".to_string(), "sqlite::memory:".to_string());
	context.set_option(
		"migrations-dir".to_string(),
		migrations.path().to_string_lossy().into_owned(),
	);

	let diagnostic = command.execute(&context).await.unwrap_err().to_string();

	assert!(diagnostic.contains("showmigrations"));
	assert!(diagnostic.contains("[REDACTED]"));
	assert!(!diagnostic.contains(alias));
	assert!(!diagnostic.contains("alias-secret"));
}

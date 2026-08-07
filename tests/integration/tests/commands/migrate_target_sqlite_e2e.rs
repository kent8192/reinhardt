//! SQLite-backed target-migration integration tests.
//!
//! These cases exercise the target-only branches without requiring a container
//! runtime. The temporary database is a file because each command opens its
//! own connection.

#![cfg(feature = "sqlite")]

use reinhardt_commands::{BaseCommand, CommandContext, MigrateCommand};
use reinhardt_db::backends::DatabaseConnection;
use reinhardt_db::migrations::DatabaseMigrationRecorder;
use std::path::Path;
use tempfile::TempDir;

fn write_migration(root: &Path, app: &str, name: &str, dependencies: &[(&str, &str)]) {
	let app_dir = root.join(app);
	std::fs::create_dir_all(&app_dir).expect("create migration app directory");
	let dependencies = dependencies
		.iter()
		.map(|(dependency_app, dependency_name)| {
			format!("(\"{dependency_app}\".to_string(), \"{dependency_name}\".to_string())")
		})
		.collect::<Vec<_>>()
		.join(", ");
	std::fs::write(
		app_dir.join(format!("{name}.rs")),
		format!(
			"use reinhardt::db::migrations::prelude::*;\n\
			 pub(super) fn migration() -> Migration {{\n\
			 \tMigration {{\n\
			 \t\tapp_label: \"{app}\".to_string(),\n\
			 \t\tname: \"{name}\".to_string(),\n\
			 \t\toperations: vec![],\n\
			 \t\tdependencies: vec![{dependencies}],\n\
			 \t\tatomic: true,\n\
			 \t\treplaces: vec![],\n\
			 \t\tinitial: None,\n\
			 \t\tstate_only: false,\n\
			 \t\tdatabase_only: false,\n\
			 \t\tswappable_dependencies: vec![],\n\
			 \t\toptional_dependencies: vec![],\n\
			 \t}}\n\
			 }}\n"
		),
	)
	.expect("write migration file");
}

fn arrange_chain() -> (TempDir, std::path::PathBuf, String) {
	let tempdir = tempfile::tempdir().expect("create temporary migration project");
	let migrations_dir = tempdir.path().join("migrations");
	write_migration(&migrations_dir, "myapp", "0001_first", &[]);
	write_migration(
		&migrations_dir,
		"myapp",
		"0002_second",
		&[("myapp", "0001_first")],
	);
	write_migration(
		&migrations_dir,
		"myapp",
		"0003_third",
		&[("myapp", "0002_second")],
	);
	let database_url = format!(
		"sqlite:{}",
		tempdir.path().join("migrations.sqlite3").display()
	);
	(tempdir, migrations_dir, database_url)
}

fn migration_context(
	migrations_dir: &Path,
	database_url: &str,
	target: &str,
	plan: bool,
	fake: bool,
) -> CommandContext {
	let mut ctx = CommandContext::default();
	ctx.add_arg("myapp".to_string());
	ctx.add_arg(target.to_string());
	ctx.set_option("database".to_string(), database_url.to_string());
	ctx.set_option(
		"migrations-dir".to_string(),
		migrations_dir.to_string_lossy().into_owned(),
	);
	if plan {
		ctx.set_option("plan".to_string(), "true".to_string());
	}
	if fake {
		ctx.set_option("fake".to_string(), "true".to_string());
	}
	ctx
}

async fn record_applied(database_url: &str, names: &[&str]) {
	let connection = DatabaseConnection::connect_sqlite(database_url)
		.await
		.expect("connect SQLite database");
	let recorder = DatabaseMigrationRecorder::new(connection);
	recorder
		.ensure_schema_table()
		.await
		.expect("create migration recorder table");
	for name in names {
		recorder
			.record_applied("myapp", name)
			.await
			.expect("record applied migration");
	}
}

async fn applied_names(database_url: &str) -> Vec<String> {
	let connection = DatabaseConnection::connect_sqlite(database_url)
		.await
		.expect("reconnect SQLite database");
	let recorder = DatabaseMigrationRecorder::new(connection);
	recorder
		.get_applied_migrations()
		.await
		.expect("query applied migrations")
		.into_iter()
		.filter(|record| record.app == "myapp")
		.map(|record| record.name)
		.collect()
}

#[tokio::test]
async fn target_zero_fake_unapplies_every_recorded_migration() {
	let (_tempdir, migrations_dir, database_url) = arrange_chain();
	record_applied(&database_url, &["0001_first", "0002_second", "0003_third"]).await;

	MigrateCommand
		.execute(&migration_context(
			&migrations_dir,
			&database_url,
			"zero",
			false,
			true,
		))
		.await
		.expect("fake zero rollback succeeds");

	assert!(applied_names(&database_url).await.is_empty());
}

#[tokio::test]
async fn target_current_plan_leaves_later_migrations_recorded() {
	let (_tempdir, migrations_dir, database_url) = arrange_chain();
	record_applied(&database_url, &["0001_first", "0002_second", "0003_third"]).await;

	MigrateCommand
		.execute(&migration_context(
			&migrations_dir,
			&database_url,
			"0001_first",
			true,
			false,
		))
		.await
		.expect("target rollback plan succeeds");

	assert_eq!(
		applied_names(&database_url).await,
		vec!["0001_first", "0002_second", "0003_third"]
	);
}

#[tokio::test]
async fn missing_target_is_rejected_before_any_recorder_mutation() {
	let (_tempdir, migrations_dir, database_url) = arrange_chain();

	let error = MigrateCommand
		.execute(&migration_context(
			&migrations_dir,
			&database_url,
			"0099_missing",
			false,
			false,
		))
		.await
		.expect_err("unknown target is rejected");

	assert_eq!(
		error.to_string(),
		"Execution error: Migration myapp:0099_missing does not exist on disk"
	);
	assert!(applied_names(&database_url).await.is_empty());
}

#[tokio::test]
async fn forward_target_plan_keeps_recorder_empty_for_sqlite() {
	let (_tempdir, migrations_dir, database_url) = arrange_chain();

	MigrateCommand
		.execute(&migration_context(
			&migrations_dir,
			&database_url,
			"0003_third",
			true,
			false,
		))
		.await
		.expect("forward target plan succeeds");

	let connection = DatabaseConnection::connect_sqlite(&database_url)
		.await
		.expect("reconnect SQLite database");
	let recorder = DatabaseMigrationRecorder::new(connection);
	assert!(
		recorder.get_applied_migrations().await.is_err(),
		"planning must not create the recorder table"
	);
}

#[tokio::test]
async fn forward_target_fake_records_only_the_sqlite_dependency_closure() {
	let (_tempdir, migrations_dir, database_url) = arrange_chain();

	MigrateCommand
		.execute(&migration_context(
			&migrations_dir,
			&database_url,
			"0002_second",
			false,
			true,
		))
		.await
		.expect("fake forward target succeeds");

	assert_eq!(
		applied_names(&database_url).await,
		vec!["0001_first", "0002_second"]
	);
}

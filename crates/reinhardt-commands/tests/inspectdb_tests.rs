//! Command adapter tests for inspectdb.

#![cfg(all(feature = "migrations", feature = "sqlite"))]

use reinhardt_commands::{BaseCommand, CommandContext, InspectDbCommand, InspectDbWriter};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::io;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

#[derive(Default)]
struct CapturedOutput {
	stdout: Mutex<Vec<String>>,
	stderr: Mutex<Vec<String>>,
}

impl InspectDbWriter for CapturedOutput {
	fn write_stdout(&self, content: &str) -> io::Result<()> {
		self.stdout
			.lock()
			.expect("stdout capture lock")
			.push(content.to_string());
		Ok(())
	}

	fn write_stderr(&self, content: &str) -> io::Result<()> {
		self.stderr
			.lock()
			.expect("stderr capture lock")
			.push(content.to_string());
		Ok(())
	}
}

async fn sqlite_fixture() -> (TempDir, String) {
	let temp = tempfile::Builder::new()
		.prefix("reinhardt-inspectdb-")
		.tempdir_in("/tmp")
		.expect("temporary database directory");
	let database_path = temp.path().join("inspectdb.sqlite3");
	let database_url = format!("sqlite:///{}", database_path.display());
	let pool = SqlitePoolOptions::new()
		.max_connections(1)
		.connect_with(
			SqliteConnectOptions::new()
				.filename(&database_path)
				.create_if_missing(true),
		)
		.await
		.expect("connect fixture database");
	sqlx::query(
		"CREATE TABLE users (
			id INTEGER PRIMARY KEY AUTOINCREMENT,
			display_name TEXT NOT NULL
		)",
	)
	.execute(&pool)
	.await
	.expect("create fixture table");
	pool.close().await;
	(temp, database_url)
}

#[tokio::test]
async fn stdout_mode_writes_one_parseable_module_and_sends_progress_to_stderr() {
	let (temp, database_url) = sqlite_fixture().await;
	let output = Arc::new(CapturedOutput::default());
	let command = InspectDbCommand::with_writer(output.clone());
	let mut context = CommandContext::new(vec!["users".to_string()]);
	context.set_option("database".to_string(), "default".to_string());
	context.set_option("database-url".to_string(), database_url);

	command
		.execute(&context)
		.await
		.expect("inspectdb stdout mode succeeds");

	let stdout = output.stdout.lock().expect("stdout capture lock");
	let stderr = output.stderr.lock().expect("stderr capture lock");
	assert_eq!(stdout.len(), 1, "the complete module must use one write");
	syn::parse_file(&stdout[0]).expect("stdout is one parseable Rust module");
	assert!(stdout[0].contains("pub struct Users"));
	assert!(!stdout[0].contains("[INFO]"));
	assert!(!stdout[0].contains("[SUCCESS]"));
	assert!(!stderr.is_empty(), "progress should be reported");
	assert!(
		stderr
			.iter()
			.any(|line| line.contains("Inspecting database"))
	);
	drop(stderr);
	drop(stdout);
	temp.close().expect("remove temporary database directory");
}

#[tokio::test]
async fn directory_mode_keeps_stdout_clean_and_writes_generated_files() {
	let (temp, database_url) = sqlite_fixture().await;
	let output_directory = temp.path().join("models");
	let output = Arc::new(CapturedOutput::default());
	let command = InspectDbCommand::with_writer(output.clone());
	let mut context = CommandContext::new(vec!["users".to_string()]);
	context.set_option("database".to_string(), "default".to_string());
	context.set_option("database-url".to_string(), database_url);
	context.set_option(
		"output".to_string(),
		output_directory.to_string_lossy().into_owned(),
	);

	command
		.execute(&context)
		.await
		.expect("inspectdb directory mode succeeds");

	let stdout = output.stdout.lock().expect("stdout capture lock");
	let stderr = output.stderr.lock().expect("stderr capture lock");
	assert!(stdout.is_empty(), "directory mode must not write stdout");
	assert!(output_directory.join("users.rs").is_file());
	assert!(output_directory.join("mod.rs").is_file());
	assert!(stderr.iter().any(|line| line.contains("Generated 2 files")));
	drop(stderr);
	drop(stdout);
	temp.close().expect("remove temporary output directory");
}

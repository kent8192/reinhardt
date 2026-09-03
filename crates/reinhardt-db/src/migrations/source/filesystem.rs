//! Filesystem-based migration source
//!
//! Loads migrations from `.rs` files on disk and extracts metadata using AST parsing.

use super::{Migration, MigrationError, MigrationSource, Result};
use crate::migrations::ast_parser;
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use syn::File;

/// Migration source that loads from filesystem
///
/// This source scans directories for `.rs` migration files and parses them
/// using `syn` to extract metadata like dependencies, atomic flag, and replaces.
///
/// The filesystem path is authoritative for migration identity. For a path
/// `<root>/<app>/<name>.rs`, the loaded migration uses `<app>` and `<name>` even
/// if duplicate `app_label` or `name` fields appear in the source literal.
pub struct FilesystemSource {
	/// Root directory containing migration files
	root_dir: PathBuf,
}

impl FilesystemSource {
	/// Create a new FilesystemSource
	///
	/// # Arguments
	///
	/// * `root_dir` - Root directory to scan for migration files
	///
	/// # Workspace Safety
	///
	/// When used in a Cargo workspace, relative paths are resolved from the
	/// current working directory, which may differ depending on where `cargo`
	/// commands are invoked. Use `env!("CARGO_MANIFEST_DIR")` for reliable paths:
	///
	/// ```rust,no_run
	/// use reinhardt_db::migrations::FilesystemSource;
	///
	/// // Workspace-safe: path is always relative to the crate's Cargo.toml
	/// let source = FilesystemSource::new(
	///     format!("{}/migrations", env!("CARGO_MANIFEST_DIR"))
	/// );
	/// ```
	pub fn new<P: AsRef<Path>>(root_dir: P) -> Self {
		Self {
			root_dir: root_dir.as_ref().to_path_buf(),
		}
	}

	/// Parse a migration file and extract metadata
	///
	/// This function reads the file, parses it with `syn`, and extracts:
	/// - dependencies from `dependencies()` function
	/// - atomic flag from `atomic()` function
	/// - replaces from `replaces()` function
	fn parse_migration_file(&self, path: &Path) -> Result<Migration> {
		// Read file contents
		let content = std::fs::read_to_string(path).map_err(|e| {
			MigrationError::IoError(std::io::Error::other(format!(
				"Failed to read {}: {}",
				path.display(),
				e
			)))
		})?;
		super::super::source_format::validate_source_version(&content).map_err(
			|error| match error {
				MigrationError::InvalidMigration(message) => MigrationError::InvalidMigration(
					format!("Failed to parse {}: {message}", path.display()),
				),
				other => other,
			},
		)?;

		// Parse with syn
		let ast: File = syn::parse_file(&content).map_err(|e| {
			MigrationError::InvalidMigration(format!("Failed to parse {}: {}", path.display(), e))
		})?;

		// Extract app_label and name from path
		// Expected format: <root_dir>/<app_label>/migrations/<name>.rs
		let (app_label, name) = self.extract_app_and_name(path)?;

		// Extract metadata from AST using ast_parser utility
		ast_parser::extract_migration_metadata_strict(&ast, &app_label, &name).map_err(|error| {
			MigrationError::InvalidMigration(format!(
				"Failed to load {} as {}.{}: {}",
				path.display(),
				app_label,
				name,
				error
			))
		})
	}

	/// Extract app_label and migration name from file path
	///
	/// Supports two formats:
	/// 1. `<root_dir>/<app_label>/<name>.rs` (Django-style, preferred)
	/// 2. `<root_dir>/<app_label>/migrations/<name>.rs` (legacy)
	///
	/// The app_label is the directory immediately under root_dir.
	fn extract_app_and_name(&self, path: &Path) -> Result<(String, String)> {
		// Get the path relative to root_dir
		let relative_path = path.strip_prefix(&self.root_dir).map_err(|_| {
			MigrationError::InvalidMigration(format!(
				"Path {} is not under root_dir {}",
				path.display(),
				self.root_dir.display()
			))
		})?;

		// Collect path components
		let components: Vec<_> = relative_path
			.components()
			.filter_map(|c| match c {
				std::path::Component::Normal(s) => s.to_str(),
				_ => None,
			})
			.collect();

		// Need at least 2 components: <app_label>/<name>.rs
		if components.len() < 2 {
			return Err(MigrationError::InvalidMigration(format!(
				"Path {} does not have enough components (expected <app_label>/<name>.rs)",
				path.display()
			)));
		}

		// The app_label is always the first component under root_dir
		let app_label = components[0].to_string();

		// Extract migration name from file name (without extension)
		let file_name = path
			.file_stem()
			.and_then(|s| s.to_str())
			.ok_or_else(|| MigrationError::InvalidMigration("Invalid file name".to_string()))?;

		Ok((app_label, file_name.to_string()))
	}
}

#[async_trait]
impl MigrationSource for FilesystemSource {
	async fn all_migrations(&self) -> Result<Vec<Migration>> {
		let mut migrations = Vec::new();

		// Warn when root directory does not exist (common with relative paths in workspaces)
		if !self.root_dir.exists() {
			tracing::warn!(
				path = %self.root_dir.display(),
				"Migration root directory does not exist. No migrations will be loaded. \
				 Hint: Use `env!(\"CARGO_MANIFEST_DIR\")` for workspace-safe absolute paths."
			);
			return Ok(Vec::new());
		}

		// Walk directory tree to find all .rs files
		let mut entries = Vec::new();
		let mut traversal_errors = Vec::new();
		for result in walkdir::WalkDir::new(&self.root_dir)
			.follow_links(true)
			.into_iter()
		{
			match result {
				Ok(entry) => entries.push(entry),
				Err(error) => {
					let key = error
						.path()
						.map(|path| path.to_string_lossy().into_owned())
						.unwrap_or_default();
					traversal_errors.push((key, error.to_string()));
				}
			}
		}
		traversal_errors.sort();
		if let Some((path, error)) = traversal_errors.first() {
			return Err(MigrationError::IoError(std::io::Error::other(format!(
				"Failed to traverse migration path {}: {}",
				path, error
			))));
		}
		entries.sort_by(|left, right| left.path().cmp(right.path()));

		for entry in entries {
			let path = entry.path();

			// Warn when .sql files are found (Reinhardt uses .rs migration files)
			// Note: This check is case-sensitive and only matches lowercase ".sql"
			// Uppercase variants (".SQL", ".Sql") are not detected as SQL migration files.
			// This is intentional - migration files should follow standard naming conventions.
			if path.extension().and_then(|s| s.to_str()) == Some("sql") {
				tracing::warn!(
					path = %path.display(),
					"Found SQL migration file but Reinhardt uses Rust (.rs) migration files. \
					 This file will be ignored. Run `cargo run --bin manage makemigrations` \
					 to generate Rust migration files from your model definitions."
				);
				continue;
			}

			// Skip if not a .rs file
			if path.extension().and_then(|s| s.to_str()) != Some("rs") {
				continue;
			}
			// Migration source files begin with their numeric migration sequence.
			// Rust 2024 module files such as `migrations.rs` share the extension
			// but have no migration entrypoint and must not be parsed as migrations.
			if !path
				.file_stem()
				.and_then(|stem| stem.to_str())
				.is_some_and(|stem| stem.starts_with(|character: char| character.is_ascii_digit()))
			{
				continue;
			}

			// Skip files directly in root_dir (need at least one subdirectory for app_label)
			let relative_path = match path.strip_prefix(&self.root_dir) {
				Ok(p) => p,
				Err(_) => continue,
			};

			// Need at least 2 components: <app_label>/<name>.rs
			let component_count = relative_path.components().count();
			if component_count < 2 {
				continue;
			}

			// Parse migration file
			migrations.push(self.parse_migration_file(path)?);
		}

		// Sort by app and numeric prefix for deterministic ordering (#1335)
		migrations.sort_by(|a, b| {
			let num_a = a
				.name
				.chars()
				.take_while(|c| c.is_ascii_digit())
				.collect::<String>()
				.parse::<u32>()
				.unwrap_or(0);
			let num_b = b
				.name
				.chars()
				.take_while(|c| c.is_ascii_digit())
				.collect::<String>()
				.parse::<u32>()
				.unwrap_or(0);
			a.app_label
				.cmp(&b.app_label)
				.then_with(|| num_a.cmp(&num_b))
				.then_with(|| a.name.cmp(&b.name))
		});

		Ok(migrations)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use serial_test::serial;
	use std::fs;
	use tempfile::TempDir;

	/// Helper to create a test migration file
	///
	/// Creates file at: `<dir>/<app>/<name>.rs` (Django-style)
	fn create_migration_file(dir: &Path, app: &str, name: &str, content: &str) {
		let app_dir = dir.join(app);
		fs::create_dir_all(&app_dir).unwrap();
		let file_path = app_dir.join(format!("{}.rs", name));
		fs::write(file_path, content).unwrap();
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_source)]
	async fn test_filesystem_source_new() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();

		// Act
		let source = FilesystemSource::new(temp_dir.path());

		// Assert
		assert_eq!(source.root_dir, temp_dir.path());
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_source)]
	async fn rejects_migration_sources_from_newer_format_versions() {
		let temp_dir = TempDir::new().unwrap();
		create_migration_file(
			temp_dir.path(),
			"app",
			"0001_initial",
			"// reinhardt-migration-source: 2\nfn migration() -> Migration { Migration::new(\"0001\", \"app\") }\n",
		);

		let error = FilesystemSource::new(temp_dir.path())
			.all_migrations()
			.await
			.expect_err("future source format must be rejected");

		assert!(error.to_string().contains("newer Reinhardt tool"));
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_source)]
	async fn test_filesystem_source_all_migrations() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		create_migration_file(
			temp_dir.path(),
			"polls",
			"0001_initial",
			r#"
use reinhardt_db::migrations::prelude::*;

pub fn migration() -> Migration {
	Migration {
		app_label: "polls",
		name: "0001_initial",
		operations: vec![],
		dependencies: vec![],
		atomic: true,
		replaces: vec![],
	}
}
"#,
		);
		create_migration_file(
			temp_dir.path(),
			"users",
			"0001_initial",
			r#"
use reinhardt_db::migrations::prelude::*;

pub fn migration() -> Migration {
	Migration {
		app_label: "users",
		name: "0001_initial",
		operations: vec![],
		dependencies: vec![],
		atomic: true,
		replaces: vec![],
	}
}
"#,
		);
		let source = FilesystemSource::new(temp_dir.path());

		// Act
		let migrations = source.all_migrations().await.unwrap();

		// Assert
		assert_eq!(migrations.len(), 2);
		assert!(migrations.iter().any(|m| m.app_label == "polls"));
		assert!(migrations.iter().any(|m| m.app_label == "users"));
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_source)]
	async fn filesystem_source_skips_rust_migration_module_entry_points() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		create_migration_file(
			temp_dir.path(),
			"polls",
			"0001_initial",
			r#"
use reinhardt_db::migrations::prelude::*;

pub fn migration() -> Migration {
	Migration::new("0001_initial", "polls")
}
"#,
		);
		fs::write(
			temp_dir.path().join("polls/migrations.rs"),
			"pub mod _0001_initial;",
		)
		.expect("write a Rust module entry point");

		// Act
		let migrations = FilesystemSource::new(temp_dir.path())
			.all_migrations()
			.await
			.expect("load migration files");

		// Assert
		assert_eq!(migrations.len(), 1);
		assert_eq!(migrations[0].name, "0001_initial");
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_source)]
	async fn test_filesystem_source_migrations_for_app() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		create_migration_file(
			temp_dir.path(),
			"polls",
			"0001_initial",
			r#"
use reinhardt_db::migrations::prelude::*;

pub fn migration() -> Migration {
	Migration {
		app_label: "polls",
		name: "0001_initial",
		operations: vec![],
		dependencies: vec![],
		atomic: true,
		replaces: vec![],
	}
}
"#,
		);
		create_migration_file(
			temp_dir.path(),
			"polls",
			"0002_add_field",
			r#"
use reinhardt_db::migrations::prelude::*;

pub fn migration() -> Migration {
	Migration {
		app_label: "polls",
		name: "0002_add_field",
		operations: vec![],
		dependencies: vec![("polls", "0001_initial")],
		atomic: true,
		replaces: vec![],
	}
}
"#,
		);
		let source = FilesystemSource::new(temp_dir.path());

		// Act
		let polls_migrations = source.migrations_for_app("polls").await.unwrap();

		// Assert
		assert_eq!(polls_migrations.len(), 2);
		assert!(polls_migrations.iter().all(|m| m.app_label == "polls"));
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_source)]
	async fn test_filesystem_source_get_migration() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		create_migration_file(
			temp_dir.path(),
			"polls",
			"0001_initial",
			r#"
use reinhardt_db::migrations::prelude::*;

pub fn migration() -> Migration {
	Migration {
		app_label: "polls",
		name: "0001_initial",
		operations: vec![],
		dependencies: vec![],
		atomic: true,
		replaces: vec![],
	}
}
"#,
		);
		let source = FilesystemSource::new(temp_dir.path());

		// Act
		let migration = source.get_migration("polls", "0001_initial").await.unwrap();

		// Assert
		assert_eq!(migration.app_label, "polls");
		assert_eq!(migration.name, "0001_initial");
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_source)]
	async fn test_filesystem_source_get_migration_not_found() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let source = FilesystemSource::new(temp_dir.path());

		// Act
		let result = source.get_migration("polls", "0001_initial").await;

		// Assert
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), MigrationError::NotFound(_)));
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_source)]
	async fn test_sql_files_are_ignored_in_migration_scan() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let app_dir = temp_dir.path().join("polls");
		fs::create_dir_all(&app_dir).unwrap();

		// Create .sql files that should be ignored
		fs::write(
			app_dir.join("0001_initial.sql"),
			"CREATE TABLE polls (id SERIAL PRIMARY KEY);",
		)
		.unwrap();
		fs::write(
			app_dir.join("0002_add_field.sql"),
			"ALTER TABLE polls ADD COLUMN name TEXT;",
		)
		.unwrap();

		let source = FilesystemSource::new(temp_dir.path());

		// Act
		let migrations = source.all_migrations().await.unwrap();

		// Assert
		assert_eq!(
			migrations.len(),
			0,
			"SQL files should not be loaded as migrations"
		);
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_source)]
	async fn test_sql_files_ignored_while_rs_files_loaded() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let app_dir = temp_dir.path().join("polls");
		fs::create_dir_all(&app_dir).unwrap();

		// Create a .sql file (should be ignored)
		fs::write(
			app_dir.join("0001_initial.sql"),
			"CREATE TABLE polls (id SERIAL PRIMARY KEY);",
		)
		.unwrap();

		// Create a valid .rs migration file (should be loaded)
		create_migration_file(
			temp_dir.path(),
			"polls",
			"0001_initial",
			r#"
use reinhardt_db::migrations::prelude::*;

pub fn migration() -> Migration {
	Migration {
		app_label: "polls",
		name: "0001_initial",
		operations: vec![],
		dependencies: vec![],
		atomic: true,
		replaces: vec![],
	}
}
"#,
		);

		let source = FilesystemSource::new(temp_dir.path());

		// Act
		let migrations = source.all_migrations().await.unwrap();

		// Assert
		assert_eq!(
			migrations.len(),
			1,
			"Only .rs files should be loaded as migrations"
		);
		assert_eq!(migrations[0].app_label, "polls");
		assert_eq!(migrations[0].name, "0001_initial");
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_source)]
	async fn skips_rust_module_files_beside_numbered_migrations() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		create_migration_file(
			temp_dir.path(),
			"polls",
			"0001_initial",
			r#"
pub fn migration() -> Migration {
	Migration { operations: vec![], dependencies: vec![], replaces: vec![] }
}
"#,
		);
		fs::write(
			temp_dir.path().join("polls").join("migrations.rs"),
			"pub mod migrations;",
		)
		.unwrap();

		// Act
		let migrations = FilesystemSource::new(temp_dir.path())
			.all_migrations()
			.await
			.expect("module files must be ignored");

		// Assert
		assert_eq!(migrations.len(), 1);
		assert_eq!(migrations[0].name, "0001_initial");
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_source)]
	async fn test_nonexistent_directory_returns_empty_with_warning() {
		use std::sync::{Arc, Mutex};
		use tracing_subscriber::layer::SubscriberExt as _;
		use tracing_subscriber::util::SubscriberInitExt as _;

		// Arrange
		struct LogCapture {
			logs: Arc<Mutex<Vec<String>>>,
		}

		impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for LogCapture {
			fn on_event(
				&self,
				event: &tracing::Event<'_>,
				_ctx: tracing_subscriber::layer::Context<'_, S>,
			) {
				struct MessageVisitor {
					message: String,
				}

				impl tracing::field::Visit for MessageVisitor {
					fn record_debug(
						&mut self,
						field: &tracing::field::Field,
						value: &dyn std::fmt::Debug,
					) {
						if field.name() == "message" {
							self.message = format!("{:?}", value);
						}
					}
				}

				let mut visitor = MessageVisitor {
					message: String::new(),
				};
				event.record(&mut visitor);

				let mut logs = self.logs.lock().unwrap();
				logs.push(format!(
					"[{}] {}",
					event.metadata().level(),
					visitor.message
				));
			}
		}

		let logs = Arc::new(Mutex::new(Vec::new()));
		let capture = LogCapture { logs: logs.clone() };
		let _guard = tracing_subscriber::registry().with(capture).set_default();

		let source = FilesystemSource::new("/nonexistent/path/that/does/not/exist");

		// Act
		let migrations = source.all_migrations().await.unwrap();

		// Assert
		assert!(
			migrations.is_empty(),
			"Non-existent directory should return empty migrations"
		);

		let captured = logs.lock().unwrap();
		let has_warning = captured
			.iter()
			.any(|log| log.contains("WARN") && log.contains("does not exist"));
		assert!(
			has_warning,
			"Expected warning log for non-existent directory, but got: {:?}",
			*captured
		);
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_source)]
	async fn test_sql_files_trigger_warning_log() {
		use std::sync::{Arc, Mutex};
		use tracing_subscriber::layer::SubscriberExt as _;
		use tracing_subscriber::util::SubscriberInitExt as _;

		// Arrange
		/// A tracing layer that captures log messages to a Vec<String>
		struct LogCapture {
			logs: Arc<Mutex<Vec<String>>>,
		}

		impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for LogCapture {
			fn on_event(
				&self,
				event: &tracing::Event<'_>,
				_ctx: tracing_subscriber::layer::Context<'_, S>,
			) {
				// Create a visitor to extract the message
				struct MessageVisitor {
					message: String,
				}

				impl tracing::field::Visit for MessageVisitor {
					fn record_debug(
						&mut self,
						field: &tracing::field::Field,
						value: &dyn std::fmt::Debug,
					) {
						if field.name() == "message" {
							self.message = format!("{:?}", value);
						}
					}
				}

				let mut visitor = MessageVisitor {
					message: String::new(),
				};
				event.record(&mut visitor);

				let mut logs = self.logs.lock().unwrap();
				logs.push(format!(
					"[{}] {}",
					event.metadata().level(),
					visitor.message
				));
			}
		}

		let logs = Arc::new(Mutex::new(Vec::new()));
		let capture = LogCapture { logs: logs.clone() };

		// Set up the subscriber with our capture layer
		let _guard = tracing_subscriber::registry().with(capture).set_default();

		let temp_dir = TempDir::new().unwrap();
		let app_dir = temp_dir.path().join("polls");
		fs::create_dir_all(&app_dir).unwrap();

		// Create a .sql file that should trigger a warning
		fs::write(
			app_dir.join("0001_initial.sql"),
			"CREATE TABLE polls (id SERIAL PRIMARY KEY);",
		)
		.unwrap();

		let source = FilesystemSource::new(temp_dir.path());

		// Act
		let _ = source.all_migrations().await.unwrap();

		// Assert
		let captured = logs.lock().unwrap();
		let has_warning = captured
			.iter()
			.any(|log| log.contains("WARN") && log.contains("SQL migration file"));
		assert!(
			has_warning,
			"Expected warning log for SQL file, but got: {:?}",
			*captured
		);
	}
}

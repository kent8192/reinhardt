//! Filesystem-based migration repository
//!
//! Persists migrations as `.rs` files on disk.

use super::{Migration, MigrationError, MigrationRepository, Result};
use crate::migrations::ast_parser;
use crate::migrations::dependency::DependencyCondition;
use async_trait::async_trait;
use quote::quote;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use syn::parse_quote;

/// Controls optional content in rendered migration source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MigrationRenderOptions {
	/// Include the generated-file header.
	pub include_header: bool,
}

/// Repository that persists migrations as `.rs` files
///
/// This repository writes migrations to disk in the format:
/// ```rust,no_run
/// // <app_label>/migrations/<name>.rs
/// use reinhardt_db::migrations::Migration;
/// // use reinhardt::db::migrations::prelude::*;
/// // use reinhardt::db::migrations::FieldType;
///
/// fn migration() -> Migration {
///     Migration::new("0001_initial", "app")
/// }
/// ```
pub struct FilesystemRepository {
	/// Root directory for migration files
	root_dir: PathBuf,
}

struct IncompleteFile {
	path: PathBuf,
	file: Option<File>,
	complete: bool,
}

impl IncompleteFile {
	fn new(path: PathBuf, file: File) -> Self {
		Self {
			path,
			file: Some(file),
			complete: false,
		}
	}

	fn file_mut(&mut self) -> &mut File {
		self.file.as_mut().expect("incomplete file must be open")
	}

	fn complete(mut self) -> PathBuf {
		self.complete = true;
		self.path.clone()
	}
}

impl Drop for IncompleteFile {
	fn drop(&mut self) {
		self.file.take();
		if !self.complete {
			let _ = std::fs::remove_file(&self.path);
		}
	}
}

impl FilesystemRepository {
	fn operation_kind(operation: &crate::migrations::Operation) -> &'static str {
		use crate::migrations::Operation;

		match operation {
			Operation::CreateTable { .. } => "CreateTable",
			Operation::DropTable { .. } => "DropTable",
			Operation::AddColumn { .. } => "AddColumn",
			Operation::DropColumn { .. } => "DropColumn",
			Operation::AlterColumn { .. } => "AlterColumn",
			Operation::RenameTable { .. } => "RenameTable",
			Operation::RenameColumn { .. } => "RenameColumn",
			Operation::AddConstraint { .. } => "AddConstraint",
			Operation::AddConstraintDefinition { .. } => "AddConstraintDefinition",
			Operation::AddConstraintRepair { .. } => "AddConstraintRepair",
			Operation::RestoreConstraintOnRollback { .. } => "RestoreConstraintOnRollback",
			Operation::DropConstraint { .. } => "DropConstraint",
			Operation::DropConstraintDefinition { .. } => "DropConstraintDefinition",
			Operation::CreateIndex { .. } => "CreateIndex",
			#[cfg(feature = "pgvector")]
			Operation::CreateNamedIndex { .. } => "CreateNamedIndex",
			Operation::CreateIndexRepair { .. } => "CreateIndexRepair",
			Operation::RestoreIndexOnRollback { .. } => "RestoreIndexOnRollback",
			Operation::DropIndex { .. } => "DropIndex",
			#[cfg(feature = "pgvector")]
			Operation::DropNamedIndex { .. } => "DropNamedIndex",
			Operation::RunSQL { .. } => "RunSQL",
			Operation::RunRust { .. } => "RunRust",
			Operation::AlterTableComment { .. } => "AlterTableComment",
			Operation::AlterUniqueTogether { .. } => "AlterUniqueTogether",
			Operation::AlterModelOptions { .. } => "AlterModelOptions",
			Operation::CreateInheritedTable { .. } => "CreateInheritedTable",
			Operation::AddDiscriminatorColumn { .. } => "AddDiscriminatorColumn",
			Operation::MoveModel { .. } => "MoveModel",
			Operation::CreateSchema { .. } => "CreateSchema",
			Operation::DropSchema { .. } => "DropSchema",
			Operation::CreateExtension { .. } => "CreateExtension",
			Operation::BulkLoad { .. } => "BulkLoad",
			Operation::SetAutoIncrementValue { .. } => "SetAutoIncrementValue",
			Operation::CreateCompositePrimaryKey { .. } => "CreateCompositePrimaryKey",
		}
	}

	/// Create a new FilesystemRepository
	///
	/// # Arguments
	///
	/// * `root_dir` - Root directory where migration files will be stored
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_db::migrations::FilesystemRepository;
	/// let repo = FilesystemRepository::new("./migrations");
	/// ```
	pub fn new<P: AsRef<Path>>(root_dir: P) -> Self {
		Self {
			root_dir: root_dir.as_ref().to_path_buf(),
		}
	}

	/// Validate that a path component does not contain traversal sequences.
	///
	/// Rejects components containing `..`, path separators, or null bytes
	/// to prevent directory traversal attacks that could escape the
	/// migration root directory.
	fn validate_path_component(component: &str, label: &str) -> Result<()> {
		if component.is_empty() {
			return Err(MigrationError::PathTraversal(format!(
				"{} cannot be empty",
				label
			)));
		}

		// Reject path traversal sequences
		if component.contains("..") {
			return Err(MigrationError::PathTraversal(format!(
				"{} contains path traversal sequence '..': {}",
				label, component
			)));
		}

		// Reject path separators (both Unix and Windows)
		if component.contains('/') || component.contains('\\') {
			return Err(MigrationError::PathTraversal(format!(
				"{} contains path separator: {}",
				label, component
			)));
		}

		// Reject null bytes
		if component.contains('\0') {
			return Err(MigrationError::PathTraversal(format!(
				"{} contains null byte: {}",
				label, component
			)));
		}

		Ok(())
	}

	/// Get the path for a migration file
	///
	/// Returns: `<root_dir>/<app_label>/<name>.rs`
	///
	/// Validates that `app_label` and `name` do not contain path traversal
	/// sequences before constructing the path.
	fn migration_path(&self, app_label: &str, name: &str) -> Result<PathBuf> {
		Self::validate_path_component(app_label, "App label")?;
		Self::validate_path_component(name, "Migration name")?;

		let path = self.root_dir.join(app_label).join(format!("{}.rs", name));

		// Final safety check: if both paths can be canonicalized (i.e., exist on disk),
		// verify the resolved path stays within root_dir.
		// When directories don't exist yet (e.g., during save), the component-level
		// validation above is sufficient to prevent traversal.
		if let (Ok(canonical_root), Some(parent)) = (self.root_dir.canonicalize(), path.parent())
			&& let Ok(canonical_parent) = parent.canonicalize()
			&& !canonical_parent.starts_with(&canonical_root)
		{
			return Err(MigrationError::PathTraversal(format!(
				"Resolved path escapes migration root directory: {}",
				path.display()
			)));
		}

		Ok(path)
	}

	/// Render a complete Rust migration source file.
	pub fn render(&self, migration: &Migration, options: MigrationRenderOptions) -> Result<String> {
		// Build dependencies vector (tuple elements need .to_string() for String type)
		let deps: Vec<_> = migration
			.dependencies
			.iter()
			.map(|(app, name)| {
				quote! { (#app.to_string(), #name.to_string()) }
			})
			.collect();

		// Build replaces vector (tuple elements need .to_string() for String type)
		let replaces: Vec<_> = migration
			.replaces
			.iter()
			.map(|(app, name)| {
				quote! { (#app.to_string(), #name.to_string()) }
			})
			.collect();
		let swappable_dependencies = migration.swappable_dependencies.iter().map(|dependency| {
			let setting_key = &dependency.setting_key;
			let default_app = &dependency.default_app;
			let default_model = &dependency.default_model;
			let migration_name = &dependency.migration_name;
			quote! {
				SwappableDependency::new(
					#setting_key,
					#default_app,
					#default_model,
					#migration_name,
				)
			}
		});
		let optional_dependencies = migration.optional_dependencies.iter().map(|dependency| {
			let app_label = &dependency.app_label;
			let migration_name = &dependency.migration_name;
			let condition = match &dependency.condition {
				DependencyCondition::AppInstalled(value) => {
					quote! { DependencyCondition::AppInstalled(#value.to_string()) }
				}
				DependencyCondition::SettingEnabled(value) => {
					quote! { DependencyCondition::SettingEnabled(#value.to_string()) }
				}
				DependencyCondition::FeatureEnabled(value) => {
					quote! { DependencyCondition::FeatureEnabled(#value.to_string()) }
				}
			};
			quote! {
				OptionalDependency::new(#app_label, #migration_name, #condition)
			}
		});

		let app_label = &migration.app_label;
		let name = &migration.name;
		let atomic = migration.atomic;
		let state_only = migration.state_only;
		let database_only = migration.database_only;

		// Build initial field token
		let initial_tokens = match migration.initial {
			Some(true) => quote! { Some(true) },
			Some(false) => quote! { Some(false) },
			None => quote! { None },
		};

		// Generate operation code
		let ops_tokens = migration.operations.iter();
		let operations_code = quote! { vec![#(#ops_tokens),*] };

		// Generate full migration file
		let file: syn::File = parse_quote! {
			use reinhardt::db::migrations::prelude::*;
			use reinhardt::db::migrations::dependency::{
				DependencyCondition, OptionalDependency, SwappableDependency,
			};
			use reinhardt::db::migrations::FieldType;

			pub(super) fn migration() -> Migration {
				Migration {
					app_label: #app_label.to_string(),
					name: #name.to_string(),
					operations: #operations_code,
					dependencies: vec![#(#deps),*],
					atomic: #atomic,
					replaces: vec![#(#replaces),*],
					initial: #initial_tokens,
					state_only: #state_only,
					database_only: #database_only,
					swappable_dependencies: vec![#(#swappable_dependencies),*],
					optional_dependencies: vec![#(#optional_dependencies),*],
				}
			}
		};

		// Format with prettyplease first, then apply rustfmt
		let prettyplease_output = prettyplease::unparse(&file);
		let formatted = Self::format_with_rustfmt(&prettyplease_output)?;
		syn::parse_file(&formatted).map_err(|error| {
			let operations = migration
				.operations
				.iter()
				.map(Self::operation_kind)
				.collect::<Vec<_>>()
				.join(", ");
			MigrationError::UnsupportedMigrationRendering {
				operation: format!("{operations}: {error}"),
			}
		})?;

		if options.include_header {
			Ok(format!(
				"// Generated by Reinhardt migrations.\n\n{formatted}"
			))
		} else {
			Ok(formatted)
		}
	}

	/// Create a new migration source file without overwriting an existing file.
	pub fn create_new_source(
		&self,
		app_label: &str,
		migration_name: &str,
		source: &str,
	) -> Result<PathBuf> {
		self.create_new_source_with(app_label, migration_name, source, |file, formatted| {
			file.write_all(formatted.as_bytes())?;
			file.flush()?;
			file.sync_all()
		})
	}

	fn create_new_source_with<F>(
		&self,
		app_label: &str,
		migration_name: &str,
		source: &str,
		write_source: F,
	) -> Result<PathBuf>
	where
		F: FnOnce(&mut File, &str) -> std::io::Result<()>,
	{
		let path = self.migration_path(app_label, migration_name)?;
		syn::parse_file(source).map_err(|error| {
			MigrationError::InvalidMigration(format!("Failed to parse migration source: {error}"))
		})?;
		let formatted = Self::format_with_rustfmt(source)?;

		if let Some(parent) = path.parent() {
			std::fs::create_dir_all(parent)?;
		}
		let file = OpenOptions::new()
			.write(true)
			.create_new(true)
			.open(&path)?;
		let mut incomplete = IncompleteFile::new(path, file);
		write_source(incomplete.file_mut(), &formatted)?;
		Ok(incomplete.complete())
	}

	/// Format code with rustfmt, applying project's rustfmt.toml settings (hard_tabs = true)
	///
	/// Falls back to prettyplease output if rustfmt is not available or fails.
	fn format_with_rustfmt(code: &str) -> Result<String> {
		use std::process::{Command, Stdio};

		// Try to run rustfmt
		let child = Command::new("rustfmt")
			.arg("--edition=2024")
			.stdin(Stdio::piped())
			.stdout(Stdio::piped())
			.stderr(Stdio::piped())
			.spawn();

		match child {
			Ok(mut child_process) => {
				// Write code to stdin
				if let Some(stdin) = child_process.stdin.as_mut() {
					stdin.write_all(code.as_bytes()).map_err(|e| {
						MigrationError::IoError(std::io::Error::other(format!(
							"Failed to write to rustfmt stdin: {}",
							e
						)))
					})?;
				}

				// Get formatted output
				let output = child_process.wait_with_output().map_err(|e| {
					MigrationError::IoError(std::io::Error::other(format!(
						"Failed to read rustfmt output: {}",
						e
					)))
				})?;

				if output.status.success() {
					String::from_utf8(output.stdout).map_err(|e| {
						MigrationError::IoError(std::io::Error::other(format!(
							"Invalid UTF-8 from rustfmt: {}",
							e
						)))
					})
				} else {
					// rustfmt failed, fallback to prettyplease output
					eprintln!("Warning: rustfmt failed, using prettyplease output");
					Ok(code.to_string())
				}
			}
			Err(_) => {
				// rustfmt not available, use prettyplease output
				eprintln!("Warning: rustfmt not found, using prettyplease output (space-indented)");
				Ok(code.to_string())
			}
		}
	}

	/// Check if two migrations have identical operations
	///
	/// Returns true if the operations vectors are equal.
	fn has_identical_operations(&self, m1: &Migration, m2: &Migration) -> bool {
		m1.operations == m2.operations
	}
}

#[async_trait]
impl MigrationRepository for FilesystemRepository {
	async fn save(&mut self, migration: &Migration) -> Result<()> {
		let path = self.migration_path(&migration.app_label, &migration.name)?;

		// Check if migration file already exists to prevent overwriting
		if tokio::fs::try_exists(&path).await.unwrap_or(false) {
			return Err(MigrationError::IoError(std::io::Error::other(format!(
				"Migration file already exists: {}. \
				If you want to replace it, please delete the existing file first.",
				path.display()
			))));
		}

		// Check for duplicate operations with existing migrations.
		// Skip for migrations with empty operations (e.g., merge migrations)
		// since empty-vs-empty comparison is never a meaningful duplicate signal.
		// The file-exists check above already prevents actual overwrites.
		if !migration.operations.is_empty() {
			let existing_migrations = self.list(&migration.app_label).await?;
			for existing in &existing_migrations {
				if self.has_identical_operations(existing, migration) {
					return Err(MigrationError::DuplicateOperations(format!(
						"Migration '{}' has identical operations to existing migration '{}'. \
						This usually indicates a problem with from_state construction. \
						The existing migration was created at the same location and performs \
						the same database changes.",
						migration.name, existing.name
					)));
				}
			}
		}

		// Create parent directories
		if let Some(parent) = path.parent() {
			tokio::fs::create_dir_all(parent).await.map_err(|e| {
				MigrationError::IoError(std::io::Error::other(format!(
					"Failed to create directory {}: {}",
					parent.display(),
					e
				)))
			})?;
		}

		let code = self.render(
			migration,
			MigrationRenderOptions {
				include_header: true,
			},
		)?;
		self.create_new_source(&migration.app_label, &migration.name, &code)?;

		Ok(())
	}

	async fn get(&self, app_label: &str, name: &str) -> Result<Migration> {
		let path = self.migration_path(app_label, name)?;

		if !path.exists() {
			return Err(MigrationError::NotFound(format!("{}.{}", app_label, name)));
		}

		// Read and parse file
		let content = tokio::fs::read_to_string(&path).await.map_err(|e| {
			MigrationError::IoError(std::io::Error::other(format!(
				"Failed to read {}: {}",
				path.display(),
				e
			)))
		})?;

		// Parse with syn
		let ast: syn::File = syn::parse_file(&content).map_err(|e| {
			MigrationError::InvalidMigration(format!("Failed to parse {}: {}", path.display(), e))
		})?;

		// Extract migration data from AST using ast_parser utility
		ast_parser::extract_migration_metadata(&ast, app_label, name)
	}

	async fn list(&self, app_label: &str) -> Result<Vec<Migration>> {
		Self::validate_path_component(app_label, "App label")?;
		let migrations_dir = self.root_dir.join(app_label);

		if !migrations_dir.exists() {
			return Ok(vec![]);
		}

		let mut migrations = Vec::new();

		// Read directory
		let mut entries = tokio::fs::read_dir(&migrations_dir).await.map_err(|e| {
			MigrationError::IoError(std::io::Error::other(format!(
				"Failed to read directory {}: {}",
				migrations_dir.display(),
				e
			)))
		})?;

		while let Some(entry) = entries.next_entry().await.map_err(|e| {
			MigrationError::IoError(std::io::Error::other(format!(
				"Failed to read directory entry: {}",
				e
			)))
		})? {
			let path = entry.path();

			// Skip non-.rs files
			if path.extension().and_then(|s| s.to_str()) != Some("rs") {
				continue;
			}

			// Extract name from filename
			if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
				// Get migration
				match self.get(app_label, name).await {
					Ok(migration) => migrations.push(migration),
					Err(e) => {
						eprintln!("Warning: Failed to load migration {}: {}", name, e);
					}
				}
			}
		}

		Ok(migrations)
	}

	async fn delete(&mut self, app_label: &str, name: &str) -> Result<()> {
		let path = self.migration_path(app_label, name)?;

		if !path.exists() {
			return Err(MigrationError::NotFound(format!("{}.{}", app_label, name)));
		}

		tokio::fs::remove_file(&path).await.map_err(|e| {
			MigrationError::IoError(std::io::Error::other(format!(
				"Failed to delete {}: {}",
				path.display(),
				e
			)))
		})?;

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::migrations::fields::FieldType;
	use crate::migrations::operations::{ColumnDefinition, Operation};
	use rstest::rstest;
	use serial_test::serial;
	use tempfile::TempDir;

	/// Creates a test migration with a unique CreateTable operation based on the migration name.
	/// This ensures each migration has distinct operations to avoid duplicate detection errors.
	fn create_test_migration(app_label: &str, name: &str) -> Migration {
		let mut migration = Migration::new(name, app_label);

		// Create a unique table name derived from the migration name
		let table_name = format!("table_{}", name.replace('-', "_"));
		migration.operations.push(Operation::CreateTable {
			name: table_name,
			columns: vec![ColumnDefinition::new("id", FieldType::Integer)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		});

		migration
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_repository)]
	async fn test_filesystem_repository_new() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();

		// Act
		let repo = FilesystemRepository::new(temp_dir.path());

		// Assert
		assert_eq!(repo.root_dir, temp_dir.path());
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_repository)]
	async fn test_filesystem_repository_save() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let mut repo = FilesystemRepository::new(temp_dir.path());
		let migration = create_test_migration("polls", "0001_initial");

		// Act
		repo.save(&migration).await.unwrap();

		// Assert
		let path = repo.migration_path("polls", "0001_initial").unwrap();
		assert!(tokio::fs::try_exists(&path).await.unwrap());

		let content = tokio::fs::read_to_string(&path).await.unwrap();
		assert!(content.contains("pub(super) fn migration() -> Migration"));
		assert!(content.contains("app_label: \"polls\""));
		assert!(content.contains("name: \"0001_initial\""));
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_repository)]
	async fn test_filesystem_repository_get() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let mut repo = FilesystemRepository::new(temp_dir.path());
		let migration = create_test_migration("polls", "0001_initial");
		repo.save(&migration).await.unwrap();

		// Act
		let retrieved = repo.get("polls", "0001_initial").await.unwrap();

		// Assert
		assert_eq!(retrieved.app_label, "polls");
		assert_eq!(retrieved.name, "0001_initial");
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_repository)]
	async fn test_filesystem_repository_get_not_found() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let repo = FilesystemRepository::new(temp_dir.path());

		// Act
		let result = repo.get("polls", "0001_initial").await;

		// Assert
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), MigrationError::NotFound(_)));
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_repository)]
	async fn test_filesystem_repository_list() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let mut repo = FilesystemRepository::new(temp_dir.path());
		repo.save(&create_test_migration("polls", "0001_initial"))
			.await
			.unwrap();
		repo.save(&create_test_migration("polls", "0002_add_field"))
			.await
			.unwrap();

		// Act
		let migrations = repo.list("polls").await.unwrap();

		// Assert
		assert_eq!(migrations.len(), 2);
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_repository)]
	async fn test_filesystem_repository_list_empty() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let repo = FilesystemRepository::new(temp_dir.path());

		// Act
		let migrations = repo.list("polls").await.unwrap();

		// Assert
		assert_eq!(migrations.len(), 0);
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_repository)]
	async fn test_filesystem_repository_delete() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let mut repo = FilesystemRepository::new(temp_dir.path());
		let migration = create_test_migration("polls", "0001_initial");
		repo.save(&migration).await.unwrap();
		let path = repo.migration_path("polls", "0001_initial").unwrap();
		assert!(tokio::fs::try_exists(&path).await.unwrap());

		// Act
		repo.delete("polls", "0001_initial").await.unwrap();

		// Assert
		assert!(!tokio::fs::try_exists(&path).await.unwrap());
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_repository)]
	async fn test_filesystem_repository_delete_not_found() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let mut repo = FilesystemRepository::new(temp_dir.path());

		// Act
		let result = repo.delete("polls", "0001_initial").await;

		// Assert
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), MigrationError::NotFound(_)));
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_repository)]
	async fn test_filesystem_repository_save_with_dependencies() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let mut repo = FilesystemRepository::new(temp_dir.path());
		let migration =
			Migration::new("0002_add_field", "polls").add_dependency("polls", "0001_initial");

		// Act
		repo.save(&migration).await.unwrap();

		// Assert
		let path = repo.migration_path("polls", "0002_add_field").unwrap();
		let content = tokio::fs::read_to_string(&path).await.unwrap();
		assert!(content.contains("dependencies"));
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_repository)]
	async fn test_filesystem_repository_save_prevents_overwrite() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let mut repo = FilesystemRepository::new(temp_dir.path());
		let migration = create_test_migration("polls", "0001_initial");
		repo.save(&migration).await.unwrap();
		let path = repo.migration_path("polls", "0001_initial").unwrap();
		assert!(tokio::fs::try_exists(&path).await.unwrap());

		// Act
		let duplicate_migration = create_test_migration("polls", "0001_initial");
		let result = repo.save(&duplicate_migration).await;

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(matches!(err, MigrationError::IoError(_)));
		assert!(err.to_string().contains("already exists"));
	}

	#[rstest]
	#[case("../etc", "0001_initial", "App label")]
	#[case("polls", "../secret", "Migration name")]
	#[case("../../root", "0001_initial", "App label")]
	#[case("polls", "../../etc/passwd", "Migration name")]
	fn test_path_traversal_rejected(
		#[case] app_label: &str,
		#[case] name: &str,
		#[case] expected_label: &str,
	) {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let repo = FilesystemRepository::new(temp_dir.path());

		// Act
		let result = repo.migration_path(app_label, name);

		// Assert
		assert!(result.is_err(), "Path traversal should be rejected");
		let err = result.unwrap_err();
		assert!(matches!(err, MigrationError::PathTraversal(_)));
		assert!(
			err.to_string().contains(expected_label),
			"Error should mention '{}', got: {}",
			expected_label,
			err
		);
	}

	#[rstest]
	#[case("polls/subdir", "0001_initial")]
	#[case("polls\\subdir", "0001_initial")]
	#[case("polls", "name/with/slashes")]
	fn test_path_separator_rejected(#[case] app_label: &str, #[case] name: &str) {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let repo = FilesystemRepository::new(temp_dir.path());

		// Act
		let result = repo.migration_path(app_label, name);

		// Assert
		assert!(result.is_err(), "Path separators should be rejected");
		assert!(matches!(
			result.unwrap_err(),
			MigrationError::PathTraversal(_)
		));
	}

	#[rstest]
	#[case("polls\0evil", "0001_initial")]
	#[case("polls", "0001\0evil")]
	fn test_null_byte_rejected(#[case] app_label: &str, #[case] name: &str) {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let repo = FilesystemRepository::new(temp_dir.path());

		// Act
		let result = repo.migration_path(app_label, name);

		// Assert
		assert!(result.is_err(), "Null bytes should be rejected");
		assert!(matches!(
			result.unwrap_err(),
			MigrationError::PathTraversal(_)
		));
	}

	#[rstest]
	#[case("", "0001_initial")]
	#[case("polls", "")]
	fn test_empty_component_rejected(#[case] app_label: &str, #[case] name: &str) {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let repo = FilesystemRepository::new(temp_dir.path());

		// Act
		let result = repo.migration_path(app_label, name);

		// Assert
		assert!(result.is_err(), "Empty components should be rejected");
		assert!(matches!(
			result.unwrap_err(),
			MigrationError::PathTraversal(_)
		));
	}

	#[rstest]
	fn test_valid_path_accepted() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let repo = FilesystemRepository::new(temp_dir.path());

		// Act
		let result = repo.migration_path("polls", "0001_initial");

		// Assert
		assert!(result.is_ok(), "Valid path should be accepted");
		let path = result.unwrap();
		assert!(path.starts_with(temp_dir.path()));
		assert!(path.ends_with("0001_initial.rs"));
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_repository)]
	async fn test_save_rejects_traversal_in_app_label() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let mut repo = FilesystemRepository::new(temp_dir.path());
		let migration = create_test_migration("../etc", "0001_initial");

		// Act
		let result = repo.save(&migration).await;

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			MigrationError::PathTraversal(_)
		));
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_repository)]
	async fn test_list_rejects_traversal_in_app_label() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let repo = FilesystemRepository::new(temp_dir.path());

		// Act
		let result = repo.list("../etc").await;

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			MigrationError::PathTraversal(_)
		));
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_repository)]
	async fn test_filesystem_repository_save_with_initial_true() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let mut repo = FilesystemRepository::new(temp_dir.path());
		let mut migration = create_test_migration("polls", "0001_initial_true");
		migration.initial = Some(true);

		// Act
		repo.save(&migration).await.unwrap();

		// Assert - verify generated code contains initial: Some(true)
		let path = repo.migration_path("polls", "0001_initial_true").unwrap();
		let content = tokio::fs::read_to_string(&path).await.unwrap();
		assert!(
			content.contains("Some(true)"),
			"Generated code should contain Some(true), got: {}",
			content
		);

		// Assert - round-trip: get() parses back the same initial value
		let retrieved = repo.get("polls", "0001_initial_true").await.unwrap();
		assert_eq!(retrieved.initial, Some(true));
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_repository)]
	async fn test_filesystem_repository_save_with_initial_false() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let mut repo = FilesystemRepository::new(temp_dir.path());
		let mut migration = create_test_migration("polls", "0001_initial_false");
		migration.initial = Some(false);

		// Act
		repo.save(&migration).await.unwrap();

		// Assert - verify generated code contains initial: Some(false)
		let path = repo.migration_path("polls", "0001_initial_false").unwrap();
		let content = tokio::fs::read_to_string(&path).await.unwrap();
		assert!(
			content.contains("Some(false)"),
			"Generated code should contain Some(false), got: {}",
			content
		);

		// Assert - round-trip: get() parses back the same initial value
		let retrieved = repo.get("polls", "0001_initial_false").await.unwrap();
		assert_eq!(retrieved.initial, Some(false));
	}

	#[test]
	fn create_new_source_removes_partial_file_when_injected_write_fails() {
		use std::io::Write;

		let temp_dir = TempDir::new().unwrap();
		let repo = FilesystemRepository::new(temp_dir.path());
		let source = repo
			.render(
				&create_test_migration("polls", "0001_squashed"),
				MigrationRenderOptions {
					include_header: false,
				},
			)
			.unwrap();

		let result =
			repo.create_new_source_with("polls", "0001_squashed", &source, |file, formatted| {
				file.write_all(&formatted.as_bytes()[..16])?;
				Err(std::io::Error::other("injected write failure"))
			});

		assert!(matches!(result, Err(MigrationError::IoError(_))));
		assert!(!temp_dir.path().join("polls/0001_squashed.rs").exists());
	}
}

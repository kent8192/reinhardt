//! Migration squashing command orchestration.

use crate::{CommandError, CommandResult};
use reinhardt_db::migrations::{
	DependencyResolutionContext, FilesystemRepository, FilesystemSource, MigrationCatalog,
	MigrationError, MigrationRenderOptions, MigrationSquasher,
};
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

/// Parsed options for the `squashmigrations` command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SquashMigrationsOptions {
	/// Application whose migrations will be squashed.
	pub app_label: String,
	/// Optional first migration in the squash range.
	pub start_migration: Option<String>,
	/// Last migration in the squash range.
	pub migration_name: String,
	/// Preserve the exact source operation order.
	pub no_optimize: bool,
	/// Write without interactive confirmation.
	pub no_input: bool,
	/// Omit the generated-file header.
	pub no_header: bool,
	/// Optional explicit destination migration name.
	pub squashed_name: Option<String>,
}

/// Exact result fields reported after a squashed migration is created.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SquashMigrationsSummary {
	/// Application whose migrations were squashed.
	pub app_label: String,
	/// Fully resolved first migration name.
	pub start_migration: String,
	/// Fully resolved last migration name.
	pub end_migration: String,
	/// Name assigned to the new squashed migration.
	pub squashed_name: String,
	/// Number of source migrations replaced.
	pub migration_count: usize,
	/// Operation count before optimization.
	pub original_operation_count: usize,
	/// Operation count after optimization.
	pub optimized_operation_count: usize,
	/// Path of the newly created migration source.
	pub path: PathBuf,
}

/// Terminal input boundary used by interactive confirmation.
pub trait ConfirmationReader {
	/// Return whether this reader is connected to a terminal.
	fn is_terminal(&self) -> bool;

	/// Read one line of confirmation input.
	fn read_line(&mut self, buffer: &mut String) -> io::Result<usize>;
}

/// Confirmation reader backed by process standard input.
#[derive(Debug, Default)]
pub struct StdinConfirmationReader;

impl ConfirmationReader for StdinConfirmationReader {
	fn is_terminal(&self) -> bool {
		io::stdin().is_terminal()
	}

	fn read_line(&mut self, buffer: &mut String) -> io::Result<usize> {
		io::stdin().lock().read_line(buffer)
	}
}

pub(crate) fn migration_error_to_command_error(error: MigrationError) -> CommandError {
	match error {
		MigrationError::NotFound(_)
		| MigrationError::DependencyError(_)
		| MigrationError::InvalidMigration(_)
		| MigrationError::CircularDependency { .. }
		| MigrationError::NodeNotFound { .. }
		| MigrationError::DuplicateOperations(_)
		| MigrationError::PathTraversal(_) => CommandError::InvalidArguments(error.to_string()),
		MigrationError::IoError(error) => CommandError::IoError(error),
		MigrationError::SqlError(_)
		| MigrationError::DatabaseError(_)
		| MigrationError::FrameworkError(_)
		| MigrationError::IrreversibleError(_)
		| MigrationError::FmtError(_)
		| MigrationError::IntrospectionError(_)
		| MigrationError::UnsupportedDatabase(_)
		| MigrationError::UnsupportedBackendFeature { .. }
		| MigrationError::ForeignKeyViolation(_)
		| MigrationError::UnsupportedMigrationRendering { .. } => {
			CommandError::ExecutionError(error.to_string())
		}
		_ => CommandError::ExecutionError(format!("Unclassified migration error: {error}")),
	}
}

fn numbered_prefix(name: &str) -> &str {
	name.split_once('_').map_or(name, |(number, _)| number)
}

fn default_squashed_name(start: &str, end: &str) -> String {
	format!(
		"{}_squashed_{}",
		numbered_prefix(start),
		numbered_prefix(end)
	)
}

fn is_safe_migration_suffix(suffix: &str) -> bool {
	suffix
		.as_bytes()
		.first()
		.is_some_and(u8::is_ascii_alphabetic)
		&& suffix
			.bytes()
			.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn confirmed(answer: &str) -> bool {
	matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

fn write_summary(output: &mut dyn Write, summary: &SquashMigrationsSummary) -> io::Result<()> {
	writeln!(output, "App: {}", summary.app_label)?;
	writeln!(
		output,
		"Range: {} -> {}",
		summary.start_migration, summary.end_migration
	)?;
	writeln!(output, "Migrations: {}", summary.migration_count)?;
	writeln!(
		output,
		"Operations: {} -> {}",
		summary.original_operation_count, summary.optimized_operation_count
	)?;
	writeln!(output, "Created: {}", summary.path.display())
}

/// Resolve, render, confirm, and create a squashed migration source.
///
/// Resolution, optimization, and rendering all complete before any prompt is
/// emitted. The destination is created only after confirmation succeeds.
pub async fn execute_squashmigrations_with_io(
	migrations_root: &Path,
	options: SquashMigrationsOptions,
	confirmation: &mut dyn ConfirmationReader,
	stdout: &mut dyn Write,
	stderr: &mut dyn Write,
) -> CommandResult<Option<SquashMigrationsSummary>> {
	execute_squashmigrations_with_io_and_context(
		migrations_root,
		options,
		&DependencyResolutionContext::new(),
		confirmation,
		stdout,
		stderr,
	)
	.await
}

/// Resolve, render, confirm, and create a squashed migration with settings-aware dependencies.
pub async fn execute_squashmigrations_with_io_and_context(
	migrations_root: &Path,
	options: SquashMigrationsOptions,
	dependency_context: &DependencyResolutionContext,
	confirmation: &mut dyn ConfirmationReader,
	stdout: &mut dyn Write,
	stderr: &mut dyn Write,
) -> CommandResult<Option<SquashMigrationsSummary>> {
	if let Some(name) = options.squashed_name.as_deref()
		&& !is_safe_migration_suffix(name)
	{
		return Err(CommandError::InvalidArguments(
			"--squashed-name must be a safe ASCII Rust filename suffix".to_string(),
		));
	}

	let source = FilesystemSource::new(migrations_root);
	let catalog = MigrationCatalog::load_strict_with_context(&source, dependency_context)
		.await
		.map_err(migration_error_to_command_error)?;
	let range = catalog
		.squash_range(
			&options.app_label,
			options.start_migration.as_deref(),
			&options.migration_name,
		)
		.map_err(migration_error_to_command_error)?;
	let start_migration = range
		.migrations
		.first()
		.expect("a resolved squash range must not be empty")
		.name
		.clone();
	let end_migration = range
		.migrations
		.last()
		.expect("a resolved squash range must not be empty")
		.name
		.clone();
	let migration_count = range.migrations.len();
	let squashed_name = options
		.squashed_name
		.map(|suffix| format!("{}_{}", numbered_prefix(&start_migration), suffix))
		.unwrap_or_else(|| default_squashed_name(&start_migration, &end_migration));
	let result = MigrationSquasher::new()
		.squash_range_with_context(
			&range,
			&squashed_name,
			!options.no_optimize,
			dependency_context,
		)
		.map_err(migration_error_to_command_error)?;
	let repository = FilesystemRepository::new(migrations_root);
	let rendered = repository
		.render(
			&result.migration,
			MigrationRenderOptions {
				include_header: !options.no_header,
			},
		)
		.map_err(migration_error_to_command_error)?;

	if !options.no_input {
		if !confirmation.is_terminal() {
			return Err(CommandError::InvalidArguments(
				"squashmigrations requires terminal input; use --no-input in non-interactive \
				 environments"
					.to_string(),
			));
		}
		write!(
			stderr,
			"Squash {migration_count} migrations ({} operations -> {}) into \
			 {squashed_name}? [y/N] ",
			result.original_operation_count, result.optimized_operation_count
		)?;
		stderr.flush()?;
		let mut answer = String::new();
		confirmation.read_line(&mut answer)?;
		if !confirmed(&answer) {
			writeln!(stderr, "Cancelled.")?;
			return Ok(None);
		}
	}

	let path = repository
		.create_new_source(&options.app_label, &squashed_name, &rendered)
		.map_err(migration_error_to_command_error)?;
	let summary = SquashMigrationsSummary {
		app_label: options.app_label,
		start_migration,
		end_migration,
		squashed_name,
		migration_count,
		original_operation_count: result.original_operation_count,
		optimized_operation_count: result.optimized_operation_count,
		path,
	};
	write_summary(stdout, &summary)?;
	Ok(Some(summary))
}

#[cfg(test)]
mod tests {
	use super::*;
	use reinhardt_db::migrations::MigrationError;

	#[derive(Debug, Clone, Copy)]
	enum ExpectedCategory {
		InvalidArguments,
		IoError,
		ExecutionError,
	}

	#[test]
	fn migration_errors_map_to_their_command_error_categories() {
		// Arrange
		let cases = [
			(
				MigrationError::NotFound("polls.0002".to_string()),
				ExpectedCategory::InvalidArguments,
			),
			(
				MigrationError::IoError(io::Error::new(
					io::ErrorKind::PermissionDenied,
					"migration tree denied",
				)),
				ExpectedCategory::IoError,
			),
			(
				MigrationError::UnsupportedMigrationRendering {
					operation: "RunRust".to_string(),
				},
				ExpectedCategory::ExecutionError,
			),
		];

		for (error, expected) in cases {
			// Act
			let mapped = migration_error_to_command_error(error);

			// Assert
			match (mapped, expected) {
				(CommandError::InvalidArguments(message), ExpectedCategory::InvalidArguments) => {
					assert_eq!(message, "Migration not found: polls.0002")
				}
				(CommandError::IoError(error), ExpectedCategory::IoError) => {
					assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
					assert_eq!(error.to_string(), "migration tree denied");
				}
				(CommandError::ExecutionError(message), ExpectedCategory::ExecutionError) => {
					assert_eq!(message, "Unsupported migration rendering: RunRust");
				}
				(actual, expected) => {
					panic!("unexpected migration error mapping: {actual:?} for {expected:?}")
				}
			}
		}
	}
}

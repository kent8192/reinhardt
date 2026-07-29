//! Migration squashing command orchestration.

use crate::{CommandError, CommandResult};
use reinhardt_db::migrations::{
	FilesystemRepository, FilesystemSource, MigrationCatalog, MigrationRenderOptions,
	MigrationSquasher,
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

fn command_error(error: impl std::fmt::Display) -> CommandError {
	CommandError::ExecutionError(error.to_string())
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

fn is_safe_migration_name(name: &str) -> bool {
	if name.contains("..") || name.contains('/') || name.contains('\\') || name.contains('\0') {
		return false;
	}
	let Some((number, description)) = name.split_once('_') else {
		return false;
	};
	number.len() >= 4
		&& number.bytes().all(|byte| byte.is_ascii_digit())
		&& !description.is_empty()
		&& description
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
	if let Some(name) = options.squashed_name.as_deref()
		&& !is_safe_migration_name(name)
	{
		return Err(CommandError::InvalidArguments(
			"--squashed-name must be a numbered safe ASCII Rust filename component".to_string(),
		));
	}

	let source = FilesystemSource::new(migrations_root);
	let catalog = MigrationCatalog::load_strict(&source)
		.await
		.map_err(command_error)?;
	let range = catalog
		.squash_range(
			&options.app_label,
			options.start_migration.as_deref(),
			&options.migration_name,
		)
		.map_err(command_error)?;
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
		.unwrap_or_else(|| default_squashed_name(&start_migration, &end_migration));
	let result = MigrationSquasher::new()
		.squash_range(&range, &squashed_name, !options.no_optimize)
		.map_err(command_error)?;
	let repository = FilesystemRepository::new(migrations_root);
	let rendered = repository
		.render(
			&result.migration,
			MigrationRenderOptions {
				include_header: !options.no_header,
			},
		)
		.map_err(command_error)?;

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
		.map_err(command_error)?;
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

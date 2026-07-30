//! Read-only migration state display.

use crate::database_selector::{DatabaseSelector, resolve_database};
use crate::{
	BaseCommand, CommandArgument, CommandContext, CommandError, CommandOption, CommandResult,
};
use async_trait::async_trait;
use reinhardt_db::migrations::{
	DatabaseMigrationRecorder, FilesystemSource, MigrationCatalog, MigrationKey, MigrationSnapshot,
};
use std::collections::BTreeMap;
use std::io::{self, Write as _};
use std::path::PathBuf;
use std::sync::Arc;

/// Output mode for `showmigrations`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShowMigrationsMode {
	/// Group migrations by application.
	List,
	/// Display the complete selected dependency order.
	Plan,
}

/// Output sink used by migration visibility commands.
pub trait MigrationVisibilityWriter: Send + Sync {
	/// Write one complete, fully buffered stdout result.
	fn write_stdout(&self, content: &str) -> io::Result<()>;
}

pub(crate) struct StandardMigrationVisibilityWriter;

impl MigrationVisibilityWriter for StandardMigrationVisibilityWriter {
	fn write_stdout(&self, content: &str) -> io::Result<()> {
		io::stdout().lock().write_all(content.as_bytes())
	}
}

/// Format one immutable migration snapshot.
pub fn format_migration_snapshot(
	snapshot: &MigrationSnapshot,
	mode: ShowMigrationsMode,
	verbosity: u8,
) -> String {
	match mode {
		ShowMigrationsMode::List => format_list(snapshot, verbosity),
		ShowMigrationsMode::Plan => format_plan(snapshot),
	}
}

fn format_list(snapshot: &MigrationSnapshot, verbosity: u8) -> String {
	let mut applications = BTreeMap::<&str, Vec<_>>::new();
	for migration in &snapshot.ordered {
		applications
			.entry(&migration.app_label)
			.or_default()
			.push(migration);
	}

	let mut output = String::new();
	for (application, migrations) in applications {
		output.push_str(application);
		output.push('\n');
		for migration in migrations {
			let key = MigrationKey::new(&migration.app_label, &migration.name);
			match snapshot.applied.get(&key) {
				Some(applied_at) => {
					output.push_str(" [X] ");
					output.push_str(&migration.name);
					if verbosity >= 2 {
						output.push_str(" (applied at ");
						output.push_str(&applied_at.format("%Y-%m-%d %H:%M:%S").to_string());
						output.push(')');
					}
				}
				None => {
					output.push_str(" [ ] ");
					output.push_str(&migration.name);
				}
			}
			output.push('\n');
		}
	}
	output
}

fn format_plan(snapshot: &MigrationSnapshot) -> String {
	let mut output = String::new();
	for migration in &snapshot.ordered {
		let key = MigrationKey::new(&migration.app_label, &migration.name);
		let marker = if snapshot.applied.contains_key(&key) {
			"[X]"
		} else {
			"[ ]"
		};
		output.push_str(marker);
		output.push(' ');
		output.push_str(&key.id());
		output.push('\n');
	}
	output
}

/// Display migration application state without modifying recorder history.
pub struct ShowMigrationsCommand {
	writer: Arc<dyn MigrationVisibilityWriter>,
}

impl ShowMigrationsCommand {
	/// Create a command with an injected stdout sink.
	pub fn with_writer(writer: Arc<dyn MigrationVisibilityWriter>) -> Self {
		Self { writer }
	}
}

impl Default for ShowMigrationsCommand {
	fn default() -> Self {
		Self::with_writer(Arc::new(StandardMigrationVisibilityWriter))
	}
}

pub(crate) fn with_command_context(error: CommandError, context: &str) -> CommandError {
	let message = |detail: String| format!("{context}: {detail}");
	match error {
		CommandError::NotFound(detail) => CommandError::NotFound(message(detail)),
		CommandError::InvalidArguments(detail) => CommandError::InvalidArguments(message(detail)),
		CommandError::ExecutionError(detail) => CommandError::ExecutionError(message(detail)),
		CommandError::FeatureDisabled(detail) => CommandError::FeatureDisabled(message(detail)),
		CommandError::IoError(error) => {
			CommandError::IoError(io::Error::new(error.kind(), message(error.to_string())))
		}
		CommandError::ParseError(detail) => CommandError::ParseError(message(detail)),
		CommandError::TemplateError(detail) => CommandError::TemplateError(message(detail)),
	}
}

#[async_trait]
impl BaseCommand for ShowMigrationsCommand {
	fn name(&self) -> &str {
		"showmigrations"
	}

	fn description(&self) -> &str {
		"Display migration application state or dependency order"
	}

	fn arguments(&self) -> Vec<CommandArgument> {
		vec![CommandArgument::optional(
			"app_labels",
			"Applications to include",
		)]
	}

	fn options(&self) -> Vec<CommandOption> {
		vec![
			CommandOption::flag(Some('l'), "list", "Group migrations by application"),
			CommandOption::flag(Some('p'), "plan", "Display dependency order"),
			CommandOption::option(None, "database", "Configured database alias")
				.with_default("default"),
			CommandOption::option(None, "database-url", "One-off database URL override"),
		]
	}

	fn requires_system_checks(&self) -> bool {
		false
	}

	async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
		let selector = DatabaseSelector {
			alias: ctx
				.option("database")
				.cloned()
				.unwrap_or_else(|| "default".to_string()),
			url_override: ctx.option("database-url").cloned(),
		};
		let command_context = format!(
			"showmigrations for database alias `{}`",
			selector.display_alias()
		);
		let resolved = resolve_database(&selector, ctx.settings.as_deref())
			.map_err(|error| with_command_context(error, &command_context))?;
		let source = FilesystemSource::new(
			ctx.option("migrations-dir")
				.map(PathBuf::from)
				.unwrap_or_else(|| PathBuf::from("./migrations")),
		);
		let catalog = MigrationCatalog::load_strict(&source)
			.await
			.map_err(crate::squashmigrations::migration_error_to_command_error)
			.map_err(|error| with_command_context(error, &command_context))?;
		let connection = resolved
			.connect()
			.await
			.map_err(|error| with_command_context(error, &command_context))?;
		let recorder = DatabaseMigrationRecorder::new(connection);
		let snapshot = catalog
			.snapshot(&recorder, &ctx.args)
			.await
			.map_err(crate::squashmigrations::migration_error_to_command_error)
			.map_err(|error| with_command_context(error, &command_context))?;
		let mode = if ctx.has_option("plan") {
			ShowMigrationsMode::Plan
		} else {
			ShowMigrationsMode::List
		};
		let output = format_migration_snapshot(&snapshot, mode, ctx.verbosity());
		self.writer
			.write_stdout(&output)
			.map_err(CommandError::IoError)
			.map_err(|error| with_command_context(error, &command_context))?;
		Ok(())
	}
}

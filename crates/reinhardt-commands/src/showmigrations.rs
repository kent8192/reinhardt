//! Read-only migration state display.

use crate::database_selector::{DatabaseSelector, resolve_database};
use crate::inspectdb::ensure_sqlite_database_exists;
use crate::{
	BaseCommand, CommandArgument, CommandContext, CommandError, CommandOption, CommandResult,
};
use async_trait::async_trait;
use reinhardt_conf::MigrationSettings;
use reinhardt_db::migrations::{
	DatabaseMigrationRecorder, DependencyResolutionContext, FilesystemSource, MigrationCatalog,
	MigrationKey, MigrationSnapshot,
};
use std::collections::BTreeMap;
use std::io::{self, Write as _};
use std::path::PathBuf;
use std::sync::Arc;

const MIGRATION_FEATURES_OPTION: &str = "__reinhardt_migration_features";
const MIGRATION_SETTING_OPTION_PREFIX: &str = "__reinhardt_migration_setting:";

pub(crate) fn attach_migration_settings(ctx: &mut CommandContext, settings: &MigrationSettings) {
	ctx.set_option_multi(
		MIGRATION_FEATURES_OPTION.to_string(),
		settings.migration_features.clone(),
	);
	for (key, value) in settings
		.migration_settings
		.iter()
		.chain(&settings.migration_swappable_settings)
	{
		ctx.set_option(
			format!("{MIGRATION_SETTING_OPTION_PREFIX}{key}"),
			value.clone(),
		);
	}
}

pub(crate) fn migration_source_path(ctx: &CommandContext) -> PathBuf {
	ctx.option("migrations-dir")
		.map(PathBuf::from)
		.unwrap_or_else(|| {
			ctx.settings.as_ref().map_or_else(
				|| PathBuf::from("./migrations"),
				|settings| settings.core().base_dir.join("migrations"),
			)
		})
}

pub(crate) fn migration_dependency_context(ctx: &CommandContext) -> DependencyResolutionContext {
	let mut dependency_context =
		ctx.settings
			.as_ref()
			.map_or_else(DependencyResolutionContext::new, |settings| {
				let core = settings.core();
				let mut context = DependencyResolutionContext::new()
					.with_apps(core.installed_apps.iter().cloned());
				for (key, value) in &core.migration_swappable_settings {
					context = context.with_setting(key, value);
				}
				for feature in &core.migration_features {
					context = context.with_feature(feature);
				}
				context
			});
	for feature in ctx
		.option_values(MIGRATION_FEATURES_OPTION)
		.unwrap_or_default()
	{
		dependency_context = dependency_context.with_feature(feature);
	}
	for (option, values) in &ctx.options {
		if let Some(key) = option.strip_prefix(MIGRATION_SETTING_OPTION_PREFIX)
			&& let Some(value) = values.first()
		{
			dependency_context = dependency_context.with_setting(key, value);
		}
	}
	dependency_context
}

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
			CommandOption::option(
				None,
				"migrations-dir",
				"Root directory containing migration files",
			),
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
		if resolved.backend() == reinhardt_db::backends::DatabaseType::Sqlite {
			ensure_sqlite_database_exists(resolved.url())
				.map_err(|error| with_command_context(error, &command_context))?;
		}
		let source = FilesystemSource::new(migration_source_path(ctx));
		let dependency_context = migration_dependency_context(ctx);
		let catalog = MigrationCatalog::load_strict_with_context(&source, &dependency_context)
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

//! Read-only migration SQL rendering.

use crate::database_selector::{DatabaseSelector, resolve_database};
use crate::inspectdb::ensure_sqlite_database_exists;
use crate::showmigrations::{
	MigrationVisibilityWriter, StandardMigrationVisibilityWriter, migration_dependency_context,
	migration_source_path, with_command_context,
};
use crate::{BaseCommand, CommandArgument, CommandContext, CommandOption, CommandResult};
use async_trait::async_trait;
use reinhardt_db::backends::{DatabaseConnection, DatabaseType};
use reinhardt_db::migrations::{
	FilesystemSource, Migration, MigrationCatalog, MigrationDirection, MigrationKey, SqlDialect,
	plan_migration_sql_with_states,
};
use std::sync::Arc;

/// Render one migration through the SQL planner shared with migration execution.
pub async fn render_migration_sql(
	connection: &DatabaseConnection,
	catalog: &MigrationCatalog,
	migration: &Migration,
	key: &MigrationKey,
	direction: MigrationDirection,
) -> reinhardt_db::migrations::Result<String> {
	let (state_before, state_after) = match direction {
		MigrationDirection::Forward => (catalog.state_before(key)?, catalog.state_after(key)?),
		MigrationDirection::Backward => catalog.states_for_rollback(key)?,
	};
	let plan = plan_migration_sql_with_states(
		connection,
		migration,
		&state_before,
		&state_after,
		direction,
	)
	.await?;
	Ok(plan.render(sql_dialect(connection)))
}

fn sql_dialect(connection: &DatabaseConnection) -> SqlDialect {
	if connection.is_cockroachdb() {
		return SqlDialect::Cockroachdb;
	}
	match connection.database_type() {
		DatabaseType::Postgres => SqlDialect::Postgres,
		DatabaseType::Mysql => SqlDialect::Mysql,
		DatabaseType::Sqlite => SqlDialect::Sqlite,
	}
}

/// Render SQL for one migration without executing schema or history changes.
pub struct SqlMigrateCommand {
	writer: Arc<dyn MigrationVisibilityWriter>,
}

impl SqlMigrateCommand {
	/// Create a command with an injected stdout sink.
	pub fn with_writer(writer: Arc<dyn MigrationVisibilityWriter>) -> Self {
		Self { writer }
	}
}

impl Default for SqlMigrateCommand {
	fn default() -> Self {
		Self::with_writer(Arc::new(StandardMigrationVisibilityWriter))
	}
}

#[async_trait]
impl BaseCommand for SqlMigrateCommand {
	fn name(&self) -> &str {
		"sqlmigrate"
	}

	fn description(&self) -> &str {
		"Render migration SQL without executing it"
	}

	fn arguments(&self) -> Vec<CommandArgument> {
		vec![
			CommandArgument::required("app_label", "Application containing the migration"),
			CommandArgument::required(
				"migration_name",
				"Exact migration name or unambiguous prefix",
			),
		]
	}

	fn options(&self) -> Vec<CommandOption> {
		vec![
			CommandOption::flag(None, "backwards", "Render rollback SQL"),
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
		let [app_label, migration_name] = ctx.args.as_slice() else {
			return Err(crate::CommandError::InvalidArguments(
				"sqlmigrate requires APP_LABEL and MIGRATION_NAME.".to_string(),
			));
		};
		let selector = DatabaseSelector {
			alias: ctx
				.option("database")
				.cloned()
				.unwrap_or_else(|| "default".to_string()),
			url_override: ctx.option("database-url").cloned(),
		};
		let command_context = format!(
			"sqlmigrate for {}.{} on database alias `{}`",
			app_label,
			migration_name,
			selector.display_alias()
		);
		let resolved = resolve_database(&selector, ctx.settings.as_deref())
			.map_err(|error| with_command_context(error, &command_context))?;
		if resolved.backend() == DatabaseType::Sqlite {
			ensure_sqlite_database_exists(resolved.url())
				.map_err(|error| with_command_context(error, &command_context))?;
		}
		let source = FilesystemSource::new(migration_source_path(ctx));
		let dependency_context = migration_dependency_context(ctx);
		let catalog = MigrationCatalog::load_strict_with_context(&source, &dependency_context)
			.await
			.map_err(crate::squashmigrations::migration_error_to_command_error)
			.map_err(|error| with_command_context(error, &command_context))?;
		let key = catalog
			.resolve_unique_prefix(app_label, migration_name)
			.map_err(crate::squashmigrations::migration_error_to_command_error)
			.map_err(|error| with_command_context(error, &command_context))?;
		let migration = catalog
			.migration(&key)
			.map_err(crate::squashmigrations::migration_error_to_command_error)
			.map_err(|error| with_command_context(error, &command_context))?;
		let connection = resolved
			.connect()
			.await
			.map_err(|error| with_command_context(error, &command_context))?;
		let direction = if ctx.has_option("backwards") {
			MigrationDirection::Backward
		} else {
			MigrationDirection::Forward
		};
		let output = render_migration_sql(&connection, &catalog, migration, &key, direction)
			.await
			.map_err(crate::squashmigrations::migration_error_to_command_error)
			.map_err(|error| with_command_context(error, &command_context))?;
		self.writer
			.write_stdout(&output)
			.map_err(crate::CommandError::IoError)
			.map_err(|error| with_command_context(error, &command_context))?;
		Ok(())
	}
}

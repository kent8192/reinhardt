//! Database schema inspection command adapter.

use crate::database_selector::{DatabaseSelector, resolve_database};
use crate::{
	BaseCommand, CommandArgument, CommandContext, CommandError, CommandOption, CommandResult,
};
use async_trait::async_trait;
use reinhardt_db::backends::DatabaseType;
use reinhardt_db::migrations::introspect::write_generated_files_atomically;
use reinhardt_db::migrations::{
	GeneratedOutput, InspectDbOptions, IntrospectConfig, generate_models_canonical,
	inspect_database, render_models_module,
};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Output sink used by [`InspectDbCommand`].
///
/// Generated source and human-readable progress use distinct methods so command
/// callers can safely redirect stdout to a Rust source file.
pub trait InspectDbWriter: Send + Sync {
	/// Write the complete generated stdout artifact.
	fn write_stdout(&self, content: &str) -> io::Result<()>;

	/// Write one progress or diagnostic message to stderr.
	fn write_stderr(&self, content: &str) -> io::Result<()>;

	/// Publish a complete generated directory output.
	///
	/// The default strategy uses the rollback-safe database migration writer.
	/// Embedders may replace the strategy when they own an equivalent
	/// all-or-nothing publication boundary.
	fn publish_generated_files(
		&self,
		output: &GeneratedOutput,
		force: bool,
	) -> reinhardt_db::migrations::Result<()> {
		write_generated_files_atomically(output, force)
	}
}

struct StandardInspectDbWriter;

impl InspectDbWriter for StandardInspectDbWriter {
	fn write_stdout(&self, content: &str) -> io::Result<()> {
		io::stdout().lock().write_all(content.as_bytes())
	}

	fn write_stderr(&self, content: &str) -> io::Result<()> {
		let mut stderr = io::stderr().lock();
		stderr.write_all(content.as_bytes())?;
		stderr.write_all(b"\n")
	}
}

/// Generate Reinhardt models from an existing database schema.
pub struct InspectDbCommand {
	writer: Arc<dyn InspectDbWriter>,
}

impl InspectDbCommand {
	/// Create a command with an injected stdout/stderr sink.
	///
	/// This supports embedders that need to capture generated source without
	/// replacing process-global streams.
	pub fn with_writer(writer: Arc<dyn InspectDbWriter>) -> Self {
		Self { writer }
	}

	fn progress(&self, message: &str) -> CommandResult<()> {
		self.writer.write_stderr(message)?;
		Ok(())
	}
}

impl Default for InspectDbCommand {
	fn default() -> Self {
		Self::with_writer(Arc::new(StandardInspectDbWriter))
	}
}

#[async_trait]
impl BaseCommand for InspectDbCommand {
	fn name(&self) -> &str {
		"inspectdb"
	}

	fn description(&self) -> &str {
		"Generate Reinhardt ORM models from an existing database schema"
	}

	fn arguments(&self) -> Vec<CommandArgument> {
		vec![CommandArgument::optional(
			"tables",
			"Exact table names to inspect",
		)]
	}

	fn options(&self) -> Vec<CommandOption> {
		vec![
			CommandOption::option(None, "database", "Configured database alias")
				.with_default("default"),
			CommandOption::option(None, "database-url", "One-off database URL override"),
			CommandOption::flag(None, "include-views", "Include database views"),
			CommandOption::flag(None, "include-partitions", "Include PostgreSQL partitions"),
			CommandOption::option(Some('o'), "output", "Output directory for generated files"),
			CommandOption::option(Some('c'), "config", "Path to configuration TOML file"),
			CommandOption::flag(None, "force", "Overwrite existing generated files"),
		]
	}

	fn requires_system_checks(&self) -> bool {
		false
	}

	async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
		let output_directory = ctx.option("output").map(PathBuf::from);
		if ctx.has_option("force") && output_directory.is_none() {
			return Err(CommandError::InvalidArguments(
				"`--force` requires `--output`.".to_string(),
			));
		}

		let config_path = ctx.option("config");
		let mut config = match config_path {
			Some(path) => load_config(PathBuf::from(path))?,
			None => IntrospectConfig::default(),
		};
		if config_path.is_none() {
			config.tables.exclude.clear();
		}
		// Database selection is owned by DatabaseSelector. Configuration files
		// control generation and filtering but never override the selected URL.
		config.database.url.clear();
		if let Some(directory) = &output_directory {
			config.output.directory = directory.clone();
			config.output.single_file = false;
		}

		let selector = DatabaseSelector {
			alias: ctx
				.option("database")
				.cloned()
				.unwrap_or_else(|| "default".to_string()),
			url_override: ctx.option("database-url").cloned(),
		};
		let resolved = resolve_database(&selector, ctx.settings.as_deref())?;
		if resolved.alias() != selector.alias || !resolved.url().contains(':') {
			return Err(CommandError::ExecutionError(
				"Database selection returned an invalid identity.".to_string(),
			));
		}
		if ctx.has_option("include-partitions") && resolved.backend() != DatabaseType::Postgres {
			return Err(CommandError::InvalidArguments(
				"include_partitions is only supported for PostgreSQL.".to_string(),
			));
		}
		if resolved.backend() == DatabaseType::Sqlite {
			ensure_sqlite_database_exists(resolved.url())?;
		}
		let options = InspectDbOptions {
			tables: ctx.args.clone(),
			include_views: ctx.has_option("include-views"),
			include_partitions: ctx.has_option("include-partitions"),
		};

		self.progress(&format!(
			"Inspecting database schema ({:?})...",
			resolved.backend()
		))?;
		let connection = resolved.connect().await?;
		let schema = inspect_database(&connection, &options)
			.await
			.map_err(|error| {
				CommandError::ExecutionError(format!("Database inspection failed: {error}"))
			})?;
		if let Some(table) = schema
			.tables
			.values()
			.filter(|table| config.should_include_table(&table.name))
			.find(|table| table.primary_key.is_empty())
		{
			return Err(CommandError::ExecutionError(format!(
				"Cannot generate model for `{}` because it has no primary key.",
				table.name
			)));
		}
		self.progress(&format!("Found {} schema objects", schema.tables.len()))?;

		if output_directory.is_some() {
			let output = generate_models_canonical(&config, &schema).map_err(generation_error)?;
			validate_generated_files(&output)?;
			self.writer
				.publish_generated_files(&output, ctx.has_option("force"))
				.map_err(|error| {
					CommandError::ExecutionError(format!("Generated file write failed: {error}"))
				})?;
			self.progress(&format!("Generated {} files", output.files.len()))?;
		} else {
			let module = render_models_module(&config, &schema).map_err(generation_error)?;
			syn::parse_file(&module).map_err(|error| {
				CommandError::ParseError(format!(
					"inspectdb generated an invalid Rust module: {error}"
				))
			})?;
			self.writer.write_stdout(&module)?;
			self.progress("Generated models module")?;
		}

		Ok(())
	}
}

fn ensure_sqlite_database_exists(url: &str) -> CommandResult<()> {
	let Some(path_and_query) = url.strip_prefix("sqlite:") else {
		return Ok(());
	};
	let (path, query) = path_and_query
		.split_once('?')
		.unwrap_or((path_and_query, ""));
	if path == ":memory:" || query.split('&').any(|part| part == "mode=memory") {
		return Ok(());
	}
	let path = path.strip_prefix("//").unwrap_or(path);
	if path.is_empty() || Path::new(path).is_file() {
		return Ok(());
	}
	Err(CommandError::ExecutionError(
		"SQLite database file does not exist.".to_string(),
	))
}

fn load_config(path: PathBuf) -> CommandResult<IntrospectConfig> {
	let content = std::fs::read_to_string(&path).map_err(|error| {
		CommandError::InvalidArguments(format!(
			"Failed to read inspectdb configuration `{}` ({:?}).",
			path.display(),
			error.kind()
		))
	})?;
	IntrospectConfig::from_toml(&content).map_err(|_| {
		CommandError::InvalidArguments(format!(
			"Failed to parse inspectdb configuration `{}`.",
			path.display()
		))
	})
}

fn validate_generated_files(output: &GeneratedOutput) -> CommandResult<()> {
	for file in &output.files {
		syn::parse_file(&file.content).map_err(|error| {
			CommandError::ParseError(format!(
				"inspectdb generated invalid Rust source for `{}`: {error}",
				file.path.display()
			))
		})?;
	}
	Ok(())
}

fn generation_error(error: reinhardt_db::migrations::MigrationError) -> CommandError {
	CommandError::ExecutionError(format!("Model generation failed: {error}"))
}

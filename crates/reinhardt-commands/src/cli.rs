//! CLI runner for Reinhardt management commands
//!
//! This module provides a unified interface for executing commands from generated `manage.rs` files.
//! It handles argument parsing, command context creation, and command execution.

#[cfg(feature = "migrations")]
use crate::MakeMigrationsCommand;
use crate::base::BaseCommand;
use crate::collectstatic::{CollectStaticCommand, CollectStaticOptions};
use crate::local_infra::InfraSubcommand;
use crate::registry::CommandRegistry;
use crate::{
	CheckCommand, CommandContext, MigrateCommand, RunServerCommand, ShellCommand, ShellConfig,
};
#[cfg(feature = "introspect")]
use clap::ValueEnum;
use clap::{Parser, Subcommand};
use reinhardt_conf::HasCommonSettings;
use reinhardt_conf::settings::builder::SettingsBuilder;
use reinhardt_conf::settings::profile::Profile;
use reinhardt_conf::settings::sources::{DefaultSource, LowPriorityEnvSource, TomlFileSource};
#[cfg(feature = "migrations")]
use reinhardt_db::migrations::DependencyResolutionContext;
use reinhardt_utils::staticfiles::{PathResolver, StaticFilesConfig};
use serde_json::Value;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[cfg(feature = "routers")]
use crate::builtin::ShowUrlsCommand;

/// Reinhardt Project Management CLI
///
/// This is the CLI parser used by `execute_from_command_line()`.
/// Can also be used directly for testing CLI parsing behavior.
#[derive(Debug, Parser)]
#[command(name = "manage")]
#[command(about = "Reinhardt management interface", long_about = None)]
#[command(version)]
pub struct Cli {
	/// Subcommand to execute
	#[command(subcommand)]
	pub command: Commands,

	/// Verbosity level (can be repeated for more output)
	#[arg(short, long, action = clap::ArgAction::Count)]
	pub verbosity: u8,
}

#[cfg(feature = "migrations")]
fn default_migrations_dir() -> PathBuf {
	PathResolver::find_project_root()
		.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
		.join("migrations")
}

/// Output format for the introspect command
#[cfg(feature = "introspect")]
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
	/// YAML output (default)
	Yaml,
	/// JSON output
	Json,
}

/// Command-line interface commands
///
/// This enum defines all available management commands.
#[derive(Debug, Clone, Subcommand)]
pub enum Commands {
	/// Create new migrations based on model changes
	#[cfg(feature = "migrations")]
	Makemigrations {
		/// App labels to create migrations for
		#[arg(value_name = "APP_LABEL")]
		app_labels: Vec<String>,

		/// Dry run - don't actually write files
		#[arg(long)]
		dry_run: bool,

		/// Migration name
		#[arg(short = 'n', long, value_name = "NAME")]
		name: Option<String>,

		/// Check if migrations are missing
		#[arg(long)]
		check: bool,

		/// Create empty migration
		#[arg(long)]
		empty: bool,

		/// Fix migration conflicts (create merge migration)
		#[arg(long)]
		merge: bool,

		/// Force using empty state when database/TestContainers is unavailable (dangerous)
		#[arg(long)]
		force_empty_state: bool,

		/// Migration directory
		#[arg(long, default_value = "./migrations")]
		migration_dir: PathBuf,
	},

	/// Squash a continuous range of migrations into one migration
	#[cfg(feature = "migrations")]
	#[command(allow_missing_positional = true)]
	Squashmigrations {
		/// Application whose migrations will be squashed
		#[arg(value_name = "APP_LABEL")]
		app_label: String,

		/// Optional first migration in the squash range
		#[arg(value_name = "START_MIGRATION")]
		start_migration: Option<String>,

		/// Last migration in the squash range
		#[arg(value_name = "MIGRATION_NAME")]
		migration_name: String,

		/// Preserve the exact source operation order
		#[arg(long)]
		no_optimize: bool,

		/// Do not prompt for confirmation
		#[arg(long, visible_alias = "noinput")]
		no_input: bool,

		/// Omit the generated-file header
		#[arg(long)]
		no_header: bool,

		/// Explicit name for the new squashed migration
		#[arg(long, value_name = "NAME")]
		squashed_name: Option<String>,

		/// Root directory containing migration files
		#[arg(long, value_name = "DIR")]
		migrations_dir: Option<PathBuf>,
	},

	/// Apply database migrations
	Migrate {
		/// App label to migrate
		#[arg(value_name = "APP_LABEL")]
		app_label: Option<String>,

		/// Migration name to migrate to
		#[arg(value_name = "MIGRATION_NAME")]
		migration_name: Option<String>,

		/// Database connection string
		#[arg(long, value_name = "DATABASE")]
		database: Option<String>,

		/// Fake migration (mark as applied without running)
		#[arg(long)]
		fake: bool,

		/// Fake initial migration only
		#[arg(long)]
		fake_initial: bool,

		/// Show migration plan without applying
		#[arg(long)]
		plan: bool,

		/// Root directory containing migration files (default: ./migrations)
		#[arg(long, value_name = "DIR")]
		migrations_dir: Option<PathBuf>,
	},

	/// Manage local development infrastructure containers
	Infra {
		/// Infrastructure subcommand to execute
		#[command(subcommand)]
		command: InfraSubcommand,
	},

	/// Start the development server
	#[non_exhaustive]
	Runserver {
		/// Server address (default: 127.0.0.1:8000)
		#[arg(value_name = "ADDRESS", default_value = "127.0.0.1:8000")]
		address: String,

		/// Disable auto-reload
		#[arg(long)]
		noreload: bool,

		/// Watch delay in milliseconds for file change debouncing
		#[arg(long = "watch-delay", default_value_t = 120)]
		watch_delay: u64,

		/// Disable the WASM rebuild pipeline during hot-reload (server pipeline still runs).
		#[arg(long = "no-wasm-rebuild")]
		no_wasm_rebuild: bool,

		/// Skip the WASM build at startup (existing artifacts in dist/ are served as-is).
		#[arg(long = "no-wasm")]
		no_wasm: bool,

		/// Reuse existing WASM artifacts in dist/ if present (default: rebuild on every start).
		#[arg(long = "no-override-wasm")]
		no_override_wasm: bool,

		/// DEPRECATED: rebuild is now the default. Use --no-override-wasm to opt out.
		#[arg(long = "force-wasm")]
		force_wasm: bool,

		/// Allow the server to start even when the WASM build fails.
		#[arg(long = "wasm-optional")]
		wasm_optional: bool,

		/// Serve static files in development mode
		#[arg(long)]
		insecure: bool,

		/// Disable automatic OpenAPI documentation endpoints
		#[arg(long)]
		no_docs: bool,

		/// Enable WASM frontend serving (serves static files from dist/)
		#[arg(long)]
		with_pages: bool,

		/// Static files directory for WASM frontend
		#[arg(long, default_value = "dist")]
		static_dir: String,

		/// Disable SPA mode (no index.html fallback)
		#[arg(long)]
		no_spa: bool,

		/// Path to index.html for SPA fallback (auto-detected from project root)
		#[arg(long)]
		index: Option<String>,

		/// Cargo package containing component style definitions
		#[arg(long, value_name = "NAME")]
		package: Option<String>,

		/// Cargo features enabled for the Pages component style package
		#[arg(
			long,
			value_delimiter = ',',
			value_name = "FEATURE",
			conflicts_with = "all_features"
		)]
		features: Vec<String>,

		/// Enable all Cargo features for the Pages component style package
		#[arg(long)]
		all_features: bool,
	},

	/// Run an interactive Rust shell (REPL)
	Shell {
		/// Execute a command and exit
		#[arg(short = 'c', long, value_name = "COMMAND")]
		command: Option<String>,
	},

	/// Check the project for common issues
	Check {
		/// Check specific app
		#[arg(value_name = "APP_LABEL")]
		app_label: Option<String>,

		/// Deploy check (stricter checks)
		#[arg(long)]
		deploy: bool,
	},

	/// Collect static files into STATIC_ROOT
	#[non_exhaustive]
	Collectstatic {
		/// Clear existing files before collecting
		#[arg(long)]
		clear: bool,

		/// Do not prompt for confirmation
		#[arg(long)]
		no_input: bool,

		/// Do not actually collect, just show what would be collected
		#[arg(long)]
		dry_run: bool,

		/// Create symbolic links instead of copying files
		#[arg(long)]
		link: bool,

		/// Ignore file patterns (glob)
		#[arg(long, value_name = "PATTERN")]
		ignore: Vec<String>,

		/// Path to index.html source file (auto-detected from project root)
		#[arg(long)]
		index: Option<String>,

		/// Cargo package containing component style definitions
		#[arg(long, value_name = "NAME")]
		package: Option<String>,

		/// Cargo features enabled for the component style package
		#[arg(
			long,
			value_delimiter = ',',
			value_name = "FEATURE",
			conflicts_with = "all_features"
		)]
		features: Vec<String>,

		/// Enable all Cargo features for the component style package
		#[arg(long)]
		all_features: bool,
	},

	/// Display all registered server URL patterns
	Showurls {
		/// Show only named URLs
		#[arg(long)]
		names: bool,
	},

	/// Output structured project metadata for platform introspection
	#[cfg(feature = "introspect")]
	Introspect {
		/// Output format: yaml (default) or json
		#[arg(short = 'f', long, value_enum, default_value_t = OutputFormat::Yaml)]
		format: OutputFormat,

		/// Output only a specific section (app, databases, routes, middleware, settings, features)
		#[arg(short = 's', long)]
		section: Option<String>,
	},

	/// Generate OpenAPI 3.0 schema from registered endpoints
	#[cfg(feature = "openapi")]
	Generateopenapi {
		/// Output format (json or yaml)
		#[arg(short = 'f', long, default_value = "json")]
		format: String,

		/// Output file path
		#[arg(short = 'o', long, default_value = "openapi.json")]
		output: PathBuf,

		/// Also generate Postman Collection
		#[arg(long)]
		postman: bool,
	},

	/// Create a superuser account.
	///
	/// In non-interactive mode (`--noinput`), the password is read from the
	/// `REINHARDT_SUPERUSER_PASSWORD` environment variable. Use `--no-password`
	/// to create an account with no password (the account will not be able to
	/// log in). Setting `--no-password` together with `REINHARDT_SUPERUSER_PASSWORD`
	/// is rejected as mutually exclusive.
	#[cfg(feature = "auth")]
	Createsuperuser {
		/// Username for the superuser
		#[arg(long, value_name = "USERNAME")]
		username: Option<String>,

		/// Email address for the superuser
		#[arg(long, value_name = "EMAIL")]
		email: Option<String>,

		/// Skip the password prompt and create an account with no password
		/// (cannot log in; mutually exclusive with REINHARDT_SUPERUSER_PASSWORD)
		#[arg(long)]
		no_password: bool,

		/// Non-interactive mode (requires --username, --email, and either
		/// REINHARDT_SUPERUSER_PASSWORD or --no-password)
		#[arg(long)]
		noinput: bool,

		/// Database connection string
		#[arg(long, value_name = "DATABASE")]
		database: Option<String>,
	},

	/// Execute a custom command registered in a `CommandRegistry`
	///
	/// This variant is not exposed in the CLI help. It is used internally
	/// by [`execute_from_command_line_with_registry`] to dispatch commands
	/// registered by the downstream project and fixture commands that preserve
	/// their CLI syntax without expanding this public enum.
	#[command(skip)]
	Custom {
		/// The name of the custom command to execute.
		name: String,
		/// Positional arguments forwarded to the custom command.
		args: Vec<String>,
	},
}

#[cfg(feature = "reinhardt-db")]
#[derive(Debug)]
enum FixtureCommand {
	Dumpdata {
		selectors: Vec<String>,
		exclude: Vec<String>,
	},
	Loaddata {
		fixtures: Vec<PathBuf>,
	},
	Seed {
		app_labels: Vec<String>,
	},
}

#[cfg(feature = "reinhardt-db")]
#[derive(Debug, Parser)]
#[command(name = "dumpdata")]
struct DumpdataArgs {
	#[arg(value_name = "APP_OR_MODEL")]
	selectors: Vec<String>,
	#[arg(long, value_name = "APP_OR_MODEL")]
	exclude: Vec<String>,
}

#[cfg(feature = "reinhardt-db")]
#[derive(Debug, Parser)]
#[command(name = "loaddata")]
struct LoaddataArgs {
	#[arg(value_name = "FIXTURE")]
	fixtures: Vec<PathBuf>,
}

#[cfg(feature = "reinhardt-db")]
#[derive(Debug, Parser)]
#[command(name = "seed")]
struct SeedArgs {
	#[arg(value_name = "APP_LABEL")]
	app_labels: Vec<String>,
}

#[cfg(feature = "reinhardt-db")]
fn fixture_command_argv(name: &str, args: &[String]) -> Vec<String> {
	std::iter::once(name.to_string())
		.chain(args.iter().cloned())
		.collect()
}

#[cfg(feature = "reinhardt-db")]
fn parse_fixture_command(
	name: &str,
	args: &[String],
) -> Result<Option<FixtureCommand>, clap::Error> {
	match name {
		"dumpdata" => DumpdataArgs::try_parse_from(fixture_command_argv(name, args)).map(|args| {
			Some(FixtureCommand::Dumpdata {
				selectors: args.selectors,
				exclude: args.exclude,
			})
		}),
		"loaddata" => LoaddataArgs::try_parse_from(fixture_command_argv(name, args)).map(|args| {
			Some(FixtureCommand::Loaddata {
				fixtures: args.fixtures,
			})
		}),
		"seed" => SeedArgs::try_parse_from(fixture_command_argv(name, args)).map(|args| {
			Some(FixtureCommand::Seed {
				app_labels: args.app_labels,
			})
		}),
		_ => Ok(None),
	}
}

fn is_fixture_command_name(name: &str) -> bool {
	#[cfg(feature = "reinhardt-db")]
	{
		matches!(name, "dumpdata" | "loaddata" | "seed")
	}
	#[cfg(not(feature = "reinhardt-db"))]
	{
		let _ = name;
		false
	}
}

/// Execute commands from command-line arguments
///
/// This is the Django-style entry point that parses command-line arguments
/// and executes the appropriate command. This should be called from `manage.rs`.
///
/// # Automatic Router Registration
///
/// The framework automatically discovers and registers URL pattern functions
/// from projects that use the `register_url_patterns!()` macro in their
/// `src/config/urls.rs` file. No manual router registration is needed in `manage.rs`.
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error message on failure.
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_commands::execute_from_command_line;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     unsafe {
///         std::env::set_var("REINHARDT_SETTINGS_MODULE", "myproject.config.settings");
///     }
///
///     if let Err(e) = execute_from_command_line().await {
///         eprintln!("Error: {}", e);
///         std::process::exit(1);
///     }
///     Ok(())
/// }
/// ```
pub async fn execute_from_command_line() -> Result<(), Box<dyn std::error::Error>> {
	execute_from_command_line_with_registry(CommandRegistry::new()).await
}

/// Execute commands from command-line arguments with the project's composed settings.
///
/// This is the settings-aware counterpart of [`execute_from_command_line`]. The
/// project's generated `src/bin/manage.rs` calls this with its own
/// `get_settings()` result so that database-requiring commands (`migrate`,
/// `makemigrations`, `runserver`, `createsuperuser`) can resolve the database
/// connection from `settings/*.toml` (the `[core.databases.default]` block) when
/// `DATABASE_URL` is not set. Without settings, those commands fall back to the
/// `DATABASE_URL` environment variable (see [`execute_from_command_line`]).
///
/// # Arguments
///
/// * `settings` - The composed application settings, typically a
///   `ProjectSettings` value built via `#[settings(...)]` + `SettingsBuilder`.
///   Any type implementing [`HasCommonSettings`] is accepted.
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error message on failure.
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_commands::execute_from_command_line_with_settings;
/// # use reinhardt_conf::settings::contacts::ContactSettings;
/// # use reinhardt_conf::settings::core_settings::CoreSettings;
/// # use reinhardt_conf::settings::fragment::HasSettings;
/// # // Stands in for the project's `#[settings(...)]`-generated `ProjectSettings`.
/// # struct ProjectSettings { core: CoreSettings, contacts: ContactSettings }
/// # impl HasSettings<CoreSettings> for ProjectSettings {
/// #     fn get_settings(&self) -> &CoreSettings { &self.core }
/// # }
/// # impl HasSettings<ContactSettings> for ProjectSettings {
/// #     fn get_settings(&self) -> &ContactSettings { &self.contacts }
/// # }
/// # fn get_settings() -> ProjectSettings {
/// #     ProjectSettings { core: CoreSettings::default(), contacts: ContactSettings::default() }
/// # }
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     if let Err(e) = execute_from_command_line_with_settings(get_settings()).await {
///         eprintln!("Error: {}", e);
///         std::process::exit(1);
///     }
///     Ok(())
/// }
/// ```
pub async fn execute_from_command_line_with_settings<S>(
	settings: S,
) -> Result<(), Box<dyn std::error::Error>>
where
	S: HasCommonSettings + 'static,
{
	execute_from_command_line_with_registry_and_settings(CommandRegistry::new(), settings).await
}

/// Execute command-line arguments with project settings and Rust shell configuration.
pub async fn execute_from_command_line_with_settings_and_shell<S>(
	settings: S,
	shell: ShellConfig,
) -> crate::CommandResult<()>
where
	S: HasCommonSettings + Clone + Send + Sync + 'static,
{
	execute_from_command_line_with_registry_and_settings_and_shell(
		CommandRegistry::new(),
		settings,
		shell,
	)
	.await
}

/// Execute commands from command-line arguments with a custom command registry.
///
/// This entry point works like [`execute_from_command_line`] but additionally
/// accepts a [`CommandRegistry`] containing user-defined management commands.
/// If the subcommand parsed from CLI arguments does not match any built-in
/// command, the registry is consulted for a matching custom command.
///
/// # Arguments
///
/// * `registry` - A [`CommandRegistry`] holding custom commands to make available.
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error message on failure.
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_commands::{execute_from_command_line_with_registry, CommandRegistry};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     unsafe {
///         std::env::set_var("REINHARDT_SETTINGS_MODULE", "myproject.config.settings");
///     }
///
///     let mut registry = CommandRegistry::new();
///     // registry.register(Box::new(MyCustomCommand));
///
///     if let Err(e) = execute_from_command_line_with_registry(registry).await {
///         eprintln!("Error: {}", e);
///         std::process::exit(1);
///     }
///     Ok(())
/// }
/// ```
pub async fn execute_from_command_line_with_registry(
	registry: CommandRegistry,
) -> Result<(), Box<dyn std::error::Error>> {
	execute_with_registry_and_optional_settings(registry, None, None).await
}

/// Execute commands from CLI arguments with a custom command registry **and** the
/// project's composed settings.
///
/// Combines [`execute_from_command_line_with_registry`] (custom commands) with
/// [`execute_from_command_line_with_settings`] (settings-aware database
/// resolution). See those for details.
///
/// # Arguments
///
/// * `registry` - A [`CommandRegistry`] holding custom commands to make available.
/// * `settings` - The composed application settings (any [`HasCommonSettings`]).
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error message on failure.
pub async fn execute_from_command_line_with_registry_and_settings<S>(
	registry: CommandRegistry,
	settings: S,
) -> Result<(), Box<dyn std::error::Error>>
where
	S: HasCommonSettings + 'static,
{
	execute_with_registry_and_optional_settings(
		registry,
		Some(Arc::new(settings) as Arc<dyn HasCommonSettings>),
		None,
	)
	.await
}

/// Execute CLI arguments with a custom registry, project settings, and shell configuration.
pub async fn execute_from_command_line_with_registry_and_settings_and_shell<S>(
	registry: CommandRegistry,
	settings: S,
	shell: ShellConfig,
) -> crate::CommandResult<()>
where
	S: HasCommonSettings + Clone + Send + Sync + 'static,
{
	execute_with_registry_and_optional_settings(
		registry,
		Some(Arc::new(settings) as Arc<dyn HasCommonSettings>),
		Some(shell),
	)
	.await
	.map_err(boxed_command_error)
}

fn boxed_command_error(error: Box<dyn std::error::Error>) -> crate::CommandError {
	match error.downcast::<crate::CommandError>() {
		Ok(error) => *error,
		Err(error) => crate::CommandError::ExecutionError(error.to_string()),
	}
}

/// Shared driver: parse CLI arguments, perform pre-dispatch registration, and run
/// the resolved command with the optional composed settings threaded into the
/// command context.
async fn execute_with_registry_and_optional_settings(
	registry: CommandRegistry,
	settings: Option<Arc<dyn HasCommonSettings>>,
	shell: Option<ShellConfig>,
) -> Result<(), Box<dyn std::error::Error>> {
	// Attempt normal clap parsing first. If it fails (e.g., unknown subcommand),
	// fall back to checking the registry for a matching custom command.
	let raw_args: Vec<String> = env::args().collect();
	let normalized_args = normalize_count_style_verbosity_args(&raw_args);
	let (command, verbosity) = match Cli::try_parse_from(&normalized_args) {
		Ok(cli) => (cli.command, cli.verbosity),
		Err(clap_err) => {
			// Only intercept "unknown subcommand" errors; re-raise others (--help, --version, etc.)
			if !is_unknown_subcommand(&clap_err) {
				clap_err.exit();
			}

			// Resolve custom commands from the original arguments to preserve the
			// legacy value-style verbosity convention for their command contexts.
			match resolve_custom_command(&raw_args, &registry) {
				Some((name, args, verbosity)) => {
					#[cfg(feature = "reinhardt-db")]
					if registry.get(&name).is_none()
						&& is_fixture_command_name(&name)
						&& let Err(error) = parse_fixture_command(&name, &args)
					{
						error.exit();
					}
					(Commands::Custom { name, args }, verbosity)
				}
				None => {
					// No custom command matched either; let clap display its error.
					clap_err.exit();
				}
			}
		}
	};

	// Only register router for commands that serve HTTP traffic.
	// DB-only commands (migrate, makemigrations) and utility commands
	// (shell, check, collectstatic) must not require route registration.
	if requires_router(&command) {
		auto_register_router().await?;
	}

	// Auto-register SuperuserCreator from inventory (if available).
	// This replaces the manual register_superuser_creator() call that
	// users previously had to add in main(). (#3187)
	#[cfg(feature = "auth")]
	reinhardt_auth::auto_register_superuser_creator();

	run_command_core(command, verbosity, registry, settings, shell).await
}

/// Returns `true` for commands that need URL patterns registered **before**
/// the command body runs.
///
/// `Runserver` is intentionally **not** in this list (Refs #4453): the
/// HTTP-route inventory pull is now performed explicitly inside
/// [`RunServerCommand::execute`] so that the
/// registration step is visible at the command's call site rather than
/// hidden in this dispatch loop. `Showurls` / `Introspect` /
/// `Generateopenapi` still receive the pre-dispatch
/// [`auto_register_router`] call until they grow their own explicit
/// `register_*_from_inventory()` methods (tracked separately).
fn requires_router(command: &Commands) -> bool {
	match command {
		#[cfg(feature = "routers")]
		Commands::Showurls { .. } => true,
		#[cfg(feature = "introspect")]
		Commands::Introspect { .. } => true,
		#[cfg(feature = "openapi")]
		Commands::Generateopenapi { .. } => true,
		_ => false,
	}
}

/// Returns `true` for commands that require ORM database initialization.
///
/// Database-requiring commands get automatic ORM initialization
/// before execution via [`initialize_orm_database()`].
/// This is symmetric with [`requires_router()`] which controls HTTP route registration.
#[cfg(feature = "reinhardt-db")]
fn requires_database(command: &Commands, registry: &CommandRegistry) -> bool {
	match command {
		Commands::Runserver { .. } => true,
		Commands::Migrate { .. } => true,
		Commands::Shell { .. } => false,
		Commands::Custom { name, .. } => {
			registry.get(name).is_none() && is_fixture_command_name(name)
		}
		#[cfg(feature = "auth")]
		Commands::Createsuperuser { .. } => true,
		_ => false,
	}
}

/// Execute a command with the given verbosity level.
///
/// This is the internal entry point for executing built-in commands.
/// For most use cases, prefer using [`execute_from_command_line`] or
/// [`execute_from_command_line_with_registry`] instead.
///
/// Fixture command names carried by [`Commands::Custom`] are dispatched
/// directly. Use [`run_command_with_registry`] when registered custom commands
/// may be present.
///
/// # Arguments
///
/// * `command` - The command to execute
/// * `verbosity` - Verbosity level (0-3, higher is more verbose)
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error message on failure.
pub async fn run_command(
	command: Commands,
	verbosity: u8,
) -> Result<(), Box<dyn std::error::Error>> {
	run_command_with_registry(command, verbosity, CommandRegistry::new()).await
}

/// Execute a command with the given verbosity level and a custom command registry.
///
/// This extends [`run_command`] by also checking the provided [`CommandRegistry`]
/// when a [`Commands::Custom`] variant is encountered.
///
/// # Arguments
///
/// * `command` - The command to execute
/// * `verbosity` - Verbosity level (0-3, higher is more verbose)
/// * `registry` - A [`CommandRegistry`] for resolving custom commands
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error message on failure.
pub async fn run_command_with_registry(
	command: Commands,
	verbosity: u8,
	registry: CommandRegistry,
) -> Result<(), Box<dyn std::error::Error>> {
	run_command_core(command, verbosity, registry, None, None).await
}

/// Execute a command with optional composed settings threaded into the context.
///
/// This settings-aware core backs both the no-settings entry points
/// ([`run_command`], [`run_command_with_registry`]) and the settings-aware ones
/// ([`execute_from_command_line_with_settings`]). When `settings` is `Some`, the
/// database-init context receives the composed `ProjectSettings`, so
/// [`initialize_orm_database`] can
/// resolve the URL from `[core.databases.default]` even when `DATABASE_URL` is
/// unset (#5042).
async fn run_command_core(
	command: Commands,
	verbosity: u8,
	registry: CommandRegistry,
	settings: Option<Arc<dyn HasCommonSettings>>,
	shell: Option<ShellConfig>,
) -> Result<(), Box<dyn std::error::Error>> {
	// Initialize ORM database for commands that require it.
	// This must happen before command dispatch so that commands like
	// createsuperuser can use the ORM connection pool. (#3186)
	#[cfg(feature = "reinhardt-db")]
	if requires_database(&command, &registry) {
		let mut ctx = crate::CommandContext::new(vec![]);
		ctx.verbosity = verbosity;
		ctx.set_output_suppressed(matches!(
			&command,
			Commands::Custom { name, .. } if name == "dumpdata"
		));
		// Thread the project's composed settings into the ORM-init context so the
		// database URL can be resolved from `settings/*.toml`
		// (`[core.databases.default]`) when `DATABASE_URL` is unset (#5042).
		if let Some(s) = settings.clone() {
			ctx = ctx.with_settings(s);
		}
		crate::builtin::initialize_orm_database(&ctx).await?;
	}

	// `settings` is consumed by the database-init block above and the
	// `makemigrations` arm below; bind it in feature combinations where neither
	// path is compiled so it does not trip the unused-variable lint.
	#[cfg(not(any(feature = "reinhardt-db", feature = "migrations")))]
	let _ = &settings;

	match command {
		#[cfg(feature = "migrations")]
		Commands::Makemigrations {
			app_labels,
			dry_run,
			name,
			check,
			empty,
			merge,
			force_empty_state,
			migration_dir: _,
		} => {
			execute_makemigrations(
				app_labels,
				dry_run,
				name,
				check,
				empty,
				merge,
				force_empty_state,
				verbosity,
				settings.clone(),
			)
			.await
		}
		#[cfg(feature = "migrations")]
		Commands::Squashmigrations {
			app_label,
			start_migration,
			migration_name,
			no_optimize,
			no_input,
			no_header,
			squashed_name,
			migrations_dir,
		} => {
			let dependency_context =
				settings
					.as_ref()
					.map_or_else(DependencyResolutionContext::new, |settings| {
						let core = settings.core();
						let context = core.migration_features.iter().fold(
							DependencyResolutionContext::new()
								.with_apps(core.installed_apps.iter().cloned()),
							|context, feature| context.with_feature(feature.clone()),
						);
						core.migration_swappable_settings.iter().fold(
							context,
							|context, (key, value)| {
								context.with_setting(key.clone(), value.clone())
							},
						)
					});
			let mut confirmation = crate::StdinConfirmationReader;
			let standard_output = std::io::stdout();
			let standard_error = std::io::stderr();
			let mut stdout = standard_output.lock();
			let mut stderr = standard_error.lock();
			let migrations_dir = migrations_dir.unwrap_or_else(default_migrations_dir);
			crate::execute_squashmigrations_with_context_and_io(
				&migrations_dir,
				crate::SquashMigrationsOptions {
					app_label,
					start_migration,
					migration_name,
					no_optimize,
					no_input,
					no_header,
					squashed_name,
				},
				&dependency_context,
				&mut confirmation,
				&mut stdout,
				&mut stderr,
			)
			.await
			.map(|_| ())
			.map_err(Into::into)
		}
		Commands::Migrate {
			app_label,
			migration_name,
			database,
			fake,
			fake_initial,
			plan,
			migrations_dir,
		} => {
			execute_migrate(MigrateParams {
				app_label,
				migration_name,
				database,
				fake,
				fake_initial,
				plan,
				migrations_dir,
				verbosity,
			})
			.await
		}
		Commands::Infra { command } => {
			crate::local_infra::InfraCommand::execute(
				command,
				&std::env::current_dir()?,
				settings.as_deref(),
			)
			.await
		}
		Commands::Runserver {
			address,
			noreload,
			watch_delay,
			no_wasm_rebuild,
			no_wasm,
			no_override_wasm,
			force_wasm,
			wasm_optional,
			insecure,
			no_docs,
			with_pages,
			static_dir,
			no_spa,
			index,
			package,
			features,
			all_features,
		} => {
			execute_runserver(RunServerOptions {
				address,
				noreload,
				watch_delay,
				no_wasm_rebuild,
				no_wasm,
				no_override_wasm,
				force_wasm,
				wasm_optional,
				insecure,
				no_docs,
				with_pages,
				static_dir,
				no_spa,
				index,
				package,
				features,
				all_features,
				verbosity,
			})
			.await
		}
		Commands::Shell { command } => execute_shell(command, verbosity, shell).await,
		Commands::Check { app_label, deploy } => execute_check(app_label, deploy, verbosity).await,
		Commands::Collectstatic {
			clear,
			no_input,
			dry_run,
			link,
			ignore,
			index,
			package,
			features,
			all_features,
		} => {
			execute_collectstatic(
				clear,
				no_input,
				dry_run,
				link,
				ignore,
				index,
				package,
				features,
				all_features,
				verbosity,
			)
			.await
		}
		Commands::Showurls { names } => execute_showurls(names, verbosity).await,
		#[cfg(feature = "introspect")]
		Commands::Introspect { format, section } => execute_introspect(format, section, verbosity).await,
		#[cfg(feature = "openapi")]
		Commands::Generateopenapi {
			format,
			output,
			postman,
		} => execute_generateopenapi(format, output, postman, verbosity).await,
		#[cfg(feature = "auth")]
		Commands::Createsuperuser {
			username,
			email,
			no_password,
			noinput,
			database,
		} => {
			crate::createsuperuser::execute_createsuperuser(
				username,
				email,
				no_password,
				noinput,
				database,
				verbosity,
			)
			.await
		}
		Commands::Custom { name, args } => {
			#[cfg(feature = "reinhardt-db")]
			if registry.get(&name).is_none()
				&& let Some(command) = parse_fixture_command(&name, &args)?
			{
				return execute_fixture_command(command, verbosity, settings).await;
			}
			execute_custom_command(&name, &args, verbosity, &registry).await
		}
	}
}

/// Rewrite `--verbosity=N` into clap's count-style verbosity flags.
///
/// This lets the standard parser reach its `InvalidSubcommand` path for a
/// custom command, while [`resolve_custom_command`] still reads the original
/// arguments and passes the requested numeric verbosity to that command.
fn normalize_count_style_verbosity_args(raw_args: &[String]) -> Vec<String> {
	let mut normalized = Vec::with_capacity(raw_args.len());
	for argument in raw_args {
		let Some(value) = argument.strip_prefix("--verbosity=") else {
			normalized.push(argument.clone());
			continue;
		};
		let Ok(count) = value.parse::<u8>() else {
			normalized.push(argument.clone());
			continue;
		};
		normalized.extend(std::iter::repeat_n("--verbosity".to_string(), count.into()));
	}
	normalized
}

/// Returns `true` when the clap error represents an unrecognised subcommand.
///
/// Only `InvalidSubcommand` is intercepted. `UnknownArgument` is intentionally
/// excluded because it fires for unknown flags/options (e.g. `--bogus-flag`)
/// which should still produce the normal clap error output.
fn is_unknown_subcommand(err: &clap::Error) -> bool {
	matches!(err.kind(), clap::error::ErrorKind::InvalidSubcommand)
}

/// Try to resolve raw CLI arguments into a custom command from the registry.
///
/// The convention is: `manage <subcommand> [args...]`.  Global flags that
/// appear before the subcommand (e.g., `-v`) are skipped. Both count-style
/// `--verbosity` and legacy value-style `--verbosity 2` are accepted.
fn resolve_custom_command(
	raw_args: &[String],
	registry: &CommandRegistry,
) -> Option<(String, Vec<String>, u8)> {
	let mut verbosity: u8 = 0;

	// Skip the binary name (argv[0]) and parse leading global flags.
	let mut iter = raw_args.iter().skip(1).peekable();
	while let Some(arg) = iter.peek() {
		if !arg.starts_with('-') {
			break;
		}
		let flag = iter.next().unwrap(); // safe: peeked above

		if flag == "-v" || flag == "--verbose" {
			verbosity = verbosity.saturating_add(1);
		} else if flag == "--verbosity" {
			if let Some(value) = iter.peek().and_then(|value| value.parse::<u8>().ok()) {
				verbosity = value;
				iter.next();
			} else {
				verbosity = verbosity.saturating_add(1);
			}
		} else if let Some(value) = flag.strip_prefix("--verbosity=") {
			verbosity = value.parse().ok()?;
		}
	}

	let subcommand = iter.next()?;
	if registry.get(subcommand).is_some() || is_fixture_command_name(subcommand) {
		let remaining: Vec<String> = iter.cloned().collect();
		Some((subcommand.clone(), remaining, verbosity))
	} else {
		None
	}
}

/// Execute a custom command looked up from the registry.
async fn execute_custom_command(
	name: &str,
	args: &[String],
	verbosity: u8,
	registry: &CommandRegistry,
) -> Result<(), Box<dyn std::error::Error>> {
	let cmd = registry.get(name).ok_or_else(|| {
		format!(
			"Custom command '{}' not found in registry.\nRegistered commands: {}",
			name,
			registry.list().join(", ")
		)
	})?;

	let mut ctx = CommandContext::default();
	ctx.set_verbosity(verbosity);
	for arg in args {
		ctx.add_arg(arg.clone());
	}

	cmd.execute(&ctx).await.map_err(|e| e.into())
}

#[cfg(feature = "reinhardt-db")]
async fn execute_fixture_command(
	command: FixtureCommand,
	verbosity: u8,
	settings: Option<Arc<dyn HasCommonSettings>>,
) -> Result<(), Box<dyn std::error::Error>> {
	match command {
		FixtureCommand::Dumpdata { selectors, exclude } => {
			crate::data_commands::execute_dumpdata(selectors, exclude).await
		}
		FixtureCommand::Loaddata { fixtures } => {
			crate::data_commands::execute_loaddata(fixtures).await
		}
		FixtureCommand::Seed { app_labels } => {
			crate::data_commands::execute_seed(app_labels, verbosity, settings).await
		}
	}
}

/// Execute the makemigrations command
#[cfg(feature = "migrations")]
// Allow too_many_arguments: CLI flags are mapped 1:1 to function parameters for clarity
#[allow(clippy::too_many_arguments)]
async fn execute_makemigrations(
	app_labels: Vec<String>,
	dry_run: bool,
	name: Option<String>,
	check: bool,
	empty: bool,
	merge: bool,
	force_empty_state: bool,
	verbosity: u8,
	settings: Option<Arc<dyn HasCommonSettings>>,
) -> Result<(), Box<dyn std::error::Error>> {
	let mut ctx = CommandContext::default();
	ctx.set_verbosity(verbosity);
	// Attach the project's composed settings so `makemigrations` can resolve the
	// database URL from `settings/*.toml` when no `--database`/`DATABASE_URL` is
	// provided (#5042). `makemigrations` does not flow through
	// `initialize_orm_database`, so it carries its own settings handle.
	if let Some(s) = settings {
		ctx = ctx.with_settings(s);
	}

	if !app_labels.is_empty() {
		for label in app_labels {
			ctx.add_arg(label);
		}
	}

	if dry_run {
		ctx.set_option("dry-run".to_string(), "true".to_string());
	}
	if check {
		ctx.set_option("check".to_string(), "true".to_string());
	}
	if empty {
		ctx.set_option("empty".to_string(), "true".to_string());
	}
	if merge {
		ctx.set_option("merge".to_string(), "true".to_string());
	}
	if force_empty_state {
		ctx.set_option("force-empty-state".to_string(), "true".to_string());
	}
	if let Some(n) = name {
		ctx.set_option("name".to_string(), n);
	}

	let cmd = MakeMigrationsCommand;
	cmd.execute(&ctx).await.map_err(|e| e.into())
}

/// Parameters for the migrate command
#[derive(Debug)]
struct MigrateParams {
	app_label: Option<String>,
	migration_name: Option<String>,
	database: Option<String>,
	fake: bool,
	fake_initial: bool,
	plan: bool,
	migrations_dir: Option<PathBuf>,
	verbosity: u8,
}

/// Execute the migrate command
async fn execute_migrate(params: MigrateParams) -> Result<(), Box<dyn std::error::Error>> {
	let mut ctx = CommandContext::default();
	ctx.set_verbosity(params.verbosity);

	if let Some(app) = params.app_label {
		ctx.add_arg(app);
		if let Some(migration) = params.migration_name {
			ctx.add_arg(migration);
		}
	}

	if params.fake {
		ctx.set_option("fake".to_string(), "true".to_string());
	}
	if params.fake_initial {
		ctx.set_option("fake-initial".to_string(), "true".to_string());
	}
	if params.plan {
		ctx.set_option("plan".to_string(), "true".to_string());
	}
	if let Some(db) = params.database {
		ctx.set_option("database".to_string(), db);
	}
	if let Some(dir) = params.migrations_dir {
		ctx.set_option(
			"migrations-dir".to_string(),
			dir.to_string_lossy().to_string(),
		);
	}

	let cmd = MigrateCommand;
	cmd.execute(&ctx).await.map_err(|e| e.into())
}

/// Options for the runserver command
struct RunServerOptions {
	address: String,
	noreload: bool,
	watch_delay: u64,
	no_wasm_rebuild: bool,
	no_wasm: bool,
	no_override_wasm: bool,
	force_wasm: bool,
	wasm_optional: bool,
	insecure: bool,
	no_docs: bool,
	with_pages: bool,
	static_dir: String,
	no_spa: bool,
	index: Option<String>,
	package: Option<String>,
	features: Vec<String>,
	all_features: bool,
	verbosity: u8,
}

fn runserver_context_from_options(options: &RunServerOptions) -> CommandContext {
	let mut ctx = CommandContext::default();
	ctx.set_verbosity(options.verbosity);
	ctx.add_arg(options.address.clone());
	ctx.set_option("watch-delay".to_string(), options.watch_delay.to_string());

	if options.noreload {
		ctx.set_option("noreload".to_string(), "true".to_string());
	}
	if options.no_wasm_rebuild {
		ctx.set_option("no-wasm-rebuild".to_string(), "true".to_string());
	}
	if options.no_wasm {
		ctx.set_option("no-wasm".to_string(), "true".to_string());
	}
	if options.no_override_wasm {
		ctx.set_option("no-override-wasm".to_string(), "true".to_string());
	}
	if options.force_wasm {
		ctx.set_option("force-wasm".to_string(), "true".to_string());
	}
	if options.wasm_optional {
		ctx.set_option("wasm-optional".to_string(), "true".to_string());
	}
	if options.insecure {
		ctx.set_option("insecure".to_string(), "true".to_string());
	}
	if options.no_docs {
		ctx.set_option("no_docs".to_string(), "true".to_string());
	}
	if options.with_pages {
		ctx.set_option("with-pages".to_string(), "true".to_string());
	}
	ctx.set_option("static-dir".to_string(), options.static_dir.clone());
	if options.no_spa {
		ctx.set_option("no-spa".to_string(), "true".to_string());
	}
	if let Some(ref index) = options.index {
		ctx.set_option("index".to_string(), index.clone());
	}
	if let Some(ref package) = options.package {
		ctx.set_option("package".to_string(), package.clone());
	}
	if !options.features.is_empty() {
		ctx.set_option("features".to_string(), options.features.join(","));
	}
	if options.all_features {
		ctx.set_option("all-features".to_string(), "true".to_string());
	}

	ctx
}

/// Execute the runserver command
async fn execute_runserver(options: RunServerOptions) -> Result<(), Box<dyn std::error::Error>> {
	let ctx = runserver_context_from_options(&options);
	let cmd = RunServerCommand;
	cmd.execute(&ctx).await.map_err(|e| e.into())
}

/// Execute the shell command
async fn execute_shell(
	command: Option<String>,
	verbosity: u8,
	shell: Option<ShellConfig>,
) -> Result<(), Box<dyn std::error::Error>> {
	let mut ctx = CommandContext::default();
	ctx.set_verbosity(verbosity);

	if let Some(cmd_str) = command {
		ctx.set_option("command".to_string(), cmd_str);
	}

	let cmd = shell.map(ShellCommand::new).unwrap_or_default();
	cmd.execute(&ctx).await.map_err(|e| e.into())
}

/// Execute the check command
async fn execute_check(
	app_label: Option<String>,
	deploy: bool,
	verbosity: u8,
) -> Result<(), Box<dyn std::error::Error>> {
	let mut ctx = CommandContext::default();
	ctx.set_verbosity(verbosity);

	if let Some(app) = app_label {
		ctx.add_arg(app);
	}

	if deploy {
		ctx.set_option("deploy".to_string(), "true".to_string());
	}

	let cmd = CheckCommand;
	cmd.execute(&ctx).await.map_err(|e| e.into())
}

/// Execute the collectstatic command
#[allow(clippy::too_many_arguments)] // The handler mirrors collectstatic's independent CLI options.
async fn execute_collectstatic(
	clear: bool,
	no_input: bool,
	dry_run: bool,
	link: bool,
	ignore: Vec<String>,
	index: Option<String>,
	package: Option<String>,
	features: Vec<String>,
	all_features: bool,
	verbosity: u8,
) -> Result<(), Box<dyn std::error::Error>> {
	// Load settings from TOML files
	let profile_str = env::var("REINHARDT_ENV").unwrap_or_else(|_| "local".to_string());
	let profile = Profile::parse(&profile_str);

	let base_dir =
		env::current_dir().map_err(|e| format!("Failed to get current directory: {e}"))?;
	let settings_dir = base_dir.join("settings");

	// Generate a random secret key for the default to avoid shipping a
	// hardcoded value that could be reused across deployments.
	let default_secret_key = generate_random_secret_key();

	let merged = SettingsBuilder::new()
		.profile(profile)
		.add_source(
			DefaultSource::new()
				.with_value(
					"base_dir",
					Value::String(
						base_dir
							.to_str()
							.ok_or_else(|| {
								format!("base_dir contains invalid UTF-8: {}", base_dir.display())
							})?
							.to_string(),
					),
				)
				.with_value("debug", Value::Bool(true))
				.with_value("secret_key", Value::String(default_secret_key))
				.with_value("allowed_hosts", Value::Array(vec![]))
				.with_value("installed_apps", Value::Array(vec![]))
				.with_value("databases", serde_json::json!({}))
				.with_value("templates", Value::Array(vec![]))
				.with_value("static_url", Value::String("/static/".to_string()))
				.with_value(
					"static_root",
					Value::String(base_dir.join("staticfiles").to_string_lossy().to_string()),
				)
				.with_value("staticfiles_dirs", Value::Array(vec![]))
				.with_value("media_url", Value::String("/media/".to_string()))
				.with_value("language_code", Value::String("en-us".to_string()))
				.with_value("time_zone", Value::String("UTC".to_string()))
				.with_value("use_i18n", Value::Bool(false))
				.with_value("use_tz", Value::Bool(false))
				.with_value(
					"default_auto_field",
					Value::String("reinhardt.db.models.BigAutoField".to_string()),
				)
				.with_value("secure_ssl_redirect", Value::Bool(false))
				.with_value("secure_hsts_include_subdomains", Value::Bool(false))
				.with_value("secure_hsts_preload", Value::Bool(false))
				.with_value("session_cookie_secure", Value::Bool(false))
				.with_value("csrf_cookie_secure", Value::Bool(false))
				.with_value("append_slash", Value::Bool(false))
				// Middleware
				.with_value("middleware", Value::Array(vec![]))
				// URL configuration
				.with_value("root_urlconf", Value::String(String::new()))
				// Media files
				.with_value("media_root", Value::Null)
				// Admin/Manager contacts
				.with_value("admins", Value::Array(vec![]))
				.with_value("managers", Value::Array(vec![])),
		)
		.add_source(LowPriorityEnvSource::new().with_prefix("REINHARDT_"))
		.add_source(TomlFileSource::new(settings_dir.join("base.toml")))
		.add_source(TomlFileSource::new(
			settings_dir.join(format!("{}.toml", profile_str)),
		))
		.build()?;

	let static_settings = crate::StaticAssetSettings::from_merged(&merged, &base_dir);
	let config = StaticFilesConfig {
		static_root: static_settings.static_root,
		static_url: static_settings.static_url,
		staticfiles_dirs: static_settings.staticfiles_dirs,
		media_url: None,
	};

	// Create options
	let options = CollectStaticOptions {
		clear,
		no_input,
		dry_run,
		interactive: !no_input,
		link,
		ignore_patterns: ignore,
		verbosity,
		enable_hashing: true,
		fast_compare: false,
	};

	// Resolve index source path
	// Refs #2869: Auto-detect index.html from project root for collectstatic
	let index_source = match &index {
		Some(path) => Some(PathBuf::from(path)),
		None => {
			// Auto-detect from project root
			let candidate = base_dir.join("index.html");
			if candidate.exists() {
				Some(candidate)
			} else {
				None
			}
		}
	};

	// Create and execute command in blocking context
	let mut cmd = CollectStaticCommand::new(config, options);
	cmd.set_index_source(index_source);
	let feature_selection = if all_features {
		crate::StyleFeatureSelection::all_features()
	} else {
		crate::StyleFeatureSelection::with_features(features)
	};
	let style_context = resolve_collectstatic_style_context(
		&base_dir.join("Cargo.toml"),
		package.as_deref(),
		feature_selection,
	)
	.map_err(|error| format!("failed to select component style package: {error}"))?;
	cmd.set_style_context(style_context);
	let result = tokio::task::spawn_blocking(move || {
		// Call the sync execute() method directly (not the BaseCommand trait method)
		CollectStaticCommand::execute(&mut cmd)
	})
	.await;

	match result {
		Ok(Ok(_stats)) => Ok(()),
		Ok(Err(e)) => Err(Box::new(e) as Box<dyn std::error::Error>),
		Err(e) => Err(Box::new(e) as Box<dyn std::error::Error>),
	}
}

fn resolve_collectstatic_style_context(
	manifest_path: &Path,
	requested_package: Option<&str>,
	feature_selection: crate::StyleFeatureSelection,
) -> Result<Option<crate::StylePackageContext>, String> {
	match crate::StylePackageContext::resolve_with_features(
		manifest_path,
		requested_package,
		feature_selection,
	) {
		Ok(context) => Ok(Some(context)),
		Err(error)
			if requested_package.is_none() && virtual_workspace_has_no_style_package(&error) =>
		{
			Ok(None)
		}
		Err(error) => Err(error),
	}
}

fn virtual_workspace_has_no_style_package(error: &str) -> bool {
	error == "the Cargo workspace has no root package; pass --package <NAME>"
		|| (error.contains("manifest is virtual") && error.contains("workspace has no members"))
}

/// Execute the showurls command
#[cfg(feature = "routers")]
async fn execute_showurls(names: bool, verbosity: u8) -> Result<(), Box<dyn std::error::Error>> {
	let mut ctx = CommandContext::default();
	ctx.set_verbosity(verbosity);

	if names {
		ctx.set_option("names".to_string(), "true".to_string());
	}

	let cmd = ShowUrlsCommand;
	cmd.execute(&ctx).await.map_err(|e| e.into())
}

#[cfg(not(feature = "routers"))]
async fn execute_showurls(_names: bool, _verbosity: u8) -> Result<(), Box<dyn std::error::Error>> {
	Err("showurls command requires 'routers' feature. \
		Enable it in your Cargo.toml: \
		reinhardt-commands = { version = \"0.1.0\", features = [\"routers\"] }"
		.into())
}

/// Execute the introspect command
#[cfg(feature = "introspect")]
async fn execute_introspect(
	format: OutputFormat,
	section: Option<String>,
	verbosity: u8,
) -> Result<(), Box<dyn std::error::Error>> {
	use crate::introspect::{collect_introspect_data, format_json, format_yaml};
	use colored::Colorize;

	if verbosity > 0 {
		eprintln!("{}", "Collecting project metadata...".cyan().bold());
	}

	let output = collect_introspect_data()?;

	// If a section filter is specified, extract just that section
	let content = if let Some(ref section_name) = section {
		let valid_sections = [
			"app",
			"databases",
			"routes",
			"middleware",
			"settings",
			"features",
		];
		if !valid_sections.contains(&section_name.as_str()) {
			return Err(format!(
				"Invalid section '{}'. Valid sections: {}",
				section_name,
				valid_sections.join(", ")
			)
			.into());
		}

		// Serialize to serde_json::Value, then extract the section
		let full_value = serde_json::to_value(&output)?;
		let section_value = full_value
			.get(section_name)
			.ok_or_else(|| format!("Section '{}' not found in output", section_name))?;

		match format {
			OutputFormat::Json => serde_json::to_string_pretty(section_value)?,
			OutputFormat::Yaml => serde_yaml::to_string(section_value)?,
		}
	} else {
		match format {
			OutputFormat::Json => format_json(&output)?,
			OutputFormat::Yaml => format_yaml(&output)?,
		}
	};

	println!("{}", content);

	Ok(())
}

// Stub when introspect feature is disabled — not reachable because the
// Commands::Introspect variant is also feature-gated, but keeps the match arm
// exhaustive for non-introspect builds that might add a fallback.

/// Execute the generateopenapi command
#[cfg(feature = "openapi")]
async fn execute_generateopenapi(
	format: String,
	output: PathBuf,
	postman: bool,
	verbosity: u8,
) -> Result<(), Box<dyn std::error::Error>> {
	use colored::Colorize;

	if verbosity > 0 {
		println!("{}", "Generating OpenAPI schema...".cyan().bold());
	}

	// Create SchemaGenerator
	let generator = reinhardt_rest::openapi::SchemaGenerator::new()
		.title(env::var("OPENAPI_TITLE").unwrap_or_else(|_| "API Documentation".to_string()))
		.version(env::var("OPENAPI_VERSION").unwrap_or_else(|_| "1.0.0".to_string()))
		.description(env::var("OPENAPI_DESCRIPTION").unwrap_or_default())
		.add_function_based_endpoints();

	// Generate content based on format
	let content = match format.as_str() {
		"yaml" | "yml" => generator.to_yaml()?,
		_ => generator.to_json()?,
	};

	// Write to file
	std::fs::write(&output, content)?;

	if verbosity > 0 {
		println!(
			"{} {}",
			"OpenAPI schema generated:".green().bold(),
			output.display()
		);
	}

	// Generate Postman Collection if requested
	if postman {
		let postman_output = output.with_extension("postman.json");

		if verbosity > 0 {
			println!("{}", "Generating Postman Collection...".cyan().bold());
		}

		// Use npx openapi-to-postmanv2 to convert
		let status = std::process::Command::new("npx")
			.args([
				"openapi-to-postmanv2",
				"-s",
				output.to_str().unwrap(),
				"-o",
				postman_output.to_str().unwrap(),
				"-p",
			])
			.status()?;

		if !status.success() {
			return Err("Failed to generate Postman Collection. \
				Make sure Node.js and npx are installed: \
				npm install -g openapi-to-postmanv2"
				.into());
		}

		if verbosity > 0 {
			println!(
				"{} {}",
				"Postman Collection generated:".green().bold(),
				postman_output.display()
			);
		}
	}

	Ok(())
}

#[cfg(not(feature = "openapi"))]
// Allow dead_code: stub entry point when openapi feature is disabled
#[allow(dead_code)]
async fn execute_generateopenapi(
	_format: String,
	_output: PathBuf,
	_postman: bool,
	_verbosity: u8,
) -> Result<(), Box<dyn std::error::Error>> {
	Err("generateopenapi command requires 'openapi' feature. \
		Enable it in your Cargo.toml: \
		reinhardt-commands = { version = \"0.1.0\", features = [\"openapi\"] }"
		.into())
}

// ============================================================================
// Automatic Router Registration
// ============================================================================

/// Automatically discover and register URL pattern functions.
///
/// This function uses the `inventory` crate to discover URL pattern functions
/// that were registered at compile time using the `#[routes]` attribute macro,
/// then installs the resulting router into the global router slot consumed by
/// [`RunServerCommand`].
///
/// [`execute_from_command_line`] calls this internally for HTTP-serving
/// subcommands, so most applications never need to invoke it directly. It is
/// exposed as a public building block for **non-CLI server entrypoints** —
/// for example, a container entrypoint binary that calls
/// [`RunServerCommand::execute`] directly without
/// going through clap argument parsing.
///
/// For the common "just start the HTTP server" case, prefer the higher-level
/// [`start_server`] helper which wraps this function and `RunServerCommand`.
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if:
/// - No URL patterns were registered (no `#[routes]` function was reachable
///   from the linked binary)
/// - Multiple `#[routes]` functions were detected (should normally be caught
///   at link time)
///
/// # Examples
///
/// Compose with [`RunServerCommand`] directly when
/// you need control beyond what [`start_server`] offers:
///
/// ```rust,no_run
/// use reinhardt_commands::{auto_register_router, BaseCommand, CommandContext, RunServerCommand};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     auto_register_router().await?;
///
///     let mut ctx = CommandContext::new(vec!["0.0.0.0:8080".to_string()]);
///     ctx.set_option("noreload".to_string(), "true".to_string());
///
///     RunServerCommand.execute(&ctx).await?;
///     Ok(())
/// }
/// ```
#[cfg(feature = "routers")]
pub async fn auto_register_router() -> Result<(), Box<dyn std::error::Error>> {
	// Thin delegation to the named consumer on `RunServerCommand`
	// (Refs #4453). The actual inventory iteration, multi/empty
	// validation, and global registration live in
	// `RunServerCommand::register_http_routes_from_inventory(..)` so the
	// `runserver` command body can call them at a visible call site.
	//
	// This function is preserved as a `pub` API for backward
	// compatibility with non-`runserver` HTTP-serving commands
	// (`showurls`, `generateopenapi`, `introspect`) whose own explicit
	// `register_*_from_inventory()` plumbing is tracked separately.
	//
	// Short-circuit if a router was already registered manually via
	// `reinhardt_urls::routers::register_router(..)`. Without this guard,
	// `register_http_routes_from_inventory()`'s DP-7 mutual-exclusion
	// check would turn the pre-registered router (a reusable manual
	// escape hatch) into a hard error on `showurls`/`introspect`/
	// `generateopenapi` and `start_server()`. The explicit
	// `RunServerCommand::register_http_routes_from_inventory()` entry
	// keeps the strict guard for its single direct call site.
	if reinhardt_urls::routers::is_router_registered() {
		return Ok(());
	}
	crate::RunServerCommand
		.register_http_routes_from_inventory()
		.await
}

/// No-op implementation when the `routers` feature is disabled.
///
/// Kept public to preserve API stability across feature-flag toggles: callers
/// of [`auto_register_router`] should compile regardless of whether `routers`
/// is enabled in the consuming crate.
#[cfg(not(feature = "routers"))]
pub async fn auto_register_router() -> Result<(), Box<dyn std::error::Error>> {
	// No router registration needed when routers feature is disabled
	Ok(())
}

/// Start the HTTP server bound to `addr`.
///
/// This is a one-call convenience wrapper around [`RunServerCommand`] for
/// **non-CLI server entrypoints** — for example, a container entrypoint
/// binary that should expose only an HTTP server without the full `manage`
/// clap surface.
///
/// HTTP-route inventory registration happens inside
/// [`RunServerCommand::execute`] itself (Refs #4453 DP-1), guarded by
/// `reinhardt_urls::routers::is_router_registered()`. This wrapper therefore
/// delegates straight to `execute` without a separate
/// [`auto_register_router`] call — wiring both would double-register on
/// callers that do not pre-mount a router and would no-op redundantly on
/// callers that do.
///
/// All [`RunServerCommand`] options other than the bind address use their
/// built-in defaults (autoreload enabled, no WASM frontend, `dist` static
/// directory, etc.). Callers needing finer control should construct a
/// [`CommandContext`] and call [`RunServerCommand::execute`] directly.
///
/// Use [`execute_from_command_line`] instead when you want full clap argument
/// parsing for the `manage` subcommand surface.
///
/// # Arguments
///
/// * `addr` - Bind address in `host:port` form (e.g. `"0.0.0.0:8080"`).
///
/// # Returns
///
/// Returns `Ok(())` on graceful shutdown, or an error if route registration
/// or the server itself fails.
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_commands::start_server;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     start_server("0.0.0.0:8080").await
/// }
/// ```
#[cfg(feature = "server")]
pub async fn start_server(addr: &str) -> Result<(), Box<dyn std::error::Error>> {
	let ctx = CommandContext::new(vec![addr.to_string()]);
	RunServerCommand.execute(&ctx).await.map_err(Into::into)
}

/// Generate a cryptographically random secret key for fallback use.
///
/// Produces a 50-character hex string (200 bits of entropy). This is used
/// as the default `SECRET_KEY` when no explicit key is configured, ensuring
/// that each process gets a unique key rather than a shared hardcoded value.
pub(crate) fn generate_random_secret_key() -> String {
	use rand::Rng;
	use std::fmt::Write;

	let mut rng = rand::rng();
	let bytes: [u8; 25] = rng.random();
	let mut hex_string = String::with_capacity(50);
	for b in bytes {
		let _ = write!(hex_string, "{:02x}", b);
	}
	hex_string
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	#[cfg(feature = "reinhardt-db")]
	use std::sync::atomic::{AtomicBool, Ordering};

	#[cfg(feature = "reinhardt-db")]
	struct RegisteredFixtureNameCommand;

	#[cfg(feature = "reinhardt-db")]
	static REGISTERED_FIXTURE_NAME_COMMAND_EXECUTED: AtomicBool = AtomicBool::new(false);

	#[cfg(feature = "reinhardt-db")]
	#[async_trait::async_trait]
	impl BaseCommand for RegisteredFixtureNameCommand {
		fn name(&self) -> &str {
			"seed"
		}

		async fn execute(&self, _ctx: &CommandContext) -> crate::CommandResult<()> {
			REGISTERED_FIXTURE_NAME_COMMAND_EXECUTED.store(true, Ordering::SeqCst);
			Ok(())
		}
	}

	/// `Runserver` is intentionally **not** in the pre-dispatch
	/// `requires_router` list (Refs #4453): the HTTP-route inventory
	/// pull now happens inside
	/// [`RunServerCommand::register_http_routes_from_inventory`] at a
	/// visible call site, so the dispatcher must **not** auto-register
	/// a router for `runserver` ahead of time.
	#[rstest]
	fn test_runserver_does_not_require_pre_dispatch_router_registration() {
		// Arrange
		let command = Commands::Runserver {
			address: "127.0.0.1:8000".to_string(),
			noreload: false,
			watch_delay: 120,
			no_wasm_rebuild: false,
			no_wasm: false,
			no_override_wasm: false,
			force_wasm: false,
			wasm_optional: false,
			insecure: false,
			no_docs: false,
			with_pages: false,
			static_dir: "dist".to_string(),
			no_spa: false,
			index: None,
			package: None,
			features: vec![],
			all_features: false,
		};

		// Act
		let result = requires_router(&command);

		// Assert
		assert!(!result);
	}

	#[cfg(feature = "routers")]
	#[rstest]
	fn test_requires_router_for_showurls() {
		// Arrange
		let command = Commands::Showurls { names: false };

		// Act
		let result = requires_router(&command);

		// Assert
		assert!(result);
	}

	#[cfg(feature = "openapi")]
	#[rstest]
	fn test_requires_router_for_generateopenapi() {
		// Arrange
		let command = Commands::Generateopenapi {
			format: "json".to_string(),
			output: std::path::PathBuf::from("openapi.json"),
			postman: false,
		};

		// Act
		let result = requires_router(&command);

		// Assert
		assert!(result);
	}

	#[rstest]
	fn test_does_not_require_router_for_migrate() {
		// Arrange
		let command = Commands::Migrate {
			app_label: None,
			migration_name: None,
			database: None,
			fake: false,
			fake_initial: false,
			plan: false,
			migrations_dir: None,
		};

		// Act
		let result = requires_router(&command);

		// Assert
		assert!(!result);
	}

	#[rstest]
	fn test_does_not_require_router_for_shell() {
		// Arrange
		let command = Commands::Shell { command: None };

		// Act
		let result = requires_router(&command);

		// Assert
		assert!(!result);
	}

	#[rstest]
	fn test_does_not_require_router_for_check() {
		// Arrange
		let command = Commands::Check {
			app_label: None,
			deploy: false,
		};

		// Act
		let result = requires_router(&command);

		// Assert
		assert!(!result);
	}

	#[rstest]
	fn test_does_not_require_router_for_collectstatic() {
		// Arrange
		let command = Commands::Collectstatic {
			clear: false,
			no_input: false,
			dry_run: false,
			link: false,
			ignore: vec![],
			index: None,
			package: None,
			features: vec![],
			all_features: false,
		};

		// Act
		let result = requires_router(&command);

		// Assert
		assert!(!result);
	}

	#[cfg(feature = "migrations")]
	#[rstest]
	fn test_does_not_require_router_for_makemigrations() {
		// Arrange
		let command = Commands::Makemigrations {
			app_labels: vec![],
			dry_run: false,
			name: None,
			check: false,
			empty: false,
			merge: false,
			force_empty_state: false,
			migration_dir: std::path::PathBuf::from("./migrations"),
		};

		// Act
		let result = requires_router(&command);

		// Assert
		assert!(!result);
	}

	#[cfg(feature = "introspect")]
	#[rstest]
	fn test_requires_router_for_introspect() {
		// Arrange
		let command = Commands::Introspect {
			format: OutputFormat::Yaml,
			section: None,
		};

		// Act
		let result = requires_router(&command);

		// Assert
		assert!(result);
	}

	#[cfg(feature = "routers")]
	#[rstest]
	#[tokio::test]
	async fn test_auto_register_router_returns_error_with_lib_bin_hint_when_no_routes() {
		// Arrange: no #[routes] registered in test binary
		// (test binaries do not include application inventory::submit! side effects)

		// Act
		let result = auto_register_router().await;

		// Assert: must fail because no routes are registered
		assert!(
			result.is_err(),
			"Expected error when no routes are registered"
		);
		let error_msg = result.unwrap_err().to_string();
		assert!(
			error_msg.contains("No URL patterns registered"),
			"Expected 'No URL patterns registered' in error, got: {}",
			error_msg
		);
		assert!(
			error_msg.contains("library/binary split"),
			"Expected lib+bin hint in error message, got: {}",
			error_msg
		);
	}

	#[rstest]
	fn test_runserver_with_index_option() {
		// Arrange
		let command = Commands::Runserver {
			address: "127.0.0.1:8000".to_string(),
			noreload: false,
			watch_delay: 120,
			no_wasm_rebuild: false,
			no_wasm: false,
			no_override_wasm: false,
			force_wasm: false,
			wasm_optional: false,
			insecure: false,
			no_docs: false,
			with_pages: true,
			static_dir: "dist".to_string(),
			no_spa: false,
			index: Some("./index.html".to_string()),
			package: None,
			features: vec![],
			all_features: false,
		};

		// Act & Assert
		if let Commands::Runserver { index, .. } = command {
			assert_eq!(index, Some("./index.html".to_string()));
		} else {
			panic!("Expected Runserver command");
		}
	}

	#[rstest]
	fn test_runserver_without_index_option() {
		// Arrange
		let command = Commands::Runserver {
			address: "127.0.0.1:8000".to_string(),
			noreload: false,
			watch_delay: 120,
			no_wasm_rebuild: false,
			no_wasm: false,
			no_override_wasm: false,
			force_wasm: false,
			wasm_optional: false,
			insecure: false,
			no_docs: false,
			with_pages: false,
			static_dir: "dist".to_string(),
			no_spa: false,
			index: None,
			package: None,
			features: vec![],
			all_features: false,
		};

		// Act & Assert
		if let Commands::Runserver { index, .. } = command {
			assert!(index.is_none());
		} else {
			panic!("Expected Runserver command");
		}
	}

	#[rstest]
	fn test_runserver_index_with_no_spa() {
		// Arrange & Act
		let command = Commands::Runserver {
			address: "127.0.0.1:8000".to_string(),
			noreload: false,
			watch_delay: 120,
			no_wasm_rebuild: false,
			no_wasm: false,
			no_override_wasm: false,
			force_wasm: false,
			wasm_optional: false,
			insecure: false,
			no_docs: false,
			with_pages: true,
			static_dir: "dist".to_string(),
			no_spa: true,
			index: Some("./index.html".to_string()),
			package: None,
			features: vec![],
			all_features: false,
		};

		// Assert
		if let Commands::Runserver { no_spa, index, .. } = command {
			assert!(no_spa);
			assert_eq!(index, Some("./index.html".to_string()));
		} else {
			panic!("Expected Runserver command");
		}
	}

	#[rstest]
	fn test_runserver_index_without_with_pages() {
		// Arrange & Act
		let command = Commands::Runserver {
			address: "127.0.0.1:8000".to_string(),
			noreload: false,
			watch_delay: 120,
			no_wasm_rebuild: false,
			no_wasm: false,
			no_override_wasm: false,
			force_wasm: false,
			wasm_optional: false,
			insecure: false,
			no_docs: false,
			with_pages: false,
			static_dir: "dist".to_string(),
			no_spa: false,
			index: Some("./index.html".to_string()),
			package: None,
			features: vec![],
			all_features: false,
		};

		// Assert
		if let Commands::Runserver {
			with_pages, index, ..
		} = command
		{
			assert!(!with_pages);
			assert_eq!(index, Some("./index.html".to_string()));
		} else {
			panic!("Expected Runserver command");
		}
	}

	#[rstest]
	fn test_runserver_with_no_wasm_rebuild_flag() {
		// Arrange: build options as the CLI parser would after `--no-wasm-rebuild`
		let options = RunServerOptions {
			address: "127.0.0.1:8000".to_string(),
			noreload: false,
			watch_delay: 120,
			no_wasm_rebuild: true,
			no_wasm: false,
			no_override_wasm: false,
			force_wasm: false,
			wasm_optional: false,
			insecure: false,
			no_docs: false,
			with_pages: false,
			static_dir: "dist".to_string(),
			no_spa: false,
			index: None,
			package: None,
			features: vec![],
			all_features: false,
			verbosity: 0,
		};

		// Act
		let ctx = runserver_context_from_options(&options);

		// Assert
		assert_eq!(ctx.option("no-wasm-rebuild"), Some(&"true".to_string()));
		assert_eq!(ctx.option("watch-delay"), Some(&"120".to_string()));
	}

	#[rstest]
	fn test_runserver_no_override_wasm_flag_propagates() {
		// Arrange: build options as the CLI parser would after `--no-override-wasm`
		let options = RunServerOptions {
			address: "127.0.0.1:8000".to_string(),
			noreload: false,
			watch_delay: 120,
			no_wasm_rebuild: false,
			no_wasm: false,
			no_override_wasm: true,
			force_wasm: false,
			wasm_optional: false,
			insecure: false,
			no_docs: false,
			with_pages: true,
			static_dir: "dist".to_string(),
			no_spa: false,
			index: None,
			package: None,
			features: vec![],
			all_features: false,
			verbosity: 0,
		};

		// Act
		let ctx = runserver_context_from_options(&options);

		// Assert
		assert_eq!(ctx.option("no-override-wasm"), Some(&"true".to_string()));
		assert_eq!(ctx.option("with-pages"), Some(&"true".to_string()));
	}

	#[rstest]
	fn test_runserver_force_wasm_legacy_flag_propagates() {
		// Arrange: legacy `--force-wasm` is still parsed but emits a deprecation
		// warning at runtime; the CommandContext key is preserved so RunServerCommand
		// can detect it and emit the warning.
		let options = RunServerOptions {
			address: "127.0.0.1:8000".to_string(),
			noreload: false,
			watch_delay: 120,
			no_wasm_rebuild: false,
			no_wasm: false,
			no_override_wasm: false,
			force_wasm: true,
			wasm_optional: false,
			insecure: false,
			no_docs: false,
			with_pages: true,
			static_dir: "dist".to_string(),
			no_spa: false,
			index: None,
			package: None,
			features: vec![],
			all_features: false,
			verbosity: 0,
		};

		// Act
		let ctx = runserver_context_from_options(&options);

		// Assert
		assert_eq!(ctx.option("force-wasm"), Some(&"true".to_string()));
	}

	#[rstest]
	fn test_runserver_watch_delay_option_propagates() {
		// Arrange
		let options = RunServerOptions {
			address: "127.0.0.1:8000".to_string(),
			noreload: false,
			watch_delay: 75,
			no_wasm_rebuild: false,
			no_wasm: false,
			no_override_wasm: false,
			force_wasm: false,
			wasm_optional: false,
			insecure: false,
			no_docs: false,
			with_pages: true,
			static_dir: "dist".to_string(),
			no_spa: false,
			index: None,
			package: None,
			features: vec![],
			all_features: false,
			verbosity: 0,
		};

		// Act
		let ctx = runserver_context_from_options(&options);

		// Assert
		assert_eq!(ctx.option("watch-delay"), Some(&"75".to_string()));
	}

	#[rstest]
	fn test_runserver_clap_accepts_no_override_wasm() {
		use clap::Parser;

		// Arrange & Act: clap parsing must accept the new flag.
		let cli = Cli::parse_from(["manage", "runserver", "--with-pages", "--no-override-wasm"]);

		// Assert
		match cli.command {
			Commands::Runserver {
				with_pages,
				watch_delay,
				no_override_wasm,
				force_wasm,
				..
			} => {
				assert!(with_pages, "--with-pages should be parsed");
				assert_eq!(watch_delay, 120, "--watch-delay should default to 120 ms");
				assert!(no_override_wasm, "--no-override-wasm should be parsed");
				assert!(!force_wasm, "--force-wasm was not provided");
			}
			#[allow(unreachable_patterns)]
			_ => panic!("Expected Commands::Runserver"),
		}
	}

	#[rstest]
	fn test_runserver_clap_accepts_watch_delay() {
		use clap::Parser;

		// Arrange & Act
		let cli = Cli::parse_from(["manage", "runserver", "--watch-delay", "75"]);

		// Assert
		match cli.command {
			Commands::Runserver { watch_delay, .. } => {
				assert_eq!(watch_delay, 75, "--watch-delay should be parsed");
			}
			#[allow(unreachable_patterns)]
			_ => panic!("Expected Commands::Runserver"),
		}
	}

	#[rstest]
	fn test_runserver_clap_accepts_force_wasm_legacy() {
		use clap::Parser;

		// Arrange & Act: legacy `--force-wasm` must still parse for back-compat.
		let cli = Cli::parse_from(["manage", "runserver", "--with-pages", "--force-wasm"]);

		// Assert
		match cli.command {
			Commands::Runserver {
				force_wasm,
				no_override_wasm,
				..
			} => {
				assert!(force_wasm, "--force-wasm should still parse (deprecated)");
				assert!(!no_override_wasm, "--no-override-wasm was not provided");
			}
			#[allow(unreachable_patterns)]
			_ => panic!("Expected Commands::Runserver"),
		}
	}

	#[cfg(feature = "reinhardt-db")]
	#[rstest]
	fn test_fixture_commands_parse_without_public_command_variants() {
		let command = parse_fixture_command(
			"dumpdata",
			&[
				"writing_sources.WritingProject".to_string(),
				"auth".to_string(),
				"--exclude".to_string(),
				"sessions.Session".to_string(),
			],
		)
		.expect("dumpdata arguments must parse")
		.expect("dumpdata must be recognized as a built-in fixture command");

		match command {
			FixtureCommand::Dumpdata { selectors, exclude } => {
				assert_eq!(
					selectors,
					vec![
						"writing_sources.WritingProject".to_string(),
						"auth".to_string()
					]
				);
				assert_eq!(exclude, vec!["sessions.Session".to_string()]);
			}
			_ => panic!("Expected FixtureCommand::Dumpdata"),
		}

		let registry = CommandRegistry::new();
		let resolved = resolve_custom_command(
			&[
				"manage".to_string(),
				"dumpdata".to_string(),
				"writing_sources.WritingProject".to_string(),
			],
			&registry,
		);

		assert_eq!(
			resolved,
			Some((
				"dumpdata".to_string(),
				vec!["writing_sources.WritingProject".to_string()],
				0,
			))
		);
	}

	#[cfg(feature = "reinhardt-db")]
	#[rstest]
	fn test_fixture_commands_resolve_after_count_style_verbosity() {
		let registry = CommandRegistry::new();

		for fixture_command in ["dumpdata", "seed"] {
			let resolved = resolve_custom_command(
				&[
					"manage".to_string(),
					"--verbosity".to_string(),
					fixture_command.to_string(),
				],
				&registry,
			);

			assert_eq!(resolved, Some((fixture_command.to_string(), Vec::new(), 1)));
		}
	}

	#[cfg(feature = "reinhardt-db")]
	#[rstest]
	fn test_fixture_commands_resolve_after_value_style_verbosity() {
		let registry = CommandRegistry::new();

		for args in [
			vec!["--verbosity".to_string(), "2".to_string()],
			vec!["--verbosity=2".to_string()],
		] {
			let mut raw_args = vec!["manage".to_string()];
			raw_args.extend(args);
			raw_args.push("dumpdata".to_string());
			assert_eq!(
				resolve_custom_command(&raw_args, &registry),
				Some(("dumpdata".to_string(), Vec::new(), 2))
			);
		}
	}

	#[cfg(feature = "reinhardt-db")]
	#[rstest]
	fn test_value_style_verbosity_reaches_the_custom_command_fallback() {
		let registry = CommandRegistry::new();
		let raw_args = vec![
			"manage".to_string(),
			"--verbosity=2".to_string(),
			"seed".to_string(),
		];

		let normalized_args = normalize_count_style_verbosity_args(&raw_args);
		let clap_error = Cli::try_parse_from(&normalized_args)
			.expect_err("fixture subcommands must remain eligible for custom resolution");

		assert!(is_unknown_subcommand(&clap_error));
		assert_eq!(
			resolve_custom_command(&raw_args, &registry),
			Some(("seed".to_string(), Vec::new(), 2))
		);
	}

	#[cfg(feature = "reinhardt-db")]
	#[tokio::test]
	async fn test_registered_fixture_name_dispatches_before_fixture_commands() {
		REGISTERED_FIXTURE_NAME_COMMAND_EXECUTED.store(false, Ordering::SeqCst);
		let mut registry = CommandRegistry::new();
		registry.register(Box::new(RegisteredFixtureNameCommand));
		let command = Commands::Custom {
			name: "seed".to_string(),
			args: vec!["project".to_string()],
		};

		assert!(!requires_database(&command, &registry));
		run_command_with_registry(command, 0, registry)
			.await
			.expect("registered fixture-named commands must dispatch through the registry");
		assert!(REGISTERED_FIXTURE_NAME_COMMAND_EXECUTED.load(Ordering::SeqCst));
	}

	#[cfg(feature = "reinhardt-db")]
	#[rstest]
	fn test_loaddata_fixture_parser_accepts_paths() {
		let command = parse_fixture_command("loaddata", &["fixtures/dev.json".to_string()])
			.expect("loaddata arguments must parse")
			.expect("loaddata must be recognized as a built-in fixture command");

		match command {
			FixtureCommand::Loaddata { fixtures } => {
				assert_eq!(
					fixtures,
					vec![std::path::PathBuf::from("fixtures/dev.json")]
				);
			}
			_ => panic!("Expected FixtureCommand::Loaddata"),
		}
	}

	#[cfg(feature = "reinhardt-db")]
	#[rstest]
	fn test_seed_fixture_parser_accepts_app_labels() {
		let command =
			parse_fixture_command("seed", &["writing_sources".to_string(), "auth".to_string()])
				.expect("seed arguments must parse")
				.expect("seed must be recognized as a built-in fixture command");

		match command {
			FixtureCommand::Seed { app_labels } => {
				assert_eq!(
					app_labels,
					vec!["writing_sources".to_string(), "auth".to_string()]
				);
			}
			_ => panic!("Expected FixtureCommand::Seed"),
		}
	}

	#[rstest]
	fn test_collectstatic_with_index_option() {
		// Arrange & Act
		let command = Commands::Collectstatic {
			clear: false,
			no_input: false,
			dry_run: false,
			link: false,
			ignore: vec![],
			index: Some("./index.html".to_string()),
			package: None,
			features: vec![],
			all_features: false,
		};

		// Assert
		if let Commands::Collectstatic { index, .. } = command {
			assert_eq!(index, Some("./index.html".to_string()));
		} else {
			panic!("Expected Collectstatic command");
		}
	}

	#[rstest]
	fn collectstatic_package_option_parses() {
		let cli = Cli::try_parse_from(["manage", "collectstatic", "--package", "poll-app"])
			.expect("collectstatic package option should parse");

		match cli.command {
			Commands::Collectstatic { package, .. } => {
				assert_eq!(package.as_deref(), Some("poll-app"));
			}
			_ => panic!("expected collectstatic command"),
		}
	}

	#[rstest]
	fn collectstatic_style_feature_options_parse() {
		let cli = Cli::try_parse_from(["manage", "collectstatic", "--features", "theme,brand"])
			.expect("collectstatic style features should parse");

		let Commands::Collectstatic {
			features,
			all_features,
			..
		} = cli.command
		else {
			panic!("expected collectstatic command");
		};
		assert_eq!(features, ["theme", "brand"]);
		assert!(!all_features);
	}

	#[rstest]
	fn collectstatic_without_package_allows_a_virtual_workspace_root() {
		let directory = tempfile::tempdir().expect("create temporary workspace");
		let manifest_path = directory.path().join("Cargo.toml");
		std::fs::write(
			&manifest_path,
			"[workspace]\nmembers = []\nresolver = \"3\"\n",
		)
		.expect("write virtual workspace manifest");

		let context = resolve_collectstatic_style_context(
			&manifest_path,
			None,
			crate::StyleFeatureSelection::default(),
		)
		.expect("a virtual workspace has no component stylesheet package by default");

		assert!(context.is_none());
	}

	#[rstest]
	fn runserver_package_option_parses_and_forwards() {
		let cli = Cli::try_parse_from(["manage", "runserver", "--package", "poll-app"])
			.expect("runserver package option should parse");

		let Commands::Runserver { package, .. } = cli.command else {
			panic!("expected runserver command");
		};
		assert_eq!(package.as_deref(), Some("poll-app"));
	}

	#[rstest]
	fn runserver_all_style_features_option_parses() {
		let cli = Cli::try_parse_from(["manage", "runserver", "--all-features"])
			.expect("runserver all style features should parse");

		let Commands::Runserver {
			features,
			all_features,
			..
		} = cli.command
		else {
			panic!("expected runserver command");
		};
		assert!(features.is_empty());
		assert!(all_features);
	}

	#[cfg(feature = "reinhardt-db")]
	#[rstest]
	fn test_requires_database_for_runserver() {
		// Arrange
		let command = Commands::Runserver {
			address: "127.0.0.1:8000".to_string(),
			noreload: false,
			watch_delay: 120,
			no_wasm_rebuild: false,
			no_wasm: false,
			no_override_wasm: false,
			force_wasm: false,
			wasm_optional: false,
			insecure: false,
			no_docs: false,
			with_pages: false,
			static_dir: "dist".to_string(),
			no_spa: false,
			index: None,
			package: None,
			features: vec![],
			all_features: false,
		};

		// Act
		let result = requires_database(&command, &CommandRegistry::new());

		// Assert
		assert!(result);
	}

	#[cfg(feature = "auth")]
	#[rstest]
	fn test_requires_database_for_createsuperuser() {
		// Arrange
		let command = Commands::Createsuperuser {
			username: Some("admin".to_string()),
			email: Some("admin@example.com".to_string()),
			no_password: true,
			noinput: true,
			database: None,
		};

		// Act
		let result = requires_database(&command, &CommandRegistry::new());

		// Assert
		assert!(result);
	}

	#[cfg(feature = "reinhardt-db")]
	#[rstest]
	fn test_requires_database_for_migrate() {
		// Arrange
		let command = Commands::Migrate {
			app_label: None,
			migration_name: None,
			database: None,
			fake: false,
			fake_initial: false,
			plan: false,
			migrations_dir: None,
		};

		// Act
		let result = requires_database(&command, &CommandRegistry::new());

		// Assert
		assert!(result);
	}

	#[cfg(feature = "migrations")]
	#[rstest]
	fn test_squashmigrations_does_not_require_database_initialization() {
		// Arrange
		let command = Commands::Squashmigrations {
			app_label: "polls".to_string(),
			start_migration: None,
			migration_name: "0002".to_string(),
			no_optimize: false,
			no_input: true,
			no_header: false,
			squashed_name: None,
			migrations_dir: None,
		};

		// Act
		let result = requires_database(&command, &CommandRegistry::new());

		// Assert
		assert!(!result);
	}

	#[cfg(feature = "reinhardt-db")]
	#[rstest]
	fn test_requires_database_for_model_fixture_commands() {
		// Arrange
		let commands = [
			Commands::Custom {
				name: "dumpdata".to_string(),
				args: vec![],
			},
			Commands::Custom {
				name: "loaddata".to_string(),
				args: vec!["fixtures/dev.json".to_string()],
			},
			Commands::Custom {
				name: "seed".to_string(),
				args: vec![],
			},
		];

		// Act & Assert
		for command in commands {
			assert!(requires_database(&command, &CommandRegistry::new()));
		}
	}

	#[cfg(feature = "reinhardt-db")]
	#[rstest]
	fn test_does_not_require_database_for_shell() {
		// Arrange
		let command = Commands::Shell { command: None };

		// Act
		let result = requires_database(&command, &CommandRegistry::new());

		// Assert
		assert!(!result);
	}

	#[cfg(not(feature = "shell"))]
	#[tokio::test]
	async fn shell_without_feature_returns_the_direct_and_facade_feature_error() {
		let error = execute_shell(None, 0, None)
			.await
			.expect_err("disabled shell support must return a nonzero error");

		assert_eq!(
			error.to_string(),
			"The shell command requires the `shell` feature when using \
			 `reinhardt-commands` directly, or `commands-shell` through the \
			 `reinhardt` facade."
		);
	}

	#[cfg(feature = "reinhardt-db")]
	#[rstest]
	fn test_does_not_require_database_for_check() {
		// Arrange
		let command = Commands::Check {
			app_label: None,
			deploy: false,
		};

		// Act
		let result = requires_database(&command, &CommandRegistry::new());

		// Assert
		assert!(!result);
	}

	#[cfg(feature = "reinhardt-db")]
	#[rstest]
	fn test_does_not_require_database_for_collectstatic() {
		// Arrange
		let command = Commands::Collectstatic {
			clear: false,
			no_input: false,
			dry_run: false,
			link: false,
			ignore: vec![],
			index: None,
			package: None,
			features: vec![],
			all_features: false,
		};

		// Act
		let result = requires_database(&command, &CommandRegistry::new());

		// Assert
		assert!(!result);
	}
}

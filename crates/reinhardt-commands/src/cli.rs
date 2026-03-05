//! CLI runner for Reinhardt management commands
//!
//! This module provides a unified interface for executing commands from generated `manage.rs` files.
//! It handles argument parsing, command context creation, and command execution.

#[cfg(feature = "migrations")]
use crate::MakeMigrationsCommand;
use crate::base::BaseCommand;
use crate::collectstatic::{CollectStaticCommand, CollectStaticOptions};
use crate::{CheckCommand, CommandContext, MigrateCommand, RunServerCommand, ShellCommand};
use clap::{Parser, Subcommand};
use reinhardt_conf::settings::builder::SettingsBuilder;
use reinhardt_conf::settings::profile::Profile;
use reinhardt_conf::settings::sources::{DefaultSource, LowPriorityEnvSource, TomlFileSource};
use serde_json::Value;
use std::env;
#[allow(unused)]
use std::path::PathBuf;

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

		/// Force using empty state when database/TestContainers is unavailable (dangerous)
		#[arg(long)]
		force_empty_state: bool,

		/// Migration directory
		#[arg(long, default_value = "./migrations")]
		migration_dir: PathBuf,
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
	},

	/// Start the development server
	Runserver {
		/// Server address (default: 127.0.0.1:8000)
		#[arg(value_name = "ADDRESS", default_value = "127.0.0.1:8000")]
		address: String,

		/// Disable auto-reload
		#[arg(long)]
		noreload: bool,

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
	},

	/// Display all registered URL patterns
	Showurls {
		/// Show only named URLs
		#[arg(long)]
		names: bool,
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
	let cli = Cli::parse();

	// Only register router for commands that serve HTTP traffic.
	// DB-only commands (migrate, makemigrations) and utility commands
	// (shell, check, collectstatic) must not require route registration.
	if requires_router(&cli.command) {
		auto_register_router().await?;
	}

	run_command(cli.command, cli.verbosity).await
}

/// Returns `true` for commands that require HTTP route registration.
///
/// Only HTTP-serving commands (`runserver`, `showurls`, `generateopenapi`)
/// need URL patterns registered. DB-only and utility commands work without
/// a `#[routes]` function being present.
fn requires_router(command: &Commands) -> bool {
	match command {
		Commands::Runserver { .. } => true,
		#[cfg(feature = "routers")]
		Commands::Showurls { .. } => true,
		#[cfg(feature = "openapi")]
		Commands::Generateopenapi { .. } => true,
		_ => false,
	}
}

/// Execute a command with the given verbosity level
///
/// This is the internal entry point for executing commands.
/// For most use cases, prefer using `execute_from_command_line()` instead.
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
	match command {
		#[cfg(feature = "migrations")]
		Commands::Makemigrations {
			app_labels,
			dry_run,
			name,
			check,
			empty,
			force_empty_state,
			migration_dir: _,
		} => {
			execute_makemigrations(
				app_labels,
				dry_run,
				name,
				check,
				empty,
				force_empty_state,
				verbosity,
			)
			.await
		}
		Commands::Migrate {
			app_label,
			migration_name,
			database,
			fake,
			fake_initial,
			plan,
		} => {
			execute_migrate(MigrateParams {
				app_label,
				migration_name,
				database,
				fake,
				fake_initial,
				plan,
				verbosity,
			})
			.await
		}
		Commands::Runserver {
			address,
			noreload,
			insecure,
			no_docs,
			with_pages,
			static_dir,
			no_spa,
		} => {
			execute_runserver(RunServerOptions {
				address,
				noreload,
				insecure,
				no_docs,
				with_pages,
				static_dir,
				no_spa,
				verbosity,
			})
			.await
		}
		Commands::Shell { command } => execute_shell(command, verbosity).await,
		Commands::Check { app_label, deploy } => execute_check(app_label, deploy, verbosity).await,
		Commands::Collectstatic {
			clear,
			no_input,
			dry_run,
			link,
			ignore,
		} => execute_collectstatic(clear, no_input, dry_run, link, ignore, verbosity).await,
		Commands::Showurls { names } => execute_showurls(names, verbosity).await,
		#[cfg(feature = "openapi")]
		Commands::Generateopenapi {
			format,
			output,
			postman,
		} => execute_generateopenapi(format, output, postman, verbosity).await,
	}
}

/// Execute the makemigrations command
#[cfg(feature = "migrations")]
async fn execute_makemigrations(
	app_labels: Vec<String>,
	dry_run: bool,
	name: Option<String>,
	check: bool,
	empty: bool,
	force_empty_state: bool,
	verbosity: u8,
) -> Result<(), Box<dyn std::error::Error>> {
	let mut ctx = CommandContext::default();
	ctx.set_verbosity(verbosity);

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

	let cmd = MigrateCommand;
	cmd.execute(&ctx).await.map_err(|e| e.into())
}

/// Options for the runserver command
struct RunServerOptions {
	address: String,
	noreload: bool,
	insecure: bool,
	no_docs: bool,
	with_pages: bool,
	static_dir: String,
	no_spa: bool,
	verbosity: u8,
}

/// Execute the runserver command
async fn execute_runserver(options: RunServerOptions) -> Result<(), Box<dyn std::error::Error>> {
	let mut ctx = CommandContext::default();
	ctx.set_verbosity(options.verbosity);
	ctx.add_arg(options.address);

	if options.noreload {
		ctx.set_option("noreload".to_string(), "true".to_string());
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
	ctx.set_option("static-dir".to_string(), options.static_dir);
	if options.no_spa {
		ctx.set_option("no-spa".to_string(), "true".to_string());
	}

	let cmd = RunServerCommand;
	cmd.execute(&ctx).await.map_err(|e| e.into())
}

/// Execute the shell command
async fn execute_shell(
	command: Option<String>,
	verbosity: u8,
) -> Result<(), Box<dyn std::error::Error>> {
	let mut ctx = CommandContext::default();
	ctx.set_verbosity(verbosity);

	if let Some(cmd_str) = command {
		ctx.set_option("command".to_string(), cmd_str);
	}

	let cmd = ShellCommand;
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
async fn execute_collectstatic(
	clear: bool,
	no_input: bool,
	dry_run: bool,
	link: bool,
	ignore: Vec<String>,
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
				.with_value("admins", Value::Array(vec![]))
				.with_value("managers", Value::Array(vec![])),
		)
		.add_source(LowPriorityEnvSource::new().with_prefix("REINHARDT_"))
		.add_source(TomlFileSource::new(settings_dir.join("base.toml")))
		.add_source(TomlFileSource::new(
			settings_dir.join(format!("{}.toml", profile_str)),
		))
		.build()?;

	let settings = merged.into_typed::<reinhardt_conf::Settings>()?;

	// Convert Settings to StaticFilesConfig
	let config = settings
		.get_static_config()
		.map_err(|e| format!("Failed to get static config: {}", e))?;

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

	// Create and execute command in blocking context
	let mut cmd = CollectStaticCommand::new(config, options);
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
#[allow(dead_code)] // Entry point when openapi feature is disabled
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

/// Automatically discover and register URL pattern functions
///
/// This function uses the `inventory` crate to discover URL pattern functions
/// that were registered at compile time using the `#[routes]` attribute macro.
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if:
/// - No URL patterns were registered
/// - Multiple `#[routes]` functions were detected (should normally be caught at link time)
#[cfg(feature = "routers")]
async fn auto_register_router() -> Result<(), Box<dyn std::error::Error>> {
	use reinhardt_urls::routers::{UrlPatternsRegistration, register_router_arc};

	// Collect all registrations for validation
	let registrations: Vec<_> = inventory::iter::<UrlPatternsRegistration>().collect();

	// Validate single registration
	match registrations.len() {
		0 => {
			return Err("No URL patterns registered.\n\
				 Add the `#[routes]` attribute to your routes function in src/config/urls.rs:\n\n\
				 #[routes]\n\
				 pub fn routes() -> UnifiedRouter {\n\
				     UnifiedRouter::new()\n\
				 }\n\n\
				 If your project uses a library/binary split (src/lib.rs + src/bin/manage.rs),\n\
				 the linker may silently discard route registrations from the library crate.\n\
				 Fix: add `use your_crate_name as _;` to src/bin/manage.rs to force-link\n\
				 the library and preserve its side-effectful route registrations."
				.to_string()
				.into());
		}
		1 => {
			// Expected case: exactly one registration
		}
		n => {
			// Multiple registrations detected.
			// This should normally be caught at link time by the linker marker,
			// but we provide a clear error message as a fallback.
			return Err(format!(
				"Multiple #[routes] functions detected ({n} found).\n\
				 Only one function in the entire project should be annotated with #[routes].\n\n\
				 Please ensure that:\n\
				 1. Only one #[routes] attribute exists in your codebase\n\
				 2. Check src/config/urls.rs and any other files that might have #[routes]\n\
				 3. If you have multiple router configurations, combine them into a single function\n\n\
				 Example:\n\
				 #[routes]\n\
				 pub fn routes() -> UnifiedRouter {{\n\
				     UnifiedRouter::new()\n\
				         .mount(\"/api/\", api::routes())  // NOT annotated with #[routes]\n\
				         .mount(\"/admin/\", admin::routes())\n\
				 }}"
			)
			.into());
		}
	}

	// Get and register the router
	let registration = &registrations[0];
	let router = (registration.get_server_router)();
	register_router_arc(router);

	Ok(())
}

/// No-op implementation when routers feature is disabled
#[cfg(not(feature = "routers"))]
async fn auto_register_router() -> Result<(), Box<dyn std::error::Error>> {
	// No router registration needed when routers feature is disabled
	Ok(())
}

/// Generate a cryptographically random secret key for fallback use.
///
/// Produces a 50-character hex string (200 bits of entropy). This is used
/// as the default `SECRET_KEY` when no explicit key is configured, ensuring
/// that each process gets a unique key rather than a shared hardcoded value.
fn generate_random_secret_key() -> String {
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

	#[rstest]
	fn test_requires_router_for_runserver() {
		// Arrange
		let command = Commands::Runserver {
			address: "127.0.0.1:8000".to_string(),
			noreload: false,
			insecure: false,
			no_docs: false,
			with_pages: false,
			static_dir: "dist".to_string(),
			no_spa: false,
		};

		// Act
		let result = requires_router(&command);

		// Assert
		assert!(result);
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
			force_empty_state: false,
			migration_dir: std::path::PathBuf::from("./migrations"),
		};

		// Act
		let result = requires_router(&command);

		// Assert
		assert!(!result);
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
}

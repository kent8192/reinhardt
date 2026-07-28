#![warn(missing_docs)]

//! Reinhardt Admin CLI
//!
//! Global command-line tool for Reinhardt project management.
//! This is the equivalent of Django's `django-admin` command.
//!
//! ## Installation
//!
//! Install the current stable CLI with Cargo. The command below pins this
//! guide to the documented release for reproducibility; omit `--version` to
//! let Cargo choose the latest stable release. The literal below is
//! release-managed.
//!
//! <!-- reinhardt-version-sync -->
//! ```bash
//! cargo install reinhardt-admin-cli --version "0.4.0-alpha.2"
//! ```
//!
//! ## Usage
//!
//! ```bash
//! reinhardt-admin startproject myproject --with-rest
//! reinhardt-admin startproject myproject --with-pages
//! reinhardt-admin startapp myapp --with-rest
//! reinhardt-formatter fmt src/
//! reinhardt-admin --help
//! ```

use reinhardt_admin_cli::migrate_v2;

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use colored::Colorize;
use dialoguer::Select;
use reinhardt_commands::{
	BaseCommand, CommandContext, CommandResult, ConfigureCommand, PluginDisableCommand,
	PluginEnableCommand, PluginInfoCommand, PluginInstallCommand, PluginListCommand,
	PluginRemoveCommand, PluginSearchCommand, PluginUpdateCommand, StartAppCommand,
	StartProjectCommand,
};
use std::io::IsTerminal;
use std::process;

/// Project/app architecture type used with `--template`.
#[derive(Clone, Debug, ValueEnum)]
enum TemplateType {
	/// RESTful API project/app structure
	Rest,
	/// Pages (WASM + SSR) project/app structure
	Pages,
}

/// The resolved project architecture, after alias expansion.
enum ResolvedProjectType {
	Pages,
	Rest,
}

/// Resolves the mutually-exclusive `--template`/`--with-pages`/`--with-rest` group
/// into a single `ResolvedProjectType`.
///
/// # Panics
///
/// Unreachable in practice — clap's `ArgGroup` with `required = true` guarantees
/// that exactly one of the three args is set.
fn resolve_project_type(
	template: Option<TemplateType>,
	with_pages: bool,
	with_rest: bool,
) -> Option<ResolvedProjectType> {
	match (template, with_pages, with_rest) {
		(Some(TemplateType::Pages), _, _) | (_, true, _) => Some(ResolvedProjectType::Pages),
		(Some(TemplateType::Rest), _, _) | (_, _, true) => Some(ResolvedProjectType::Rest),
		_ => None,
	}
}

#[derive(Parser)]
#[command(name = "reinhardt-admin")]
#[command(about = "Reinhardt project administration utility", long_about = None)]
#[command(version)]
struct Cli {
	#[command(subcommand)]
	command: Commands,

	/// Verbosity level (can be repeated)
	#[arg(short, long, action = clap::ArgAction::Count)]
	verbosity: u8,
}

#[derive(Subcommand)]
enum Commands {
	/// Create a new Reinhardt project
	#[command(group(
		clap::ArgGroup::new("project_type")
			.args(["template", "with_pages", "with_rest"])
	))]
	Startproject {
		/// Name of the project
		#[arg(value_name = "PROJECT_NAME")]
		name: String,

		/// Directory to create the project in (defaults to current directory)
		#[arg(value_name = "DIRECTORY")]
		directory: Option<String>,

		/// Project architecture type: rest (RESTful API) or pages (WASM + SSR)
		#[arg(long, value_name = "TYPE", value_enum, group = "project_type")]
		template: Option<TemplateType>,

		/// Create a project with reinhardt-pages (WASM + SSR). Alias for --template pages.
		#[arg(long, group = "project_type")]
		with_pages: bool,

		/// Create a RESTful API project. Alias for --template rest.
		#[arg(long, group = "project_type")]
		with_rest: bool,

		/// Root directory whose sub-templates override embedded defaults.
		/// Also reads the REINHARDT_TEMPLATE_DIR environment variable.
		#[arg(long, value_name = "DIR")]
		template_dir: Option<String>,

		/// Reinhardt version requirement to write into Cargo.toml.
		#[arg(long, value_name = "VERSION")]
		reinhardt_version: Option<String>,

		/// Reinhardt feature to enable. Can be repeated.
		#[arg(long = "feature", value_name = "FEATURE")]
		feature: Vec<String>,

		/// Comma-separated Reinhardt features to enable.
		#[arg(long, value_name = "CSV")]
		features: Option<String>,

		/// Whether to keep Cargo default features enabled for the Reinhardt dependency.
		#[arg(long, action = clap::ArgAction::Set)]
		default_features: Option<bool>,

		/// Disable prompts and use deterministic defaults for omitted choices.
		#[arg(long)]
		no_interactive: bool,
	},

	/// Create a new Reinhardt app
	#[command(group(
		clap::ArgGroup::new("app_type")
			.required(true)
			.args(["template", "with_pages", "with_rest"])
	))]
	Startapp {
		/// Name of the app
		#[arg(value_name = "APP_NAME")]
		name: String,

		/// Directory to create the app in (defaults to current directory)
		#[arg(value_name = "DIRECTORY")]
		directory: Option<String>,

		/// App architecture type: rest (RESTful API) or pages (WASM + SSR)
		#[arg(long, value_name = "TYPE", value_enum, group = "app_type")]
		template: Option<TemplateType>,

		/// Create an app with reinhardt-pages (WASM + SSR). Alias for --template pages.
		#[arg(long, group = "app_type")]
		with_pages: bool,

		/// Create a RESTful API app. Alias for --template rest.
		#[arg(long, group = "app_type")]
		with_rest: bool,

		/// Root directory whose sub-templates override embedded defaults.
		/// Also reads the REINHARDT_TEMPLATE_DIR environment variable.
		#[arg(long, value_name = "DIR")]
		template_dir: Option<String>,
	},

	/// Manage Reinhardt plugins (Dentdelion)
	Plugin {
		#[command(subcommand)]
		subcommand: PluginCommands,
	},

	/// Configure Reinhardt version and features in an existing project
	Configure {
		/// Project directory containing Cargo.toml (defaults to current directory)
		#[arg(value_name = "DIRECTORY")]
		directory: Option<String>,

		/// Reinhardt version requirement to write into Cargo.toml.
		#[arg(long, value_name = "VERSION")]
		reinhardt_version: Option<String>,

		/// Reinhardt feature to enable. Can be repeated.
		#[arg(long = "feature", value_name = "FEATURE")]
		feature: Vec<String>,

		/// Comma-separated Reinhardt features to enable.
		#[arg(long, value_name = "CSV")]
		features: Option<String>,

		/// Whether to keep Cargo default features enabled for the Reinhardt dependency.
		#[arg(long, action = clap::ArgAction::Set)]
		default_features: Option<bool>,

		/// Disable prompts and use deterministic defaults for omitted choices.
		#[arg(long)]
		no_interactive: bool,
	},

	/// Format Rust code and page!/form!/head!/style! macro DSL in source files
	///
	/// By default, formats page!/form!/head!/style! DSL macros with Topiary and then runs rustfmt.
	/// Use --with-rustfmt=false to only format Reinhardt DSL macros.
	Fmt {
		/// Path to file or directory to format
		#[arg(value_name = "PATH")]
		path: PathBuf,

		/// Check if files are formatted without modifying them
		#[arg(long)]
		check: bool,

		/// Also run rustfmt after DSL formatting
		#[arg(long, default_value = "true", action = clap::ArgAction::Set)]
		with_rustfmt: bool,

		/// Path to rustfmt.toml configuration file
		#[arg(long, value_name = "PATH")]
		config_path: Option<PathBuf>,

		/// Rust edition to use (overrides rustfmt.toml)
		#[arg(long, value_name = "EDITION")]
		edition: Option<String>,

		/// Style edition to use (overrides rustfmt.toml)
		#[arg(long, value_name = "EDITION")]
		style_edition: Option<String>,

		/// Set options from command line (overrides rustfmt.toml)
		/// Format: key1=val1,key2=val2
		#[arg(long, value_name = "OPTIONS")]
		config: Option<String>,

		/// Use colored output
		#[arg(long, value_name = "WHEN", default_value = "auto")]
		color: String,

		/// Backup any modified files
		#[arg(long)]
		backup: bool,
	},

	/// Format all code: Reinhardt DSL macros via Topiary + Rust via rustfmt
	///
	/// This command formats page!/form!/head!/style! macros first, then runs
	/// `cargo fmt --all` on the root workspace.
	///
	/// Files that belong to a separate (nested) cargo workspace, such as
	/// `examples/` or the trybuild fixture crates, are skipped. `cargo fmt
	/// --all` only formats the root workspace, so formatting their DSL macros
	/// without the rustfmt pass would leave them inconsistent. Format those
	/// sub-workspaces with their own task (e.g. `cargo make fmt-check-examples`).
	FmtAll {
		/// Check if files are formatted without modifying them
		#[arg(long)]
		check: bool,

		/// Path to rustfmt.toml configuration file
		/// If not specified, searches upward from current directory for rustfmt.toml
		#[arg(long, value_name = "PATH")]
		config_path: Option<PathBuf>,

		/// Rust edition to use (overrides rustfmt.toml)
		#[arg(long, value_name = "EDITION")]
		edition: Option<String>,

		/// Style edition to use (overrides rustfmt.toml)
		#[arg(long, value_name = "EDITION")]
		style_edition: Option<String>,

		/// Set options from command line (overrides rustfmt.toml)
		/// Format: key1=val1,key2=val2
		#[arg(long, value_name = "OPTIONS")]
		config: Option<String>,

		/// Use colored output
		#[arg(long, value_name = "WHEN", default_value = "auto")]
		color: String,

		/// Backup any modified files
		#[arg(long)]
		backup: bool,
	},

	/// Migrate Manouche v1 source files to v2 grammar (spec §6.1 + §6.2).
	MigrateManoucheV2(migrate_v2::MigrateV2Args),
}

#[derive(Debug)]
struct DependencyOptions {
	reinhardt_version: Option<String>,
	feature: Vec<String>,
	features: Option<String>,
	default_features: Option<bool>,
	no_interactive: bool,
}

#[derive(Debug)]
struct ProjectTypeOptions {
	template: Option<TemplateType>,
	with_pages: bool,
	with_rest: bool,
}

/// Plugin management subcommands
#[derive(Subcommand)]
enum PluginCommands {
	/// List installed plugins
	List {
		/// Show detailed information
		#[arg(short, long)]
		verbose: bool,

		/// Show only enabled plugins
		#[arg(long)]
		enabled: bool,

		/// Show only disabled plugins
		#[arg(long)]
		disabled: bool,

		/// Project root directory
		#[arg(long)]
		project_root: Option<String>,
	},

	/// Show plugin information
	Info {
		/// Plugin name (e.g., auth-delion)
		#[arg(value_name = "NAME")]
		name: String,

		/// Fetch info from crates.io instead of local
		#[arg(long)]
		remote: bool,

		/// Project root directory
		#[arg(long)]
		project_root: Option<String>,
	},

	/// Install a plugin from crates.io
	Install {
		/// Plugin name (e.g., auth-delion)
		#[arg(value_name = "NAME")]
		name: String,

		/// Specific version to install
		#[arg(long)]
		version: Option<String>,

		/// Skip confirmation prompt
		#[arg(short, long)]
		yes: bool,

		/// Project root directory
		#[arg(long)]
		project_root: Option<String>,
	},

	/// Remove a plugin
	Remove {
		/// Plugin name to remove
		#[arg(value_name = "NAME")]
		name: String,

		/// Also remove plugin configuration
		#[arg(long)]
		purge: bool,

		/// Skip confirmation prompt
		#[arg(short, long)]
		yes: bool,

		/// Project root directory
		#[arg(long)]
		project_root: Option<String>,
	},

	/// Enable a disabled plugin
	Enable {
		/// Plugin name to enable
		#[arg(value_name = "NAME")]
		name: String,

		/// Project root directory
		#[arg(long)]
		project_root: Option<String>,
	},

	/// Disable an enabled plugin
	Disable {
		/// Plugin name to disable
		#[arg(value_name = "NAME")]
		name: String,

		/// Project root directory
		#[arg(long)]
		project_root: Option<String>,
	},

	/// Search for plugins on crates.io
	Search {
		/// Search query
		#[arg(value_name = "QUERY")]
		query: String,

		/// Maximum number of results
		#[arg(long, default_value = "10")]
		limit: u64,
	},

	/// Update plugin(s) to latest version
	Update {
		/// Plugin name (omit for --all)
		#[arg(value_name = "NAME")]
		name: Option<String>,

		/// Update all plugins
		#[arg(long)]
		all: bool,

		/// Skip confirmation prompt
		#[arg(short, long)]
		yes: bool,

		/// Project root directory
		#[arg(long)]
		project_root: Option<String>,
	},
}

#[tokio::main]
async fn main() {
	let cli = Cli::parse();

	let result = match cli.command {
		Commands::Startproject {
			name,
			directory,
			template,
			with_pages,
			with_rest,
			template_dir,
			reinhardt_version,
			feature,
			features,
			default_features,
			no_interactive,
		} => {
			let project_type_options = ProjectTypeOptions {
				template,
				with_pages,
				with_rest,
			};
			let dependency_options = DependencyOptions {
				reinhardt_version,
				feature,
				features,
				default_features,
				no_interactive,
			};
			run_startproject(
				name,
				directory,
				project_type_options,
				template_dir,
				dependency_options,
				cli.verbosity,
			)
			.await
		}
		Commands::Startapp {
			name,
			directory,
			template,
			with_pages,
			with_rest,
			template_dir,
		} => {
			run_startapp(
				name,
				directory,
				template,
				with_pages,
				with_rest,
				template_dir,
				cli.verbosity,
			)
			.await
		}
		Commands::Plugin { subcommand } => run_plugin(subcommand, cli.verbosity).await,
		Commands::Configure {
			directory,
			reinhardt_version,
			feature,
			features,
			default_features,
			no_interactive,
		} => {
			let dependency_options = DependencyOptions {
				reinhardt_version,
				feature,
				features,
				default_features,
				no_interactive,
			};
			run_configure(directory, dependency_options, cli.verbosity).await
		}
		Commands::Fmt {
			path,
			check,
			with_rustfmt,
			config_path,
			edition,
			style_edition,
			config,
			color,
			backup,
		} => run_fmt(
			path,
			check,
			with_rustfmt,
			config_path,
			edition,
			style_edition,
			config,
			color,
			backup,
			cli.verbosity,
		),
		Commands::FmtAll {
			check,
			config_path,
			edition,
			style_edition,
			config,
			color,
			backup,
		} => run_fmt_all(
			check,
			config_path,
			edition,
			style_edition,
			config,
			color,
			backup,
			cli.verbosity,
		),
		Commands::MigrateManoucheV2(args) => {
			if let Err(e) = migrate_v2::run(args) {
				eprintln!("{}", format!("error: {e}").red());
				process::exit(1);
			}
			Ok(())
		}
	};

	if let Err(e) = result {
		eprintln!("Error: {}", e);
		process::exit(1);
	}
}

async fn run_startproject(
	name: String,
	directory: Option<String>,
	project_type_options: ProjectTypeOptions,
	template_dir: Option<String>,
	dependency_options: DependencyOptions,
	verbosity: u8,
) -> CommandResult<()> {
	let mut ctx = CommandContext::default();
	ctx.set_verbosity(verbosity);
	ctx.add_arg(name);
	if let Some(dir) = directory {
		ctx.add_arg(dir);
	}
	let project_type = match resolve_project_type(
		project_type_options.template,
		project_type_options.with_pages,
		project_type_options.with_rest,
	) {
		Some(project_type) => project_type,
		None if should_prompt(dependency_options.no_interactive) => prompt_project_type()?,
		None => ResolvedProjectType::Rest,
	};
	match project_type {
		ResolvedProjectType::Pages => ctx.set_option("with-pages".to_string(), "true".to_string()),
		ResolvedProjectType::Rest => ctx.set_option("restful".to_string(), "true".to_string()),
	}
	if let Some(td) = template_dir {
		ctx.set_option("template-dir".to_string(), td);
	}
	push_dependency_options(&mut ctx, dependency_options);

	let cmd = StartProjectCommand;
	cmd.execute(&ctx).await
}

async fn run_startapp(
	name: String,
	directory: Option<String>,
	template: Option<TemplateType>,
	with_pages: bool,
	with_rest: bool,
	template_dir: Option<String>,
	verbosity: u8,
) -> CommandResult<()> {
	let mut ctx = CommandContext::default();
	ctx.set_verbosity(verbosity);
	ctx.add_arg(name);
	if let Some(dir) = directory {
		ctx.add_arg(dir);
	}
	match resolve_project_type(template, with_pages, with_rest) {
		Some(ResolvedProjectType::Pages) => {
			ctx.set_option("with-pages".to_string(), "true".to_string())
		}
		Some(ResolvedProjectType::Rest) => {
			ctx.set_option("restful".to_string(), "true".to_string())
		}
		None => unreachable!("clap requires a startapp project type"),
	}
	if let Some(td) = template_dir {
		ctx.set_option("template-dir".to_string(), td);
	}

	let cmd = StartAppCommand;
	cmd.execute(&ctx).await
}

async fn run_configure(
	directory: Option<String>,
	dependency_options: DependencyOptions,
	verbosity: u8,
) -> CommandResult<()> {
	let mut ctx = CommandContext::default();
	ctx.set_verbosity(verbosity);
	if let Some(directory) = directory {
		ctx.add_arg(directory);
	}
	push_dependency_options(&mut ctx, dependency_options);

	ConfigureCommand.execute(&ctx).await
}

fn push_dependency_options(ctx: &mut CommandContext, options: DependencyOptions) {
	if let Some(version) = options.reinhardt_version {
		ctx.set_option("reinhardt-version".to_string(), version);
	}
	if !options.feature.is_empty() {
		ctx.set_option_multi("feature".to_string(), options.feature);
	}
	if let Some(features) = options.features {
		ctx.set_option("features".to_string(), features);
	}
	if let Some(default_features) = options.default_features {
		ctx.set_option("default-features".to_string(), default_features.to_string());
	}
	if options.no_interactive {
		ctx.set_option("no-interactive".to_string(), "true".to_string());
	}
}

fn should_prompt(no_interactive: bool) -> bool {
	!cfg!(test)
		&& !no_interactive
		&& std::env::var("REINHARDT_TEST_MODE").is_err()
		&& std::io::stdin().is_terminal()
}

fn prompt_project_type() -> CommandResult<ResolvedProjectType> {
	let choices = ["RESTful API", "Pages (WASM + SSR)"];
	let selected = Select::new()
		.with_prompt("Select Reinhardt project type")
		.items(choices)
		.default(0)
		.interact()
		.map_err(|error| {
			reinhardt_commands::CommandError::ExecutionError(format!("Prompt failed: {error}"))
		})?;
	if selected == 1 {
		Ok(ResolvedProjectType::Pages)
	} else {
		Ok(ResolvedProjectType::Rest)
	}
}

async fn run_plugin(subcommand: PluginCommands, verbosity: u8) -> CommandResult<()> {
	match subcommand {
		PluginCommands::List {
			verbose,
			enabled,
			disabled,
			project_root,
		} => {
			let mut ctx = CommandContext::default();
			ctx.set_verbosity(verbosity);
			if verbose {
				ctx.set_option("verbose".to_string(), "true".to_string());
			}
			if enabled {
				ctx.set_option("enabled".to_string(), "true".to_string());
			}
			if disabled {
				ctx.set_option("disabled".to_string(), "true".to_string());
			}
			if let Some(root) = project_root {
				ctx.set_option("project-root".to_string(), root);
			}
			PluginListCommand.execute(&ctx).await
		}
		PluginCommands::Info {
			name,
			remote,
			project_root,
		} => {
			let mut ctx = CommandContext::default();
			ctx.set_verbosity(verbosity);
			ctx.add_arg(name);
			if remote {
				ctx.set_option("remote".to_string(), "true".to_string());
			}
			if let Some(root) = project_root {
				ctx.set_option("project-root".to_string(), root);
			}
			PluginInfoCommand.execute(&ctx).await
		}
		PluginCommands::Install {
			name,
			version,
			yes,
			project_root,
		} => {
			let mut ctx = CommandContext::default();
			ctx.set_verbosity(verbosity);
			ctx.add_arg(name);
			if let Some(v) = version {
				ctx.set_option("version".to_string(), v);
			}
			if yes {
				ctx.set_option("yes".to_string(), "true".to_string());
			}
			if let Some(root) = project_root {
				ctx.set_option("project-root".to_string(), root);
			}
			PluginInstallCommand.execute(&ctx).await
		}
		PluginCommands::Remove {
			name,
			purge,
			yes,
			project_root,
		} => {
			let mut ctx = CommandContext::default();
			ctx.set_verbosity(verbosity);
			ctx.add_arg(name);
			if purge {
				ctx.set_option("purge".to_string(), "true".to_string());
			}
			if yes {
				ctx.set_option("yes".to_string(), "true".to_string());
			}
			if let Some(root) = project_root {
				ctx.set_option("project-root".to_string(), root);
			}
			PluginRemoveCommand.execute(&ctx).await
		}
		PluginCommands::Enable { name, project_root } => {
			let mut ctx = CommandContext::default();
			ctx.set_verbosity(verbosity);
			ctx.add_arg(name);
			if let Some(root) = project_root {
				ctx.set_option("project-root".to_string(), root);
			}
			PluginEnableCommand.execute(&ctx).await
		}
		PluginCommands::Disable { name, project_root } => {
			let mut ctx = CommandContext::default();
			ctx.set_verbosity(verbosity);
			ctx.add_arg(name);
			if let Some(root) = project_root {
				ctx.set_option("project-root".to_string(), root);
			}
			PluginDisableCommand.execute(&ctx).await
		}
		PluginCommands::Search { query, limit } => {
			let mut ctx = CommandContext::default();
			ctx.set_verbosity(verbosity);
			ctx.add_arg(query);
			ctx.set_option("limit".to_string(), limit.to_string());
			PluginSearchCommand.execute(&ctx).await
		}
		PluginCommands::Update {
			name,
			all,
			yes,
			project_root,
		} => {
			let mut ctx = CommandContext::default();
			ctx.set_verbosity(verbosity);
			if let Some(n) = name {
				ctx.add_arg(n);
			}
			if all {
				ctx.set_option("all".to_string(), "true".to_string());
			}
			if yes {
				ctx.set_option("yes".to_string(), "true".to_string());
			}
			if let Some(root) = project_root {
				ctx.set_option("project-root".to_string(), root);
			}
			PluginUpdateCommand.execute(&ctx).await
		}
	}
}

#[allow(clippy::too_many_arguments)] // CLI command handler with many options
fn run_fmt(
	path: PathBuf,
	check: bool,
	with_rustfmt: bool,
	config_path: Option<PathBuf>,
	edition: Option<String>,
	style_edition: Option<String>,
	config: Option<String>,
	color: String,
	backup: bool,
	verbosity: u8,
) -> CommandResult<()> {
	let command = build_fmt_command(
		path,
		check,
		with_rustfmt,
		config_path,
		edition,
		style_edition,
		config,
		color,
		backup,
		verbosity,
	);
	run_formatter_delegate(command)
}

#[allow(clippy::too_many_arguments)] // Pure command builder mirrors the CLI options.
fn build_fmt_command(
	path: PathBuf,
	check: bool,
	with_rustfmt: bool,
	config_path: Option<PathBuf>,
	edition: Option<String>,
	style_edition: Option<String>,
	config: Option<String>,
	color: String,
	backup: bool,
	verbosity: u8,
) -> process::Command {
	let mut command = formatter_delegate_command(verbosity);
	command.arg("fmt").arg(path);
	push_flag(&mut command, "--check", check);
	if !with_rustfmt {
		command.arg("--with-rustfmt=false");
	}
	push_optional_path(&mut command, "--config-path", config_path.as_ref());
	push_optional_str(&mut command, "--edition", edition.as_deref());
	push_optional_str(&mut command, "--style-edition", style_edition.as_deref());
	push_optional_str(&mut command, "--config", config.as_deref());
	push_optional_str(&mut command, "--color", Some(&color));
	push_flag(&mut command, "--backup", backup);
	command
}

/// Format all code: DSL macros via Topiary, then Rust via `cargo fmt --all`.
#[allow(clippy::too_many_arguments)] // CLI command handler with many options
fn run_fmt_all(
	check: bool,
	config_path: Option<PathBuf>,
	edition: Option<String>,
	style_edition: Option<String>,
	config: Option<String>,
	color: String,
	backup: bool,
	verbosity: u8,
) -> CommandResult<()> {
	let command = build_fmt_all_command(
		check,
		config_path,
		edition,
		style_edition,
		config,
		color,
		backup,
		verbosity,
	);
	run_formatter_delegate(command)
}

#[allow(clippy::too_many_arguments)] // Pure command builder mirrors the CLI options.
fn build_fmt_all_command(
	check: bool,
	config_path: Option<PathBuf>,
	edition: Option<String>,
	style_edition: Option<String>,
	config: Option<String>,
	color: String,
	backup: bool,
	verbosity: u8,
) -> process::Command {
	let mut command = formatter_delegate_command(verbosity);
	command.arg("fmt-all");
	push_flag(&mut command, "--check", check);
	push_optional_path(&mut command, "--config-path", config_path.as_ref());
	push_optional_str(&mut command, "--edition", edition.as_deref());
	push_optional_str(&mut command, "--style-edition", style_edition.as_deref());
	push_optional_str(&mut command, "--config", config.as_deref());
	push_optional_str(&mut command, "--color", Some(&color));
	push_flag(&mut command, "--backup", backup);
	command
}

fn formatter_delegate_command(verbosity: u8) -> process::Command {
	let mut command = process::Command::new("reinhardt-formatter");
	for _ in 0..verbosity {
		command.arg("-v");
	}
	command
}

fn push_flag(command: &mut process::Command, flag: &str, enabled: bool) {
	if enabled {
		command.arg(flag);
	}
}

fn push_optional_path(command: &mut process::Command, flag: &str, value: Option<&PathBuf>) {
	if let Some(value) = value {
		command.arg(flag).arg(value);
	}
}

fn push_optional_str(command: &mut process::Command, flag: &str, value: Option<&str>) {
	if let Some(value) = value {
		command.arg(flag).arg(value);
	}
}

fn run_formatter_delegate(mut command: process::Command) -> CommandResult<()> {
	let status = command.status().map_err(|err| {
		reinhardt_commands::CommandError::ExecutionError(format!(
			"failed to run reinhardt-formatter: {err}. Install or build the formatter binary to use fmt commands."
		))
	})?;

	if status.success() {
		Ok(())
	} else {
		Err(reinhardt_commands::CommandError::ExecutionError(format!(
			"reinhardt-formatter exited with status {status}"
		)))
	}
}

#[cfg(test)]
mod resolve_project_type_tests {
	use super::*;

	#[test]
	fn with_pages_bool_resolves_to_pages() {
		assert!(matches!(
			resolve_project_type(None, true, false),
			Some(ResolvedProjectType::Pages)
		));
	}

	#[test]
	fn with_rest_bool_resolves_to_rest() {
		assert!(matches!(
			resolve_project_type(None, false, true),
			Some(ResolvedProjectType::Rest)
		));
	}

	#[test]
	fn template_pages_resolves_to_pages() {
		assert!(matches!(
			resolve_project_type(Some(TemplateType::Pages), false, false),
			Some(ResolvedProjectType::Pages)
		));
	}

	#[test]
	fn template_rest_resolves_to_rest() {
		assert!(matches!(
			resolve_project_type(Some(TemplateType::Rest), false, false),
			Some(ResolvedProjectType::Rest)
		));
	}

	#[test]
	fn template_pages_with_bool_false_resolves_to_pages() {
		// --template pages takes precedence, both bools false
		assert!(matches!(
			resolve_project_type(Some(TemplateType::Pages), false, false),
			Some(ResolvedProjectType::Pages)
		));
	}

	#[test]
	fn template_rest_with_bool_false_resolves_to_rest() {
		// --template rest takes precedence, both bools false
		assert!(matches!(
			resolve_project_type(Some(TemplateType::Rest), false, false),
			Some(ResolvedProjectType::Rest)
		));
	}

	#[test]
	fn omitted_type_resolves_to_none() {
		assert!(resolve_project_type(None, false, false).is_none());
	}
}

#[cfg(test)]
mod arg_group_tests {
	use super::*;
	fn try_parse(args: &[&str]) -> Result<Cli, clap::Error> {
		Cli::try_parse_from(args)
	}

	#[test]
	fn startproject_with_pages_flag_accepted() {
		assert!(
			try_parse(&["reinhardt-admin", "startproject", "myproj", "--with-pages"]).is_ok(),
			"--with-pages should be accepted"
		);
	}

	#[test]
	fn startproject_with_rest_flag_accepted() {
		assert!(
			try_parse(&["reinhardt-admin", "startproject", "myproj", "--with-rest"]).is_ok(),
			"--with-rest should be accepted"
		);
	}

	#[test]
	fn startproject_template_pages_accepted() {
		assert!(
			try_parse(&[
				"reinhardt-admin",
				"startproject",
				"myproj",
				"--template",
				"pages"
			])
			.is_ok(),
			"--template pages should be accepted"
		);
	}

	#[test]
	fn startproject_dependency_flags_are_accepted() {
		assert!(
			try_parse(&[
				"reinhardt-admin",
				"startproject",
				"myproj",
				"--template",
				"pages",
				"--features",
				"standard,pages,admin,conf,commands,db-sqlite,forms,auth-session,argon2-hasher",
				"--no-interactive",
			])
			.is_ok(),
			"startproject dependency flags should parse"
		);
	}

	#[test]
	fn startproject_template_rest_accepted() {
		assert!(
			try_parse(&[
				"reinhardt-admin",
				"startproject",
				"myproj",
				"--template",
				"rest"
			])
			.is_ok(),
			"--template rest should be accepted"
		);
	}

	#[test]
	fn startproject_missing_type_is_accepted_for_interactive_or_default() {
		let result = try_parse(&["reinhardt-admin", "startproject", "myproj"]);
		assert!(result.is_ok(), "type flag can be omitted");
	}

	#[test]
	fn startproject_duplicate_flags_are_error() {
		assert!(
			try_parse(&[
				"reinhardt-admin",
				"startproject",
				"myproj",
				"--with-pages",
				"--with-rest",
			])
			.is_err(),
			"duplicate type flags should be rejected"
		);
	}

	#[test]
	fn startproject_template_and_alias_together_are_error() {
		assert!(
			try_parse(&[
				"reinhardt-admin",
				"startproject",
				"myproj",
				"--template",
				"pages",
				"--with-pages",
			])
			.is_err(),
			"--template + --with-pages should be rejected"
		);
	}

	#[test]
	fn startapp_with_pages_flag_accepted() {
		assert!(
			try_parse(&["reinhardt-admin", "startapp", "myapp", "--with-pages"]).is_ok(),
			"--with-pages should be accepted for startapp"
		);
	}

	#[test]
	fn startapp_missing_type_is_error() {
		let result = try_parse(&["reinhardt-admin", "startapp", "myapp"]);
		assert!(result.is_err(), "expected Err when type flag omitted");
		assert_eq!(
			result.err().unwrap().kind(),
			clap::error::ErrorKind::MissingRequiredArgument
		);
	}

	#[test]
	fn configure_dependency_flags_are_accepted() {
		assert!(
			try_parse(&[
				"reinhardt-admin",
				"configure",
				"--reinhardt-version",
				"0.2.0-rc.4",
				"--features",
				"minimal,db-sqlite",
				"--no-interactive",
			])
			.is_ok(),
			"configure dependency flags should parse"
		);
	}

	#[test]
	fn formatter_delegate_builds_exact_style_fmt_command() {
		let directory = tempfile::tempdir().expect("create temporary directory");
		let source = directory.path().join("styles.rs");
		std::fs::write(
			&source,
			"#[style_def] static STYLES: TestStyles = style! { .card { color: red; } };",
		)
		.expect("write temporary source");

		let command = build_fmt_command(
			source.clone(),
			true,
			false,
			Some(PathBuf::from("rustfmt.toml")),
			Some("2024".to_string()),
			Some("2024".to_string()),
			Some("hard_tabs=true".to_string()),
			"never".to_string(),
			true,
			2,
		);

		assert_eq!(command.get_program(), "reinhardt-formatter");
		let args: Vec<String> = command
			.get_args()
			.map(|argument| argument.to_string_lossy().into_owned())
			.collect();
		assert_eq!(
			args,
			vec![
				"-v".to_string(),
				"-v".to_string(),
				"fmt".to_string(),
				source.to_string_lossy().into_owned(),
				"--check".to_string(),
				"--with-rustfmt=false".to_string(),
				"--config-path".to_string(),
				"rustfmt.toml".to_string(),
				"--edition".to_string(),
				"2024".to_string(),
				"--style-edition".to_string(),
				"2024".to_string(),
				"--config".to_string(),
				"hard_tabs=true".to_string(),
				"--color".to_string(),
				"never".to_string(),
				"--backup".to_string(),
			]
		);
	}

	#[test]
	fn formatter_delegate_builds_exact_fmt_all_command() {
		let command =
			build_fmt_all_command(true, None, None, None, None, "auto".to_string(), false, 1);

		assert_eq!(command.get_program(), "reinhardt-formatter");
		let args: Vec<String> = command
			.get_args()
			.map(|argument| argument.to_string_lossy().into_owned())
			.collect();
		assert_eq!(args, ["-v", "fmt-all", "--check", "--color", "auto"]);
	}
}

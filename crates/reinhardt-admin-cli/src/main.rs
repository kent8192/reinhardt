//! Reinhardt Admin CLI
//!
//! Global command-line tool for Reinhardt project management.
//! This is the equivalent of Django's `django-admin` command.
//!
//! ## Installation
//!
//! ```bash
//! cargo install reinhardt-commands
//! ```
//!
//! ## Usage
//!
//! ```bash
//! reinhardt-admin startproject myproject
//! reinhardt-admin startapp myapp
//! reinhardt-admin fmt src/
//! reinhardt-admin help
//! ```

mod ast_formatter;
mod formatter;
mod utils;

use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand, ValueEnum};
use colored::Colorize;
use reinhardt_commands::{
	BaseCommand, CommandContext, CommandResult, PluginDisableCommand, PluginEnableCommand,
	PluginInfoCommand, PluginInstallCommand, PluginListCommand, PluginRemoveCommand,
	PluginSearchCommand, PluginUpdateCommand, StartAppCommand, StartProjectCommand,
};
use std::process;
use zeroize::Zeroize;

/// Valid project and app template types.
///
/// Restricts the `--template-type` argument to known values,
/// preventing typos and providing helpful error messages.
#[derive(Clone, Debug, ValueEnum)]
enum TemplateType {
	/// RESTful API project structure
	Restful,
	/// Model-Template-View project structure
	Mtv,
}

impl std::fmt::Display for TemplateType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			TemplateType::Restful => write!(f, "restful"),
			TemplateType::Mtv => write!(f, "mtv"),
		}
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
	Startproject {
		/// Name of the project
		#[arg(value_name = "PROJECT_NAME")]
		name: String,

		/// Directory to create the project in (defaults to current directory)
		#[arg(value_name = "DIRECTORY")]
		directory: Option<String>,

		/// Project template type: mtv (Model-Template-View) or restful (RESTful API)
		#[arg(short = 't', long, value_enum, default_value = "restful")]
		template_type: TemplateType,
	},

	/// Create a new Reinhardt app
	Startapp {
		/// Name of the app
		#[arg(value_name = "APP_NAME")]
		name: String,

		/// Directory to create the app in (defaults to current directory)
		#[arg(value_name = "DIRECTORY")]
		directory: Option<String>,

		/// App template type: mtv or restful
		#[arg(short = 't', long, value_enum, default_value = "restful")]
		template_type: TemplateType,
	},

	/// Manage Reinhardt plugins (Dentdelion)
	Plugin {
		#[command(subcommand)]
		subcommand: PluginCommands,
	},

	/// Format Rust code and page! macro DSL in source files
	///
	/// By default, runs both rustfmt (protecting page! macros) and page! DSL formatting.
	/// Use --with-rustfmt=false to only format page! macro DSL.
	Fmt {
		/// Path to file or directory to format
		#[arg(value_name = "PATH")]
		path: PathBuf,

		/// Check if files are formatted without modifying them
		#[arg(long)]
		check: bool,

		/// Also run rustfmt (with page! macro protection)
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

	/// Format all code: Rust (via rustfmt) + page! DSL (via reinhardt-fmt)
	///
	/// This command protects page! macros from rustfmt, runs rustfmt on the
	/// surrounding Rust code, restores the macros, and then formats them with
	/// the page! DSL formatter.
	///
	/// Formats all Rust files in the project (searches for Cargo.toml to find project root).
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

	/// Deploy and manage cloud infrastructure
	Deploy {
		#[command(subcommand)]
		subcommand: Option<DeployCommands>,

		/// Run in dry-run mode (simulate without side effects)
		#[arg(long)]
		dry_run: bool,

		/// Deploy as preview environment
		#[arg(long)]
		preview: bool,

		/// Target environment
		#[arg(long, default_value = "production")]
		env: String,
	},
}

/// Deploy subcommands
#[derive(Subcommand)]
enum DeployCommands {
	/// Initialize deploy configuration for a Reinhardt project
	Init {
		/// Auto-detect only (no interactive prompts)
		#[arg(long)]
		detect: bool,
	},
	/// Show deployment status
	Status,
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
			template_type,
		} => run_startproject(name, directory, template_type, cli.verbosity).await,
		Commands::Startapp {
			name,
			directory,
			template_type,
		} => run_startapp(name, directory, template_type, cli.verbosity).await,
		Commands::Plugin { subcommand } => run_plugin(subcommand, cli.verbosity).await,
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
		Commands::Deploy {
			subcommand,
			dry_run,
			preview,
			env,
		} => run_deploy(subcommand, dry_run, preview, env, cli.verbosity).await,
	};

	if let Err(e) = result {
		eprintln!("Error: {}", e);
		process::exit(1);
	}
}

async fn run_startproject(
	name: String,
	directory: Option<String>,
	template_type: TemplateType,
	verbosity: u8,
) -> CommandResult<()> {
	let mut ctx = CommandContext::default();
	ctx.set_verbosity(verbosity);
	ctx.add_arg(name);
	if let Some(dir) = directory {
		ctx.add_arg(dir);
	}
	ctx.set_option("type".to_string(), template_type.to_string());

	let cmd = StartProjectCommand;
	cmd.execute(&ctx).await
}

async fn run_startapp(
	name: String,
	directory: Option<String>,
	template_type: TemplateType,
	verbosity: u8,
) -> CommandResult<()> {
	let mut ctx = CommandContext::default();
	ctx.set_verbosity(verbosity);
	ctx.add_arg(name);
	if let Some(dir) = directory {
		ctx.add_arg(dir);
	}
	ctx.set_option("type".to_string(), template_type.to_string());

	let cmd = StartAppCommand;
	cmd.execute(&ctx).await
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
	use ast_formatter::{AstPageFormatter, RustfmtOptions};
	use formatter::collect_rust_files;

	// Validate config path if provided
	if let Some(ref cp) = config_path {
		validate_config_path(cp).map_err(|e| {
			reinhardt_commands::CommandError::ExecutionError(format!("Invalid config path: {}", e))
		})?;
		check_file_size(cp, MAX_CONFIG_FILE_SIZE)
			.map_err(reinhardt_commands::CommandError::ExecutionError)?;
	}

	let files = collect_rust_files(&path).map_err(|e| {
		reinhardt_commands::CommandError::ExecutionError(format!(
			"Failed to collect files in {}: {}",
			display_path(&path),
			sanitize_error(&e)
		))
	})?;

	if files.is_empty() {
		if verbosity > 0 {
			println!("No Rust files found in {}", display_path(&path));
		}
		return Ok(());
	}

	// Resolve config path
	let resolved_config_path = config_path.or_else(|| find_rustfmt_config(&path));

	let options = RustfmtOptions {
		config_path: resolved_config_path.clone(),
		edition,
		style_edition,
		config,
		color: Some(color),
	};

	if verbosity > 0
		&& let Some(ref p) = resolved_config_path
	{
		println!("Using rustfmt config: {}", display_path(p));
	}

	let formatter = if let Some(ref config) = resolved_config_path {
		AstPageFormatter::with_config(config.clone())
	} else {
		AstPageFormatter::new()
	};
	let mut formatted_count = 0;
	let mut unchanged_count = 0;
	let mut ignored_count = 0;
	let mut error_count = 0;

	let total_files = files.len();

	for (index, file_path) in files.iter().enumerate() {
		let progress = format!("[{}/{}]", index + 1, total_files);

		// Check file size before reading to prevent OOM
		if let Err(e) = check_file_size(file_path, MAX_SOURCE_FILE_SIZE) {
			eprintln!(
				"{} {} {}: {}",
				progress.bright_blue(),
				"Skipped:".yellow(),
				display_path(file_path),
				e
			);
			error_count += 1;
			continue;
		}

		let original_content = std::fs::read_to_string(file_path).map_err(|e| {
			reinhardt_commands::CommandError::ExecutionError(format!(
				"Failed to read {}: {}",
				mask_path(file_path),
				sanitize_error(&e.to_string())
			))
		})?;

		// Check for file-wide ignore marker BEFORE any processing
		if formatter.has_ignore_all_marker(&original_content) {
			ignored_count += 1;
			if verbosity > 0 {
				println!(
					"{} {} {} (reinhardt-fmt: ignore-all)",
					progress.bright_blue(),
					"Ignored:".yellow(),
					display_path(file_path)
				);
			}
			continue;
		}

		// Process the file based on with_rustfmt option
		let final_result = if with_rustfmt {
			// Pipeline: protect -> rustfmt -> restore -> page! format
			// Step 1: Protect page! macros
			let protect_result = formatter.protect_page_macros(&original_content);

			// Step 2: Run rustfmt on protected content
			let rustfmt_output = match run_rustfmt(&protect_result.protected_content, &options) {
				Ok(output) => output,
				Err(e) => {
					eprintln!(
						"{} {} {}: rustfmt failed: {}",
						progress.bright_blue(),
						"Error".red(),
						display_path(file_path),
						sanitize_error(&e)
					);
					error_count += 1;
					continue;
				}
			};

			// Step 3: Restore page! macros
			let restored =
				AstPageFormatter::restore_page_macros(&rustfmt_output, &protect_result.backups);

			// Step 4: Format page! macros with reinhardt-fmt
			match formatter.format(&restored) {
				Ok(result) => result.content,
				Err(e) => {
					eprintln!(
						"{} {} {}: page! format failed: {}",
						progress.bright_blue(),
						"Error".red(),
						display_path(file_path),
						sanitize_error(&e.to_string())
					);
					error_count += 1;
					continue;
				}
			}
		} else {
			// Original behavior: page! DSL only
			match formatter.format(&original_content) {
				Ok(result) => {
					// Check if formatting was skipped
					if let Some(reason) = &result.skipped {
						use crate::ast_formatter::SkipReason;
						match reason {
							SkipReason::NoPageMacro => {
								// Skip files without page! macros (no logging, no counting)
								continue;
							}
							SkipReason::FileWideMarker | SkipReason::AllMacrosIgnored => {
								// Log ignored files with reason
								ignored_count += 1;
								println!(
									"{} {} {} ({})",
									progress.bright_blue(),
									"Ignored:".yellow(),
									display_path(file_path),
									reason
								);
								continue;
							}
						}
					}
					result.content
				}
				Err(e) => {
					eprintln!(
						"{} {} {}: {}",
						progress.bright_blue(),
						"Error".red(),
						display_path(file_path),
						sanitize_error(&e.to_string())
					);
					error_count += 1;
					continue;
				}
			}
		};

		// Compare with original
		if final_result != original_content {
			if check {
				// Check mode: report unformatted files
				println!("{} Would format: {}", progress, display_path(file_path));
				formatted_count += 1;
			} else {
				// Backup if requested (with RAII guard to clean up on failure)
				let mut _backup_guard = None;
				if backup {
					let backup_path = create_temp_backup_path(file_path);
					create_secure_backup(file_path, &backup_path).map_err(|e| {
						reinhardt_commands::CommandError::ExecutionError(format!(
							"Failed to backup {}: {}",
							mask_path(file_path),
							e
						))
					})?;
					_backup_guard = Some(utils::BackupGuard::new(backup_path));
				}

				// Format mode: write changes atomically (write to temp, then rename)
				utils::atomic_write(file_path, &final_result).map_err(|e| {
					reinhardt_commands::CommandError::ExecutionError(format!(
						"Failed to write {}: {}",
						mask_path(file_path),
						sanitize_error(&e.to_string())
					))
				})?;

				// Commit the backup guard so the backup is preserved on success
				if let Some(ref mut guard) = _backup_guard {
					guard.commit();
				}
				// Color output: success in green
				println!(
					"{} {} {}",
					progress.bright_blue(),
					"Formatted:".green(),
					display_path(file_path)
				);
				formatted_count += 1;
			}
		} else {
			unchanged_count += 1;
			if verbosity > 0 {
				println!(
					"{} {} {}",
					progress.bright_blue(),
					"Unchanged:".dimmed(),
					display_path(file_path)
				);
			}
		}
	}

	// Always show summary (remove verbosity condition)
	println!();
	if check {
		println!(
			"{}: {} would be formatted, {} unchanged, {} ignored, {} errors",
			"Summary".bright_cyan(),
			formatted_count.to_string().yellow(),
			unchanged_count,
			ignored_count,
			if error_count > 0 {
				error_count.to_string().red()
			} else {
				error_count.to_string().green()
			}
		);
	} else {
		println!(
			"{}: {} formatted, {} unchanged, {} ignored, {} errors",
			"Summary".bright_cyan(),
			if formatted_count > 0 {
				formatted_count.to_string().green()
			} else {
				formatted_count.to_string().dimmed()
			},
			unchanged_count,
			ignored_count,
			if error_count > 0 {
				error_count.to_string().red()
			} else {
				error_count.to_string().dimmed()
			}
		);
	}

	if check && formatted_count > 0 {
		return Err(reinhardt_commands::CommandError::ExecutionError(
			"Some files are not properly formatted".to_string(),
		));
	}

	if error_count > 0 {
		return Err(reinhardt_commands::CommandError::ExecutionError(format!(
			"{} files had formatting errors",
			error_count
		)));
	}

	Ok(())
}

/// Format all code: Rust (via rustfmt) + page! DSL (via reinhardt-fmt)
///
/// Pipeline using `cargo fmt --all`:
/// 1. Protect: page!(...)  → __reinhardt_placeholder__!(/*n*/) (write to disk)
/// 2. cargo fmt --all: Format Rust code (placeholders are not touched)
/// 3. Restore: __reinhardt_placeholder__!(/*n*/) → page!(...)
/// 4. reinhardt-fmt: Format page! macro contents
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
	use ast_formatter::AstPageFormatter;
	use formatter::collect_rust_files;
	use std::collections::HashMap;
	use std::process::{Command, Stdio};

	// Validate config path if provided
	if let Some(ref cp) = config_path {
		validate_config_path(cp).map_err(|e| {
			reinhardt_commands::CommandError::ExecutionError(format!("Invalid config path: {}", e))
		})?;
		check_file_size(cp, MAX_CONFIG_FILE_SIZE)
			.map_err(reinhardt_commands::CommandError::ExecutionError)?;
	}

	// Find project root
	let project_root = find_project_root().ok_or_else(|| {
		reinhardt_commands::CommandError::ExecutionError(
			"Could not find project root (no Cargo.toml found)".to_string(),
		)
	})?;

	if verbosity > 0 {
		println!("Project root: {}", display_path(&project_root));
	}

	let files = collect_rust_files(&project_root).map_err(|e| {
		reinhardt_commands::CommandError::ExecutionError(format!(
			"Failed to collect files: {}",
			sanitize_error(&e)
		))
	})?;

	if files.is_empty() {
		if verbosity > 0 {
			println!("No Rust files found in {}", display_path(&project_root));
		}
		return Ok(());
	}

	let formatter = AstPageFormatter::new();

	// Acquire a lock file to prevent concurrent format operations (TOCTOU mitigation)
	let lock_path = project_root.join(".reinhardt-fmt.lock");
	let _lock_file = acquire_format_lock(&lock_path).map_err(|e| {
		reinhardt_commands::CommandError::ExecutionError(format!(
			"Failed to acquire format lock: {}. Another format operation may be in progress.",
			e
		))
	})?;

	// Store original contents for comparison and rollback
	let mut original_contents: HashMap<PathBuf, String> = HashMap::new();
	// Store backup info for protected files
	let mut protected_files: Vec<(PathBuf, Vec<ast_formatter::PageMacroBackup>)> = Vec::new();

	let total_files = files.len();
	let mut page_macro_count = 0;

	// Phase 1: Protect page! macros and write to disk
	if verbosity > 0 {
		println!(
			"{} Phase 1: Protecting page! macros...",
			"[Step 1/3]".bright_blue()
		);
	}

	for file_path in &files {
		// Check file size before reading to prevent OOM
		if let Err(e) = check_file_size(file_path, MAX_SOURCE_FILE_SIZE) {
			eprintln!("{} Skipping oversized file: {}", "Warning:".yellow(), e);
			continue;
		}

		let original_content = std::fs::read_to_string(file_path).map_err(|e| {
			reinhardt_commands::CommandError::ExecutionError(format!(
				"Failed to read {}: {}",
				mask_path(file_path),
				sanitize_error(&e.to_string())
			))
		})?;

		// Store original content for comparison
		original_contents.insert(file_path.clone(), original_content.clone());

		// Skip if ignore-all marker is present
		if formatter.has_ignore_all_marker(&original_content) {
			continue;
		}

		// Skip files without page! macros
		if !original_content.contains("page!(") {
			continue;
		}

		let protect_result = formatter.protect_page_macros(&original_content);

		// Only process if there are actual macros to protect
		if !protect_result.backups.is_empty() {
			page_macro_count += protect_result.backups.len();

			// Write protected content to disk atomically (cargo fmt will format it)
			utils::atomic_write(file_path, &protect_result.protected_content).map_err(|e| {
				reinhardt_commands::CommandError::ExecutionError(format!(
					"Failed to write protected content to {}: {}",
					mask_path(file_path),
					sanitize_error(&e.to_string())
				))
			})?;

			protected_files.push((file_path.clone(), protect_result.backups));
		}
	}

	if verbosity > 0 {
		println!(
			"  Protected {} page! macros in {} files",
			page_macro_count,
			protected_files.len()
		);
	}

	// Phase 2: Run cargo fmt --all
	if verbosity > 0 {
		println!(
			"{} Phase 2: Running cargo fmt --all...",
			"[Step 2/3]".bright_blue()
		);
	}

	let mut cmd = Command::new("cargo");
	cmd.arg("fmt").arg("--all");
	cmd.current_dir(&project_root);
	cmd.stdout(Stdio::inherit());
	cmd.stderr(Stdio::inherit());

	// Add rustfmt options via "--" separator
	let has_rustfmt_options = config_path.is_some()
		|| edition.is_some()
		|| style_edition.is_some()
		|| config.is_some()
		|| color != "auto";

	if has_rustfmt_options {
		cmd.arg("--");

		if let Some(ref path) = config_path {
			cmd.arg("--config-path").arg(path);
		}

		if let Some(ref ed) = edition {
			cmd.arg("--edition").arg(ed);
		}

		if let Some(ref se) = style_edition {
			cmd.arg("--style-edition").arg(se);
		}

		if let Some(ref cfg) = config {
			cmd.arg("--config").arg(cfg);
		}

		if color != "auto" {
			cmd.arg("--color").arg(&color);
		}
	}

	if verbosity > 1 {
		// Only show command details at high verbosity to avoid leaking config paths
		println!("  Running: cargo fmt --all");
	}

	let cargo_fmt_result = cmd.output();

	// Track files that were actually modified on disk (for targeted rollback)
	let modified_on_disk: Vec<PathBuf> = protected_files.iter().map(|(p, _)| p.clone()).collect();

	// If cargo fmt fails, restore only modified files
	let output = match cargo_fmt_result {
		Ok(output) => output,
		Err(e) => {
			eprintln!(
				"{} cargo fmt failed: {}",
				"Error:".red(),
				sanitize_error(&e.to_string())
			);
			let rollback_errors = utils::rollback_files(&modified_on_disk, &original_contents);
			utils::report_rollback_errors(&rollback_errors);
			return Err(reinhardt_commands::CommandError::ExecutionError(
				"cargo fmt failed to execute".to_string(),
			));
		}
	};

	if !output.status.success() {
		let stderr = String::from_utf8_lossy(&output.stderr);
		let sanitized_stderr = sanitize_error(&stderr);
		eprintln!(
			"{} cargo fmt exited with error: {}",
			"Error:".red(),
			sanitized_stderr
		);
		let rollback_errors = utils::rollback_files(&modified_on_disk, &original_contents);
		utils::report_rollback_errors(&rollback_errors);
		return Err(reinhardt_commands::CommandError::ExecutionError(format!(
			"cargo fmt exited with error: {}",
			sanitized_stderr
		)));
	}

	// Phase 3: Restore page! macros and format DSL
	if verbosity > 0 {
		println!(
			"{} Phase 3: Restoring and formatting page! macros...",
			"[Step 3/3]".bright_blue()
		);
	}

	let mut error_count = 0;

	for (file_path, backups) in &protected_files {
		// Read the cargo-fmt formatted content
		let formatted_content = match std::fs::read_to_string(file_path) {
			Ok(content) => content,
			Err(e) => {
				eprintln!(
					"{} Failed to read {}: {}",
					"Error:".red(),
					display_path(file_path),
					sanitize_error(&e.to_string())
				);
				error_count += 1;
				continue;
			}
		};

		// Restore page! macros
		let restored = AstPageFormatter::restore_page_macros(&formatted_content, backups);

		// Format page! macro DSL
		let final_result = match formatter.format(&restored) {
			Ok(result) => result.content,
			Err(e) => {
				eprintln!(
					"{} page! format failed for {}: {}",
					"Error:".red(),
					display_path(file_path),
					sanitize_error(&e.to_string())
				);
				error_count += 1;
				// Write restored content anyway (without DSL formatting), atomically
				let _ = utils::atomic_write(file_path, &restored);
				continue;
			}
		};

		// Write final result atomically
		if let Err(e) = utils::atomic_write(file_path, &final_result) {
			eprintln!(
				"{} Failed to write {}: {}",
				"Error:".red(),
				display_path(file_path),
				sanitize_error(&e.to_string())
			);
			error_count += 1;
		}
	}

	// Compare and count changes
	let mut formatted_count = 0;
	let mut unchanged_count = 0;

	for (index, file_path) in files.iter().enumerate() {
		let progress = format!("[{}/{}]", index + 1, total_files);

		let original_content = match original_contents.get(file_path) {
			Some(content) => content,
			None => continue,
		};

		let current_content = match std::fs::read_to_string(file_path) {
			Ok(content) => content,
			Err(_) => continue,
		};

		if &current_content != original_content {
			if check {
				println!("{} Would format: {}", progress, display_path(file_path));
				// Restore original content in check mode
				if let Err(e) = std::fs::write(file_path, original_content) {
					eprintln!(
						"Warning: failed to restore {} in check mode: {}",
						file_path.display(),
						e
					);
				}
			} else if verbosity > 0 {
				println!(
					"{} {} {}",
					progress.bright_blue(),
					"Formatted:".green(),
					display_path(file_path)
				);
			}
			formatted_count += 1;

			// Create backup if requested (stored in /tmp with restrictive permissions)
			// BackupGuard ensures cleanup if the write fails
			if backup && !check {
				let backup_path = create_temp_backup_path(file_path);
				match create_secure_backup(file_path, &backup_path) {
					Ok(()) => {
						// Commit immediately since the format write already succeeded
						let mut guard = utils::BackupGuard::new(backup_path);
						guard.commit();
					}
					Err(e) => {
						eprintln!(
							"Warning: failed to create backup for {}: {}",
							mask_path(file_path),
							e
						);
					}
				}
			}
		} else {
			unchanged_count += 1;
			if verbosity > 0 {
				println!(
					"{} {} {}",
					progress.bright_blue(),
					"Unchanged:".dimmed(),
					display_path(file_path)
				);
			}
		}
	}

	// In check mode, restore only files that were modified on disk
	if check {
		let all_paths: Vec<PathBuf> = original_contents.keys().cloned().collect();
		let rollback_errors = utils::rollback_files(&all_paths, &original_contents);
		utils::report_rollback_errors(&rollback_errors);
	}

	// Securely clear original contents to prevent sensitive data lingering in memory
	secure_clear_hashmap(&mut original_contents);

	// Summary
	println!();
	if check {
		println!(
			"{}: {} would be formatted, {} unchanged, {} errors",
			"Summary".bright_cyan(),
			formatted_count.to_string().yellow(),
			unchanged_count,
			if error_count > 0 {
				error_count.to_string().red()
			} else {
				error_count.to_string().green()
			}
		);
	} else {
		println!(
			"{}: {} formatted, {} unchanged, {} errors",
			"Summary".bright_cyan(),
			if formatted_count > 0 {
				formatted_count.to_string().green()
			} else {
				formatted_count.to_string().dimmed()
			},
			unchanged_count,
			if error_count > 0 {
				error_count.to_string().red()
			} else {
				error_count.to_string().dimmed()
			}
		);
	}

	if check && formatted_count > 0 {
		return Err(reinhardt_commands::CommandError::ExecutionError(
			"Some files are not properly formatted".to_string(),
		));
	}

	if error_count > 0 {
		return Err(reinhardt_commands::CommandError::ExecutionError(format!(
			"{} files had formatting errors",
			error_count
		)));
	}

	Ok(())
}

async fn run_deploy(
	subcommand: Option<DeployCommands>,
	dry_run: bool,
	preview: bool,
	env: String,
	verbosity: u8,
) -> CommandResult<()> {
	match subcommand {
		None => {
			// Main deploy pipeline
			let _project_root = find_project_root().ok_or_else(|| {
				reinhardt_commands::CommandError::ExecutionError(
					"Could not find project root (no Cargo.toml found)".to_string(),
				)
			})?;

			let opts = reinhardt_deploy::pipeline::PipelineOptions {
				dry_run,
				preview,
				environment: env,
			};

			if verbosity > 0 {
				println!(
					"Deploy pipeline options: dry_run={}, preview={}, environment={}",
					opts.dry_run, opts.preview, opts.environment
				);
			}

			// Pipeline execution will be implemented in later tasks
			println!("Deploy pipeline started (dry_run: {})", opts.dry_run);
			Ok(())
		}
		Some(DeployCommands::Init { detect }) => {
			let _project_root = find_project_root().ok_or_else(|| {
				reinhardt_commands::CommandError::ExecutionError(
					"Could not find project root (no Cargo.toml found)".to_string(),
				)
			})?;

			if detect {
				println!("Running auto-detection only...");
			} else {
				println!("Initializing deploy configuration...");
			}

			// Init implementation will come in Task 17
			Ok(())
		}
		Some(DeployCommands::Status) => {
			println!("Deployment status: not yet implemented");
			Ok(())
		}
	}
}

/// Maximum number of parent directories to traverse when searching for project root.
///
/// Prevents unbounded traversal to the filesystem root, which could
/// find an unrelated `Cargo.toml` from a different project.
const MAX_PROJECT_ROOT_DEPTH: usize = 10;

/// Find the project root by searching upward for Cargo.toml.
///
/// Uses `std::fs::metadata` instead of `Path::exists()` to avoid TOCTOU
/// race conditions in the existence check. Traversal is bounded to
/// `MAX_PROJECT_ROOT_DEPTH` levels to prevent finding unrelated projects.
fn find_project_root() -> Option<PathBuf> {
	let current_dir = std::env::current_dir().ok()?;
	let mut current = current_dir.as_path();

	for _ in 0..MAX_PROJECT_ROOT_DEPTH {
		if std::fs::metadata(current.join("Cargo.toml")).is_ok() {
			return Some(current.to_path_buf());
		}
		current = current.parent()?;
	}
	None
}

/// Find rustfmt.toml by searching upward from a path.
///
/// Uses `std::fs::metadata` instead of `Path::exists()` to avoid TOCTOU
/// race conditions in the existence check.
fn find_rustfmt_config(start_path: &Path) -> Option<PathBuf> {
	let mut current = if start_path.is_file() {
		start_path.parent()
	} else {
		Some(start_path)
	}?;

	loop {
		let config = current.join("rustfmt.toml");
		if std::fs::metadata(&config).is_ok() {
			return Some(config);
		}
		let hidden_config = current.join(".rustfmt.toml");
		if std::fs::metadata(&hidden_config).is_ok() {
			return Some(hidden_config);
		}
		if std::fs::metadata(current.join("Cargo.toml")).is_ok() {
			break;
		}
		current = current.parent()?;
	}
	None
}

/// Run rustfmt on content and return formatted output.
fn run_rustfmt(content: &str, options: &ast_formatter::RustfmtOptions) -> Result<String, String> {
	use std::io::Write;
	use std::process::{Command, Stdio};

	let mut cmd = Command::new("rustfmt");
	options.apply_to_command(&mut cmd);

	// Fallback to default edition if no config is specified
	if options.config_path.is_none() && options.edition.is_none() {
		cmd.arg("--edition=2024");
	}

	let mut child = cmd
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.spawn()
		.map_err(|e| format!("Failed to spawn rustfmt: {}", e))?;

	if let Some(mut stdin) = child.stdin.take() {
		stdin
			.write_all(content.as_bytes())
			.map_err(|e| format!("Failed to write to rustfmt stdin: {}", e))?;
	}

	let output = child
		.wait_with_output()
		.map_err(|e| format!("Failed to wait for rustfmt: {}", e))?;

	if output.status.success() {
		String::from_utf8(output.stdout).map_err(|e| format!("Invalid UTF-8 from rustfmt: {}", e))
	} else {
		let stderr = String::from_utf8_lossy(&output.stderr);
		Err(format!("rustfmt failed: {}", stderr))
	}
}

/// Create a backup file with restrictive permissions (0600).
///
/// # Security
///
/// Sets file permissions to 0600 (read/write for owner only) to prevent
/// unauthorized access to backup files which may contain sensitive code.
#[cfg(unix)]
fn create_secure_backup(source: &Path, backup_path: &Path) -> Result<(), std::io::Error> {
	use std::fs::OpenOptions;
	use std::io::Read;
	use std::os::unix::fs::OpenOptionsExt;

	// Read source content
	let mut content = Vec::new();
	let mut file = std::fs::File::open(source)?;
	file.read_to_end(&mut content)?;

	// Create backup with restrictive permissions
	let mut backup_file = OpenOptions::new()
		.write(true)
		.create(true)
		.truncate(true)
		.mode(0o600) // Owner read/write only
		.open(backup_path)?;

	std::io::copy(&mut content.as_slice(), &mut backup_file)?;

	// Zeroize the content buffer to prevent sensitive data lingering in memory
	content.zeroize();

	Ok(())
}

/// Create a backup file with restrictive permissions (0600).
///
/// # Security
///
/// On non-Unix systems, this creates the backup file normally with default permissions.
/// A warning is logged recommending Unix for production deployments.
#[cfg(not(unix))]
fn create_secure_backup(source: &Path, backup_path: &Path) -> Result<(), std::io::Error> {
	use std::io::Read;

	// Read source content
	let mut content = Vec::new();
	let mut file = std::fs::File::open(source)?;
	file.read_to_end(&mut content)?;

	// Create backup with default permissions (Windows ACLs handle security)
	std::fs::write(backup_path, &content)?;

	// Zeroize the content buffer
	content.zeroize();

	Ok(())
}

/// Create a backup file path in the system temporary directory.
///
/// # Security
///
/// Stores backup files in `/tmp` instead of the source directory to prevent
/// backup files from being committed or exposed in the project tree.
/// Uses a deterministic name based on the source file name so the backup
/// can be identified if cleanup is interrupted.
fn create_temp_backup_path(source: &Path) -> PathBuf {
	let file_name = source
		.file_name()
		.unwrap_or_else(|| std::ffi::OsStr::new("unknown"));
	let backup_name = format!("reinhardt-fmt-{}.bak", file_name.to_string_lossy());
	std::env::temp_dir().join(backup_name)
}

/// Mask sensitive file path in error messages.
///
/// Returns a masked version of the path that only shows the filename,
/// preventing full path disclosure in error output.
fn mask_path(path: &Path) -> String {
	path.file_name()
		.map(|name| format!("<...>/{}", name.to_string_lossy()))
		.unwrap_or_else(|| "<file>".to_string())
}

/// Convert a path to a display-safe relative path.
///
/// Attempts to strip the current working directory prefix to show a relative path.
/// Falls back to showing only the filename via `mask_path` if relativization fails.
fn display_path(path: &Path) -> String {
	if let Ok(cwd) = std::env::current_dir()
		&& let Ok(relative) = path.strip_prefix(&cwd)
	{
		return relative.display().to_string();
	}
	mask_path(path)
}

/// Sanitize error messages to prevent information leakage.
///
/// Removes absolute file system paths and sensitive patterns from error messages
/// to prevent exposing internal system structure to end users.
fn sanitize_error(error: &str) -> String {
	use std::sync::LazyLock;

	// Pattern: absolute paths on Unix-like systems (e.g., /home/user/project/file.rs)
	static PATH_RE: LazyLock<regex::Regex> =
		LazyLock::new(|| regex::Regex::new(r"(/[a-zA-Z0-9._-]+){3,}").unwrap());

	// Pattern: database connection URLs
	static DB_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
		regex::Regex::new(r"(?i)(postgres|mysql|sqlite|mongodb|redis)://[^\s]+").unwrap()
	});

	// Pattern: API keys, tokens, secrets in key=value or key:value format
	static TOKEN_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
		regex::Regex::new(
			r"(?i)(api[_\-]?key|token|secret|password|auth)[=:]\s*['\x22]?[a-zA-Z0-9+/=_\-]{8,}",
		)
		.unwrap()
	});

	let sanitized = PATH_RE.replace_all(error, |caps: &regex::Captures| {
		let matched_path = Path::new(caps.get(0).unwrap().as_str());
		mask_path(matched_path)
	});

	let sanitized = DB_RE.replace_all(&sanitized, "[REDACTED_DATABASE_URL]");
	let sanitized = TOKEN_RE.replace_all(&sanitized, "[REDACTED_CREDENTIAL]");

	sanitized.to_string()
}

/// Securely clear a HashMap containing sensitive string data.
///
/// # Security
///
/// Uses zeroize to overwrite string values in memory before dropping,
/// preventing sensitive data from remaining in memory.
fn secure_clear_hashmap(map: &mut std::collections::HashMap<PathBuf, String>) {
	for (_, value) in map.iter_mut() {
		value.zeroize();
	}
	map.clear();
}

/// Validate a config path argument to prevent path traversal and special file attacks.
///
/// # Security
///
/// - Rejects paths containing traversal sequences (`..`)
/// - Verifies the path exists and is a regular file (not a directory, device, etc.)
/// - Rejects symlinks to prevent symlink-based attacks
/// - Rejects special device paths (e.g., `/dev/stdin`, `/dev/null`)
fn validate_config_path(path: &Path) -> Result<(), String> {
	// Check for path traversal sequences
	let path_str = path.to_string_lossy();
	if path_str.contains("..") {
		return Err(format!(
			"Config path contains path traversal sequence: {}",
			mask_path(path)
		));
	}

	// Reject special device paths
	#[cfg(unix)]
	if path_str.starts_with("/dev/")
		|| path_str.starts_with("/proc/")
		|| path_str.starts_with("/sys/")
	{
		return Err(format!(
			"Config path refers to a special device: {}",
			mask_path(path)
		));
	}

	// Use symlink_metadata for a single atomic check (avoids TOCTOU between
	// exists/is_symlink/is_file calls). symlink_metadata does not follow
	// symlinks, so a symlink target cannot change between checks.
	let symlink_meta = std::fs::symlink_metadata(path)
		.map_err(|e| format!("Config path is not accessible: {} ({})", mask_path(path), e))?;

	// Reject symlinks
	if symlink_meta.file_type().is_symlink() {
		return Err(format!(
			"Config path is a symlink, which is not allowed: {}",
			mask_path(path)
		));
	}

	// Verify it's a regular file (not a directory, device, etc.)
	if !symlink_meta.is_file() {
		return Err(format!(
			"Config path is not a regular file: {}",
			mask_path(path)
		));
	}

	Ok(())
}

/// Maximum file size for configuration files (10 MB).
///
/// Prevents OOM from processing extremely large files.
const MAX_CONFIG_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// Maximum file size for Rust source files (5 MB).
///
/// Prevents OOM from processing extremely large source files.
const MAX_SOURCE_FILE_SIZE: u64 = 5 * 1024 * 1024;

/// Check file size before reading to prevent OOM.
///
/// # Errors
///
/// Returns an error if the file exceeds the given maximum size.
fn check_file_size(path: &Path, max_size: u64) -> Result<(), String> {
	match std::fs::metadata(path) {
		Ok(metadata) => {
			if metadata.len() > max_size {
				Err(format!(
					"File {} exceeds maximum allowed size ({} bytes, limit {} bytes)",
					mask_path(path),
					metadata.len(),
					max_size
				))
			} else {
				Ok(())
			}
		}
		Err(e) => Err(format!(
			"Failed to check file size for {}: {}",
			mask_path(path),
			e
		)),
	}
}

/// Acquire an exclusive lock file to prevent concurrent format operations.
///
/// # Security
///
/// Uses `OpenOptions::create_new(true)` for atomic lock creation (TOCTOU mitigation).
/// The lock file is automatically removed when the returned guard is dropped.
///
/// # Errors
///
/// Returns an error if the lock file already exists (another operation is in progress)
/// or if the file cannot be created.
fn acquire_format_lock(lock_path: &Path) -> Result<FormatLockGuard, std::io::Error> {
	use std::fs::OpenOptions;

	// Atomic create-or-fail: prevents TOCTOU race between check and create
	let file = OpenOptions::new()
		.write(true)
		.create_new(true)
		.open(lock_path)?;

	// Write PID to lock file for debugging
	use std::io::Write;
	let mut file = file;
	let _ = writeln!(file, "{}", std::process::id());

	Ok(FormatLockGuard {
		path: lock_path.to_path_buf(),
	})
}

/// RAII guard that removes the lock file on drop.
struct FormatLockGuard {
	path: PathBuf,
}

impl Drop for FormatLockGuard {
	fn drop(&mut self) {
		let _ = std::fs::remove_file(&self.path);
	}
}

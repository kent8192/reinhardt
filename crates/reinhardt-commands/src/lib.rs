#![warn(missing_docs)]
//! # Reinhardt Management Commands
//!
//! Django-style management command framework for Reinhardt.
//!
//! ## Features
//!
//! - **BaseCommand**: Trait for creating custom commands
//! - **Standard Commands**: migrate, shell, runserver, etc.
//! - **Argument Parsing**: Clap-based argument handling
//! - **Command Registry**: Automatic command discovery
//! - **Interactive Mode**: Support for interactive prompts
//! - **Colored Output**: Rich terminal output
//! - **AST-Based Code Generation**: Robust code generation using Abstract Syntax Trees
//! - **Auto-Reload**: Built-in hot-reload for the development server (server + wasm)
//! - **Tera Template Engine**: Powerful template rendering for project/app generation
//!
//! ## Example
//!
//! ```rust,no_run
//! # use reinhardt_commands::{BaseCommand, CommandContext, CommandResult};
//! # #[tokio::main]
//! # async fn main() {
//! // struct MyCommand;
//! //
//! // #[async_trait]
//! // impl BaseCommand for MyCommand {
//! //     fn name(&self) -> &str {
//! //         "mycommand"
//! //     }
//! //
//! //     async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
//! //         println!("Hello from my command!");
//! //         Ok(())
//! //     }
//! // }
//! # }
//! ```
//!
//! ## Template System
//!
//! The command framework uses [Tera](https://keats.github.io/tera/) for template rendering.
//! Tera is a powerful template engine inspired by Jinja2/Django templates.
//!
//! ### Template Context
//!
//! Templates receive context variables through `TemplateContext`:
//!
//! ```rust
//! use reinhardt_commands::TemplateContext;
//!
//! let mut context = TemplateContext::new();
//! context.insert("project_name", "my_project").unwrap();
//! context.insert("version", "1.0.0").unwrap();
//! context.insert("features", vec!["auth", "admin"]).unwrap();  // Any Serialize type
//! ```
//!
//! ### Template Variables
//!
//! The `insert` method accepts any type implementing `serde::Serialize`
//! and returns `Result<(), serde_json::Error>`:
//!
//! - Strings: `context.insert("name", "value")?`
//! - Numbers: `context.insert("count", 42)?`
//! - Booleans: `context.insert("enabled", true)?`
//! - Collections: `context.insert("items", vec!["a", "b"])?`
//! - Custom types: `context.insert("data", &my_struct)?`
//!
//! ## AST-Based Code Generation
//!
//! The `startapp` command uses Abstract Syntax Tree (AST) parsing via `syn` and `quote`
//! for robust code generation and modification. This approach provides several benefits:
//!
//! ### Benefits of AST Approach
//!
//! 1. **Syntax Awareness**: Understands code structure, not just text patterns
//!    - Correctly distinguishes `pub mod app;` from `// pub mod app;` (commented)
//!    - Handles variations in whitespace and formatting automatically
//!
//! 2. **Duplicate Detection**: Structurally detects existing declarations
//!    - Avoids adding duplicate module declarations
//!    - Works correctly even with complex existing code
//!
//! 3. **Consistent Formatting**: Uses `prettyplease` for standardized output
//!    - Ensures consistent code style across generated files
//!    - Integrates well with rustfmt
//!
//! ### Example: apps.rs Generation
//!
//! When you run `startapp myapp`, the command:
//! 1. Parses existing `src/apps.rs` using `syn::parse_file`
//! 2. Checks for existing `pub mod myapp;` declaration structurally
//! 3. Adds new module and use declarations if not present
//! 4. Formats output with `prettyplease::unparse`
//!
//! ```rust,ignore
//! // Generated apps.rs
//! pub mod myapp;
//! pub use myapp::MyappConfig;
//! ```
//!
//! This is more reliable than string-based approaches that can be confused by
//! comments, unusual formatting, or complex code patterns.
//!
//! ## Auto-Reload for Development Server
//!
//! The `runserver` command reloads automatically on file changes. No external
//! tool (such as `cargo-watch` or `bacon`) is required — the watcher is built
//! into the `autoreload` feature.
//!
//! ```text
//! cargo run --bin manage -- runserver --with-pages
//! ```
//!
//! Edit any Rust source file (server-side or wasm-side) and the bundle plus
//! the server are rebuilt in place. Pass `--noreload` to disable auto-reload
//! entirely, or `--no-wasm-rebuild` to keep server reload but manage the wasm
//! build yourself. The server restart success log is emitted only after the
//! respawned child accepts connections at the advertised development address.
//!
//! See [`runserver_hooks`] for the full hot-reload runbook and failure modes.

/// Base command trait and argument/option definitions.
pub mod base;
/// Built-in management commands (migrate, runserver, shell, etc.).
pub mod builtin;
/// CLI argument parsing and command dispatch.
pub mod cli;
/// Static file collection command.
pub mod collectstatic;
/// Command execution context (settings, output, verbosity).
pub mod context;
/// Superuser creation command.
#[cfg(feature = "auth")]
pub(crate) mod createsuperuser;
/// Debounced file-system watcher for hot-reload (replaces inline watcher).
#[cfg(feature = "autoreload")]
#[doc(hidden)]
pub mod debounced_watcher;
/// Embedded Tera templates for project/app scaffolding.
pub mod embedded_templates;
/// Code formatting utilities for generated code.
pub mod formatter;
/// Internationalization commands (makemessages, compilemessages).
pub mod i18n_commands;
/// Project introspection command for platform metadata discovery.
#[cfg(feature = "introspect")]
pub mod introspect;
/// Local development infrastructure management.
pub mod local_infra;
/// Email testing command.
pub mod mail_commands;
/// Terminal output wrapper with styling support.
pub mod output;
/// Compile-free static Pages hot patch support.
#[cfg(all(feature = "autoreload", feature = "pages"))]
mod page_hot_patch;
/// Plugin management commands.
#[cfg(feature = "plugins")]
pub mod plugin_commands;
mod process;
/// Project dependency configuration commands.
pub mod project_config;
/// Command registry for discovery and dispatch.
pub mod registry;
/// Runserver lifecycle hooks for concurrent services and pre-listen validation.
#[cfg(feature = "server")]
pub mod runserver_hooks;
/// Hot-reload server rebuild pipeline (cargo build + child process swap).
#[cfg(feature = "autoreload")]
#[doc(hidden)]
pub mod server_rebuild_pipeline;
/// Source-tree enumeration for hot-reload watch targets.
#[cfg(feature = "autoreload")]
#[doc(hidden)]
pub mod source_roots;
/// Project and app scaffolding commands (startproject, startapp).
pub mod start_commands;
/// Template-based code generation utilities.
pub mod template;
/// Template source abstraction over embedded and filesystem assets.
pub mod template_source;
/// WASM build tooling for client-side compilation.
pub mod wasm_builder;
/// Hot-reload WASM rebuild pipeline (timing + structured logging wrapper).
#[cfg(all(feature = "autoreload", feature = "pages"))]
#[doc(hidden)]
pub mod wasm_rebuild_pipeline;
/// Development server welcome page.
pub mod welcome_page;

/// Internal test surface for the hot-reload integration tests.
///
/// This module is intentionally `#[doc(hidden)]` and re-exports the otherwise
/// crate-private hot-reload pieces so that integration tests living under
/// `tests/` (a separate crate target) can drive them end-to-end. It is not
/// part of the public API and may change without notice.
#[cfg(feature = "autoreload")]
#[doc(hidden)]
pub mod __hot_reload_test_api {
	pub use crate::debounced_watcher::{
		DEBOUNCE_WINDOW, RebuildTargets, WatcherConfig, debounce_next, is_relevant_change,
		rebuild_targets_for_paths, run_rebuild_for_paths, run_watcher,
	};
	#[cfg(all(feature = "autoreload", feature = "pages"))]
	pub use crate::page_hot_patch::render_static_page_patch;
	pub use crate::server_rebuild_pipeline::{ServerRebuildOutcome, ServerRebuildPipeline};
	pub use crate::source_roots::SourceRoots;
	#[cfg(feature = "pages")]
	pub use crate::wasm_rebuild_pipeline::{WasmRebuildOutcome, WasmRebuildPipeline};

	/// HR-8 regression entry point (#4244): exercise only the
	/// autoreload-parent validation step. Wraps the crate-private
	/// `RunServerCommand::validate_hooks_only_for_tests` so the surface stays
	/// inside `__hot_reload_test_api` instead of widening
	/// `RunServerCommand`'s public API.
	#[cfg(feature = "server")]
	pub async fn validate_hooks_only(ctx: &crate::CommandContext) -> crate::CommandResult<()> {
		crate::RunServerCommand::validate_hooks_only_for_tests(ctx).await
	}
}

use thiserror::Error;

pub use base::{BaseCommand, CommandArgument, CommandOption};
#[cfg(feature = "migrations")]
pub use builtin::MakeMigrationsCommand;
#[cfg(feature = "routers")]
pub use builtin::ShowUrlsCommand;
pub use builtin::{CheckCommand, CheckDiCommand, MigrateCommand, RunServerCommand, ShellCommand};
#[cfg(feature = "server")]
pub use cli::start_server;
pub use cli::{
	Cli, Commands, auto_register_router, execute_from_command_line,
	execute_from_command_line_with_registry, execute_from_command_line_with_registry_and_settings,
	execute_from_command_line_with_settings, run_command, run_command_with_registry,
};
pub use collectstatic::{CollectStaticCommand, CollectStaticOptions, CollectStaticStats};
pub use context::CommandContext;
pub use i18n_commands::{CompileMessagesCommand, MakeMessagesCommand};
#[cfg(feature = "introspect")]
pub use introspect::IntrospectCommand;
pub use mail_commands::SendTestEmailCommand;
pub use output::OutputWrapper;
pub use project_config::{ConfigureCommand, ReinhardtDependencySelection};
pub use registry::CommandRegistry;
#[cfg(feature = "server")]
pub use runserver_hooks::{RunserverContext, RunserverHook, RunserverHookRegistration};
pub use start_commands::{StartAppCommand, StartProjectCommand};
pub use template::{TemplateCommand, TemplateContext, generate_secret_key, to_camel_case};
pub use wasm_builder::{
	WasmBuildConfig, WasmBuildError, WasmBuildOutput, WasmBuilder, check_wasm_tools_installed,
	detect_cdylib_in_cargo_toml, detect_cdylib_in_cargo_toml_content, is_wasm_stale,
	latest_source_mtime,
};
pub use welcome_page::WelcomePage;

#[cfg(feature = "plugins")]
pub use plugin_commands::{
	PluginDisableCommand, PluginEnableCommand, PluginInfoCommand, PluginInstallCommand,
	PluginListCommand, PluginRemoveCommand, PluginSearchCommand, PluginUpdateCommand,
};

/// Errors that can occur during management command execution.
#[derive(Debug, Error)]
pub enum CommandError {
	/// The requested command was not found in the registry.
	#[error("Command not found: {0}")]
	NotFound(String),

	/// The provided command arguments are invalid.
	#[error("Invalid arguments: {0}")]
	InvalidArguments(String),

	/// A runtime error occurred during command execution.
	#[error("Execution error: {0}")]
	ExecutionError(String),

	/// An I/O error occurred.
	#[error("IO error: {0}")]
	IoError(#[from] std::io::Error),

	/// An error occurred while parsing command input.
	#[error("Parse error: {0}")]
	ParseError(String),

	/// A template rendering error occurred.
	#[error("Template error: {0}")]
	TemplateError(String),
}

impl From<tera::Error> for CommandError {
	fn from(err: tera::Error) -> Self {
		CommandError::TemplateError(err.to_string())
	}
}

impl From<String> for CommandError {
	fn from(err: String) -> Self {
		CommandError::ExecutionError(err)
	}
}

impl From<serde_json::Error> for CommandError {
	fn from(err: serde_json::Error) -> Self {
		CommandError::ExecutionError(format!("Serialization error: {}", err))
	}
}

/// A specialized `Result` type for management command operations.
pub type CommandResult<T> = std::result::Result<T, CommandError>;

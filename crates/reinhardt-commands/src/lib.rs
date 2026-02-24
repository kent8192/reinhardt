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
//! - **Auto-Reload**: Development server auto-reload with bacon integration
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
//! The `runserver` command supports automatic reloading when code changes are detected,
//! using bacon for complete rebuild and restart functionality.
//!
//! ### Using bacon
//!
//! Install bacon:
//!
//! ```bash
//! cargo install --locked bacon
//! ```
//!
//! Run the development server with auto-reload:
//!
//! ```bash
//! # Using bacon directly
//! bacon runserver
//!
//! # Or using cargo make
//! cargo make watch
//! ```
//!
//! ### How It Works
//!
//! Bacon provides a background code checker that:
//! 1. Detects file changes in `src/`, `Cargo.toml`, and other watched paths
//! 2. Automatically runs the configured job (check, clippy, test, runserver, etc.)
//! 3. Displays build output and errors in real-time
//! 4. Supports keyboard shortcuts for switching between different jobs
//!
//! ### Configuration
//!
//! Bacon can be configured via `bacon.toml` in the project root. See the bacon
//! documentation for more details: <https://dystroy.org/bacon/>

pub mod base;
pub mod builtin;
pub mod cli;
pub mod collectstatic;
pub mod context;
pub mod embedded_templates;
pub mod formatter;
pub mod i18n_commands;
pub mod mail_commands;
pub mod output;
#[cfg(feature = "plugins")]
pub mod plugin_commands;
pub mod registry;
pub mod start_commands;
pub mod template;
pub mod wasm_builder;
pub mod welcome_page;

use thiserror::Error;

pub use base::{BaseCommand, CommandArgument, CommandOption};
#[cfg(feature = "migrations")]
pub use builtin::MakeMigrationsCommand;
#[cfg(feature = "routers")]
pub use builtin::ShowUrlsCommand;
pub use builtin::{CheckCommand, CheckDiCommand, MigrateCommand, RunServerCommand, ShellCommand};
pub use cli::{Cli, Commands, execute_from_command_line, run_command};
pub use collectstatic::{CollectStaticCommand, CollectStaticOptions, CollectStaticStats};
pub use context::CommandContext;
pub use i18n_commands::{CompileMessagesCommand, MakeMessagesCommand};
pub use mail_commands::SendTestEmailCommand;
pub use output::OutputWrapper;
pub use registry::CommandRegistry;
pub use start_commands::{StartAppCommand, StartProjectCommand};
pub use template::{TemplateCommand, TemplateContext, generate_secret_key, to_camel_case};
pub use wasm_builder::{
	WasmBuildConfig, WasmBuildError, WasmBuildOutput, WasmBuilder, check_wasm_tools_installed,
};
pub use welcome_page::WelcomePage;

#[cfg(feature = "plugins")]
pub use plugin_commands::{
	PluginDisableCommand, PluginEnableCommand, PluginInfoCommand, PluginInstallCommand,
	PluginListCommand, PluginRemoveCommand, PluginSearchCommand, PluginUpdateCommand,
};

#[derive(Debug, Error)]
pub enum CommandError {
	#[error("Command not found: {0}")]
	NotFound(String),

	#[error("Invalid arguments: {0}")]
	InvalidArguments(String),

	#[error("Execution error: {0}")]
	ExecutionError(String),

	#[error("IO error: {0}")]
	IoError(#[from] std::io::Error),

	#[error("Parse error: {0}")]
	ParseError(String),

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

pub type CommandResult<T> = std::result::Result<T, CommandError>;

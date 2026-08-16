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
//! - **Data Fixtures**: Django-compatible `dumpdata`, transaction-safe `loaddata`,
//!   binary fixture values, many-to-many arrays, and seed hooks
//! - **Schema Inspection**: Django-compatible `inspectdb` with deterministic
//!   PostgreSQL, MySQL, and SQLite model generation
//! - **Native Database Shell**: `dbshell` launches `psql`, `mysql`, or `sqlite3`
//!   with inherited terminal streams and child-scoped credentials
//! - **AST-Based Code Generation**: Robust code generation using Abstract Syntax Trees
//! - **Auto-Reload**: Built-in hot-reload for the development server (server + wasm)
//! - **Native Protocol Launch**: Aggregated HTTP, WebSocket, and gRPC startup
//! - **Tera Template Engine**: Powerful template rendering for project/app generation
//!
//! ## Squashing Migrations
//!
//! The `squashmigrations` command supports Django-compatible range syntax:
//!
//! ```text
//! manage squashmigrations APP_LABEL MIGRATION_NAME
//! manage squashmigrations APP_LABEL START_MIGRATION MIGRATION_NAME
//! ```
//!
//! Exact migration names and unambiguous prefixes are accepted. Resolution
//! rejects ambiguous prefixes, branched ancestry, and ranges that are not
//! continuous same-application ancestor chains. Dependencies entering the
//! selected range are preserved.
//!
//! Interactive execution prompts before creating the file. Pass `--no-input`
//! or its `--noinput` alias for automation, `--no-optimize` to preserve the
//! exact operation sequence, and `--no-header` to omit the generated-file
//! header. A descriptive `--squashed-name release_window` becomes a name such
//! as `0001_release_window`. In a Cargo workspace, pass
//! `--migrations-dir path/to/member/migrations` to select the target member's
//! migration root explicitly.
//!
//! Optimization never crosses an operation barrier. Data operations, renames,
//! constraints, indexes, bulk operations, custom operations, and any operation
//! without a proven schema reduction retain their order. The command validates
//! and renders the entire migration before prompting, creates a new file
//! without overwriting an existing destination, and attempts anchored cleanup
//! after a failed write. If cleanup also fails, the error reports both
//! failures. It reads migration sources only, so no database connection is
//! required.
//!
//! ## Migration Visibility
//!
//! `showmigrations` reads one immutable catalog and recorder snapshot, then
//! displays either application-grouped `[X]` / `[ ]` state or the selected
//! dependency order. It treats an absent recorder table as an empty applied
//! set and never creates migration history. `--list` / `-l` and `--plan` /
//! `-p` are mutually exclusive, list mode is the default, and verbosity level
//! two includes recorded timestamps. Application filtering retains transitive
//! cross-application dependencies.
//!
//! `sqlmigrate APP MIGRATION` accepts an exact name or unique prefix and uses
//! the SQL planner shared with migration execution. `--backwards` reconstructs
//! both sides of the migration before rendering rollback SQL. The complete
//! uncolored script is buffered before one stdout write; no schema or history
//! statement is executed. An irreversible rollback or late planning error
//! therefore emits no partial script.
//!
//! Both commands accept `--database ALIAS`. Without `--database-url`, the alias
//! is looked up in configured settings. A URL override bypasses alias lookup
//! and connects directly while retaining the alias as a safe diagnostic label;
//! settings are not modified. Diagnostics redact URL credentials and
//! sensitive-looking aliases. Transaction wrappers are emitted only for an
//! atomic migration plan on a backend that supports transactional DDL, so
//! MySQL DDL remains unwrapped. SQLite emits table recreation SQL when the
//! requested alteration requires it.
//!
//! ```text
//! manage showmigrations polls --plan --database default
//! manage sqlmigrate polls 0002 --database default
//! manage sqlmigrate polls 0002 --backwards --database default
//! ```
//!
//! ## Application Contract Export
//!
//! With the `contract` feature enabled, a management binary dispatched through
//! [`execute_from_command_line_with_resolved_settings`] can export deterministic
//! models, migrations, routes, and settings metadata:
//!
//! ```text
//! manage contract export --format json
//! ```
//!
//! See the canonical [application contract documentation](https://reinhardt-web.dev/docs/application-contract/)
//! for the schema and field rules.
//!
//! ## Application Contract Verification
//!
//! With the `contract` feature enabled, `manage verify` runs a contract check
//! with human-readable output by default and a versioned report when passed
//! `--format json`:
//!
//! ```text
//! cargo run --bin manage -- verify
//! cargo run --bin manage -- verify --format json
//! ```
//!
//! The command first replays the consumer Cargo check captured by the generated
//! launcher. A spawn failure or non-zero Cargo status stops before contract
//! collection. After a successful check, schema, authorization, and settings
//! validators run independently and render stable finding codes, including
//! `schema.missing_migration`, `schema.unapplied_migration`,
//! `authorization.missing_declaration`, and the four `settings.*` codes.
//! Applied-migration coverage is optional; when no applied snapshot is
//! available, only that coverage check is omitted.
//!
//! JSON reports are the only stdout output in JSON mode; Cargo output and
//! diagnostics use stderr. Endpoint checks materialize synchronous in-memory route
//! registrations without installing a global router. Asynchronous factories
//! are rejected without polling, and verification does not initialize
//! dependency injection or open a database. Settings
//! validation uses the builder's typed-coercion mode and redacts values,
//! concrete map keys, and parser/deserializer diagnostics from findings. Use
//! `cargo run` for the supported freshness path; invoking a prebuilt `manage`
//! binary directly does not detect that it is stale.
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
//!
//! ## Rust Management Shell
//!
//! The `shell` feature enables a stateful Rust evaluator. The facade exposes it
//! to generated projects as the opt-in `commands-shell` feature; generated
//! defaults intentionally omit it. Projects opting in provide a
//! [`ShellConfig`], call [`shell_runtime_hook`] from the outer native `main`
//! before constructing Tokio, and dispatch through
//! [`execute_from_command_line_with_migration_settings_and_shell`].
//!
//! ```rust,ignore
//! #[cfg(not(target_arch = "wasm32"))]
//! fn main() {
//!     reinhardt::commands::shell_runtime_hook();
//!     native::main();
//! }
//! ```
//!
//! The evaluator exposes concrete `settings`, a copyable ORM `db` handle, and
//! the application `di` context. Unique installed model names are imported;
//! inaccessible paths and collisions produce deterministic warnings with
//! concrete registered crate paths, while the `project_crate` alias can
//! reference the same types. A project's [`ShellConfig`] must pass any
//! non-default Cargo feature selection used for the management binary through
//! [`ShellConfig::with_dependency_features`] and
//! [`ShellConfig::without_default_features`].
//! Interactive input supports top-level `.await` and `>>> ` / `... ` prompts.
//! A panic, evaluator exit, or evaluation interrupt clears user state and
//! reloads every prelude layer. One-shot `shell -c` returns an error for any
//! unsuccessful bootstrap or evaluation. Reinhardt-owned diagnostics do not
//! echo the raw source, but arbitrary Rust, compiler output, panics, and user
//! code can print literals; the shell is not a sandbox or secrecy boundary.
//! History is best-effort under the platform local data directory at
//! `reinhardt/shell/<package-name>.history`; a missing file is a normal silent
//! first run, while access or directory-resolution failures warn and continue.
//! A cold start may compile the project and evaluator support; warm starts
//! reuse unchanged Cargo artifacts.
//!
//! `shell-rhai` has been removed: `shell` now means the Rust evaluator. Existing
//! settings-only entry points remain compatible with non-shell commands.
//!
//! ## Database Schema Inspection
//!
//! `manage inspectdb [TABLE ...]` accepts exact table names and writes one
//! parseable Rust module to stdout by default. The `--database` option selects
//! a configured alias and defaults to `default`; use `--database-url` only for
//! an explicit URL override. Human-readable progress is written to stderr.
//!
//! Explicit `--output DIRECTORY` mode generates `DIRECTORY/models.rs` plus
//! `DIRECTORY/models/<table>.rs` child modules and never generates `mod.rs`.
//! Existing destinations are rejected unless `--force` is also present, and a
//! failed publication is rollback-safe and all-or-nothing when the command
//! reports failure: replaced files are restored and newly created partial
//! output is removed. The `--force` option is invalid without `--output`.
//!
//! ## Native Database Shell
//!
//! `manage dbshell` launches the native client for the selected database:
//! `psql` for PostgreSQL, `mysql` for MySQL, or `sqlite3` for SQLite. The
//! matching client executable must be available on `PATH`.
//!
//! The configured alias defaults to `default`; pass `--database ALIAS` to
//! select another alias. `--database-url URL` is an explicit one-off override
//! and takes precedence over the alias. Arguments following `--` are passed to
//! the native client unchanged:
//!
//! ```text
//! cargo run --bin manage -- dbshell --database reporting
//! cargo run --bin manage -- dbshell -- --expanded
//! ```
//!
//! The client inherits standard input, output, and error so it remains
//! interactive. Passwords are excluded from the native client argv and
//! Reinhardt diagnostics. PostgreSQL and MySQL credentials are supplied only
//! in the child process environment through `PGPASSWORD` and `MYSQL_PWD`,
//! respectively.

/// Base command trait and argument/option definitions.
pub mod base;
/// Built-in management commands (migrate, runserver, shell, etc.).
pub mod builtin;
/// CLI argument parsing and command dispatch.
pub mod cli;
/// Static file collection command.
pub mod collectstatic;
/// Generated component stylesheet ownership for development servers.
pub mod component_styles;
/// Command execution context (settings, output, verbosity).
pub mod context;
#[cfg(feature = "contract")]
mod contract;
/// Superuser creation command.
#[cfg(feature = "auth")]
pub(crate) mod createsuperuser;
/// Data fixture and development seeding commands.
#[cfg(feature = "reinhardt-db")]
pub mod data_commands;
#[cfg(feature = "reinhardt-db")]
pub(crate) mod database_selector;
#[cfg(feature = "reinhardt-db")]
pub(crate) mod dbshell;
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
/// Database schema inspection command.
#[cfg(feature = "migrations")]
pub mod inspectdb;
/// Project introspection command for platform metadata discovery.
#[cfg(feature = "introspect")]
pub mod introspect;
/// Local development infrastructure management.
pub mod local_infra;
/// Email testing command.
pub mod mail_commands;
/// Terminal output wrapper with styling support.
pub mod output;
/// Plugin management commands.
#[cfg(feature = "plugins")]
pub mod plugin_commands;
/// Project dependency configuration commands.
pub mod project_config;
/// Command registry for discovery and dispatch.
pub mod registry;
#[cfg(feature = "contract")]
mod resolved_contract;
/// Runserver lifecycle hooks for concurrent services and pre-listen validation.
#[cfg(feature = "server")]
pub mod runserver_hooks;
/// Hot-reload server rebuild pipeline (cargo build + child process swap).
#[cfg(feature = "autoreload")]
#[doc(hidden)]
pub mod server_rebuild_pipeline;
mod shell;
/// Read-only migration state display.
#[cfg(feature = "migrations")]
pub mod showmigrations;
/// Source-tree enumeration for hot-reload watch targets.
#[cfg(feature = "autoreload")]
#[doc(hidden)]
pub mod source_roots;
/// Read-only migration SQL rendering.
#[cfg(feature = "migrations")]
pub mod sqlmigrate;
/// Migration squashing command orchestration.
#[cfg(feature = "migrations")]
pub mod squashmigrations;
/// Project and app scaffolding commands (startproject, startapp).
pub mod start_commands;
/// Shared static asset settings resolution.
pub mod static_asset_settings;
/// Component-style package selection and deterministic source extraction.
pub mod style_extractor;
/// Template-based code generation utilities.
pub mod template;
/// Source-change classification for development template hot patching.
#[cfg(feature = "pages")]
pub mod template_classifier;
/// Normalized compiler diagnostics for development HMR clients.
#[cfg(feature = "pages")]
pub mod template_diagnostics;
/// Coordination of template patch dispatch and fallback rebuilds.
#[cfg(feature = "pages")]
pub mod template_hot_reload;
/// Compiler manifest collection for development template hot patching.
#[cfg(feature = "pages")]
pub mod template_manifest;
/// Template source abstraction over embedded and filesystem assets.
pub mod template_source;
/// Successful client baselines and mutable static overlays.
#[cfg(feature = "pages")]
pub mod template_state;
/// Deterministic contract verification and Cargo replay.
#[cfg(feature = "contract")]
pub mod verify;
/// WASM build tooling for client-side compilation.
pub mod wasm_builder;
/// Hot-reload WASM rebuild pipeline (timing + structured logging wrapper).
#[cfg(all(feature = "autoreload", feature = "pages"))]
#[doc(hidden)]
pub mod wasm_rebuild_pipeline;
/// Development server welcome page.
#[cfg(feature = "pages")]
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
	execute_from_command_line_with_migration_settings,
	execute_from_command_line_with_migration_settings_and_shell,
	execute_from_command_line_with_registry, execute_from_command_line_with_registry_and_settings,
	execute_from_command_line_with_registry_and_settings_and_shell,
	execute_from_command_line_with_settings, execute_from_command_line_with_settings_and_shell,
	run_command, run_command_with_registry,
};
#[cfg(feature = "contract")]
pub use cli::{
	ContractOutputFormat, ContractSubcommand, execute_from_command_line_with_pending_settings,
	execute_from_command_line_with_pending_settings_and_cargo_context,
	execute_from_command_line_with_pending_settings_and_cargo_context_and_shell,
	execute_from_command_line_with_pending_settings_and_shell,
	execute_from_command_line_with_registry_and_resolved_settings,
	execute_from_command_line_with_registry_and_resolved_settings_and_shell,
	execute_from_command_line_with_resolved_settings,
	execute_from_command_line_with_resolved_settings_and_shell,
};
pub use collectstatic::{
	CollectStaticCommand, CollectStaticOptions, CollectStaticStats, VirtualStaticAsset,
};
pub use component_styles::{
	ComponentStyleStageResult, ComponentStyleState, GeneratedStyleAssets, join_static_url,
};
pub use context::CommandContext;
#[cfg(feature = "reinhardt-db")]
pub use data_commands::{
	SeedContext, SeedHook, SeedHookRegistration, collect_seed_hooks, execute_dumpdata,
	execute_loaddata, execute_seed,
};
pub use i18n_commands::{CompileMessagesCommand, MakeMessagesCommand};
#[cfg(feature = "migrations")]
pub use inspectdb::{InspectDbCommand, InspectDbWriter};
#[cfg(feature = "introspect")]
pub use introspect::IntrospectCommand;
pub use mail_commands::SendTestEmailCommand;
pub use output::OutputWrapper;
pub use project_config::{ConfigureCommand, ReinhardtDependencySelection};
pub use registry::CommandRegistry;
#[cfg(feature = "contract")]
pub use resolved_contract::{
	ContractResolutionError, ContractResolutionErrorKind, ResolvedContractState,
	SafeContractTarget, resolve_contract_state,
};
#[cfg(feature = "server")]
pub use runserver_hooks::{RunserverContext, RunserverHook, RunserverHookRegistration};
#[cfg(feature = "shell")]
pub use shell::ShellEnvironment;
pub use shell::{ShellConfig, shell_runtime_hook};
#[cfg(feature = "migrations")]
pub use showmigrations::{
	MigrationVisibilityWriter, ShowMigrationsCommand, ShowMigrationsMode, format_migration_snapshot,
};
#[cfg(feature = "migrations")]
pub use sqlmigrate::{SqlMigrateCommand, render_migration_sql};
#[cfg(feature = "migrations")]
pub use squashmigrations::{
	ConfirmationReader, SquashMigrationsOptions, SquashMigrationsSummary, StdinConfirmationReader,
	execute_squashmigrations_with_context_and_io, execute_squashmigrations_with_io,
};
pub use start_commands::{StartAppCommand, StartProjectCommand};
pub use static_asset_settings::StaticAssetSettings;
pub use style_extractor::{
	COMPONENT_STYLES_PATH, ExtractedStyleDefinition, StyleBundle, StyleExtractor,
	StyleFeatureSelection, StyleFingerprints, StylePackageContext,
};
pub use template::{TemplateCommand, TemplateContext, generate_secret_key, to_camel_case};
#[cfg(feature = "pages")]
pub use template_classifier::{
	RebuildReason, TemplateClassification, TemplateDiagnostic, TemplatePatchSet,
	classify_source_change,
};
#[cfg(feature = "pages")]
pub use template_diagnostics::normalize_build_diagnostics;
#[cfg(feature = "pages")]
pub use template_hot_reload::{
	CoordinatorError, DispatchOutcome, TemplateBuildArtifact, TemplateHotReloadCoordinator,
};
#[cfg(feature = "pages")]
pub use template_state::{CompiledBaseline, SourceBaseline, StaticOverlayStore};
#[cfg(feature = "contract")]
pub use verify::report::{
	VerificationClassV1, VerificationReportV1, VerificationSeverityV1, VerificationStatusV1,
	VerificationTargetV1, VerificationViolationV1,
};
#[cfg(feature = "contract")]
pub use verify::{
	CargoCheckContext, CargoCheckPlan, CargoConfigReplay, CargoProfile, CargoReplayUnsupported,
	VerificationCheckError, VerificationFinding, VerificationOutputFormat, VerificationRun,
	execute_verify, execute_verify_with_applied_migrations, plan_cargo_check, render_verification,
	render_verification_output,
};
pub use wasm_builder::{
	WasmBuildConfig, WasmBuildError, WasmBuildOutput, WasmBuilder, check_wasm_tools_installed,
	detect_cdylib_in_cargo_toml, detect_cdylib_in_cargo_toml_content, is_wasm_stale,
	is_wasm_stale_for_roots, latest_source_mtime,
};
#[cfg(feature = "pages")]
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

	/// Contract verification found violations.
	#[error("Contract verification found violations")]
	VerificationFailed,

	/// Contract verification could not complete safely.
	#[error("Contract verification could not complete: {0}")]
	VerificationExecution(String),

	/// A command requires an optional Cargo feature that is not enabled.
	#[error("{0}")]
	FeatureDisabled(String),

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

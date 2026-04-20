//! # Start Commands
//!
//! Django's startproject and startapp commands translation to Rust
//!
//! Source:
//! - django/core/management/commands/startproject.py
//! - django/core/management/commands/startapp.py

use crate::template_source::{EmbeddedSource, FilesystemSource, MergedSource, TemplateSource};
use crate::{
	BaseCommand, CommandArgument, CommandContext, CommandError, CommandOption, CommandResult,
	TemplateCommand, TemplateContext, generate_secret_key, to_camel_case,
};
use async_trait::async_trait;
use std::env;
use std::path::{Path, PathBuf};

/// Validate that a name does not use the reserved `reinhardt_*` namespace.
///
/// Names starting with `reinhardt_` or `reinhardt-` conflict with the DI
/// pseudo orphan rule (#3468, #3502) which treats `reinhardt_*::*` as
/// framework-managed types.
fn validate_not_reserved_namespace(name: &str) -> CommandResult<()> {
	let normalized = name.replace('-', "_");
	if normalized.starts_with("reinhardt_") || normalized == "reinhardt" {
		return Err(CommandError::InvalidArguments(format!(
			"Name '{}' is not allowed: names starting with 'reinhardt_' or 'reinhardt-' \
			 are reserved for the Reinhardt framework. This conflicts with the DI pseudo \
			 orphan rule which treats 'reinhardt_*' namespaces as framework-managed. \
			 Please choose a different name.",
			name
		)));
	}
	Ok(())
}

/// Create a Reinhardt project directory structure
///
/// Translation of Django's startproject command
pub struct StartProjectCommand;

#[async_trait]
impl BaseCommand for StartProjectCommand {
	fn name(&self) -> &str {
		"startproject"
	}

	fn description(&self) -> &str {
		"Creates a Reinhardt project directory structure for the given project name in the current directory or optionally in the given directory."
	}

	fn arguments(&self) -> Vec<CommandArgument> {
		vec![
			CommandArgument::required("name", "Name of the project"),
			CommandArgument::optional("directory", "Optional destination directory"),
		]
	}

	fn options(&self) -> Vec<CommandOption> {
		vec![
			CommandOption::option(None, "template", "The path to load the template from"),
			CommandOption::option(
				None,
				"template-dir",
				"Root directory whose sub-templates override embedded defaults (also reads REINHARDT_TEMPLATE_DIR)",
			),
			CommandOption::option(
				Some('e'),
				"extension",
				"The file extension(s) to render (default: \"rs\")",
			)
			.with_default("rs"),
			CommandOption::flag(None, "restful", "Create a RESTful API project (default)"),
			CommandOption::flag(
				None,
				"with-pages",
				"Create a project with reinhardt-pages (WASM + SSR)",
			),
		]
	}

	async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
		let project_name = ctx
			.arg(0)
			.ok_or_else(|| {
				CommandError::InvalidArguments("You must provide a project name.".to_string())
			})?
			.clone();

		// Reject reserved reinhardt_* namespace (#3502)
		validate_not_reserved_namespace(&project_name)?;

		let target = ctx.arg(1).map(PathBuf::from);

		// Determine project type
		let is_restful = ctx.has_option("restful");
		let with_pages = ctx.has_option("with-pages")
			|| ctx
				.option("type")
				.is_some_and(|t| t == "mtv" || t == "pages");

		// Validate exclusive flags
		if is_restful && with_pages {
			return Err(CommandError::InvalidArguments(
				"Only one of --restful or --with-pages can be specified".to_string(),
			));
		}

		// Determine project type and template key
		let (project_type, template_key) = if with_pages {
			("Pages (WASM + SSR)", "pages")
		} else {
			("RESTful API", "restful") // Default
		};

		ctx.info(&format!(
			"Creating {} project '{}'...",
			project_type, project_name
		));

		// Generate a random secret key
		let secret_key = format!("insecure-{}", generate_secret_key());

		// Prepare template context
		let mut context = TemplateContext::new();
		context.insert("project_name", &project_name)?;
		context.insert("crate_name", project_name.replace('-', "_"))?;
		context.insert("secret_key", &secret_key)?;
		context.set_example_override(
			"secret_key",
			"CHANGE_THIS_IN_PRODUCTION_MUST_BE_KEPT_SECRET",
		)?;
		context.insert("camel_case_project_name", to_camel_case(&project_name))?;
		context.insert("reinhardt_version", env!("CARGO_PKG_VERSION"))?;
		context.insert("is_restful", if !with_pages { "true" } else { "false" })?;
		context.insert("with_pages", if with_pages { "true" } else { "false" })?;

		// Determine template source (--template > --template-dir/env > embedded)
		let subdir = format!("project_{}_template", template_key);
		let source: Box<dyn TemplateSource> = if let Some(template_path) = ctx.option("template") {
			Box::new(FilesystemSource::new(template_path)?)
		} else {
			let override_root = effective_template_dir_override(ctx);
			resolve_source(override_root.as_deref(), &subdir)?
		};

		// Create project using TemplateCommand
		let template_cmd = TemplateCommand::new();
		template_cmd.handle(
			&project_name,
			target.as_deref(),
			source.as_ref(),
			context,
			ctx,
		)?;

		ctx.success(&format!(
			"{} project '{}' created successfully! Next steps:",
			project_type, project_name
		));
		ctx.info(&format!("  cd {}", project_name));

		// Display appropriate next steps based on project type
		if with_pages {
			ctx.info("  # Install development tools");
			ctx.info("  cargo make install-tools");
			ctx.info("  # Build WASM and start development server");
			ctx.info("  cargo make dev");
		} else {
			ctx.info("  cargo run");
		}

		Ok(())
	}
}

/// Create a Reinhardt app directory structure
///
/// Translation of Django's startapp command
pub struct StartAppCommand;

#[async_trait]
impl BaseCommand for StartAppCommand {
	fn name(&self) -> &str {
		"startapp"
	}

	fn description(&self) -> &str {
		"Creates a Reinhardt app directory structure for the given app name in the current directory or optionally in the given directory."
	}

	fn arguments(&self) -> Vec<CommandArgument> {
		vec![
			CommandArgument::required("name", "Name of the application"),
			CommandArgument::optional("directory", "Optional destination directory"),
		]
	}

	fn options(&self) -> Vec<CommandOption> {
		vec![
			CommandOption::option(None, "template", "The path to load the template from"),
			CommandOption::option(
				None,
				"template-dir",
				"Root directory whose sub-templates override embedded defaults (also reads REINHARDT_TEMPLATE_DIR)",
			),
			CommandOption::option(
				Some('e'),
				"extension",
				"The file extension(s) to render (default: \"rs\")",
			)
			.with_default("rs"),
			CommandOption::flag(None, "restful", "Create a RESTful API app (default)"),
			CommandOption::flag(
				None,
				"with-pages",
				"Create an app with reinhardt-pages (WASM + SSR)",
			),
			CommandOption::flag(
				None,
				"workspace",
				"Create app as a separate workspace crate instead of a module",
			),
		]
	}

	async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
		let app_name = ctx
			.arg(0)
			.ok_or_else(|| {
				CommandError::InvalidArguments("You must provide an application name.".to_string())
			})?
			.clone();

		// Reject reserved reinhardt_* namespace (#3502)
		validate_not_reserved_namespace(&app_name)?;

		let target = ctx.arg(1).map(PathBuf::from);

		// Determine app type and structure
		let is_restful = ctx.has_option("restful");
		let with_pages = ctx.has_option("with-pages")
			|| ctx
				.option("type")
				.is_some_and(|t| t == "mtv" || t == "pages");
		let is_workspace = ctx.has_option("workspace");

		// Validate exclusive flags
		if is_restful && with_pages {
			return Err(CommandError::InvalidArguments(
				"Only one of --restful or --with-pages can be specified".to_string(),
			));
		}

		// Determine app type and template key
		let (app_type, template_key) = if with_pages {
			("Pages (WASM + SSR)", "pages")
		} else {
			("RESTful API", "restful") // Default
		};

		let structure_type = if is_workspace {
			"workspace crate"
		} else {
			"module"
		};
		ctx.info(&format!(
			"Creating {} app '{}' as a {}...",
			app_type, app_name, structure_type
		));

		if is_workspace {
			// Create as workspace crate
			create_workspace_app(&app_name, target.as_deref(), with_pages, ctx).await?;

			ctx.success(&format!(
				"{} app '{}' created successfully as a workspace crate in apps/{}!",
				app_type, app_name, app_name
			));
			ctx.info("The app has been added to the workspace members in Cargo.toml");
			ctx.info(
				"Don't forget to add it as a dependency and to INSTALLED_APPS in your settings.rs",
			);
		} else {
			// Create as module (default)
			// Create src/apps directory if it doesn't exist
			let apps_dir = PathBuf::from("src/apps");
			if !apps_dir.exists() {
				std::fs::create_dir_all(&apps_dir).map_err(|e| {
					CommandError::ExecutionError(format!("Failed to create apps directory: {}", e))
				})?;
				ctx.verbose("Created src/apps/ directory");
			}

			// Set target to src/apps/{app_name} if no custom target is specified
			// Track whether a custom target was provided before consuming target
			let has_custom_target = target.is_some();
			let app_target = if has_custom_target {
				target
			} else {
				Some(apps_dir.join(&app_name))
			};

			// Prepare template context
			let mut context = TemplateContext::new();
			context.insert("app_name", &app_name)?;
			context.insert("camel_case_app_name", to_camel_case(&app_name))?;
			context.insert("is_restful", if !with_pages { "true" } else { "false" })?;
			context.insert("with_pages", if with_pages { "true" } else { "false" })?;

			// Determine template source (--template > --template-dir/env > embedded)
			let subdir = format!("app_{}_template", template_key);
			let source: Box<dyn TemplateSource> =
				if let Some(template_path) = ctx.option("template") {
					Box::new(FilesystemSource::new(template_path)?)
				} else {
					let override_root = effective_template_dir_override(ctx);
					resolve_source(override_root.as_deref(), &subdir)?
				};

			// Create app using TemplateCommand
			let template_cmd = TemplateCommand::new();
			template_cmd.handle(
				&app_name,
				app_target.as_deref(),
				source.as_ref(),
				context,
				ctx,
			)?;

			// Rust 2024 Edition: rename {app_name}/lib.rs -> {app_name}.rs
			// Module entry points must be named after the module, not lib.rs.
			// lib.rs is only special at the crate root.
			// Only apply this rename for the default location (src/apps/{name}/);
			// when a custom target is specified, preserve lib.rs in that location.
			if !has_custom_target && let Some(ref target_path) = app_target {
				let lib_rs_path = target_path.join("lib.rs");
				if lib_rs_path.exists() {
					// The module entry point goes one level up, alongside the subdirectory
					let module_rs_path = target_path
						.parent()
						.map(|parent| parent.join(format!("{}.rs", app_name)))
						.ok_or_else(|| {
							CommandError::ExecutionError(format!(
								"Failed to determine parent directory for '{}'",
								target_path.display()
							))
						})?;
					std::fs::rename(&lib_rs_path, &module_rs_path).map_err(|e| {
						CommandError::ExecutionError(format!(
							"Failed to move lib.rs to {}.rs: {}",
							app_name, e
						))
					})?;
					ctx.verbose(&format!(
						"Moved {}/lib.rs -> {}.rs (Rust 2024 Edition module convention)",
						app_name, app_name
					));
				}
			}

			// Update or create apps.rs to export the new app
			update_apps_export(&app_name)?;

			// Append to installed_apps! { ... } block (Issue #3670).
			// Idempotent and silently skipped if src/config/apps.rs is
			// missing (older project structure).
			update_installed_apps_block(&app_name)?;

			ctx.success(&format!(
				"{} app '{}' created successfully in src/apps/{}!",
				app_type, app_name, app_name
			));
			ctx.info("The app has been added to src/apps.rs and src/config/apps.rs");
		}

		Ok(())
	}
}

/// Resolve a `TemplateSource` for a given template subdirectory key.
///
/// Priority (highest first):
/// 1. `--template` CLI flag (handled by each command directly — full replacement via `FilesystemSource`)
/// 2. `--template-dir` CLI flag or `REINHARDT_TEMPLATE_DIR` env — `MergedSource` with embedded fallback
/// 3. Embedded-only (`EmbeddedSource`)
fn resolve_source(
	override_root: Option<&Path>,
	subdir: &str,
) -> CommandResult<Box<dyn TemplateSource>> {
	if let Some(root) = override_root {
		if !root.exists() || !root.is_dir() {
			return Err(CommandError::ExecutionError(format!(
				"template override root does not exist or is not a directory: {}",
				root.display()
			)));
		}
		let subdir_path = root.join(subdir);
		if subdir_path.exists() {
			let primary = FilesystemSource::new(&subdir_path)?;
			return Ok(Box::new(MergedSource {
				primary,
				fallback: EmbeddedSource::new(subdir),
			}));
		}
		// Override root exists but has no subdir for this template type;
		// fall through to embedded-only so partial override trees are still valid.
	}
	Ok(Box::new(EmbeddedSource::new(subdir)))
}

fn effective_template_dir_override(ctx: &CommandContext) -> Option<PathBuf> {
	if let Some(v) = ctx.option("template-dir").filter(|v| !v.trim().is_empty()) {
		return Some(PathBuf::from(v));
	}
	if let Some(v) = env::var("REINHARDT_TEMPLATE_DIR")
		.ok()
		.filter(|v| !v.is_empty())
	{
		return Some(PathBuf::from(v));
	}
	None
}

/// Create a workspace-based app
async fn create_workspace_app(
	app_name: &str,
	target: Option<&Path>,
	with_pages: bool,
	ctx: &CommandContext,
) -> CommandResult<()> {
	// Create apps directory if it doesn't exist
	let apps_dir = PathBuf::from("apps");
	if !apps_dir.exists() {
		std::fs::create_dir_all(&apps_dir).map_err(|e| {
			CommandError::ExecutionError(format!("Failed to create apps directory: {}", e))
		})?;
		ctx.verbose("Created apps/ directory");
	}

	// Set target to apps/{app_name} if no custom target is specified
	let app_target = if let Some(t) = target {
		t.to_path_buf()
	} else {
		apps_dir.join(app_name)
	};

	// Prepare template context
	let mut context = TemplateContext::new();
	context.insert("app_name", app_name)?;
	context.insert("camel_case_app_name", to_camel_case(app_name))?;
	context.insert("is_restful", if !with_pages { "true" } else { "false" })?;
	context.insert("with_pages", if with_pages { "true" } else { "false" })?;
	// Workspace apps reference the parent project crate by name (for example
	// `use my_project::config::apps::InstalledApp;`). Derive that name from
	// the current directory (the workspace root), normalizing hyphens to
	// underscores so the import is a valid Rust path. Falls back to
	// `"project"` when the directory name is unavailable.
	let project_crate_name = std::env::current_dir()
		.ok()
		.and_then(|p| p.file_name().map(|n| n.to_string_lossy().replace('-', "_")))
		.unwrap_or_else(|| "project".to_string());
	context.insert("project_crate_name", &project_crate_name)?;

	// Determine template source via embedded fallback
	let template_key = if with_pages { "pages" } else { "restful" };
	let subdir = format!("app_{}_workspace_template", template_key);
	let source = resolve_source(effective_template_dir_override(ctx).as_deref(), &subdir)?;

	// Create app using TemplateCommand
	let template_cmd = TemplateCommand::new();
	template_cmd.handle(app_name, Some(&app_target), source.as_ref(), context, ctx)?;

	// Update workspace Cargo.toml
	update_workspace_members(app_name)?;

	Ok(())
}

/// Update workspace Cargo.toml to add new app as a member
fn update_workspace_members(app_name: &str) -> CommandResult<()> {
	use std::fs;

	let cargo_toml_path = PathBuf::from("Cargo.toml");

	if !cargo_toml_path.exists() {
		return Err(CommandError::ExecutionError(
			"Cargo.toml not found in current directory. Make sure you're in the project root."
				.to_string(),
		));
	}

	let content = fs::read_to_string(&cargo_toml_path)
		.map_err(|e| CommandError::ExecutionError(format!("Failed to read Cargo.toml: {}", e)))?;

	let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
	let member_line = format!("    \"apps/{}\",", app_name);

	// Find [workspace] section and members array
	let mut in_workspace_section = false;
	let mut in_members_array = false;
	let mut insert_index = None;

	for (i, line) in lines.iter().enumerate() {
		let trimmed = line.trim();

		if trimmed == "[workspace]" {
			in_workspace_section = true;
			continue;
		}

		if in_workspace_section {
			if trimmed.starts_with('[') && trimmed != "[workspace]" {
				// Entered a different section
				break;
			}

			if trimmed.starts_with("members") {
				in_members_array = true;
				continue;
			}

			if in_members_array && trimmed == "]" {
				// Found end of members array, insert before this line
				insert_index = Some(i);
				break;
			}
		}
	}

	if let Some(idx) = insert_index {
		// Check if member already exists
		let member_exists = lines
			.iter()
			.any(|line| line.contains(&format!("apps/{}", app_name)));

		if !member_exists {
			lines.insert(idx, member_line);
		}
	} else {
		// No workspace section found, add it
		return Err(CommandError::ExecutionError(
            "No [workspace] section with members array found in Cargo.toml. Please add one manually or use a workspace template.".to_string()
        ));
	}

	// Write back
	let new_content = lines.join("\n") + "\n";
	fs::write(&cargo_toml_path, new_content)
		.map_err(|e| CommandError::ExecutionError(format!("Failed to write Cargo.toml: {}", e)))?;

	Ok(())
}

/// Update or create apps.rs to export the new app using AST
///
/// Uses AST parsing to robustly detect existing module declarations
/// and add new ones, avoiding issues with comments and formatting.
fn update_apps_export(app_name: &str) -> CommandResult<()> {
	use std::fs;
	use syn::{File, Item, ItemMod, ItemUse, parse_file};

	let apps_file = PathBuf::from("src/apps.rs");
	let camel_case_name = to_camel_case(app_name);

	// Parse existing file or create default AST
	let mut ast: File = if apps_file.exists() {
		let content = fs::read_to_string(&apps_file)
			.map_err(|e| CommandError::ExecutionError(format!("Failed to read apps.rs: {}", e)))?;
		parse_file(&content)
			.map_err(|e| CommandError::ExecutionError(format!("Failed to parse apps.rs: {}", e)))?
	} else {
		parse_file("//! Apps module - exports all applications\n").map_err(|e| {
			CommandError::ExecutionError(format!("Failed to create default AST: {}", e))
		})?
	};

	// Validate app_name is a valid Rust identifier
	// syn::Ident::new will panic if the name is not valid, so we check first
	if !app_name
		.chars()
		.next()
		.is_some_and(|c| c.is_alphabetic() || c == '_')
	{
		return Err(CommandError::InvalidArguments(format!(
			"App name '{}' is not a valid Rust identifier (must start with a letter or underscore)",
			app_name
		)));
	}

	if !app_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
		return Err(CommandError::InvalidArguments(format!(
			"App name '{}' contains invalid characters (only letters, numbers, and underscores allowed)",
			app_name
		)));
	}

	// Check if module declaration already exists (structurally)
	let app_ident = syn::Ident::new(app_name, proc_macro2::Span::call_site());
	let has_mod_declaration = ast
		.items
		.iter()
		.any(|item| matches!(item, Item::Mod(ItemMod { ident, .. }) if ident == &app_ident));

	if !has_mod_declaration {
		// Add module declaration: pub mod app_name;
		let mod_item: ItemMod = syn::parse_quote! {
			pub mod #app_ident;
		};
		ast.items.push(Item::Mod(mod_item));

		// Add use declaration: pub use app_name::AppNameConfig;
		let config_name = format!("{}Config", camel_case_name);
		let config_ident = syn::Ident::new(&config_name, proc_macro2::Span::call_site());
		let use_item: ItemUse = syn::parse_quote! {
			pub use #app_ident::#config_ident;
		};
		ast.items.push(Item::Use(use_item));
	}

	// Format and write back to file
	let formatted = prettyplease::unparse(&ast);
	fs::write(&apps_file, formatted)
		.map_err(|e| CommandError::ExecutionError(format!("Failed to write apps.rs: {}", e)))?;

	Ok(())
}

/// Append a new app entry to the `installed_apps! { ... }` block in
/// `src/config/apps.rs`.
///
/// Issue #3670: the typed `#[url_patterns(InstalledApp::<name>, ...)]`
/// form requires the app's label to be registered via `installed_apps!`.
/// This function is idempotent: if an entry with the same label already
/// exists, it is left alone.
///
/// Silently succeeds if `src/config/apps.rs` does not exist (projects
/// scaffolded before this change may not have it; users are expected to
/// add it manually following the migration guide).
fn update_installed_apps_block(app_name: &str) -> CommandResult<()> {
	use std::fs;

	let apps_file = PathBuf::from("src/config/apps.rs");
	if !apps_file.exists() {
		// Pre-#3670 projects don't have this file — skip silently. Users
		// on an older project structure can still use the new macro
		// syntax by manually creating the file per the migration guide.
		return Ok(());
	}

	let src = fs::read_to_string(&apps_file).map_err(|e| {
		CommandError::ExecutionError(format!("Failed to read {}: {}", apps_file.display(), e))
	})?;

	// Idempotency: skip if the label is already present.
	// We match `<name>:` since installed_apps! entries are of the form
	// `<label>: "<path>"`.
	let needle = format!("{}:", app_name);
	if src.contains(&needle) {
		return Ok(());
	}

	// Locate `installed_apps! { ... }` and append the entry before the
	// closing `}`. A simple brace-walker suffices: we find the opening
	// brace after `installed_apps!` and then the matching closing brace.
	let Some(macro_start) = src.find("installed_apps!") else {
		return Err(CommandError::ExecutionError(format!(
			"{} does not contain `installed_apps! {{ ... }}`; cannot register new app",
			apps_file.display()
		)));
	};

	let Some(open_rel) = src[macro_start..].find('{') else {
		return Err(CommandError::ExecutionError(format!(
			"malformed installed_apps! block in {} (no opening brace)",
			apps_file.display()
		)));
	};
	let open_idx = macro_start + open_rel;

	// Find matching closing brace.
	let mut depth = 0usize;
	let mut close_idx: Option<usize> = None;
	for (i, ch) in src[open_idx..].char_indices() {
		match ch {
			'{' => depth += 1,
			'}' => {
				depth -= 1;
				if depth == 0 {
					close_idx = Some(open_idx + i);
					break;
				}
			}
			_ => {}
		}
	}
	let Some(close_idx) = close_idx else {
		return Err(CommandError::ExecutionError(format!(
			"malformed installed_apps! block in {} (unmatched brace)",
			apps_file.display()
		)));
	};

	// Insert the new entry before the closing brace. Preserve existing
	// trailing newline/indent style as best-effort.
	let new_entry = format!("    {}: \"{}\",\n", app_name, app_name);
	let mut out = String::with_capacity(src.len() + new_entry.len());
	out.push_str(&src[..close_idx]);
	// Ensure the content ends with a newline before we append.
	if !out.ends_with('\n') {
		out.push('\n');
	}
	out.push_str(&new_entry);
	out.push_str(&src[close_idx..]);

	fs::write(&apps_file, out).map_err(|e| {
		CommandError::ExecutionError(format!("Failed to write {}: {}", apps_file.display(), e))
	})?;

	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;
	use tempfile::{TempDir, tempdir};

	#[fixture]
	fn template_dir() -> TempDir {
		tempdir().unwrap()
	}

	#[fixture]
	fn output_dir() -> TempDir {
		tempdir().unwrap()
	}

	#[test]
	fn test_startproject_command_name() {
		let cmd = StartProjectCommand;
		assert_eq!(cmd.name(), "startproject");
	}

	#[test]
	fn test_startapp_command_name() {
		let cmd = StartAppCommand;
		assert_eq!(cmd.name(), "startapp");
	}

	#[test]
	fn test_with_pages_flag_exists() {
		let cmd = StartProjectCommand;
		let options = cmd.options();
		assert!(
			options.iter().any(|opt| opt.long == "with-pages"),
			"--with-pages flag should exist"
		);
	}

	#[test]
	fn test_restful_flag_exists() {
		let cmd = StartProjectCommand;
		let options = cmd.options();
		assert!(
			options.iter().any(|opt| opt.long == "restful"),
			"--restful flag should exist"
		);
	}

	#[test]
	fn test_mtv_flag_removed() {
		let cmd = StartProjectCommand;
		let options = cmd.options();
		assert!(
			!options.iter().any(|opt| opt.long == "mtv"),
			"--mtv flag should be removed"
		);
	}

	#[test]
	fn test_startapp_with_pages_flag_exists() {
		let cmd = StartAppCommand;
		let options = cmd.options();
		assert!(
			options.iter().any(|opt| opt.long == "with-pages"),
			"--with-pages flag should exist in StartAppCommand"
		);
	}

	#[test]
	fn test_startapp_mtv_flag_removed() {
		let cmd = StartAppCommand;
		let options = cmd.options();
		assert!(
			!options.iter().any(|opt| opt.long == "mtv"),
			"--mtv flag should be removed from StartAppCommand"
		);
	}

	#[rstest]
	fn test_example_file_duplication(template_dir: TempDir, output_dir: TempDir) {
		use crate::template::TemplateCommand;
		use std::fs;

		// Create a mock template file with .example.toml
		let settings_dir = template_dir.path().join("settings");
		fs::create_dir_all(&settings_dir).unwrap();
		let example_file = settings_dir.join("base.example.toml");
		fs::write(&example_file, "debug = true\n").unwrap();

		// Process the template
		let cmd = TemplateCommand::new();
		let context = crate::template::TemplateContext::new();
		let ctx = crate::CommandContext::new(vec![]);
		let source = crate::template_source::FilesystemSource::new(template_dir.path()).unwrap();

		cmd.handle("test", Some(output_dir.path()), &source, context, &ctx)
			.unwrap();

		// Verify that both files exist
		let output_file_with_example = output_dir.path().join("settings").join("base.example.toml");
		let output_file_without_example = output_dir.path().join("settings").join("base.toml");

		assert!(
			output_file_with_example.exists(),
			"Expected base.example.toml to exist"
		);
		assert!(
			output_file_without_example.exists(),
			"Expected base.toml to exist"
		);

		// Verify both files have the same content
		let content_with_example = fs::read_to_string(&output_file_with_example).unwrap();
		let content_without_example = fs::read_to_string(&output_file_without_example).unwrap();

		assert_eq!(content_with_example, "debug = true\n");
		assert_eq!(content_without_example, "debug = true\n");
	}

	#[rstest]
	fn test_tpl_and_example_file_duplication(template_dir: TempDir, output_dir: TempDir) {
		use crate::template::TemplateCommand;
		use std::fs;

		// Create a mock template file with both .example and .tpl
		let settings_dir = template_dir.path().join("settings");
		fs::create_dir_all(&settings_dir).unwrap();
		let example_file = settings_dir.join("base.example.toml.tpl");
		fs::write(&example_file, "debug = {{debug_value}}\n").unwrap();

		// Process the template with context
		let cmd = TemplateCommand::new();
		let mut context = crate::template::TemplateContext::new();
		context.insert("debug_value", "false").unwrap();
		let ctx = crate::CommandContext::new(vec![]);

		let source = crate::template_source::FilesystemSource::new(template_dir.path()).unwrap();
		cmd.handle("test", Some(output_dir.path()), &source, context, &ctx)
			.unwrap();

		// Verify that both files exist (without .tpl but with/without .example)
		let output_file_with_example = output_dir.path().join("settings").join("base.example.toml");
		let output_file_without_example = output_dir.path().join("settings").join("base.toml");

		assert!(
			output_file_with_example.exists(),
			"Expected base.example.toml to exist"
		);
		assert!(
			output_file_without_example.exists(),
			"Expected base.toml to exist"
		);

		// Verify both files have the same rendered content
		let content_with_example = fs::read_to_string(&output_file_with_example).unwrap();
		let content_without_example = fs::read_to_string(&output_file_without_example).unwrap();

		assert_eq!(content_with_example, "debug = false\n");
		assert_eq!(content_without_example, "debug = false\n");
	}

	#[rstest]
	fn test_startproject_type_option_mtv() {
		// Arrange
		let cmd = StartProjectCommand;
		let options = cmd.options();

		// Act & Assert
		// Verify that the --with-pages flag exists, which is the target
		// for type option "mtv" / "pages" mapping
		assert!(
			options.iter().any(|opt| opt.long == "with-pages"),
			"--with-pages flag should exist for mtv type mapping"
		);
	}

	#[rstest]
	fn test_startapp_type_option_mtv() {
		// Arrange
		let cmd = StartAppCommand;
		let options = cmd.options();

		// Act & Assert
		assert!(
			options.iter().any(|opt| opt.long == "with-pages"),
			"--with-pages flag should exist in StartAppCommand for mtv type mapping"
		);
	}

	#[rstest]
	#[case("reinhardt_myapp")]
	#[case("reinhardt-myapp")]
	#[case("reinhardt_")]
	#[case("reinhardt-")]
	#[case("reinhardt")]
	fn test_reserved_namespace_rejected(#[case] name: &str) {
		// Act
		let result = validate_not_reserved_namespace(name);

		// Assert
		assert!(result.is_err(), "should reject '{}'", name);
	}

	#[rstest]
	#[case("myapp")]
	#[case("my_reinhardt_app")]
	#[case("cool_project")]
	#[case("reinhard")]
	fn test_non_reserved_namespace_accepted(#[case] name: &str) {
		// Act
		let result = validate_not_reserved_namespace(name);

		// Assert
		assert!(result.is_ok(), "should accept '{}'", name);
	}
}

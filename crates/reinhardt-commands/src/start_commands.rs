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
	TemplateCommand, TemplateContext, generate_secret_key, project_config, to_camel_case,
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
		let required_features = if with_pages {
			&[
				"minimal",
				"pages",
				"client-router",
				"admin",
				"conf",
				"commands",
				"commands-contract",
				"commands-server",
				"commands-autoreload",
				"server",
				"grpc",
				"websockets",
				"db-sqlite",
				"forms",
				"auth-session",
				"middleware",
				"argon2-hasher",
			][..]
		} else {
			&[
				"conf",
				"commands",
				"client-router",
				"db-postgres",
				"api",
				"commands-contract",
			][..]
		};
		let dependency_selection =
			project_config::resolve_dependency_selection(ctx, required_features).await?;

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
		context.insert("reinhardt_version", &dependency_selection.version)?;
		context.insert(
			"reinhardt_default_features",
			dependency_selection.default_features,
		)?;
		context.insert(
			"reinhardt_features_toml",
			dependency_selection.features_toml(),
		)?;
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
		if with_pages {
			// Validate and update the project facade before generating files so an
			// unsupported manifest cannot leave a partially generated app behind.
			ensure_native_protocol_features()?;
		}

		if is_workspace {
			// Create as workspace crate
			let workspace_destination = target
				.clone()
				.unwrap_or_else(|| PathBuf::from("apps").join(&app_name));
			create_workspace_app(&app_name, target.as_deref(), with_pages, ctx).await?;

			ctx.success(&format!(
				"{} app '{}' created successfully as a workspace crate in {}!",
				app_type,
				app_name,
				workspace_destination.display()
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
			context.insert("is_workspace", "false")?;
			context.insert("project_crate_name", "")?;

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
			update_apps_export(&app_name, with_pages)?;

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
	let apps_dir = PathBuf::from("apps");
	let app_target = target
		.map(Path::to_path_buf)
		.unwrap_or_else(|| apps_dir.join(app_name));
	if target.is_none() && !apps_dir.exists() {
		std::fs::create_dir_all(&apps_dir).map_err(|e| {
			CommandError::ExecutionError(format!("Failed to create apps directory: {}", e))
		})?;
		ctx.verbose("Created apps/ directory");
	}

	// Prepare template context
	let mut context = TemplateContext::new();
	context.insert("app_name", app_name)?;
	context.insert("camel_case_app_name", to_camel_case(app_name))?;
	context.insert("is_restful", if !with_pages { "true" } else { "false" })?;
	context.insert("with_pages", if with_pages { "true" } else { "false" })?;
	context.insert("is_workspace", "true")?;
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

	// Reuse the non-workspace template; is_workspace conditionals handle the
	// import-path divergence inside the templates themselves.
	let template_key = if with_pages { "pages" } else { "restful" };
	let subdir = format!("app_{}_template", template_key);
	let source = resolve_source(effective_template_dir_override(ctx).as_deref(), &subdir)?;

	// Render template into apps/<name>/src/ so the standard crate layout is
	// preserved (Cargo.toml sits one level above, at apps/<name>/).
	let src_target = app_target.join("src");
	let template_cmd = TemplateCommand::new();
	template_cmd.handle(app_name, Some(&src_target), source.as_ref(), context, ctx)?;

	// Generate workspace infrastructure files at apps/<name>/
	generate_workspace_cargo_toml(app_name, with_pages, &app_target)?;
	if with_pages {
		generate_workspace_build_rs(app_name, &app_target)?;
	}

	// Update workspace Cargo.toml
	update_workspace_manifest(app_name, &app_target)?;

	Ok(())
}

/// Generate a workspace app manifest with target-specific facade dependencies.
fn generate_workspace_cargo_toml(
	app_name: &str,
	with_pages: bool,
	app_dir: &Path,
) -> CommandResult<()> {
	use std::fs;
	use toml_edit::{Array, DocumentMut, Item, Table, Value};

	let root_content = fs::read_to_string("Cargo.toml").map_err(|error| {
		CommandError::ExecutionError(format!("Failed to read workspace Cargo.toml: {error}"))
	})?;
	let root = root_content.parse::<DocumentMut>().map_err(|error| {
		CommandError::ExecutionError(format!("Failed to parse workspace Cargo.toml: {error}"))
	})?;
	let native_target = "cfg(not(target_arch = \"wasm32\"))";
	let wasm_target = "cfg(target_arch = \"wasm32\")";

	let mut manifest = "[package]\nname = \"\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[lib]\npath = \"src/lib.rs\"\n"
		.parse::<DocumentMut>()
		.map_err(|error| CommandError::ExecutionError(error.to_string()))?;
	manifest["package"]["name"] = app_name.into();
	if with_pages {
		manifest["lib"]["crate-type"] = {
			let mut array = Array::default();
			array.push("cdylib");
			array.push("rlib");
			toml_edit::Item::Value(Value::Array(array))
		};
	}

	if with_pages {
		let native_source = root["target"][native_target]["dependencies"]["reinhardt"].clone();
		let wasm_source = root["target"][wasm_target]["dependencies"]["reinhardt"].clone();
		if native_source.is_none() || wasm_source.is_none() {
			return Err(CommandError::ExecutionError(
				"Workspace pages app generation requires target-specific reinhardt dependencies in the project manifest."
					.to_string(),
			));
		}
		let target = manifest
			.entry("target")
			.or_insert(Item::Table(Table::new()))
			.as_table_mut()
			.ok_or_else(|| CommandError::ExecutionError("invalid target table".to_string()))?;
		let native = target
			.entry(native_target)
			.or_insert(Item::Table(Table::new()))
			.as_table_mut()
			.ok_or_else(|| {
				CommandError::ExecutionError("invalid native target table".to_string())
			})?;
		let native_dependencies = native
			.entry("dependencies")
			.or_insert(Item::Table(Table::new()))
			.as_table_mut()
			.ok_or_else(|| {
				CommandError::ExecutionError("invalid native dependencies table".to_string())
			})?;
		native_dependencies.insert(
			"reinhardt",
			with_dependency_features(
				rebase_dependency_path(native_source, app_dir)?,
				&["minimal", "pages", "grpc", "websockets"],
			),
		);

		let wasm = target
			.entry(wasm_target)
			.or_insert(Item::Table(Table::new()))
			.as_table_mut()
			.ok_or_else(|| CommandError::ExecutionError("invalid wasm target table".to_string()))?;
		let wasm_dependencies = wasm
			.entry("dependencies")
			.or_insert(Item::Table(Table::new()))
			.as_table_mut()
			.ok_or_else(|| {
				CommandError::ExecutionError("invalid wasm dependencies table".to_string())
			})?;
		wasm_dependencies.insert(
			"reinhardt",
			with_dependency_features(
				rebase_dependency_path(wasm_source, app_dir)?,
				&["pages", "client-router"],
			),
		);
	} else {
		let dependencies = manifest
			.entry("dependencies")
			.or_insert(Item::Table(Table::new()))
			.as_table_mut()
			.ok_or_else(|| {
				CommandError::ExecutionError("invalid dependencies table".to_string())
			})?;
		let source = root
			.get("dependencies")
			.and_then(Item::as_table)
			.and_then(|dependencies| dependencies.get("reinhardt"))
			.cloned()
			.unwrap_or(Item::None);
		dependencies.insert(
			"reinhardt",
			rebase_dependency_path(with_dependency_features(source, &[]), app_dir)?,
		);
	}

	if with_pages {
		let build_dependencies = manifest
			.entry("build-dependencies")
			.or_insert(Item::Table(Table::new()))
			.as_table_mut()
			.ok_or_else(|| {
				CommandError::ExecutionError("invalid build dependencies table".to_string())
			})?;
		build_dependencies.insert("cfg_aliases", "0.2".into());
	}
	manifest["features"]["default"] = toml_edit::Item::Value(Value::Array(Array::default()));

	fs::create_dir_all(app_dir).map_err(|error| {
		CommandError::ExecutionError(format!("Failed to create app directory: {error}"))
	})?;
	fs::write(app_dir.join("Cargo.toml"), manifest.to_string()).map_err(|error| {
		CommandError::ExecutionError(format!("Failed to write workspace app Cargo.toml: {error}"))
	})?;
	Ok(())
}

fn rebase_dependency_path(
	mut item: toml_edit::Item,
	app_dir: &Path,
) -> CommandResult<toml_edit::Item> {
	use toml_edit::{Item, Value};

	let source_path = match &item {
		Item::Value(Value::InlineTable(table)) => table.get("path").and_then(Value::as_str),
		Item::Table(table) => table
			.get("path")
			.and_then(Item::as_value)
			.and_then(Value::as_str),
		_ => None,
	}
	.map(str::to_owned);
	let Some(source_path) = source_path else {
		return Ok(item);
	};
	let source_path = Path::new(&source_path);
	if source_path.is_absolute() {
		return Ok(item);
	}

	let workspace_root = env::current_dir().map_err(|error| {
		CommandError::ExecutionError(format!("Failed to resolve workspace root: {error}"))
	})?;
	let source_path = workspace_root.join(source_path);
	let app_dir = if app_dir.is_absolute() {
		app_dir.to_path_buf()
	} else {
		workspace_root.join(app_dir)
	};
	let rebased = relative_path(&app_dir, &source_path);
	let rebased = rebased.to_string_lossy().replace('\\', "/");

	match &mut item {
		Item::Value(Value::InlineTable(table)) => {
			table.insert("path", rebased.into());
		}
		Item::Table(table) => {
			table.insert("path", Item::Value(Value::from(rebased)));
		}
		_ => {}
	}
	Ok(item)
}

fn relative_path(from: &Path, to: &Path) -> PathBuf {
	use std::path::Component;

	fn normalize(path: &Path) -> PathBuf {
		let mut normalized = PathBuf::new();
		for component in path.components() {
			match component {
				Component::CurDir => {}
				Component::ParentDir => {
					if !normalized.pop() && !normalized.has_root() {
						normalized.push(component.as_os_str());
					}
				}
				_ => normalized.push(component.as_os_str()),
			}
		}
		normalized
	}

	let from = normalize(from);
	let to = normalize(to);
	let from_components: Vec<_> = from
		.components()
		.map(|component| component.as_os_str().to_owned())
		.collect();
	let to_components: Vec<_> = to
		.components()
		.map(|component| component.as_os_str().to_owned())
		.collect();
	let common = from_components
		.iter()
		.zip(&to_components)
		.take_while(|(from, to)| from == to)
		.count();
	let mut relative = PathBuf::new();
	for _ in common..from_components.len() {
		relative.push("..");
	}
	for component in &to_components[common..] {
		relative.push(component);
	}
	if relative.as_os_str().is_empty() {
		PathBuf::from(".")
	} else {
		relative
	}
}

fn with_dependency_features(mut item: toml_edit::Item, features: &[&str]) -> toml_edit::Item {
	use toml_edit::{Array, InlineTable, Item, Value};

	let mut array = Array::default();
	for feature in features {
		array.push(*feature);
	}
	match &mut item {
		Item::Value(Value::InlineTable(table)) => {
			table.insert("features", Value::Array(array));
		}
		Item::Table(table) => {
			table.insert("features", Item::Value(Value::Array(array)));
		}
		_ => {
			let mut table = InlineTable::new();
			table.insert("package", "reinhardt-web".into());
			table.insert("version", env!("CARGO_PKG_VERSION").into());
			table.insert("default-features", false.into());
			table.insert("features", Value::Array(array));
			item = Item::Value(Value::InlineTable(table));
		}
	}
	item
}

/// Generate `build.rs` for a workspace pages app crate (cfg_aliases setup).
fn generate_workspace_build_rs(app_name: &str, app_dir: &Path) -> CommandResult<()> {
	use std::fs;
	use std::io::Write;

	let content = format!(
		"//! Build script for {app_name}.\n\
		 //!\n\
		 //! Sets up cfg aliases for simplified conditional compilation.\n\
		 \n\
		 use cfg_aliases::cfg_aliases;\n\
		 \n\
		 fn main() {{\n\
		 \t// Rust 2024 edition requires explicit check-cfg declarations\n\
		 \tprintln!(\"cargo::rustc-check-cfg=cfg(client)\");\n\
		 \tprintln!(\"cargo::rustc-check-cfg=cfg(server)\");\n\
		 \tprintln!(\"cargo::rustc-check-cfg=cfg(wasm)\");\n\
		 \tprintln!(\"cargo::rustc-check-cfg=cfg(native)\");\n\
		 \n\
		 \tcfg_aliases! {{\n\
		 \t\t// Platform aliases for simpler conditional compilation\n\
		 \t\t// Use `#[cfg(client)]` instead of `#[cfg(target_arch = \"wasm32\")]`\n\
		 \t\tclient: {{ target_arch = \"wasm32\" }},\n\
		 \t\t// Use `#[cfg(server)]` instead of `#[cfg(not(target_arch = \"wasm32\"))]`\n\
		 \t\tserver: {{ not(target_arch = \"wasm32\") }},\n\
		 \t\t// Compatibility aliases used by framework macro expansions.\n\
		 \t\twasm: {{ target_arch = \"wasm32\" }},\n\
		 \t\tnative: {{ not(target_arch = \"wasm32\") }},\n\
		 \t}}\n\
		 }}\n"
	);

	let build_path = app_dir.join("build.rs");
	let mut f = fs::File::create(&build_path).map_err(|e| {
		CommandError::ExecutionError(format!("Failed to create {}: {}", build_path.display(), e))
	})?;
	f.write_all(content.as_bytes()).map_err(|e| {
		CommandError::ExecutionError(format!("Failed to write {}: {}", build_path.display(), e))
	})?;

	Ok(())
}
/// Update the project workspace and dependency tables for a workspace app.
fn update_workspace_manifest(app_name: &str, app_dir: &Path) -> CommandResult<()> {
	use std::fs;
	use toml_edit::{Array, DocumentMut, InlineTable, Item, Value};

	let cargo_toml_path = PathBuf::from("Cargo.toml");
	let content = fs::read_to_string(&cargo_toml_path).map_err(|error| {
		CommandError::ExecutionError(format!("Failed to read Cargo.toml: {error}"))
	})?;
	let mut document = content.parse::<DocumentMut>().map_err(|error| {
		CommandError::ExecutionError(format!("Failed to parse Cargo.toml: {error}"))
	})?;

	let workspace = document
		.get_mut("workspace")
		.and_then(Item::as_table_mut)
		.ok_or_else(|| {
			CommandError::ExecutionError("No [workspace] section found in Cargo.toml.".to_string())
		})?;
	let members = workspace
		.entry("members")
		.or_insert(Item::Value(Value::Array(Array::default())))
		.as_array_mut()
		.ok_or_else(|| {
			CommandError::ExecutionError("Workspace members must be an array.".to_string())
		})?;
	let workspace_root = env::current_dir().map_err(|error| {
		CommandError::ExecutionError(format!("Failed to resolve workspace root: {error}"))
	})?;
	let member_path = if app_dir.is_absolute() {
		app_dir.strip_prefix(&workspace_root).unwrap_or(app_dir)
	} else {
		app_dir
	};
	let member = member_path
		.to_string_lossy()
		.replace('\\', "/")
		.trim_start_matches("./")
		.trim_end_matches('/')
		.to_owned();
	if member.is_empty() {
		return Err(CommandError::InvalidArguments(format!(
			"Workspace app destination for '{}' must not be the workspace root.",
			app_name
		)));
	}
	if !members
		.iter()
		.any(|value| value.as_str() == Some(member.as_str()))
	{
		members.push(member.as_str());
	}

	let dependencies = document
		.entry("dependencies")
		.or_insert(Item::Table(toml_edit::Table::new()))
		.as_table_mut()
		.ok_or_else(|| CommandError::ExecutionError("Dependencies must be a table.".to_string()))?;
	if let Some(existing) = dependencies.get(app_name) {
		let existing_path = existing
			.as_value()
			.and_then(|value| match value {
				Value::InlineTable(table) => table.get("path").and_then(Value::as_str),
				_ => None,
			})
			.or_else(|| {
				existing.as_table().and_then(|table| {
					table
						.get("path")
						.and_then(Item::as_value)
						.and_then(Value::as_str)
				})
			});
		let normalized_existing = existing_path.map(|path| {
			path.replace('\\', "/")
				.trim_start_matches("./")
				.trim_end_matches('/')
				.to_owned()
		});
		if normalized_existing.as_deref() != Some(member.as_str()) {
			return Err(CommandError::InvalidArguments(format!(
				"Dependency '{}' already exists and does not point to generated workspace member '{}'.",
				app_name, member
			)));
		}
	} else {
		let mut dependency = InlineTable::new();
		dependency.insert("path", Value::from(member.clone()));
		dependencies.insert(app_name, Item::Value(Value::InlineTable(dependency)));
	}

	fs::write(&cargo_toml_path, document.to_string()).map_err(|error| {
		CommandError::ExecutionError(format!("Failed to write Cargo.toml: {error}"))
	})?;
	Ok(())
}

fn ensure_native_protocol_features() -> CommandResult<()> {
	use std::fs;
	use toml_edit::{DocumentMut, Item, Table};

	let path = PathBuf::from("Cargo.toml");
	let content = fs::read_to_string(&path).map_err(|error| {
		CommandError::ExecutionError(format!("Failed to read Cargo.toml: {error}"))
	})?;
	let mut document = content.parse::<DocumentMut>().map_err(|error| {
		CommandError::ExecutionError(format!("Failed to parse Cargo.toml: {error}"))
	})?;
	let native_target_key = document
		.get("target")
		.and_then(Item::as_table)
		.and_then(|target| {
			let exact = "cfg(not(target_arch = \"wasm32\"))";
			if target.contains_key(exact) {
				return Some(exact.to_string());
			}
			target.iter().find_map(|(key, item)| {
				let has_native_facade = item
					.as_table()
					.and_then(|table| table.get("dependencies"))
					.and_then(Item::as_table)
					.is_some_and(|dependencies| dependencies.contains_key("reinhardt"));
				(key.contains("not") && key.contains("wasm32") && has_native_facade)
					.then(|| key.to_string())
			})
		});
	let dependencies = if let Some(native_target_key) = native_target_key {
		document
			.get_mut("target")
			.and_then(Item::as_table_mut)
			.and_then(|target| target.get_mut(&native_target_key))
			.and_then(Item::as_table_mut)
			.ok_or_else(|| {
				CommandError::ExecutionError("Native target table must be a table.".to_string())
			})?
			.entry("dependencies")
			.or_insert(Item::Table(Table::new()))
			.as_table_mut()
			.ok_or_else(|| {
				CommandError::ExecutionError("Dependencies must be a table.".to_string())
			})?
	} else {
		document
			.get_mut("dependencies")
			.and_then(Item::as_table_mut)
			.ok_or_else(|| {
				CommandError::ExecutionError(
					"Cargo.toml must define native Reinhardt dependencies in [dependencies] or a non-WASM target table."
						.to_string(),
				)
			})?
	};
	append_dependency_features(dependencies, "reinhardt", &["grpc", "websockets"], false)?;
	append_dependency_features(
		dependencies,
		"reinhardt-commands",
		&["server", "autoreload", "grpc", "websockets"],
		true,
	)?;
	fs::write(path, document.to_string()).map_err(|error| {
		CommandError::ExecutionError(format!("Failed to write Cargo.toml: {error}"))
	})?;
	Ok(())
}

fn append_dependency_features(
	dependencies: &mut toml_edit::Table,
	name: &str,
	required: &[&str],
	optional: bool,
) -> CommandResult<()> {
	use toml_edit::{InlineTable, Item, Value};

	let item = dependencies.entry(name).or_insert_with(|| {
		let mut table = InlineTable::new();
		table.insert("version", env!("CARGO_PKG_VERSION").into());
		table.insert("default-features", false.into());
		table.insert("optional", optional.into());
		Item::Value(Value::InlineTable(table))
	});
	match item {
		Item::Value(Value::InlineTable(table)) => {
			let mut features = table
				.get("features")
				.and_then(|value| match value {
					Value::Array(array) => Some(array.clone()),
					_ => None,
				})
				.unwrap_or_default();
			for feature in required {
				if !features
					.iter()
					.any(|value| value.as_str() == Some(*feature))
				{
					features.push(*feature);
				}
			}
			table.insert("features", Value::Array(features));
		}
		Item::Value(Value::String(version)) => {
			let mut table = InlineTable::new();
			table.insert("version", Value::String(version.clone()));
			let mut features = toml_edit::Array::default();
			for feature in required {
				features.push(*feature);
			}
			table.insert("features", Value::Array(features));
			*item = Item::Value(Value::InlineTable(table));
		}
		Item::Table(table) => {
			let mut features = table
				.get("features")
				.and_then(Item::as_array)
				.cloned()
				.unwrap_or_default();
			for feature in required {
				if !features
					.iter()
					.any(|value| value.as_str() == Some(*feature))
				{
					features.push(*feature);
				}
			}
			table.insert("features", Item::Value(Value::Array(features)));
		}
		_ => {
			return Err(CommandError::ExecutionError(format!(
				"Dependency {name} must be an inline or standard TOML table."
			)));
		}
	}
	Ok(())
}

/// Update or create apps.rs to export the new app using AST
///
/// Uses AST parsing to robustly detect existing module declarations
/// and add new ones, avoiding issues with comments and formatting.
///
/// `with_pages` controls whether the emitted `pub use <app>::<App>Config;`
/// re-export is gated by `#[cfg(server)]`. Pages projects compile `apps.rs`
/// on the WASM target where the `#[app_config]`-generated `Config` struct
/// is itself `#[cfg(server)]`, so the re-export must match. REST projects
/// do not define a `server` cfg alias and the `Config` struct is not
/// cfg-gated, so adding `#[cfg(server)]` there would silently drop the
/// re-export (and would emit an `unexpected_cfgs` warning) — keep it
/// un-gated for REST.
fn update_apps_export(app_name: &str, with_pages: bool) -> CommandResult<()> {
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

		// Add use declaration: `pub use app_name::AppNameConfig;`, gated
		// by `#[cfg(server)]` only for Pages projects.
		//
		// The `Config` struct is created by `#[app_config(...)]`. In Pages
		// projects, that struct is itself server-only (`#[cfg(server)]`)
		// and `apps.rs` compiles on the WASM target as well, so the
		// re-export must match — leaving it ungated would produce E0432
		// "unresolved import" at the `apps.rs` line on the WASM build.
		// REST projects do not define a `server` cfg alias and do not
		// gate the `Config` struct, so adding `#[cfg(server)]` there
		// would silently drop the re-export (and emit `unexpected_cfgs`).
		let config_name = format!("{}Config", camel_case_name);
		let config_ident = syn::Ident::new(&config_name, proc_macro2::Span::call_site());
		let use_item: ItemUse = if with_pages {
			syn::parse_quote! {
				#[cfg(server)]
				pub use #app_ident::#config_ident;
			}
		} else {
			syn::parse_quote! {
				pub use #app_ident::#config_ident;
			}
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
/// Issue #3670: typed route declarations require the app's label to be
/// registered via `installed_apps!`.
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

//! # Start Commands
//!
//! Django's startproject and startapp commands translation to Rust
//!
//! Source:
//! - django/core/management/commands/startproject.py
//! - django/core/management/commands/startapp.py

use crate::{
	BaseCommand, CommandArgument, CommandContext, CommandError, CommandOption, CommandResult,
	TemplateCommand, TemplateContext, generate_secret_key, to_camel_case,
};
use async_trait::async_trait;
use std::env;
use std::path::{Path, PathBuf};

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

		let target = ctx.arg(1).map(PathBuf::from);

		// Determine project type
		let is_restful = ctx.has_option("restful");
		let with_pages = ctx.has_option("with-pages");

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
		context.insert("secret_key", &secret_key)?;
		context.insert("camel_case_project_name", to_camel_case(&project_name))?;
		context.insert("reinhardt_version", env!("CARGO_PKG_VERSION"))?;
		context.insert("is_restful", if !with_pages { "true" } else { "false" })?;
		context.insert("with_pages", if with_pages { "true" } else { "false" })?;

		// Determine template directory
		let template_dir = if let Some(template_path) = ctx.option("template") {
			PathBuf::from(template_path)
		} else {
			// Use built-in template based on project type
			get_project_template_dir(template_key)?
		};

		// Create project using TemplateCommand
		let template_cmd = TemplateCommand::new();
		template_cmd.handle(
			&project_name,
			target.as_deref(),
			&template_dir,
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

		let target = ctx.arg(1).map(PathBuf::from);

		// Determine app type and structure
		let is_restful = ctx.has_option("restful");
		let with_pages = ctx.has_option("with-pages");
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
			let app_target = if target.is_some() {
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

			// Determine template directory
			let template_dir = if let Some(template_path) = ctx.option("template") {
				PathBuf::from(template_path)
			} else {
				// Use built-in template based on app type
				get_app_template_dir(template_key)?
			};

			// Create app using TemplateCommand
			let template_cmd = TemplateCommand::new();
			template_cmd.handle(
				&app_name,
				app_target.as_deref(),
				&template_dir,
				context,
				ctx,
			)?;

			// Update or create apps.rs to export the new app
			update_apps_export(&app_name)?;

			ctx.success(&format!(
				"{} app '{}' created successfully in src/apps/{}!",
				app_type, app_name, app_name
			));
			ctx.info("The app has been added to src/apps.rs");
			ctx.info("Don't forget to add it to INSTALLED_APPS in your settings.rs");
		}

		Ok(())
	}
}

/// Get the path to the built-in project template directory
fn get_project_template_dir(template_type: &str) -> CommandResult<PathBuf> {
	// template_type: "mvc" or "restful"
	let manifest_dir = env!("CARGO_MANIFEST_DIR");
	let template_dir = PathBuf::from(manifest_dir)
		.join("templates")
		.join(format!("project_{}_template", template_type));

	if !template_dir.exists() {
		return Err(CommandError::ExecutionError(format!(
			"Project template directory not found at {}. Falling back to default template.",
			template_dir.display()
		)));
	}

	Ok(template_dir)
}

/// Get the path to the built-in app template directory
fn get_app_template_dir(template_type: &str) -> CommandResult<PathBuf> {
	// template_type: "mvc" or "restful"
	let manifest_dir = env!("CARGO_MANIFEST_DIR");
	let template_dir = PathBuf::from(manifest_dir)
		.join("templates")
		.join(format!("app_{}_template", template_type));

	if !template_dir.exists() {
		return Err(CommandError::ExecutionError(format!(
			"App template directory not found at {}. Falling back to default template.",
			template_dir.display()
		)));
	}

	Ok(template_dir)
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

	// Determine template directory for workspace apps
	let template_key = if with_pages { "pages" } else { "restful" };
	let template_dir = get_app_workspace_template_dir(template_key)?;

	// Create app using TemplateCommand
	let template_cmd = TemplateCommand::new();
	template_cmd.handle(app_name, Some(&app_target), &template_dir, context, ctx)?;

	// Update workspace Cargo.toml
	update_workspace_members(app_name)?;

	Ok(())
}

/// Get the path to the built-in workspace app template directory
fn get_app_workspace_template_dir(template_type: &str) -> CommandResult<PathBuf> {
	// template_type: "mvc" or "restful"
	let manifest_dir = env!("CARGO_MANIFEST_DIR");
	let template_dir = PathBuf::from(manifest_dir)
		.join("templates")
		.join(format!("app_{}_workspace_template", template_type));

	if !template_dir.exists() {
		return Err(CommandError::ExecutionError(format!(
			"Workspace app template directory not found at {}.",
			template_dir.display()
		)));
	}

	Ok(template_dir)
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
		let config_ident = syn::Ident::new(&camel_case_name, proc_macro2::Span::call_site());
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

		cmd.handle(
			"test",
			Some(output_dir.path()),
			template_dir.path(),
			context,
			&ctx,
		)
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

		cmd.handle(
			"test",
			Some(output_dir.path()),
			template_dir.path(),
			context,
			&ctx,
		)
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
}

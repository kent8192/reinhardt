//! Template utilities for command code generation

use crate::CommandResult;
use crate::template_source::TemplateSource;
use crate::{BaseCommand, CommandContext};
use async_trait::async_trait;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use tera::Tera;

/// Context for template rendering, holding key-value pairs passed to Tera templates.
///
/// Supports example overrides: when rendering `.example.` files, override values
/// are substituted for specified keys so that example files contain safe placeholder
/// strings while the actual settings files receive the real generated values.
#[derive(Debug, Clone)]
pub struct TemplateContext {
	variables: HashMap<String, JsonValue>,
	example_overrides: HashMap<String, JsonValue>,
}

impl From<TemplateContext> for tera::Context {
	fn from(ctx: TemplateContext) -> Self {
		let mut tera_ctx = tera::Context::new();
		for (key, value) in ctx.variables {
			tera_ctx.insert(key, &value);
		}
		tera_ctx
	}
}

impl TemplateContext {
	/// Creates a new empty template context.
	pub fn new() -> Self {
		Self {
			variables: HashMap::new(),
			example_overrides: HashMap::new(),
		}
	}

	/// Inserts a serializable value into the context under the given key.
	pub fn insert<K, V>(&mut self, key: K, value: V) -> Result<(), serde_json::Error>
	where
		K: Into<String>,
		V: Serialize,
	{
		let json_value = serde_json::to_value(value)?;
		self.variables.insert(key.into(), json_value);
		Ok(())
	}

	/// Sets an override value for `.example.` files.
	///
	/// When rendering `.example.` files, the override value is used instead of
	/// the normal value for this key. This allows example files to contain safe
	/// placeholder strings while actual settings files receive real values.
	pub fn set_example_override<K, V>(&mut self, key: K, value: V) -> Result<(), serde_json::Error>
	where
		K: Into<String>,
		V: Serialize,
	{
		let json_value = serde_json::to_value(value)?;
		self.example_overrides.insert(key.into(), json_value);
		Ok(())
	}

	/// Creates a context for rendering `.example.` files by applying overrides.
	fn to_example_context(&self) -> Self {
		let mut ctx = self.clone();
		for (key, value) in &self.example_overrides {
			ctx.variables.insert(key.clone(), value.clone());
		}
		ctx
	}
}

impl Default for TemplateContext {
	fn default() -> Self {
		Self::new()
	}
}

/// Command that processes a template directory, rendering Tera templates into output files.
pub struct TemplateCommand;

impl TemplateCommand {
	/// Creates a new template command instance.
	pub fn new() -> Self {
		Self
	}

	/// Processes templates from the given source, rendering them with the provided context.
	pub fn handle(
		&self,
		name: &str,
		target: Option<&std::path::Path>,
		source: &dyn TemplateSource,
		context: TemplateContext,
		ctx: &CommandContext,
	) -> CommandResult<()> {
		use crate::CommandError;
		use std::fs;
		use std::path::Path;

		let output_dir = if let Some(t) = target {
			t.to_path_buf()
		} else {
			std::path::PathBuf::from(name)
		};

		if output_dir.exists() {
			ctx.verbose(&format!(
				"Directory '{}' already exists, will write into it",
				output_dir.display()
			));
		} else {
			fs::create_dir_all(&output_dir).map_err(|e| {
				CommandError::ExecutionError(format!(
					"Failed to create output directory '{}': {}",
					output_dir.display(),
					e
				))
			})?;
		}

		self.process_directory(source, Path::new(""), &output_dir, &context, ctx)
	}

	fn process_directory(
		&self,
		source: &dyn TemplateSource,
		rel_dir: &std::path::Path,
		output_base: &std::path::Path,
		context: &TemplateContext,
		ctx: &CommandContext,
	) -> CommandResult<()> {
		use crate::CommandError;
		use std::fs;

		let entries = source.list_entries(rel_dir)?;

		for entry in entries {
			let file_name = entry
				.rel_path
				.file_name()
				.map(|s| s.to_string_lossy().into_owned())
				.unwrap_or_default();

			// Skip hidden files and __pycache__, but keep .gitkeep and .gitignore(.tpl).
			// Strip the .tpl extension before comparing so that `.gitignore.tpl` is also
			// recognized as the allowed dotfile `.gitignore`.
			let base_name = file_name.strip_suffix(".tpl").unwrap_or(&file_name);
			if (file_name.starts_with('.') && base_name != ".gitkeep" && base_name != ".gitignore")
				|| file_name == "__pycache__"
			{
				continue;
			}

			if entry.is_dir {
				let output_dir = output_base.join(&entry.rel_path);
				fs::create_dir_all(&output_dir).map_err(|e| {
					CommandError::ExecutionError(format!(
						"Failed to create directory '{}': {}",
						output_dir.display(),
						e
					))
				})?;
				self.process_directory(source, &entry.rel_path, output_base, context, ctx)?;
			} else {
				self.process_file(source, &entry.rel_path, output_base, context, ctx)?;
			}
		}

		Ok(())
	}

	fn process_file(
		&self,
		source: &dyn TemplateSource,
		rel_path: &std::path::Path,
		output_base: &std::path::Path,
		context: &TemplateContext,
		ctx: &CommandContext,
	) -> CommandResult<()> {
		use crate::CommandError;
		use std::fs;
		use std::io::Write;

		let file_path_str = rel_path.to_str().ok_or_else(|| {
			CommandError::ExecutionError("Invalid UTF-8 in file path".to_string())
		})?;

		let mut processed_name = file_path_str.to_string();

		// Remove .tpl extension if present
		if processed_name.ends_with(".tpl") {
			processed_name = processed_name[..processed_name.len() - 4].to_string();
		}

		// Check if this is an .example file
		let has_example_suffix = processed_name.contains(".example.");

		let output_path_with_example = output_base.join(&processed_name);

		if let Some(parent) = output_path_with_example.parent() {
			fs::create_dir_all(parent).map_err(|e| {
				CommandError::ExecutionError(format!(
					"Failed to create parent directory for '{}': {}",
					output_path_with_example.display(),
					e
				))
			})?;
		}

		// Read template content via the abstracted source
		let raw = source.read_file(rel_path)?;
		let template_content = std::str::from_utf8(&raw)
			.map_err(|e| {
				CommandError::ExecutionError(format!(
					"template '{}' is not valid UTF-8: {}",
					rel_path.display(),
					e
				))
			})?
			.to_string();

		if has_example_suffix {
			let example_context = context.to_example_context();
			let example_content = self.render_template(&template_content, &example_context)?;

			let mut output_file = fs::File::create(&output_path_with_example).map_err(|e| {
				CommandError::ExecutionError(format!(
					"Failed to create output file '{}': {}",
					output_path_with_example.display(),
					e
				))
			})?;
			output_file
				.write_all(example_content.as_bytes())
				.map_err(|e| {
					CommandError::ExecutionError(format!(
						"Failed to write to output file '{}': {}",
						output_path_with_example.display(),
						e
					))
				})?;
			ctx.verbose(&format!(
				"Created: {}",
				output_path_with_example
					.strip_prefix(output_base)
					.unwrap_or(&output_path_with_example)
					.display()
			));

			let rendered_content = self.render_template(&template_content, context)?;
			let processed_name_without_example =
				if let Some(pos) = processed_name.rfind(".example.") {
					format!("{}{}", &processed_name[..pos], &processed_name[pos + 8..])
				} else {
					processed_name.clone()
				};

			let output_path_without_example = output_base.join(processed_name_without_example);
			let mut output_file_no_example = fs::File::create(&output_path_without_example)
				.map_err(|e| {
					CommandError::ExecutionError(format!(
						"Failed to create output file '{}': {}",
						output_path_without_example.display(),
						e
					))
				})?;
			output_file_no_example
				.write_all(rendered_content.as_bytes())
				.map_err(|e| {
					CommandError::ExecutionError(format!(
						"Failed to write to output file '{}': {}",
						output_path_without_example.display(),
						e
					))
				})?;
			ctx.verbose(&format!(
				"Created: {}",
				output_path_without_example
					.strip_prefix(output_base)
					.unwrap_or(&output_path_without_example)
					.display()
			));
		} else {
			let rendered_content = self.render_template(&template_content, context)?;

			let mut output_file = fs::File::create(&output_path_with_example).map_err(|e| {
				CommandError::ExecutionError(format!(
					"Failed to create output file '{}': {}",
					output_path_with_example.display(),
					e
				))
			})?;
			output_file
				.write_all(rendered_content.as_bytes())
				.map_err(|e| {
					CommandError::ExecutionError(format!(
						"Failed to write to output file '{}': {}",
						output_path_with_example.display(),
						e
					))
				})?;
			ctx.verbose(&format!(
				"Created: {}",
				output_path_with_example
					.strip_prefix(output_base)
					.unwrap_or(&output_path_with_example)
					.display()
			));
		}

		Ok(())
	}

	fn render_template(&self, template: &str, context: &TemplateContext) -> CommandResult<String> {
		let tera_context: tera::Context = context.clone().into();
		Tera::one_off(template, &tera_context, false)
			.map_err(|e| crate::CommandError::TemplateError(e.to_string()))
	}
}

impl Default for TemplateCommand {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl BaseCommand for TemplateCommand {
	fn name(&self) -> &str {
		"template"
	}

	async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
		use crate::CommandError;

		let name = ctx
			.arg(0)
			.ok_or_else(|| CommandError::InvalidArguments("You must provide a name.".to_string()))?
			.clone();

		let target = ctx.arg(1).map(std::path::PathBuf::from);

		let template_dir = ctx.option("template").ok_or_else(|| {
			CommandError::InvalidArguments(
				"You must provide a template directory via --template.".to_string(),
			)
		})?;

		let source = crate::template_source::FilesystemSource::new(template_dir)?;

		let context = TemplateContext::new();

		self.handle(&name, target.as_deref(), &source, context, ctx)?;

		ctx.success("Template processed successfully");

		Ok(())
	}
}

/// Generate a Django-compatible secret key
pub fn generate_secret_key() -> String {
	use rand::Rng;
	const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz\
                             ABCDEFGHIJKLMNOPQRSTUVWXYZ\
                             0123456789\
                             !@#$%^&*(-_=+)";
	let mut rng = rand::rng();
	(0..50)
		.map(|_| {
			let idx = rng.random_range(0..CHARSET.len());
			CHARSET[idx] as char
		})
		.collect()
}

/// Convert a string to CamelCase
pub fn to_camel_case(s: &str) -> String {
	s.split(['_', '-'])
		.filter(|part| !part.is_empty())
		.map(|part| {
			let mut chars = part.chars();
			match chars.next() {
				None => String::new(),
				Some(first) => {
					format!("{}{}", first.to_uppercase(), chars.as_str().to_lowercase())
				}
			}
		})
		.collect()
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::CommandError;
	use crate::template_source::TemplateEntry;
	use std::borrow::Cow;
	use std::path::{Path, PathBuf};
	use tempfile::TempDir;

	struct UnavailableSource;

	impl TemplateSource for UnavailableSource {
		fn list_entries(&self, _rel: &Path) -> CommandResult<Vec<TemplateEntry>> {
			Ok(vec![TemplateEntry {
				rel_path: PathBuf::from("missing.tpl"),
				is_dir: false,
			}])
		}

		fn read_file(&self, rel: &Path) -> CommandResult<Cow<'_, [u8]>> {
			Err(CommandError::ExecutionError(format!(
				"template source unavailable: {}",
				rel.display()
			)))
		}

		fn exists(&self, _rel: &Path) -> bool {
			false
		}
	}

	fn filesystem_source(dir: &TempDir) -> crate::template_source::FilesystemSource {
		crate::template_source::FilesystemSource::new(dir.path()).unwrap()
	}

	#[test]
	fn test_render_template_without_spaces() {
		let template_cmd = TemplateCommand::new();
		let mut context = TemplateContext::new();
		context.insert("project_name", "my_project").unwrap();
		context.insert("version", "1.0.0").unwrap();

		let template = "name = \"{{project_name}}\"\nversion = \"{{version}}\"";
		let result = template_cmd.render_template(template, &context).unwrap();

		assert_eq!(result, "name = \"my_project\"\nversion = \"1.0.0\"");
	}

	#[test]
	fn test_render_template_with_spaces() {
		let template_cmd = TemplateCommand::new();
		let mut context = TemplateContext::new();
		context.insert("project_name", "my_project").unwrap();
		context.insert("version", "1.0.0").unwrap();

		let template = "name = \"{{ project_name }}\"\nversion = \"{{ version }}\"";
		let result = template_cmd.render_template(template, &context).unwrap();

		assert_eq!(result, "name = \"my_project\"\nversion = \"1.0.0\"");
	}

	#[test]
	fn test_render_template_mixed_formats() {
		let template_cmd = TemplateCommand::new();
		let mut context = TemplateContext::new();
		context.insert("project_name", "my_project").unwrap();
		context.insert("version", "1.0.0").unwrap();

		let template = "name = \"{{ project_name }}\"\nversion = \"{{version}}\"";
		let result = template_cmd.render_template(template, &context).unwrap();

		assert_eq!(result, "name = \"my_project\"\nversion = \"1.0.0\"");
	}

	#[test]
	fn test_render_template_no_variables() {
		let template_cmd = TemplateCommand::new();
		let context = TemplateContext::new();

		let template = "name = \"static_value\"\nversion = \"1.0.0\"";
		let result = template_cmd.render_template(template, &context).unwrap();

		assert_eq!(result, template);
	}

	#[test]
	fn test_render_template_undefined_variable() {
		let template_cmd = TemplateCommand::new();
		let context = TemplateContext::new();

		let template = "name = \"{{ undefined_var }}\"";
		let result = template_cmd.render_template(template, &context);

		// Undefined variables cause an error in Tera
		assert!(result.is_err());
	}

	#[test]
	fn test_to_example_context_applies_overrides() {
		// Arrange
		let mut ctx = TemplateContext::new();
		ctx.insert("secret_key", "real-key").unwrap();
		ctx.insert("project_name", "my_project").unwrap();
		ctx.set_example_override("secret_key", "PLACEHOLDER")
			.unwrap();

		// Act
		let example_ctx = ctx.to_example_context();

		// Assert - example context should have override applied
		let template_cmd = TemplateCommand::new();
		let example_result = template_cmd
			.render_template("{{ secret_key }}", &example_ctx)
			.unwrap();
		assert_eq!(example_result, "PLACEHOLDER");

		// Assert - original context should retain real value
		let real_result = template_cmd
			.render_template("{{ secret_key }}", &ctx)
			.unwrap();
		assert_eq!(real_result, "real-key");

		// Assert - non-overridden keys should be the same in both
		let example_name = template_cmd
			.render_template("{{ project_name }}", &example_ctx)
			.unwrap();
		let real_name = template_cmd
			.render_template("{{ project_name }}", &ctx)
			.unwrap();
		assert_eq!(example_name, "my_project");
		assert_eq!(real_name, "my_project");
	}

	#[test]
	fn test_set_example_override_returns_ok() {
		let mut ctx = TemplateContext::new();
		let result = ctx.set_example_override("key", "value");
		assert!(result.is_ok());
	}

	#[test]
	fn test_example_context_with_no_overrides_is_identical() {
		// Arrange
		let mut ctx = TemplateContext::new();
		ctx.insert("key", "value").unwrap();
		// No overrides set

		// Act
		let example_ctx = ctx.to_example_context();

		// Assert
		let template_cmd = TemplateCommand::new();
		let original = template_cmd.render_template("{{ key }}", &ctx).unwrap();
		let example = template_cmd
			.render_template("{{ key }}", &example_ctx)
			.unwrap();
		assert_eq!(original, example);
	}

	#[test]
	fn handle_renders_nested_tpl_file_at_the_suffix_stripped_path() {
		// Arrange
		let template_dir = TempDir::new().unwrap();
		let output_dir = TempDir::new().unwrap();
		std::fs::create_dir_all(template_dir.path().join("nested/config")).unwrap();
		std::fs::write(
			template_dir.path().join("nested/config/settings.toml.tpl"),
			"name = \"{{ project }}\"\n",
		)
		.unwrap();
		let mut context = TemplateContext::new();
		context.insert("project", "demo").unwrap();

		// Act
		TemplateCommand::new()
			.handle(
				"ignored",
				Some(output_dir.path()),
				&filesystem_source(&template_dir),
				context,
				&CommandContext::new(vec![]),
			)
			.unwrap();

		// Assert
		assert_eq!(
			std::fs::read_to_string(output_dir.path().join("nested/config/settings.toml")).unwrap(),
			"name = \"demo\"\n"
		);
		assert!(
			!output_dir
				.path()
				.join("nested/config/settings.toml.tpl")
				.exists()
		);
	}

	#[test]
	fn handle_renders_example_copy_with_overrides_and_normal_copy_without_them() {
		// Arrange
		let template_dir = TempDir::new().unwrap();
		let output_dir = TempDir::new().unwrap();
		std::fs::write(
			template_dir.path().join("settings.example.toml.tpl"),
			"token = \"{{ token }}\"\n",
		)
		.unwrap();
		let mut context = TemplateContext::new();
		context.insert("token", "generated-token").unwrap();
		context.set_example_override("token", "replace-me").unwrap();

		// Act
		TemplateCommand::new()
			.handle(
				"ignored",
				Some(output_dir.path()),
				&filesystem_source(&template_dir),
				context,
				&CommandContext::new(vec![]),
			)
			.unwrap();

		// Assert
		assert_eq!(
			std::fs::read_to_string(output_dir.path().join("settings.example.toml")).unwrap(),
			"token = \"replace-me\"\n"
		);
		assert_eq!(
			std::fs::read_to_string(output_dir.path().join("settings.toml")).unwrap(),
			"token = \"generated-token\"\n"
		);
	}

	#[test]
	fn handle_returns_template_error_for_undefined_variable() {
		// Arrange
		let template_dir = TempDir::new().unwrap();
		let output_dir = TempDir::new().unwrap();
		std::fs::write(template_dir.path().join("settings.tpl"), "{{ undefined }}").unwrap();

		// Act
		let error = TemplateCommand::new()
			.handle(
				"ignored",
				Some(output_dir.path()),
				&filesystem_source(&template_dir),
				TemplateContext::new(),
				&CommandContext::new(vec![]),
			)
			.unwrap_err();

		// Assert
		assert!(matches!(error, CommandError::TemplateError(_)));
	}

	#[test]
	fn handle_returns_template_error_for_invalid_tera_syntax() {
		// Arrange
		let template_dir = TempDir::new().unwrap();
		let output_dir = TempDir::new().unwrap();
		std::fs::write(template_dir.path().join("settings.tpl"), "{% if project %}").unwrap();

		// Act
		let error = TemplateCommand::new()
			.handle(
				"ignored",
				Some(output_dir.path()),
				&filesystem_source(&template_dir),
				TemplateContext::new(),
				&CommandContext::new(vec![]),
			)
			.unwrap_err();

		// Assert
		assert!(matches!(error, CommandError::TemplateError(_)));
	}

	#[test]
	fn handle_reports_execution_error_when_destination_parent_is_a_file() {
		// Arrange
		let template_dir = TempDir::new().unwrap();
		let output_dir = TempDir::new().unwrap();
		std::fs::create_dir_all(template_dir.path().join("nested")).unwrap();
		std::fs::write(template_dir.path().join("nested/settings.tpl"), "ready").unwrap();
		std::fs::write(output_dir.path().join("nested"), "not a directory").unwrap();

		// Act
		let error = TemplateCommand::new()
			.handle(
				"ignored",
				Some(output_dir.path()),
				&filesystem_source(&template_dir),
				TemplateContext::new(),
				&CommandContext::new(vec![]),
			)
			.unwrap_err();

		// Assert
		assert!(matches!(error, CommandError::ExecutionError(_)));
	}

	#[test]
	fn handle_preserves_unavailable_source_error() {
		// Arrange
		let output_dir = TempDir::new().unwrap();

		// Act
		let error = TemplateCommand::new()
			.handle(
				"ignored",
				Some(output_dir.path()),
				&UnavailableSource,
				TemplateContext::new(),
				&CommandContext::new(vec![]),
			)
			.unwrap_err();

		// Assert
		assert_eq!(
			error.to_string(),
			"Execution error: template source unavailable: missing.tpl"
		);
	}
}

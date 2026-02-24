//! Template utilities for command code generation

use crate::CommandResult;
use crate::{BaseCommand, CommandContext};
use async_trait::async_trait;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use tera::Tera;

#[derive(Debug, Clone)]
pub struct TemplateContext {
	variables: HashMap<String, JsonValue>,
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
	pub fn new() -> Self {
		Self {
			variables: HashMap::new(),
		}
	}

	pub fn insert<K, V>(&mut self, key: K, value: V) -> Result<(), serde_json::Error>
	where
		K: Into<String>,
		V: Serialize,
	{
		let json_value = serde_json::to_value(value)?;
		self.variables.insert(key.into(), json_value);
		Ok(())
	}
}

impl Default for TemplateContext {
	fn default() -> Self {
		Self::new()
	}
}

pub struct TemplateCommand;

impl TemplateCommand {
	pub fn new() -> Self {
		Self
	}

	pub fn handle(
		&self,
		name: &str,
		target: Option<&std::path::Path>,
		template_dir: &std::path::Path,
		context: TemplateContext,
		ctx: &CommandContext,
	) -> CommandResult<()> {
		use crate::CommandError;
		use std::fs;

		// Validate template directory exists
		if !template_dir.exists() {
			return Err(CommandError::ExecutionError(format!(
				"Template directory does not exist: {}",
				template_dir.display()
			)));
		}

		// Determine output directory
		let output_dir = if let Some(t) = target {
			t.to_path_buf()
		} else {
			std::path::PathBuf::from(name)
		};

		// Create output directory
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

		// Process all files in template directory recursively
		self.process_directory(template_dir, &output_dir, template_dir, &context, ctx)?;

		Ok(())
	}

	fn process_directory(
		&self,
		current_dir: &std::path::Path,
		output_base: &std::path::Path,
		template_base: &std::path::Path,
		context: &TemplateContext,
		ctx: &CommandContext,
	) -> CommandResult<()> {
		use crate::CommandError;
		use std::fs;

		let entries = fs::read_dir(current_dir).map_err(|e| {
			CommandError::ExecutionError(format!(
				"Failed to read template directory '{}': {}",
				current_dir.display(),
				e
			))
		})?;

		for entry in entries {
			let entry = entry.map_err(|e| {
				CommandError::ExecutionError(format!("Failed to read directory entry: {}", e))
			})?;

			let path = entry.path();
			let file_name = entry.file_name();
			let file_name_str = file_name.to_string_lossy();

			// Skip hidden files and __pycache__, but keep .gitkeep and .gitignore
			if (file_name_str.starts_with('.')
				&& file_name_str != ".gitkeep"
				&& file_name_str != ".gitignore")
				|| file_name_str == "__pycache__"
			{
				continue;
			}

			// Calculate relative path from template base
			let relative_path = path.strip_prefix(template_base).map_err(|e| {
				CommandError::ExecutionError(format!("Failed to compute relative path: {}", e))
			})?;

			if path.is_dir() {
				// Create corresponding directory in output
				let output_dir = output_base.join(relative_path);
				fs::create_dir_all(&output_dir).map_err(|e| {
					CommandError::ExecutionError(format!(
						"Failed to create directory '{}': {}",
						output_dir.display(),
						e
					))
				})?;

				// Recursively process subdirectory
				self.process_directory(&path, output_base, template_base, context, ctx)?;
			} else {
				// Process file
				self.process_file(&path, output_base, template_base, context, ctx)?;
			}
		}

		Ok(())
	}

	fn process_file(
		&self,
		template_file: &std::path::Path,
		output_base: &std::path::Path,
		template_base: &std::path::Path,
		context: &TemplateContext,
		ctx: &CommandContext,
	) -> CommandResult<()> {
		use crate::CommandError;
		use std::fs;
		use std::io::Write;

		// Calculate relative path from template base
		let relative_path = template_file.strip_prefix(template_base).map_err(|e| {
			CommandError::ExecutionError(format!("Failed to compute relative path: {}", e))
		})?;

		// Determine output file names
		// We'll process .tpl extension and potentially create two files for .example files
		let file_path_str = relative_path.to_str().ok_or_else(|| {
			CommandError::ExecutionError("Invalid UTF-8 in file path".to_string())
		})?;

		let mut processed_name = file_path_str.to_string();

		// Remove .tpl extension if present
		if processed_name.ends_with(".tpl") {
			processed_name = processed_name[..processed_name.len() - 4].to_string();
		}

		// Check if this is an .example file
		let has_example_suffix = processed_name.contains(".example.");

		// Create the file with .example (original name after .tpl removal)
		let output_path_with_example = output_base.join(&processed_name);

		// Ensure parent directory exists
		if let Some(parent) = output_path_with_example.parent() {
			fs::create_dir_all(parent).map_err(|e| {
				CommandError::ExecutionError(format!(
					"Failed to create parent directory for '{}': {}",
					output_path_with_example.display(),
					e
				))
			})?;
		}

		// Read template content
		let template_content = fs::read_to_string(template_file).map_err(|e| {
			CommandError::ExecutionError(format!(
				"Failed to read template file '{}': {}",
				template_file.display(),
				e
			))
		})?;

		// Replace template variables
		let rendered_content = self.render_template(&template_content, context)?;

		// Write the file with .example suffix (if it has one)
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

		// If file has .example suffix, also create a version without it
		if has_example_suffix {
			let processed_name_without_example =
				if let Some(pos) = processed_name.rfind(".example.") {
					format!(
						"{}{}",
						&processed_name[..pos],
						&processed_name[pos + 8..] // ".example" is 8 characters
					)
				} else {
					processed_name.clone()
				};

			let output_path_without_example = output_base.join(processed_name_without_example);

			// Write the same content to the file without .example
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

		let template_path = std::path::PathBuf::from(template_dir);

		let context = TemplateContext::new();

		self.handle(&name, target.as_deref(), &template_path, context, ctx)?;

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
	let mut rng = rand::thread_rng();
	(0..50)
		.map(|_| {
			let idx = rng.gen_range(0..CHARSET.len());
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
}

//! Settings documentation generation
//!
//! Automatically generates documentation from settings structures using
//! derive macros and doc comments.

use serde_json::Value;

/// Documentation for a single setting
#[derive(Debug, Clone)]
pub struct SettingDoc {
	/// Setting key/name
	pub key: String,

	/// Type of the setting
	pub type_name: String,

	/// Description from doc comments
	pub description: Option<String>,

	/// Default value
	pub default: Option<String>,

	/// Whether the setting is required
	pub required: bool,

	/// Example values
	pub examples: Vec<String>,

	/// Related settings
	pub related: Vec<String>,

	/// Validation constraints
	pub constraints: Vec<String>,

	/// Since which version this setting exists
	pub since: Option<String>,

	/// Whether this setting is deprecated
	pub deprecated: Option<String>,
}

impl SettingDoc {
	/// Create a new setting documentation
	pub fn new(key: impl Into<String>, type_name: impl Into<String>) -> Self {
		Self {
			key: key.into(),
			type_name: type_name.into(),
			description: None,
			default: None,
			required: false,
			examples: Vec::new(),
			related: Vec::new(),
			constraints: Vec::new(),
			since: None,
			deprecated: None,
		}
	}
	/// Add a description
	pub fn with_description(mut self, desc: impl Into<String>) -> Self {
		self.description = Some(desc.into());
		self
	}
	/// Add a default value
	pub fn with_default(mut self, default: impl Into<String>) -> Self {
		self.default = Some(default.into());
		self
	}
	/// Mark as required
	///
	pub fn required(mut self) -> Self {
		self.required = true;
		self
	}
	/// Add an example
	///
	pub fn add_example(mut self, example: impl Into<String>) -> Self {
		self.examples.push(example.into());
		self
	}
	/// Add a constraint
	///
	pub fn add_constraint(mut self, constraint: impl Into<String>) -> Self {
		self.constraints.push(constraint.into());
		self
	}
	/// Mark as deprecated
	///
	pub fn deprecated_since(mut self, version: impl Into<String>) -> Self {
		self.deprecated = Some(version.into());
		self
	}
}

/// Documentation for a group of settings
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct SettingsGroup {
	/// Group name
	pub name: String,

	/// Group description
	pub description: Option<String>,

	/// Settings in this group
	pub settings: Vec<SettingDoc>,

	/// Subgroups
	pub subgroups: Vec<SettingsGroup>,
}

impl SettingsGroup {
	/// Create a new settings group
	pub fn new(name: impl Into<String>) -> Self {
		Self {
			name: name.into(),
			description: None,
			settings: Vec::new(),
			subgroups: Vec::new(),
		}
	}
	/// Add a description
	pub fn with_description(mut self, desc: impl Into<String>) -> Self {
		self.description = Some(desc.into());
		self
	}
	/// Add a setting
	///
	pub fn add_setting(mut self, setting: SettingDoc) -> Self {
		self.settings.push(setting);
		self
	}
	/// Add a subgroup
	///
	pub fn add_subgroup(mut self, group: SettingsGroup) -> Self {
		self.subgroups.push(group);
		self
	}
}

/// Documentation generator
pub struct DocsGenerator {
	groups: Vec<SettingsGroup>,
}

impl DocsGenerator {
	/// Create a new documentation generator
	pub fn new() -> Self {
		Self { groups: Vec::new() }
	}
	/// Add a settings group
	///
	pub fn add_group(&mut self, group: SettingsGroup) {
		self.groups.push(group);
	}
	/// Generate Markdown documentation
	///
	pub fn generate_markdown(&self) -> String {
		let mut output = String::new();

		output.push_str("# Settings Documentation\n\n");
		output.push_str("Auto-generated settings documentation.\n\n");

		output.push_str("## Table of Contents\n\n");
		for group in &self.groups {
			output.push_str(&format!(
				"- [{}](#{})\n",
				group.name,
				Self::anchor(&group.name)
			));
		}
		output.push_str("\n---\n\n");

		for group in &self.groups {
			self.generate_group_markdown(&mut output, group, 2);
		}

		output
	}

	fn generate_group_markdown(&self, output: &mut String, group: &SettingsGroup, level: usize) {
		// Group heading
		output.push_str(&format!("{} {}\n\n", "#".repeat(level), group.name));

		// Group description
		if let Some(desc) = &group.description {
			output.push_str(&format!("{}\n\n", desc));
		}

		// Settings table
		if !group.settings.is_empty() {
			output.push_str("| Setting | Type | Required | Default | Description |\n");
			output.push_str("|---------|------|----------|---------|-------------|\n");

			for setting in &group.settings {
				let required = if setting.required { "✓" } else { "" };
				let default = setting.default.as_deref().unwrap_or("-");
				let desc = setting.description.as_deref().unwrap_or("");

				output.push_str(&format!(
					"| `{}` | `{}` | {} | `{}` | {} |\n",
					setting.key, setting.type_name, required, default, desc
				));
			}
			output.push('\n');

			// Detailed documentation for each setting
			for setting in &group.settings {
				self.generate_setting_markdown(output, setting, level + 1);
			}
		}

		// Subgroups
		for subgroup in &group.subgroups {
			self.generate_group_markdown(output, subgroup, level + 1);
		}
	}

	fn generate_setting_markdown(&self, output: &mut String, setting: &SettingDoc, level: usize) {
		output.push_str(&format!("{} `{}`\n\n", "#".repeat(level), setting.key));

		if let Some(desc) = &setting.description {
			output.push_str(&format!("{}\n\n", desc));
		}

		output.push_str(&format!("- **Type**: `{}`\n", setting.type_name));
		output.push_str(&format!(
			"- **Required**: {}\n",
			if setting.required { "Yes" } else { "No" }
		));

		if let Some(default) = &setting.default {
			output.push_str(&format!("- **Default**: `{}`\n", default));
		}

		if !setting.examples.is_empty() {
			output.push_str("- **Examples**:\n");
			for example in &setting.examples {
				output.push_str(&format!("  - `{}`\n", example));
			}
		}

		if !setting.constraints.is_empty() {
			output.push_str("- **Constraints**:\n");
			for constraint in &setting.constraints {
				output.push_str(&format!("  - {}\n", constraint));
			}
		}

		if let Some(deprecated) = &setting.deprecated {
			output.push_str(&format!("- **⚠️ Deprecated**: since {}\n", deprecated));
		}

		output.push('\n');
	}
	/// Generate JSON Schema
	///
	pub fn generate_json_schema(&self) -> Value {
		let mut properties = serde_json::Map::new();
		let mut required = Vec::new();

		for group in &self.groups {
			Self::add_group_to_schema(&mut properties, &mut required, group);
		}

		serde_json::json!({
			"$schema": "http://json-schema.org/draft-07/schema#",
			"type": "object",
			"properties": properties,
			"required": required
		})
	}

	fn add_group_to_schema(
		properties: &mut serde_json::Map<String, Value>,
		required: &mut Vec<String>,
		group: &SettingsGroup,
	) {
		for setting in &group.settings {
			let mut prop = serde_json::json!({
				"type": Self::json_type(&setting.type_name),
			});

			if let Some(desc) = &setting.description {
				prop["description"] = Value::String(desc.clone());
			}

			if let Some(default) = &setting.default {
				prop["default"] = Value::String(default.clone());
			}

			if !setting.examples.is_empty() {
				prop["examples"] = Value::Array(
					setting
						.examples
						.iter()
						.map(|e| Value::String(e.clone()))
						.collect(),
				);
			}

			properties.insert(setting.key.clone(), prop);

			if setting.required {
				required.push(setting.key.clone());
			}
		}

		for subgroup in &group.subgroups {
			Self::add_group_to_schema(properties, required, subgroup);
		}
	}

	fn json_type(rust_type: &str) -> &'static str {
		match rust_type {
			"String" | "str" | "&str" => "string",
			"i32" | "i64" | "u32" | "u64" | "isize" | "usize" => "integer",
			"f32" | "f64" => "number",
			"bool" => "boolean",
			t if t.starts_with("Vec<") || t.starts_with("&[") => "array",
			t if t.starts_with("HashMap<") || t.starts_with("BTreeMap<") => "object",
			_ => "string",
		}
	}

	fn anchor(text: &str) -> String {
		text.to_lowercase().replace(' ', "-")
	}
}

impl Default for DocsGenerator {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_setting_doc_builder() {
		let doc = SettingDoc::new("DEBUG", "bool")
			.with_description("Enable debug mode")
			.with_default("false")
			.add_example("true")
			.add_example("false")
			.add_constraint("Must be a boolean value");

		assert_eq!(doc.key, "DEBUG");
		assert_eq!(doc.type_name, "bool");
		assert_eq!(doc.description, Some("Enable debug mode".to_string()));
		assert_eq!(doc.default, Some("false".to_string()));
		assert_eq!(doc.examples.len(), 2);
		assert_eq!(doc.constraints.len(), 1);
	}

	#[test]
	fn test_settings_group() {
		let group = SettingsGroup::new("Database")
			.with_description("Database configuration settings")
			.add_setting(
				SettingDoc::new("DB_HOST", "String")
					.with_description("Database host")
					.with_default("localhost")
					.required(),
			)
			.add_setting(
				SettingDoc::new("DB_PORT", "u16")
					.with_description("Database port")
					.with_default("5432"),
			);

		assert_eq!(group.name, "Database");
		assert_eq!(group.settings.len(), 2);
		assert!(group.settings[0].required);
		assert!(!group.settings[1].required);
	}

	#[test]
	fn test_markdown_generation() {
		let mut generator = DocsGenerator::new();

		let group = SettingsGroup::new("Security")
			.with_description("Security-related settings")
			.add_setting(
				SettingDoc::new("SECRET_KEY", "String")
					.with_description("Secret key for cryptographic operations")
					.required()
					.add_example("my-secret-key-here")
					.add_constraint("Must be at least 32 characters"),
			);

		generator.add_group(group);

		let markdown = generator.generate_markdown();

		assert!(markdown.contains("# Settings Documentation"));
		assert!(markdown.contains("## Security"));
		assert!(markdown.contains("SECRET_KEY"));
		assert!(markdown.contains("String"));
		assert!(markdown.contains("✓")); // Required marker
	}

	#[test]
	fn test_json_schema_generation() {
		let mut generator = DocsGenerator::new();

		let group = SettingsGroup::new("App")
			.add_setting(
				SettingDoc::new("APP_NAME", "String")
					.with_description("Application name")
					.with_default("MyApp")
					.required(),
			)
			.add_setting(
				SettingDoc::new("DEBUG", "bool")
					.with_description("Debug mode")
					.with_default("false"),
			);

		generator.add_group(group);

		let schema = generator.generate_json_schema();

		assert_eq!(schema["$schema"], "http://json-schema.org/draft-07/schema#");
		assert!(schema["properties"]["APP_NAME"].is_object());
		assert_eq!(schema["properties"]["APP_NAME"]["type"], "string");
		assert_eq!(schema["properties"]["DEBUG"]["type"], "boolean");
		assert!(
			schema["required"]
				.as_array()
				.unwrap()
				.contains(&Value::String("APP_NAME".to_string()))
		);
	}

	#[test]
	fn test_deprecated_setting() {
		let doc = SettingDoc::new("OLD_SETTING", "String").deprecated_since("v2.0");

		assert_eq!(doc.deprecated, Some("v2.0".to_string()));
	}

	#[test]
	fn test_nested_groups() {
		let subgroup = SettingsGroup::new("Cache").add_setting(
			SettingDoc::new("CACHE_TTL", "i64")
				.with_description("Cache time-to-live in seconds")
				.with_default("3600"),
		);

		let group = SettingsGroup::new("Performance").add_subgroup(subgroup);

		assert_eq!(group.subgroups.len(), 1);
		assert_eq!(group.subgroups[0].name, "Cache");
	}
}

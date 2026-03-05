//! Configuration for database introspection and code generation.
//!
//! Supports TOML configuration files and CLI argument overrides.

use super::naming::NamingConvention;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Main configuration for database introspection.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct IntrospectConfig {
	/// Database connection configuration
	pub database: DatabaseConfig,

	/// Output configuration
	pub output: OutputConfig,

	/// Code generation configuration
	pub generation: GenerationConfig,

	/// Table filtering configuration
	pub tables: TableFilterConfig,

	/// Type overrides: "table.column" -> "RustType"
	#[serde(default)]
	pub type_overrides: HashMap<String, String>,

	/// Additional imports configuration
	#[serde(default)]
	pub imports: ImportsConfig,
}

impl IntrospectConfig {
	/// Create a new configuration with the given database URL.
	pub fn with_database_url(mut self, url: &str) -> Self {
		self.database.url = url.to_string();
		self
	}

	/// Create a new configuration with the given output directory.
	pub fn with_output_dir(mut self, dir: impl Into<PathBuf>) -> Self {
		self.output.directory = dir.into();
		self
	}

	/// Create a new configuration with the given app label.
	pub fn with_app_label(mut self, label: &str) -> Self {
		self.generation.app_label = label.to_string();
		self
	}

	/// Load configuration from a TOML file.
	///
	/// # Errors
	///
	/// Returns error if file cannot be read or parsed.
	pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
		let content = std::fs::read_to_string(path.as_ref()).map_err(|e| ConfigError::IoError {
			path: path.as_ref().to_path_buf(),
			source: e,
		})?;

		Self::from_toml(&content)
	}

	/// Parse configuration from TOML string.
	pub fn from_toml(content: &str) -> Result<Self, ConfigError> {
		toml::from_str(content).map_err(|e| ConfigError::ParseError {
			message: e.to_string(),
		})
	}

	/// Check if a table should be included based on filter configuration.
	pub fn should_include_table(&self, table_name: &str) -> bool {
		// Check exclude patterns first
		for pattern in &self.tables.exclude {
			if let Ok(re) = Regex::new(pattern)
				&& re.is_match(table_name)
			{
				return false;
			}
		}

		// Check include patterns
		if self.tables.include.is_empty() {
			return true;
		}

		for pattern in &self.tables.include {
			if let Ok(re) = Regex::new(pattern)
				&& re.is_match(table_name)
			{
				return true;
			}
		}

		false
	}

	/// Get type override for a specific table.column.
	pub fn get_type_override(&self, table: &str, column: &str) -> Option<&str> {
		let key = format!("{}.{}", table, column);
		self.type_overrides.get(&key).map(|s| s.as_str())
	}

	/// Merge CLI arguments into configuration.
	///
	/// CLI arguments take precedence over config file values.
	pub fn merge_cli_args(&mut self, args: &CliArgs) {
		if let Some(ref url) = args.database_url {
			self.database.url = url.clone();
		}

		if let Some(ref dir) = args.output_dir {
			self.output.directory = dir.clone();
		}

		if let Some(ref label) = args.app_label {
			self.generation.app_label = label.clone();
		}

		if let Some(ref pattern) = args.include_tables {
			self.tables.include = vec![pattern.clone()];
		}

		if let Some(ref pattern) = args.exclude_tables {
			self.tables.exclude.push(pattern.clone());
		}
	}
}

/// Database connection configuration.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct DatabaseConfig {
	/// Database connection URL
	///
	/// Can include environment variable reference: "${DATABASE_URL}"
	pub url: String,
}

impl DatabaseConfig {
	/// Resolve the database URL, expanding environment variables.
	pub fn resolve_url(&self) -> Result<String, ConfigError> {
		if self.url.starts_with("${") && self.url.ends_with('}') {
			let var_name = &self.url[2..self.url.len() - 1];
			std::env::var(var_name).map_err(|_| ConfigError::EnvVarNotFound {
				name: var_name.to_string(),
			})
		} else if self.url.is_empty() {
			// Try DATABASE_URL environment variable as fallback
			std::env::var("DATABASE_URL").map_err(|_| ConfigError::MissingDatabaseUrl)
		} else {
			Ok(self.url.clone())
		}
	}
}

/// Output configuration.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OutputConfig {
	/// Output directory for generated files
	pub directory: PathBuf,

	/// Generate all models in a single file
	#[serde(default)]
	pub single_file: bool,

	/// File name when single_file is true
	#[serde(default = "default_single_file_name")]
	pub single_file_name: String,
}

fn default_single_file_name() -> String {
	"models.rs".to_string()
}

impl Default for OutputConfig {
	fn default() -> Self {
		Self {
			directory: PathBuf::from("src/models/generated"),
			single_file: false,
			single_file_name: default_single_file_name(),
		}
	}
}

/// Code generation configuration.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GenerationConfig {
	/// App label for generated models
	pub app_label: String,

	/// Detect relationships from foreign keys
	#[serde(default = "default_true")]
	pub detect_relationships: bool,

	/// Derive macros to add to generated structs
	#[serde(default = "default_derives")]
	pub derives: Vec<String>,

	/// Include column comments as doc comments
	#[serde(default = "default_true")]
	pub include_column_comments: bool,

	/// Naming convention for struct names
	#[serde(default)]
	pub struct_naming: NamingConventionConfig,

	/// Naming convention for field names
	#[serde(default)]
	pub field_naming: NamingConventionConfig,
}

fn default_true() -> bool {
	true
}

fn default_derives() -> Vec<String> {
	vec![
		"Debug".to_string(),
		"Clone".to_string(),
		"Serialize".to_string(),
		"Deserialize".to_string(),
	]
}

impl Default for GenerationConfig {
	fn default() -> Self {
		Self {
			app_label: "app".to_string(),
			detect_relationships: true,
			derives: default_derives(),
			include_column_comments: true,
			struct_naming: NamingConventionConfig::default(),
			// Rust struct fields should use snake_case by convention
			field_naming: NamingConventionConfig::SnakeCase,
		}
	}
}

impl GenerationConfig {
	/// Get the naming convention for struct names.
	pub fn struct_naming_convention(&self) -> NamingConvention {
		self.struct_naming.to_convention()
	}

	/// Get the naming convention for field names.
	pub fn field_naming_convention(&self) -> NamingConvention {
		self.field_naming.to_convention()
	}
}

/// Naming convention configuration (for serde).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NamingConventionConfig {
	#[default]
	PascalCase,
	SnakeCase,
	Preserve,
}

impl NamingConventionConfig {
	/// Convert to NamingConvention enum.
	pub fn to_convention(&self) -> NamingConvention {
		match self {
			NamingConventionConfig::PascalCase => NamingConvention::PascalCase,
			NamingConventionConfig::SnakeCase => NamingConvention::SnakeCase,
			NamingConventionConfig::Preserve => NamingConvention::Preserve,
		}
	}
}

/// Table filtering configuration.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TableFilterConfig {
	/// Include tables matching these patterns (regex)
	pub include: Vec<String>,

	/// Exclude tables matching these patterns (regex)
	pub exclude: Vec<String>,
}

impl Default for TableFilterConfig {
	fn default() -> Self {
		Self {
			include: vec![".*".to_string()],
			exclude: vec![
				"^pg_".to_string(),
				"^reinhardt_migrations".to_string(),
				"^django_".to_string(),
				"^auth_".to_string(),
			],
		}
	}
}

/// Additional imports configuration.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ImportsConfig {
	/// Additional use statements to include
	pub additional: Vec<String>,
}

/// CLI arguments that can override config file values.
#[non_exhaustive]
#[derive(Debug, Clone, Default)]
pub struct CliArgs {
	pub database_url: Option<String>,
	pub output_dir: Option<PathBuf>,
	pub app_label: Option<String>,
	pub include_tables: Option<String>,
	pub exclude_tables: Option<String>,
	pub config_file: Option<PathBuf>,
	pub dry_run: bool,
	pub force: bool,
	pub verbose: bool,
}

/// Configuration errors.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
	#[error("IO error reading {path}: {source}")]
	IoError {
		path: PathBuf,
		#[source]
		source: std::io::Error,
	},

	#[error("Failed to parse configuration: {message}")]
	ParseError { message: String },

	#[error("Environment variable not found: {name}")]
	EnvVarNotFound { name: String },

	#[error(
		"Database URL not specified. Set DATABASE_URL environment variable or use --database option"
	)]
	MissingDatabaseUrl,

	#[error("Invalid regex pattern: {pattern}")]
	InvalidPattern { pattern: String },
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_default_config() {
		let config = IntrospectConfig::default();

		assert_eq!(
			config.output.directory,
			PathBuf::from("src/models/generated")
		);
		assert!(config.generation.detect_relationships);
		assert!(!config.output.single_file);
	}

	#[test]
	fn test_parse_toml_config() {
		let toml = r#"
[database]
url = "postgres://localhost/test"

[output]
directory = "src/generated"

[generation]
app_label = "myapp"
detect_relationships = true

[tables]
include = ["users", "posts"]
exclude = ["^pg_"]

[type_overrides]
"users.status" = "UserStatus"
"#;

		let config = IntrospectConfig::from_toml(toml).unwrap();

		assert_eq!(config.database.url, "postgres://localhost/test");
		assert_eq!(config.output.directory, PathBuf::from("src/generated"));
		assert_eq!(config.generation.app_label, "myapp");
		assert_eq!(config.tables.include, vec!["users", "posts"]);
		assert_eq!(
			config.type_overrides.get("users.status"),
			Some(&"UserStatus".to_string())
		);
	}

	#[test]
	fn test_table_filtering() {
		let config = IntrospectConfig {
			tables: TableFilterConfig {
				include: vec!["users".to_string(), "posts".to_string()],
				exclude: vec!["^pg_".to_string()],
			},
			..Default::default()
		};

		assert!(config.should_include_table("users"));
		assert!(config.should_include_table("posts"));
		assert!(!config.should_include_table("comments"));
		assert!(!config.should_include_table("pg_tables"));
	}

	#[test]
	fn test_cli_args_merge() {
		let mut config = IntrospectConfig::default();
		let args = CliArgs {
			database_url: Some("postgres://cli/db".to_string()),
			app_label: Some("cli_app".to_string()),
			..Default::default()
		};

		config.merge_cli_args(&args);

		assert_eq!(config.database.url, "postgres://cli/db");
		assert_eq!(config.generation.app_label, "cli_app");
	}

	#[test]
	fn test_builder_pattern() {
		let config = IntrospectConfig::default()
			.with_database_url("postgres://localhost/test")
			.with_output_dir("./output")
			.with_app_label("test_app");

		assert_eq!(config.database.url, "postgres://localhost/test");
		assert_eq!(config.output.directory, PathBuf::from("./output"));
		assert_eq!(config.generation.app_label, "test_app");
	}
}

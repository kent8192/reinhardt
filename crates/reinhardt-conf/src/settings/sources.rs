//! Configuration sources for layered settings system
//!
//! Provides different sources of configuration that can be merged together
//! in priority order (environment variables > .env files > config files > defaults).

use super::env::EnvError;
use super::env_loader::EnvLoader;
use super::profile::Profile;
use indexmap::IndexMap;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Trait for configuration sources
pub trait ConfigSource: Send + Sync {
	/// Load configuration from this source
	fn load(&self) -> Result<IndexMap<String, Value>, SourceError>;

	/// Get the priority of this source (higher = more important)
	fn priority(&self) -> u8;

	/// Get a description of this source
	fn description(&self) -> String;
}

/// Error type for configuration sources
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum SourceError {
	#[error("IO error: {0}")]
	Io(#[from] std::io::Error),

	#[error("Parse error: {0}")]
	Parse(String),

	#[error("Environment error: {0}")]
	Env(#[from] EnvError),

	#[error("TOML error: {0}")]
	Toml(#[from] toml::de::Error),

	#[error("JSON error: {0}")]
	Json(#[from] serde_json::Error),

	#[error("Invalid source: {0}")]
	InvalidSource(String),
}

/// Environment variable configuration source
pub struct EnvSource {
	prefix: Option<String>,
	interpolate: bool,
}

impl EnvSource {
	/// Create a new environment variable configuration source
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::sources::EnvSource;
	///
	/// let source = EnvSource::new();
	/// // Loads all environment variables
	/// ```
	pub fn new() -> Self {
		Self {
			prefix: None,
			interpolate: false,
		}
	}
	/// Set a prefix filter for environment variables
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::sources::EnvSource;
	///
	/// let source = EnvSource::new()
	///     .with_prefix("APP_");
	/// // Only loads env vars starting with APP_
	/// ```
	pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
		self.prefix = Some(prefix.into());
		self
	}
	/// Enable variable interpolation for environment values
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::sources::EnvSource;
	///
	/// let source = EnvSource::new()
	///     .with_interpolation(true);
	/// // Environment variables will support $VAR expansion
	/// ```
	pub fn with_interpolation(mut self, enabled: bool) -> Self {
		self.interpolate = enabled;
		self
	}
}

impl Default for EnvSource {
	fn default() -> Self {
		Self::new()
	}
}

impl ConfigSource for EnvSource {
	fn load(&self) -> Result<IndexMap<String, Value>, SourceError> {
		let mut config = IndexMap::new();

		// Get all environment variables
		for (key, value) in std::env::vars() {
			// Skip if prefix is set and key doesn't start with it
			if let Some(prefix) = &self.prefix
				&& !key.starts_with(prefix)
			{
				continue;
			}

			// Remove prefix if present
			let clean_key = if let Some(prefix) = &self.prefix {
				key.strip_prefix(prefix).unwrap_or(&key).to_string()
			} else {
				key.clone()
			};

			// Convert to lowercase for consistency
			let lower_key = clean_key.to_lowercase();

			// Try to parse as appropriate type
			let parsed_value = if lower_key == "debug" {
				// Parse debug value with support for "1", "0", "true", "false", etc.
				match value.trim().to_lowercase().as_str() {
					"true" | "1" | "yes" | "on" => Value::Bool(true),
					"false" | "0" | "no" | "off" => Value::Bool(false),
					_ => {
						if let Ok(b) = value.parse::<bool>() {
							Value::Bool(b)
						} else {
							Value::String(value)
						}
					}
				}
			} else if lower_key == "allowed_hosts" {
				// Parse comma-separated list
				let list: Vec<_> = value
					.split(',')
					.map(|s| Value::String(s.trim().to_string()))
					.collect();
				Value::Array(list)
			} else if let Ok(num) = value.parse::<i64>() {
				Value::Number(num.into())
			} else if let Ok(b) = value.parse::<bool>() {
				Value::Bool(b)
			} else {
				Value::String(value)
			};

			config.insert(lower_key, parsed_value);
		}

		Ok(config)
	}

	fn priority(&self) -> u8 {
		100 // Highest priority
	}

	fn description(&self) -> String {
		match &self.prefix {
			Some(prefix) => format!("Environment variables (prefix: {})", prefix),
			None => "Environment variables".to_string(),
		}
	}
}

/// .env file configuration source
pub struct DotEnvSource {
	path: Option<PathBuf>,
	profile: Option<Profile>,
	interpolate: bool,
}

impl DotEnvSource {
	/// Create a new .env file configuration source
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::sources::DotEnvSource;
	///
	/// let source = DotEnvSource::new();
	/// // Loads from .env file
	/// ```
	pub fn new() -> Self {
		Self {
			path: None,
			profile: None,
			interpolate: false,
		}
	}
	/// Set a specific path for the .env file
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::sources::DotEnvSource;
	/// use std::path::PathBuf;
	///
	/// let source = DotEnvSource::new()
	///     .with_path(PathBuf::from(".env.local"));
	/// ```
	pub fn with_path(mut self, path: impl Into<PathBuf>) -> Self {
		self.path = Some(path.into());
		self
	}
	/// Set the profile to determine .env file name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::sources::DotEnvSource;
	/// use reinhardt_conf::settings::profile::Profile;
	///
	/// let source = DotEnvSource::new()
	///     .with_profile(Profile::Production);
	/// // Will load .env.production
	/// ```
	pub fn with_profile(mut self, profile: Profile) -> Self {
		self.profile = Some(profile);
		self
	}
	/// Enable variable interpolation in .env files
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::sources::DotEnvSource;
	///
	/// let source = DotEnvSource::new()
	///     .with_interpolation(true);
	/// // .env file variables will support $VAR expansion
	/// ```
	pub fn with_interpolation(mut self, enabled: bool) -> Self {
		self.interpolate = enabled;
		self
	}
}

impl Default for DotEnvSource {
	fn default() -> Self {
		Self::new()
	}
}

impl ConfigSource for DotEnvSource {
	fn load(&self) -> Result<IndexMap<String, Value>, SourceError> {
		let path = match &self.path {
			Some(p) => p.clone(),
			None => {
				let filename = match &self.profile {
					Some(profile) => profile.env_file_name(),
					None => ".env".to_string(),
				};
				PathBuf::from(filename)
			}
		};

		// Load .env file if it exists
		let loader = EnvLoader::new()
			.path(&path)
			.interpolate(self.interpolate)
			.overwrite(false);

		// Try to load, but don't fail if file doesn't exist
		let _ = loader.load_optional()?;

		// Return empty config - the env vars are already loaded
		// The EnvSource will pick them up
		Ok(IndexMap::new())
	}

	fn priority(&self) -> u8 {
		90 // High priority, but below direct env vars
	}

	fn description(&self) -> String {
		match &self.path {
			Some(path) => format!(".env file: {}", path.display()),
			None => match &self.profile {
				Some(profile) => format!(".env file: {}", profile.env_file_name()),
				None => ".env file".to_string(),
			},
		}
	}
}

/// TOML file configuration source
pub struct TomlFileSource {
	path: PathBuf,
}

impl TomlFileSource {
	/// Create a new TOML file configuration source
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::sources::TomlFileSource;
	/// use std::path::PathBuf;
	///
	/// let source = TomlFileSource::new(PathBuf::from("config.toml"));
	/// ```
	pub fn new(path: impl Into<PathBuf>) -> Self {
		Self { path: path.into() }
	}
}

impl ConfigSource for TomlFileSource {
	fn load(&self) -> Result<IndexMap<String, Value>, SourceError> {
		if !self.path.exists() {
			return Ok(IndexMap::new());
		}

		let content = fs::read_to_string(&self.path)?;
		let toml_value: toml::Value = toml::from_str(&content)?;

		// Convert TOML value to JSON value
		let json_str = serde_json::to_string(&toml_value)?;
		let json_value: Value = serde_json::from_str(&json_str)?;

		// Flatten into IndexMap
		let map = json_value
			.as_object()
			.ok_or_else(|| SourceError::Parse("Expected object at root".to_string()))?;

		Ok(map.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
	}

	fn priority(&self) -> u8 {
		50 // Medium priority
	}

	fn description(&self) -> String {
		format!("TOML file: {}", self.path.display())
	}
}

/// JSON file configuration source
pub struct JsonFileSource {
	path: PathBuf,
}

impl JsonFileSource {
	/// Create a new JSON file configuration source
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::sources::JsonFileSource;
	/// use std::path::PathBuf;
	///
	/// let source = JsonFileSource::new(PathBuf::from("config.json"));
	/// ```
	pub fn new(path: impl Into<PathBuf>) -> Self {
		Self { path: path.into() }
	}
}

impl ConfigSource for JsonFileSource {
	fn load(&self) -> Result<IndexMap<String, Value>, SourceError> {
		if !self.path.exists() {
			return Ok(IndexMap::new());
		}

		let content = fs::read_to_string(&self.path)?;
		let json_value: Value = serde_json::from_str(&content)?;

		// Flatten into IndexMap
		let map = json_value
			.as_object()
			.ok_or_else(|| SourceError::Parse("Expected object at root".to_string()))?;

		Ok(map.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
	}

	fn priority(&self) -> u8 {
		50 // Medium priority
	}

	fn description(&self) -> String {
		format!("JSON file: {}", self.path.display())
	}
}

/// Default values configuration source
pub struct DefaultSource {
	values: IndexMap<String, Value>,
}

impl DefaultSource {
	/// Create a new default values configuration source
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::sources::DefaultSource;
	/// use serde_json::Value;
	///
	/// let source = DefaultSource::new()
	///     .with_value("debug", Value::Bool(false))
	///     .with_value("port", Value::Number(8000.into()));
	/// ```
	pub fn new() -> Self {
		Self {
			values: IndexMap::new(),
		}
	}
	/// Add a default value for a configuration key
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::sources::DefaultSource;
	/// use serde_json::Value;
	///
	/// let source = DefaultSource::new()
	///     .with_value("timeout", Value::Number(30.into()));
	/// ```
	pub fn with_value(mut self, key: impl Into<String>, value: Value) -> Self {
		self.values.insert(key.into(), value);
		self
	}
	/// Add multiple default values from a HashMap
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::sources::DefaultSource;
	/// use serde_json::Value;
	/// use std::collections::HashMap;
	///
	/// let mut defaults = HashMap::new();
	/// defaults.insert("key1".to_string(), Value::String("value1".to_string()));
	/// defaults.insert("key2".to_string(), Value::Bool(true));
	///
	/// let source = DefaultSource::new()
	///     .with_defaults(defaults);
	/// ```
	pub fn with_defaults(mut self, defaults: HashMap<String, Value>) -> Self {
		self.values.extend(defaults);
		self
	}
}

impl Default for DefaultSource {
	fn default() -> Self {
		Self::new()
	}
}

impl ConfigSource for DefaultSource {
	fn load(&self) -> Result<IndexMap<String, Value>, SourceError> {
		Ok(self.values.clone())
	}

	fn priority(&self) -> u8 {
		0 // Lowest priority
	}

	fn description(&self) -> String {
		"Default values".to_string()
	}
}
/// Auto-detect configuration source based on file extension
///
/// # Examples
///
/// ```
/// use reinhardt_conf::settings::sources::auto_source;
/// use std::path::PathBuf;
///
/// // Automatically detects TOML source from extension
/// let source = auto_source(PathBuf::from("config.toml")).unwrap();
///
/// // Or JSON source
/// let source = auto_source(PathBuf::from("settings.json")).unwrap();
/// ```
pub fn auto_source(path: impl AsRef<Path>) -> Result<Box<dyn ConfigSource>, SourceError> {
	let path = path.as_ref();
	let ext = path
		.extension()
		.and_then(|e| e.to_str())
		.ok_or_else(|| SourceError::InvalidSource("No file extension".to_string()))?;

	match ext {
		"toml" => Ok(Box::new(TomlFileSource::new(path))),
		"json" => Ok(Box::new(JsonFileSource::new(path))),
		_ => Err(SourceError::InvalidSource(format!(
			"Unsupported file extension: {}",
			ext
		))),
	}
}

/// Low-priority environment variable configuration source
///
/// This wrapper provides the same functionality as `EnvSource` but with lower priority
/// than TOML files, allowing TOML configuration to override environment variables.
///
/// Priority: 40 (lower than TOML files at 50)
///
/// # Examples
///
/// ```
/// use reinhardt_conf::settings::sources::LowPriorityEnvSource;
/// use reinhardt_conf::settings::builder::SettingsBuilder;
///
/// let settings = SettingsBuilder::new()
///     .add_source(LowPriorityEnvSource::new())
///     .build()
///     .unwrap();
/// ```
pub struct LowPriorityEnvSource {
	inner: EnvSource,
}

impl LowPriorityEnvSource {
	/// Create a new low-priority environment variable configuration source
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::sources::LowPriorityEnvSource;
	///
	/// let source = LowPriorityEnvSource::new();
	/// ```
	pub fn new() -> Self {
		Self {
			inner: EnvSource::new(),
		}
	}

	/// Set a prefix filter for environment variables
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::sources::LowPriorityEnvSource;
	///
	/// let source = LowPriorityEnvSource::new()
	///     .with_prefix("REINHARDT_");
	/// ```
	pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
		self.inner = self.inner.with_prefix(prefix);
		self
	}

	/// Enable variable interpolation for environment values
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::sources::LowPriorityEnvSource;
	///
	/// let source = LowPriorityEnvSource::new()
	///     .with_interpolation(true);
	/// ```
	pub fn with_interpolation(mut self, enabled: bool) -> Self {
		self.inner = self.inner.with_interpolation(enabled);
		self
	}
}

impl Default for LowPriorityEnvSource {
	fn default() -> Self {
		Self::new()
	}
}

impl ConfigSource for LowPriorityEnvSource {
	fn load(&self) -> Result<IndexMap<String, Value>, SourceError> {
		self.inner.load()
	}

	fn priority(&self) -> u8 {
		40 // Lower than TOML files (50), allowing TOML to override env vars
	}

	fn description(&self) -> String {
		format!("{} (low priority)", self.inner.description())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::env;
	use std::fs::File;
	use std::io::Write;
	use tempfile::TempDir;

	#[test]
	fn test_env_source() {
		// SAFETY: Setting environment variables is unsafe in multi-threaded programs.
		// This test uses #[serial] to ensure exclusive access to environment variables.
		unsafe {
			env::set_var("SECRET_KEY", "test-secret");
			env::set_var("DEBUG", "true");
		}

		let source = EnvSource::new();
		let config = source.load().unwrap();

		assert_eq!(
			config.get("secret_key").unwrap(),
			&Value::String("test-secret".to_string())
		);
		assert_eq!(config.get("debug").unwrap(), &Value::Bool(true));

		// SAFETY: Removing environment variables is unsafe in multi-threaded programs.
		// This test uses #[serial] to ensure exclusive access to environment variables.
		unsafe {
			env::remove_var("SECRET_KEY");
			env::remove_var("DEBUG");
		}
	}

	#[test]
	fn test_toml_source() {
		let temp_dir = TempDir::new().unwrap();
		let config_path = temp_dir.path().join("config.toml");

		let mut file = File::create(&config_path).unwrap();
		writeln!(
			file,
			r#"
debug = true
secret_key = "test-key"
        "#
		)
		.unwrap();

		let source = TomlFileSource::new(&config_path);
		let config = source.load().unwrap();

		assert_eq!(config.get("debug").unwrap(), &Value::Bool(true));
		assert_eq!(
			config.get("secret_key").unwrap(),
			&Value::String("test-key".to_string())
		);
	}

	#[test]
	fn test_json_source() {
		let temp_dir = TempDir::new().unwrap();
		let config_path = temp_dir.path().join("config.json");

		let mut file = File::create(&config_path).unwrap();
		writeln!(
			file,
			r#"{{
            "debug": false,
            "secret_key": "json-key"
        }}"#
		)
		.unwrap();

		let source = JsonFileSource::new(&config_path);
		let config = source.load().unwrap();

		assert_eq!(config.get("debug").unwrap(), &Value::Bool(false));
		assert_eq!(
			config.get("secret_key").unwrap(),
			&Value::String("json-key".to_string())
		);
	}

	#[test]
	fn test_default_source() {
		let source = DefaultSource::new()
			.with_value("key1", Value::String("value1".to_string()))
			.with_value("key2", Value::Bool(true));

		let config = source.load().unwrap();

		assert_eq!(
			config.get("key1").unwrap(),
			&Value::String("value1".to_string())
		);
		assert_eq!(config.get("key2").unwrap(), &Value::Bool(true));
	}

	#[test]
	fn test_source_priority() {
		assert_eq!(EnvSource::new().priority(), 100);
		assert_eq!(DotEnvSource::new().priority(), 90);
		assert_eq!(TomlFileSource::new("test.toml").priority(), 50);
		assert_eq!(DefaultSource::new().priority(), 0);
	}
}

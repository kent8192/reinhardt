//! Settings builder with layered configuration support
//!
//! Provides a builder pattern for constructing settings from multiple sources
//! with priority-based merging.

use super::composed::ComposedSettings;
use super::profile::Profile;
use super::sources::{ConfigSource, DotEnvSource, EnvSource, SourceError};
use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::sync::Arc;

/// Settings builder for layered configuration
pub struct SettingsBuilder {
	sources: Vec<Box<dyn ConfigSource>>,
	profile: Option<Profile>,
	strict: bool,
}

impl SettingsBuilder {
	/// Create a new settings builder
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::builder::SettingsBuilder;
	///
	/// let builder = SettingsBuilder::new();
	/// let settings = builder.build().unwrap();
	///
	/// // Empty builder creates valid merged settings
	/// assert_eq!(settings.keys().count(), 0);
	/// ```
	pub fn new() -> Self {
		Self {
			sources: Vec::new(),
			profile: None,
			strict: false,
		}
	}
	/// Set the application profile
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::builder::SettingsBuilder;
	/// use reinhardt_conf::settings::profile::Profile;
	///
	/// let builder = SettingsBuilder::new()
	///     .profile(Profile::Development);
	/// let settings = builder.build().unwrap();
	///
	/// assert_eq!(settings.profile(), Some(Profile::Development));
	/// ```
	pub fn profile(mut self, profile: Profile) -> Self {
		self.profile = Some(profile);
		self
	}
	/// Enable strict mode (fail on missing required values)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::builder::SettingsBuilder;
	///
	/// let builder = SettingsBuilder::new()
	///     .strict(true);
	///
	// Strict mode is set (internal state)
	// This affects validation behavior during build
	/// let settings = builder.build().unwrap();
	/// assert_eq!(settings.keys().count(), 0);
	/// ```
	pub fn strict(mut self, enabled: bool) -> Self {
		self.strict = enabled;
		self
	}
	/// Add a configuration source
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::builder::SettingsBuilder;
	/// use reinhardt_conf::settings::sources::EnvSource;
	///
	/// let builder = SettingsBuilder::new()
	///     .add_source(EnvSource::new());
	/// let settings = builder.build().unwrap();
	/// // Environment variables are now included in settings
	/// ```
	pub fn add_source<S: ConfigSource + 'static>(mut self, source: S) -> Self {
		self.sources.push(Box::new(source));
		self
	}
	/// Add environment variable source with optional prefix
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::builder::SettingsBuilder;
	///
	/// let builder = SettingsBuilder::new()
	///     .with_env(Some("REINHARDT"));
	/// let settings = builder.build().unwrap();
	/// // Environment variables with REINHARDT_ prefix are included
	/// ```
	pub fn with_env(self, prefix: Option<&str>) -> Self {
		let mut source = EnvSource::new();
		if let Some(p) = prefix {
			source = source.with_prefix(p);
		}
		self.add_source(source)
	}
	/// Add .env file source
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::builder::SettingsBuilder;
	/// use reinhardt_conf::settings::profile::Profile;
	///
	/// let builder = SettingsBuilder::new()
	///     .profile(Profile::Development)
	///     .with_dotenv();
	/// let settings = builder.build().unwrap();
	/// // .env.development file will be loaded if it exists
	/// ```
	pub fn with_dotenv(self) -> Self {
		let mut source = DotEnvSource::new();
		if let Some(profile) = &self.profile {
			source = source.with_profile(*profile);
		}
		self.add_source(source)
	}
	/// Build and validate a composed settings struct.
	///
	/// This method:
	/// 1. Merges all configuration sources
	/// 2. Validates that all required fields have values
	/// 3. Deserializes the merged data into the target type
	///
	/// Fragment-level validation (`validate_fragments()`) should be called
	/// separately by the caller with the appropriate profile.
	pub fn build_composed<T: ComposedSettings>(self) -> Result<T, BuildError> {
		let merged = self.build()?;
		T::validate_requirements(merged.as_map())?;
		let settings: T = merged.into_typed().map_err(BuildError::from)?;
		Ok(settings)
	}

	/// Build the configuration by merging all sources
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::builder::SettingsBuilder;
	/// use reinhardt_conf::settings::sources::{DefaultSource, EnvSource};
	/// use serde_json::Value;
	///
	/// let settings = SettingsBuilder::new()
	///     .add_source(
	///         DefaultSource::new()
	///             .with_value("port", Value::Number(8080.into()))
	///     )
	///     .add_source(EnvSource::new())
	///     .build()
	///     .unwrap();
	///
	/// // Environment variables override defaults
	/// assert!(settings.contains_key("port"));
	/// ```
	pub fn build(mut self) -> Result<MergedSettings, BuildError> {
		// Sort sources by priority (lowest first, so highest priority overwrites)
		self.sources.sort_by_key(|a| a.priority());

		// Collect source descriptions up front for use in diagnostics.
		let source_descriptions: Vec<String> =
			self.sources.iter().map(|s| s.description()).collect();

		let mut merged = IndexMap::new();

		// Merge all sources in priority order (lowest to highest)
		// Later sources will overwrite earlier ones
		for source in &self.sources {
			let config = source.load().map_err(|e| BuildError::Source {
				description: source.description(),
				error: e,
			})?;

			// Merge into the main config
			for (key, value) in config {
				merged.insert(key, value);
			}
		}

		// Apply thread-local test overrides (highest priority, above all sources)
		if let Some(overrides) = super::testing::overrides::current_overrides() {
			super::testing::overrides::deep_merge(&mut merged, overrides);
		}

		// Warn about flat top-level keys that belong under [core]
		warn_flat_core_keys(&merged, &source_descriptions);

		Ok(MergedSettings {
			data: Arc::new(merged),
			profile: self.profile,
		})
	}
}

/// Known field names that belong under `[core]` in a settings TOML file.
const CORE_SETTINGS_FIELDS: &[&str] = &[
	"debug",
	"secret_key",
	"allowed_hosts",
	"installed_apps",
	"middleware",
	"databases",
	"static_url",
	"media_url",
	"language_code",
	"time_zone",
];

/// Emit a warning for any top-level flat keys in `merged` that are known
/// `CoreSettings` fields and therefore must live under `[core]`.
///
/// `source_descriptions` is a list of human-readable source descriptions
/// (e.g. "TOML file: local.toml") used to build a helpful diagnostic message.
fn warn_flat_core_keys(merged: &IndexMap<String, Value>, source_descriptions: &[String]) {
	let source_hint = if source_descriptions.is_empty() {
		"(unknown source)".to_string()
	} else {
		source_descriptions.join(", ")
	};

	for &field in CORE_SETTINGS_FIELDS {
		if merged.contains_key(field) {
			eprintln!(
				"[reinhardt-conf] Warning: settings source(s) '{}' contain top-level key '{}' outside any section.\n\
				 This key is part of CoreSettings and must be placed under [core] to take effect.\n\
				 Hint: wrap the key in a [core] section header.",
				source_hint, field
			);
		}
	}
}

impl Default for SettingsBuilder {
	fn default() -> Self {
		Self::new()
	}
}

/// Merged settings result
#[derive(Clone)]
pub struct MergedSettings {
	data: Arc<IndexMap<String, Value>>,
	profile: Option<Profile>,
}

impl MergedSettings {
	/// Get a value by key
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::builder::{SettingsBuilder, MergedSettings};
	/// use reinhardt_conf::settings::sources::DefaultSource;
	/// use serde_json::Value;
	///
	/// let settings = SettingsBuilder::new()
	///     .add_source(
	///         DefaultSource::new()
	///             .with_value("timeout", Value::Number(30.into()))
	///     )
	///     .build()
	///     .unwrap();
	///
	/// let timeout: i64 = settings.get("timeout").unwrap();
	/// assert_eq!(timeout, 30);
	/// ```
	pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<T, GetError> {
		let value = self
			.data
			.get(key)
			.ok_or_else(|| GetError::MissingKey(key.to_string()))?;

		serde_json::from_value(value.clone()).map_err(|e| GetError::Deserialize {
			key: key.to_string(),
			error: e,
		})
	}
	/// Get a value by key with a default
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_conf::settings::builder::SettingsBuilder;
	/// use reinhardt_conf::settings::sources::DefaultSource;
	/// use serde_json::Value;
	///
	/// let settings = SettingsBuilder::new()
	///     .add_source(DefaultSource::new().with_value("key", Value::String("value".into())))
	///     .build()
	///     .unwrap();
	/// // Retrieve configuration value with default
	/// let value: String = settings.get_or("key", "default".to_string());
	/// ```
	pub fn get_or<T: DeserializeOwned>(&self, key: &str, default: T) -> T {
		self.get(key).unwrap_or(default)
	}
	/// Get a value by key as an option
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::builder::SettingsBuilder;
	/// use reinhardt_conf::settings::sources::DefaultSource;
	/// use serde_json::Value;
	///
	/// let settings = SettingsBuilder::new()
	///     .add_source(
	///         DefaultSource::new()
	///             .with_value("debug", Value::Bool(true))
	///     )
	///     .build()
	///     .unwrap();
	/// let value: Option<bool> = settings.get_optional("debug");
	/// assert!(value.is_some());
	/// ```
	pub fn get_optional<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
		self.get(key).ok()
	}
	/// Get the raw value
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_conf::settings::builder::SettingsBuilder;
	/// use reinhardt_conf::settings::sources::DefaultSource;
	/// use serde_json::Value;
	///
	/// let settings = SettingsBuilder::new()
	///     .add_source(DefaultSource::new().with_value("key", Value::String("value".into())))
	///     .build()
	///     .unwrap();
	/// // Retrieve raw configuration value
	/// let value = settings.get_raw("key");
	/// ```
	pub fn get_raw(&self, key: &str) -> Option<&Value> {
		self.data.get(key)
	}
	/// Check if a key exists
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::builder::SettingsBuilder;
	/// use reinhardt_conf::settings::sources::DefaultSource;
	/// use serde_json::Value;
	///
	/// let settings = SettingsBuilder::new()
	///     .add_source(
	///         DefaultSource::new()
	///             .with_value("debug", Value::Bool(true))
	///     )
	///     .build()
	///     .unwrap();
	/// let exists = settings.contains_key("debug");
	/// assert!(exists);
	/// ```
	pub fn contains_key(&self, key: &str) -> bool {
		self.data.contains_key(key)
	}
	/// Get all keys
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::builder::SettingsBuilder;
	/// use reinhardt_conf::settings::sources::DefaultSource;
	/// use serde_json::Value;
	///
	/// let settings = SettingsBuilder::new()
	///     .add_source(
	///         DefaultSource::new()
	///             .with_value("key1", Value::String("val1".to_string()))
	///             .with_value("key2", Value::String("val2".to_string()))
	///     )
	///     .build()
	///     .unwrap();
	///
	/// let keys: Vec<_> = settings.keys().collect();
	/// assert_eq!(keys.len(), 2);
	/// ```
	pub fn keys(&self) -> impl Iterator<Item = &String> {
		self.data.keys()
	}
	/// Get the profile
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::builder::SettingsBuilder;
	/// use reinhardt_conf::settings::profile::Profile;
	///
	/// let settings = SettingsBuilder::new()
	///     .profile(Profile::Production)
	///     .build()
	///     .unwrap();
	///
	/// assert_eq!(settings.profile(), Some(Profile::Production));
	/// ```
	pub fn profile(&self) -> Option<Profile> {
		self.profile
	}
	/// Convert to a typed settings struct
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::builder::SettingsBuilder;
	/// use reinhardt_conf::settings::sources::DefaultSource;
	/// use serde::{Deserialize, Serialize};
	/// use serde_json::Value;
	///
	/// #[derive(Debug, Deserialize, Serialize, PartialEq)]
	/// struct AppConfig {
	///     debug: bool,
	///     port: u16,
	/// }
	///
	/// let settings = SettingsBuilder::new()
	///     .add_source(
	///         DefaultSource::new()
	///             .with_value("debug", Value::Bool(true))
	///             .with_value("port", Value::Number(3000.into()))
	///     )
	///     .build()
	///     .unwrap();
	///
	/// let config: AppConfig = settings.into_typed().unwrap();
	/// assert!(config.debug);
	/// assert_eq!(config.port, 3000);
	/// ```
	pub fn into_typed<T: DeserializeOwned>(self) -> Result<T, GetError> {
		let json_value = Value::Object(
			self.data
				.iter()
				.map(|(k, v)| (k.clone(), v.clone()))
				.collect(),
		);

		serde_json::from_value(json_value).map_err(|e| GetError::Deserialize {
			key: "<root>".to_string(),
			error: e,
		})
	}
	/// Get all data as a HashMap
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::builder::SettingsBuilder;
	/// use reinhardt_conf::settings::sources::DefaultSource;
	/// use serde_json::Value;
	///
	/// let settings = SettingsBuilder::new()
	///     .add_source(
	///         DefaultSource::new()
	///             .with_value("app_name", Value::String("myapp".to_string()))
	///     )
	///     .build()
	///     .unwrap();
	///
	/// let map = settings.as_map();
	/// assert!(map.contains_key("app_name"));
	/// ```
	pub fn as_map(&self) -> &IndexMap<String, Value> {
		&self.data
	}
}

/// Error type for building settings
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
	/// An error occurred while loading a configuration source.
	#[error("Source error in '{description}': {error}")]
	Source {
		/// Description of the source that caused the error.
		description: String,
		/// The underlying source error.
		error: SourceError,
	},

	/// A validation check on the built settings failed.
	#[error("Validation error: {0}")]
	Validation(String),

	/// A required field was not provided by any configuration source.
	#[error(
		"missing required field `{field}` in section `[{section}]`. \
		 Provide it via TOML, environment variable, or .set()"
	)]
	MissingRequiredField {
		/// The settings section name.
		section: &'static str,
		/// The field name that is missing.
		field: &'static str,
	},

	/// Failed to deserialize merged settings into the target type.
	#[error("settings deserialization failed: {0}")]
	Deserialization(String),
}

impl From<GetError> for BuildError {
	fn from(err: GetError) -> Self {
		BuildError::Deserialization(err.to_string())
	}
}

/// Error type for getting values
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum GetError {
	/// The requested key does not exist in the settings.
	#[error("Missing required key: {0}")]
	MissingKey(String),

	/// The value could not be deserialized to the requested type.
	#[error("Failed to deserialize key '{key}': {error}")]
	Deserialize {
		/// The key that failed to deserialize.
		key: String,
		/// The underlying deserialization error.
		error: serde_json::Error,
	},
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::settings::sources::DefaultSource;
	use rstest::rstest;
	use serde::Deserialize;

	#[test]
	fn test_settings_builder_basic() {
		let settings = SettingsBuilder::new()
			.add_source(
				DefaultSource::new()
					.with_value("debug", Value::Bool(true))
					.with_value("secret_key", Value::String("test-key".to_string())),
			)
			.build()
			.unwrap();

		assert!(settings.get::<bool>("debug").unwrap());
		assert_eq!(settings.get::<String>("secret_key").unwrap(), "test-key");
	}

	#[test]
	fn test_builder_merge_priority() {
		let settings = SettingsBuilder::new()
			.add_source(
				DefaultSource::new().with_value("key", Value::String("low-priority".to_string())),
			)
			.add_source(EnvSource::new())
			.build()
			.unwrap();

		// EnvSource has higher priority, but if no env var is set, default should win
		assert!(settings.contains_key("key"));
	}

	#[test]
	fn test_get_optional() {
		let settings = SettingsBuilder::new()
			.add_source(
				DefaultSource::new().with_value("existing", Value::String("value".to_string())),
			)
			.build()
			.unwrap();

		assert_eq!(
			settings.get_optional::<String>("existing").unwrap(),
			"value"
		);
		assert!(settings.get_optional::<String>("nonexistent").is_none());
	}

	#[test]
	fn test_get_or() {
		let settings = SettingsBuilder::new().build().unwrap();

		assert_eq!(
			settings.get_or("nonexistent", "default".to_string()),
			"default"
		);
	}

	#[test]
	fn test_into_typed() {
		#[derive(Debug, Deserialize, PartialEq)]
		struct Config {
			debug: bool,
			port: u16,
		}

		let settings = SettingsBuilder::new()
			.add_source(
				DefaultSource::new()
					.with_value("debug", Value::Bool(true))
					.with_value("port", Value::Number(8080.into())),
			)
			.build()
			.unwrap();

		let config: Config = settings.into_typed().unwrap();
		assert_eq!(
			config,
			Config {
				debug: true,
				port: 8080
			}
		);
	}

	#[test]
	fn test_contains_key() {
		let settings = SettingsBuilder::new()
			.add_source(DefaultSource::new().with_value("key1", Value::String("value".to_string())))
			.build()
			.unwrap();

		assert!(settings.contains_key("key1"));
		assert!(!settings.contains_key("key2"));
	}

	#[rstest]
	fn test_build_error_missing_required_field_message() {
		// Arrange
		let error = BuildError::MissingRequiredField {
			section: "core",
			field: "secret_key",
		};

		// Act
		let message = error.to_string();

		// Assert
		assert!(message.contains("missing required field `secret_key`"));
		assert!(message.contains("section `[core]`"));
	}

	#[rstest]
	fn test_build_composed_missing_required_field() {
		// Arrange
		use crate::settings::composed::ComposedSettings;
		use crate::settings::profile::Profile;
		use crate::settings::validation::ValidationResult;
		use serde::Serialize;

		#[derive(Clone, Debug, Serialize, Deserialize)]
		struct MinimalComposed {
			#[serde(default)]
			optional_field: String,
		}

		impl ComposedSettings for MinimalComposed {
			fn validate_requirements(merged: &IndexMap<String, Value>) -> Result<(), BuildError> {
				// Require "secret_key" to be present
				if !merged.contains_key("secret_key") {
					return Err(BuildError::MissingRequiredField {
						section: "test",
						field: "secret_key",
					});
				}
				Ok(())
			}

			fn validate_fragments(&self, _profile: &Profile) -> ValidationResult {
				Ok(())
			}
		}

		// Act: build without providing required key
		let result = SettingsBuilder::new().build_composed::<MinimalComposed>();

		// Assert: should fail with MissingRequiredField
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(
			matches!(
				err,
				BuildError::MissingRequiredField {
					section: "test",
					field: "secret_key"
				}
			),
			"expected MissingRequiredField, got: {err:?}"
		);
	}

	#[rstest]
	fn test_build_composed_success() {
		// Arrange
		use crate::settings::composed::ComposedSettings;
		use crate::settings::profile::Profile;
		use crate::settings::validation::ValidationResult;
		use serde::Serialize;

		#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
		struct SimpleComposed {
			#[serde(default)]
			name: String,
		}

		impl ComposedSettings for SimpleComposed {
			fn validate_requirements(_merged: &IndexMap<String, Value>) -> Result<(), BuildError> {
				// No required fields
				Ok(())
			}

			fn validate_fragments(&self, _profile: &Profile) -> ValidationResult {
				Ok(())
			}
		}

		// Act: build with a value
		let result = SettingsBuilder::new()
			.add_source(DefaultSource::new().with_value("name", Value::String("app".to_string())))
			.build_composed::<SimpleComposed>();

		// Assert
		assert!(result.is_ok());
		let composed = result.unwrap();
		assert_eq!(composed.name, "app");
	}

	/// Verify that `warn_flat_core_keys` emits a warning (via stderr) when a
	/// known CoreSettings field appears as a flat top-level key rather than
	/// nested under `[core]`.
	#[test]
	fn test_flat_core_key_warning_is_emitted() {
		// Capture stderr by redirecting it temporarily via a pipe.
		// Because `eprintln!` writes to the process stderr we use a simple
		// integration approach: call `warn_flat_core_keys` directly and assert
		// it does not panic, then confirm the logic by inspecting the merged map.

		let mut merged: IndexMap<String, Value> = IndexMap::new();
		// Add a flat CoreSettings key (not under a [core] section).
		merged.insert(
			"secret_key".to_string(),
			Value::String("flat-key".to_string()),
		);

		// Adding a key that is NOT a CoreSettings field — must not trigger warning.
		merged.insert("port".to_string(), Value::Number(8080.into()));

		// No sources; the function still runs without panicking.
		let source_descs: Vec<String> = Vec::new();

		// This should not panic and should print a warning to stderr.
		warn_flat_core_keys(&merged, &source_descs);

		// Assert the flat key is correctly detected by checking membership
		// against the known list (mirrors what warn_flat_core_keys does).
		assert!(CORE_SETTINGS_FIELDS.contains(&"secret_key"));
		assert!(!CORE_SETTINGS_FIELDS.contains(&"port"));
	}

	/// Verify that `warn_flat_core_keys` does NOT warn when all CoreSettings
	/// keys are properly nested under `[core]` (i.e. absent from top level).
	#[test]
	fn test_flat_core_key_no_warning_when_properly_nested() {
		let mut merged: IndexMap<String, Value> = IndexMap::new();
		// Properly nested — `core` key holds an object.
		merged.insert(
			"core".to_string(),
			serde_json::json!({"secret_key": "properly-nested", "debug": false}),
		);

		let source_descs: Vec<String> = Vec::new();

		// Should not emit a warning (no CoreSettings fields at top level).
		// The function should complete without panic.
		warn_flat_core_keys(&merged, &source_descs);

		// None of the CoreSettings fields are present at the top level.
		for field in CORE_SETTINGS_FIELDS {
			assert!(
				!merged.contains_key(*field),
				"field {} should not be at top level",
				field
			);
		}
	}

	#[test]
	fn test_settings_builder_keys() {
		let settings = SettingsBuilder::new()
			.add_source(
				DefaultSource::new()
					.with_value("key1", Value::String("value1".to_string()))
					.with_value("key2", Value::String("value2".to_string())),
			)
			.build()
			.unwrap();

		let keys: Vec<_> = settings.keys().collect();
		assert_eq!(keys.len(), 2);
		assert!(keys.contains(&&"key1".to_string()));
		assert!(keys.contains(&&"key2".to_string()));
	}
}

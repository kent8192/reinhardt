//! Settings builder with layered configuration support
//!
//! Provides a builder pattern for constructing settings from multiple sources
//! with priority-based merging.

use super::composed::{ComposedSettings, ResolvedSettings};
use super::profile::Profile;
use super::sources::{ConfigSource, DotEnvSource, EnvSource, SourceError};
use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::sync::Arc;

/// Strategy for merging multiple configuration sources.
///
/// Selected via [`SettingsBuilder::with_merge_strategy`]. The default
/// differs between [`SettingsBuilder::build`] (uses
/// [`MergeStrategy::Shallow`]) and [`SettingsBuilder::build_composed`]
/// (uses [`MergeStrategy::Deep`]) — see those methods for the rationale.
///
/// See [issue #4260](https://github.com/kent8192/reinhardt-web/issues/4260).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MergeStrategy {
	/// Top-level key replacement. Each later source overwrites the entire
	/// value at any conflicting top-level key. This preserves env-source /
	/// flat-key composition (e.g., `REINHARDT_REDIS_URL` overwriting a
	/// scalar `redis_url` without disturbing other top-level keys) and
	/// matches the historical behaviour of [`SettingsBuilder::build`].
	Shallow,
	/// Recursive merge of nested tables. When two sources both define a
	/// table at the same key, sibling keys from both sides are preserved
	/// and only conflicting leaves are replaced. Arrays and scalars are
	/// still replaced wholesale, so flat-key fallback paths continue to
	/// work the same as under [`MergeStrategy::Shallow`].
	Deep,
}

/// Settings builder for layered configuration
pub struct SettingsBuilder {
	sources: Vec<Box<dyn ConfigSource>>,
	profile: Option<Profile>,
	strict: bool,
	typed_coercion: bool,
	merge_strategy: Option<MergeStrategy>,
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
			typed_coercion: true,
			merge_strategy: None,
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
	/// Enable or disable typed string coercion at deserialize time.
	///
	/// When `true` (default), `Value::String` values whose target type is
	/// not `String` are parsed via the type's `FromStr` / JSON
	/// representation. When `false`, the legacy `serde_json::from_value`
	/// passthrough is used and string-typed fields surface as serde
	/// type-mismatch errors.
	///
	/// See [issue #4226](https://github.com/kent8192/reinhardt-web/issues/4226).
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::builder::SettingsBuilder;
	///
	/// // Disable typed coercion to fall through to the legacy path.
	/// let builder = SettingsBuilder::new().with_typed_coercion(false);
	/// let settings = builder.build().unwrap();
	/// assert_eq!(settings.keys().count(), 0);
	/// ```
	pub fn with_typed_coercion(mut self, enable: bool) -> Self {
		self.typed_coercion = enable;
		self
	}
	/// Override the merge strategy used when combining configuration sources.
	///
	/// When unset, [`SettingsBuilder::build`] defaults to
	/// [`MergeStrategy::Shallow`] and [`SettingsBuilder::build_composed`]
	/// defaults to [`MergeStrategy::Deep`]. Calling this method forces both
	/// build paths to use the supplied strategy regardless of the entry
	/// point.
	///
	/// See [issue #4260](https://github.com/kent8192/reinhardt-web/issues/4260)
	/// for the design discussion.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::builder::{MergeStrategy, SettingsBuilder};
	/// use reinhardt_conf::settings::sources::DefaultSource;
	/// use serde_json::json;
	///
	/// // Force `build()` to deep-merge layered TOML files.
	/// let builder = SettingsBuilder::new()
	///     .add_source(DefaultSource::new().with_value("core", json!({
	///         "secret_key": "from-base",
	///         "security": {"secure_ssl_redirect": true},
	///     })))
	///     .add_source(DefaultSource::new().with_value("core", json!({"debug": true})))
	///     .with_merge_strategy(MergeStrategy::Deep);
	/// let settings = builder.build().unwrap();
	///
	/// let core = settings.get_raw("core").unwrap().as_object().unwrap();
	/// assert_eq!(core.get("debug").unwrap(), &json!(true));
	/// assert_eq!(core.get("secret_key").unwrap(), &json!("from-base"));
	/// assert!(core.get("security").is_some());
	/// ```
	pub fn with_merge_strategy(mut self, strategy: MergeStrategy) -> Self {
		self.merge_strategy = Some(strategy);
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
	///
	/// # Merge strategy
	///
	/// Defaults to [`MergeStrategy::Deep`] so that layered TOML files
	/// (e.g. `base.toml` + `local.toml`) preserve sibling keys inside
	/// nested tables. Use [`SettingsBuilder::with_merge_strategy`] to
	/// opt back into [`MergeStrategy::Shallow`] for the legacy
	/// top-level-replacement behaviour. See
	/// [issue #4260](https://github.com/kent8192/reinhardt-web/issues/4260).
	pub fn build_composed<T: ComposedSettings>(self) -> Result<T, BuildError> {
		let (merged, typed_coercion) = self.build_composed_merged::<T>()?;
		deserialize_composed(merged, typed_coercion)
	}

	/// Build composed settings with value-free metadata about resolved leaves.
	pub fn build_resolved_composed<T: ComposedSettings>(
		self,
	) -> Result<ResolvedSettings<T>, BuildError> {
		let (merged, typed_coercion) = self.build_composed_merged::<T>()?;
		let metadata = T::resolution_metadata(merged.as_map())?;
		let settings = deserialize_composed(merged, typed_coercion)?;
		Ok(ResolvedSettings { settings, metadata })
	}

	fn build_composed_merged<T: ComposedSettings>(
		mut self,
	) -> Result<(MergedSettings, bool), BuildError> {
		// `build_composed` exists for layered TOML files where deep merging is
		// the natural expectation. Apply the deep default only when the caller
		// has not explicitly chosen a strategy, so explicit `Shallow` opt-outs
		// still work.
		if self.merge_strategy.is_none() {
			self.merge_strategy = Some(MergeStrategy::Deep);
		}
		// Capture the flag before `self.build()` consumes self.
		let typed_coercion = self.typed_coercion;
		let merged = self.build()?;
		T::validate_requirements(merged.as_map())?;

		Ok((merged, typed_coercion))
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

		// Resolve the merge strategy for this build. `build()` keeps `Shallow`
		// as its default to preserve env-source / flat-key composition; only
		// callers that explicitly opt in (or `build_composed`, which sets its
		// own default upstream) will recurse into nested tables.
		// See issue #4260.
		let strategy = self.merge_strategy.unwrap_or(MergeStrategy::Shallow);

		let mut merged = IndexMap::new();
		// Track every loaded source's map alongside its description so that flat-key
		// diagnostics can be attributed to the originating source rather than to the
		// post-merge view. Internal sources (e.g., DefaultSource) are filtered out
		// during warning emission rather than at collection time.
		let mut per_source: Vec<(String, IndexMap<String, Value>)> =
			Vec::with_capacity(self.sources.len());

		// Merge all sources in priority order (lowest to highest)
		// Later sources will overwrite earlier ones
		for source in &self.sources {
			let description = source.description();
			let config = source.load().map_err(|e| BuildError::Source {
				description: description.clone(),
				error: e,
			})?;

			match strategy {
				MergeStrategy::Shallow => {
					for (key, value) in &config {
						merged.insert(key.clone(), value.clone());
					}
				}
				MergeStrategy::Deep => {
					super::merge::deep_merge(&mut merged, config.clone());
				}
			}

			per_source.push((description, config));
		}

		// Apply thread-local test overrides (highest priority, above all sources).
		// Overrides are internal test machinery and intentionally bypass the
		// flat-key warning logic below.
		if let Some(overrides) = super::testing::overrides::current_overrides() {
			super::merge::deep_merge(&mut merged, overrides);
		}

		// Warn about flat top-level keys that belong under [core], deciding per
		// source so that built-in defaults never trigger noisy false positives.
		for (description, config) in &per_source {
			if is_default_source_description(description) {
				continue;
			}
			for warning in flat_core_warnings(config, description) {
				eprintln!("{warning}");
			}
		}

		Ok(MergedSettings {
			data: Arc::new(merged),
			profile: self.profile,
			typed_coercion: self.typed_coercion,
		})
	}
}

fn deserialize_composed<T: ComposedSettings>(
	merged: MergedSettings,
	typed_coercion: bool,
) -> Result<T, BuildError> {
	if typed_coercion {
		use crate::settings::typed_deserializer::TypedSettingsDeserializer;
		let json_value = Value::Object(
			merged
				.as_map()
				.iter()
				.map(|(key, value)| (key.clone(), value.clone()))
				.collect(),
		);
		let de = TypedSettingsDeserializer::new(&json_value);
		T::deserialize(de).map_err(BuildError::Coercion)
	} else {
		merged.into_typed().map_err(BuildError::from)
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

/// Description string used by the built-in `DefaultSource`. The default source
/// uses `#[serde(flatten)]` to populate every `CoreSettings` field at the top
/// level, so it would otherwise spuriously trigger the flat-key warning on
/// every build. Matching by description lets us skip it without leaking source
/// internals into this module.
const DEFAULT_SOURCE_DESCRIPTION: &str = "Default values";

/// Returns true when the given source description identifies the built-in
/// `DefaultSource`, whose flat top-level layout is intentional.
fn is_default_source_description(description: &str) -> bool {
	description == DEFAULT_SOURCE_DESCRIPTION
}

/// Build the list of flat-key warning messages a single user-controlled
/// configuration source would produce.
///
/// For each top-level key in `source_map` that matches a known `CoreSettings`
/// field, returns one warning string explaining that the key must live under
/// `[core]`. Returns an empty vector when nothing is wrong, which makes the
/// helper trivially testable.
///
/// The caller is responsible for skipping internal sources whose flat layout
/// is intentional (e.g. the built-in `DefaultSource`).
fn flat_core_warnings(
	source_map: &IndexMap<String, Value>,
	source_description: &str,
) -> Vec<String> {
	let mut warnings = Vec::new();
	for &field in CORE_SETTINGS_FIELDS {
		if source_map.contains_key(field) {
			warnings.push(format!(
				"[reinhardt-conf] Warning: settings source '{source_description}' contains top-level key '{field}' outside any section.\n\
				 This key is part of CoreSettings and must be placed under [core] to take effect.\n\
				 Hint: wrap the key in a [core] section header."
			));
		}
	}
	warnings
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
	typed_coercion: bool,
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

		if self.typed_coercion {
			use crate::settings::typed_deserializer::TypedSettingsDeserializer;
			use serde::de::Error as _;
			let de = TypedSettingsDeserializer::new(&json_value);
			T::deserialize(de).map_err(|e| GetError::Deserialize {
				key: "<root>".to_string(),
				// Bridge: CoercionError's Display preserves all the structured info
				// (target type, key path, parse source). We surface the message
				// through `serde_json::Error::custom` so the existing GetError shape
				// is preserved without breaking downstream pattern matches.
				error: serde_json::Error::custom(e.to_string()),
			})
		} else {
			serde_json::from_value(json_value).map_err(|e| GetError::Deserialize {
				key: "<root>".to_string(),
				error: e,
			})
		}
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
	/// A manual composed type did not provide resolution metadata.
	#[error(
		"resolved settings metadata is unavailable for `{settings_type}`; use a macro-generated composed type or implement ComposedSettings::resolution_metadata"
	)]
	ResolutionMetadataUnavailable {
		/// The composed settings type that lacks metadata support.
		settings_type: &'static str,
	},

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

	/// A required nested settings path was not provided by any configuration source.
	#[error(
		"missing required settings path `{path}`. \
		 Provide it via TOML, environment variable, or .set()"
	)]
	MissingRequiredPath {
		/// The full settings path that is missing.
		path: crate::settings::schema::SettingsPathBuf,
	},

	/// Failed to deserialize merged settings into the target type.
	#[error("settings deserialization failed: {0}")]
	Deserialization(String),

	/// Type coercion failed during the typed deserialize pass (issue #4226).
	#[error(transparent)]
	Coercion(#[from] crate::settings::typed_deserializer::CoercionError),
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

	#[rstest]
	fn build_resolved_composed_reports_unavailable_manual_metadata() {
		use crate::settings::composed::ComposedSettings;
		use crate::settings::profile::Profile;
		use crate::settings::validation::ValidationResult;
		use serde::Serialize;

		#[derive(Clone, Debug, Serialize, Deserialize)]
		struct ManualComposed {
			#[serde(default)]
			secret: String,
		}

		impl ComposedSettings for ManualComposed {
			fn validate_requirements(_merged: &IndexMap<String, Value>) -> Result<(), BuildError> {
				Ok(())
			}

			fn validate_fragments(&self, _profile: &Profile) -> ValidationResult {
				Ok(())
			}
		}

		let error = SettingsBuilder::new()
			.add_source(
				DefaultSource::new()
					.with_value("secret", Value::String("do-not-store-this".to_string())),
			)
			.build_resolved_composed::<ManualComposed>()
			.err()
			.expect("manual composed settings do not provide resolution metadata");

		assert!(matches!(
			error,
			BuildError::ResolutionMetadataUnavailable { settings_type }
				if settings_type == std::any::type_name::<ManualComposed>()
		));
	}

	/// `flat_core_warnings` emits one message per known CoreSettings field that
	/// appears as a flat top-level key in a user-controlled source.
	#[rstest]
	fn test_flat_core_warnings_detects_flat_core_key() {
		// Arrange
		let mut source_map: IndexMap<String, Value> = IndexMap::new();
		source_map.insert(
			"secret_key".to_string(),
			Value::String("flat-key".to_string()),
		);
		// A non-CoreSettings key must never trigger a warning.
		source_map.insert("port".to_string(), Value::Number(8080.into()));

		// Act
		let warnings = flat_core_warnings(&source_map, "TOML file: local.toml");

		// Assert
		assert_eq!(warnings.len(), 1);
		assert!(warnings[0].contains("'secret_key'"));
		assert!(warnings[0].contains("TOML file: local.toml"));
	}

	/// `flat_core_warnings` returns no messages when every CoreSettings field is
	/// properly nested under `[core]` (and therefore absent from the top level).
	#[rstest]
	fn test_flat_core_warnings_silent_when_properly_nested() {
		// Arrange
		let mut source_map: IndexMap<String, Value> = IndexMap::new();
		source_map.insert(
			"core".to_string(),
			serde_json::json!({"secret_key": "properly-nested", "debug": false}),
		);

		// Act
		let warnings = flat_core_warnings(&source_map, "TOML file: local.toml");

		// Assert
		assert!(warnings.is_empty());
	}

	/// A builder configured with only `DefaultSource` must not produce any
	/// flat-key warnings, regardless of how many CoreSettings fields the
	/// default source flattens onto the top level.
	#[rstest]
	fn test_default_source_alone_produces_no_warnings() {
		// Arrange: simulate the flat top-level layout that `DefaultSource`
		// produces via `#[serde(flatten)]` over `CoreSettings`.
		let mut default_map: IndexMap<String, Value> = IndexMap::new();
		default_map.insert("debug".to_string(), Value::Bool(false));
		default_map.insert(
			"secret_key".to_string(),
			Value::String("default".to_string()),
		);
		default_map.insert("installed_apps".to_string(), serde_json::json!([]));

		// Act: the builder logic skips the default source by description, so
		// confirm both sides of that contract.
		assert!(is_default_source_description("Default values"));
		let warnings_if_evaluated = flat_core_warnings(&default_map, "Default values");

		// Assert: had the default source been evaluated, it would have produced
		// noise; the builder must skip it before reaching this branch.
		assert!(!warnings_if_evaluated.is_empty());
	}

	/// A user TOML source that correctly nests everything under `[core]`
	/// produces no warnings even when merged on top of the default source.
	#[rstest]
	fn test_user_source_with_properly_nested_core_produces_no_warnings() {
		// Arrange: user TOML loads as a single top-level `core` table.
		let mut user_map: IndexMap<String, Value> = IndexMap::new();
		user_map.insert(
			"core".to_string(),
			serde_json::json!({
				"secret_key": "user-secret",
				"debug": true,
				"allowed_hosts": ["localhost"],
			}),
		);

		// Act
		let warnings = flat_core_warnings(&user_map, "TOML file: settings.toml");

		// Assert
		assert!(warnings.is_empty());
	}

	/// A user TOML source with a flat top-level `secret_key` produces exactly
	/// one warning that names both the offending key and the source.
	#[rstest]
	fn test_user_source_with_flat_secret_key_produces_one_warning() {
		// Arrange
		let mut user_map: IndexMap<String, Value> = IndexMap::new();
		user_map.insert(
			"secret_key".to_string(),
			Value::String("flat-user-secret".to_string()),
		);

		// Act
		let warnings = flat_core_warnings(&user_map, "TOML file: settings.toml");

		// Assert
		assert_eq!(warnings.len(), 1);
		assert!(warnings[0].contains("secret_key"));
		assert!(warnings[0].contains("TOML file: settings.toml"));
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

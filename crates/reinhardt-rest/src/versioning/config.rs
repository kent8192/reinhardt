//! Global configuration system for API versioning
//!
//! Provides centralized configuration management for versioning strategies,
//! allowing applications to configure versioning behavior through settings.

use super::{
	AcceptHeaderVersioning, HostNameVersioning, NamespaceVersioning, QueryParameterVersioning,
	URLPathVersioning,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Global versioning configuration
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersioningConfig {
	/// Default version to use when no version is specified
	pub default_version: String,

	/// Allowed versions (empty means any version is allowed)
	pub allowed_versions: Vec<String>,

	/// Versioning strategy configuration
	pub strategy: VersioningStrategy,

	/// Whether to raise errors for invalid versions
	pub strict_mode: bool,

	/// Custom version parameter name for query parameter versioning
	pub version_param: Option<String>,

	/// Custom hostname patterns for hostname versioning
	pub hostname_patterns: Option<HashMap<String, String>>,
}

impl Default for VersioningConfig {
	fn default() -> Self {
		Self {
			default_version: "1.0".to_string(),
			allowed_versions: vec![],
			strategy: VersioningStrategy::AcceptHeader,
			strict_mode: true,
			version_param: None,
			hostname_patterns: None,
		}
	}
}

/// Versioning strategy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "config")]
pub enum VersioningStrategy {
	/// Accept header versioning
	AcceptHeader,

	/// URL path versioning
	URLPath {
		/// URL pattern for version extraction (e.g., "/v{version}/")
		pattern: Option<String>,
	},

	/// Query parameter versioning
	QueryParameter {
		/// Parameter name (default: "version")
		param_name: Option<String>,
	},

	/// Hostname versioning
	HostName {
		/// Hostname patterns mapping versions to hostnames
		patterns: Option<HashMap<String, String>>,
	},

	/// Namespace versioning
	Namespace {
		/// Namespace pattern (e.g., "/v{version}/")
		pattern: Option<String>,
	},
}

impl VersioningConfig {
	/// Create a new versioning configuration
	pub fn new() -> Self {
		Self::default()
	}

	/// Set the default version
	pub fn with_default_version(mut self, version: impl Into<String>) -> Self {
		self.default_version = version.into();
		self
	}

	/// Set allowed versions
	pub fn with_allowed_versions(mut self, versions: Vec<String>) -> Self {
		self.allowed_versions = versions;
		self
	}

	/// Set versioning strategy
	pub fn with_strategy(mut self, strategy: VersioningStrategy) -> Self {
		self.strategy = strategy;
		self
	}

	/// Enable or disable strict mode
	pub fn with_strict_mode(mut self, strict: bool) -> Self {
		self.strict_mode = strict;
		self
	}

	/// Set custom version parameter name
	pub fn with_version_param(mut self, param: impl Into<String>) -> Self {
		self.version_param = Some(param.into());
		self
	}

	/// Set hostname patterns
	pub fn with_hostname_patterns(mut self, patterns: HashMap<String, String>) -> Self {
		self.hostname_patterns = Some(patterns);
		self
	}

	/// Create versioning instance from configuration
	pub fn create_versioning(&self) -> Box<dyn super::BaseVersioning + Send + Sync> {
		match &self.strategy {
			VersioningStrategy::AcceptHeader => {
				let mut versioning =
					AcceptHeaderVersioning::new().with_default_version(&self.default_version);

				if !self.allowed_versions.is_empty() {
					versioning = versioning.with_allowed_versions(self.allowed_versions.clone());
				}

				Box::new(versioning)
			}

			VersioningStrategy::URLPath { pattern } => {
				let mut versioning =
					URLPathVersioning::new().with_default_version(&self.default_version);

				if let Some(p) = pattern {
					versioning = versioning.with_pattern(p);
				}

				if !self.allowed_versions.is_empty() {
					versioning = versioning.with_allowed_versions(self.allowed_versions.clone());
				}

				Box::new(versioning)
			}

			VersioningStrategy::QueryParameter { param_name } => {
				let mut versioning =
					QueryParameterVersioning::new().with_default_version(&self.default_version);

				if let Some(name) = param_name {
					versioning = versioning.with_version_param(name);
				} else if let Some(name) = &self.version_param {
					versioning = versioning.with_version_param(name);
				}

				if !self.allowed_versions.is_empty() {
					versioning = versioning.with_allowed_versions(self.allowed_versions.clone());
				}

				Box::new(versioning)
			}

			VersioningStrategy::HostName { patterns } => {
				let mut versioning =
					HostNameVersioning::new().with_default_version(&self.default_version);

				if let Some(p) = patterns {
					for (version, hostname) in p {
						versioning = versioning.with_hostname_pattern(version, hostname);
					}
				} else if let Some(p) = &self.hostname_patterns {
					for (version, hostname) in p {
						versioning = versioning.with_hostname_pattern(version, hostname);
					}
				}

				if !self.allowed_versions.is_empty() {
					versioning = versioning.with_allowed_versions(self.allowed_versions.clone());
				}

				Box::new(versioning)
			}

			VersioningStrategy::Namespace { pattern } => {
				let mut versioning =
					NamespaceVersioning::new().with_default_version(&self.default_version);

				if let Some(p) = pattern {
					versioning = versioning.with_pattern(p);
				}

				if !self.allowed_versions.is_empty() {
					versioning = versioning.with_allowed_versions(self.allowed_versions.clone());
				}

				Box::new(versioning)
			}
		}
	}
}

impl VersioningConfig {
	/// Create a new versioning configuration from environment variables
	pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
		// Try to load from environment variables
		let default_version = std::env::var("REINHARDT_VERSIONING_DEFAULT_VERSION")
			.unwrap_or_else(|_| "1.0".to_string());

		let allowed_versions = std::env::var("REINHARDT_VERSIONING_ALLOWED_VERSIONS")
			.map(|v| v.split(',').map(|s| s.trim().to_string()).collect())
			.unwrap_or_default();

		let strategy = std::env::var("REINHARDT_VERSIONING_STRATEGY")
			.unwrap_or_else(|_| "accept_header".to_string());

		let strategy = match strategy.to_lowercase().as_str() {
			"accept_header" => VersioningStrategy::AcceptHeader,
			"url_path" => VersioningStrategy::URLPath { pattern: None },
			"query_parameter" => VersioningStrategy::QueryParameter { param_name: None },
			"hostname" => VersioningStrategy::HostName { patterns: None },
			"namespace" => VersioningStrategy::Namespace { pattern: None },
			_ => VersioningStrategy::AcceptHeader,
		};

		let strict_mode = std::env::var("REINHARDT_VERSIONING_STRICT_MODE")
			.map(|v| v.to_lowercase() == "true")
			.unwrap_or(true);

		Ok(VersioningConfig {
			default_version,
			allowed_versions,
			strategy,
			strict_mode,
			version_param: None,
			hostname_patterns: None,
		})
	}
}

/// Global versioning manager
pub struct VersioningManager {
	config: VersioningConfig,
	versioning: Arc<dyn super::BaseVersioning + Send + Sync>,
}

impl std::fmt::Debug for VersioningManager {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("VersioningManager")
			.field("config", &self.config)
			.field("versioning", &"<dyn BaseVersioning>")
			.finish()
	}
}

impl VersioningManager {
	/// Create a new versioning manager with configuration
	pub fn new(config: VersioningConfig) -> Self {
		let versioning = config.create_versioning();
		Self {
			config,
			versioning: Arc::from(versioning),
		}
	}

	/// Get the current configuration
	pub fn config(&self) -> &VersioningConfig {
		&self.config
	}

	/// Get the versioning instance
	pub fn versioning(&self) -> Arc<dyn super::BaseVersioning + Send + Sync> {
		self.versioning.clone()
	}

	/// Update configuration and recreate versioning instance
	pub fn update_config(&mut self, config: VersioningConfig) {
		self.config = config;
		self.versioning = Arc::from(self.config.create_versioning());
	}
}

impl Default for VersioningManager {
	fn default() -> Self {
		Self::new(VersioningConfig::default())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use serial_test::serial;
	use std::collections::HashMap;
	use std::env;

	/// Clear all versioning-related environment variables
	///
	/// # Safety
	/// This function modifies environment variables. It should only be called
	/// in single-threaded test contexts with `#[serial]` attribute.
	unsafe fn clear_versioning_env_vars() {
		// SAFETY: This is inside an unsafe fn and the caller ensures serial execution
		unsafe {
			env::remove_var("REINHARDT_VERSIONING_DEFAULT_VERSION");
			env::remove_var("REINHARDT_VERSIONING_ALLOWED_VERSIONS");
			env::remove_var("REINHARDT_VERSIONING_STRATEGY");
			env::remove_var("REINHARDT_VERSIONING_STRICT_MODE");
		}
	}

	#[test]
	fn test_versioning_config_default() {
		let config = VersioningConfig::default();
		assert_eq!(config.default_version, "1.0");
		assert!(config.allowed_versions.is_empty());
		assert!(matches!(config.strategy, VersioningStrategy::AcceptHeader));
		assert!(config.strict_mode);
	}

	#[test]
	fn test_versioning_config_builder() {
		let config = VersioningConfig::new()
			.with_default_version("2.0")
			.with_allowed_versions(vec!["2.0".to_string(), "3.0".to_string()])
			.with_strategy(VersioningStrategy::URLPath { pattern: None })
			.with_strict_mode(false);

		assert_eq!(config.default_version, "2.0");
		assert_eq!(config.allowed_versions, vec!["2.0", "3.0"]);
		assert!(matches!(
			config.strategy,
			VersioningStrategy::URLPath { .. }
		));
		assert!(!config.strict_mode);
	}

	#[test]
	fn test_versioning_strategy_serialization() {
		let strategy = VersioningStrategy::QueryParameter {
			param_name: Some("v".to_string()),
		};

		let json = serde_json::to_string(&strategy).unwrap();
		let deserialized: VersioningStrategy = serde_json::from_str(&json).unwrap();

		match deserialized {
			VersioningStrategy::QueryParameter { param_name } => {
				assert_eq!(param_name, Some("v".to_string()));
			}
			_ => panic!("Expected QueryParameter strategy"),
		}
	}

	#[test]
	fn test_versioning_manager_creation() {
		let config = VersioningConfig::new()
			.with_default_version("1.0")
			.with_strategy(VersioningStrategy::AcceptHeader);

		let manager = VersioningManager::new(config);
		assert_eq!(manager.config().default_version, "1.0");
	}

	#[tokio::test]
	async fn test_hostname_patterns() {
		let mut patterns = HashMap::new();
		patterns.insert("v1".to_string(), "v1.api.example.com".to_string());
		patterns.insert("v2".to_string(), "v2.api.example.com".to_string());

		let config = VersioningConfig::new().with_strategy(VersioningStrategy::HostName {
			patterns: Some(patterns.clone()),
		});

		let versioning = config.create_versioning();
		// The versioning instance should be created successfully
		assert!(
			versioning
				.determine_version(&crate::versioning::test_utils::create_test_request(
					"/",
					vec![]
				))
				.await
				.is_ok()
		);
	}

	#[test]
	#[serial(versioning_env)]
	fn test_from_env_default_values() {
		// SAFETY: This test runs serially with #[serial] attribute
		unsafe {
			clear_versioning_env_vars();
		}

		let config = VersioningConfig::from_env().unwrap();

		assert_eq!(config.default_version, "1.0");
		assert!(config.allowed_versions.is_empty());
		assert!(matches!(config.strategy, VersioningStrategy::AcceptHeader));
		assert!(config.strict_mode);
	}

	#[test]
	#[serial(versioning_env)]
	fn test_from_env_custom_default_version() {
		// SAFETY: This test runs serially with #[serial] attribute
		unsafe {
			clear_versioning_env_vars();
			env::set_var("REINHARDT_VERSIONING_DEFAULT_VERSION", "2.0");
		}

		let config = VersioningConfig::from_env().unwrap();
		assert_eq!(config.default_version, "2.0");

		// SAFETY: Cleanup after test
		unsafe {
			clear_versioning_env_vars();
		}
	}

	#[test]
	#[serial(versioning_env)]
	fn test_from_env_allowed_versions_parsing() {
		// SAFETY: This test runs serially with #[serial] attribute
		unsafe {
			clear_versioning_env_vars();
			env::set_var("REINHARDT_VERSIONING_ALLOWED_VERSIONS", "1.0, 2.0, 3.0");
		}

		let config = VersioningConfig::from_env().unwrap();
		assert_eq!(config.allowed_versions.len(), 3);
		assert!(config.allowed_versions.contains(&"1.0".to_string()));
		assert!(config.allowed_versions.contains(&"2.0".to_string()));
		assert!(config.allowed_versions.contains(&"3.0".to_string()));

		// SAFETY: Cleanup after test
		unsafe {
			clear_versioning_env_vars();
		}
	}

	#[test]
	#[serial(versioning_env)]
	fn test_from_env_strategy_url_path() {
		// SAFETY: This test runs serially with #[serial] attribute
		unsafe {
			clear_versioning_env_vars();
			env::set_var("REINHARDT_VERSIONING_STRATEGY", "url_path");
		}

		let config = VersioningConfig::from_env().unwrap();
		assert!(matches!(
			config.strategy,
			VersioningStrategy::URLPath { .. }
		));

		// SAFETY: Cleanup after test
		unsafe {
			clear_versioning_env_vars();
		}
	}

	#[test]
	#[serial(versioning_env)]
	fn test_from_env_strategy_query_parameter() {
		// SAFETY: This test runs serially with #[serial] attribute
		unsafe {
			clear_versioning_env_vars();
			env::set_var("REINHARDT_VERSIONING_STRATEGY", "query_parameter");
		}

		let config = VersioningConfig::from_env().unwrap();
		assert!(matches!(
			config.strategy,
			VersioningStrategy::QueryParameter { .. }
		));

		// SAFETY: Cleanup after test
		unsafe {
			clear_versioning_env_vars();
		}
	}

	#[test]
	#[serial(versioning_env)]
	fn test_from_env_strategy_hostname() {
		// SAFETY: This test runs serially with #[serial] attribute
		unsafe {
			clear_versioning_env_vars();
			env::set_var("REINHARDT_VERSIONING_STRATEGY", "hostname");
		}

		let config = VersioningConfig::from_env().unwrap();
		assert!(matches!(
			config.strategy,
			VersioningStrategy::HostName { .. }
		));

		// SAFETY: Cleanup after test
		unsafe {
			clear_versioning_env_vars();
		}
	}

	#[test]
	#[serial(versioning_env)]
	fn test_from_env_strategy_namespace() {
		// SAFETY: This test runs serially with #[serial] attribute
		unsafe {
			clear_versioning_env_vars();
			env::set_var("REINHARDT_VERSIONING_STRATEGY", "namespace");
		}

		let config = VersioningConfig::from_env().unwrap();
		assert!(matches!(
			config.strategy,
			VersioningStrategy::Namespace { .. }
		));

		// SAFETY: Cleanup after test
		unsafe {
			clear_versioning_env_vars();
		}
	}

	#[test]
	#[serial(versioning_env)]
	fn test_from_env_strict_mode_false() {
		// SAFETY: This test runs serially with #[serial] attribute
		unsafe {
			clear_versioning_env_vars();
			env::set_var("REINHARDT_VERSIONING_STRICT_MODE", "false");
		}

		let config = VersioningConfig::from_env().unwrap();
		assert!(!config.strict_mode);

		// SAFETY: Cleanup after test
		unsafe {
			clear_versioning_env_vars();
		}
	}

	#[test]
	#[serial(versioning_env)]
	fn test_from_env_strict_mode_true_explicit() {
		// SAFETY: This test runs serially with #[serial] attribute
		unsafe {
			clear_versioning_env_vars();
			env::set_var("REINHARDT_VERSIONING_STRICT_MODE", "true");
		}

		let config = VersioningConfig::from_env().unwrap();
		assert!(config.strict_mode);

		// SAFETY: Cleanup after test
		unsafe {
			clear_versioning_env_vars();
		}
	}

	#[test]
	#[serial(versioning_env)]
	fn test_from_env_combined_settings() {
		// SAFETY: This test runs serially with #[serial] attribute
		unsafe {
			clear_versioning_env_vars();
			env::set_var("REINHARDT_VERSIONING_DEFAULT_VERSION", "3.0");
			env::set_var("REINHARDT_VERSIONING_ALLOWED_VERSIONS", "2.0,3.0,4.0");
			env::set_var("REINHARDT_VERSIONING_STRATEGY", "url_path");
			env::set_var("REINHARDT_VERSIONING_STRICT_MODE", "false");
		}

		let config = VersioningConfig::from_env().unwrap();

		assert_eq!(config.default_version, "3.0");
		assert_eq!(config.allowed_versions.len(), 3);
		assert!(matches!(
			config.strategy,
			VersioningStrategy::URLPath { .. }
		));
		assert!(!config.strict_mode);

		// SAFETY: Cleanup after test
		unsafe {
			clear_versioning_env_vars();
		}
	}

	#[test]
	#[serial(versioning_env)]
	fn test_from_env_unknown_strategy_defaults_to_accept_header() {
		// SAFETY: This test runs serially with #[serial] attribute
		unsafe {
			clear_versioning_env_vars();
			env::set_var("REINHARDT_VERSIONING_STRATEGY", "unknown_strategy");
		}

		let config = VersioningConfig::from_env().unwrap();
		assert!(matches!(config.strategy, VersioningStrategy::AcceptHeader));

		// SAFETY: Cleanup after test
		unsafe {
			clear_versioning_env_vars();
		}
	}
}

//! Core settings fragment
//!
//! Essential configuration shared by all reinhardt applications.

use super::database_config::DatabaseConfig;
use super::fragment::SettingsFragment;
use super::profile::Profile;
use super::security::SecuritySettings;
use super::validation::{ValidationError, ValidationResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Core application settings.
///
/// Contains essential configuration: base directory, secret key, debug mode,
/// allowed hosts, database configs, security settings, middleware, and apps.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CoreSettings {
	/// Base directory of the project.
	#[serde(default = "default_base_dir")]
	pub base_dir: PathBuf,
	/// Secret key for cryptographic signing.
	pub secret_key: String,
	/// Debug mode flag.
	#[serde(default = "default_debug")]
	pub debug: bool,
	/// List of allowed host/domain names.
	#[serde(default)]
	pub allowed_hosts: Vec<String>,
	/// Database configurations keyed by alias.
	#[serde(default = "default_databases")]
	pub databases: HashMap<String, DatabaseConfig>,
	/// Security settings (nested fragment).
	///
	/// Flattened for backward-compatible deserialization: legacy TOML keys like
	/// `secure_ssl_redirect` at the top level are accepted alongside the nested
	/// `[security]` section form.
	#[serde(default, flatten)]
	pub security: SecuritySettings,
	/// Middleware class paths.
	#[serde(default)]
	pub middleware: Vec<String>,
	/// Root URL configuration module.
	#[serde(default)]
	pub root_urlconf: String,
	/// List of installed application paths.
	#[serde(default)]
	pub installed_apps: Vec<String>,
}

fn default_base_dir() -> PathBuf {
	PathBuf::from(".")
}

fn default_debug() -> bool {
	true
}

fn default_databases() -> HashMap<String, DatabaseConfig> {
	let mut map = HashMap::new();
	map.insert("default".to_string(), DatabaseConfig::default());
	map
}

impl Default for CoreSettings {
	fn default() -> Self {
		Self {
			base_dir: default_base_dir(),
			secret_key: String::new(),
			debug: true,
			allowed_hosts: Vec::new(),
			databases: default_databases(),
			security: SecuritySettings::default(),
			middleware: Vec::new(),
			root_urlconf: String::new(),
			installed_apps: Vec::new(),
		}
	}
}

impl SettingsFragment for CoreSettings {
	fn section() -> &'static str {
		"core"
	}

	fn validate(&self, profile: &Profile) -> ValidationResult {
		if self.secret_key.is_empty() {
			return Err(ValidationError::MissingRequired("secret_key".to_string()));
		}

		if profile.is_production() {
			if self.debug {
				return Err(ValidationError::Security(
					"debug must be false in production".to_string(),
				));
			}
			if self.allowed_hosts.is_empty() {
				return Err(ValidationError::MissingRequired(
					"allowed_hosts".to_string(),
				));
			}
		}

		// Delegate to nested security fragment
		self.security.validate(profile)?;

		Ok(())
	}
}

/// Trait for accessing [`CoreSettings`] from a composed settings type.
pub trait HasCoreSettings {
	/// Get a reference to the core settings.
	fn core(&self) -> &CoreSettings;
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::settings::fragment::SettingsFragment;
	use crate::settings::profile::Profile;
	use rstest::rstest;

	#[rstest]
	fn test_core_settings_section() {
		// Arrange / Act / Assert
		assert_eq!(CoreSettings::section(), "core");
	}

	#[rstest]
	fn test_core_settings_default() {
		// Arrange / Act
		let settings = CoreSettings::default();

		// Assert
		assert!(settings.debug);
		assert!(settings.secret_key.is_empty());
		assert!(settings.allowed_hosts.is_empty());
		assert!(settings.databases.contains_key("default"));
		assert!(!settings.security.secure_ssl_redirect);
	}

	#[rstest]
	fn test_core_settings_validate_missing_secret_key() {
		// Arrange
		let settings = CoreSettings::default();

		// Act
		let result = settings.validate(&Profile::Development);

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_core_settings_validate_development_ok() {
		// Arrange
		let settings = CoreSettings {
			secret_key: "test-secret-key".to_string(),
			..Default::default()
		};

		// Act
		let result = settings.validate(&Profile::Development);

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_core_settings_validate_production_debug_fails() {
		// Arrange
		let settings = CoreSettings {
			secret_key: "production-secret-key".to_string(),
			debug: true,
			..Default::default()
		};

		// Act
		let result = settings.validate(&Profile::Production);

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_core_settings_validate_production_ok() {
		// Arrange
		let settings = CoreSettings {
			secret_key: "production-secret-key".to_string(),
			debug: false,
			allowed_hosts: vec!["example.com".to_string()],
			security: SecuritySettings {
				secure_ssl_redirect: true,
				session_cookie_secure: true,
				csrf_cookie_secure: true,
				..Default::default()
			},
			..Default::default()
		};

		// Act
		let result = settings.validate(&Profile::Production);

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_core_settings_delegates_security_validation() {
		// Arrange - security validation fails in production
		let settings = CoreSettings {
			secret_key: "production-secret-key".to_string(),
			debug: false,
			allowed_hosts: vec!["example.com".to_string()],
			security: SecuritySettings::default(), // SSL redirect is false
			..Default::default()
		};

		// Act
		let result = settings.validate(&Profile::Production);

		// Assert
		assert!(result.is_err());
	}
}

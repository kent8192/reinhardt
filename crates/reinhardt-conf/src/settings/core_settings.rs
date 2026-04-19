//! Core settings fragment
//!
//! Essential configuration shared by all reinhardt applications.

use super::database_config::DatabaseConfig;
use super::fragment::SettingsValidation;
use super::profile::Profile;
use super::security::SecuritySettings;
use super::validation::{ValidationError, ValidationResult};
use reinhardt_core::macros::settings;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Core application settings.
///
/// Contains essential configuration: base directory, secret key, debug mode,
/// allowed hosts, database configs, security settings, middleware, and apps.
#[settings(fragment = true, section = "core", validate = false)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CoreSettings {
	/// Base directory of the project.
	#[serde(default = "default_base_dir")]
	pub base_dir: PathBuf,
	/// Secret key for cryptographic signing.
	#[setting(required)]
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
	/// Security settings (nested sub-section).
	///
	/// In TOML, security fields are placed under a `[core.security]` sub-section
	/// (composable settings) or `[security]` section (legacy `Settings`):
	///
	/// ```toml
	/// # Composable settings format
	/// [core]
	/// secret_key = "..."
	/// debug = false
	///
	/// [core.security]
	/// secure_ssl_redirect = true
	/// session_cookie_secure = true
	/// ```
	///
	/// ```toml
	/// # Legacy Settings format (CoreSettings flattened at root)
	/// secret_key = "..."
	/// debug = false
	///
	/// [security]
	/// secure_ssl_redirect = true
	/// session_cookie_secure = true
	/// ```
	#[serde(default)]
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

impl SettingsValidation for CoreSettings {
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

#[cfg(test)]
mod tests {
	use super::{CoreSettings, SecuritySettings};
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

	#[rstest]
	fn test_core_settings_field_policies_secret_key_required() {
		use crate::settings::policy::FieldRequirement;

		// Arrange / Act
		let policies = CoreSettings::field_policies();

		// Assert: secret_key must be marked as Required with no default
		let secret_key_policy = policies.iter().find(|p| p.name == "secret_key");
		assert!(
			secret_key_policy.is_some(),
			"secret_key must have an explicit field policy"
		);
		let policy = secret_key_policy.unwrap();
		assert_eq!(
			policy.requirement,
			FieldRequirement::Required,
			"secret_key must be Required"
		);
		assert!(
			!policy.has_default,
			"secret_key must not have a default value"
		);
	}

	#[rstest]
	fn test_core_settings_deserialize_nested_security_section() {
		// Arrange — composable settings format with [core.security] sub-section
		let toml_str = r#"
secret_key = "test-secret"
debug = false

[security]
secure_ssl_redirect = true
session_cookie_secure = true
csrf_cookie_secure = true
secure_hsts_seconds = 31536000
secure_hsts_include_subdomains = true
secure_hsts_preload = true
"#;

		// Act
		let settings: CoreSettings = toml::from_str(toml_str).expect("failed to parse TOML");

		// Assert — security fields parsed from nested section
		assert_eq!(settings.secret_key, "test-secret");
		assert!(!settings.debug);
		assert!(settings.security.secure_ssl_redirect);
		assert!(settings.security.session_cookie_secure);
		assert!(settings.security.csrf_cookie_secure);
		assert_eq!(settings.security.secure_hsts_seconds, Some(31536000));
		assert!(settings.security.secure_hsts_include_subdomains);
		assert!(settings.security.secure_hsts_preload);
	}

	#[rstest]
	fn test_core_settings_deserialize_omitted_security_uses_defaults() {
		// Arrange — no security section at all
		let toml_str = r#"
secret_key = "test-secret"
debug = true
"#;

		// Act
		let settings: CoreSettings = toml::from_str(toml_str).expect("failed to parse TOML");

		// Assert — security defaults applied
		assert!(!settings.security.secure_ssl_redirect);
		assert!(!settings.security.session_cookie_secure);
		assert!(!settings.security.csrf_cookie_secure);
		assert_eq!(settings.security.secure_hsts_seconds, None);
	}

	#[rstest]
	fn test_core_settings_deserialize_partial_security_section() {
		// Arrange — only some security fields specified
		let toml_str = r#"
secret_key = "test-secret"

[security]
secure_ssl_redirect = true
"#;

		// Act
		let settings: CoreSettings = toml::from_str(toml_str).expect("failed to parse TOML");

		// Assert — specified field overridden, others use defaults
		assert!(settings.security.secure_ssl_redirect);
		assert!(!settings.security.session_cookie_secure);
		assert!(!settings.security.csrf_cookie_secure);
	}

	#[rstest]
	fn test_core_settings_field_policies_other_fields_optional() {
		use crate::settings::policy::FieldRequirement;

		// Arrange / Act
		let policies = CoreSettings::field_policies();

		// Assert: all fields except secret_key are Optional with defaults
		let optional_fields = [
			"base_dir",
			"debug",
			"allowed_hosts",
			"databases",
			"security",
			"middleware",
			"root_urlconf",
			"installed_apps",
		];
		for field_name in optional_fields {
			let policy = policies.iter().find(|p| p.name == field_name);
			assert!(
				policy.is_some(),
				"field '{field_name}' must have a field policy"
			);
			let policy = policy.unwrap();
			assert_eq!(
				policy.requirement,
				FieldRequirement::Optional,
				"field '{field_name}' must be Optional"
			);
			assert!(
				policy.has_default,
				"field '{field_name}' must have a default value"
			);
		}
	}
}

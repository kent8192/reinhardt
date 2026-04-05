//! Security settings fragment
//!
//! Controls HTTPS enforcement, HSTS policy, cookie security, and URL handling.

use super::fragment::SettingsValidation;
use super::profile::Profile;
use super::validation::{ValidationError, ValidationResult};
use reinhardt_core::macros::settings;
use serde::{Deserialize, Serialize};

/// Security-related configuration settings.
///
/// Controls HTTPS enforcement, HSTS policy, cookie security, and URL handling.
/// Nested inside [`CoreSettings`] by default, but can also be used as
/// a top-level fragment.
///
/// [`CoreSettings`]: super::core_settings::CoreSettings
#[settings(fragment = true, section = "security", validate = false)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SecuritySettings {
	/// Header name and value for identifying secure requests behind a proxy.
	#[serde(default)]
	pub secure_proxy_ssl_header: Option<(String, String)>,
	/// Redirect all HTTP requests to HTTPS.
	#[serde(default)]
	pub secure_ssl_redirect: bool,
	/// Seconds to set HSTS max-age header.
	#[serde(default)]
	pub secure_hsts_seconds: Option<u64>,
	/// Include subdomains in HSTS policy.
	#[serde(default)]
	pub secure_hsts_include_subdomains: bool,
	/// Include preload directive in HSTS header.
	#[serde(default)]
	pub secure_hsts_preload: bool,
	/// Only send session cookies over HTTPS.
	#[serde(default)]
	pub session_cookie_secure: bool,
	/// Only send CSRF cookie over HTTPS.
	#[serde(default)]
	pub csrf_cookie_secure: bool,
	/// Automatically append trailing slashes to URLs.
	#[serde(default = "default_append_slash")]
	pub append_slash: bool,
}

fn default_append_slash() -> bool {
	true
}

impl Default for SecuritySettings {
	fn default() -> Self {
		Self {
			secure_proxy_ssl_header: None,
			secure_ssl_redirect: false,
			secure_hsts_seconds: None,
			secure_hsts_include_subdomains: false,
			secure_hsts_preload: false,
			session_cookie_secure: false,
			csrf_cookie_secure: false,
			append_slash: true,
		}
	}
}

impl SettingsValidation for SecuritySettings {
	fn validate(&self, profile: &Profile) -> ValidationResult {
		if profile.is_production() {
			if !self.secure_ssl_redirect {
				return Err(ValidationError::Security(
					"secure_ssl_redirect should be true in production".to_string(),
				));
			}
			if !self.session_cookie_secure {
				return Err(ValidationError::Security(
					"session_cookie_secure should be true in production".to_string(),
				));
			}
			if !self.csrf_cookie_secure {
				return Err(ValidationError::Security(
					"csrf_cookie_secure should be true in production".to_string(),
				));
			}
		}
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::SecuritySettings;
	use crate::settings::fragment::SettingsFragment;
	use crate::settings::profile::Profile;
	use rstest::rstest;

	#[rstest]
	fn test_security_settings_section() {
		// Arrange / Act
		let section = SecuritySettings::section();

		// Assert
		assert_eq!(section, "security");
	}

	#[rstest]
	fn test_security_settings_default() {
		// Arrange / Act
		let settings = SecuritySettings::default();

		// Assert
		assert!(!settings.secure_ssl_redirect);
		assert!(!settings.session_cookie_secure);
		assert!(!settings.csrf_cookie_secure);
		assert!(settings.append_slash);
		assert!(settings.secure_proxy_ssl_header.is_none());
	}

	#[rstest]
	fn test_security_settings_development_validation_ok() {
		// Arrange
		let settings = SecuritySettings::default();

		// Act
		let result = settings.validate(&Profile::Development);

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_security_settings_production_validation_fails_without_ssl() {
		// Arrange
		let settings = SecuritySettings::default();

		// Act
		let result = settings.validate(&Profile::Production);

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_security_settings_production_validation_ok() {
		// Arrange
		let settings = SecuritySettings {
			secure_ssl_redirect: true,
			session_cookie_secure: true,
			csrf_cookie_secure: true,
			..Default::default()
		};

		// Act
		let result = settings.validate(&Profile::Production);

		// Assert
		assert!(result.is_ok());
	}
}

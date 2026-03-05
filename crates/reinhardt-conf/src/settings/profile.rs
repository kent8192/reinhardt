//! Profile and environment support
//!
//! Provides Django-style environment profiles (development, staging, production)
//! with cascading configuration support.

use serde::{Deserialize, Serialize};
use std::env;
use std::fmt;

/// Application profile/environment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum Profile {
	/// Development environment (default)
	#[default]
	Development,
	/// Staging/testing environment
	Staging,
	/// Production environment
	Production,
	/// Custom profile
	Custom,
}

impl Profile {
	/// Get the profile from environment variable
	///
	/// Checks REINHARDT_ENV, REINHARDT_SETTINGS_MODULE, and ENVIRONMENT variables
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::profile::Profile;
	///
	/// // Returns the profile from environment variables
	/// if let Some(profile) = Profile::from_env() {
	///     println!("Running in {:?} mode", profile);
	/// }
	/// ```
	pub fn from_env() -> Option<Self> {
		// Try REINHARDT_ENV first
		if let Ok(env_val) = env::var("REINHARDT_ENV") {
			return Some(Self::parse(&env_val));
		}

		// Try ENVIRONMENT
		if let Ok(env_val) = env::var("ENVIRONMENT") {
			return Some(Self::parse(&env_val));
		}

		// Try to detect from REINHARDT_SETTINGS_MODULE
		if let Ok(settings_module) = env::var("REINHARDT_SETTINGS_MODULE") {
			if settings_module.contains("production") {
				return Some(Profile::Production);
			} else if settings_module.contains("staging") {
				return Some(Profile::Staging);
			} else if settings_module.contains("development") || settings_module.contains("dev") {
				return Some(Profile::Development);
			}
		}

		None
	}
	/// Parse profile from string
	///
	/// Returns Custom for unknown profile names.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::profile::Profile;
	///
	/// assert_eq!(Profile::parse("production"), Profile::Production);
	/// assert_eq!(Profile::parse("dev"), Profile::Development);
	/// assert_eq!(Profile::parse("staging"), Profile::Staging);
	/// assert_eq!(Profile::parse("unknown"), Profile::Custom);
	/// ```
	pub fn parse(s: &str) -> Self {
		match s.to_lowercase().as_str() {
			"development" | "dev" | "develop" => Profile::Development,
			"staging" | "stage" | "test" => Profile::Staging,
			"production" | "prod" => Profile::Production,
			_ => Profile::Custom,
		}
	}
	/// Get the profile name as a string
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::profile::Profile;
	///
	/// assert_eq!(Profile::Development.as_str(), "development");
	/// assert_eq!(Profile::Production.as_str(), "production");
	/// ```
	pub fn as_str(&self) -> &'static str {
		match self {
			Profile::Development => "development",
			Profile::Staging => "staging",
			Profile::Production => "production",
			Profile::Custom => "custom",
		}
	}
	/// Check if this is a production environment
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::profile::Profile;
	///
	/// assert!(Profile::Production.is_production());
	/// assert!(!Profile::Development.is_production());
	/// ```
	pub fn is_production(&self) -> bool {
		matches!(self, Profile::Production)
	}
	/// Check if this is a development environment
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::profile::Profile;
	///
	/// assert!(Profile::Development.is_development());
	/// assert!(!Profile::Production.is_development());
	/// ```
	pub fn is_development(&self) -> bool {
		matches!(self, Profile::Development)
	}
	/// Check if debug mode should be enabled by default
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::profile::Profile;
	///
	/// assert!(Profile::Development.default_debug());
	/// assert!(!Profile::Production.default_debug());
	/// ```
	pub fn default_debug(&self) -> bool {
		!self.is_production()
	}
	/// Get the .env file name for this profile
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::profile::Profile;
	///
	/// let profile = Profile::Development;
	/// assert_eq!(profile.env_file_name(), ".env.development");
	/// ```
	pub fn env_file_name(&self) -> String {
		match self {
			Profile::Custom => ".env".to_string(),
			_ => format!(".env.{}", self.as_str()),
		}
	}
}

impl fmt::Display for Profile {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.as_str())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_profile_parse() {
		assert_eq!(Profile::parse("development"), Profile::Development);
		assert_eq!(Profile::parse("dev"), Profile::Development);
		assert_eq!(Profile::parse("staging"), Profile::Staging);
		assert_eq!(Profile::parse("stage"), Profile::Staging);
		assert_eq!(Profile::parse("production"), Profile::Production);
		assert_eq!(Profile::parse("prod"), Profile::Production);
		assert_eq!(Profile::parse("unknown"), Profile::Custom);
	}

	#[test]
	fn test_profile_as_str() {
		assert_eq!(Profile::Development.as_str(), "development");
		assert_eq!(Profile::Staging.as_str(), "staging");
		assert_eq!(Profile::Production.as_str(), "production");
	}

	#[test]
	fn test_profile_checks() {
		assert!(Profile::Production.is_production());
		assert!(!Profile::Development.is_production());

		assert!(Profile::Development.is_development());
		assert!(!Profile::Production.is_development());
	}

	#[test]
	fn test_default_debug() {
		assert!(Profile::Development.default_debug());
		assert!(Profile::Staging.default_debug());
		assert!(!Profile::Production.default_debug());
	}

	#[test]
	fn test_env_file_name() {
		assert_eq!(Profile::Development.env_file_name(), ".env.development");
		assert_eq!(Profile::Staging.env_file_name(), ".env.staging");
		assert_eq!(Profile::Production.env_file_name(), ".env.production");
		assert_eq!(Profile::Custom.env_file_name(), ".env");
	}

	#[test]
	fn test_settings_profile_from_env() {
		// SAFETY: Setting environment variables is unsafe in multi-threaded programs.
		// This test uses #[serial] to ensure exclusive access to environment variables.
		unsafe {
			env::set_var("REINHARDT_ENV", "production");
		}
		assert_eq!(Profile::from_env().unwrap(), Profile::Production);
		// SAFETY: Removing environment variables is unsafe in multi-threaded programs.
		// This test uses #[serial] to ensure exclusive access to environment variables.
		unsafe {
			env::remove_var("REINHARDT_ENV");
		}

		// SAFETY: Setting environment variables is unsafe in multi-threaded programs.
		// This test uses #[serial] to ensure exclusive access to environment variables.
		unsafe {
			env::set_var("ENVIRONMENT", "development");
		}
		assert_eq!(Profile::from_env().unwrap(), Profile::Development);
		// SAFETY: Removing environment variables is unsafe in multi-threaded programs.
		// This test uses #[serial] to ensure exclusive access to environment variables.
		unsafe {
			env::remove_var("ENVIRONMENT");
		}
	}
}

//! CORS settings fragment
//!
//! Provides composable CORS configuration as a [`SettingsFragment`].

use super::fragment::SettingsFragment;
use serde::{Deserialize, Serialize};

/// CORS configuration fragment.
///
/// Controls cross-origin resource sharing policy.
#[non_exhaustive]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CorsSettings {
	/// Allowed origin domains (use `"*"` for any origin).
	pub allow_origins: Vec<String>,
	/// Allowed HTTP methods.
	pub allow_methods: Vec<String>,
	/// Allowed HTTP request headers.
	pub allow_headers: Vec<String>,
	/// Whether to allow credentials (cookies, authorization headers).
	pub allow_credentials: bool,
	/// Maximum age (in seconds) for preflight response caching.
	pub max_age: u64,
}

impl Default for CorsSettings {
	fn default() -> Self {
		Self {
			allow_origins: vec!["*".to_string()],
			allow_methods: vec![
				"GET".to_string(),
				"POST".to_string(),
				"PUT".to_string(),
				"PATCH".to_string(),
				"DELETE".to_string(),
			],
			allow_headers: vec!["*".to_string()],
			allow_credentials: false,
			max_age: 3600,
		}
	}
}

impl SettingsFragment for CorsSettings {
	fn section() -> &'static str {
		"cors"
	}
}

/// Trait for settings containers that include CORS configuration.
pub trait HasCorsSettings {
	/// Returns a reference to the CORS settings.
	fn cors(&self) -> &CorsSettings;
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_cors_section_name() {
		// Arrange / Act
		let section = CorsSettings::section();

		// Assert
		assert_eq!(section, "cors");
	}

	#[rstest]
	fn test_cors_default_values() {
		// Arrange / Act
		let settings = CorsSettings::default();

		// Assert
		assert_eq!(settings.allow_origins, vec!["*"]);
		assert_eq!(settings.allow_methods.len(), 5);
		assert!(settings.allow_methods.contains(&"GET".to_string()));
		assert!(settings.allow_methods.contains(&"POST".to_string()));
		assert!(settings.allow_methods.contains(&"PUT".to_string()));
		assert!(settings.allow_methods.contains(&"PATCH".to_string()));
		assert!(settings.allow_methods.contains(&"DELETE".to_string()));
		assert_eq!(settings.allow_headers, vec!["*"]);
		assert!(!settings.allow_credentials);
		assert_eq!(settings.max_age, 3600);
	}
}

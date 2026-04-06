//! Cache settings fragment
//!
//! Provides composable cache configuration as a [`SettingsFragment`](crate::settings::fragment::SettingsFragment).

use reinhardt_core::macros::settings;
use serde::{Deserialize, Serialize};

fn default_backend() -> String {
	"memory".to_string()
}

fn default_timeout() -> u64 {
	300
}

/// Cache configuration fragment.
///
/// Controls the cache backend, location, and default timeout.
#[settings(fragment = true, section = "cache")]
#[non_exhaustive]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CacheSettings {
	/// Cache backend type (e.g., `"memory"`, `"redis"`, `"database"`).
	#[serde(default = "default_backend")]
	pub backend: String,
	/// Backend-specific connection location or URL.
	pub location: Option<String>,
	/// Default cache entry timeout in seconds.
	#[serde(default = "default_timeout")]
	pub timeout: u64,
}

impl Default for CacheSettings {
	fn default() -> Self {
		Self {
			backend: "memory".to_string(),
			location: None,
			timeout: 300,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::settings::fragment::SettingsFragment;
	use rstest::rstest;

	#[rstest]
	fn test_cache_section_name() {
		// Arrange / Act
		let section = CacheSettings::section();

		// Assert
		assert_eq!(section, "cache");
	}

	#[rstest]
	fn test_cache_default_values() {
		// Arrange / Act
		let settings = CacheSettings::default();

		// Assert
		assert_eq!(settings.backend, "memory");
		assert!(settings.location.is_none());
		assert_eq!(settings.timeout, 300);
	}
}

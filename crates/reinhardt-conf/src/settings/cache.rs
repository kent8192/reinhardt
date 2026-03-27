//! Cache settings fragment
//!
//! Provides composable cache configuration as a [`SettingsFragment`].

use super::fragment::{HasSettings, SettingsFragment};
use serde::{Deserialize, Serialize};

/// Cache configuration fragment.
///
/// Controls the cache backend, location, and default timeout.
#[non_exhaustive]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CacheSettings {
	/// Cache backend type (e.g., `"memory"`, `"redis"`, `"database"`).
	pub backend: String,
	/// Backend-specific connection location or URL.
	pub location: Option<String>,
	/// Default cache entry timeout in seconds.
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

impl SettingsFragment for CacheSettings {
	type Accessor = dyn HasCacheSettings;

	fn section() -> &'static str {
		"cache"
	}
}

/// Trait for settings containers that include cache configuration.
pub trait HasCacheSettings {
	/// Returns a reference to the cache settings.
	fn cache(&self) -> &CacheSettings;
}

impl<T: HasSettings<CacheSettings>> HasCacheSettings for T {
	fn cache(&self) -> &CacheSettings {
		self.get_settings()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
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

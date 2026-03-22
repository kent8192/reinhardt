//! Composable settings fragment trait
//!
//! Defines the [`SettingsFragment`] trait that all composable settings units implement.
//! Each fragment maps to a TOML section and can be validated independently.

use super::profile::Profile;
use super::validation::ValidationResult;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;

/// A composable unit of configuration.
///
/// Each fragment maps to a TOML section and can be validated independently.
/// Fragments are composed into a `ProjectSettings` struct using the
/// `#[settings(key: Type | !Type)]` macro.
///
/// # Implementing
///
/// Use `#[settings(fragment = true, section = "...")]` to auto-derive this trait,
/// or implement it manually for custom validation.
pub trait SettingsFragment:
	Clone + Debug + Serialize + DeserializeOwned + Send + Sync + 'static
{
	/// TOML section name (e.g., `"cache"`, `"core"`).
	fn section() -> &'static str;

	/// Validate this fragment against the given profile.
	///
	/// Default implementation: no-op (always valid).
	fn validate(&self, _profile: &Profile) -> ValidationResult {
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::settings::profile::Profile;
	use rstest::rstest;

	#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
	struct TestFragment {
		pub value: String,
	}

	impl SettingsFragment for TestFragment {
		fn section() -> &'static str {
			"test"
		}
	}

	#[rstest]
	fn test_settings_fragment_section() {
		// Arrange
		// (no setup needed)

		// Act
		let section = TestFragment::section();

		// Assert
		assert_eq!(section, "test");
	}

	#[rstest]
	fn test_settings_fragment_validate_default_ok() {
		// Arrange
		let fragment = TestFragment {
			value: "hello".to_string(),
		};
		let profile = Profile::Development;

		// Act
		let result = fragment.validate(&profile);

		// Assert
		assert!(result.is_ok());
	}
}

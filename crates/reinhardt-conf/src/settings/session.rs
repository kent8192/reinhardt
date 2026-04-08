//! Session settings fragment
//!
//! Provides composable session configuration as a [`SettingsFragment`](crate::settings::fragment::SettingsFragment).

use reinhardt_core::macros::settings;
use serde::{Deserialize, Serialize};

fn default_engine() -> String {
	"cookie".to_string()
}

fn default_cookie_name() -> String {
	"sessionid".to_string()
}

fn default_cookie_age() -> u64 {
	1209600
}

fn default_cookie_httponly() -> bool {
	true
}

fn default_cookie_samesite() -> String {
	"lax".to_string()
}

/// Session configuration fragment.
///
/// Controls session storage engine and cookie attributes.
#[settings(fragment = true, section = "session")]
#[non_exhaustive]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionSettings {
	/// Session storage engine (e.g., `"cookie"`, `"database"`, `"redis"`).
	#[serde(default = "default_engine")]
	pub engine: String,
	/// Name of the session cookie.
	#[serde(default = "default_cookie_name")]
	pub cookie_name: String,
	/// Maximum age of the session cookie in seconds.
	#[serde(default = "default_cookie_age")]
	pub cookie_age: u64,
	/// Whether to set the `Secure` flag on the session cookie.
	pub cookie_secure: bool,
	/// Whether to set the `HttpOnly` flag on the session cookie.
	#[serde(default = "default_cookie_httponly")]
	pub cookie_httponly: bool,
	/// `SameSite` attribute for the session cookie (e.g., `"lax"`, `"strict"`, `"none"`).
	#[serde(default = "default_cookie_samesite")]
	pub cookie_samesite: String,
}

impl Default for SessionSettings {
	fn default() -> Self {
		Self {
			engine: "cookie".to_string(),
			cookie_name: "sessionid".to_string(),
			cookie_age: 1209600, // 2 weeks
			cookie_secure: false,
			cookie_httponly: true,
			cookie_samesite: "lax".to_string(),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::settings::fragment::SettingsFragment;
	use rstest::rstest;

	#[rstest]
	fn test_session_section_name() {
		// Arrange / Act
		let section = SessionSettings::section();

		// Assert
		assert_eq!(section, "session");
	}

	#[rstest]
	fn test_session_default_values() {
		// Arrange / Act
		let settings = SessionSettings::default();

		// Assert
		assert_eq!(settings.engine, "cookie");
		assert_eq!(settings.cookie_name, "sessionid");
		assert_eq!(settings.cookie_age, 1209600);
		assert!(!settings.cookie_secure);
		assert!(settings.cookie_httponly);
		assert_eq!(settings.cookie_samesite, "lax");
	}
}

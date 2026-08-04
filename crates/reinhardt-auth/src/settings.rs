//! Settings fragments for authentication backends.
//!
//! These fragments map authentication configuration onto the Reinhardt
//! `#[settings]` macro. Each fragment owns a globally unique section (prefixed
//! with `auth_`) and converts into the matching legacy `XxxConfig` value during
//! the 0.2 compatibility window. New code should prefer the fragments and the
//! `create_*_from_settings` constructors.

#![allow(deprecated)] // Conversions target legacy config types during the compatibility window.

#[cfg(any(feature = "sessions", feature = "jwt", feature = "token"))]
use serde::{Deserialize, Serialize};

// --- session --------------------------------------------------------------

#[cfg(feature = "sessions")]
mod session_settings {
	use super::*;
	use crate::sessions::config::{SameSite, SessionConfig};
	use reinhardt_core::macros::settings;
	use std::time::Duration;

	fn default_cookie_name() -> String {
		"sessionid".to_string()
	}

	fn default_cookie_path() -> String {
		"/".to_string()
	}

	fn default_cookie_secure() -> bool {
		true
	}

	fn default_cookie_httponly() -> bool {
		true
	}

	fn default_cookie_samesite() -> String {
		"lax".to_string()
	}

	/// Parse a SameSite policy string into the legacy [`SameSite`] enum.
	///
	/// Unknown values fall back to the secure default (`Lax`).
	fn parse_samesite(value: &str) -> SameSite {
		match value.to_ascii_lowercase().as_str() {
			"strict" => SameSite::Strict,
			"none" => SameSite::None,
			_ => SameSite::Lax,
		}
	}

	/// Session configuration fragment.
	///
	/// Maps to the `[auth_session]` section and composes with the `#[settings]`
	/// macro. Convert into the deprecated [`SessionConfig`] via [`to_config`].
	///
	/// The session cookie max age is expressed as `cookie_age_secs` (an integer
	/// number of seconds). `None` means a browser-session cookie.
	///
	/// [`to_config`]: SessionSettings::to_config
	#[settings(fragment = true, section = "auth_session")]
	#[derive(Clone, Debug, Serialize, Deserialize)]
	pub struct SessionSettings {
		/// Name of the session cookie.
		#[serde(default = "default_cookie_name")]
		pub cookie_name: String,
		/// Maximum cookie age, in seconds. `None` yields a session cookie.
		#[serde(default)]
		pub cookie_age_secs: Option<u64>,
		/// Cookie path.
		#[serde(default = "default_cookie_path")]
		pub cookie_path: String,
		/// Cookie domain. `None` uses the current domain.
		#[serde(default)]
		pub cookie_domain: Option<String>,
		/// Whether the cookie sets the `Secure` flag (HTTPS only).
		#[serde(default = "default_cookie_secure")]
		pub cookie_secure: bool,
		/// Whether the cookie sets the `HttpOnly` flag.
		#[serde(default = "default_cookie_httponly")]
		pub cookie_httponly: bool,
		/// SameSite policy: `"strict"`, `"lax"`, or `"none"`.
		#[serde(default = "default_cookie_samesite")]
		pub cookie_samesite: String,
		/// Whether to save the session on every request.
		#[serde(default)]
		pub save_every_request: bool,
	}

	impl Default for SessionSettings {
		fn default() -> Self {
			Self {
				cookie_name: default_cookie_name(),
				cookie_age_secs: None,
				cookie_path: default_cookie_path(),
				cookie_domain: None,
				cookie_secure: default_cookie_secure(),
				cookie_httponly: default_cookie_httponly(),
				cookie_samesite: default_cookie_samesite(),
				save_every_request: false,
			}
		}
	}

	impl SessionSettings {
		/// Convert these settings into the deprecated compatibility config.
		///
		/// The legacy [`SessionConfig`] uses private fields, so the conversion
		/// rebuilds it through [`SessionConfig::builder`].
		pub fn to_config(&self) -> SessionConfig {
			let mut builder = SessionConfig::builder()
				.cookie_name(self.cookie_name.clone())
				.cookie_path(self.cookie_path.clone())
				.cookie_secure(self.cookie_secure)
				.cookie_httponly(self.cookie_httponly)
				.cookie_samesite(parse_samesite(&self.cookie_samesite))
				.save_every_request(self.save_every_request);
			if let Some(secs) = self.cookie_age_secs {
				builder = builder.cookie_age(Duration::from_secs(secs));
			}
			if let Some(domain) = &self.cookie_domain {
				builder = builder.cookie_domain(domain.clone());
			}
			builder.build()
		}
	}

	impl From<&SessionSettings> for SessionConfig {
		fn from(settings: &SessionSettings) -> Self {
			settings.to_config()
		}
	}
}

#[cfg(feature = "sessions")]
pub use session_settings::SessionSettings;

// --- jwt session backend --------------------------------------------------

#[cfg(feature = "jwt")]
mod jwt_session_settings {
	use super::*;
	use crate::sessions::backends::jwt::{JwtConfig, JwtSessionBackend, JwtSessionError};
	use jsonwebtoken::Algorithm;
	use reinhardt_conf::settings::secret_types::SecretString;
	use reinhardt_core::macros::settings;

	fn default_algorithm() -> String {
		"HS256".to_string()
	}

	fn default_expiration() -> u64 {
		3600 // 1 hour
	}

	fn default_secret() -> SecretString {
		SecretString::new("")
	}

	/// Parse a JWT algorithm string into the [`Algorithm`] enum.
	///
	/// `jsonwebtoken::Algorithm` is not `serde`-serializable in this version,
	/// so the fragment stores the algorithm as a string and maps it here.
	/// Unknown values fall back to the default `HS256`.
	fn parse_algorithm(value: &str) -> Algorithm {
		match value.to_ascii_uppercase().as_str() {
			"HS384" => Algorithm::HS384,
			"HS512" => Algorithm::HS512,
			_ => Algorithm::HS256,
		}
	}

	/// JWT session backend configuration fragment.
	///
	/// Maps to the `[auth_jwt_session]` section and composes with the
	/// `#[settings]` macro. Convert into the deprecated [`JwtConfig`] via
	/// [`to_config`].
	///
	/// The HMAC secret length is validated when the backend is constructed
	/// (see [`create_jwt_session_backend_from_settings`]), not when the
	/// fragment is parsed, matching the legacy [`JwtSessionBackend::new`]
	/// behavior.
	///
	/// [`to_config`]: JwtSessionSettings::to_config
	#[settings(fragment = true, section = "auth_jwt_session")]
	#[derive(Clone, Debug, Serialize, Deserialize)]
	pub struct JwtSessionSettings {
		/// Secret key used for HMAC signing.
		#[serde(default = "default_secret")]
		pub secret: SecretString,
		/// JWT signing algorithm: `"HS256"`, `"HS384"`, or `"HS512"`.
		#[serde(default = "default_algorithm")]
		pub algorithm: String,
		/// Default token expiration, in seconds.
		#[serde(default = "default_expiration")]
		pub expiration: u64,
		/// Token issuer.
		#[serde(default)]
		pub issuer: Option<String>,
		/// Token audience.
		#[serde(default)]
		pub audience: Option<String>,
	}

	impl Default for JwtSessionSettings {
		fn default() -> Self {
			Self {
				secret: default_secret(),
				algorithm: default_algorithm(),
				expiration: default_expiration(),
				issuer: None,
				audience: None,
			}
		}
	}

	impl JwtSessionSettings {
		/// Convert these settings into the deprecated compatibility config.
		pub fn to_config(&self) -> JwtConfig {
			JwtConfig {
				secret: self.secret.expose_secret().to_owned(),
				algorithm: parse_algorithm(&self.algorithm),
				expiration: self.expiration,
				issuer: self.issuer.clone(),
				audience: self.audience.clone(),
			}
		}
	}

	impl From<&JwtSessionSettings> for JwtConfig {
		fn from(settings: &JwtSessionSettings) -> Self {
			settings.to_config()
		}
	}

	/// Build a [`JwtSessionBackend`] from a [`JwtSessionSettings`] fragment.
	///
	/// Returns an error if the configured HMAC secret is too short for the
	/// selected algorithm, preserving the legacy validation performed by
	/// [`JwtSessionBackend::new`].
	pub fn create_jwt_session_backend_from_settings(
		settings: &JwtSessionSettings,
	) -> Result<JwtSessionBackend, JwtSessionError> {
		JwtSessionBackend::new(settings.to_config())
	}
}

#[cfg(feature = "jwt")]
pub use jwt_session_settings::{JwtSessionSettings, create_jwt_session_backend_from_settings};

// --- token rotation -------------------------------------------------------

#[cfg(feature = "token")]
mod token_rotation_settings {
	use super::*;
	use crate::token_rotation::{AutoTokenRotationManager, TokenRotationConfig};
	use reinhardt_core::macros::settings;

	fn default_rotation_interval() -> i64 {
		3600 // 1 hour
	}

	fn default_grace_period() -> i64 {
		300 // 5 minutes
	}

	fn default_max_active_tokens() -> usize {
		5
	}

	/// Token rotation configuration fragment.
	///
	/// Maps to the `[auth_token_rotation]` section and composes with the
	/// `#[settings]` macro. Convert into the deprecated [`TokenRotationConfig`]
	/// via [`to_config`].
	///
	/// [`to_config`]: TokenRotationSettings::to_config
	#[settings(fragment = true, section = "auth_token_rotation")]
	#[derive(Clone, Debug, Serialize, Deserialize)]
	pub struct TokenRotationSettings {
		/// Interval, in seconds, between rotations.
		#[serde(default = "default_rotation_interval")]
		pub rotation_interval: i64,
		/// Grace period, in seconds, during which the old token stays valid.
		#[serde(default = "default_grace_period")]
		pub grace_period: i64,
		/// Maximum number of active tokens per user.
		#[serde(default = "default_max_active_tokens")]
		pub max_active_tokens: usize,
		/// Whether to rotate the token on every use.
		#[serde(default)]
		pub rotate_on_use: bool,
	}

	impl Default for TokenRotationSettings {
		fn default() -> Self {
			Self {
				rotation_interval: default_rotation_interval(),
				grace_period: default_grace_period(),
				max_active_tokens: default_max_active_tokens(),
				rotate_on_use: false,
			}
		}
	}

	impl TokenRotationSettings {
		/// Convert these settings into the deprecated compatibility config.
		pub fn to_config(&self) -> TokenRotationConfig {
			TokenRotationConfig {
				rotation_interval: self.rotation_interval,
				grace_period: self.grace_period,
				max_active_tokens: self.max_active_tokens,
				rotate_on_use: self.rotate_on_use,
			}
		}
	}

	impl From<&TokenRotationSettings> for TokenRotationConfig {
		fn from(settings: &TokenRotationSettings) -> Self {
			settings.to_config()
		}
	}

	/// Build an [`AutoTokenRotationManager`] from a [`TokenRotationSettings`]
	/// fragment.
	pub fn create_token_rotation_manager_from_settings(
		settings: &TokenRotationSettings,
	) -> AutoTokenRotationManager {
		AutoTokenRotationManager::new(settings.to_config())
	}
}

#[cfg(feature = "token")]
pub use token_rotation_settings::{
	TokenRotationSettings, create_token_rotation_manager_from_settings,
};

#[cfg(test)]
mod tests {
	#[cfg(feature = "sessions")]
	#[test]
	fn session_settings_default_matches_legacy_config() {
		use super::SessionSettings;
		use crate::sessions::config::SessionConfig;

		let from_settings = SessionSettings::default().to_config();
		let legacy = SessionConfig::default();

		assert_eq!(from_settings.cookie_name(), legacy.cookie_name());
		assert_eq!(from_settings.cookie_age(), legacy.cookie_age());
		assert_eq!(from_settings.cookie_path(), legacy.cookie_path());
		assert_eq!(from_settings.cookie_domain(), legacy.cookie_domain());
		assert_eq!(from_settings.cookie_secure(), legacy.cookie_secure());
		assert_eq!(from_settings.cookie_httponly(), legacy.cookie_httponly());
		assert_eq!(from_settings.cookie_samesite(), legacy.cookie_samesite());
		assert_eq!(
			from_settings.save_every_request(),
			legacy.save_every_request()
		);
	}

	#[cfg(feature = "sessions")]
	#[test]
	fn session_settings_round_trips_samesite_and_age() {
		use super::SessionSettings;
		use crate::sessions::config::SameSite;
		use std::time::Duration;

		let settings = SessionSettings {
			cookie_samesite: "strict".to_string(),
			cookie_age_secs: Some(7200),
			cookie_domain: Some("example.com".to_string()),
			..SessionSettings::default()
		};

		let config = settings.to_config();
		assert_eq!(config.cookie_samesite(), SameSite::Strict);
		assert_eq!(config.cookie_age(), Some(Duration::from_secs(7200)));
		assert_eq!(config.cookie_domain(), Some("example.com"));
	}

	#[cfg(feature = "jwt")]
	#[test]
	fn jwt_session_settings_default_matches_legacy_config() {
		use super::JwtSessionSettings;
		use crate::sessions::backends::jwt::JwtConfig;
		use jsonwebtoken::Algorithm;

		let settings = JwtSessionSettings::default();
		let config = settings.to_config();
		// The legacy `JwtConfig::new` default carries the same algorithm,
		// expiration, and empty optional fields.
		let legacy = JwtConfig::new(String::new());

		assert_eq!(config.secret, settings.secret.expose_secret());
		assert_eq!(config.algorithm, Algorithm::HS256);
		assert_eq!(config.algorithm, legacy.algorithm);
		assert_eq!(config.expiration, legacy.expiration);
		assert_eq!(config.issuer, legacy.issuer);
		assert_eq!(config.audience, legacy.audience);
	}

	#[cfg(feature = "jwt")]
	#[test]
	fn jwt_session_settings_redacts_secret_in_debug_and_json() {
		use super::JwtSessionSettings;
		use reinhardt_conf::settings::secret_types::SecretString;

		let raw_secret = "super-secret-signing-key";
		let settings = JwtSessionSettings {
			secret: SecretString::new(raw_secret),
			..JwtSessionSettings::default()
		};

		let debug = format!("{settings:?}");
		let json = serde_json::to_string(&settings).expect("settings should serialize");

		assert_eq!(settings.to_config().secret, raw_secret);
		assert!(!debug.contains(raw_secret));
		assert!(debug.contains("[REDACTED]"));
		assert!(!json.contains(raw_secret));
		assert!(json.contains("[REDACTED]"));
	}

	#[cfg(feature = "jwt")]
	#[test]
	fn jwt_session_settings_marks_secret_in_schema_and_supports_secret_sources() {
		use super::JwtSessionSettings;
		use reinhardt_conf::settings::schema::SettingsNode;

		let settings: JwtSessionSettings = serde_json::from_value(serde_json::json!({
			"secret": { "secret": "super-secret-signing-key" }
		}))
		.expect("secret source should deserialize");
		let mut secret_paths = Vec::new();
		JwtSessionSettings::node_schema().collect_secret_paths(&mut secret_paths);

		assert_eq!(settings.secret.expose_secret(), "super-secret-signing-key");
		assert_eq!(secret_paths.len(), 1);
		assert_eq!(secret_paths[0].to_string(), "secret");
	}

	#[cfg(feature = "token")]
	#[test]
	fn token_rotation_settings_default_matches_legacy_config() {
		use super::TokenRotationSettings;
		use crate::token_rotation::TokenRotationConfig;

		let from_settings = TokenRotationSettings::default().to_config();
		let legacy = TokenRotationConfig::default();

		assert_eq!(from_settings.rotation_interval, legacy.rotation_interval);
		assert_eq!(from_settings.grace_period, legacy.grace_period);
		assert_eq!(from_settings.max_active_tokens, legacy.max_active_tokens);
		assert_eq!(from_settings.rotate_on_use, legacy.rotate_on_use);
	}
}

//! Session configuration management
//!
//! This module provides configuration structures and builders for session management.
//! It allows fine-grained control over session behavior, security, and storage options.
//!
//! ## Features
//!
//! - **SessionConfig**: Main configuration structure for sessions
//! - **SessionConfigBuilder**: Builder pattern for flexible configuration
//! - **SameSite**: Cookie SameSite attribute support
//!
//! ## Example
//!
//! ```rust
//! use reinhardt_auth::sessions::config::{SessionConfig, SameSite};
//! use std::time::Duration;
//!
//! // Using builder pattern
//! let config = SessionConfig::builder()
//!     .cookie_name("my_session")
//!     .cookie_age(Duration::from_secs(7200))
//!     .cookie_secure(true)
//!     .cookie_httponly(true)
//!     .cookie_samesite(SameSite::Strict)
//!     .save_every_request(false)
//!     .build();
//!
//! assert_eq!(config.cookie_name(), "my_session");
//! assert!(config.cookie_secure());
//! ```

use std::time::Duration;

/// SameSite cookie attribute
///
/// Controls when cookies are sent with cross-site requests, providing
/// CSRF protection. This is a critical security setting for session cookies.
///
/// ## Example
///
/// ```rust
/// use reinhardt_auth::sessions::config::SameSite;
///
/// let strict = SameSite::Strict;  // Maximum security
/// let lax = SameSite::Lax;        // Balanced (default)
/// let none = SameSite::None;      // Least restrictive
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SameSite {
	/// Cookies are only sent in a first-party context
	///
	/// Strictest setting. Cookies are never sent with cross-site requests,
	/// providing maximum CSRF protection.
	Strict,

	/// Cookies are sent on top-level navigation and with GET requests
	///
	/// Balanced setting. Provides CSRF protection while allowing cookies
	/// to be sent when users navigate to your site from external links.
	#[default]
	Lax,

	/// Cookies are sent with both first-party and cross-site requests
	///
	/// Least restrictive. Requires `Secure` flag to be set.
	None,
}

impl SameSite {
	/// Convert to cookie string value
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::config::SameSite;
	///
	/// assert_eq!(SameSite::Strict.as_str(), "Strict");
	/// assert_eq!(SameSite::Lax.as_str(), "Lax");
	/// assert_eq!(SameSite::None.as_str(), "None");
	/// ```
	pub fn as_str(&self) -> &'static str {
		match self {
			SameSite::Strict => "Strict",
			SameSite::Lax => "Lax",
			SameSite::None => "None",
		}
	}
}

/// Session configuration
///
/// Configures all aspects of session management including cookie settings,
/// security options, and behavior.
///
/// ## Example
///
/// ```rust
/// use reinhardt_auth::sessions::config::{SessionConfig, SameSite};
/// use std::time::Duration;
///
/// let config = SessionConfig::builder()
///     .cookie_name("sessionid")
///     .cookie_age(Duration::from_secs(3600))
///     .cookie_secure(true)
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct SessionConfig {
	/// Name of the session cookie
	cookie_name: String,
	/// Maximum age for the cookie (None = session cookie)
	cookie_age: Option<Duration>,
	/// Path for the cookie
	cookie_path: String,
	/// Domain for the cookie (None = current domain)
	cookie_domain: Option<String>,
	/// Whether to set the Secure flag (HTTPS only)
	cookie_secure: bool,
	/// Whether to set the HttpOnly flag (no JavaScript access)
	cookie_httponly: bool,
	/// SameSite attribute
	cookie_samesite: SameSite,
	/// Whether to save the session on every request
	save_every_request: bool,
}

impl SessionConfig {
	/// Create a new builder for SessionConfig
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::config::SessionConfig;
	///
	/// let config = SessionConfig::builder()
	///     .cookie_name("my_session")
	///     .build();
	/// ```
	pub fn builder() -> SessionConfigBuilder {
		SessionConfigBuilder::new()
	}

	/// Get the cookie name
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::config::SessionConfig;
	///
	/// let config = SessionConfig::default();
	/// assert_eq!(config.cookie_name(), "sessionid");
	/// ```
	pub fn cookie_name(&self) -> &str {
		&self.cookie_name
	}

	/// Get the cookie age
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::config::SessionConfig;
	/// use std::time::Duration;
	///
	/// let config = SessionConfig::builder()
	///     .cookie_age(Duration::from_secs(3600))
	///     .build();
	///
	/// assert_eq!(config.cookie_age(), Some(Duration::from_secs(3600)));
	/// ```
	pub fn cookie_age(&self) -> Option<Duration> {
		self.cookie_age
	}

	/// Get the cookie path
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::config::SessionConfig;
	///
	/// let config = SessionConfig::default();
	/// assert_eq!(config.cookie_path(), "/");
	/// ```
	pub fn cookie_path(&self) -> &str {
		&self.cookie_path
	}

	/// Get the cookie domain
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::config::SessionConfig;
	///
	/// let config = SessionConfig::builder()
	///     .cookie_domain("example.com")
	///     .build();
	///
	/// assert_eq!(config.cookie_domain(), Some("example.com"));
	/// ```
	pub fn cookie_domain(&self) -> Option<&str> {
		self.cookie_domain.as_deref()
	}

	/// Get the cookie secure flag
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::config::SessionConfig;
	///
	/// let config = SessionConfig::builder()
	///     .cookie_secure(true)
	///     .build();
	///
	/// assert!(config.cookie_secure());
	/// ```
	pub fn cookie_secure(&self) -> bool {
		self.cookie_secure
	}

	/// Get the cookie httponly flag
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::config::SessionConfig;
	///
	/// let config = SessionConfig::default();
	/// assert!(config.cookie_httponly());
	/// ```
	pub fn cookie_httponly(&self) -> bool {
		self.cookie_httponly
	}

	/// Get the cookie SameSite attribute
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::config::{SessionConfig, SameSite};
	///
	/// let config = SessionConfig::default();
	/// assert_eq!(config.cookie_samesite(), SameSite::Lax);
	/// ```
	pub fn cookie_samesite(&self) -> SameSite {
		self.cookie_samesite
	}

	/// Get the save_every_request flag
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::config::SessionConfig;
	///
	/// let config = SessionConfig::default();
	/// assert!(!config.save_every_request());
	/// ```
	pub fn save_every_request(&self) -> bool {
		self.save_every_request
	}
}

impl Default for SessionConfig {
	/// Create default session configuration
	///
	/// Default values:
	/// - cookie_name: "sessionid"
	/// - cookie_age: None (session cookie)
	/// - cookie_path: "/"
	/// - cookie_domain: None
	/// - cookie_secure: true
	/// - cookie_httponly: true
	/// - cookie_samesite: SameSite::Lax
	/// - save_every_request: false
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::config::{SessionConfig, SameSite};
	///
	/// let config = SessionConfig::default();
	/// assert_eq!(config.cookie_name(), "sessionid");
	/// assert_eq!(config.cookie_path(), "/");
	/// assert!(config.cookie_secure());
	/// assert!(config.cookie_httponly());
	/// assert_eq!(config.cookie_samesite(), SameSite::Lax);
	/// ```
	fn default() -> Self {
		Self {
			cookie_name: String::from("sessionid"),
			cookie_age: None,
			cookie_path: String::from("/"),
			cookie_domain: None,
			cookie_secure: true,
			cookie_httponly: true,
			cookie_samesite: SameSite::Lax,
			save_every_request: false,
		}
	}
}

/// Builder for SessionConfig
///
/// Provides a fluent API for constructing SessionConfig instances.
///
/// ## Example
///
/// ```rust
/// use reinhardt_auth::sessions::config::{SessionConfigBuilder, SameSite};
/// use std::time::Duration;
///
/// let config = SessionConfigBuilder::new()
///     .cookie_name("my_session")
///     .cookie_age(Duration::from_secs(7200))
///     .cookie_path("/api")
///     .cookie_domain("example.com")
///     .cookie_secure(true)
///     .cookie_httponly(true)
///     .cookie_samesite(SameSite::Strict)
///     .save_every_request(true)
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct SessionConfigBuilder {
	cookie_name: Option<String>,
	cookie_age: Option<Duration>,
	cookie_path: Option<String>,
	cookie_domain: Option<String>,
	cookie_secure: Option<bool>,
	cookie_httponly: Option<bool>,
	cookie_samesite: Option<SameSite>,
	save_every_request: Option<bool>,
}

impl SessionConfigBuilder {
	/// Create a new builder with default values
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::config::SessionConfigBuilder;
	///
	/// let builder = SessionConfigBuilder::new();
	/// let config = builder.build();
	/// ```
	pub fn new() -> Self {
		Self {
			cookie_name: None,
			cookie_age: None,
			cookie_path: None,
			cookie_domain: None,
			cookie_secure: None,
			cookie_httponly: None,
			cookie_samesite: None,
			save_every_request: None,
		}
	}

	/// Set the cookie name
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::config::SessionConfigBuilder;
	///
	/// let config = SessionConfigBuilder::new()
	///     .cookie_name("my_session")
	///     .build();
	///
	/// assert_eq!(config.cookie_name(), "my_session");
	/// ```
	pub fn cookie_name(mut self, name: impl Into<String>) -> Self {
		self.cookie_name = Some(name.into());
		self
	}

	/// Set the cookie age
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::config::SessionConfigBuilder;
	/// use std::time::Duration;
	///
	/// let config = SessionConfigBuilder::new()
	///     .cookie_age(Duration::from_secs(3600))
	///     .build();
	///
	/// assert_eq!(config.cookie_age(), Some(Duration::from_secs(3600)));
	/// ```
	pub fn cookie_age(mut self, age: Duration) -> Self {
		self.cookie_age = Some(age);
		self
	}

	/// Set the cookie path
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::config::SessionConfigBuilder;
	///
	/// let config = SessionConfigBuilder::new()
	///     .cookie_path("/api")
	///     .build();
	///
	/// assert_eq!(config.cookie_path(), "/api");
	/// ```
	pub fn cookie_path(mut self, path: impl Into<String>) -> Self {
		self.cookie_path = Some(path.into());
		self
	}

	/// Set the cookie domain
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::config::SessionConfigBuilder;
	///
	/// let config = SessionConfigBuilder::new()
	///     .cookie_domain("example.com")
	///     .build();
	///
	/// assert_eq!(config.cookie_domain(), Some("example.com"));
	/// ```
	pub fn cookie_domain(mut self, domain: impl Into<String>) -> Self {
		self.cookie_domain = Some(domain.into());
		self
	}

	/// Set the cookie secure flag
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::config::SessionConfigBuilder;
	///
	/// let config = SessionConfigBuilder::new()
	///     .cookie_secure(true)
	///     .build();
	///
	/// assert!(config.cookie_secure());
	/// ```
	pub fn cookie_secure(mut self, secure: bool) -> Self {
		self.cookie_secure = Some(secure);
		self
	}

	/// Set the cookie httponly flag
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::config::SessionConfigBuilder;
	///
	/// let config = SessionConfigBuilder::new()
	///     .cookie_httponly(false)
	///     .build();
	///
	/// assert!(!config.cookie_httponly());
	/// ```
	pub fn cookie_httponly(mut self, httponly: bool) -> Self {
		self.cookie_httponly = Some(httponly);
		self
	}

	/// Set the cookie SameSite attribute
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::config::{SessionConfigBuilder, SameSite};
	///
	/// let config = SessionConfigBuilder::new()
	///     .cookie_samesite(SameSite::Strict)
	///     .build();
	///
	/// assert_eq!(config.cookie_samesite(), SameSite::Strict);
	/// ```
	pub fn cookie_samesite(mut self, samesite: SameSite) -> Self {
		self.cookie_samesite = Some(samesite);
		self
	}

	/// Set the save_every_request flag
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::config::SessionConfigBuilder;
	///
	/// let config = SessionConfigBuilder::new()
	///     .save_every_request(true)
	///     .build();
	///
	/// assert!(config.save_every_request());
	/// ```
	pub fn save_every_request(mut self, save: bool) -> Self {
		self.save_every_request = Some(save);
		self
	}

	/// Build the SessionConfig
	///
	/// Uses default values for any unset fields.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::config::SessionConfigBuilder;
	///
	/// let config = SessionConfigBuilder::new()
	///     .cookie_name("custom")
	///     .build();
	///
	/// assert_eq!(config.cookie_name(), "custom");
	/// assert_eq!(config.cookie_path(), "/"); // Default value
	/// ```
	pub fn build(self) -> SessionConfig {
		let defaults = SessionConfig::default();

		SessionConfig {
			cookie_name: self.cookie_name.unwrap_or(defaults.cookie_name),
			cookie_age: self.cookie_age.or(defaults.cookie_age),
			cookie_path: self.cookie_path.unwrap_or(defaults.cookie_path),
			cookie_domain: self.cookie_domain.or(defaults.cookie_domain),
			cookie_secure: self.cookie_secure.unwrap_or(defaults.cookie_secure),
			cookie_httponly: self.cookie_httponly.unwrap_or(defaults.cookie_httponly),
			cookie_samesite: self.cookie_samesite.unwrap_or(defaults.cookie_samesite),
			save_every_request: self
				.save_every_request
				.unwrap_or(defaults.save_every_request),
		}
	}
}

impl Default for SessionConfigBuilder {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_samesite_as_str() {
		assert_eq!(SameSite::Strict.as_str(), "Strict");
		assert_eq!(SameSite::Lax.as_str(), "Lax");
		assert_eq!(SameSite::None.as_str(), "None");
	}

	#[test]
	fn test_samesite_default() {
		assert_eq!(SameSite::default(), SameSite::Lax);
	}

	#[test]
	fn test_session_config_default() {
		let config = SessionConfig::default();

		assert_eq!(config.cookie_name(), "sessionid");
		assert_eq!(config.cookie_age(), None);
		assert_eq!(config.cookie_path(), "/");
		assert_eq!(config.cookie_domain(), None);
		assert!(config.cookie_secure());
		assert!(config.cookie_httponly());
		assert_eq!(config.cookie_samesite(), SameSite::Lax);
		assert!(!config.save_every_request());
	}

	#[test]
	fn test_session_config_builder_basic() {
		let config = SessionConfigBuilder::new()
			.cookie_name("test_session")
			.build();

		assert_eq!(config.cookie_name(), "test_session");
		assert_eq!(config.cookie_path(), "/"); // Default
	}

	#[test]
	fn test_session_config_builder_all_fields() {
		let config = SessionConfigBuilder::new()
			.cookie_name("my_session")
			.cookie_age(Duration::from_secs(7200))
			.cookie_path("/api")
			.cookie_domain("example.com")
			.cookie_secure(true)
			.cookie_httponly(false)
			.cookie_samesite(SameSite::Strict)
			.save_every_request(true)
			.build();

		assert_eq!(config.cookie_name(), "my_session");
		assert_eq!(config.cookie_age(), Some(Duration::from_secs(7200)));
		assert_eq!(config.cookie_path(), "/api");
		assert_eq!(config.cookie_domain(), Some("example.com"));
		assert!(config.cookie_secure());
		assert!(!config.cookie_httponly());
		assert_eq!(config.cookie_samesite(), SameSite::Strict);
		assert!(config.save_every_request());
	}

	#[test]
	fn test_session_config_builder_partial() {
		let config = SessionConfigBuilder::new()
			.cookie_name("partial")
			.cookie_secure(true)
			.build();

		assert_eq!(config.cookie_name(), "partial");
		assert!(config.cookie_secure());
		// Other fields should use defaults
		assert_eq!(config.cookie_path(), "/");
		assert!(config.cookie_httponly());
		assert_eq!(config.cookie_samesite(), SameSite::Lax);
	}

	#[test]
	fn test_session_config_builder_default() {
		let builder = SessionConfigBuilder::default();
		let config = builder.build();

		assert_eq!(config.cookie_name(), "sessionid");
		assert_eq!(config.cookie_path(), "/");
	}

	#[test]
	fn test_session_config_builder_from_config_builder() {
		let config = SessionConfig::builder()
			.cookie_name("from_builder")
			.cookie_age(Duration::from_secs(3600))
			.build();

		assert_eq!(config.cookie_name(), "from_builder");
		assert_eq!(config.cookie_age(), Some(Duration::from_secs(3600)));
	}

	#[test]
	fn test_session_config_cookie_age_none() {
		let config = SessionConfigBuilder::new()
			.cookie_name("session_cookie")
			.build();

		assert_eq!(config.cookie_age(), None);
	}

	#[test]
	fn test_session_config_cookie_domain_none() {
		let config = SessionConfigBuilder::new().build();
		assert_eq!(config.cookie_domain(), None);
	}

	#[test]
	fn test_session_config_security_settings() {
		let secure_config = SessionConfigBuilder::new()
			.cookie_secure(true)
			.cookie_httponly(true)
			.cookie_samesite(SameSite::Strict)
			.build();

		assert!(secure_config.cookie_secure());
		assert!(secure_config.cookie_httponly());
		assert_eq!(secure_config.cookie_samesite(), SameSite::Strict);
	}

	#[test]
	fn test_session_config_builder_fluent_api() {
		let config = SessionConfigBuilder::new()
			.cookie_name("fluent")
			.cookie_path("/app")
			.cookie_secure(true)
			.save_every_request(true)
			.build();

		assert_eq!(config.cookie_name(), "fluent");
		assert_eq!(config.cookie_path(), "/app");
		assert!(config.cookie_secure());
		assert!(config.save_every_request());
	}

	#[test]
	fn test_session_config_into_string() {
		let config = SessionConfigBuilder::new()
			.cookie_name("test")
			.cookie_path("/test")
			.cookie_domain("test.com")
			.build();

		assert_eq!(config.cookie_name(), "test");
		assert_eq!(config.cookie_path(), "/test");
		assert_eq!(config.cookie_domain(), Some("test.com"));
	}

	#[test]
	fn test_session_config_multiple_builds() {
		let builder = SessionConfigBuilder::new().cookie_name("shared");

		let config1 = builder.clone().cookie_secure(true).build();
		let config2 = builder.clone().cookie_secure(false).build();

		assert_eq!(config1.cookie_name(), "shared");
		assert!(config1.cookie_secure());

		assert_eq!(config2.cookie_name(), "shared");
		assert!(!config2.cookie_secure());
	}
}

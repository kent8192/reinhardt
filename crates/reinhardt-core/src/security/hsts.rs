//! HSTS (HTTP Strict Transport Security) Support
//!
//! Provides utilities for managing HSTS headers and policies.

/// HSTS configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HstsConfig {
	/// HSTS max-age in seconds
	pub max_age: u64,
	/// Include subdomains in HSTS policy
	pub include_subdomains: bool,
	/// Include preload directive
	pub preload: bool,
}

impl HstsConfig {
	/// Create a new HSTS configuration with the given max-age
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::security::HstsConfig;
	///
	/// let config = HstsConfig::new(31536000); // 1 year
	/// assert_eq!(config.max_age, 31536000);
	/// assert!(!config.include_subdomains);
	/// assert!(!config.preload);
	/// ```
	pub fn new(max_age: u64) -> Self {
		Self {
			max_age,
			include_subdomains: false,
			preload: false,
		}
	}
	/// Set include_subdomains flag
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::security::HstsConfig;
	///
	/// let config = HstsConfig::new(3600).with_subdomains(true);
	/// assert!(config.include_subdomains);
	/// ```
	pub fn with_subdomains(mut self, include: bool) -> Self {
		self.include_subdomains = include;
		self
	}
	/// Set preload flag
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::security::HstsConfig;
	///
	/// let config = HstsConfig::new(63072000).with_preload(true);
	/// assert!(config.preload);
	/// ```
	pub fn with_preload(mut self, preload: bool) -> Self {
		self.preload = preload;
		self
	}
	/// Build HSTS header value
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::security::HstsConfig;
	///
	/// let config = HstsConfig::new(3600);
	/// assert_eq!(config.build_header(), "max-age=3600");
	///
	/// let full_config = HstsConfig::new(63072000)
	///     .with_subdomains(true)
	///     .with_preload(true);
	/// assert_eq!(full_config.build_header(), "max-age=63072000; includeSubDomains; preload");
	/// ```
	pub fn build_header(&self) -> String {
		let mut parts = vec![format!("max-age={}", self.max_age)];

		if self.include_subdomains {
			parts.push("includeSubDomains".to_string());
		}

		if self.preload {
			parts.push("preload".to_string());
		}

		parts.join("; ")
	}
}

impl Default for HstsConfig {
	fn default() -> Self {
		Self {
			max_age: 31536000, // 1 year
			include_subdomains: false,
			preload: false,
		}
	}
}

/// HSTS Middleware
///
/// # Examples
///
/// ```
/// use reinhardt_core::security::hsts::{HstsMiddleware, HstsConfig};
///
/// let config = HstsConfig::new(31536000)
///     .with_subdomains(true)
///     .with_preload(true);
/// let middleware = HstsMiddleware::new(config);
/// assert_eq!(middleware.config().max_age, 31536000);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HstsMiddleware {
	config: HstsConfig,
}

impl HstsMiddleware {
	/// Create a new HSTS middleware
	pub fn new(config: HstsConfig) -> Self {
		Self { config }
	}

	/// Create an HSTS middleware with default configuration
	pub fn default_config() -> Self {
		Self {
			config: HstsConfig::default(),
		}
	}

	/// Get the configuration
	pub fn config(&self) -> &HstsConfig {
		&self.config
	}

	/// Get HSTS header value
	pub fn get_header_value(&self) -> String {
		self.config.build_header()
	}
}

impl Default for HstsMiddleware {
	fn default() -> Self {
		Self::default_config()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_default_hsts_config() {
		let config = HstsConfig::default();
		assert_eq!(config.max_age, 31536000);
		assert!(!config.include_subdomains);
		assert!(!config.preload);
	}

	#[test]
	fn test_build_basic_hsts_header() {
		let config = HstsConfig::new(3600);
		let header = config.build_header();
		assert_eq!(header, "max-age=3600");
	}

	#[test]
	fn test_build_hsts_header_with_subdomains() {
		let config = HstsConfig::new(3600).with_subdomains(true);
		let header = config.build_header();
		assert_eq!(header, "max-age=3600; includeSubDomains");
	}

	#[test]
	fn test_build_hsts_header_with_preload() {
		let config = HstsConfig::new(63072000).with_preload(true);
		let header = config.build_header();
		assert_eq!(header, "max-age=63072000; preload");
	}

	#[test]
	fn test_build_full_hsts_header() {
		let config = HstsConfig::new(63072000)
			.with_subdomains(true)
			.with_preload(true);
		let header = config.build_header();
		assert_eq!(header, "max-age=63072000; includeSubDomains; preload");
	}

	#[test]
	fn test_hsts_config_builder_pattern() {
		let config = HstsConfig::new(31536000)
			.with_subdomains(true)
			.with_preload(false);
		assert_eq!(config.max_age, 31536000);
		assert!(config.include_subdomains);
		assert!(!config.preload);
	}

	#[test]
	fn test_hsts_middleware_creation() {
		let config = HstsConfig::new(3600);
		let middleware = HstsMiddleware::new(config);
		assert_eq!(middleware.config().max_age, 3600);
	}

	#[test]
	fn test_hsts_middleware_default() {
		let middleware = HstsMiddleware::default();
		assert_eq!(middleware.config().max_age, 31536000);
		assert!(!middleware.config().include_subdomains);
		assert!(!middleware.config().preload);
	}

	#[test]
	fn test_hsts_middleware_get_header_value() {
		let config = HstsConfig::new(3600)
			.with_subdomains(true)
			.with_preload(true);
		let middleware = HstsMiddleware::new(config);
		let header = middleware.get_header_value();
		assert_eq!(header, "max-age=3600; includeSubDomains; preload");
	}
}

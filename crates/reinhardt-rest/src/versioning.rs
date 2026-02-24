//! # Reinhardt Versioning
//!
//! API versioning strategies for Reinhardt framework.
//!
//! ## Features
//!
//! - **AcceptHeaderVersioning**: Version from Accept header (e.g., `Accept: application/json; version=1.0`)
//! - **URLPathVersioning**: Version from URL path (e.g., `/v1/users/`)
//! - **NamespaceVersioning**: Version from URL namespace
//! - **HostNameVersioning**: Version from subdomain (e.g., `v1.api.example.com`)
//! - **QueryParameterVersioning**: Version from query parameter (e.g., `?version=1.0`)
//! - **VersioningMiddleware**: Automatic version detection middleware
//!
//! ## Example
//!
//! ```rust
//! use reinhardt_rest::versioning::{BaseVersioning, AcceptHeaderVersioning, QueryParameterVersioning};
//! use reinhardt_rest::versioning::{VersioningMiddleware, RequestVersionExt};
//!
//! // Accept header versioning
//! let accept_versioning = AcceptHeaderVersioning::new()
//!     .with_default_version("1.0")
//!     .with_allowed_versions(vec!["1.0", "2.0"]);
//!
//! // Query parameter versioning
//! let query_versioning = QueryParameterVersioning::new()
//!     .with_version_param("v")
//!     .with_default_version("1.0");
//!
//! // Middleware for automatic version detection
//! let middleware = VersioningMiddleware::new(accept_versioning);
//! ```

pub mod config;
pub mod handler;
pub mod middleware;
pub mod reverse;

use async_trait::async_trait;
pub use config::{VersioningConfig, VersioningManager, VersioningStrategy};
pub use handler::{
	ConfigurableVersionedHandler, SimpleVersionedHandler, VersionResponseBuilder, VersionedHandler,
	VersionedHandlerBuilder, VersionedHandlerWrapper,
};
pub use middleware::{ApiVersion, RequestVersionExt, VersioningMiddleware};
use regex::Regex;
use reinhardt_core::exception::{Error, Result};
use reinhardt_http::Request;
pub use reverse::{
	ApiDocFormat, ApiDocUrlBuilder, UrlReverseManager, VersionedUrlBuilder,
	VersioningStrategy as ReverseVersioningStrategy,
};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum VersioningError {
	#[error("Invalid version in Accept header")]
	InvalidAcceptHeader,

	#[error("Invalid version in URL path")]
	InvalidURLPath,

	#[error("Invalid version in URL namespace")]
	InvalidNamespace,

	#[error("Invalid version in hostname")]
	InvalidHostname,

	#[error("Invalid version in query parameter")]
	InvalidQueryParameter,

	#[error("Version not allowed: {0}")]
	VersionNotAllowed(String),
}

/// Base trait for API versioning strategies
#[async_trait]
pub trait BaseVersioning: Send + Sync {
	/// Determine the API version from the request
	async fn determine_version(&self, request: &Request) -> Result<String>;

	/// Get the default version
	fn default_version(&self) -> Option<&str>;

	/// Get allowed versions
	fn allowed_versions(&self) -> Option<&HashSet<String>>;

	/// Check if a version is allowed
	fn is_allowed_version(&self, version: &str) -> bool {
		if let Some(allowed) = self.allowed_versions() {
			if allowed.is_empty() {
				return true;
			}
			return allowed.contains(version) || (self.default_version() == Some(version));
		}
		true
	}

	/// Get the version parameter name
	fn version_param(&self) -> &str {
		"version"
	}
}

/// Accept header versioning
///
/// Example: `Accept: application/json; version=1.0`
#[derive(Debug, Clone)]
pub struct AcceptHeaderVersioning {
	pub default_version: Option<String>,
	pub allowed_versions: HashSet<String>,
	pub version_param: String,
}

impl AcceptHeaderVersioning {
	/// Create a new AcceptHeaderVersioning instance
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::versioning::{AcceptHeaderVersioning, BaseVersioning};
	///
	/// let versioning = AcceptHeaderVersioning::new();
	/// assert_eq!(versioning.default_version.as_deref(), None);
	/// assert_eq!(versioning.version_param.as_str(), "version");
	/// ```
	pub fn new() -> Self {
		Self {
			default_version: None,
			allowed_versions: HashSet::new(),
			version_param: "version".to_string(),
		}
	}
	/// Set the default version to use when no version is specified
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::versioning::{AcceptHeaderVersioning, BaseVersioning};
	///
	/// let versioning = AcceptHeaderVersioning::new()
	///     .with_default_version("1.0");
	/// assert_eq!(versioning.default_version.as_deref(), Some("1.0"));
	/// ```
	pub fn with_default_version(mut self, version: impl Into<String>) -> Self {
		self.default_version = Some(version.into());
		self
	}
	/// Set the allowed versions
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::versioning::{AcceptHeaderVersioning, BaseVersioning};
	///
	/// let versioning = AcceptHeaderVersioning::new()
	///     .with_allowed_versions(vec!["1.0", "2.0", "3.0"]);
	/// assert!(versioning.is_allowed_version("1.0"));
	/// assert!(versioning.is_allowed_version("2.0"));
	/// assert!(!versioning.is_allowed_version("4.0"));
	/// ```
	pub fn with_allowed_versions(mut self, versions: Vec<impl Into<String>>) -> Self {
		self.allowed_versions = versions.into_iter().map(|v| v.into()).collect();
		self
	}
	/// Set the version parameter name to look for in the Accept header
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::versioning::{AcceptHeaderVersioning, BaseVersioning};
	///
	/// let versioning = AcceptHeaderVersioning::new()
	///     .with_version_param("api-version");
	/// assert_eq!(versioning.version_param.as_str(), "api-version");
	/// ```
	pub fn with_version_param(mut self, param: impl Into<String>) -> Self {
		self.version_param = param.into();
		self
	}
}

impl Default for AcceptHeaderVersioning {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl BaseVersioning for AcceptHeaderVersioning {
	async fn determine_version(&self, request: &Request) -> Result<String> {
		// Parse Accept header for version parameter
		if let Some(accept) = request.headers.get("accept") {
			let accept_str = accept
				.to_str()
				.map_err(|_| Error::Validation(VersioningError::InvalidAcceptHeader.to_string()))?;

			// Parse media type parameters
			if let Some(params_start) = accept_str.find(';') {
				let params = &accept_str[params_start + 1..];
				for param in params.split(';') {
					let param = param.trim();
					if let Some((key, value)) = param.split_once('=')
						&& key.trim() == self.version_param
					{
						let version = value.trim().trim_matches('"');
						if self.is_allowed_version(version) {
							return Ok(version.to_string());
						} else {
							return Err(Error::Validation(
								VersioningError::VersionNotAllowed(version.to_string()).to_string(),
							));
						}
					}
				}
			}
		}

		// Return default version if no version in header
		Ok(self
			.default_version
			.clone()
			.unwrap_or_else(|| "1.0".to_string()))
	}

	fn default_version(&self) -> Option<&str> {
		self.default_version.as_deref()
	}

	fn allowed_versions(&self) -> Option<&HashSet<String>> {
		Some(&self.allowed_versions)
	}

	fn version_param(&self) -> &str {
		&self.version_param
	}
}

/// URL path versioning
///
/// Example: `/v1/users/` or `/api/v2/users/`
#[derive(Debug, Clone)]
pub struct URLPathVersioning {
	pub default_version: Option<String>,
	pub allowed_versions: HashSet<String>,
	pub version_param: String,
	pub path_regex: Regex,
}

impl URLPathVersioning {
	/// Create a new URLPathVersioning instance
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::versioning::{URLPathVersioning, BaseVersioning};
	///
	/// let versioning = URLPathVersioning::new();
	/// assert_eq!(versioning.default_version.as_deref(), None);
	/// ```
	pub fn new() -> Self {
		Self {
			default_version: None,
			allowed_versions: HashSet::new(),
			version_param: "version".to_string(),
			path_regex: Regex::new(r"/v?(\d+\.?\d*)").unwrap(),
		}
	}
	/// Set the default version to use when no version is found in the path
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::versioning::{URLPathVersioning, BaseVersioning};
	///
	/// let versioning = URLPathVersioning::new()
	///     .with_default_version("1.0");
	/// assert_eq!(versioning.default_version.as_deref(), Some("1.0"));
	/// ```
	pub fn with_default_version(mut self, version: impl Into<String>) -> Self {
		self.default_version = Some(version.into());
		self
	}
	/// Set the allowed versions
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::versioning::{URLPathVersioning, BaseVersioning};
	///
	/// let versioning = URLPathVersioning::new()
	///     .with_allowed_versions(vec!["1", "2", "3"]);
	/// assert!(versioning.is_allowed_version("1"));
	/// assert!(!versioning.is_allowed_version("99"));
	/// ```
	pub fn with_allowed_versions(mut self, versions: Vec<impl Into<String>>) -> Self {
		self.allowed_versions = versions.into_iter().map(|v| v.into()).collect();
		self
	}
	/// Set the version parameter name (for trait compatibility)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::versioning::{URLPathVersioning, BaseVersioning};
	///
	/// let versioning = URLPathVersioning::new()
	///     .with_version_param("v");
	/// assert_eq!(versioning.version_param.as_str(), "v");
	/// ```
	pub fn with_version_param(mut self, param: impl Into<String>) -> Self {
		self.version_param = param.into();
		self
	}
	/// Set a custom regex pattern for extracting version from path
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::versioning::URLPathVersioning;
	/// use regex::Regex;
	///
	/// let custom_regex = Regex::new(r"/api/v(\d+)").unwrap();
	/// let versioning = URLPathVersioning::new()
	///     .with_path_regex(custom_regex);
	// The versioning will now match paths like /api/v1, /api/v2, etc.
	/// ```
	pub fn with_path_regex(mut self, regex: Regex) -> Self {
		self.path_regex = regex;
		self
	}

	/// Set a custom pattern for extracting version from path (for configuration compatibility)
	///
	/// This converts a pattern like "/v{version}/" into a regex pattern.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::versioning::URLPathVersioning;
	///
	/// let versioning = URLPathVersioning::new()
	///     .with_pattern("/v{version}/");
	// The versioning will now match paths like /v1/, /v2/, etc.
	/// ```
	pub fn with_pattern(mut self, pattern: &str) -> Self {
		// Convert pattern like "/v{version}/" to regex "/v?([^/]+)"
		let regex_pattern = pattern.replace("{version}", "([^/]+)");
		if let Ok(regex) = Regex::new(&regex_pattern) {
			self.path_regex = regex;
		}
		self
	}
}

impl Default for URLPathVersioning {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl BaseVersioning for URLPathVersioning {
	async fn determine_version(&self, request: &Request) -> Result<String> {
		let path = request.uri.path();

		// Try to extract version from path using regex
		if let Some(captures) = self.path_regex.captures(path)
			&& let Some(version_match) = captures.get(1)
		{
			let version = version_match.as_str();
			if self.is_allowed_version(version) {
				return Ok(version.to_string());
			} else {
				return Err(Error::Validation(
					VersioningError::VersionNotAllowed(version.to_string()).to_string(),
				));
			}
		}

		// Return default version if no version in path
		Ok(self
			.default_version
			.clone()
			.unwrap_or_else(|| "1.0".to_string()))
	}

	fn default_version(&self) -> Option<&str> {
		self.default_version.as_deref()
	}

	fn allowed_versions(&self) -> Option<&HashSet<String>> {
		Some(&self.allowed_versions)
	}

	fn version_param(&self) -> &str {
		&self.version_param
	}
}

/// Hostname versioning
///
/// Example: `v1.api.example.com` or `api-v2.example.com`
#[derive(Debug, Clone)]
pub struct HostNameVersioning {
	pub default_version: Option<String>,
	pub allowed_versions: HashSet<String>,
	pub hostname_regex: Regex,
	/// Maps specific hostnames to their API versions.
	/// Takes precedence over regex extraction.
	pub hostname_to_version: HashMap<String, String>,
}

impl HostNameVersioning {
	/// Create a new HostNameVersioning instance
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::versioning::HostNameVersioning;
	///
	/// let versioning = HostNameVersioning::new();
	/// assert_eq!(versioning.default_version.as_deref(), None);
	/// ```
	pub fn new() -> Self {
		Self {
			default_version: None,
			allowed_versions: HashSet::new(),
			hostname_regex: Regex::new(r"^([a-zA-Z0-9]+)\.").unwrap(),
			hostname_to_version: HashMap::new(),
		}
	}
	/// Set the default version to use when no version is found in hostname
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::versioning::HostNameVersioning;
	///
	/// let versioning = HostNameVersioning::new()
	///     .with_default_version("1.0");
	/// assert_eq!(versioning.default_version.as_deref(), Some("1.0"));
	/// ```
	pub fn with_default_version(mut self, version: impl Into<String>) -> Self {
		self.default_version = Some(version.into());
		self
	}
	/// Set the allowed versions
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::versioning::{HostNameVersioning, BaseVersioning};
	///
	/// let versioning = HostNameVersioning::new()
	///     .with_allowed_versions(vec!["v1", "v2", "v3"]);
	/// assert!(versioning.is_allowed_version("v1"));
	/// assert!(!versioning.is_allowed_version("v99"));
	/// ```
	pub fn with_allowed_versions(mut self, versions: Vec<impl Into<String>>) -> Self {
		self.allowed_versions = versions.into_iter().map(|v| v.into()).collect();
		self
	}
	/// Set a custom regex pattern for extracting version from hostname
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::versioning::HostNameVersioning;
	/// use regex::Regex;
	///
	/// let custom_regex = Regex::new(r"^v(\d+)-api\.").unwrap();
	/// let versioning = HostNameVersioning::new()
	///     .with_hostname_regex(custom_regex);
	// The versioning will now match hostnames like v1-api.example.com
	/// ```
	pub fn with_hostname_regex(mut self, regex: Regex) -> Self {
		self.hostname_regex = regex;
		self
	}

	/// Set a host format pattern (for configuration compatibility)
	///
	/// This converts a host format like "{version}.api.example.com" into a regex pattern.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::versioning::HostNameVersioning;
	///
	/// let versioning = HostNameVersioning::new()
	///     .with_host_format("{version}.api.example.com");
	// The versioning will match hostnames like v1.api.example.com
	/// ```
	pub fn with_host_format(mut self, format: &str) -> Self {
		// Convert format like "{version}.api.example.com" to regex "^([^.]+)\.api\.example\.com"
		let pattern = format.replace("{version}", "([^.]+)").replace(".", "\\.");
		let pattern = format!("^{}", pattern);
		if let Ok(regex) = Regex::new(&pattern) {
			self.hostname_regex = regex;
		}
		self
	}

	/// Set hostname patterns for version mapping (for configuration compatibility)
	///
	/// This allows mapping specific hostnames to their API versions.
	/// The hostname mapping takes precedence over regex extraction when determining version.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::versioning::HostNameVersioning;
	///
	/// let versioning = HostNameVersioning::new()
	///     .with_hostname_pattern("v1", "v1.api.example.com")
	///     .with_hostname_pattern("v2", "v2.api.example.com");
	/// // Request to v1.api.example.com will resolve to version "v1"
	/// // Request to v2.api.example.com will resolve to version "v2"
	/// ```
	pub fn with_hostname_pattern(mut self, version: &str, hostname: &str) -> Self {
		self.allowed_versions.insert(version.to_string());
		self.hostname_to_version
			.insert(hostname.to_string(), version.to_string());
		self
	}
}

impl Default for HostNameVersioning {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl BaseVersioning for HostNameVersioning {
	async fn determine_version(&self, request: &Request) -> Result<String> {
		// Extract hostname from request
		if let Some(host) = request.headers.get("host") {
			let host_str = host
				.to_str()
				.map_err(|_| Error::Validation(VersioningError::InvalidHostname.to_string()))?;

			// Remove port if present
			let hostname = host_str.split(':').next().unwrap_or(host_str);

			// Priority 1: Check explicit hostname→version mapping
			if let Some(version) = self.hostname_to_version.get(hostname)
				&& self.is_allowed_version(version)
			{
				return Ok(version.clone());
			}

			// Priority 2: Try to extract version from hostname using regex
			if let Some(captures) = self.hostname_regex.captures(hostname)
				&& let Some(version_match) = captures.get(1)
			{
				let version = version_match.as_str();
				if self.is_allowed_version(version) {
					return Ok(version.to_string());
				}
			}
		}

		// Return default version if no version in hostname
		Ok(self
			.default_version
			.clone()
			.unwrap_or_else(|| "1.0".to_string()))
	}

	fn default_version(&self) -> Option<&str> {
		self.default_version.as_deref()
	}

	fn allowed_versions(&self) -> Option<&HashSet<String>> {
		Some(&self.allowed_versions)
	}
}

/// Query parameter versioning
///
/// Example: `/users/?version=1.0` or `/users/?v=2.0`
#[derive(Debug, Clone)]
pub struct QueryParameterVersioning {
	pub default_version: Option<String>,
	pub allowed_versions: HashSet<String>,
	pub version_param: String,
}

impl QueryParameterVersioning {
	/// Create a new QueryParameterVersioning instance
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::versioning::QueryParameterVersioning;
	///
	/// let versioning = QueryParameterVersioning::new();
	/// assert_eq!(versioning.default_version.as_deref(), None);
	/// assert_eq!(versioning.version_param.as_str(), "version");
	/// ```
	pub fn new() -> Self {
		Self {
			default_version: None,
			allowed_versions: HashSet::new(),
			version_param: "version".to_string(),
		}
	}
	/// Set the default version to use when no version is in query parameters
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::versioning::QueryParameterVersioning;
	///
	/// let versioning = QueryParameterVersioning::new()
	///     .with_default_version("1.0");
	/// assert_eq!(versioning.default_version.as_deref(), Some("1.0"));
	/// ```
	pub fn with_default_version(mut self, version: impl Into<String>) -> Self {
		self.default_version = Some(version.into());
		self
	}
	/// Set the allowed versions
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::versioning::{QueryParameterVersioning, BaseVersioning};
	///
	/// let versioning = QueryParameterVersioning::new()
	///     .with_allowed_versions(vec!["1.0", "2.0", "3.0"]);
	/// assert!(versioning.is_allowed_version("1.0"));
	/// assert!(!versioning.is_allowed_version("4.0"));
	/// ```
	pub fn with_allowed_versions(mut self, versions: Vec<impl Into<String>>) -> Self {
		self.allowed_versions = versions.into_iter().map(|v| v.into()).collect();
		self
	}
	/// Set the query parameter name to use for version detection
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::versioning::QueryParameterVersioning;
	///
	/// let versioning = QueryParameterVersioning::new()
	///     .with_version_param("v");
	/// assert_eq!(versioning.version_param.as_str(), "v");
	// This will now look for ?v=1.0 instead of ?version=1.0
	/// ```
	pub fn with_version_param(mut self, param: impl Into<String>) -> Self {
		self.version_param = param.into();
		self
	}
}

impl Default for QueryParameterVersioning {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl BaseVersioning for QueryParameterVersioning {
	async fn determine_version(&self, request: &Request) -> Result<String> {
		// Parse query string for version parameter
		if let Some(query) = request.uri.query() {
			for param in query.split('&') {
				if let Some((key, value)) = param.split_once('=')
					&& key == self.version_param
				{
					if self.is_allowed_version(value) {
						return Ok(value.to_string());
					} else {
						return Err(Error::Validation(
							VersioningError::VersionNotAllowed(value.to_string()).to_string(),
						));
					}
				}
			}
		}

		// Return default version if no version in query
		Ok(self
			.default_version
			.clone()
			.unwrap_or_else(|| "1.0".to_string()))
	}

	fn default_version(&self) -> Option<&str> {
		self.default_version.as_deref()
	}

	fn allowed_versions(&self) -> Option<&HashSet<String>> {
		Some(&self.allowed_versions)
	}

	fn version_param(&self) -> &str {
		&self.version_param
	}
}

/// Namespace versioning (URL namespace-based)
///
/// Extracts version from URL namespace patterns (e.g., /v1/, /v2/)
/// Now fully implemented with router namespace support
#[derive(Debug)]
pub struct NamespaceVersioning {
	pub default_version: Option<String>,
	pub allowed_versions: HashSet<String>,
	/// Pattern for extracting version from namespace (e.g., "/v{version}/")
	pub pattern: String,
	/// Namespace prefix (e.g., "api")
	pub namespace_prefix: Option<String>,
	/// Cached compiled regex for version extraction
	compiled_regex: OnceLock<Option<Regex>>,
}

impl Clone for NamespaceVersioning {
	fn clone(&self) -> Self {
		Self {
			default_version: self.default_version.clone(),
			allowed_versions: self.allowed_versions.clone(),
			pattern: self.pattern.clone(),
			namespace_prefix: self.namespace_prefix.clone(),
			// Reset compiled_regex so it will be recompiled on first use
			compiled_regex: OnceLock::new(),
		}
	}
}

impl NamespaceVersioning {
	/// Create a new NamespaceVersioning instance
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::versioning::NamespaceVersioning;
	///
	/// let versioning = NamespaceVersioning::new();
	/// assert_eq!(versioning.default_version.as_deref(), None);
	/// assert_eq!(versioning.pattern, "/v{version}/");
	/// ```
	pub fn new() -> Self {
		Self {
			default_version: None,
			allowed_versions: HashSet::new(),
			pattern: "/v{version}/".to_string(),
			namespace_prefix: None,
			compiled_regex: OnceLock::new(),
		}
	}
	/// Set the default version to use when no version is found in namespace
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::versioning::NamespaceVersioning;
	///
	/// let versioning = NamespaceVersioning::new()
	///     .with_default_version("1.0");
	/// assert_eq!(versioning.default_version.as_deref(), Some("1.0"));
	/// ```
	pub fn with_default_version(mut self, version: impl Into<String>) -> Self {
		self.default_version = Some(version.into());
		self
	}
	/// Set the allowed versions
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::versioning::{NamespaceVersioning, BaseVersioning};
	///
	/// let versioning = NamespaceVersioning::new()
	///     .with_allowed_versions(vec!["1", "1.0", "2", "2.0"]);
	/// assert!(versioning.is_allowed_version("1"));
	/// assert!(versioning.is_allowed_version("2.0"));
	/// assert!(!versioning.is_allowed_version("99"));
	/// ```
	pub fn with_allowed_versions(mut self, versions: Vec<impl Into<String>>) -> Self {
		self.allowed_versions = versions.into_iter().map(|v| v.into()).collect();
		self
	}

	/// Set the namespace prefix (e.g., "api")
	///
	/// This prefix is used when constructing full namespace patterns for version detection.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::versioning::NamespaceVersioning;
	///
	/// let versioning = NamespaceVersioning::new()
	///     .with_namespace_prefix("api");
	/// assert_eq!(versioning.namespace_prefix, Some("api".to_string()));
	/// ```
	pub fn with_namespace_prefix(mut self, prefix: &str) -> Self {
		self.namespace_prefix = Some(prefix.to_string());
		self
	}

	/// Set a custom pattern for extracting version from namespace
	///
	/// This converts a pattern like "/v{version}/" into a regex pattern for matching
	/// namespaces like /v1/, /v2/, etc.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::versioning::NamespaceVersioning;
	///
	/// let versioning = NamespaceVersioning::new()
	///     .with_pattern("/api/v{version}/");
	/// assert_eq!(versioning.pattern, "/api/v{version}/");
	/// ```
	pub fn with_pattern(mut self, pattern: &str) -> Self {
		self.pattern = pattern.to_string();
		// Reset cached regex since the pattern changed
		self.compiled_regex = OnceLock::new();
		self
	}
}

impl Default for NamespaceVersioning {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl BaseVersioning for NamespaceVersioning {
	async fn determine_version(&self, request: &Request) -> Result<String> {
		let path = request.uri.path();

		// Use the configured pattern to extract version
		if let Some(version) = self.extract_version_from_path(path)
			&& self.is_allowed_version(&version)
		{
			return Ok(version);
		}

		// Fallback to default version
		Ok(self
			.default_version
			.clone()
			.unwrap_or_else(|| "1.0".to_string()))
	}

	fn default_version(&self) -> Option<&str> {
		self.default_version.as_deref()
	}

	fn allowed_versions(&self) -> Option<&HashSet<String>> {
		Some(&self.allowed_versions)
	}
}

impl NamespaceVersioning {
	/// Get or compile the regex for version extraction from the configured pattern
	fn get_compiled_regex(&self) -> Option<&Regex> {
		self.compiled_regex
			.get_or_init(|| {
				let regex_pattern = self
					.pattern
					.replace("{version}", r"([^/]+)")
					.replace("/", r"\/");
				let full_pattern = format!("^{}", regex_pattern);
				regex::Regex::new(&full_pattern).ok()
			})
			.as_ref()
	}

	/// Extract version from a path using the configured pattern
	fn extract_version_from_path(&self, path: &str) -> Option<String> {
		if let Some(regex) = self.get_compiled_regex()
			&& let Some(captures) = regex.captures(path)
			&& let Some(version_match) = captures.get(1)
		{
			return Some(version_match.as_str().to_string());
		}
		None
	}

	/// Check if a version is allowed
	fn is_allowed_version(&self, version: &str) -> bool {
		self.allowed_versions.is_empty() || self.allowed_versions.contains(version)
	}

	/// Extract version from a router's namespace pattern
	/// This method integrates with reinhardt-routers for namespace-based versioning
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_rest::versioning::NamespaceVersioning;
	/// use reinhardt_urls::routers::DefaultRouter;
	///
	/// let versioning = NamespaceVersioning::new()
	///     .with_pattern("/v{version}/")
	///     .with_allowed_versions(vec!["1", "2"]);
	///
	/// let router = DefaultRouter::new();
	/// let version = versioning.extract_version_from_router(&router, "/v1/users/");
	/// assert_eq!(version, Some("1".to_string()));
	/// ```
	// Router integration disabled due to circular dependency (reinhardt-urls ↔ reinhardt-rest)
	// Use extract_version_from_path() directly instead
	#[allow(dead_code)]
	fn extract_version_from_router_stub(&self, _router: &(), path: &str) -> Option<String> {
		self.extract_version_from_path(path)
	}

	/// Get available versions from a router's registered routes
	/// This discovers all versions that are currently registered in the router
	///
	/// # Examples
	///
	/// ```ignore
	/// // This example is disabled because router integration is disabled
	/// // due to circular dependency (reinhardt-urls ↔ reinhardt-rest)
	/// use reinhardt_rest::versioning::NamespaceVersioning;
	/// use reinhardt_urls::routers::{DefaultRouter, Router, path};
	/// use reinhardt_http::Handler;
	/// use std::sync::Arc;
	///
	/// # use async_trait::async_trait;
	/// # use reinhardt_http::{Request, Response, Result};
	/// # struct DummyHandler;
	/// # #[async_trait]
	/// # impl Handler for DummyHandler {
	/// #     async fn handle(&self, _req: Request) -> Result<Response> {
	/// #         Ok(Response::ok())
	/// #     }
	/// # }
	/// let versioning = NamespaceVersioning::new()
	///     .with_pattern("/v{version}/");
	///
	/// let mut router = DefaultRouter::new();
	/// let handler = Arc::new(DummyHandler);
	/// router.add_route(path("/v1/users/", handler.clone()).with_namespace("v1"));
	/// router.add_route(path("/v2/users/", handler).with_namespace("v2"));
	///
	/// let versions = versioning.get_available_versions_from_router(&router);
	/// assert!(versions.contains(&"1".to_string()));
	/// assert!(versions.contains(&"2".to_string()));
	/// ```
	// Router integration disabled due to circular dependency (reinhardt-urls ↔ reinhardt-rest)
	#[allow(dead_code)]
	fn get_available_versions_from_router_stub(&self, _router: &()) -> Vec<String> {
		Vec::new()
	}
}

#[cfg(test)]
pub mod test_utils {
	use bytes::Bytes;
	use hyper::header::HeaderName;
	use hyper::{HeaderMap, Method, Uri, Version};
	use reinhardt_http::Request;

	pub fn create_test_request(uri: &str, headers: Vec<(String, String)>) -> Request {
		let uri = uri.parse::<Uri>().unwrap();
		let mut header_map = HeaderMap::new();
		for (key, value) in headers {
			let header_name: HeaderName = key.parse().unwrap();
			header_map.insert(header_name, value.parse().unwrap());
		}

		Request::builder()
			.method(Method::GET)
			.uri(uri)
			.version(Version::HTTP_11)
			.headers(header_map)
			.body(Bytes::new())
			.build()
			.unwrap()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use test_utils::create_test_request;

	#[tokio::test]
	async fn test_accept_header_versioning() {
		let versioning = AcceptHeaderVersioning::new()
			.with_default_version("1.0")
			.with_allowed_versions(vec!["1.0", "2.0"]);

		// Test with version in Accept header
		let request = create_test_request(
			"/users/",
			vec![(
				"accept".to_string(),
				"application/json; version=2.0".to_string(),
			)],
		);
		let version = versioning.determine_version(&request).await.unwrap();
		assert_eq!(version, "2.0");

		// Test without version (should return default)
		let request = create_test_request(
			"/users/",
			vec![("accept".to_string(), "application/json".to_string())],
		);
		let version = versioning.determine_version(&request).await.unwrap();
		assert_eq!(version, "1.0");
	}

	#[tokio::test]
	async fn test_url_path_versioning() {
		let versioning = URLPathVersioning::new()
			.with_default_version("1.0")
			.with_allowed_versions(vec!["1.0", "2.0", "2"]);

		// Test with version in path
		let request = create_test_request("/v2/users/", vec![]);
		let version = versioning.determine_version(&request).await.unwrap();
		assert_eq!(version, "2");

		// Test without version (should return default)
		let request = create_test_request("/users/", vec![]);
		let version = versioning.determine_version(&request).await.unwrap();
		assert_eq!(version, "1.0");
	}

	#[tokio::test]
	async fn test_hostname_versioning() {
		let versioning = HostNameVersioning::new()
			.with_default_version("1.0")
			.with_allowed_versions(vec!["v1", "v2"]);

		// Test with version in hostname
		let request = create_test_request(
			"/users/",
			vec![("host".to_string(), "v2.api.example.com".to_string())],
		);
		let version = versioning.determine_version(&request).await.unwrap();
		assert_eq!(version, "v2");

		// Test without version (should return default)
		let request = create_test_request(
			"/users/",
			vec![("host".to_string(), "api.example.com".to_string())],
		);
		let version = versioning.determine_version(&request).await.unwrap();
		assert_eq!(version, "1.0");
	}

	#[tokio::test]
	async fn test_query_parameter_versioning() {
		let versioning = QueryParameterVersioning::new()
			.with_default_version("1.0")
			.with_allowed_versions(vec!["1.0", "2.0"]);

		// Test with version in query parameter
		let request = create_test_request("/users/?version=2.0", vec![]);
		let version = versioning.determine_version(&request).await.unwrap();
		assert_eq!(version, "2.0");

		// Test without version (should return default)
		let request = create_test_request("/users/", vec![]);
		let version = versioning.determine_version(&request).await.unwrap();
		assert_eq!(version, "1.0");
	}

	#[tokio::test]
	async fn test_namespace_versioning() {
		let versioning = NamespaceVersioning::new()
			.with_default_version("1.0")
			.with_allowed_versions(vec!["1", "1.0", "2", "2.0", "3.0"]);

		// Test with version in namespace (v1 format)
		let request = create_test_request("/v1/users/", vec![]);
		let version = versioning.determine_version(&request).await.unwrap();
		assert_eq!(version, "1");

		// Test with version in namespace (v2.0 format)
		let request = create_test_request("/v2.0/users/", vec![]);
		let version = versioning.determine_version(&request).await.unwrap();
		assert_eq!(version, "2.0");

		// Test without version (should return default)
		let request = create_test_request("/users/", vec![]);
		let version = versioning.determine_version(&request).await.unwrap();
		assert_eq!(version, "1.0");

		// Test with non-version namespace
		let request = create_test_request("/api/users/", vec![]);
		let version = versioning.determine_version(&request).await.unwrap();
		assert_eq!(version, "1.0");
	}

	#[tokio::test]
	async fn test_namespace_versioning_with_custom_pattern() {
		let versioning = NamespaceVersioning::new()
			.with_default_version("1.0")
			.with_pattern("/api/v{version}/")
			.with_allowed_versions(vec!["1", "2"]);

		// Test with custom pattern
		let request = create_test_request("/api/v1/users/", vec![]);
		let version = versioning.determine_version(&request).await.unwrap();
		assert_eq!(version, "1");

		// Test with different version
		let request = create_test_request("/api/v2/users/", vec![]);
		let version = versioning.determine_version(&request).await.unwrap();
		assert_eq!(version, "2");

		// Test with old pattern (should not match)
		let request = create_test_request("/v1/users/", vec![]);
		let version = versioning.determine_version(&request).await.unwrap();
		assert_eq!(version, "1.0"); // Falls back to default
	}

	// Note: Router integration test removed to avoid circular dependency with reinhardt-urls.
	// Router integration tests should be placed in /tests/integration crate.
}

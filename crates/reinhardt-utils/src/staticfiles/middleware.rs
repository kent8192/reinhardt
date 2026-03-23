//! Static files middleware for serving WASM builds and static assets.
//!
//! This middleware intercepts requests and serves static files from a configured directory.
//! It supports SPA (Single Page Application) mode for WASM frontend applications.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use reinhardt_core::exception::Result;
use reinhardt_http::{Handler, Middleware};
use reinhardt_http::{Request, Response};

use super::caching::CacheControlConfig;
use super::handler::{StaticError, StaticFileHandler};

/// Configuration for the static files middleware.
#[derive(Debug, Clone)]
pub struct StaticFilesConfig {
	/// Root directory for static files
	pub root_dir: PathBuf,
	/// URL path prefix (e.g., "/static/")
	pub url_prefix: String,
	/// Enable SPA mode - fallback to index.html for 404s
	pub spa_mode: bool,
	/// Index files to serve for directories
	pub index_files: Vec<String>,
	/// Explicit path to the SPA fallback index file.
	///
	/// Can be outside `root_dir` (e.g., project root). When set,
	/// takes priority over `index_files` for SPA fallback.
	pub index_file: Option<PathBuf>,
	/// File extensions to serve (empty = all)
	pub allowed_extensions: Vec<String>,
	/// Path prefixes to exclude from SPA fallback (e.g., ["/api/", "/docs"])
	pub excluded_prefixes: Vec<String>,
	/// Cache control configuration for static file responses
	pub cache_config: CacheControlConfig,
}

impl Default for StaticFilesConfig {
	fn default() -> Self {
		Self {
			root_dir: PathBuf::from("dist"),
			url_prefix: "/".to_string(),
			spa_mode: true,
			index_files: vec!["index.html".to_string()],
			index_file: None,
			allowed_extensions: vec![],
			excluded_prefixes: vec!["/api/".to_string()],
			cache_config: CacheControlConfig::new(),
		}
	}
}

impl StaticFilesConfig {
	/// Create a new configuration with the given root directory.
	pub fn new(root_dir: impl Into<PathBuf>) -> Self {
		Self {
			root_dir: root_dir.into(),
			..Default::default()
		}
	}

	/// Set the URL prefix for static files.
	pub fn url_prefix(mut self, prefix: impl Into<String>) -> Self {
		self.url_prefix = prefix.into();
		self
	}

	/// Enable or disable SPA mode.
	pub fn spa_mode(mut self, enabled: bool) -> Self {
		self.spa_mode = enabled;
		self
	}

	/// Set custom index files.
	pub fn index_files(mut self, files: Vec<String>) -> Self {
		self.index_files = files;
		self
	}

	/// Set a specific index file path for SPA fallback.
	///
	/// This path can be outside `root_dir` (e.g., project root).
	/// When set, this takes priority over `index_files` for SPA fallback.
	pub fn index_file(mut self, path: impl Into<PathBuf>) -> Self {
		self.index_file = Some(path.into());
		self
	}

	/// Set allowed file extensions.
	pub fn allowed_extensions(mut self, extensions: Vec<String>) -> Self {
		self.allowed_extensions = extensions;
		self
	}

	/// Set path prefixes to exclude from SPA fallback.
	pub fn excluded_prefixes(mut self, prefixes: Vec<String>) -> Self {
		self.excluded_prefixes = prefixes;
		self
	}

	/// Set cache control configuration.
	pub fn cache_config(mut self, config: CacheControlConfig) -> Self {
		self.cache_config = config;
		self
	}
}

/// Middleware for serving static files.
///
/// This middleware intercepts requests matching the configured URL prefix
/// and serves files from the root directory. It's designed for serving
/// WASM frontend builds and static assets.
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_utils::staticfiles::middleware::{StaticFilesMiddleware, StaticFilesConfig};
/// use std::path::PathBuf;
///
/// let config = StaticFilesConfig::new("dist")
///     .url_prefix("/")
///     .spa_mode(true);
///
/// let middleware = StaticFilesMiddleware::new(config);
/// ```
pub struct StaticFilesMiddleware {
	config: StaticFilesConfig,
	handler: StaticFileHandler,
}

impl StaticFilesMiddleware {
	/// Create a new static files middleware with the given configuration.
	pub fn new(config: StaticFilesConfig) -> Self {
		let handler = StaticFileHandler::new(config.root_dir.clone())
			.with_index_files(config.index_files.clone());
		Self { config, handler }
	}

	/// Create a middleware with default configuration for the given directory.
	pub fn for_directory(root_dir: impl Into<PathBuf>) -> Self {
		Self::new(StaticFilesConfig::new(root_dir))
	}

	/// Check if the request path matches the URL prefix.
	fn matches_prefix(&self, path: &str) -> bool {
		if self.config.url_prefix == "/" {
			true
		} else {
			path.starts_with(&self.config.url_prefix)
		}
	}

	/// Get the file path relative to the root directory.
	fn get_file_path(&self, request_path: &str) -> String {
		if self.config.url_prefix == "/" {
			request_path.to_string()
		} else {
			request_path
				.strip_prefix(&self.config.url_prefix)
				.unwrap_or(request_path)
				.to_string()
		}
	}

	/// Check if the file extension is allowed.
	fn is_extension_allowed(&self, path: &str) -> bool {
		if self.config.allowed_extensions.is_empty() {
			return true;
		}

		let extension = path
			.rsplit('.')
			.next()
			.map(|s| s.to_lowercase())
			.unwrap_or_default();

		self.config
			.allowed_extensions
			.iter()
			.any(|ext| ext.eq_ignore_ascii_case(&extension))
	}

	/// Try to serve a static file.
	async fn try_serve(&self, path: &str) -> Option<Response> {
		match self.handler.serve(path).await {
			Ok(file) => {
				let mut response = Response::ok()
					.with_header("Content-Type", &file.mime_type)
					.with_header("ETag", &file.etag());

				// Only set cache headers when caching is enabled
				if self.config.cache_config.enabled {
					let policy = self.config.cache_config.get_policy(path);
					let cache_value = policy.to_header_value();
					response = response.with_header("Cache-Control", &cache_value);

					// Apply Vary header if specified in the policy
					if let Some(vary) = &policy.vary {
						response = response.with_header("Vary", vary);
					}
				}

				response = response.with_body(file.content);
				Some(response)
			}
			Err(StaticError::NotFound(_)) => None,
			Err(_) => None,
		}
	}

	/// Serve the SPA fallback (index.html).
	async fn serve_spa_fallback(&self) -> Option<Response> {
		for index_file in &self.config.index_files {
			if let Some(response) = self.try_serve(index_file).await {
				return Some(response);
			}
		}
		None
	}
}

#[async_trait]
impl Middleware for StaticFilesMiddleware {
	async fn process(&self, request: Request, next: Arc<dyn Handler>) -> Result<Response> {
		let path = request.uri.path();

		// Check if this request matches our prefix
		if !self.matches_prefix(path) {
			return next.handle(request).await;
		}

		let file_path = self.get_file_path(path);

		// Check extension allowlist
		if !self.is_extension_allowed(&file_path) {
			return next.handle(request).await;
		}

		// Try to serve the static file
		if let Some(response) = self.try_serve(&file_path).await {
			return Ok(response);
		}

		// In SPA mode, try to serve index.html for routes not in excluded_prefixes
		if self.config.spa_mode
			&& !self
				.config
				.excluded_prefixes
				.iter()
				.any(|prefix| path.starts_with(prefix))
			&& let Some(response) = self.serve_spa_fallback().await
		{
			return Ok(response);
		}

		// Fall through to the next handler
		next.handle(request).await
	}

	fn should_continue(&self, request: &Request) -> bool {
		// Only process GET and HEAD requests
		let method = request.method.as_str();
		method == "GET" || method == "HEAD"
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::staticfiles::caching::{CacheControlConfig, CachePolicy};
	use rstest::rstest;

	#[test]
	fn test_config_defaults() {
		let config = StaticFilesConfig::default();
		assert_eq!(config.root_dir, PathBuf::from("dist"));
		assert_eq!(config.url_prefix, "/");
		assert!(config.spa_mode);
		assert_eq!(config.index_files, vec!["index.html".to_string()]);
	}

	#[test]
	fn test_config_builder() {
		let config = StaticFilesConfig::new("public")
			.url_prefix("/static/")
			.spa_mode(false)
			.index_files(vec!["index.html".to_string(), "default.html".to_string()]);

		assert_eq!(config.root_dir, PathBuf::from("public"));
		assert_eq!(config.url_prefix, "/static/");
		assert!(!config.spa_mode);
		assert_eq!(config.index_files.len(), 2);
	}

	#[test]
	fn test_matches_prefix() {
		let config = StaticFilesConfig::new("dist").url_prefix("/static/");
		let middleware = StaticFilesMiddleware::new(config);

		assert!(middleware.matches_prefix("/static/app.js"));
		assert!(middleware.matches_prefix("/static/"));
		assert!(!middleware.matches_prefix("/api/users"));
	}

	#[test]
	fn test_matches_prefix_root() {
		let config = StaticFilesConfig::new("dist").url_prefix("/");
		let middleware = StaticFilesMiddleware::new(config);

		assert!(middleware.matches_prefix("/app.js"));
		assert!(middleware.matches_prefix("/api/users"));
	}

	#[test]
	fn test_get_file_path() {
		let config = StaticFilesConfig::new("dist").url_prefix("/static/");
		let middleware = StaticFilesMiddleware::new(config);

		assert_eq!(middleware.get_file_path("/static/app.js"), "app.js");
		assert_eq!(
			middleware.get_file_path("/static/css/style.css"),
			"css/style.css"
		);
	}

	#[test]
	fn test_is_extension_allowed_empty() {
		let config = StaticFilesConfig::new("dist");
		let middleware = StaticFilesMiddleware::new(config);

		assert!(middleware.is_extension_allowed("app.js"));
		assert!(middleware.is_extension_allowed("style.css"));
		assert!(middleware.is_extension_allowed("file.wasm"));
	}

	#[test]
	fn test_is_extension_allowed_restricted() {
		let config = StaticFilesConfig::new("dist").allowed_extensions(vec![
			"js".to_string(),
			"css".to_string(),
			"wasm".to_string(),
		]);
		let middleware = StaticFilesMiddleware::new(config);

		assert!(middleware.is_extension_allowed("app.js"));
		assert!(middleware.is_extension_allowed("style.css"));
		assert!(middleware.is_extension_allowed("app.wasm"));
		assert!(!middleware.is_extension_allowed("secret.json"));
	}

	#[rstest]
	fn test_config_default_has_cache_config() {
		// Arrange
		let config = StaticFilesConfig::default();

		// Act
		let html_policy = config.cache_config.get_policy("index.html");
		let js_policy = config.cache_config.get_policy("app.js");

		// Assert
		assert_eq!(
			html_policy.to_header_value(),
			"public, must-revalidate, max-age=300"
		);
		assert_eq!(
			js_policy.to_header_value(),
			"public, immutable, max-age=31536000"
		);
	}

	#[rstest]
	#[case("style.css", "public, immutable, max-age=31536000")]
	#[case("app.js", "public, immutable, max-age=31536000")]
	#[case("app.wasm", "public, immutable, max-age=31536000")]
	#[case("font.woff2", "public, immutable, max-age=31536000")]
	fn test_config_cache_long_term_extensions(#[case] path: &str, #[case] expected: &str) {
		// Arrange
		let config = StaticFilesConfig::default();

		// Act
		let policy = config.cache_config.get_policy(path);

		// Assert
		assert_eq!(policy.to_header_value(), expected);
	}

	#[rstest]
	#[case("index.html", "public, must-revalidate, max-age=300")]
	#[case("file.unknown", "public, must-revalidate, max-age=300")]
	fn test_config_cache_short_term_extensions(#[case] path: &str, #[case] expected: &str) {
		// Arrange
		let config = StaticFilesConfig::default();

		// Act
		let policy = config.cache_config.get_policy(path);

		// Assert
		assert_eq!(policy.to_header_value(), expected);
	}

	#[rstest]
	fn test_config_custom_cache_config() {
		// Arrange
		let custom_cache =
			CacheControlConfig::new().with_type_policy("html".to_string(), CachePolicy::no_cache());

		// Act
		let config = StaticFilesConfig::new("dist").cache_config(custom_cache);
		let html_policy = config.cache_config.get_policy("index.html");

		// Assert
		assert_eq!(
			html_policy.to_header_value(),
			"no-cache, no-store, must-revalidate"
		);
	}

	#[rstest]
	fn test_config_index_file_default_is_none() {
		// Arrange & Act
		let config = StaticFilesConfig::default();

		// Assert
		assert!(config.index_file.is_none());
	}

	#[rstest]
	fn test_config_index_file_builder_sets_path() {
		// Arrange & Act
		let config = StaticFilesConfig::new("dist")
			.index_file("./index.html");

		// Assert
		assert_eq!(config.index_file, Some(PathBuf::from("./index.html")));
	}

	#[rstest]
	fn test_config_index_file_absolute_path_preserved() {
		// Arrange & Act
		let config = StaticFilesConfig::new("dist")
			.index_file("/absolute/path/index.html");

		// Assert
		assert_eq!(
			config.index_file,
			Some(PathBuf::from("/absolute/path/index.html"))
		);
	}
}

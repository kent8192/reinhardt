//! Static files middleware for serving WASM builds and static assets.
//!
//! This middleware intercepts requests and serves static files from a configured directory.
//! It supports SPA (Single Page Application) mode for WASM frontend applications.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use reinhardt_core::exception::Result;
use reinhardt_http::{Handler, Middleware};
use reinhardt_http::{Request, Response};

use super::caching::CacheControlConfig;
use super::handler::{StaticError, StaticFileHandler};

/// Detected WASM entry point for auto-injection.
#[derive(Debug, Clone)]
struct WasmEntry {
	/// JS entry file relative to root_dir (e.g., "my_app.js")
	js_file: String,
	/// WASM binary file relative to root_dir (e.g., "my_app_bg.wasm")
	wasm_file: String,
}

/// Configuration for the static files middleware.
#[derive(Debug, Clone)]
#[non_exhaustive]
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
	/// Enable automatic WASM script injection into SPA HTML responses
	pub auto_inject_wasm: bool,
	/// Explicit WASM entry point filename (e.g., "my_app.js") for fallback detection
	pub wasm_entry: Option<String>,
	/// Manifest mapping original filenames to hashed filenames
	pub wasm_manifest: Option<HashMap<String, String>>,
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
			auto_inject_wasm: true,
			wasm_entry: None,
			wasm_manifest: None,
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

	/// Enable or disable automatic WASM script injection.
	pub fn auto_inject_wasm(mut self, enabled: bool) -> Self {
		self.auto_inject_wasm = enabled;
		self
	}

	/// Set the explicit WASM entry point filename for fallback detection.
	///
	/// The entry must be a `.js` filename (e.g., `"my_app.js"` or `"pkg/my_app.js"`).
	/// The corresponding WASM file is inferred by stripping `.js` and appending `_bg.wasm`.
	///
	/// # Panics
	///
	/// Panics if `entry` contains invalid characters. Only alphanumeric characters,
	/// `-`, `_`, `.`, and `/` are allowed.
	///
	/// Panics if `entry` contains `..` path traversal sequences.
	///
	/// Panics if `entry` is empty.
	pub fn wasm_entry(mut self, entry: impl Into<String>) -> Self {
		let entry = entry.into();
		assert!(!entry.is_empty(), "wasm_entry must not be empty");
		assert!(
			!entry.contains(".."),
			"wasm_entry must not contain '..' path traversal sequences: {entry}"
		);
		if !entry
			.chars()
			.all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '/')
		{
			panic!(
				"wasm_entry contains invalid characters: only alphanumeric, '-', '_', '.', '/' are allowed"
			);
		}
		self.wasm_entry = Some(entry);
		self
	}

	/// Set the WASM manifest for filename resolution (e.g., hashed filenames).
	pub fn wasm_manifest(mut self, manifest: HashMap<String, String>) -> Self {
		self.wasm_manifest = Some(manifest);
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
	wasm_entry: Option<WasmEntry>,
}

impl StaticFilesMiddleware {
	/// Create a new static files middleware with the given configuration.
	pub fn new(config: StaticFilesConfig) -> Self {
		let handler = StaticFileHandler::new(config.root_dir.clone())
			.with_index_files(config.index_files.clone());
		let wasm_entry = if config.auto_inject_wasm {
			Self::detect_wasm_entry(&config)
		} else {
			tracing::info!("WASM auto-injection is disabled");
			None
		};
		Self {
			config,
			handler,
			wasm_entry,
		}
	}

	/// Create a middleware with default configuration for the given directory.
	pub fn for_directory(root_dir: impl Into<PathBuf>) -> Self {
		Self::new(StaticFilesConfig::new(root_dir))
	}

	/// Detect WASM entry point by scanning `root_dir` for `{name}.js` + `{name}_bg.wasm` pairs.
	///
	/// Falls back to `config.wasm_entry` when zero or multiple pairs are found.
	fn detect_wasm_entry(config: &StaticFilesConfig) -> Option<WasmEntry> {
		let root = &config.root_dir;
		tracing::debug!("scanning {:?} for WASM entry points", root);

		// Scan top-level files in root_dir for {name}.js + {name}_bg.wasm pairs
		let mut pairs: Vec<(String, String)> = Vec::new();
		if let Ok(entries) = std::fs::read_dir(root) {
			let mut js_stems: Vec<String> = Vec::new();
			let mut wasm_stems: Vec<String> = Vec::new();

			for entry in entries.flatten() {
				let path = entry.path();
				if !path.is_file() {
					continue;
				}
				if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
					if let Some(stem) = name.strip_suffix(".js") {
						js_stems.push(stem.to_string());
					} else if let Some(stem) = name.strip_suffix("_bg.wasm") {
						wasm_stems.push(stem.to_string());
					}
				}
			}

			for stem in &js_stems {
				if wasm_stems.contains(stem) {
					pairs.push((format!("{stem}.js"), format!("{stem}_bg.wasm")));
				}
			}
		}

		match pairs.len() {
			1 => {
				let (js_file, wasm_file) = pairs.into_iter().next().unwrap();
				tracing::info!(
					"auto-detected WASM entry: js={}, wasm={}",
					js_file,
					wasm_file
				);
				Some(WasmEntry { js_file, wasm_file })
			}
			0 => {
				tracing::debug!("no WASM pairs found in {:?}, trying fallback", root);
				Self::try_wasm_entry_fallback(config)
			}
			n => {
				tracing::warn!(
					"found {} WASM pairs in {:?}, cannot auto-detect; trying fallback",
					n,
					root
				);
				Self::try_wasm_entry_fallback(config)
			}
		}
	}

	/// Try to resolve WASM entry from `config.wasm_entry` fallback.
	///
	/// Accepts a `.js` filename (e.g., `"my_app.js"`) and infers the WASM file
	/// by stripping `.js` and appending `_bg.wasm`.
	fn try_wasm_entry_fallback(config: &StaticFilesConfig) -> Option<WasmEntry> {
		let entry_name = config.wasm_entry.as_ref()?;
		let js_file = entry_name.clone();
		let stem = js_file.strip_suffix(".js").unwrap_or(&js_file);
		let wasm_file = format!("{stem}_bg.wasm");

		let js_path = config.root_dir.join(&js_file);
		let wasm_path = config.root_dir.join(&wasm_file);

		if !js_path.exists() {
			tracing::warn!("fallback WASM JS file not found: {:?}", js_path);
			return None;
		}
		if !wasm_path.exists() {
			tracing::warn!("fallback WASM binary not found: {:?}", wasm_path);
			return None;
		}

		tracing::info!(
			"using fallback WASM entry: js={}, wasm={}",
			js_file,
			wasm_file
		);
		Some(WasmEntry { js_file, wasm_file })
	}

	/// Resolve the URL for a WASM-related file, applying manifest lookup if available.
	///
	/// Manifest values are validated to contain only safe characters (alphanumeric,
	/// `-`, `_`, `.`, `/`). Unsafe values are rejected and the original filename is
	/// used as a fallback to prevent HTML injection.
	fn resolve_wasm_url(
		filename: &str,
		url_prefix: &str,
		manifest: Option<&HashMap<String, String>>,
	) -> String {
		let resolved = manifest
			.and_then(|m| m.get(filename))
			.filter(|v| {
				v.chars()
					.all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'))
			})
			.map(|s| s.as_str())
			.unwrap_or(filename);
		format!("{url_prefix}{resolved}")
	}

	/// Inject a WASM auto-loader script into HTML content before `</body>`.
	///
	/// If no `</body>` tag is found (case-insensitive), the script is appended to the end.
	fn inject_wasm_script(
		html: &str,
		entry: &WasmEntry,
		url_prefix: &str,
		manifest: Option<&HashMap<String, String>>,
	) -> String {
		let js_url = Self::resolve_wasm_url(&entry.js_file, url_prefix, manifest);
		let wasm_url = Self::resolve_wasm_url(&entry.wasm_file, url_prefix, manifest);

		let script = format!(
			"\n<!-- Reinhardt WASM Auto-Loader -->\n\
			 <script type=\"module\">\n\
			 const {{ default: init }} = await import('{js_url}');\n\
			 await init('{wasm_url}');\n\
			 </script>\n"
		);

		// Case-insensitive search for </body>
		if let Some(pos) = html.to_lowercase().rfind("</body>") {
			let mut result = String::with_capacity(html.len() + script.len());
			result.push_str(&html[..pos]);
			result.push_str(&script);
			result.push_str(&html[pos..]);
			result
		} else {
			let mut result = String::with_capacity(html.len() + script.len());
			result.push_str(html);
			result.push_str(&script);
			result
		}
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

	/// Serve the SPA fallback (index.html), optionally injecting WASM auto-loader script.
	///
	/// Priority:
	/// 1. `index_file` — explicit path (can be outside `root_dir`)
	/// 2. `index_files` — searched within `root_dir`
	async fn serve_spa_fallback(&self) -> Option<Response> {
		// Priority 1: Explicit index file path (can be outside root_dir)
		if let Some(ref index_path) = self.config.index_file {
			let content = tokio::fs::read(index_path).await.ok()?;
			return self.build_spa_response(content, index_path);
		}

		// Priority 2: Search within root_dir (existing behavior)
		for index_file in &self.config.index_files {
			let path = self.config.root_dir.join(index_file);
			if let Ok(content) = tokio::fs::read(&path).await {
				return self.build_spa_response(content, &path);
			}
		}
		None
	}

	/// Build a SPA response from raw file content, injecting WASM script if applicable.
	///
	/// Computes ETag from the final (post-injection) content to ensure cache correctness.
	fn build_spa_response(&self, content: Vec<u8>, path: &Path) -> Option<Response> {
		let mime = mime_guess::from_path(path)
			.first_or_octet_stream()
			.to_string();

		let filename = path
			.file_name()
			.and_then(|n| n.to_str())
			.unwrap_or("index.html");

		// Apply WASM injection if entry is detected
		let final_content = if let Some(ref entry) = self.wasm_entry {
			match String::from_utf8(content) {
				Ok(html) => {
					let injected = Self::inject_wasm_script(
						&html,
						entry,
						&self.config.url_prefix,
						self.config.wasm_manifest.as_ref(),
					);
					tracing::debug!("injected WASM auto-loader into SPA response");
					injected.into_bytes()
				}
				Err(e) => {
					tracing::warn!(
						"SPA fallback is not valid UTF-8, serving raw content: {}",
						e
					);
					e.into_bytes()
				}
			}
		} else {
			content
		};

		// Generate ETag from final content (post-injection)
		let etag = {
			use std::collections::hash_map::DefaultHasher;
			use std::hash::{Hash, Hasher};
			let mut hasher = DefaultHasher::new();
			final_content.hash(&mut hasher);
			format!("\"{}\"", hasher.finish())
		};

		let mut response = Response::ok()
			.with_header("Content-Type", &mime)
			.with_header("ETag", &etag);

		if self.config.cache_config.enabled {
			let policy = self.config.cache_config.get_policy(filename);
			let cache_value = policy.to_header_value();
			response = response.with_header("Cache-Control", &cache_value);

			if let Some(vary) = &policy.vary {
				response = response.with_header("Vary", vary);
			}
		}

		response = response.with_body(final_content);
		Some(response)
	}

	/// Serve a file directly from a configured filesystem path (bypasses `root_dir` security check).
	///
	/// The path may be absolute or relative, depending on how it was configured (e.g. via CLI
	/// or configuration file); relative paths are resolved by the OS at runtime.
	/// This is safe because the path is a fixed, user-specified value — not derived
	/// from the request URL.
	///
	/// Generates ETag and Cache-Control headers consistent with `try_serve`.
	// Used by tests to verify header generation independently of SPA injection flow
	#[cfg(test)]
	async fn serve_direct_file(&self, path: &Path) -> Option<Response> {
		let content = tokio::fs::read(path).await.ok()?;
		let mime = mime_guess::from_path(path)
			.first_or_octet_stream()
			.to_string();

		// Generate ETag from content hash (consistent with StaticFileHandler::etag)
		let etag = {
			use std::collections::hash_map::DefaultHasher;
			use std::hash::{Hash, Hasher};
			let mut hasher = DefaultHasher::new();
			content.hash(&mut hasher);
			format!("\"{}\"", hasher.finish())
		};

		let filename = path
			.file_name()
			.and_then(|n| n.to_str())
			.unwrap_or("index.html");

		let mut response = Response::ok()
			.with_header("Content-Type", &mime)
			.with_header("ETag", &etag);

		if self.config.cache_config.enabled {
			let policy = self.config.cache_config.get_policy(filename);
			let cache_value = policy.to_header_value();
			response = response.with_header("Cache-Control", &cache_value);

			if let Some(vary) = &policy.vary {
				response = response.with_header("Vary", vary);
			}
		}

		response = response.with_body(content);
		Some(response)
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
		let config = StaticFilesConfig::new("dist").index_file("./index.html");

		// Assert
		assert_eq!(config.index_file, Some(PathBuf::from("./index.html")));
	}

	#[rstest]
	fn test_config_index_file_absolute_path_preserved() {
		// Arrange & Act
		let config = StaticFilesConfig::new("dist").index_file("/absolute/path/index.html");

		// Assert
		assert_eq!(
			config.index_file,
			Some(PathBuf::from("/absolute/path/index.html"))
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_serve_direct_file_existing_html() {
		// Arrange
		let dir = tempfile::tempdir().unwrap();
		let index_path = dir.path().join("index.html");
		std::fs::write(&index_path, "<html>hello</html>").unwrap();

		let config = StaticFilesConfig::new(dir.path().join("dist")).index_file(&index_path);
		let middleware = StaticFilesMiddleware::new(config);

		// Act
		let response = middleware.serve_direct_file(&index_path).await;

		// Assert
		let response = response.expect("should return Some");
		assert_eq!(response.headers.get("Content-Type").unwrap(), "text/html");
		assert!(response.headers.contains_key("ETag"));
		assert!(response.headers.contains_key("Cache-Control"));
	}

	#[rstest]
	#[tokio::test]
	async fn test_serve_direct_file_nonexistent_returns_none() {
		// Arrange
		let dir = tempfile::tempdir().unwrap();
		let config = StaticFilesConfig::new("dist");
		let middleware = StaticFilesMiddleware::new(config);
		let nonexistent = dir.path().join("nonexistent_index_2869.html");

		// Act
		let response = middleware.serve_direct_file(&nonexistent).await;

		// Assert
		assert!(response.is_none());
	}

	#[rstest]
	fn test_config_index_file_with_spa_mode_false() {
		// Arrange & Act
		let config = StaticFilesConfig::new("dist")
			.index_file("./index.html")
			.spa_mode(false);

		// Assert
		assert_eq!(config.index_file, Some(PathBuf::from("./index.html")));
		assert!(!config.spa_mode);
	}

	#[rstest]
	#[tokio::test]
	async fn test_serve_spa_fallback_with_index_file_serves_direct_path() {
		// Arrange
		let dir = tempfile::tempdir().unwrap();
		let index_path = dir.path().join("index.html");
		std::fs::write(&index_path, "<html>direct</html>").unwrap();

		// Create dist/ with a DIFFERENT index.html to verify priority
		let dist = dir.path().join("dist");
		std::fs::create_dir_all(&dist).unwrap();
		std::fs::write(dist.join("index.html"), "<html>dist</html>").unwrap();

		let config = StaticFilesConfig::new(&dist).index_file(&index_path);
		let middleware = StaticFilesMiddleware::new(config);

		// Act
		let response = middleware.serve_spa_fallback().await;

		// Assert
		let response = response.expect("should return Some");
		let body = std::str::from_utf8(&response.body).unwrap();
		assert_eq!(body, "<html>direct</html>");
	}

	#[rstest]
	#[tokio::test]
	async fn test_serve_spa_fallback_without_index_file_searches_root_dir() {
		// Arrange
		let dir = tempfile::tempdir().unwrap();
		let dist = dir.path().join("dist");
		std::fs::create_dir_all(&dist).unwrap();
		std::fs::write(dist.join("index.html"), "<html>dist fallback</html>").unwrap();

		let config = StaticFilesConfig::new(&dist);
		let middleware = StaticFilesMiddleware::new(config);

		// Act
		let response = middleware.serve_spa_fallback().await;

		// Assert
		let response = response.expect("should return Some");
		let body = std::str::from_utf8(&response.body).unwrap();
		assert_eq!(body, "<html>dist fallback</html>");
	}

	#[rstest]
	#[tokio::test]
	async fn test_etag_matches_static_file_handler_format() {
		// Arrange
		let dir = tempfile::tempdir().unwrap();
		let index_path = dir.path().join("index.html");
		std::fs::write(&index_path, "<html>etag test</html>").unwrap();

		let config = StaticFilesConfig::new(dir.path().join("dist")).index_file(&index_path);
		let middleware = StaticFilesMiddleware::new(config);

		// Act
		let response = middleware.serve_direct_file(&index_path).await.unwrap();
		let etag = response.headers.get("ETag").unwrap().to_str().unwrap();

		// Assert — ETag must be quoted and contain a numeric hash
		assert!(etag.starts_with('"'));
		assert!(etag.ends_with('"'));
		assert!(etag.len() > 2);
	}

	#[rstest]
	#[tokio::test]
	async fn test_etag_consistent_between_serve_direct_and_try_serve() {
		// Arrange — same file accessible via both paths
		let dir = tempfile::tempdir().unwrap();
		let index_path = dir.path().join("index.html");
		std::fs::write(&index_path, "<html>consistency</html>").unwrap();

		let config = StaticFilesConfig::new(dir.path()).index_file(&index_path);
		let middleware = StaticFilesMiddleware::new(config);

		// Act
		let direct_response = middleware.serve_direct_file(&index_path).await.unwrap();
		let try_response = middleware.try_serve("index.html").await.unwrap();

		// Assert
		let direct_etag = direct_response.headers.get("ETag").unwrap();
		let try_etag = try_response.headers.get("ETag").unwrap();
		assert_eq!(direct_etag, try_etag);
	}

	#[rstest]
	#[tokio::test]
	async fn test_backward_compat_no_index_file_uses_root_dir() {
		// Arrange
		let dir = tempfile::tempdir().unwrap();
		std::fs::write(dir.path().join("index.html"), "<html>compat</html>").unwrap();

		let config = StaticFilesConfig::new(dir.path());
		let middleware = StaticFilesMiddleware::new(config);

		// Act
		let response = middleware.serve_spa_fallback().await;

		// Assert
		let response = response.expect("should serve from root_dir");
		let body = std::str::from_utf8(&response.body).unwrap();
		assert_eq!(body, "<html>compat</html>");
	}

	#[rstest]
	#[tokio::test]
	async fn test_backward_compat_custom_index_files() {
		// Arrange
		let dir = tempfile::tempdir().unwrap();
		std::fs::write(dir.path().join("default.html"), "<html>custom</html>").unwrap();

		let config =
			StaticFilesConfig::new(dir.path()).index_files(vec!["default.html".to_string()]);
		let middleware = StaticFilesMiddleware::new(config);

		// Act
		let response = middleware.serve_spa_fallback().await;

		// Assert
		let response = response.expect("should serve custom index file");
		let body = std::str::from_utf8(&response.body).unwrap();
		assert_eq!(body, "<html>custom</html>");
	}

	#[rstest]
	#[tokio::test]
	async fn test_serve_direct_file_request_path_independent() {
		// Arrange
		let dir = tempfile::tempdir().unwrap();
		let index_path = dir.path().join("index.html");
		std::fs::write(&index_path, "<html>safe</html>").unwrap();

		let config = StaticFilesConfig::new(dir.path().join("dist"))
			.index_file(&index_path)
			.spa_mode(true);
		let middleware = StaticFilesMiddleware::new(config);

		// Act
		let response1 = middleware.serve_direct_file(&index_path).await;
		let response2 = middleware.serve_direct_file(&index_path).await;

		// Assert
		let body1 = std::str::from_utf8(&response1.unwrap().body)
			.unwrap()
			.to_string();
		let body2 = std::str::from_utf8(&response2.unwrap().body)
			.unwrap()
			.to_string();
		assert_eq!(body1, body2);
		assert_eq!(body1, "<html>safe</html>");
	}

	#[rstest]
	#[tokio::test]
	async fn test_serve_direct_file_cache_disabled_no_cache_header() {
		// Arrange
		let dir = tempfile::tempdir().unwrap();
		let index_path = dir.path().join("index.html");
		std::fs::write(&index_path, "<html>hello</html>").unwrap();

		let mut cache_config = CacheControlConfig::new();
		cache_config.enabled = false;

		let config = StaticFilesConfig::new(dir.path().join("dist"))
			.index_file(&index_path)
			.cache_config(cache_config);
		let middleware = StaticFilesMiddleware::new(config);

		// Act
		let response = middleware.serve_direct_file(&index_path).await;

		// Assert
		let response = response.expect("should return Some");
		assert!(response.headers.contains_key("ETag"));
		assert!(!response.headers.contains_key("Cache-Control"));
	}

	#[rstest]
	fn test_config_auto_inject_wasm_default_true() {
		// Arrange & Act
		let config = StaticFilesConfig::default();

		// Assert
		assert!(config.auto_inject_wasm);
	}

	#[rstest]
	fn test_config_auto_inject_wasm_builder() {
		// Arrange & Act
		let config = StaticFilesConfig::new("dist").auto_inject_wasm(false);

		// Assert
		assert!(!config.auto_inject_wasm);
	}

	#[rstest]
	fn test_config_wasm_entry_default_none() {
		// Arrange & Act
		let config = StaticFilesConfig::default();

		// Assert
		assert!(config.wasm_entry.is_none());
	}

	#[rstest]
	fn test_config_wasm_entry_builder() {
		// Arrange & Act
		let config = StaticFilesConfig::new("dist").wasm_entry("my_app.js");

		// Assert
		assert_eq!(config.wasm_entry, Some("my_app.js".to_string()));
	}

	#[rstest]
	fn test_config_wasm_manifest_default_none() {
		// Arrange & Act
		let config = StaticFilesConfig::default();

		// Assert
		assert!(config.wasm_manifest.is_none());
	}

	#[rstest]
	fn test_config_wasm_manifest_builder() {
		// Arrange
		let mut manifest = HashMap::new();
		manifest.insert("app.js".to_string(), "app.abc123.js".to_string());

		// Act
		let config = StaticFilesConfig::new("dist").wasm_manifest(manifest.clone());

		// Assert
		assert_eq!(config.wasm_manifest, Some(manifest));
	}

	#[rstest]
	#[should_panic(expected = "invalid characters")]
	fn test_config_wasm_entry_rejects_unsafe_chars() {
		// Arrange & Act & Assert
		StaticFilesConfig::new("dist").wasm_entry("my app;rm -rf.js");
	}

	#[rstest]
	fn test_config_wasm_entry_allows_path_separators() {
		// Arrange & Act
		let config = StaticFilesConfig::new("dist").wasm_entry("sub/my_app.js");

		// Assert
		assert_eq!(config.wasm_entry, Some("sub/my_app.js".to_string()));
	}

	#[rstest]
	fn test_detect_wasm_entry_single_pair() {
		// Arrange
		let dir = tempfile::tempdir().unwrap();
		std::fs::write(dir.path().join("my_app.js"), "// js").unwrap();
		std::fs::write(dir.path().join("my_app_bg.wasm"), [0u8; 4]).unwrap();
		let config = StaticFilesConfig::new(dir.path());

		// Act
		let entry = StaticFilesMiddleware::detect_wasm_entry(&config);

		// Assert
		let entry = entry.expect("should detect single pair");
		assert_eq!(entry.js_file, "my_app.js");
		assert_eq!(entry.wasm_file, "my_app_bg.wasm");
	}

	#[rstest]
	fn test_detect_wasm_entry_no_pair() {
		// Arrange
		let dir = tempfile::tempdir().unwrap();
		std::fs::write(dir.path().join("app.js"), "// js").unwrap();
		// No matching _bg.wasm file
		let config = StaticFilesConfig::new(dir.path());

		// Act
		let entry = StaticFilesMiddleware::detect_wasm_entry(&config);

		// Assert
		assert!(entry.is_none());
	}

	#[rstest]
	fn test_detect_wasm_entry_multiple_pairs_falls_back_to_wasm_entry() {
		// Arrange
		let dir = tempfile::tempdir().unwrap();
		std::fs::write(dir.path().join("app_a.js"), "// js a").unwrap();
		std::fs::write(dir.path().join("app_a_bg.wasm"), [0u8; 4]).unwrap();
		std::fs::write(dir.path().join("app_b.js"), "// js b").unwrap();
		std::fs::write(dir.path().join("app_b_bg.wasm"), [0u8; 4]).unwrap();
		let config = StaticFilesConfig::new(dir.path()).wasm_entry("app_a.js");

		// Act
		let entry = StaticFilesMiddleware::detect_wasm_entry(&config);

		// Assert
		let entry = entry.expect("should fall back to wasm_entry");
		assert_eq!(entry.js_file, "app_a.js");
		assert_eq!(entry.wasm_file, "app_a_bg.wasm");
	}

	#[rstest]
	fn test_detect_wasm_entry_fallback_missing_file() {
		// Arrange
		let dir = tempfile::tempdir().unwrap();
		// Only create js, not the wasm file
		std::fs::write(dir.path().join("missing_app.js"), "// js").unwrap();
		let config = StaticFilesConfig::new(dir.path()).wasm_entry("missing_app.js");

		// Act
		let entry = StaticFilesMiddleware::detect_wasm_entry(&config);

		// Assert
		assert!(entry.is_none());
	}

	#[rstest]
	fn test_detect_wasm_entry_disabled() {
		// Arrange
		let dir = tempfile::tempdir().unwrap();
		std::fs::write(dir.path().join("my_app.js"), "// js").unwrap();
		std::fs::write(dir.path().join("my_app_bg.wasm"), [0u8; 4]).unwrap();
		let config = StaticFilesConfig::new(dir.path()).auto_inject_wasm(false);

		// Act
		let middleware = StaticFilesMiddleware::new(config);

		// Assert
		assert!(middleware.wasm_entry.is_none());
	}

	#[rstest]
	fn test_detect_wasm_entry_ignores_non_wasm_js_files() {
		// Arrange
		let dir = tempfile::tempdir().unwrap();
		std::fs::write(dir.path().join("utils.js"), "// utility").unwrap();
		std::fs::write(dir.path().join("style.css"), "body{}").unwrap();
		std::fs::write(dir.path().join("data.json"), "{}").unwrap();
		let config = StaticFilesConfig::new(dir.path());

		// Act
		let entry = StaticFilesMiddleware::detect_wasm_entry(&config);

		// Assert
		assert!(entry.is_none());
	}

	#[rstest]
	fn test_resolve_wasm_url_no_manifest() {
		// Arrange & Act
		let url = StaticFilesMiddleware::resolve_wasm_url("app.js", "/static/", None);

		// Assert
		assert_eq!(url, "/static/app.js");
	}

	#[rstest]
	fn test_resolve_wasm_url_with_manifest_match() {
		// Arrange
		let mut manifest = HashMap::new();
		manifest.insert("app.js".to_string(), "app.abc123.js".to_string());

		// Act
		let url = StaticFilesMiddleware::resolve_wasm_url("app.js", "/static/", Some(&manifest));

		// Assert
		assert_eq!(url, "/static/app.abc123.js");
	}

	#[rstest]
	fn test_resolve_wasm_url_with_manifest_no_match() {
		// Arrange
		let mut manifest = HashMap::new();
		manifest.insert("other.js".to_string(), "other.xyz.js".to_string());

		// Act
		let url = StaticFilesMiddleware::resolve_wasm_url("app.js", "/static/", Some(&manifest));

		// Assert
		assert_eq!(url, "/static/app.js");
	}

	#[rstest]
	fn test_inject_wasm_script_before_body() {
		// Arrange
		let html = "<html><body><h1>Hello</h1></body></html>";
		let entry = WasmEntry {
			js_file: "app.js".to_string(),
			wasm_file: "app_bg.wasm".to_string(),
		};

		// Act
		let result = StaticFilesMiddleware::inject_wasm_script(html, &entry, "/", None);

		// Assert — generated HTML with dynamic URLs
		assert!(result.contains("<!-- Reinhardt WASM Auto-Loader -->"));
		assert!(result.contains("await import('/app.js')"));
		assert!(result.contains("await init('/app_bg.wasm')"));
		assert!(result.contains("</body></html>"));
	}

	#[rstest]
	fn test_inject_wasm_script_case_insensitive_body() {
		// Arrange
		let html = "<html><body><h1>Hello</h1></BODY></html>";
		let entry = WasmEntry {
			js_file: "app.js".to_string(),
			wasm_file: "app_bg.wasm".to_string(),
		};

		// Act
		let result = StaticFilesMiddleware::inject_wasm_script(html, &entry, "/", None);

		// Assert — generated HTML with dynamic URLs
		assert!(result.contains("<!-- Reinhardt WASM Auto-Loader -->"));
		assert!(result.contains("</BODY></html>"));
	}

	#[rstest]
	fn test_inject_wasm_script_no_body_tag_appends() {
		// Arrange
		let html = "<html><h1>No body tag</h1></html>";
		let entry = WasmEntry {
			js_file: "app.js".to_string(),
			wasm_file: "app_bg.wasm".to_string(),
		};

		// Act
		let result = StaticFilesMiddleware::inject_wasm_script(html, &entry, "/", None);

		// Assert — generated HTML with dynamic URLs
		assert!(result.ends_with("</script>\n"));
		assert!(result.contains("<!-- Reinhardt WASM Auto-Loader -->"));
	}

	#[rstest]
	fn test_inject_wasm_script_with_manifest() {
		// Arrange
		let html = "<html><body></body></html>";
		let entry = WasmEntry {
			js_file: "app.js".to_string(),
			wasm_file: "app_bg.wasm".to_string(),
		};
		let mut manifest = HashMap::new();
		manifest.insert("app.js".to_string(), "app.h4sh.js".to_string());
		manifest.insert("app_bg.wasm".to_string(), "app_bg.h4sh.wasm".to_string());

		// Act
		let result = StaticFilesMiddleware::inject_wasm_script(html, &entry, "/", Some(&manifest));

		// Assert — generated HTML with dynamic URLs
		assert!(result.contains("await import('/app.h4sh.js')"));
		assert!(result.contains("await init('/app_bg.h4sh.wasm')"));
	}

	#[rstest]
	fn test_inject_wasm_script_with_url_prefix() {
		// Arrange
		let html = "<html><body></body></html>";
		let entry = WasmEntry {
			js_file: "app.js".to_string(),
			wasm_file: "app_bg.wasm".to_string(),
		};

		// Act
		let result = StaticFilesMiddleware::inject_wasm_script(html, &entry, "/static/", None);

		// Assert — generated HTML with dynamic URLs
		assert!(result.contains("await import('/static/app.js')"));
		assert!(result.contains("await init('/static/app_bg.wasm')"));
	}

	#[rstest]
	fn test_detect_wasm_entry_fallback_with_path_separator() {
		// Arrange
		let dir = tempfile::tempdir().unwrap();
		let sub = dir.path().join("pkg");
		std::fs::create_dir_all(&sub).unwrap();
		std::fs::write(sub.join("my_app.js"), "// js").unwrap();
		std::fs::write(sub.join("my_app_bg.wasm"), [0u8; 4]).unwrap();
		let config = StaticFilesConfig::new(dir.path()).wasm_entry("pkg/my_app.js");

		// Act
		let entry = StaticFilesMiddleware::detect_wasm_entry(&config);

		// Assert
		let entry = entry.expect("should resolve path with separator");
		assert_eq!(entry.js_file, "pkg/my_app.js");
		assert_eq!(entry.wasm_file, "pkg/my_app_bg.wasm");
	}

	#[rstest]
	#[should_panic(expected = "path traversal")]
	fn test_config_wasm_entry_rejects_path_traversal() {
		// Arrange & Act & Assert
		StaticFilesConfig::new("dist").wasm_entry("../../etc/passwd.js");
	}

	#[rstest]
	#[should_panic(expected = "must not be empty")]
	fn test_config_wasm_entry_rejects_empty_string() {
		// Arrange & Act & Assert
		StaticFilesConfig::new("dist").wasm_entry("");
	}

	#[rstest]
	fn test_resolve_wasm_url_rejects_unsafe_manifest_values() {
		// Arrange
		let mut manifest = HashMap::new();
		manifest.insert("app.js".to_string(), "');alert('xss".to_string());

		// Act
		let url = StaticFilesMiddleware::resolve_wasm_url("app.js", "/", Some(&manifest));

		// Assert — falls back to original filename due to unsafe manifest value
		assert_eq!(url, "/app.js");
	}

	#[rstest]
	#[tokio::test]
	async fn test_serve_spa_fallback_auto_injects_wasm() {
		// Arrange
		let dir = tempfile::tempdir().unwrap();
		std::fs::write(
			dir.path().join("index.html"),
			"<html><body><h1>App</h1></body></html>",
		)
		.unwrap();
		std::fs::write(dir.path().join("my_app.js"), "// js").unwrap();
		std::fs::write(dir.path().join("my_app_bg.wasm"), [0u8; 4]).unwrap();

		let config = StaticFilesConfig::new(dir.path());
		let middleware = StaticFilesMiddleware::new(config);

		// Act
		let response = middleware.serve_spa_fallback().await;

		// Assert — generated HTML with dynamic URLs
		let response = response.expect("should return Some");
		let body = std::str::from_utf8(&response.body).unwrap();
		assert!(body.contains("<!-- Reinhardt WASM Auto-Loader -->"));
		assert!(body.contains("await import('/my_app.js')"));
		assert!(body.contains("await init('/my_app_bg.wasm')"));
		assert!(body.contains("</body></html>"));
	}

	#[rstest]
	#[tokio::test]
	async fn test_serve_spa_fallback_no_inject_when_disabled() {
		// Arrange
		let dir = tempfile::tempdir().unwrap();
		std::fs::write(
			dir.path().join("index.html"),
			"<html><body><h1>App</h1></body></html>",
		)
		.unwrap();
		std::fs::write(dir.path().join("my_app.js"), "// js").unwrap();
		std::fs::write(dir.path().join("my_app_bg.wasm"), [0u8; 4]).unwrap();

		let config = StaticFilesConfig::new(dir.path()).auto_inject_wasm(false);
		let middleware = StaticFilesMiddleware::new(config);

		// Act
		let response = middleware.serve_spa_fallback().await;

		// Assert
		let response = response.expect("should return Some");
		let body = std::str::from_utf8(&response.body).unwrap();
		assert!(!body.contains("Reinhardt WASM Auto-Loader"));
		assert_eq!(body, "<html><body><h1>App</h1></body></html>");
	}

	#[rstest]
	#[tokio::test]
	async fn test_serve_spa_fallback_etag_reflects_injected_content() {
		// Arrange
		let dir = tempfile::tempdir().unwrap();
		std::fs::write(dir.path().join("index.html"), "<html><body></body></html>").unwrap();
		std::fs::write(dir.path().join("my_app.js"), "// js").unwrap();
		std::fs::write(dir.path().join("my_app_bg.wasm"), [0u8; 4]).unwrap();

		let config_with = StaticFilesConfig::new(dir.path());
		let mw_with = StaticFilesMiddleware::new(config_with);

		let config_without = StaticFilesConfig::new(dir.path()).auto_inject_wasm(false);
		let mw_without = StaticFilesMiddleware::new(config_without);

		// Act
		let resp_with = mw_with.serve_spa_fallback().await.unwrap();
		let resp_without = mw_without.serve_spa_fallback().await.unwrap();

		// Assert — ETags must differ because content differs after injection
		let etag_with = resp_with.headers.get("ETag").unwrap();
		let etag_without = resp_without.headers.get("ETag").unwrap();
		assert_ne!(etag_with, etag_without);
	}

	#[rstest]
	#[tokio::test]
	async fn test_serve_spa_fallback_no_inject_when_spa_mode_false() {
		// Arrange — spa_mode gate is in process(), not serve_spa_fallback()
		// This test verifies that serve_spa_fallback still works independently
		let dir = tempfile::tempdir().unwrap();
		std::fs::write(dir.path().join("index.html"), "<html><body></body></html>").unwrap();

		let config = StaticFilesConfig::new(dir.path()).spa_mode(false);
		let middleware = StaticFilesMiddleware::new(config);

		// Act — calling serve_spa_fallback directly bypasses the spa_mode check in process()
		let response = middleware.serve_spa_fallback().await;

		// Assert — response is still produced (spa_mode gating is in process())
		assert!(response.is_some());
	}

	#[rstest]
	#[tokio::test]
	async fn test_serve_spa_fallback_invalid_utf8_serves_raw() {
		// Arrange
		let dir = tempfile::tempdir().unwrap();
		// Write invalid UTF-8 content as an index file
		let invalid_bytes: Vec<u8> = vec![0xFF, 0xFE, 0x00, 0x3C, 0x68, 0x74, 0x6D, 0x6C];
		std::fs::write(dir.path().join("index.html"), &invalid_bytes).unwrap();
		std::fs::write(dir.path().join("my_app.js"), "// js").unwrap();
		std::fs::write(dir.path().join("my_app_bg.wasm"), [0u8; 4]).unwrap();

		let config = StaticFilesConfig::new(dir.path());
		let middleware = StaticFilesMiddleware::new(config);

		// Act
		let response = middleware.serve_spa_fallback().await;

		// Assert — should serve raw content without injection
		let response = response.expect("should return Some");
		assert_eq!(response.body, invalid_bytes);
	}

	#[rstest]
	#[tokio::test]
	async fn test_serve_spa_fallback_preserves_content_type_and_cache_headers() {
		// Arrange
		let dir = tempfile::tempdir().unwrap();
		std::fs::write(dir.path().join("index.html"), "<html><body></body></html>").unwrap();
		std::fs::write(dir.path().join("my_app.js"), "// js").unwrap();
		std::fs::write(dir.path().join("my_app_bg.wasm"), [0u8; 4]).unwrap();

		let config = StaticFilesConfig::new(dir.path());
		let middleware = StaticFilesMiddleware::new(config);

		// Act
		let response = middleware.serve_spa_fallback().await;

		// Assert
		let response = response.expect("should return Some");
		assert_eq!(response.headers.get("Content-Type").unwrap(), "text/html");
		assert!(response.headers.contains_key("ETag"));
		assert!(response.headers.contains_key("Cache-Control"));
	}
}

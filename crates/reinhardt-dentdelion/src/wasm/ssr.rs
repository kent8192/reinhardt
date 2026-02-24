//! Server-Side Rendering (SSR) Proxy for WASM Plugins
//!
//! This module provides SSR functionality for WASM plugins through the host's
//! JavaScript/TypeScript runtime. It allows plugins to render React/Preact components
//! without requiring a JavaScript engine within the WASM sandbox.
//!
//! # SSR Backend
//!
//! - **TypeScript** (`ts` feature): Full TypeScript runtime using deno_core/rustyscript
//!
//! # Example
//!
//! ```ignore
//! use reinhardt_dentdelion::wasm::ssr::{SsrProxy, RenderOptions};
//! use reinhardt_dentdelion::wasm::TsRuntime;
//! use std::sync::Arc;
//!
//! // Create SSR proxy with TypeScript backend
//! let ts_runtime = Arc::new(TsRuntime::new()?);
//! let proxy = SsrProxy::with_ts_runtime(ts_runtime);
//!
//! // Check if SSR is available
//! if proxy.is_available() {
//!     let component_code = r#"
//!         function Component(props) {
//!             return h('div', null, 'Hello, ' + props.name);
//!         }
//!     "#;
//!     let result = proxy.render_component(
//!         component_code,
//!         r#"{"name": "World"}"#,
//!         RenderOptions::default(),
//!     )?;
//!     println!("Rendered HTML: {}", result.html);
//! }
//! ```

use serde::{Deserialize, Serialize};

#[cfg(feature = "ts")]
use super::ts_runtime::{SharedTsRuntime, TsError};

/// Render options for SSR.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RenderOptions {
	/// Whether to include hydration script for client-side takeover
	pub include_hydration: bool,
	/// Whether to extract CSS from components
	pub extract_css: bool,
	/// Whether to extract meta tags (title, description, etc.)
	pub extract_meta: bool,
}

/// Render result containing the HTML and optional extracted assets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderResult {
	/// Rendered HTML string
	pub html: String,
	/// Extracted CSS (if extract_css was true)
	pub css: Option<String>,
	/// Extracted meta tags as key-value pairs (if extract_meta was true)
	pub meta: Option<Vec<(String, String)>>,
	/// Hydration script to include in the page (if include_hydration was true)
	pub hydration_script: Option<String>,
}

/// SSR Proxy error types.
#[derive(Debug, Clone, thiserror::Error)]
pub enum SsrError {
	/// SSR is not available (no JavaScript runtime enabled)
	#[error("SSR is not available: no JavaScript runtime is enabled")]
	NotAvailable,

	/// Component not found at the specified path
	#[error("Component not found: {0}")]
	ComponentNotFound(String),

	/// Props serialization failed
	#[error("Failed to serialize props: {0}")]
	PropsSerialization(String),

	/// Render execution failed
	#[error("Render failed: {0}")]
	RenderFailed(String),

	/// JavaScript evaluation failed
	#[error("JavaScript evaluation failed: {0}")]
	EvalFailed(String),

	/// Permission denied due to insufficient trust level
	#[error("Permission denied: {0}")]
	PermissionDenied(String),

	/// Dangerous code pattern detected in component code
	#[error("Dangerous code pattern detected: {0}")]
	DangerousPattern(String),
}

#[cfg(feature = "ts")]
impl From<TsError> for SsrError {
	fn from(err: TsError) -> Self {
		match err {
			TsError::InitFailed(_msg) => SsrError::NotAvailable,
			TsError::EvalFailed(msg) => SsrError::EvalFailed(msg),
			TsError::PropsSerialization(msg) => SsrError::PropsSerialization(msg),
			TsError::RenderFailed(msg) => SsrError::RenderFailed(msg),
			TsError::ExecutionTimeout { timeout } => {
				SsrError::RenderFailed(format!("execution timed out after {timeout:?}"))
			}
			TsError::SourceTooLarge { size, max } => SsrError::RenderFailed(format!(
				"source size ({size} bytes) exceeds limit ({max} bytes)"
			)),
		}
	}
}

/// SSR Proxy for delegating rendering to the host's JavaScript/TypeScript runtime.
///
/// This proxy provides a bridge between WASM plugins and the host's SSR
/// capabilities using the TypeScript runtime:
///
/// - **TypeScript** (`ts` feature): Full runtime with deno_core/rustyscript
///
/// When no JavaScript runtime is available, all render operations return
/// [`SsrError::NotAvailable`].
pub struct SsrProxy {
	/// Whether SSR is available
	available: bool,

	/// TypeScript runtime (when `ts` feature is enabled)
	#[cfg(feature = "ts")]
	ts_runtime: Option<SharedTsRuntime>,

	/// Base directory for plugin assets (component files)
	plugin_base_dir: Option<std::path::PathBuf>,
}

impl Default for SsrProxy {
	fn default() -> Self {
		Self::new()
	}
}

impl std::fmt::Debug for SsrProxy {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let mut debug = f.debug_struct("SsrProxy");
		debug.field("available", &self.available);

		#[cfg(feature = "ts")]
		debug.field("ts_enabled", &self.ts_runtime.is_some());

		debug.field("plugin_base_dir", &self.plugin_base_dir);

		debug.finish()
	}
}

impl SsrProxy {
	/// Create a new SSR proxy.
	///
	/// By default, SSR is not available. Use [`with_ts_runtime`](Self::with_ts_runtime)
	/// to create a proxy with TypeScript backend support.
	pub fn new() -> Self {
		Self {
			available: false,
			#[cfg(feature = "ts")]
			ts_runtime: None,
			plugin_base_dir: None,
		}
	}

	/// Create an SSR proxy with the specified availability.
	///
	/// This is used by the host to indicate whether a JavaScript runtime
	/// is available for SSR.
	pub fn with_availability(available: bool) -> Self {
		Self {
			available,
			#[cfg(feature = "ts")]
			ts_runtime: None,
			plugin_base_dir: None,
		}
	}

	/// Create an SSR proxy with TypeScript backend.
	///
	/// This enables JavaScript/TypeScript SSR using rustyscript (deno_core)
	/// with Preact for React-compatible component rendering.
	///
	/// # Example
	///
	/// ```ignore
	/// use reinhardt_dentdelion::wasm::{TsRuntime, SsrProxy};
	/// use std::sync::Arc;
	///
	/// let ts_runtime = Arc::new(TsRuntime::new()?);
	/// let proxy = SsrProxy::with_ts_runtime(ts_runtime);
	/// assert!(proxy.is_available());
	/// ```
	pub fn with_ts_runtime(runtime: SharedTsRuntime) -> Self {
		Self {
			available: true,
			ts_runtime: Some(runtime),
			plugin_base_dir: None,
		}
	}

	/// Check if SSR is available on the host.
	pub fn is_available(&self) -> bool {
		self.available
	}

	/// Set the base directory for plugin assets.
	///
	/// This directory is used as the root for resolving component file paths
	/// in [`render_react`](Self::render_react).
	///
	/// # Arguments
	///
	/// * `base_dir` - Base directory path for component files
	///
	/// # Security
	///
	/// Component paths are validated against this base directory to prevent
	/// path traversal attacks.
	pub fn with_plugin_base_dir(mut self, base_dir: std::path::PathBuf) -> Self {
		self.plugin_base_dir = Some(base_dir);
		self
	}

	/// Extract CSS and meta tags from rendered HTML.
	///
	/// # Arguments
	///
	/// * `html` - Rendered HTML string
	/// * `options` - Rendering options
	///
	/// # Returns
	///
	/// Tuple of (css: `Option<String>`, meta: `Option<Vec<(String, String)>>`)
	#[cfg(feature = "ts")]
	fn extract_assets(
		html: &str,
		options: &RenderOptions,
	) -> (Option<String>, Option<Vec<(String, String)>>) {
		use scraper::{Html, Selector};

		let document = Html::parse_document(html);

		// Extract CSS if requested
		let css = if options.extract_css {
			let css_selector = Selector::parse("style").unwrap();
			let css_texts: Vec<String> = document
				.select(&css_selector)
				.filter_map(|el| el.text().next().map(|t| t.to_string()))
				.collect();

			if css_texts.is_empty() {
				None
			} else {
				Some(css_texts.join("\n"))
			}
		} else {
			None
		};

		// Extract meta tags if requested
		let meta = if options.extract_meta {
			let meta_selector = Selector::parse("meta").unwrap();
			let meta_tags: Vec<(String, String)> = document
				.select(&meta_selector)
				.filter_map(|el| {
					let name = el
						.value()
						.attr("name")
						.or_else(|| el.value().attr("property"))?;
					let content = el.value().attr("content")?;
					Some((name.to_string(), content.to_string()))
				})
				.collect();

			if meta_tags.is_empty() {
				None
			} else {
				Some(meta_tags)
			}
		} else {
			None
		};

		(css, meta)
	}

	/// Render a component to HTML using inline component code.
	///
	/// This method accepts JavaScript code that defines a `Component` function
	/// and renders it with the provided props.
	///
	/// # Arguments
	///
	/// * `component_code` - JavaScript code defining the component
	/// * `props_json` - JSON string representing component props
	/// * `options` - Rendering options
	///
	/// # Component Code Format
	///
	/// The component code should define a `Component` function using Preact's `h()`:
	///
	/// ```javascript
	/// function Component(props) {
	///     return h('div', { class: 'container' },
	///         h('h1', null, props.title),
	///         h('p', null, props.content)
	///     );
	/// }
	/// ```
	///
	/// # Example
	///
	/// ```ignore
	/// let component = r#"
	///     function Component(props) {
	///         return h('div', null, 'Hello, ' + props.name);
	///     }
	/// "#;
	/// let result = proxy.render_component(
	///     component,
	///     r#"{"name": "World"}"#,
	///     RenderOptions::default(),
	/// )?;
	/// assert!(result.html.contains("Hello, World"));
	/// ```
	pub fn render_component(
		&self,
		component_code: &str,
		props_json: &str,
		options: RenderOptions,
	) -> Result<RenderResult, SsrError> {
		if !self.available {
			return Err(SsrError::NotAvailable);
		}

		// Validate component code for dangerous patterns before execution
		validate_component_code(component_code)?;

		#[cfg(feature = "ts")]
		if let Some(ref runtime) = self.ts_runtime {
			let html = runtime.render_component(component_code, props_json)?;

			// Extract CSS and Meta tags if requested
			let (css, meta) = Self::extract_assets(&html, &options);

			return Ok(RenderResult {
				html,
				css,
				meta,
				hydration_script: if options.include_hydration {
					Some(self.generate_hydration_script(component_code, props_json))
				} else {
					None
				},
			});
		}

		Err(SsrError::NotAvailable)
	}

	/// Render a React component to HTML.
	///
	/// # Arguments
	///
	/// * `component_path` - Path to the component file (relative to plugin assets)
	/// * `props` - MessagePack-serialized component props
	/// * `options` - Rendering options
	///
	/// # Returns
	///
	/// Rendered HTML and optional extracted assets, or an error if rendering fails.
	///
	/// # Note
	///
	/// This method requires loading component code from a file path.
	/// For inline component code, use [`render_component`](Self::render_component).
	pub async fn render_react(
		&self,
		component_path: &str,
		props: &[u8],
		options: RenderOptions,
	) -> Result<RenderResult, SsrError> {
		if !self.available {
			return Err(SsrError::NotAvailable);
		}

		// Check if plugin_base_dir is set
		let base_dir = self.plugin_base_dir.as_ref().ok_or_else(|| {
			SsrError::ComponentNotFound(
				"Plugin base directory not set. Use with_plugin_base_dir()".to_string(),
			)
		})?;

		// Validate component path (security check)
		let validated_path = validate_component_path(base_dir, component_path)?;

		// Load component code from file
		let component_code = tokio::fs::read_to_string(&validated_path)
			.await
			.map_err(|e| {
				SsrError::ComponentNotFound(format!(
					"Failed to read component file {}: {}",
					component_path, e
				))
			})?;

		// Deserialize props from MessagePack
		let props_value: serde_json::Value = rmp_serde::from_slice(props).map_err(|e| {
			SsrError::PropsSerialization(format!("Failed to deserialize props: {}", e))
		})?;

		// Serialize to JSON for render_component
		let props_json = serde_json::to_string(&props_value).map_err(|e| {
			SsrError::PropsSerialization(format!("Failed to serialize props to JSON: {}", e))
		})?;

		// Call render_component with the loaded code
		self.render_component(&component_code, &props_json, options)
	}

	/// Execute arbitrary JavaScript/TypeScript code and return the result.
	///
	/// # Arguments
	///
	/// * `code` - JavaScript/TypeScript code to execute
	///
	/// # Returns
	///
	/// The result as a string, or an error if execution fails.
	///
	/// # Security
	///
	/// This function should only be available to plugins with elevated trust levels.
	pub fn eval_js(&self, code: &str) -> Result<String, SsrError> {
		if !self.available {
			return Err(SsrError::NotAvailable);
		}

		#[cfg(feature = "ts")]
		if let Some(ref runtime) = self.ts_runtime {
			return runtime.eval(code).map_err(SsrError::from);
		}

		Err(SsrError::NotAvailable)
	}

	/// Execute arbitrary JavaScript code and return the result as MessagePack bytes.
	///
	/// # Arguments
	///
	/// * `code` - JavaScript code to execute
	///
	/// # Returns
	///
	/// MessagePack-serialized result, or an error if execution fails.
	///
	/// # Example
	///
	/// ```ignore
	/// let result = proxy.eval_js_bytes("({foo: 'bar', num: 42})").await?;
	/// // result contains MessagePack-encoded {foo: "bar", num: 42}
	/// ```
	pub async fn eval_js_bytes(&self, code: &str) -> Result<Vec<u8>, SsrError> {
		if !self.available {
			return Err(SsrError::NotAvailable);
		}

		#[cfg(feature = "ts")]
		if let Some(ref runtime) = self.ts_runtime {
			// Wrap the code with JSON.stringify() to ensure JSON output
			let json_code = format!("JSON.stringify({})", code);

			// Evaluate JavaScript code and get JSON result
			let json_result = runtime.eval(&json_code)?;

			// Parse JSON string to serde_json::Value
			let value: serde_json::Value = serde_json::from_str(&json_result)
				.map_err(|e| SsrError::EvalFailed(format!("Failed to parse JSON result: {}", e)))?;

			// Serialize to MessagePack bytes
			let msgpack_bytes = rmp_serde::to_vec(&value).map_err(|e| {
				SsrError::EvalFailed(format!("Failed to serialize to MessagePack: {}", e))
			})?;

			return Ok(msgpack_bytes);
		}

		Err(SsrError::NotAvailable)
	}

	/// Generate a hydration script for client-side takeover.
	///
	/// This creates a `<script>` tag that will re-render the component
	/// on the client side for interactivity.
	///
	/// # Security
	///
	/// Both `component_code` and `props_json` are escaped to prevent XSS attacks
	/// via `</script>` tag injection. See [`escape_for_script`] for details.
	fn generate_hydration_script(&self, component_code: &str, props_json: &str) -> String {
		let safe_code = escape_for_script(component_code);
		let safe_props = escape_for_script(props_json);
		format!(
			r#"<script type="module">
import {{ h, render }} from 'https://esm.sh/preact@10';

{safe_code}

const props = {safe_props};
const root = document.getElementById('root');
if (root) {{
    render(h(Component, props), root);
}}
</script>"#
		)
	}
}

/// Escape content for safe embedding inside a `<script>` tag.
///
/// This function prevents XSS attacks by escaping the `</script>` sequence,
/// which would otherwise terminate the script tag and allow arbitrary HTML/JS injection.
///
/// # Security
///
/// The browser's HTML parser looks for `</script>` (case-insensitive) to determine
/// the end of a script tag, regardless of JavaScript string context. An attacker
/// who can inject `</script><script>malicious()</script>` into `props_json` could
/// execute arbitrary JavaScript.
///
/// # Examples
///
/// ```
/// use reinhardt_dentdelion::wasm::ssr::escape_for_script;
///
/// // Malicious input is escaped
/// let malicious = r#"{"name": "</script><script>alert(1)</script>"}"#;
/// let escaped = escape_for_script(malicious);
/// assert!(!escaped.contains("</script>"));
/// assert!(escaped.contains("<\\/script>"));
///
/// // Normal content is unchanged
/// let normal = r#"{"name": "Alice"}"#;
/// assert_eq!(escape_for_script(normal), normal);
/// ```
pub fn escape_for_script(s: &str) -> String {
	// Escape </script> (case-insensitive variants) to prevent XSS
	// The escaped version <\/script> is safe because:
	// 1. HTML parser doesn't recognize it as a closing tag
	// 2. JavaScript treats \/ as just / in string literals
	let mut result = s.to_string();

	// Replace common case variations of </script>
	// Order matters: replace specific patterns before generic ones
	result = result.replace("</script>", "<\\/script>");
	result = result.replace("</SCRIPT>", "<\\/SCRIPT>");
	result = result.replace("</Script>", "<\\/Script>");

	result
}

/// Shared SSR proxy instance type.
pub type SharedSsrProxy = std::sync::Arc<SsrProxy>;

/// Dangerous JavaScript patterns that should not appear in component code.
///
/// These patterns indicate attempts to access host system resources,
/// load external modules, or execute system commands from within
/// a component rendering context.
const DANGEROUS_PATTERNS: &[(&str, &str)] = &[
	("require(", "Node.js module loading via require()"),
	("require (", "Node.js module loading via require()"),
	("import(", "Dynamic import via import()"),
	("import (", "Dynamic import via import()"),
	("process.", "Node.js process access"),
	("Deno.", "Deno runtime access"),
	("__filename", "Node.js file system path access"),
	("__dirname", "Node.js directory path access"),
	("child_process", "Node.js child process execution"),
	("execSync", "Synchronous command execution"),
	("spawnSync", "Synchronous process spawning"),
	// Block dynamic code construction
	("Function(", "Dynamic function construction"),
	("Function (", "Dynamic function construction"),
];

/// Validate component code for dangerous patterns.
///
/// This function scans component code for patterns that could indicate
/// attempts to escape the rendering sandbox or access host resources.
///
/// # Arguments
///
/// * `code` - JavaScript component code to validate
///
/// # Returns
///
/// `Ok(())` if the code is safe, or `Err(SsrError::DangerousPattern)` if
/// a dangerous pattern is detected.
fn validate_component_code(code: &str) -> Result<(), SsrError> {
	for (pattern, description) in DANGEROUS_PATTERNS {
		if code.contains(pattern) {
			return Err(SsrError::DangerousPattern(format!(
				"{} (found '{}')",
				description, pattern
			)));
		}
	}
	Ok(())
}

/// Validate component file path for security.
///
/// This function implements a three-layer defense against path traversal attacks:
///
/// 1. **Extension whitelist** - Only `.js`, `.jsx`, `.ts`, `.tsx` files are accepted,
///    preventing loading of arbitrary file types (e.g., `/etc/passwd`).
///
/// 2. **Path canonicalization** - `canonicalize()` resolves all `..` components and
///    symlinks to produce an absolute path. This handles attacks like
///    `../../etc/passwd.js` by resolving to the actual filesystem path. Note that
///    `canonicalize()` also verifies the file exists, addressing TOCTOU concerns
///    by performing the existence check and path resolution atomically.
///
/// 3. **Base directory containment** - `starts_with()` on the canonicalized path
///    ensures the resolved file is within the allowed base directory, even if
///    symlinks or `..` components were used in the original path.
///
/// # Arguments
///
/// * `base_dir` - Base directory for plugin assets
/// * `component_path` - Relative component file path
///
/// # Returns
///
/// Canonical absolute path to the component file, or an error if validation fails.
fn validate_component_path(
	base_dir: &std::path::Path,
	component_path: &str,
) -> Result<std::path::PathBuf, SsrError> {
	// Layer 1: Extension whitelist prevents loading arbitrary file types
	let allowed_extensions = [".js", ".jsx", ".ts", ".tsx"];
	let has_valid_extension = allowed_extensions
		.iter()
		.any(|ext| component_path.ends_with(ext));

	if !has_valid_extension {
		return Err(SsrError::ComponentNotFound(format!(
			"Invalid file extension. Only .js, .jsx, .ts, .tsx are allowed: {}",
			component_path
		)));
	}

	// Construct full path
	let full_path = base_dir.join(component_path);

	// Layer 2: Canonicalize resolves ".." and symlinks, also verifies file exists
	let canonical_path = full_path
		.canonicalize()
		.map_err(|e| SsrError::ComponentNotFound(format!("Component file not found: {}", e)))?;

	// Layer 3: Ensure canonical path is within base_dir (containment check)
	let canonical_base = base_dir
		.canonicalize()
		.map_err(|e| SsrError::ComponentNotFound(format!("Base directory not found: {}", e)))?;

	if !canonical_path.starts_with(&canonical_base) {
		return Err(SsrError::ComponentNotFound(format!(
			"Path traversal attack detected: {} is outside base directory",
			component_path
		)));
	}

	Ok(canonical_path)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_ssr_proxy_unavailable_by_default() {
		let proxy = SsrProxy::new();
		assert!(!proxy.is_available());
	}

	#[test]
	fn test_ssr_proxy_with_availability() {
		let proxy = SsrProxy::with_availability(true);
		assert!(proxy.is_available());

		let proxy = SsrProxy::with_availability(false);
		assert!(!proxy.is_available());
	}

	#[tokio::test]
	async fn test_render_react_returns_not_available() {
		let proxy = SsrProxy::new();
		let result = proxy
			.render_react("test.jsx", &[], RenderOptions::default())
			.await;
		assert!(matches!(result, Err(SsrError::NotAvailable)));
	}

	#[test]
	fn test_validate_component_path_invalid_extension() {
		use std::path::PathBuf;
		let base_dir = PathBuf::from("/tmp");
		let result = validate_component_path(&base_dir, "component.py");
		assert!(matches!(result, Err(SsrError::ComponentNotFound(_))));
	}

	#[test]
	fn test_validate_component_path_valid_extensions() {
		use std::fs;
		use tempfile::TempDir;

		let temp_dir = TempDir::new().unwrap();
		let base_path = temp_dir.path();

		// Create test files with valid extensions
		for ext in [".js", ".jsx", ".ts", ".tsx"] {
			let filename = format!("component{}", ext);
			let file_path = base_path.join(&filename);
			fs::write(&file_path, "// test").unwrap();

			let result = validate_component_path(base_path, &filename);
			assert!(result.is_ok());
		}
	}

	#[test]
	fn test_validate_component_path_prevents_traversal() {
		use std::path::PathBuf;
		let base_dir = PathBuf::from("/tmp/plugin_assets");
		// Path traversal attack should fail during canonicalization
		// (file doesn't exist, so canonicalize will fail)
		let result = validate_component_path(&base_dir, "../../../etc/passwd.js");
		assert!(result.is_err());
	}

	#[test]
	fn test_eval_js_returns_not_available() {
		let proxy = SsrProxy::new();
		let result = proxy.eval_js("console.log('test')");
		assert!(matches!(result, Err(SsrError::NotAvailable)));
	}

	#[test]
	fn test_render_options_default() {
		let options = RenderOptions::default();
		assert!(!options.include_hydration);
		assert!(!options.extract_css);
		assert!(!options.extract_meta);
	}

	#[test]
	fn test_render_result_structure() {
		let result = RenderResult {
			html: "<div>Hello</div>".to_string(),
			css: Some(".container { color: red; }".to_string()),
			meta: Some(vec![("title".to_string(), "My Page".to_string())]),
			hydration_script: Some("<script>hydrate()</script>".to_string()),
		};

		assert_eq!(result.html, "<div>Hello</div>");
		assert!(result.css.is_some());
		assert!(result.meta.is_some());
		assert!(result.hydration_script.is_some());
	}

	#[test]
	fn test_ssr_error_display() {
		let err = SsrError::NotAvailable;
		assert!(err.to_string().contains("not available"));

		let err = SsrError::ComponentNotFound("Header.jsx".to_string());
		assert!(err.to_string().contains("Header.jsx"));

		let err = SsrError::RenderFailed("syntax error".to_string());
		assert!(err.to_string().contains("syntax error"));

		let err = SsrError::PermissionDenied("insufficient trust level".to_string());
		assert!(err.to_string().contains("Permission denied"));

		let err = SsrError::DangerousPattern("Node.js module loading".to_string());
		assert!(err.to_string().contains("Dangerous code pattern"));
	}

	#[test]
	fn test_render_component_without_runtime() {
		let proxy = SsrProxy::new();
		let result = proxy.render_component(
			"function Component(props) { return h('div', null, 'test'); }",
			"{}",
			RenderOptions::default(),
		);
		assert!(matches!(result, Err(SsrError::NotAvailable)));
	}

	// =========================================================================
	// Component Code Validation Tests (Issue #677)
	// =========================================================================

	#[test]
	fn test_validate_safe_component_code() {
		let safe_code = r#"
			function Component(props) {
				return h('div', null, 'Hello, ' + props.name);
			}
		"#;
		assert!(validate_component_code(safe_code).is_ok());
	}

	#[test]
	fn test_validate_rejects_require() {
		let code = "var fs = require('fs');";
		assert!(matches!(
			validate_component_code(code),
			Err(SsrError::DangerousPattern(_))
		));
	}

	#[test]
	fn test_validate_rejects_dynamic_import() {
		let code = "var mod = import('os');";
		assert!(matches!(
			validate_component_code(code),
			Err(SsrError::DangerousPattern(_))
		));
	}

	#[test]
	fn test_validate_rejects_process_access() {
		let code = "var key = process.env.SECRET;";
		assert!(matches!(
			validate_component_code(code),
			Err(SsrError::DangerousPattern(_))
		));
	}

	#[test]
	fn test_validate_rejects_deno_access() {
		let code = "Deno.readTextFileSync('/etc/passwd');";
		assert!(matches!(
			validate_component_code(code),
			Err(SsrError::DangerousPattern(_))
		));
	}

	#[test]
	fn test_validate_rejects_filename_dirname() {
		let code_filename = "console.log(__filename);";
		let code_dirname = "console.log(__dirname);";
		assert!(matches!(
			validate_component_code(code_filename),
			Err(SsrError::DangerousPattern(_))
		));
		assert!(matches!(
			validate_component_code(code_dirname),
			Err(SsrError::DangerousPattern(_))
		));
	}

	#[test]
	fn test_validate_rejects_exec_sync() {
		let code = "execSync('ls -la');";
		assert!(matches!(
			validate_component_code(code),
			Err(SsrError::DangerousPattern(_))
		));
	}

	#[test]
	fn test_validate_rejects_function_constructor() {
		let code = r#"var fn = Function("return 1");"#;
		assert!(matches!(
			validate_component_code(code),
			Err(SsrError::DangerousPattern(_))
		));
	}

	#[test]
	fn test_render_component_rejects_dangerous_code() {
		let proxy = SsrProxy::with_availability(true);
		let dangerous_code = "var fs = require('fs'); function Component() { return h('div'); }";
		let result = proxy.render_component(dangerous_code, "{}", RenderOptions::default());
		assert!(matches!(result, Err(SsrError::DangerousPattern(_))));
	}

	#[tokio::test]
	async fn test_eval_js_bytes_without_runtime() {
		let proxy = SsrProxy::new();
		let result = proxy.eval_js_bytes("({foo: 'bar'})").await;
		assert!(matches!(result, Err(SsrError::NotAvailable)));
	}

	// ===== XSS Prevention Tests =====

	#[test]
	fn test_escape_for_script_prevents_script_tag_injection() {
		// Test the most common attack vector
		let malicious = r#"{"name": "</script><script>alert(1)</script>"}"#;
		let escaped = escape_for_script(malicious);

		// The escaped string should NOT contain unescaped </script>
		assert!(!escaped.contains("</script>"));

		// It should contain the escaped version
		assert!(escaped.contains("<\\/script>"));
	}

	#[test]
	fn test_escape_for_script_handles_uppercase() {
		let malicious = r#"{"name": "</SCRIPT><script>alert(1)</script>"}"#;
		let escaped = escape_for_script(malicious);

		assert!(!escaped.contains("</SCRIPT>"));
		assert!(escaped.contains("<\\/SCRIPT>"));
	}

	#[test]
	fn test_escape_for_script_handles_mixed_case() {
		let malicious = r#"{"name": "</Script><script>alert(1)</script>"}"#;
		let escaped = escape_for_script(malicious);

		assert!(!escaped.contains("</Script>"));
		assert!(escaped.contains("<\\/Script>"));
	}

	#[test]
	fn test_escape_for_script_preserves_normal_content() {
		let normal = r#"{"name": "Alice", "age": 30}"#;
		let escaped = escape_for_script(normal);

		assert_eq!(escaped, normal);
	}

	#[test]
	fn test_escape_for_script_handles_multiple_occurrences() {
		let malicious = r#"</script></script></SCRIPT>"#;
		let escaped = escape_for_script(malicious);

		assert!(!escaped.contains("</script>"));
		assert!(!escaped.contains("</SCRIPT>"));
		assert!(escaped.matches("<\\/script>").count() == 2);
		assert!(escaped.matches("<\\/SCRIPT>").count() == 1);
	}

	#[test]
	fn test_escape_for_script_empty_string() {
		assert_eq!(escape_for_script(""), "");
	}

	#[test]
	fn test_generate_hydration_script_escapes_malicious_props() {
		let proxy = SsrProxy::new();
		let component = r#"function Component(props) { return h('div', null, props.name); }"#;
		let malicious_props = r#"{"name": "</script><script>alert(document.cookie)</script>"}"#;

		let script = proxy.generate_hydration_script(component, malicious_props);

		// The malicious </script> should be escaped
		assert!(script.contains("<\\/script>"));

		// The script should still be valid HTML with exactly one closing tag at the end
		assert!(script.starts_with("<script type=\"module\">"));
		assert!(script.ends_with("</script>"));

		// Count the number of </script> occurrences - should be exactly 1 (the closing tag)
		let closing_tag_count = script.matches("</script>").count();
		assert_eq!(
			closing_tag_count, 1,
			"Expected exactly 1 closing </script> tag"
		);
	}

	#[test]
	fn test_generate_hydration_script_escapes_malicious_code() {
		let proxy = SsrProxy::new();
		// Malicious component code trying to break out
		let malicious_code =
			r#"function Component() { return '</script><script>alert(1)</script>'; }"#;
		let props = "{}";

		let script = proxy.generate_hydration_script(malicious_code, props);

		// The malicious </script> should be escaped
		assert!(script.contains("<\\/script>"));

		// Count the number of </script> occurrences - should be exactly 1 (the closing tag)
		let closing_tag_count = script.matches("</script>").count();
		assert_eq!(
			closing_tag_count, 1,
			"Expected exactly 1 closing </script> tag"
		);
	}

	#[cfg(feature = "ts")]
	mod ts_tests {
		use super::*;
		use crate::wasm::ts_runtime::TsRuntime;
		use std::sync::Arc;

		#[test]
		fn test_ssr_proxy_with_ts_runtime() {
			let runtime = Arc::new(TsRuntime::new().unwrap());
			let proxy = SsrProxy::with_ts_runtime(runtime);
			assert!(proxy.is_available());
		}

		#[test]
		fn test_render_simple_component_with_ts_runtime() {
			let runtime = Arc::new(TsRuntime::new().unwrap());
			let proxy = SsrProxy::with_ts_runtime(runtime);

			let component = r#"
                function Component(props) {
                    return h('div', null, 'Hello, ' + props.name);
                }
            "#;
			let result =
				proxy.render_component(component, r#"{"name": "World"}"#, RenderOptions::default());

			assert!(result.is_ok());
			let render_result = result.unwrap();
			assert!(render_result.html.contains("Hello, World"));
			assert!(render_result.html.contains("<div>"));
		}

		#[test]
		fn test_eval_js_with_ts_runtime() {
			let runtime = Arc::new(TsRuntime::new().unwrap());
			let proxy = SsrProxy::with_ts_runtime(runtime);

			let result = proxy.eval_js("'Hello, ' + 'World'");
			assert!(result.is_ok());
			assert_eq!(result.unwrap(), "Hello, World");
		}

		#[test]
		fn test_hydration_script_generation() {
			let runtime = Arc::new(TsRuntime::new().unwrap());
			let proxy = SsrProxy::with_ts_runtime(runtime);

			let component = r#"
                function Component(props) {
                    return h('div', null, props.message);
                }
            "#;
			let options = RenderOptions {
				include_hydration: true,
				..Default::default()
			};
			let result = proxy.render_component(component, r#"{"message": "Hi"}"#, options);

			assert!(result.is_ok());
			let render_result = result.unwrap();
			assert!(render_result.hydration_script.is_some());
			let script = render_result.hydration_script.unwrap();
			assert!(script.contains("preact"));
			assert!(script.contains("Component"));
		}

		#[tokio::test]
		async fn test_eval_js_bytes_simple_object() {
			let runtime = Arc::new(TsRuntime::new().unwrap());
			let proxy = SsrProxy::with_ts_runtime(runtime);

			let result = proxy.eval_js_bytes("({foo: 'bar', num: 42})").await;
			assert!(result.is_ok());

			let bytes = result.unwrap();
			// Deserialize MessagePack to verify correctness
			let value: serde_json::Value = rmp_serde::from_slice(&bytes).unwrap();
			assert_eq!(value["foo"], "bar");
			assert_eq!(value["num"], 42);
		}

		#[tokio::test]
		async fn test_eval_js_bytes_complex_object() {
			let runtime = Arc::new(TsRuntime::new().unwrap());
			let proxy = SsrProxy::with_ts_runtime(runtime);

			let code = r#"
                ({
                    nested: {
                        array: [1, 2, 3],
                        bool: true
                    },
                    str: 'hello'
                })
            "#;
			let result = proxy.eval_js_bytes(code).await;
			assert!(result.is_ok());

			let bytes = result.unwrap();
			let value: serde_json::Value = rmp_serde::from_slice(&bytes).unwrap();
			assert_eq!(value["nested"]["array"][0], 1);
			assert_eq!(value["nested"]["array"][1], 2);
			assert_eq!(value["nested"]["array"][2], 3);
			assert_eq!(value["nested"]["bool"], true);
			assert_eq!(value["str"], "hello");
		}

		#[tokio::test]
		async fn test_eval_js_bytes_invalid_json() {
			let runtime = Arc::new(TsRuntime::new().unwrap());
			let proxy = SsrProxy::with_ts_runtime(runtime);

			// undefined returns "undefined" string which is not valid JSON
			let result = proxy.eval_js_bytes("undefined").await;
			assert!(result.is_err());
			assert!(matches!(result, Err(SsrError::EvalFailed(_))));
		}

		#[test]
		fn test_css_extraction() {
			let runtime = Arc::new(TsRuntime::new().unwrap());
			let proxy = SsrProxy::with_ts_runtime(runtime);

			let component = r#"
                function Component() {
                    return h('div', null,
                        h('style', null, '.test { color: red; }'),
                        h('p', null, 'Hello')
                    );
                }
            "#;

			let options = RenderOptions {
				extract_css: true,
				..Default::default()
			};

			let result = proxy.render_component(component, "{}", options);
			assert!(result.is_ok());

			let render_result = result.unwrap();
			assert!(render_result.css.is_some());
			let css = render_result.css.unwrap();
			assert!(css.contains(".test { color: red; }"));
		}

		#[test]
		fn test_meta_extraction() {
			let runtime = Arc::new(TsRuntime::new().unwrap());
			let proxy = SsrProxy::with_ts_runtime(runtime);

			let component = r#"
                function Component() {
                    return h('div', null,
                        h('meta', { name: 'description', content: 'Test description' }),
                        h('p', null, 'Content')
                    );
                }
            "#;

			let options = RenderOptions {
				extract_meta: true,
				..Default::default()
			};

			let result = proxy.render_component(component, "{}", options);
			assert!(result.is_ok());

			let render_result = result.unwrap();
			assert!(render_result.meta.is_some());
			let meta = render_result.meta.unwrap();
			assert!(meta.contains(&("description".to_string(), "Test description".to_string())));
		}

		#[test]
		fn test_no_extraction_when_disabled() {
			let runtime = Arc::new(TsRuntime::new().unwrap());
			let proxy = SsrProxy::with_ts_runtime(runtime);

			let component = r#"
                function Component() {
                    return h('div', null,
                        h('style', null, '.test { color: red; }'),
                        h('meta', { name: 'description', content: 'Test' }),
                        h('p', null, 'Content')
                    );
                }
            "#;

			// Default options (no extraction)
			let result = proxy.render_component(component, "{}", RenderOptions::default());
			assert!(result.is_ok());

			let render_result = result.unwrap();
			assert!(render_result.css.is_none());
			assert!(render_result.meta.is_none());
		}
	}
}

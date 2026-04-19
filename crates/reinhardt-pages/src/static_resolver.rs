//! Static File URL Resolver
//!
//! This module provides a global static file URL resolver that integrates with
//! the Reinhardt static files system. It allows components to resolve static
//! file paths to their final URLs, supporting manifest-based hashing for
//! cache busting.
//!
//! ## Usage
//!
//! ### Initialization
//!
//! The static resolver must be initialized before use, typically in your
//! server startup code:
//!
//! ```ignore
//! use reinhardt_pages::static_resolver::init_static_resolver;
//! use reinhardt_utils::staticfiles::TemplateStaticConfig;
//!
//! // Initialize with basic configuration
//! init_static_resolver(TemplateStaticConfig::new("/static/".to_string()));
//!
//! // Or with manifest for cache-busted filenames
//! let manifest = load_manifest("staticfiles.json").await?;
//! init_static_resolver(
//!     TemplateStaticConfig::new("/static/".to_string())
//!         .with_manifest(manifest)
//! );
//! ```
//!
//! ### Resolving URLs
//!
//! Once initialized, use `resolve_static` to get the final URL:
//!
//! ```no_run
//! use reinhardt_pages::static_resolver::resolve_static;
//!
//! let css_url = resolve_static("css/style.css");
//! // Returns: "/static/css/style.css"
//! // Or with manifest: "/static/css/style.abc123.css"
//!
//! let js_url = resolve_static("js/app.js");
//! // Returns: "/static/js/app.js"
//! ```
//!
//! ### Using with head! macro
//!
//! ```ignore
//! use reinhardt_pages::{head, static_resolver::resolve_static};
//!
//! let my_head = head!(|| {
//!     link { rel: "stylesheet", href: resolve_static("css/style.css") }
//!     script { src: resolve_static("js/app.js"), defer }
//! });
//! ```
//!
//! ## Using with page! Macro
//!
//! The `resolve_static` function works seamlessly within `page!` macros:
//!
//! ```ignore
//! use reinhardt_pages::{page, static_resolver::resolve_static};
//!
//! // Simple static image
//! page!(|| {
//!     img {
//!         src: resolve_static("images/logo.png"),
//!         alt: "Logo"
//!     }
//! })()
//!
//! // Dynamic path based on state
//! page!(|user_id: i64| {
//!     let avatar = format!("images/avatars/user_{}.png", user_id);
//!     img {
//!         src: resolve_static(&avatar),
//!         alt: "Avatar"
//!     }
//! })(user_id)
//! ```
//!
//! ## Using with head! Macro for SSR
//!
//! For server-side rendering scenarios, use `resolve_static` with `head!`:
//!
//! ```ignore
//! use reinhardt_pages::{head, static_resolver::resolve_static};
//! use reinhardt_pages::ssr::SsrRenderer;
//!
//! let page_head = head!(|| {
//!     link { rel: "stylesheet", href: resolve_static("css/app.css") }
//!     script { src: resolve_static("js/app.js"), defer }
//! });
//!
//! let mut renderer = SsrRenderer::new();
//! let html = renderer.render_page_with_head(view, page_head);
//! ```
//!
//! ## Best Practices
//!
//! ### 1. Initialize Early
//!
//! Always initialize the static resolver during application startup,
//! before rendering any components:
//!
//! ```ignore
//! #[tokio::main]
//! async fn main() {
//!     // Initialize static resolver FIRST
//!     init_static_resolver(TemplateStaticConfig::new("/static/".to_string()));
//!
//!     // Then start server
//!     let app = create_application();
//!     app.run().await;
//! }
//! ```
//!
//! ### 2. Use Manifest for Production
//!
//! In production, always use a manifest for cache-busted URLs:
//!
//! ```ignore
//! // Load manifest from collectstatic output
//! let manifest = load_manifest("staticfiles/manifest.json").await?;
//! init_static_resolver(
//!     TemplateStaticConfig::new("/static/".to_string())
//!         .with_manifest(manifest)
//! );
//! ```
//!
//! ### 3. Prefer Static Paths When Possible
//!
//! Use static strings for fixed assets to enable potential future
//! compile-time optimizations:
//!
//! ```ignore
//! // ✅ Good - static path
//! img { src: resolve_static("images/logo.png") }
//!
//! // ⚠️ Use only when necessary - dynamic path
//! let path = format!("images/{}.png", name);
//! img { src: resolve_static(&path) }
//! ```
//!
//! ### 4. Avoid Hardcoding URLs
//!
//! Never hardcode static URLs; always use `resolve_static`:
//!
//! ```ignore
//! // ❌ Bad - breaks with CDN or STATIC_URL changes
//! img { src: "/static/images/logo.png" }
//!
//! // ✅ Good - respects configuration
//! img { src: resolve_static("images/logo.png") }
//! ```
//!
//! ## Compile-Time vs Runtime Resolution
//!
//! ### Runtime Resolution (resolve_static)
//!
//! - **When to use**: All current use cases in reinhardt-pages
//! - **Pros**: Flexible, works with dynamic paths, integrates with manifest
//! - **Cons**: Small runtime overhead (negligible in practice)
//!
//! ### Future: Compile-Time Resolution
//!
//! A future `static_url!` macro may provide compile-time resolution:
//!
//! ```ignore
//! // Future API (not yet implemented)
//! img { src: static_url!("images/logo.png") }
//! // Resolved at compile time to "/static/images/logo.png"
//! ```
//!
//! **Benefits**:
//! - Zero runtime overhead
//! - Compile-time path validation
//! - Better CDN integration
//!
//! For now, use `resolve_static()` for all static URL resolution needs.
//!
//! ## Thread Safety
//!
//! The static resolver uses `OnceLock` for thread-safe lazy initialization.
//! It can only be initialized once per application lifecycle.

use std::sync::OnceLock;

#[cfg(native)]
use reinhardt_utils::staticfiles::TemplateStaticConfig;

/// Global static configuration storage.
///
/// This is initialized once at application startup and provides
/// thread-safe access to the static URL resolver.
#[cfg(native)]
static STATIC_CONFIG: OnceLock<TemplateStaticConfig> = OnceLock::new();

/// WASM-specific static URL prefix.
///
/// In WASM environments, we use a simple prefix since there's no
/// server-side manifest processing.
#[cfg(wasm)]
static STATIC_URL_PREFIX: OnceLock<String> = OnceLock::new();

/// Initializes the static resolver with the given configuration.
///
/// This function should be called once during application startup,
/// before any calls to `resolve_static`.
///
/// ## Example
///
/// ```ignore
/// use reinhardt_pages::static_resolver::init_static_resolver;
/// use reinhardt_utils::staticfiles::TemplateStaticConfig;
///
/// // Basic initialization
/// init_static_resolver(TemplateStaticConfig::new("/static/".to_string()));
/// ```
///
/// ## Panics
///
/// This function does not panic if called multiple times; subsequent
/// calls are silently ignored.
#[cfg(native)]
pub fn init_static_resolver(config: TemplateStaticConfig) {
	// OnceLock::set returns Err if already set, but we ignore this
	// to allow for idempotent initialization
	let _ = STATIC_CONFIG.set(config);
}

/// Initializes the static resolver with a URL prefix (WASM version).
///
/// In WASM environments, static files are typically served from a
/// fixed prefix without manifest-based hashing.
///
/// ## Example
///
/// ```no_run
/// use reinhardt_pages::static_resolver::init_static_resolver;
///
/// init_static_resolver("/static/".to_string());
/// ```
#[cfg(wasm)]
pub fn init_static_resolver(static_url: String) {
	let _ = STATIC_URL_PREFIX.set(static_url);
}

/// Resolves a static file path to its final URL.
///
/// This function takes a relative path to a static file and returns
/// the full URL that should be used in HTML. If a manifest is configured,
/// it will use the hashed filename for cache busting.
///
/// ## Arguments
///
/// * `path` - The relative path to the static file (e.g., "css/style.css")
///
/// ## Returns
///
/// The resolved URL (e.g., "/static/css/style.abc123.css")
///
/// ## Example
///
/// ```no_run
/// use reinhardt_pages::static_resolver::resolve_static;
///
/// let css_url = resolve_static("css/style.css");
/// // Returns: "/static/css/style.css"
///
/// let favicon = resolve_static("favicon.png");
/// // Returns: "/static/favicon.png"
/// ```
///
/// ## Fallback Behavior
///
/// If the static resolver has not been initialized:
/// - A warning is logged to stderr
/// - The function returns a fallback URL using "/static/" prefix
///
/// This ensures the application continues to work even if initialization
/// was missed, though cache busting won't be available.
#[cfg(native)]
pub fn resolve_static(path: &str) -> String {
	STATIC_CONFIG
		.get()
		.map(|config| config.resolve_url(path))
		.unwrap_or_else(|| {
			// Only warn once to avoid log spam
			static WARNED: std::sync::atomic::AtomicBool =
				std::sync::atomic::AtomicBool::new(false);
			if !WARNED.swap(true, std::sync::atomic::Ordering::SeqCst) {
				eprintln!(
					"WARNING: Static resolver not initialized. Call init_static_resolver() at startup."
				);
			}

			// Fallback: use simple concatenation
			let path = path.trim_start_matches('/');
			format!("/static/{}", path)
		})
}

/// Resolves a static file path to its URL (WASM version).
///
/// In WASM environments, this simply concatenates the configured
/// prefix with the path.
#[cfg(wasm)]
pub fn resolve_static(path: &str) -> String {
	let prefix = STATIC_URL_PREFIX
		.get()
		.map(|s| s.as_str())
		.unwrap_or("/static/");

	let prefix = prefix.trim_end_matches('/');
	let path = path.trim_start_matches('/');
	format!("{}/{}", prefix, path)
}

/// Checks if the static resolver has been initialized.
///
/// This can be useful for debugging or for conditional initialization.
///
/// ## Example
///
/// ```ignore
/// use reinhardt_pages::static_resolver::{is_initialized, init_static_resolver};
///
/// if !is_initialized() {
///     init_static_resolver(config);
/// }
/// ```
#[cfg(native)]
pub fn is_initialized() -> bool {
	STATIC_CONFIG.get().is_some()
}

/// Checks if the static resolver has been initialized (WASM version).
#[cfg(wasm)]
pub fn is_initialized() -> bool {
	STATIC_URL_PREFIX.get().is_some()
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[cfg(native)]
	mod native_tests {
		use super::*;

		#[rstest]
		fn test_resolve_static_fallback() {
			// This test relies on the resolver not being initialized in this test environment
			// Note: In a real app, we'd test with proper initialization
			let url = resolve_static("test.css");
			assert!(url.contains("test.css"));
		}

		#[rstest]
		fn test_resolve_static_strips_leading_slash() {
			let url = resolve_static("/test.css");
			assert!(url.contains("test.css"));
			// Should not have double slashes
			assert!(!url.contains("//static"));
		}
	}

	#[cfg(wasm)]
	mod wasm_tests {
		use super::*;
		use wasm_bindgen_test::*;

		#[wasm_bindgen_test]
		fn test_resolve_static_wasm() {
			let url = resolve_static("test.css");
			assert!(url.contains("test.css"));
		}
	}
}

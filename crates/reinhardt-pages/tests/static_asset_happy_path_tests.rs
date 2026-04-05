#![cfg(not(target_arch = "wasm32"))]
//! Happy path tests for static asset URL resolution
//!
//! These tests verify the normal operation of the static resolver when
//! inputs are correct and the system operates as designed.

#[cfg(native)]
mod happy_path_tests {
	use reinhardt_pages::static_resolver::{init_static_resolver, resolve_static};
	use reinhardt_utils::staticfiles::TemplateStaticConfig;
	use rstest::rstest;
	use std::collections::HashMap;

	/// Test basic path resolution with default configuration
	#[rstest]
	fn test_resolve_static_basic_css() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static("css/style.css");

		assert_eq!(result, "/static/css/style.css");
	}

	/// Test JavaScript file resolution
	#[rstest]
	fn test_resolve_static_javascript() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static("js/app.js");

		assert_eq!(result, "/static/js/app.js");
	}

	/// Test image file resolution
	#[rstest]
	fn test_resolve_static_image() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static("images/logo.png");

		assert_eq!(result, "/static/images/logo.png");
	}

	/// Test font file resolution
	#[rstest]
	fn test_resolve_static_font() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static("fonts/roboto.woff2");

		assert_eq!(result, "/static/fonts/roboto.woff2");
	}

	/// Test manifest-based hash resolution
	#[rstest]
	fn test_resolve_static_with_manifest_hash() {
		let mut manifest = HashMap::new();
		manifest.insert(
			"css/style.css".to_string(),
			"css/style.abc123def456.css".to_string(),
		);
		manifest.insert(
			"js/app.js".to_string(),
			"js/app.xyz789uvw012.js".to_string(),
		);

		let config = TemplateStaticConfig::new("/static/".to_string()).with_manifest(manifest);
		init_static_resolver(config);

		let css_result = resolve_static("css/style.css");
		let js_result = resolve_static("js/app.js");

		assert_eq!(css_result, "/static/css/style.abc123def456.css");
		assert_eq!(js_result, "/static/js/app.xyz789uvw012.js");
	}

	/// Test multiple file types resolution
	#[rstest]
	#[case("style.css", "css")]
	#[case("script.js", "js")]
	#[case("image.png", "png")]
	#[case("image.jpg", "jpg")]
	#[case("image.svg", "svg")]
	#[case("font.woff2", "woff2")]
	#[case("config.json", "json")]
	fn test_resolve_static_various_file_types(#[case] filename: &str, #[case] expected_ext: &str) {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static(filename);

		assert!(
			result.ends_with(expected_ext),
			"Result '{}' should end with '{}' (from file '{}')",
			result,
			expected_ext,
			filename
		);
		assert!(result.contains(filename));
	}

	/// Test dynamic path resolution
	#[rstest]
	fn test_resolve_static_dynamic_path() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let user_id = 42;
		let theme = "dark";
		let path = format!("images/themes/{}/user-{}.png", theme, user_id);

		let result = resolve_static(&path);

		assert!(result.contains("themes/dark"));
		assert!(result.contains("user-42.png"));
		assert_eq!(result, "/static/images/themes/dark/user-42.png");
	}

	/// Test with CDN base URL
	#[rstest]
	fn test_resolve_static_with_cdn_url() {
		let config = TemplateStaticConfig::new("https://cdn.example.com/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static("css/style.css");

		assert_eq!(result, "https://cdn.example.com/static/css/style.css");
	}

	/// Test with relative base URL
	#[rstest]
	fn test_resolve_static_with_relative_url() {
		let config = TemplateStaticConfig::new("/assets/".to_string());
		init_static_resolver(config);

		let result = resolve_static("images/logo.png");

		assert_eq!(result, "/assets/images/logo.png");
	}

	/// Test manifest fallback for missing files
	#[rstest]
	fn test_resolve_static_manifest_fallback_for_missing_files() {
		let mut manifest = HashMap::new();
		manifest.insert(
			"css/known.css".to_string(),
			"css/known.abc123.css".to_string(),
		);

		let config = TemplateStaticConfig::new("/static/".to_string()).with_manifest(manifest);
		init_static_resolver(config);

		let known_result = resolve_static("css/known.css");
		let unknown_result = resolve_static("css/unknown.css");

		assert_eq!(known_result, "/static/css/known.abc123.css");
		// Unknown files should fallback to original path
		assert_eq!(unknown_result, "/static/css/unknown.css");
	}

	/// Test directory nesting
	#[rstest]
	fn test_resolve_static_deep_nesting() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static("app/components/ui/buttons/primary.css");

		assert_eq!(result, "/static/app/components/ui/buttons/primary.css");
	}

	/// Test with query string preservation
	#[rstest]
	fn test_resolve_static_with_query_string() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static("test.css?v=1.2.3");

		assert_eq!(result, "/static/test.css?v=1.2.3");
	}

	/// Test with fragment preservation
	#[rstest]
	fn test_resolve_static_with_fragment() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static("doc.html#section");

		assert_eq!(result, "/static/doc.html#section");
	}
}

#[cfg(wasm)]
mod wasm_happy_path_tests {
	use reinhardt_pages::static_resolver::{init_static_resolver, resolve_static};
	use wasm_bindgen_test::*;

	wasm_bindgen_test_configure!(run_in_browser);

	#[wasm_bindgen_test]
	fn test_wasm_resolve_static_basic_css() {
		init_static_resolver("/static/".to_string());
		let result = resolve_static("css/style.css");
		assert_eq!(result, "/static/css/style.css");
	}

	#[wasm_bindgen_test]
	fn test_wasm_resolve_static_javascript() {
		init_static_resolver("/static/".to_string());
		let result = resolve_static("js/app.js");
		assert_eq!(result, "/static/js/app.js");
	}

	#[wasm_bindgen_test]
	fn test_wasm_resolve_static_with_cdn() {
		init_static_resolver("https://cdn.example.com/static/".to_string());
		let result = resolve_static("images/logo.png");
		assert_eq!(result, "https://cdn.example.com/static/images/logo.png");
	}
}

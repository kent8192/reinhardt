#![cfg(not(target_arch = "wasm32"))]
//! Sanity tests for static asset URL resolution
//!
//! These tests verify basic functionality of the static resolver and related
//! components to ensure the system works at its most fundamental level.

#[cfg(native)]
mod sanity_tests {
	use reinhardt_pages::static_resolver::{init_static_resolver, is_initialized, resolve_static};
	use reinhardt_utils::staticfiles::TemplateStaticConfig;
	use rstest::rstest;

	/// Test that resolve_static returns a non-empty string
	#[rstest]
	fn test_resolve_static_returns_string() {
		let result = resolve_static("test.css");
		assert!(!result.is_empty(), "Result should not be empty");
	}

	/// Test basic path resolution
	#[rstest]
	fn test_resolve_static_basic_path() {
		let result = resolve_static("css/style.css");
		assert!(
			result.contains("style.css"),
			"Result should contain the filename"
		);
		assert!(
			result.contains("css"),
			"Result should contain the directory"
		);
	}

	/// Test initialization sets initialized state
	#[rstest]
	fn test_init_resolver_sets_initialized_state() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		assert!(
			is_initialized(),
			"is_initialized should return true after init_static_resolver"
		);
	}

	/// Test resolve_static with leading slash
	#[rstest]
	fn test_resolve_static_with_leading_slash() {
		let result = resolve_static("/css/style.css");
		assert!(
			result.contains("style.css"),
			"Result should contain filename"
		);
		// Should not have double slashes
		assert!(
			!result.contains("//static"),
			"Result should not have double slashes"
		);
	}

	/// Test resolve_static with trailing slash in path
	#[rstest]
	fn test_resolve_static_with_directory_path() {
		let result = resolve_static("images/logos/");
		assert!(result.contains("images"), "Result should contain directory");
		assert!(
			result.contains("logos"),
			"Result should contain subdirectory"
		);
	}

	/// Test configuration with custom base URL
	#[rstest]
	fn test_template_config_with_custom_base_url() {
		let config = TemplateStaticConfig::new("/assets/".to_string());
		let resolved = config.resolve_url("style.css");

		assert!(
			resolved.contains("/assets/"),
			"Result should contain custom base URL"
		);
		assert!(
			resolved.contains("style.css"),
			"Result should contain filename"
		);
	}

	/// Test that resolve_static preserves file extension
	#[rstest]
	fn test_resolve_static_preserves_extension() {
		let files = vec![
			"style.css",
			"script.js",
			"image.png",
			"font.woff2",
			"data.json",
		];

		for filename in files {
			let result = resolve_static(filename);
			assert!(
				result.ends_with(filename),
				"Result should end with the original filename for {}",
				filename
			);
		}
	}

	/// Test that configuration creation succeeds with various base URLs
	#[rstest]
	fn test_config_creation_with_various_base_urls() {
		let base_urls = vec![
			"/static/",
			"/assets/",
			"https://cdn.example.com/static/",
			"/static", // without trailing slash
		];

		for base_url in base_urls {
			let config = TemplateStaticConfig::new(base_url.to_string());
			// Should not panic during creation
			let _ = config.resolve_url("test.css");
		}
	}
}

#[cfg(wasm)]
mod wasm_sanity_tests {
	use reinhardt_pages::static_resolver::{init_static_resolver, is_initialized, resolve_static};
	use wasm_bindgen_test::*;

	wasm_bindgen_test_configure!(run_in_browser);

	#[wasm_bindgen_test]
	fn test_wasm_resolve_static_returns_string() {
		let result = resolve_static("test.css");
		assert!(!result.is_empty());
	}

	#[wasm_bindgen_test]
	fn test_wasm_resolve_static_basic_path() {
		let result = resolve_static("css/style.css");
		assert!(result.contains("style.css"));
	}

	#[wasm_bindgen_test]
	fn test_wasm_init_resolver_sets_initialized_state() {
		init_static_resolver("/static/".to_string());
		assert!(is_initialized());
	}
}

#![cfg(not(target_arch = "wasm32"))]
//! Edge case tests for static asset URL resolution
//!
//! These tests verify behavior with unusual but valid inputs that might
//! cause issues if not handled correctly.

#[cfg(native)]
mod edge_case_tests {
	use reinhardt_pages::static_resolver::{init_static_resolver, resolve_static};
	use reinhardt_utils::staticfiles::TemplateStaticConfig;
	use rstest::rstest;

	/// Test path with Japanese characters
	#[rstest]
	fn test_resolve_static_japanese_path() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static("images/日本語.png");

		assert!(result.contains("日本語"));
		assert!(result.contains("images"));
		assert!(result.starts_with("/static/"));
	}

	/// Test path with emoji
	#[rstest]
	fn test_resolve_static_emoji_path() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static("icons/🚀rocket.svg");

		assert!(result.contains("🚀rocket"));
		assert!(result.starts_with("/static/"));
	}

	/// Test path with spaces
	#[rstest]
	fn test_resolve_static_path_with_spaces() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static("images/my image file.png");

		assert!(result.contains("my image file"));
		assert!(result.ends_with("my image file.png"));
	}

	/// Test path with special URL-encodable characters
	#[rstest]
	#[case("file with @.css")]
	#[case("file-with_dash.js")]
	#[case("file_with_underscore.js")]
	#[case("file.multiple.dots.css")]
	#[case("file[bracket].css")]
	fn test_resolve_static_special_chars(#[case] filename: &str) {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static(filename);

		assert!(result.contains(filename));
		assert!(result.starts_with("/static/"));
	}

	/// Test multiple leading slashes in path
	#[rstest]
	fn test_resolve_static_multiple_leading_slashes() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static("///css/style.css");

		assert!(result.contains("css/style.css"));
		assert!(!result.contains("///"), "Should normalize multiple slashes");
	}

	/// Test very deep directory structure
	#[rstest]
	fn test_resolve_static_deep_path_structure() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let deep_path = "a/b/c/d/e/f/g/h/i/j/k/l/m/n/o/p/q/r/s/t/u/v/w/x/y/z/file.css";
		let result = resolve_static(deep_path);

		assert!(result.contains(deep_path));
		assert!(result.starts_with("/static/"));
	}

	/// Test various base URL formats
	#[rstest]
	#[case("/static/", "/static/")]
	#[case("/assets/", "/assets/")]
	#[case("https://cdn.example.com/static/", "https://cdn.example.com/static/")]
	#[case("/static", "/static/")]
	#[case("static/", "static/")]
	fn test_resolve_static_various_base_urls(
		#[case] base_url: &str,
		#[case] expected_prefix: &str,
	) {
		let config = TemplateStaticConfig::new(base_url.to_string());
		init_static_resolver(config);

		let result = resolve_static("test.css");

		assert!(result.starts_with(expected_prefix));
		assert!(result.contains("test.css"));
	}

	/// Test path with only extension
	#[rstest]
	fn test_resolve_static_only_extension() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static(".htaccess");

		assert!(result.contains(".htaccess"));
		assert!(result.starts_with("/static/"));
	}

	/// Test path with multiple extensions
	#[rstest]
	fn test_resolve_static_multiple_extensions() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static("archive.tar.gz");

		assert_eq!(result, "/static/archive.tar.gz");
	}

	/// Test with trailing slash in path
	#[rstest]
	fn test_resolve_static_trailing_slash_in_path() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static("images/");

		assert!(result.contains("images"));
		assert!(result.ends_with("images/") || result.ends_with("images"));
	}

	/// Test base URL without protocol in CDN URL
	#[rstest]
	fn test_resolve_static_protocol_relative_cdn() {
		let config = TemplateStaticConfig::new("//cdn.example.com/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static("css/style.css");

		assert!(result.contains("cdn.example.com"));
		assert!(result.contains("css/style.css"));
	}

	/// Test with very long filename
	#[rstest]
	fn test_resolve_static_long_filename() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let long_name = "a".repeat(200);
		let path = format!("{}.css", long_name);
		let result = resolve_static(&path);

		assert!(result.contains(&long_name));
		assert!(result.starts_with("/static/"));
	}

	/// Test with mixed case extensions
	#[rstest]
	#[case("image.PNG")]
	#[case("script.JavaScript")]
	#[case("style.CSS")]
	#[case("FONT.WOFF2")]
	fn test_resolve_static_mixed_case_extension(#[case] filename: &str) {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static(filename);

		assert!(result.contains(filename));
		assert!(result.starts_with("/static/"));
	}

	/// Test path that looks like a relative reference
	#[rstest]
	fn test_resolve_static_looks_like_relative_path() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static("../../../etc/passwd");

		// Should be treated as a literal path, not traversal
		assert!(result.contains("etc/passwd"));
		assert!(result.starts_with("/static/"));
	}

	/// Test absolute path input
	#[rstest]
	fn test_resolve_static_absolute_path_input() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static("/absolute/path/file.css");

		// Leading slash should be handled, not doubled
		assert!(!result.contains("//static"));
		assert!(result.contains("absolute/path/file.css"));
	}

	/// Test with numeric-only filenames
	#[rstest]
	fn test_resolve_static_numeric_filename() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static("12345/67890.css");

		assert_eq!(result, "/static/12345/67890.css");
	}
}

#[cfg(wasm)]
mod wasm_edge_case_tests {
	use reinhardt_pages::static_resolver::{init_static_resolver, resolve_static};
	use wasm_bindgen_test::*;

	wasm_bindgen_test_configure!(run_in_browser);

	#[wasm_bindgen_test]
	fn test_wasm_resolve_static_multiple_slashes() {
		init_static_resolver("/static/".to_string());
		let result = resolve_static("///css/style.css");
		assert!(result.contains("css/style.css"));
	}

	#[wasm_bindgen_test]
	fn test_wasm_resolve_static_unicode() {
		init_static_resolver("/static/".to_string());
		let result = resolve_static("images/日本語.png");
		assert!(result.contains("日本語"));
	}

	#[wasm_bindgen_test]
	fn test_wasm_resolve_static_spaces() {
		init_static_resolver("/static/".to_string());
		let result = resolve_static("images/my image.png");
		assert!(result.contains("my image"));
	}
}

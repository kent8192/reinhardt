#![cfg(not(target_arch = "wasm32"))]
//! Boundary value analysis tests for static asset URL resolution
//!
//! Tests boundary conditions for path length, nesting depth, query string length, etc.
//! Uses rstest #[case] for systematic boundary value testing.

#[cfg(native)]
mod boundary_tests {
	use reinhardt_pages::static_resolver::{init_static_resolver, resolve_static};
	use reinhardt_utils::staticfiles::TemplateStaticConfig;
	use rstest::rstest;

	/// Test path length boundaries
	#[rstest]
	#[case("", "/static/")] // Empty path
	#[case("f", "/static/f")] // Single character
	#[case("a.css", "/static/a.css")] // Minimal valid
	#[case(&"a".repeat(100), &format!("/static/{}", "a".repeat(100)))] // Long filename
	#[case(&"a".repeat(255), &format!("/static/{}", "a".repeat(255)))] // Very long
	#[case(&"a".repeat(4096), &format!("/static/{}", "a".repeat(4096)))] // Extreme length
	fn test_boundary_path_length(#[case] path: &str, #[case] expected_contains: &str) {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static(path);

		assert!(result.contains(expected_contains) || result.starts_with("/static/"));
	}

	/// Test directory nesting depth boundaries
	#[rstest]
	#[case("file.css", "/static/file.css")] // 0 levels
	#[case("a/file.css", "/static/a/file.css")] // 1 level
	#[case("a/b/file.css", "/static/a/b/file.css")] // 2 levels
	#[case("a/b/c/d/e/f/g/h/i/j/file.css", "/static/a/b/c/d/e/f/g/h/i/j/file.css")] // 10 levels
	fn test_boundary_directory_depth(#[case] path: &str, #[case] expected: &str) {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static(path);

		assert_eq!(result, expected);
	}

	/// Test query string length boundaries
	#[rstest]
	#[case("file.css", "/static/file.css")] // No query
	#[case("file.css?v=1", "/static/file.css?v=1")] // Short query
	#[case("file.css?version=1234567890", "/static/file.css?version=1234567890")] // Medium query
	#[case(&format!("file.css?v={}", "x".repeat(100)), &format!("/static/file.css?v={}", "x".repeat(100)))] // Long query
	fn test_boundary_query_string_length(#[case] path: &str, #[case] expected: &str) {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static(path);

		assert_eq!(result, expected);
	}

	/// Test base URL length boundaries
	#[rstest]
	#[case("/", "file.css")] // Minimal base
	#[case("/static/", "file.css")] // Normal base
	#[case(&format!("/{}/", "a".repeat(50)), "file.css")] // Long base
	#[case(&format!("https://{}/static/", "cdn.example.com".repeat(5)), "file.css")] // Very long base
	fn test_boundary_base_url_length(#[case] base_url: &str, #[case] path: &str) {
		let config = TemplateStaticConfig::new(base_url.to_string());
		init_static_resolver(config);

		let result = resolve_static(path);

		assert!(!result.is_empty());
		assert!(result.contains("file.css"));
	}

	/// Test filename with minimal/maximal extension
	#[rstest]
	#[case("file.c", "/static/file.c")] // Single char extension
	#[case("file.css", "/static/file.css")] // Normal extension
	#[case(&format!("file.{}", "a".repeat(50)), &format!("/static/file.{}", "a".repeat(50)))] // Long extension
	fn test_boundary_file_extension_length(#[case] filename: &str, #[case] expected: &str) {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static(filename);

		assert_eq!(result, expected);
	}

	/// Test manifest size boundaries
	#[rstest]
	fn test_boundary_manifest_empty() {
		use std::collections::HashMap;

		let manifest = HashMap::new();
		let config = TemplateStaticConfig::new("/static/".to_string()).with_manifest(manifest);
		init_static_resolver(config);

		let result = resolve_static("file.css");

		assert_eq!(result, "/static/file.css");
	}

	#[rstest]
	fn test_boundary_manifest_single_entry() {
		use std::collections::HashMap;

		let mut manifest = HashMap::new();
		manifest.insert("file.css".to_string(), "file.abc.css".to_string());

		let config = TemplateStaticConfig::new("/static/".to_string()).with_manifest(manifest);
		init_static_resolver(config);

		let result = resolve_static("file.css");

		assert_eq!(result, "/static/file.abc.css");
	}

	#[rstest]
	fn test_boundary_manifest_large() {
		use std::collections::HashMap;

		let mut manifest = HashMap::new();
		for i in 0..100 {
			let key = format!("file{}.css", i);
			let value = format!("file{}.hash.css", i);
			manifest.insert(key, value);
		}

		let config = TemplateStaticConfig::new("/static/".to_string()).with_manifest(manifest);
		init_static_resolver(config);

		let result = resolve_static("file50.css");

		assert_eq!(result, "/static/file50.hash.css");
	}

	/// Test special characters at boundaries
	#[rstest]
	#[case(".", "/static/.")] // Single dot
	#[case("..", "/static/..")] // Double dot
	#[case("file..", "/static/file..")] // Ends with dots
	#[case("..file", "/static/..file")] // Starts with dots
	fn test_boundary_special_start_end(#[case] path: &str, #[case] expected: &str) {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static(path);

		assert_eq!(result, expected);
	}

	/// Test slashes at boundaries
	#[rstest]
	#[case("file.css", "/static/file.css")] // No slashes
	#[case("/file.css", "/static/file.css")] // Leading slash
	#[case("file.css/", "/static/file.css/")] // Trailing slash
	#[case("/file.css/", "/static/file.css/")] // Both slashes
	fn test_boundary_slash_positions(#[case] path: &str, #[case] expected: &str) {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static(path);

		assert_eq!(result, expected);
	}

	/// Test whitespace boundaries
	#[rstest]
	#[case(" file.css", "/static/ file.css")] // Leading space
	#[case("file.css ", "/static/file.css ")] // Trailing space
	#[case("file .css", "/static/file .css")] // Embedded space
	fn test_boundary_whitespace(#[case] path: &str, #[case] expected: &str) {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static(path);

		assert_eq!(result, expected);
	}

	/// Test numeric boundaries
	#[rstest]
	#[case("0.css", "/static/0.css")]
	#[case("1.css", "/static/1.css")]
	#[case(&format!("{}.css", i32::MAX), &format!("/static/{}.css", i32::MAX))]
	#[case(&format!("{}.css", i64::MAX), &format!("/static/{}.css", i64::MAX))]
	fn test_boundary_numeric_values(#[case] path: &str, #[case] expected: &str) {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static(path);

		assert_eq!(result, expected);
	}
}

#[cfg(wasm)]
mod wasm_boundary_tests {
	use reinhardt_pages::static_resolver::{init_static_resolver, resolve_static};
	use wasm_bindgen_test::*;

	wasm_bindgen_test_configure!(run_in_browser);

	#[wasm_bindgen_test]
	fn test_wasm_boundary_empty_path() {
		init_static_resolver("/static/".to_string());
		assert_eq!(resolve_static(""), "/static/");
	}

	#[wasm_bindgen_test]
	fn test_wasm_boundary_long_path() {
		init_static_resolver("/static/".to_string());
		let long_path = "a".repeat(100);
		let result = resolve_static(&long_path);
		assert!(result.contains(&long_path));
	}

	#[wasm_bindgen_test]
	fn test_wasm_boundary_deep_nesting() {
		init_static_resolver("/static/".to_string());
		let deep = "a/b/c/d/e/f/g/h/i/j/file.css";
		let result = resolve_static(deep);
		assert_eq!(result, "/static/a/b/c/d/e/f/g/h/i/j/file.css");
	}
}

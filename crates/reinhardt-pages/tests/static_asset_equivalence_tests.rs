#![cfg(not(target_arch = "wasm32"))]
//! Equivalence partitioning tests for static asset URL resolution
//!
//! Tests are organized by equivalence classes: file types, path formats, and base URL formats.
//! Uses rstest #[case] for systematic coverage of each partition.

#[cfg(native)]
mod equivalence_tests {
	use reinhardt_pages::static_resolver::{init_static_resolver, resolve_static};
	use reinhardt_utils::staticfiles::TemplateStaticConfig;
	use rstest::rstest;

	/// Test file extension equivalence classes
	#[rstest]
	#[case("style.css", "css")]
	#[case("app.js", "js")]
	#[case("image.png", "png")]
	#[case("image.jpg", "jpg")]
	#[case("image.svg", "svg")]
	#[case("font.woff2", "woff2")]
	#[case("data.json", "json")]
	#[case("document.pdf", "pdf")]
	#[case("video.mp4", "mp4")]
	#[case("audio.mp3", "mp3")]
	fn test_equivalence_file_extensions(#[case] filename: &str, #[case] ext: &str) {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static(filename);

		assert!(result.ends_with(ext), "Result should end with {}", ext);
		assert!(result.contains(filename));
	}

	/// Test path format equivalence classes
	#[rstest]
	#[case("style.css", "/static/style.css")] // Simple file
	#[case("css/style.css", "/static/css/style.css")] // Single directory
	#[case("css/app/theme/style.css", "/static/css/app/theme/style.css")] // Nested
	#[case("/style.css", "/static/style.css")] // Leading slash
	fn test_equivalence_path_formats(#[case] path: &str, #[case] expected: &str) {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static(path);

		assert_eq!(result, expected);
	}

	/// Test base URL format equivalence classes
	#[rstest]
	#[case("/static/", "test.css", "/static/test.css")]
	#[case("/static", "test.css", "/static/test.css")]
	#[case("/assets/", "test.css", "/assets/test.css")]
	#[case(
		"https://cdn.example.com/static/",
		"test.css",
		"https://cdn.example.com/static/test.css"
	)]
	#[case(
		"https://cdn.example.com/static",
		"test.css",
		"https://cdn.example.com/static/test.css"
	)]
	#[case(
		"//cdn.example.com/static/",
		"test.css",
		"//cdn.example.com/static/test.css"
	)]
	fn test_equivalence_base_url_formats(
		#[case] base_url: &str,
		#[case] path: &str,
		#[case] expected: &str,
	) {
		let config = TemplateStaticConfig::new(base_url.to_string());
		init_static_resolver(config);

		let result = resolve_static(path);

		assert_eq!(result, expected);
	}

	/// Test file naming convention equivalence classes
	#[rstest]
	#[case("style.css")] // Simple name
	#[case("style.min.css")] // Minified
	#[case("style.abc123.css")] // Hashed
	#[case("_style.css")] // Underscore prefix
	#[case("style-v1.css")] // Version in name
	fn test_equivalence_file_naming_conventions(#[case] filename: &str) {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static(filename);

		assert!(result.contains(filename));
		assert!(result.ends_with("css"));
	}

	/// Test query string and fragment equivalence classes
	#[rstest]
	#[case("style.css", "/static/style.css")]
	#[case("style.css?v=1", "/static/style.css?v=1")]
	#[case("style.css#section", "/static/style.css#section")]
	#[case("style.css?v=1&debug=true", "/static/style.css?v=1&debug=true")]
	#[case("style.css?v=1#section", "/static/style.css?v=1#section")]
	fn test_equivalence_query_string_and_fragments(#[case] path: &str, #[case] expected: &str) {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static(path);

		assert_eq!(result, expected);
	}

	/// Test directory structure depth equivalence classes
	#[rstest]
	#[case("file.css", "/static/file.css")] // Root level
	#[case("css/file.css", "/static/css/file.css")] // 1 level
	#[case("css/app/file.css", "/static/css/app/file.css")] // 2 levels
	#[case("css/app/theme/file.css", "/static/css/app/theme/file.css")] // 3 levels
	fn test_equivalence_directory_depth(#[case] path: &str, #[case] expected: &str) {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static(path);

		assert_eq!(result, expected);
	}

	/// Test manifest presence equivalence classes
	#[rstest]
	fn test_equivalence_manifest_presence_no_manifest() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static("css/style.css");

		assert_eq!(result, "/static/css/style.css");
	}

	#[rstest]
	fn test_equivalence_manifest_presence_with_manifest() {
		use std::collections::HashMap;

		let mut manifest = HashMap::new();
		manifest.insert(
			"css/style.css".to_string(),
			"css/style.hash.css".to_string(),
		);

		let config = TemplateStaticConfig::new("/static/".to_string()).with_manifest(manifest);
		init_static_resolver(config);

		let result = resolve_static("css/style.css");

		assert_eq!(result, "/static/css/style.hash.css");
	}

	/// Test case sensitivity equivalence classes
	#[rstest]
	#[case("Style.CSS")]
	#[case("style.css")]
	#[case("STYLE.CSS")]
	fn test_equivalence_case_sensitivity(#[case] filename: &str) {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static(filename);

		assert!(result.contains(filename));
	}
}

#[cfg(wasm)]
mod wasm_equivalence_tests {
	use reinhardt_pages::static_resolver::{init_static_resolver, resolve_static};
	use wasm_bindgen_test::*;

	wasm_bindgen_test_configure!(run_in_browser);

	#[wasm_bindgen_test]
	fn test_wasm_equivalence_file_types() {
		init_static_resolver("/static/".to_string());

		assert!(resolve_static("style.css").ends_with("css"));
		assert!(resolve_static("app.js").ends_with("js"));
		assert!(resolve_static("image.png").ends_with("png"));
	}

	#[wasm_bindgen_test]
	fn test_wasm_equivalence_path_formats() {
		init_static_resolver("/static/".to_string());

		assert_eq!(resolve_static("style.css"), "/static/style.css");
		assert_eq!(resolve_static("css/style.css"), "/static/css/style.css");
	}
}

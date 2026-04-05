#![cfg(not(target_arch = "wasm32"))]
//! Combination tests for static asset URL resolution
//!
//! Tests combinations of multiple configuration options and input variations.

#[cfg(native)]
mod combination_tests {
	use reinhardt_pages::static_resolver::{init_static_resolver, resolve_static};
	use reinhardt_utils::staticfiles::TemplateStaticConfig;
	use rstest::rstest;
	use std::collections::HashMap;

	/// Test combination: base URL with/without trailing slash × path with/without leading slash
	#[rstest]
	#[case("/static/", "file.css", "/static/file.css")]
	#[case("/static/", "/file.css", "/static/file.css")]
	#[case("/static", "file.css", "/static/file.css")]
	#[case("/static", "/file.css", "/static/file.css")]
	#[case(
		"https://cdn.example.com/static/",
		"file.css",
		"https://cdn.example.com/static/file.css"
	)]
	#[case(
		"https://cdn.example.com/static",
		"/file.css",
		"https://cdn.example.com/static/file.css"
	)]
	fn test_combination_base_url_path_slashes(
		#[case] base_url: &str,
		#[case] path: &str,
		#[case] expected: &str,
	) {
		let config = TemplateStaticConfig::new(base_url.to_string());
		init_static_resolver(config);

		let result = resolve_static(path);

		assert_eq!(result, expected);
	}

	/// Test combination: manifest enabled × file in manifest × query string present
	#[rstest]
	#[case(true, true, false, "file.css", "/static/file.hash.css")]
	#[case(true, true, true, "file.css?v=1", "/static/file.hash.css?v=1")]
	#[case(true, false, false, "unknown.css", "/static/unknown.css")]
	#[case(true, false, true, "unknown.css?v=1", "/static/unknown.css?v=1")]
	#[case(false, false, false, "file.css", "/static/file.css")]
	#[case(false, false, true, "file.css?v=1", "/static/file.css?v=1")]
	fn test_combination_manifest_file_query(
		#[case] use_manifest: bool,
		#[case] in_manifest: bool,
		#[case] has_query: bool,
		#[case] input: &str,
		#[case] expected: &str,
	) {
		let config = if use_manifest {
			let mut manifest = HashMap::new();
			if in_manifest {
				let path = if has_query {
					input.split('?').next().unwrap()
				} else {
					input
				};
				manifest.insert(path.to_string(), "file.hash.css".to_string());
			}
			TemplateStaticConfig::new("/static/".to_string()).with_manifest(manifest)
		} else {
			TemplateStaticConfig::new("/static/".to_string())
		};

		init_static_resolver(config);
		let result = resolve_static(input);

		assert_eq!(result, expected);
	}

	/// Test combination: different CDN URLs × directory depths × file extensions
	#[rstest]
	#[case("https://cdn1.example.com/static/", "css/style.css")]
	#[case("https://cdn2.example.com/assets/", "css/app/theme/style.css")]
	#[case("https://cdn3.example.com/static/", "images/icons/avatar.png")]
	#[case("//cdn.example.com/static/", "fonts/roboto/regular.woff2")]
	fn test_combination_cdn_depth_extension(#[case] base_url: &str, #[case] path: &str) {
		let config = TemplateStaticConfig::new(base_url.to_string());
		init_static_resolver(config);

		let result = resolve_static(path);

		assert!(result.starts_with(base_url.trim_end_matches('/')));
		assert!(result.contains(path));
	}

	/// Test combination: manifest with multiple entries × various query strings
	#[rstest]
	fn test_combination_large_manifest_with_queries() {
		let mut manifest = HashMap::new();
		for i in 0..10 {
			let key = format!("file{}.css", i);
			let value = format!("file{}.abc{}.css", i, i);
			manifest.insert(key, value);
		}

		let config = TemplateStaticConfig::new("/static/".to_string()).with_manifest(manifest);
		init_static_resolver(config);

		// Test with and without query strings
		let result1 = resolve_static("file5.css");
		let result2 = resolve_static("file5.css?v=1.0");
		let result3 = resolve_static("file5.css?v=1.0&debug=true");

		assert_eq!(result1, "/static/file5.abc5.css");
		assert_eq!(result2, "/static/file5.abc5.css?v=1.0");
		assert_eq!(result3, "/static/file5.abc5.css?v=1.0&debug=true");
	}

	/// Test combination: various path formats × manifest enabled/disabled
	#[rstest]
	#[case("file.css", true, "/static/file.hash.css")]
	#[case("file.css", false, "/static/file.css")]
	#[case("dir/file.css", true, "/static/dir/file.hash.css")]
	#[case("dir/file.css", false, "/static/dir/file.css")]
	#[case("dir/sub/file.css", true, "/static/dir/sub/file.hash.css")]
	#[case("dir/sub/file.css", false, "/static/dir/sub/file.css")]
	fn test_combination_path_format_manifest(
		#[case] path: &str,
		#[case] use_manifest: bool,
		#[case] expected: &str,
	) {
		let config = if use_manifest {
			let mut manifest = HashMap::new();
			manifest.insert(
				path.to_string(),
				expected.trim_start_matches("/static/").to_string(),
			);
			TemplateStaticConfig::new("/static/".to_string()).with_manifest(manifest)
		} else {
			TemplateStaticConfig::new("/static/".to_string())
		};

		init_static_resolver(config);
		let result = resolve_static(path);

		assert_eq!(result, expected);
	}

	/// Test combination: dynamic path construction with various components
	#[rstest]
	fn test_combination_dynamic_path_construction() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		// Simulate dynamic path building
		for theme in ["light", "dark"] {
			for size in ["sm", "md", "lg"] {
				let path = format!("images/themes/{}/size-{}.png", theme, size);
				let result = resolve_static(&path);

				assert!(result.contains(theme));
				assert!(result.contains(size));
				assert_eq!(result, format!("/static/{}", path));
			}
		}
	}

	/// Test combination: multiple file types with manifest
	#[rstest]
	fn test_combination_multiple_file_types_with_manifest() {
		let mut manifest = HashMap::new();
		manifest.insert("style.css".to_string(), "style.abc.css".to_string());
		manifest.insert("script.js".to_string(), "script.def.js".to_string());
		manifest.insert("logo.png".to_string(), "logo.ghi.png".to_string());

		let config = TemplateStaticConfig::new("/static/".to_string()).with_manifest(manifest);
		init_static_resolver(config);

		let css = resolve_static("style.css");
		let js = resolve_static("script.js");
		let png = resolve_static("logo.png");

		assert_eq!(css, "/static/style.abc.css");
		assert_eq!(js, "/static/script.def.js");
		assert_eq!(png, "/static/logo.ghi.png");

		// Test unknown file type
		let unknown = resolve_static("unknown.txt");
		assert_eq!(unknown, "/static/unknown.txt");
	}
}

#[cfg(wasm)]
mod wasm_combination_tests {
	use reinhardt_pages::static_resolver::{init_static_resolver, resolve_static};
	use wasm_bindgen_test::*;

	wasm_bindgen_test_configure!(run_in_browser);

	#[wasm_bindgen_test]
	fn test_wasm_combination_basic() {
		init_static_resolver("/static/".to_string());

		let files = vec!["style.css", "script.js", "image.png"];
		for file in files {
			let result = resolve_static(file);
			assert!(result.contains(file));
		}
	}
}

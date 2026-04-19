#![cfg(not(target_arch = "wasm32"))]
//! Decision table tests for static asset URL resolution
//!
//! Tests combinations of boolean and categorical conditions using decision tables.
//! Uses rstest #[case] for systematic decision table coverage.

#[cfg(native)]
mod decision_table_tests {
	use reinhardt_pages::static_resolver::{init_static_resolver, resolve_static};
	use reinhardt_utils::staticfiles::TemplateStaticConfig;
	use rstest::rstest;
	use std::collections::HashMap;

	/// Decision table: URL Resolution
	/// Columns: use_manifest, path_in_manifest, query_string
	///
	/// | use_manifest | path_in_manifest | query_string | Expected behavior |
	/// |---|---|---|---|
	/// | No | N/A | No | Return base + path |
	/// | No | N/A | Yes | Return base + path + query |
	/// | Yes | Yes | No | Return base + hashed_path |
	/// | Yes | Yes | Yes | Return base + hashed_path + query |
	/// | Yes | No | No | Return base + path (fallback) |
	/// | Yes | No | Yes | Return base + path + query (fallback) |

	#[rstest]
	#[case(false, false, false, "file.css", "/static/file.css")]
	#[case(false, false, true, "file.css?v=1", "/static/file.css?v=1")]
	#[case(true, true, false, "file.css", "/static/file.hash.css")]
	#[case(true, true, true, "file.css?v=1", "/static/file.hash.css?v=1")]
	#[case(true, false, false, "unknown.css", "/static/unknown.css")]
	#[case(true, false, true, "unknown.css?v=1", "/static/unknown.css?v=1")]
	fn test_decision_table_url_resolution(
		#[case] use_manifest: bool,
		#[case] path_in_manifest: bool,
		#[case] _query_string: bool,
		#[case] input: &str,
		#[case] expected: &str,
	) {
		let config = if use_manifest {
			let mut manifest = HashMap::new();
			if path_in_manifest {
				manifest.insert("file.css".to_string(), "file.hash.css".to_string());
			}
			TemplateStaticConfig::new("/static/".to_string()).with_manifest(manifest)
		} else {
			TemplateStaticConfig::new("/static/".to_string())
		};

		init_static_resolver(config);
		let result = resolve_static(input);

		assert_eq!(result, expected);
	}

	/// Decision table: Base URL Normalization
	/// Columns: base_has_trailing_slash, path_has_leading_slash
	///
	/// | base_trailing | path_leading | Expected separator count |
	/// |---|---|---|
	/// | Yes | No | 1 |
	/// | Yes | Yes | 1 |
	/// | No | No | 1 |
	/// | No | Yes | 1 |

	#[rstest]
	#[case("/static/", "file.css", "/static/file.css")]
	#[case("/static/", "/file.css", "/static/file.css")]
	#[case("/static", "file.css", "/static/file.css")]
	#[case("/static", "/file.css", "/static/file.css")]
	fn test_decision_table_base_url_normalization(
		#[case] base_url: &str,
		#[case] path: &str,
		#[case] expected: &str,
	) {
		let config = TemplateStaticConfig::new(base_url.to_string());
		init_static_resolver(config);

		let result = resolve_static(path);

		assert_eq!(result, expected);
	}

	/// Decision table: File Processing
	/// Columns: file_exists_in_manifest, manifest_enabled, path_has_query
	///
	/// | file_in_manifest | manifest_enabled | has_query | Expected behavior |
	/// |---|---|---|---|
	/// | No | No | No | Use original path |
	/// | No | No | Yes | Use original path + query |
	/// | No | Yes | No | Use original path (not found in manifest) |
	/// | No | Yes | Yes | Use original path + query (not found) |
	/// | Yes | Yes | No | Use hashed path |
	/// | Yes | Yes | Yes | Use hashed path + query |

	#[rstest]
	#[case(
		true,
		true,
		"css/known.css",
		"css/known.hash.css",
		false,
		"css/known.hash.css"
	)]
	#[case(
		true,
		true,
		"css/known.css?v=1",
		"css/known.hash.css",
		true,
		"css/known.hash.css?v=1"
	)]
	#[case(
		false,
		true,
		"css/unknown.css",
		"css/unknown.css",
		false,
		"css/unknown.css"
	)]
	#[case(
		false,
		true,
		"css/unknown.css?v=1",
		"css/unknown.css",
		true,
		"css/unknown.css?v=1"
	)]
	#[case(false, false, "css/any.css", "css/any.css", false, "css/any.css")]
	#[case(
		false,
		false,
		"css/any.css?v=1",
		"css/any.css",
		true,
		"css/any.css?v=1"
	)]
	fn test_decision_table_file_processing(
		#[case] in_manifest: bool,
		#[case] use_manifest: bool,
		#[case] input: &str,
		#[case] hashed: &str,
		#[case] has_query: bool,
		#[case] expected_end: &str,
	) {
		let config = if use_manifest {
			let mut manifest = HashMap::new();
			if in_manifest {
				let path = if has_query {
					input.split('?').next().unwrap()
				} else {
					input
				};
				manifest.insert(path.to_string(), hashed.to_string());
			}
			TemplateStaticConfig::new("/static/".to_string()).with_manifest(manifest)
		} else {
			TemplateStaticConfig::new("/static/".to_string())
		};

		init_static_resolver(config);
		let result = resolve_static(input);

		assert!(result.ends_with(expected_end));
	}

	/// Decision table: Path Handling
	/// Columns: path_type, leading_slash, trailing_slash
	///
	/// | path_type | leading | trailing | Expected normalization |
	/// |---|---|---|---|
	/// | Simple filename | No | No | /static/filename |
	/// | Simple filename | Yes | No | /static/filename |
	/// | Directory path | No | No | /static/dir/file |
	/// | Directory path | Yes | No | /static/dir/file |
	/// | Directory path | No | Yes | /static/dir/file/ |
	/// | Directory path | Yes | Yes | /static/dir/file/ |

	#[rstest]
	#[case("file.css", false, false, "file.css")]
	#[case("/file.css", true, false, "file.css")]
	#[case("dir/file.css", false, false, "dir/file.css")]
	#[case("/dir/file.css", true, false, "dir/file.css")]
	#[case("dir/file.css/", false, true, "dir/file.css/")]
	#[case("/dir/file.css/", true, true, "dir/file.css/")]
	fn test_decision_table_path_handling(
		#[case] path: &str,
		#[case] _has_leading: bool,
		#[case] _has_trailing: bool,
		#[case] expected_content: &str,
	) {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static(path);

		assert!(result.contains(expected_content));
	}

	/// Decision table: Content Type Resolution
	/// Columns: file_extension, mime_type_expected
	///
	/// | extension | expected | Valid |
	/// |---|---|---|
	/// | .css | text/css | Yes |
	/// | .js | text/javascript | Yes |
	/// | .png | image/png | Yes |
	/// | .jpg | image/jpeg | Yes |
	/// | .woff2 | font/woff2 | Yes |
	/// | .json | application/json | Yes |

	#[rstest]
	#[case("style.css")]
	#[case("app.js")]
	#[case("image.png")]
	#[case("photo.jpg")]
	#[case("font.woff2")]
	#[case("config.json")]
	fn test_decision_table_content_types(#[case] filename: &str) {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static(filename);

		assert!(result.ends_with(filename.split('.').last().unwrap()));
	}

	/// Decision table: Base URL Types
	/// Columns: base_url_type, path, valid
	///
	/// | type | example | valid |
	/// |---|---|---|
	/// | Relative | /static/ | Yes |
	/// | Absolute HTTP | https://cdn.example.com/static/ | Yes |
	/// | Absolute HTTPS | https://cdn.example.com/static/ | Yes |
	/// | Protocol-relative | //cdn.example.com/static/ | Yes |

	#[rstest]
	#[case("/static/", "test.css", "/static/test.css")]
	#[case(
		"https://cdn.example.com/static/",
		"test.css",
		"https://cdn.example.com/static/test.css"
	)]
	#[case(
		"//cdn.example.com/static/",
		"test.css",
		"//cdn.example.com/static/test.css"
	)]
	fn test_decision_table_base_url_types(
		#[case] base_url: &str,
		#[case] path: &str,
		#[case] expected: &str,
	) {
		let config = TemplateStaticConfig::new(base_url.to_string());
		init_static_resolver(config);

		let result = resolve_static(path);

		assert_eq!(result, expected);
	}
}

#[cfg(wasm)]
mod wasm_decision_table_tests {
	use reinhardt_pages::static_resolver::{init_static_resolver, resolve_static};
	use wasm_bindgen_test::*;

	wasm_bindgen_test_configure!(run_in_browser);

	#[wasm_bindgen_test]
	fn test_wasm_decision_table_basic() {
		init_static_resolver("/static/".to_string());

		let no_query = resolve_static("test.css");
		let with_query = resolve_static("test.css?v=1");

		assert_eq!(no_query, "/static/test.css");
		assert_eq!(with_query, "/static/test.css?v=1");
	}
}

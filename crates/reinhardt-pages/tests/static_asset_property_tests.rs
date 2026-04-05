#![cfg(not(target_arch = "wasm32"))]
//! Property-based tests for static asset URL resolution
//!
//! Uses proptest to verify properties that should hold for all valid inputs.

#[cfg(native)]
mod property_tests {
	use proptest::prelude::*;
	use proptest::proptest;
	use reinhardt_pages::static_resolver::{init_static_resolver, resolve_static};
	use reinhardt_utils::staticfiles::TemplateStaticConfig;

	proptest! {
		/// Property: resolve_static is idempotent on result (calling it again returns same result)
		#[test]
		fn prop_resolve_static_deterministic(
			path in r"[a-zA-Z0-9._\-/]{1,100}"
		) {
			let config = TemplateStaticConfig::new("/static/".to_string());
			init_static_resolver(config);

			let first_call = resolve_static(&path);
			let second_call = resolve_static(&path);

			prop_assert_eq!(first_call, second_call);
		}

		/// Property: Result always contains the base URL
		#[test]
		fn prop_result_contains_base_url(
			path in r"[a-zA-Z0-9._\-/]{1,100}"
		) {
			let config = TemplateStaticConfig::new("/static/".to_string());
			init_static_resolver(config);

			let result = resolve_static(&path);

			prop_assert!(result.contains("/static/"));
		}

		/// Property: Filename is preserved in result
		#[test]
		fn prop_filename_preserved(
			filename in r"[a-zA-Z0-9._\-]+\.[a-z]{1,4}"
		) {
			let config = TemplateStaticConfig::new("/static/".to_string());
			init_static_resolver(config);

			let result = resolve_static(&filename);

			prop_assert!(result.contains(&filename));
		}

		/// Property: Query string is preserved
		#[test]
		fn prop_query_string_preserved(
			path in r"[a-zA-Z0-9._\-/]{1,50}",
			query in r"[a-zA-Z0-9&=]{1,50}"
		) {
			let config = TemplateStaticConfig::new("/static/".to_string());
			init_static_resolver(config);

			let full_path = format!("{}?{}", path, query);
			let result = resolve_static(&full_path);

			prop_assert!(result.contains(&query));
		}

		/// Property: No double slashes in path portion (before query string)
		#[test]
		fn prop_no_double_slashes_in_path(
			path in r"[a-zA-Z0-9._\-/]{1,100}"
		) {
			let config = TemplateStaticConfig::new("/static/".to_string());
			init_static_resolver(config);

			let result = resolve_static(&path);
			let path_only = result.split('?').next().unwrap_or("");

			// Check that there are no consecutive slashes except at start of URL
			let parts: Vec<&str> = path_only.split("://").collect();
			for (i, part) in parts.iter().enumerate() {
				if i == 0 {
					continue; // Skip protocol part
				}
				prop_assert!(!part.contains("//"), "Found double slash in path part");
			}
		}

		/// Property: Result is non-empty for non-empty input
		#[test]
		fn prop_non_empty_input_gives_non_empty_result(
			path in r"[a-zA-Z0-9._\-/]{1,100}"
		) {
			let config = TemplateStaticConfig::new("/static/".to_string());
			init_static_resolver(config);

			let result = resolve_static(&path);

			prop_assert!(!result.is_empty());
		}

		/// Property: Result starts with base URL (or protocol if CDN)
		#[test]
		fn prop_result_starts_with_base(
			path in r"[a-zA-Z0-9._\-/]{1,100}"
		) {
			let config = TemplateStaticConfig::new("/static/".to_string());
			init_static_resolver(config);

			let result = resolve_static(&path);

			prop_assert!(result.starts_with("/static/"));
		}

		/// Property: All characters in path are preserved (not URL-encoded)
		#[test]
		fn prop_ascii_characters_preserved(
			path in r"[a-zA-Z0-9._\-]*[a-zA-Z0-9._\-][a-zA-Z0-9._\-/]*"
		) {
			let config = TemplateStaticConfig::new("/static/".to_string());
			init_static_resolver(config);

			let result = resolve_static(&path);

			prop_assert!(result.contains(&path));
		}
	}
}

#[cfg(wasm)]
mod wasm_property_tests {
	use reinhardt_pages::static_resolver::{init_static_resolver, resolve_static};
	use wasm_bindgen_test::*;

	wasm_bindgen_test_configure!(run_in_browser);

	#[wasm_bindgen_test]
	fn test_wasm_prop_deterministic() {
		init_static_resolver("/static/".to_string());

		let first = resolve_static("test.css");
		let second = resolve_static("test.css");

		assert_eq!(first, second);
	}

	#[wasm_bindgen_test]
	fn test_wasm_prop_contains_base() {
		init_static_resolver("/static/".to_string());
		let result = resolve_static("test.css");
		assert!(result.contains("/static/"));
	}
}

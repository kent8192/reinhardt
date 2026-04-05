#![cfg(not(target_arch = "wasm32"))]
//! Fuzz tests for static asset URL resolution
//!
//! Tests that the resolver never panics with arbitrary inputs (fuzzing).
//! Uses proptest to generate random inputs.

#[cfg(native)]
mod fuzz_tests {
	use proptest::prelude::*;
	use reinhardt_pages::static_resolver::{init_static_resolver, resolve_static};
	use reinhardt_utils::staticfiles::TemplateStaticConfig;

	proptest! {
		/// Fuzz test: resolve_static never panics with random ASCII paths
		#[test]
		fn fuzz_resolve_static_ascii_paths_never_panic(
			path in "[ -~]{0,1000}"
		) {
			let config = TemplateStaticConfig::new("/static/".to_string());
			init_static_resolver(config);

			// Should never panic
			let result = resolve_static(&path);
			prop_assert!(!result.is_empty() || path.is_empty());
		}

		/// Fuzz test: resolve_static never panics with random UTF-8 strings
		#[test]
		fn fuzz_resolve_static_unicode_never_panic(
			path in ".*"
		) {
			let config = TemplateStaticConfig::new("/static/".to_string());
			init_static_resolver(config);

			// Should never panic
			let _result = resolve_static(&path);
		}

		/// Fuzz test: resolve_static never panics with various base URLs
		#[test]
		fn fuzz_resolve_static_random_base_urls_never_panic(
			base_url in "[a-zA-Z0-9:/.\\-]{1,200}",
			path in "[a-zA-Z0-9._\\-/]{0,100}"
		) {
			let config = TemplateStaticConfig::new(base_url);
			init_static_resolver(config);

			// Should never panic
			let _result = resolve_static(&path);
		}

		/// Fuzz test: resolve_static never panics with paths containing special characters
		#[test]
		fn fuzz_resolve_static_special_chars_never_panic(
			path in "[!@#$%^&*()_+=\\[\\]{};:',.<>?/\\\\~`| -~]{0,200}"
		) {
			let config = TemplateStaticConfig::new("/static/".to_string());
			init_static_resolver(config);

			// Should never panic
			let _result = resolve_static(&path);
		}

		/// Fuzz test: TemplateStaticConfig.resolve_url never panics
		#[test]
		fn fuzz_template_config_resolve_never_panics(
			base_url in ".*",
			path in ".*"
		) {
			// Create config with fuzzy inputs
			let config = TemplateStaticConfig::new(base_url);

			// Should never panic
			let _result = config.resolve_url(&path);
		}

		/// Fuzz test: Multiple sequential calls don't panic
		#[test]
		fn fuzz_sequential_calls_never_panic(
			paths in prop::collection::vec("[a-zA-Z0-9._\\-/]*", 1..10)
		) {
			let config = TemplateStaticConfig::new("/static/".to_string());
			init_static_resolver(config);

			for path in paths {
				let _result = resolve_static(&path);
			}
		}

		/// Fuzz test: Extremely long paths don't cause issues
		#[test]
		fn fuzz_very_long_paths_never_panic(
			len in 0usize..10000
		) {
			let config = TemplateStaticConfig::new("/static/".to_string());
			init_static_resolver(config);

			let long_path = "a".repeat(len);
			let _result = resolve_static(&long_path);
		}

		/// Fuzz test: Paths with null bytes don't panic (won't occur in Rust strings)
		#[test]
		fn fuzz_paths_with_special_sequences_never_panic(
			path in r"[a-zA-Z0-9._\-/\s]{0,200}"
		) {
			let config = TemplateStaticConfig::new("/static/".to_string());
			init_static_resolver(config);

			let _result = resolve_static(&path);
		}
	}
}

#[cfg(wasm)]
mod wasm_fuzz_tests {
	use reinhardt_pages::static_resolver::{init_static_resolver, resolve_static};
	use wasm_bindgen_test::*;

	wasm_bindgen_test_configure!(run_in_browser);

	#[wasm_bindgen_test]
	fn test_wasm_fuzz_basic() {
		init_static_resolver("/static/".to_string());

		let inputs = vec![
			"",
			"test.css",
			"path/to/file.js",
			"/leading/slash.css",
			"file with spaces.css",
		];

		for input in inputs {
			let _ = resolve_static(input);
		}
	}
}

#![cfg(not(target_arch = "wasm32"))]
//! Error path tests for static asset URL resolution
//!
//! These tests verify that the system gracefully handles error conditions
//! and edge cases that should produce errors or fallback behavior.

#[cfg(native)]
mod error_tests {
	use reinhardt_pages::static_resolver::{init_static_resolver, resolve_static};
	use reinhardt_utils::staticfiles::TemplateStaticConfig;
	use rstest::rstest;
	use std::collections::HashMap;

	/// Test resolution when file not in manifest (fallback behavior)
	#[rstest]
	fn test_resolve_static_file_not_in_manifest() {
		let mut manifest = HashMap::new();
		manifest.insert(
			"css/known.css".to_string(),
			"css/known.hash.css".to_string(),
		);

		let config = TemplateStaticConfig::new("/static/".to_string()).with_manifest(manifest);
		init_static_resolver(config);

		let result = resolve_static("css/unknown.css");

		// Should fallback to original path instead of failing
		assert_eq!(result, "/static/css/unknown.css");
		assert!(!result.contains("hash"));
	}

	/// Test with empty path
	#[rstest]
	fn test_resolve_static_empty_path() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static("");

		// Should handle gracefully, returning just the base URL
		assert_eq!(result, "/static/");
	}

	/// Test manifest creation with empty HashMap
	#[rstest]
	fn test_resolve_static_empty_manifest() {
		let manifest = HashMap::new();

		let config = TemplateStaticConfig::new("/static/".to_string()).with_manifest(manifest);
		init_static_resolver(config);

		let result = resolve_static("any/file.css");

		// Should fallback to original path when manifest is empty
		assert_eq!(result, "/static/any/file.css");
	}

	/// Test with only slashes in path
	#[rstest]
	fn test_resolve_static_only_slashes() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static("///");

		// Should handle gracefully without panicking
		assert!(!result.is_empty());
		assert!(result.contains("static"));
	}

	/// Test from_storage with non-existent manifest file
	#[rstest]
	#[tokio::test]
	async fn test_from_storage_nonexistent_manifest() {
		use reinhardt_utils::staticfiles::ManifestStaticFilesStorage;
		use tempfile::tempdir;

		let temp_dir = tempdir().unwrap();
		let storage = ManifestStaticFilesStorage::new(temp_dir.path().to_path_buf(), "/static/");

		let config = TemplateStaticConfig::from_storage(&storage).await;

		// Should succeed with empty manifest instead of failing
		assert!(config.is_ok());
		let cfg = config.unwrap();
		assert!(!cfg.use_manifest);
		assert!(cfg.manifest.is_empty());
	}

	/// Test configuration with malformed manifest JSON
	#[rstest]
	#[tokio::test]
	async fn test_from_storage_invalid_json() {
		use reinhardt_utils::staticfiles::ManifestStaticFilesStorage;
		use std::io;
		use tempfile::tempdir;

		let temp_dir = tempdir().unwrap();
		let manifest_path = temp_dir.path().join("staticfiles.json");

		// Write invalid JSON
		std::fs::write(&manifest_path, "{ invalid json }").unwrap();

		let storage = ManifestStaticFilesStorage::new(temp_dir.path().to_path_buf(), "/static/");

		let config = TemplateStaticConfig::from_storage(&storage).await;

		// Should return error for invalid JSON
		assert!(config.is_err());
		match config.err().unwrap().kind() {
			io::ErrorKind::InvalidData => {
				// Expected error type
			}
			_ => panic!("Expected InvalidData error"),
		}
	}

	/// Test from_storage with truncated manifest file
	#[rstest]
	#[tokio::test]
	async fn test_from_storage_truncated_json() {
		use reinhardt_utils::staticfiles::ManifestStaticFilesStorage;
		use std::io;
		use tempfile::tempdir;

		let temp_dir = tempdir().unwrap();
		let manifest_path = temp_dir.path().join("staticfiles.json");

		// Write truncated JSON
		std::fs::write(&manifest_path, "{\"css/style.css\": \"css/style.hash").unwrap();

		let storage = ManifestStaticFilesStorage::new(temp_dir.path().to_path_buf(), "/static/");

		let config = TemplateStaticConfig::from_storage(&storage).await;

		// Should return error for truncated JSON
		assert!(config.is_err());
		match config.err().unwrap().kind() {
			io::ErrorKind::InvalidData => {
				// Expected error type
			}
			_ => panic!("Expected InvalidData error"),
		}
	}

	/// Test configuration with manifest key having no value
	#[rstest]
	fn test_resolve_static_manifest_missing_value() {
		let mut manifest = HashMap::new();
		manifest.insert("css/style.css".to_string(), String::new());

		let config = TemplateStaticConfig::new("/static/".to_string()).with_manifest(manifest);
		init_static_resolver(config);

		let result = resolve_static("css/style.css");

		// Should handle empty string value gracefully
		assert!(!result.is_empty());
	}
}

#[cfg(wasm)]
mod wasm_error_tests {
	use reinhardt_pages::static_resolver::{init_static_resolver, resolve_static};
	use wasm_bindgen_test::*;

	wasm_bindgen_test_configure!(run_in_browser);

	#[wasm_bindgen_test]
	fn test_wasm_resolve_static_empty_path() {
		init_static_resolver("/static/".to_string());
		let result = resolve_static("");
		assert_eq!(result, "/static/");
	}

	#[wasm_bindgen_test]
	fn test_wasm_resolve_static_only_slashes() {
		init_static_resolver("/static/".to_string());
		let result = resolve_static("///");
		assert!(!result.is_empty());
	}
}

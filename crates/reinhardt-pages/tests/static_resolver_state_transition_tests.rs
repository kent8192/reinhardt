#![cfg(not(target_arch = "wasm32"))]
//! State transition tests for static resolver
//!
//! These tests verify that the resolver correctly transitions between states
//! (uninitialized → initialized) and handles state-dependent behavior.
//! Uses serial_test to ensure tests don't interfere with each other.

#[cfg(native)]
mod state_transition_tests {
	use reinhardt_pages::static_resolver::{init_static_resolver, is_initialized, resolve_static};
	use reinhardt_utils::staticfiles::TemplateStaticConfig;
	use rstest::rstest;
	use serial_test::serial;

	/// Helper to reset the global state by creating a new resolver instance
	/// Note: In practice, OnceLock can only be set once, so state transitions
	/// are limited. This test verifies the first initialization behavior.
	#[rstest]
	#[serial]
	fn test_state_transition_uninitialized_to_initialized() {
		// Check initial state is uninitialized (or may be from previous tests)
		// Our test verifies that after calling init, is_initialized returns true
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		assert!(
			is_initialized(),
			"Should be initialized after init_static_resolver"
		);
	}

	/// Test that multiple initializations are idempotent
	/// Due to OnceLock semantics, second init should be ignored
	#[rstest]
	#[serial]
	fn test_state_transition_multiple_init_idempotent() {
		let config1 = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config1);

		let config2 = TemplateStaticConfig::new("/different/".to_string());
		init_static_resolver(config2);

		// The first config should still be in use
		let result = resolve_static("test.css");
		assert!(
			result.contains("/static/"),
			"First config should still be active"
		);
		assert!(
			!result.contains("/different/"),
			"Second config should be ignored"
		);
	}

	/// Test that resolve_static works after initialization
	#[rstest]
	#[serial]
	fn test_state_transition_resolve_after_init() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);

		let result = resolve_static("css/style.css");

		assert_eq!(result, "/static/css/style.css");
		assert!(is_initialized());
	}

	/// Test that fallback behavior occurs for uninitialized state (on fresh runs)
	/// Note: This test assumes it runs in an environment where the resolver
	/// hasn't been initialized yet
	#[rstest]
	fn test_state_fallback_when_uninitialized() {
		// This test checks fallback behavior - it should still work even if
		// initialization was somehow missed
		let result = resolve_static("fallback-test.css");

		// Should return a valid URL even without initialization
		assert!(!result.is_empty());
		assert!(result.contains("fallback-test.css"));
		// Should use default /static/ prefix as fallback
		assert!(result.contains("static"));
	}

	/// Test that is_initialized correctly reflects state
	#[rstest]
	#[serial]
	fn test_state_is_initialized_reflects_state() {
		// After previous tests, may already be initialized
		// So we just verify consistency
		let _before = is_initialized();
		let config = TemplateStaticConfig::new("/static/".to_string());
		init_static_resolver(config);
		let after = is_initialized();

		// After init, should be true
		assert!(after, "Should be initialized after init");
		// Second call should also return true (idempotent)
		assert_eq!(after, is_initialized());
	}

	/// Test state transition with manifest
	#[rstest]
	#[serial]
	fn test_state_transition_with_manifest_activation() {
		use std::collections::HashMap;

		let mut manifest = HashMap::new();
		manifest.insert(
			"css/style.css".to_string(),
			"css/style.hash.css".to_string(),
		);

		let config = TemplateStaticConfig::new("/static/".to_string()).with_manifest(manifest);
		init_static_resolver(config);

		let result = resolve_static("css/style.css");

		// Manifest should be active
		assert_eq!(result, "/static/css/style.hash.css");
		assert!(is_initialized());
	}

	/// Test that different base URLs can be used in sequence
	#[rstest]
	#[serial]
	fn test_state_transition_with_different_base_urls() {
		let config = TemplateStaticConfig::new("/assets/".to_string());
		init_static_resolver(config);

		let result = resolve_static("test.css");

		assert!(
			result.contains("/assets/"),
			"Should use initialized base URL"
		);
		assert_eq!(result, "/assets/test.css");
	}
}

#[cfg(wasm)]
mod wasm_state_transition_tests {
	use reinhardt_pages::static_resolver::{init_static_resolver, is_initialized, resolve_static};
	use serial_test::serial;
	use wasm_bindgen_test::*;

	wasm_bindgen_test_configure!(run_in_browser);

	#[wasm_bindgen_test]
	#[serial]
	fn test_wasm_state_transition_init() {
		init_static_resolver("/static/".to_string());
		assert!(is_initialized());
	}

	#[wasm_bindgen_test]
	#[serial]
	fn test_wasm_state_resolve_after_init() {
		init_static_resolver("/static/".to_string());
		let result = resolve_static("test.css");
		assert_eq!(result, "/static/test.css");
	}
}

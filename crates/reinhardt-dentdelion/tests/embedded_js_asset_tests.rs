//! Content verification tests for embedded JavaScript assets.
//!
//! Verifies that embedded Preact JavaScript files are not empty stubs
//! and contain expected functional markers. Covers Issue #3123.

use rstest::*;

const PREACT_CORE: &str = include_str!("../src/wasm/js/preact.min.js");
const PREACT_RENDER_TO_STRING: &str = include_str!("../src/wasm/js/preact-render-to-string.min.js");

#[rstest]
fn preact_core_is_not_empty() {
	// Assert
	assert!(!PREACT_CORE.is_empty(), "preact.min.js should not be empty");
	assert!(
		PREACT_CORE.len() > 100,
		"preact.min.js should have substantial content (got {} bytes)",
		PREACT_CORE.len()
	);
}

#[rstest]
fn preact_core_contains_functional_markers() {
	// Assert - Preact core should contain framework-identifying code
	assert!(
		PREACT_CORE.contains("createElement") || PREACT_CORE.contains("h("),
		"preact.min.js should contain createElement or h() function"
	);
}

#[rstest]
fn preact_render_to_string_is_not_empty() {
	// Assert
	assert!(
		!PREACT_RENDER_TO_STRING.is_empty(),
		"preact-render-to-string.min.js should not be empty"
	);
	assert!(
		PREACT_RENDER_TO_STRING.len() > 100,
		"preact-render-to-string.min.js should have substantial content (got {} bytes)",
		PREACT_RENDER_TO_STRING.len()
	);
}

#[rstest]
fn preact_render_to_string_contains_functional_markers() {
	// Assert - render-to-string should contain rendering-related code
	assert!(
		PREACT_RENDER_TO_STRING.contains("render") || PREACT_RENDER_TO_STRING.contains("html"),
		"preact-render-to-string.min.js should contain render-related code"
	);
}

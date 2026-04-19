//! Static Manifest Lifecycle Tests
//!
//! Verifies the specification invariants of the OnceLock-based static file manifest:
//! `init_static_context()` → `resolve_static_url()`
//!
//! The key contract: calling resolve before init must return an error,
//! and double init must be rejected.

use reinhardt_pages::integ::static_context::{
	init_static_context, reset_static_context, resolve_static_url,
};
use rstest::rstest;
use serial_test::serial;
use std::collections::HashMap;

/// Specification: `resolve_static_url()` must return `Err` before initialization.
#[rstest]
#[serial(static_context)]
fn resolve_before_init_returns_error() {
	// Arrange
	reset_static_context();

	// Act
	let result = resolve_static_url("images/logo.png");

	// Assert
	assert!(
		result.is_err(),
		"resolve_static_url must return Err before init"
	);
	let err_msg = result.unwrap_err();
	assert!(
		err_msg.contains("not initialized"),
		"error must mention initialization, got: {err_msg}"
	);
}

/// Specification: After initialization, `resolve_static_url()` must return the
/// versioned URL from the manifest.
#[rstest]
#[serial(static_context)]
fn resolve_after_init_returns_versioned_url() {
	// Arrange
	reset_static_context();
	let mut manifest = HashMap::new();
	manifest.insert(
		"images/logo.png".to_string(),
		"images/logo.abc123.png".to_string(),
	);
	init_static_context(manifest).expect("init must succeed on fresh state");

	// Act
	let result = resolve_static_url("images/logo.png");

	// Assert
	assert_eq!(
		result.unwrap(),
		"/static/images/logo.abc123.png",
		"must resolve to versioned URL from manifest"
	);
}

/// Specification: Double initialization must be rejected (OnceLock single-init).
#[rstest]
#[serial(static_context)]
fn double_init_returns_error() {
	// Arrange
	reset_static_context();
	init_static_context(HashMap::new()).expect("first init must succeed");

	// Act
	let result = init_static_context(HashMap::new());

	// Assert
	assert!(
		result.is_err(),
		"second init_static_context must return Err"
	);
}

/// Specification: For paths not in the manifest, resolve falls back to `/static/{path}`.
#[rstest]
#[serial(static_context)]
fn resolve_unknown_path_falls_back_to_static_prefix() {
	// Arrange
	reset_static_context();
	init_static_context(HashMap::new()).expect("init must succeed");

	// Act
	let result = resolve_static_url("unknown/file.css");

	// Assert
	assert_eq!(
		result.unwrap(),
		"/static/unknown/file.css",
		"unknown path must fall back to /static/ prefix"
	);
}

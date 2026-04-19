//! URL Routes Lifecycle Tests
//!
//! Verifies the specification invariants of the OnceLock-based URL resolver:
//! `init_url_resolver()` → `resolve_url()`
//!
//! The key contract: calling resolve before init must return `NotInitialized`,
//! and double init must be rejected.

use reinhardt_pages::integ::url_resolver::{
	UrlResolveError, init_url_resolver, reset_url_resolver, resolve_url,
};
use rstest::rstest;
use serial_test::serial;
use std::collections::HashMap;

/// Specification: `resolve_url()` must return `NotInitialized` before initialization.
#[rstest]
#[serial(url_resolver)]
fn resolve_before_init_returns_not_initialized() {
	// Arrange
	reset_url_resolver();

	// Act
	let result = resolve_url("home");

	// Assert
	assert_eq!(
		result,
		Err(UrlResolveError::NotInitialized),
		"resolve_url must return NotInitialized before init"
	);
}

/// Specification: After initialization, `resolve_url()` returns the mapped URL.
#[rstest]
#[serial(url_resolver)]
fn resolve_after_init_returns_url() {
	// Arrange
	reset_url_resolver();
	let mut routes = HashMap::new();
	routes.insert("home".to_string(), "/".to_string());
	routes.insert("user-profile".to_string(), "/users/{id}".to_string());
	init_url_resolver(routes).expect("init must succeed on fresh state");

	// Act
	let home = resolve_url("home");
	let profile = resolve_url("user-profile");

	// Assert
	assert_eq!(home.unwrap(), "/");
	assert_eq!(profile.unwrap(), "/users/{id}");
}

/// Specification: Double initialization must be rejected (OnceLock single-init).
#[rstest]
#[serial(url_resolver)]
fn double_init_returns_error() {
	// Arrange
	reset_url_resolver();
	init_url_resolver(HashMap::new()).expect("first init must succeed");

	// Act
	let result = init_url_resolver(HashMap::new());

	// Assert
	assert!(result.is_err(), "second init_url_resolver must return Err");
}

/// Specification: Resolving an unknown route name must return `RouteNotFound`.
#[rstest]
#[serial(url_resolver)]
fn resolve_unknown_route_returns_route_not_found() {
	// Arrange
	reset_url_resolver();
	init_url_resolver(HashMap::new()).expect("init must succeed");

	// Act
	let result = resolve_url("nonexistent");

	// Assert
	assert_eq!(
		result,
		Err(UrlResolveError::RouteNotFound {
			route_name: "nonexistent".to_string(),
		}),
		"unknown route must return RouteNotFound"
	);
}

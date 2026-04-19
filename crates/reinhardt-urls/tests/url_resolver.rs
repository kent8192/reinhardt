//! Tests for the `UrlResolver` trait.

#![cfg(native)]

use reinhardt_urls::routers::resolver::UrlResolver;
use rstest::*;
use std::collections::HashMap;

/// Minimal `UrlResolver` implementation for testing.
struct MockResolver {
	routes: HashMap<String, String>,
}

impl MockResolver {
	fn new() -> Self {
		let mut routes = HashMap::new();
		routes.insert("home".to_string(), "/".to_string());
		routes.insert("user_detail".to_string(), "/users/{id}/".to_string());
		Self { routes }
	}
}

impl UrlResolver for MockResolver {
	fn resolve_url(&self, name: &str, params: &[(&str, &str)]) -> String {
		let mut url = self
			.routes
			.get(name)
			.unwrap_or_else(|| panic!("Route '{}' not found", name))
			.clone();
		for (key, value) in params {
			url = url.replace(&format!("{{{}}}", key), value);
		}
		url
	}
}

#[rstest]
fn resolve_url_without_params() {
	// Arrange
	let resolver = MockResolver::new();

	// Act
	let url = resolver.resolve_url("home", &[]);

	// Assert
	assert_eq!(url, "/");
}

#[rstest]
fn resolve_url_with_params() {
	// Arrange
	let resolver = MockResolver::new();

	// Act
	let url = resolver.resolve_url("user_detail", &[("id", "42")]);

	// Assert
	assert_eq!(url, "/users/42/");
}

#[rstest]
#[should_panic(expected = "Route 'nonexistent' not found")]
fn resolve_url_panics_on_unknown_route() {
	// Arrange
	let resolver = MockResolver::new();

	// Act
	resolver.resolve_url("nonexistent", &[]);
}

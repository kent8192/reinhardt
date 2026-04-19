//! Integration test for the URL resolver extension trait pattern.
//!
//! Validates that the extension trait + blanket impl pattern works
//! correctly with a mock resolver (simulating what the macros generate).

#![cfg(native)]

use reinhardt_urls::routers::resolver::UrlResolver;
use rstest::*;
use std::collections::HashMap;

// Simulate what #[get("/login/", name = "auth_login")] generates
trait ResolveAuthLogin: UrlResolver {
	fn auth_login(&self) -> String {
		self.resolve_url("auth_login", &[])
	}
}
impl<T: UrlResolver> ResolveAuthLogin for T {}

// Simulate what #[get("/{id}/", name = "cluster_retrieve")] generates
trait ResolveClusterRetrieve: UrlResolver {
	fn cluster_retrieve(&self, id: &str) -> String {
		self.resolve_url("cluster_retrieve", &[("id", id)])
	}
}
impl<T: UrlResolver> ResolveClusterRetrieve for T {}

// Simulate what #[get("/{user_id}/posts/{post_id}/", name = "user_post")] generates
trait ResolveUserPost: UrlResolver {
	fn user_post(&self, user_id: &str, post_id: &str) -> String {
		self.resolve_url("user_post", &[("user_id", user_id), ("post_id", post_id)])
	}
}
impl<T: UrlResolver> ResolveUserPost for T {}

struct TestResolver {
	routes: HashMap<String, String>,
}

impl TestResolver {
	fn new() -> Self {
		let mut routes = HashMap::new();
		routes.insert("auth_login".to_string(), "/api/auth/login/".to_string());
		routes.insert(
			"cluster_retrieve".to_string(),
			"/api/clusters/{id}/".to_string(),
		);
		routes.insert(
			"user_post".to_string(),
			"/api/users/{user_id}/posts/{post_id}/".to_string(),
		);
		Self { routes }
	}
}

impl UrlResolver for TestResolver {
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
fn extension_trait_no_params() {
	// Arrange
	let resolver = TestResolver::new();

	// Act
	let url = resolver.auth_login();

	// Assert
	assert_eq!(url, "/api/auth/login/");
}

#[rstest]
fn extension_trait_single_param() {
	// Arrange
	let resolver = TestResolver::new();

	// Act
	let url = resolver.cluster_retrieve("abc-123");

	// Assert
	assert_eq!(url, "/api/clusters/abc-123/");
}

#[rstest]
fn extension_trait_multiple_params() {
	// Arrange
	let resolver = TestResolver::new();

	// Act
	let url = resolver.user_post("42", "99");

	// Assert
	assert_eq!(url, "/api/users/42/posts/99/");
}

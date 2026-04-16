#![cfg(feature = "client-router")]
//! Integration tests for client-side URL resolution.

use rstest::*;
use serial_test::serial;

use reinhardt_core::page::Page;
use reinhardt_urls::routers::client_router::{
	ClientRouter, ClientUrlReverser, clear_client_reverser, get_client_reverser,
	register_client_reverser,
};
use reinhardt_urls::routers::resolver::ClientUrlResolver;

/// Stub page component for route registration.
fn stub_page() -> Page {
	Page::Empty
}

#[rstest]
#[serial(client_reverser)]
fn test_client_url_reverser_round_trip_via_client_router() {
	// Arrange
	clear_client_reverser();
	let router = ClientRouter::new()
		.named_route("app:home", "/", stub_page)
		.named_route("app:user_detail", "/users/{id}/", stub_page)
		.named_route(
			"app:user_posts",
			"/users/{user_id}/posts/{post_id}/",
			stub_page,
		);

	// Act
	let reverser = router.to_reverser();
	register_client_reverser(reverser);
	let retrieved = get_client_reverser().expect("reverser should be registered");

	// Assert
	assert_eq!(retrieved.reverse("app:home", &[]), Some("/".to_string()));
	assert_eq!(
		retrieved.reverse("app:user_detail", &[("id", "42")]),
		Some("/users/42/".to_string())
	);
	assert_eq!(
		retrieved.reverse("app:user_posts", &[("user_id", "5"), ("post_id", "10")]),
		Some("/users/5/posts/10/".to_string())
	);
	assert_eq!(retrieved.reverse("nonexistent", &[]), None);

	// Cleanup
	clear_client_reverser();
}

/// Test that ClientUrlResolver trait works with ClientUrlReverser
#[rstest]
fn test_client_url_resolver_trait_impl() {
	// Arrange
	use std::collections::HashMap;

	let mut patterns = HashMap::new();
	patterns.insert("myapp:index".to_string(), "/".to_string());
	patterns.insert("myapp:detail".to_string(), "/items/{id}/".to_string());
	let reverser = std::sync::Arc::new(ClientUrlReverser::new(patterns));

	// Create a simple resolver wrapper
	struct TestResolver {
		reverser: std::sync::Arc<ClientUrlReverser>,
	}

	impl ClientUrlResolver for TestResolver {
		fn resolve_client_url(&self, name: &str, params: &[(&str, &str)]) -> String {
			self.reverser
				.reverse(name, params)
				.unwrap_or_else(|| panic!("Route '{}' not found", name))
		}
	}

	let resolver = TestResolver { reverser };

	// Act & Assert
	assert_eq!(resolver.resolve_client_url("myapp:index", &[]), "/");
	assert_eq!(
		resolver.resolve_client_url("myapp:detail", &[("id", "99")]),
		"/items/99/"
	);
}

#[rstest]
#[should_panic(expected = "not found")]
fn test_client_url_resolver_panics_on_unknown_route() {
	// Arrange
	use std::collections::HashMap;

	let reverser = ClientUrlReverser::new(HashMap::new());

	struct TestResolver {
		reverser: ClientUrlReverser,
	}

	impl ClientUrlResolver for TestResolver {
		fn resolve_client_url(&self, name: &str, params: &[(&str, &str)]) -> String {
			self.reverser
				.reverse(name, params)
				.unwrap_or_else(|| panic!("Route '{}' not found", name))
		}
	}

	let resolver = TestResolver { reverser };

	// Act — should panic
	resolver.resolve_client_url("nonexistent", &[]);
}

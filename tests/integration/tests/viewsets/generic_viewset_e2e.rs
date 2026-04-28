//! End-to-end test for `GenericViewSet` error guidance (Issue #3985 follow-up).
//!
//! `GenericViewSet<T>` is an abstract base ViewSet without built-in CRUD logic.
//! Before #3985 it returned the bare error `"Action not implemented"` which
//! gave users no path forward when they wired it into a router by mistake.
//!
//! These tests confirm the improved guidance message that now points users at
//! `ModelViewSet` / `ReadOnlyModelViewSet` or hand-rolled `impl ViewSet`.

use bytes::Bytes;
use hyper::{HeaderMap, Method, Version};
use reinhardt_apps::Request;
use reinhardt_urls::routers::{DefaultRouter, Router};
use reinhardt_views::viewsets::GenericViewSet;
use std::sync::Arc;

fn make_request(method: Method, uri: &str) -> Request {
	Request::builder()
		.method(method)
		.uri(uri)
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap()
}

#[tokio::test]
async fn generic_viewset_returns_guidance_message_on_dispatch() {
	// Arrange: register a bare GenericViewSet — typical "wired-up by mistake"
	// case that this test simulates.
	let mut router = DefaultRouter::new();
	let viewset: Arc<GenericViewSet<()>> = Arc::new(GenericViewSet::new("widgets", ()));
	router.register_viewset("widgets", viewset);

	// Act
	let result = router.route(make_request(Method::GET, "/widgets/")).await;

	// Assert: must NOT return a silent 200 (the placeholder regression), and
	// the error must contain actionable guidance.
	match result {
		Ok(resp) => panic!(
			"REGRESSION (#3985): GenericViewSet should not return a successful response, got status {}",
			resp.status
		),
		Err(e) => {
			let msg = e.to_string();
			assert!(
				msg.contains("GenericViewSet has no built-in CRUD"),
				"expected guidance message; got: {msg}"
			);
			assert!(
				msg.contains("ModelViewSet"),
				"expected ModelViewSet recommendation in error; got: {msg}"
			);
		}
	}
}

#[tokio::test]
async fn generic_viewset_guidance_message_mentions_custom_dispatch() {
	let mut router = DefaultRouter::new();
	let viewset: Arc<GenericViewSet<()>> = Arc::new(GenericViewSet::new("widgets", ()));
	router.register_viewset("widgets", viewset);

	let result = router.route(make_request(Method::POST, "/widgets/")).await;

	let err = result.expect_err("GenericViewSet must surface an error, not a placeholder 201");
	let msg = err.to_string();
	assert!(
		msg.contains("impl ViewSet for YourType") || msg.contains("hand-written dispatch"),
		"error must guide users toward custom impl ViewSet; got: {msg}"
	);
}

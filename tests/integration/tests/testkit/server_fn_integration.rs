use http::{HeaderMap, HeaderValue};
use reinhardt_di::SingletonScope;
use reinhardt_test::server_fn::{MockHttpRequest, ServerFnTestContext, TransactionMode};
use rstest::rstest;
use std::sync::Arc;

#[rstest]
fn server_fn_context_builds_overrides_auth_headers_and_expected_results() {
	#[derive(Clone, Debug, PartialEq, Eq)]
	struct Marker(&'static str);

	// Arrange
	let singleton = Arc::new(SingletonScope::new());
	let request =
		MockHttpRequest::post("/api/orders?dry_run=true").with_cookie("session", "request-cookie");
	let mut headers = HeaderMap::new();
	headers.insert("x-request-id", HeaderValue::from_static("req-42"));
	let env = ServerFnTestContext::new(singleton)
		.with_database(41_u16)
		.with_singleton(Marker("configured"))
		.with_request(request.clone())
		.with_request_headers(headers)
		.with_header("x-feature", "enabled")
		.with_permissions(vec!["orders:read", "orders:write"])
		.with_roles(vec!["operator", "auditor"])
		.with_mock_session()
		.with_csrf_token("csrf-42")
		.with_transaction_mode(TransactionMode::Commit)
		.build();
	let context = ServerFnTestContext::new(Arc::new(SingletonScope::new()))
		.with_singleton(Marker("context-only"))
		.build_context();

	// Act
	let saved_pool = env.context().get_singleton::<u16>().unwrap();
	let saved_marker = env.get_singleton::<Marker>().unwrap();
	let context_marker = context.get_singleton::<Marker>().unwrap();

	// Assert
	assert_eq!(*saved_pool, 41);
	assert_eq!(*saved_marker, Marker("configured"));
	assert_eq!(*context_marker, Marker("context-only"));
	assert_eq!(
		env.mock_request.as_ref().unwrap().uri_string(),
		request.uri_string()
	);
	assert_eq!(
		env.mock_request.as_ref().unwrap().get_cookie("session"),
		Some("request-cookie")
	);
	assert!(env.is_authenticated());
	assert!(env.user_id().is_some());
	assert!(env.has_permission("orders:read"));
	assert!(!env.has_permission("orders:delete"));
	assert!(env.has_role("operator"));
	assert!(!env.has_role("admin"));
	assert_eq!(env.get_header("x-request-id"), Some("req-42"));
	assert_eq!(env.get_header("x-feature"), Some("enabled"));
	assert_eq!(env.get_header("x-csrf-token"), Some("csrf-42"));
	assert_eq!(env.csrf_token.as_deref(), Some("csrf-42"));
	assert_eq!(env.mock_session.as_ref().unwrap().csrf_token, "csrf-42");
	assert_eq!(env.transaction_mode, TransactionMode::Commit);
}

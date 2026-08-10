use http::{HeaderMap, HeaderValue};
use reinhardt_di::SingletonScope;
use reinhardt_test::auth::{ForceLoginUser, JwtTestConfig};
use reinhardt_test::server_fn::{MockHttpRequest, ServerFnTestContext, TransactionMode};
use rstest::rstest;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

struct FixtureUser(Uuid);

impl ForceLoginUser for FixtureUser {
	fn session_user_id(&self) -> String {
		self.0.to_string()
	}
}

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
	let session_user_id = env
		.mock_session
		.as_ref()
		.and_then(|session| session.user.as_ref())
		.map(|user| user.id);
	assert_eq!(session_user_id, Some(env.test_user.as_ref().unwrap().id));
	assert_eq!(env.user_id(), session_user_id);
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

#[rstest]
fn server_fn_auth_builder_preserves_session_and_jwt_identity_overrides() {
	// Arrange
	let user = FixtureUser(Uuid::from_u128(0x8a5d));
	let scope = Arc::new(SingletonScope::new());

	// Act
	let session_env = ServerFnTestContext::new(scope.clone())
		.auth()
		.session(&user)
		.with_staff(true)
		.with_superuser(true)
		.done()
		.build();
	let jwt_env = ServerFnTestContext::new(scope)
		.auth()
		.jwt(&user, &JwtTestConfig::default())
		.with_staff(true)
		.with_superuser(true)
		.done()
		.build();

	// Assert
	for env in [session_env, jwt_env] {
		let session = env.mock_session.unwrap();
		assert_eq!(session.user.as_ref().unwrap().id, user.0);
		assert_eq!(session.get_raw("user_id"), Some(&json!(user.0.to_string())));
		assert_eq!(session.get_raw("is_staff"), Some(&json!(true)));
		assert_eq!(session.get_raw("is_superuser"), Some(&json!(true)));
	}
}

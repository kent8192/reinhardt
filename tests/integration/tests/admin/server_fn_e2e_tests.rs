//! End-to-end integration tests for admin server functions
//!
//! These tests exercise the full HTTP pipeline: request → ServerRouter → route resolution
//! → DI context fork → Injectable::inject() → handler execution → response.
//!
//! Unlike the direct-call tests in other server_fn_*_tests.rs files, these tests send
//! real HTTP requests through `ServerRouter::handle()`, verifying that the DI container
//! correctly resolves all `#[inject]` parameters from `InjectionContext`.
//!
//! Covers:
//! - Issue #3046: DI resolution pipeline verification for all 10 admin handlers
//! - Issue #3049: CSRF cookie-to-header parsing and auth middleware E2E verification
//!
//! Note: Tests that require `AuthUser<AdminDefaultUser>` DB lookup are `#[ignore]`
//! because the ORM deserialization of `AdminDefaultUser` from the `auth_user` table
//! requires a matching schema that is tightly coupled to the `#[user]` macro's
//! generated `FromRow`/serde implementation. These tests will be enabled once
//! a shared auth_user table fixture is established (see #3046).

use super::server_fn_helpers::{
	TEST_CSRF_TOKEN, e2e_router_context, make_e2e_request, make_e2e_request_no_auth,
	make_e2e_request_no_csrf, make_e2e_request_wrong_csrf,
};
use hyper::StatusCode;
use reinhardt_admin::adapters::MutationRequest;
use reinhardt_admin::core::{AdminDatabase, AdminRecord};
use reinhardt_http::Handler;
use reinhardt_urls::routers::ServerRouter;
use rstest::*;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

// ==================== Helper ====================

/// Creates a test record via direct DB access and returns its ID as string.
async fn insert_test_record(db: &Arc<AdminDatabase>, name: &str, status: &str) -> String {
	let mut data = HashMap::new();
	data.insert("name".to_string(), json!(name));
	data.insert("status".to_string(), json!(status));

	let result = db
		.create::<AdminRecord>("test_models", None, data)
		.await
		.expect("Failed to insert test record");
	result.to_string()
}

// ==================== Category 1: DI Resolution Pipeline Tests (#3046) ====================

/// Verify get_dashboard resolves all DI dependencies through the HTTP pipeline.
/// Injects: Arc<AdminSite>, ServerFnRequest (no AuthUser — no DB lookup needed)
#[rstest]
#[tokio::test]
async fn test_e2e_get_dashboard_resolves_di(
	#[future] e2e_router_context: (ServerRouter, Arc<AdminDatabase>),
) {
	// Arrange
	let (router, _db) = e2e_router_context.await;
	let request = make_e2e_request("/admin/api/server_fn/get_dashboard", json!({}));

	// Act
	let response = router.handle(request).await;

	// Assert - DI resolution succeeded (not a 500 internal error)
	assert!(
		response.is_ok(),
		"get_dashboard should not return router error: {:?}",
		response.err()
	);
	let response = response.unwrap();
	assert_ne!(
		response.status,
		StatusCode::INTERNAL_SERVER_ERROR,
		"DI resolution should not fail with 500"
	);
}

/// Verify get_list resolves all DI dependencies through the HTTP pipeline.
/// Injects: Arc<AdminSite>, Arc<AdminDatabase>, AuthUser<AdminDefaultUser>
#[rstest]
#[tokio::test]
#[ignore = "Requires AdminDefaultUser ORM-compatible auth_user table schema (#3046)"]
async fn test_e2e_get_list_resolves_di(
	#[future] e2e_router_context: (ServerRouter, Arc<AdminDatabase>),
) {
	// Arrange
	let (router, _db) = e2e_router_context.await;
	let request = make_e2e_request(
		"/admin/api/server_fn/get_list",
		json!({
			"model_name": "TestModel",
			"params": {}
		}),
	);

	// Act
	let response = router.handle(request).await;

	// Assert
	assert!(
		response.is_ok(),
		"get_list should not return router error: {:?}",
		response.err()
	);
	let response = response.unwrap();
	assert_ne!(
		response.status,
		StatusCode::INTERNAL_SERVER_ERROR,
		"DI resolution should not fail with 500. Body: {}",
		String::from_utf8_lossy(&response.body)
	);
}

/// Verify get_detail resolves all DI dependencies through the HTTP pipeline.
/// Injects: Arc<AdminSite>, Arc<AdminDatabase>, ServerFnRequest, AuthUser<AdminDefaultUser>
#[rstest]
#[tokio::test]
#[ignore = "Requires AdminDefaultUser ORM-compatible auth_user table schema (#3046)"]
async fn test_e2e_get_detail_resolves_di(
	#[future] e2e_router_context: (ServerRouter, Arc<AdminDatabase>),
) {
	// Arrange
	let (router, db) = e2e_router_context.await;
	let id = insert_test_record(&db, "Detail E2E", "active").await;
	let request = make_e2e_request(
		"/admin/api/server_fn/get_detail",
		json!({
			"model_name": "TestModel",
			"id": id
		}),
	);

	// Act
	let response = router.handle(request).await;

	// Assert
	assert!(
		response.is_ok(),
		"get_detail should not return router error: {:?}",
		response.err()
	);
	let response = response.unwrap();
	assert_ne!(
		response.status,
		StatusCode::INTERNAL_SERVER_ERROR,
		"DI resolution should not fail with 500. Body: {}",
		String::from_utf8_lossy(&response.body)
	);
}

/// Verify get_fields resolves all DI dependencies through the HTTP pipeline.
/// Injects: Arc<AdminSite>, Arc<AdminDatabase>, ServerFnRequest, AuthUser<AdminDefaultUser>
#[rstest]
#[tokio::test]
#[ignore = "Requires AdminDefaultUser ORM-compatible auth_user table schema (#3046)"]
async fn test_e2e_get_fields_resolves_di(
	#[future] e2e_router_context: (ServerRouter, Arc<AdminDatabase>),
) {
	// Arrange
	let (router, _db) = e2e_router_context.await;
	let request = make_e2e_request(
		"/admin/api/server_fn/get_fields",
		json!({
			"model_name": "TestModel",
			"id": null
		}),
	);

	// Act
	let response = router.handle(request).await;

	// Assert
	assert!(
		response.is_ok(),
		"get_fields should not return router error: {:?}",
		response.err()
	);
	let response = response.unwrap();
	assert_ne!(
		response.status,
		StatusCode::INTERNAL_SERVER_ERROR,
		"DI resolution should not fail with 500. Body: {}",
		String::from_utf8_lossy(&response.body)
	);
}

/// Verify create_record resolves all DI dependencies through the HTTP pipeline.
/// Injects: Arc<AdminSite>, Arc<AdminDatabase>, ServerFnRequest, AuthUser<AdminDefaultUser>
#[rstest]
#[tokio::test]
#[ignore = "Requires AdminDefaultUser ORM-compatible auth_user table schema (#3046)"]
async fn test_e2e_create_record_resolves_di(
	#[future] e2e_router_context: (ServerRouter, Arc<AdminDatabase>),
) {
	// Arrange
	let (router, _db) = e2e_router_context.await;
	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("E2E Created"));
	data.insert("status".to_string(), json!("active"));

	let mutation = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};

	let request = make_e2e_request(
		"/admin/api/server_fn/create_record",
		json!({
			"model_name": "TestModel",
			"request": mutation
		}),
	);

	// Act
	let response = router.handle(request).await;

	// Assert
	assert!(
		response.is_ok(),
		"create_record should not return router error: {:?}",
		response.err()
	);
	let response = response.unwrap();
	assert_ne!(
		response.status,
		StatusCode::INTERNAL_SERVER_ERROR,
		"DI resolution should not fail with 500. Body: {}",
		String::from_utf8_lossy(&response.body)
	);
}

/// Verify update_record resolves all DI dependencies through the HTTP pipeline.
/// Injects: Arc<AdminSite>, Arc<AdminDatabase>, ServerFnRequest, AuthUser<AdminDefaultUser>
#[rstest]
#[tokio::test]
#[ignore = "Requires AdminDefaultUser ORM-compatible auth_user table schema (#3046)"]
async fn test_e2e_update_record_resolves_di(
	#[future] e2e_router_context: (ServerRouter, Arc<AdminDatabase>),
) {
	// Arrange
	let (router, db) = e2e_router_context.await;
	let id = insert_test_record(&db, "Update E2E", "active").await;

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("Updated E2E"));

	let mutation = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};

	let request = make_e2e_request(
		"/admin/api/server_fn/update_record",
		json!({
			"model_name": "TestModel",
			"id": id,
			"request": mutation
		}),
	);

	// Act
	let response = router.handle(request).await;

	// Assert
	assert!(
		response.is_ok(),
		"update_record should not return router error: {:?}",
		response.err()
	);
	let response = response.unwrap();
	assert_ne!(
		response.status,
		StatusCode::INTERNAL_SERVER_ERROR,
		"DI resolution should not fail with 500. Body: {}",
		String::from_utf8_lossy(&response.body)
	);
}

/// Verify delete_record resolves all DI dependencies through the HTTP pipeline.
/// Injects: Arc<AdminSite>, Arc<AdminDatabase>, ServerFnRequest, AuthUser<AdminDefaultUser>
#[rstest]
#[tokio::test]
#[ignore = "Requires AdminDefaultUser ORM-compatible auth_user table schema (#3046)"]
async fn test_e2e_delete_record_resolves_di(
	#[future] e2e_router_context: (ServerRouter, Arc<AdminDatabase>),
) {
	// Arrange
	let (router, db) = e2e_router_context.await;
	let id = insert_test_record(&db, "Delete E2E", "active").await;

	let request = make_e2e_request(
		"/admin/api/server_fn/delete_record",
		json!({
			"model_name": "TestModel",
			"id": id,
			"csrf_token": TEST_CSRF_TOKEN
		}),
	);

	// Act
	let response = router.handle(request).await;

	// Assert
	assert!(
		response.is_ok(),
		"delete_record should not return router error: {:?}",
		response.err()
	);
	let response = response.unwrap();
	assert_ne!(
		response.status,
		StatusCode::INTERNAL_SERVER_ERROR,
		"DI resolution should not fail with 500. Body: {}",
		String::from_utf8_lossy(&response.body)
	);
}

/// Verify bulk_delete_records resolves all DI dependencies through the HTTP pipeline.
/// Injects: Arc<AdminSite>, Arc<AdminDatabase>, ServerFnRequest, AuthUser<AdminDefaultUser>
#[rstest]
#[tokio::test]
#[ignore = "Requires AdminDefaultUser ORM-compatible auth_user table schema (#3046)"]
async fn test_e2e_bulk_delete_resolves_di(
	#[future] e2e_router_context: (ServerRouter, Arc<AdminDatabase>),
) {
	// Arrange
	let (router, db) = e2e_router_context.await;
	let id = insert_test_record(&db, "Bulk Delete E2E", "active").await;

	let request = make_e2e_request(
		"/admin/api/server_fn/bulk_delete_records",
		json!({
			"model_name": "TestModel",
			"request": {
				"csrf_token": TEST_CSRF_TOKEN,
				"ids": [id]
			}
		}),
	);

	// Act
	let response = router.handle(request).await;

	// Assert
	assert!(
		response.is_ok(),
		"bulk_delete_records should not return router error: {:?}",
		response.err()
	);
	let response = response.unwrap();
	assert_ne!(
		response.status,
		StatusCode::INTERNAL_SERVER_ERROR,
		"DI resolution should not fail with 500. Body: {}",
		String::from_utf8_lossy(&response.body)
	);
}

/// Verify export_data resolves all DI dependencies through the HTTP pipeline.
/// Injects: Arc<AdminSite>, Arc<AdminDatabase>, ServerFnRequest, AuthUser<AdminDefaultUser>
#[rstest]
#[tokio::test]
#[ignore = "Requires AdminDefaultUser ORM-compatible auth_user table schema (#3046)"]
async fn test_e2e_export_data_resolves_di(
	#[future] e2e_router_context: (ServerRouter, Arc<AdminDatabase>),
) {
	// Arrange
	let (router, _db) = e2e_router_context.await;
	let request = make_e2e_request(
		"/admin/api/server_fn/export_data",
		json!({
			"model_name": "TestModel",
			"format": "json"
		}),
	);

	// Act
	let response = router.handle(request).await;

	// Assert
	assert!(
		response.is_ok(),
		"export_data should not return router error: {:?}",
		response.err()
	);
	let response = response.unwrap();
	assert_ne!(
		response.status,
		StatusCode::INTERNAL_SERVER_ERROR,
		"DI resolution should not fail with 500. Body: {}",
		String::from_utf8_lossy(&response.body)
	);
}

/// Verify import_data resolves all DI dependencies through the HTTP pipeline.
/// Injects: Arc<AdminSite>, Arc<AdminDatabase>, ServerFnRequest, AuthUser<AdminDefaultUser>
#[rstest]
#[tokio::test]
#[ignore = "Requires AdminDefaultUser ORM-compatible auth_user table schema (#3046)"]
async fn test_e2e_import_data_resolves_di(
	#[future] e2e_router_context: (ServerRouter, Arc<AdminDatabase>),
) {
	// Arrange
	let (router, _db) = e2e_router_context.await;
	let import_data = json!([{"name": "Imported", "status": "active"}]).to_string();
	let request = make_e2e_request(
		"/admin/api/server_fn/import_data",
		json!({
			"model_name": "TestModel",
			"format": "json",
			"data": import_data
		}),
	);

	// Act
	let response = router.handle(request).await;

	// Assert
	assert!(
		response.is_ok(),
		"import_data should not return router error: {:?}",
		response.err()
	);
	let response = response.unwrap();
	assert_ne!(
		response.status,
		StatusCode::INTERNAL_SERVER_ERROR,
		"DI resolution should not fail with 500. Body: {}",
		String::from_utf8_lossy(&response.body)
	);
}

// ==================== Category 2: CSRF Pipeline Tests (#3049) ====================

/// Verify mutation succeeds with valid CSRF cookie and body token through HTTP pipeline.
#[rstest]
#[tokio::test]
#[ignore = "Requires AdminDefaultUser ORM-compatible auth_user table schema (#3049)"]
async fn test_e2e_mutation_csrf_valid(
	#[future] e2e_router_context: (ServerRouter, Arc<AdminDatabase>),
) {
	// Arrange
	let (router, _db) = e2e_router_context.await;
	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("CSRF Valid"));
	data.insert("status".to_string(), json!("active"));

	let mutation = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};

	let request = make_e2e_request(
		"/admin/api/server_fn/create_record",
		json!({
			"model_name": "TestModel",
			"request": mutation
		}),
	);

	// Act
	let response = router.handle(request).await;

	// Assert - should succeed (200 OK)
	let response = response.expect("Router should handle request");
	assert_eq!(
		response.status,
		StatusCode::OK,
		"Valid CSRF should succeed. Body: {}",
		String::from_utf8_lossy(&response.body)
	);
}

/// Verify mutation fails when CSRF cookie is missing from the HTTP request.
#[rstest]
#[tokio::test]
#[ignore = "Requires AdminDefaultUser ORM-compatible auth_user table schema (#3049)"]
async fn test_e2e_mutation_csrf_missing_cookie(
	#[future] e2e_router_context: (ServerRouter, Arc<AdminDatabase>),
) {
	// Arrange
	let (router, _db) = e2e_router_context.await;
	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("CSRF Missing"));
	data.insert("status".to_string(), json!("active"));

	let mutation = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};

	// Request WITHOUT CSRF cookie
	let request = make_e2e_request_no_csrf(
		"/admin/api/server_fn/create_record",
		json!({
			"model_name": "TestModel",
			"request": mutation
		}),
	);

	// Act
	let response = router.handle(request).await;

	// Assert - should fail with 403 (CSRF validation failure)
	let response = response.expect("Router should handle request");
	assert_eq!(
		response.status,
		StatusCode::FORBIDDEN,
		"Missing CSRF cookie should return 403. Body: {}",
		String::from_utf8_lossy(&response.body)
	);
}

/// Verify mutation fails when CSRF cookie value doesn't match body token.
#[rstest]
#[tokio::test]
#[ignore = "Requires AdminDefaultUser ORM-compatible auth_user table schema (#3049)"]
async fn test_e2e_mutation_csrf_mismatch(
	#[future] e2e_router_context: (ServerRouter, Arc<AdminDatabase>),
) {
	// Arrange
	let (router, _db) = e2e_router_context.await;
	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("CSRF Mismatch"));
	data.insert("status".to_string(), json!("active"));

	let mutation = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};

	// Request with WRONG CSRF cookie value
	let request = make_e2e_request_wrong_csrf(
		"/admin/api/server_fn/create_record",
		json!({
			"model_name": "TestModel",
			"request": mutation
		}),
	);

	// Act
	let response = router.handle(request).await;

	// Assert - should fail with 403 (CSRF validation failure)
	let response = response.expect("Router should handle request");
	assert_eq!(
		response.status,
		StatusCode::FORBIDDEN,
		"Mismatched CSRF should return 403. Body: {}",
		String::from_utf8_lossy(&response.body)
	);
}

// ==================== Category 3: Auth Pipeline Tests (#3049) ====================

/// Verify request without AuthState fails at DI resolution level.
/// AuthUser::inject() requires AuthState in request extensions.
/// This test does NOT need a real auth_user table because the failure
/// occurs before the DB lookup (at the AuthState extraction step).
#[rstest]
#[tokio::test]
async fn test_e2e_unauthenticated_request(
	#[future] e2e_router_context: (ServerRouter, Arc<AdminDatabase>),
) {
	// Arrange
	let (router, _db) = e2e_router_context.await;

	// Request WITHOUT AuthState in extensions
	let request = make_e2e_request_no_auth(
		"/admin/api/server_fn/get_list",
		json!({
			"model_name": "TestModel",
			"params": {}
		}),
	);

	// Act
	let response = router.handle(request).await;

	// Assert - should fail (auth error, not 200)
	let response = response.expect("Router should handle request");
	assert_ne!(
		response.status,
		StatusCode::OK,
		"Unauthenticated request should not succeed"
	);
}

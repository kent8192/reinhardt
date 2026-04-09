//! Integration tests for permission denial behavior of admin server functions
//!
//! Verifies that server functions correctly reject unauthorized operations
//! based on ModelAdmin permission settings (deny-all and view-only).

use super::server_fn_helpers::{
	TEST_CSRF_TOKEN, make_auth_user, make_staff_request, server_fn_context_deny_all,
	server_fn_context_view_only,
};
use reinhardt_admin::adapters::{BulkDeleteRequest, ListQueryParams, MutationRequest};
use reinhardt_admin::core::{AdminDatabase, AdminSite, ExportFormat, ImportFormat};
use reinhardt_admin::server::{
	bulk_delete_records, create_record, delete_record, export_data, get_detail, get_fields,
	get_list, import_data, update_record,
};
use reinhardt_di::Depends;
use rstest::*;
use std::collections::HashMap;

// ==================== Deny All Permission Tests ====================

/// Verify get_list returns permission error when view permission is denied
#[rstest]
#[tokio::test]
async fn test_get_list_denied_when_view_false(
	#[future] server_fn_context_deny_all: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context_deny_all.await;
	let auth_user = make_auth_user();
	let params = ListQueryParams::default();

	// Act
	let result = get_list("TestModel".to_string(), params, site, db, auth_user).await;

	// Assert
	assert!(
		result.is_err(),
		"get_list should fail when view permission is denied"
	);
	let err = result.unwrap_err();
	let err_msg = format!("{}", err).to_lowercase();
	assert!(
		err_msg.contains("permission"),
		"Error should mention permission denial, got: {}",
		err
	);
}

/// Verify get_detail returns permission error when view permission is denied
#[rstest]
#[tokio::test]
async fn test_get_detail_denied_when_view_false(
	#[future] server_fn_context_deny_all: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context_deny_all.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Act
	let result = get_detail(
		"TestModel".to_string(),
		"1".to_string(),
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(
		result.is_err(),
		"get_detail should fail when view permission is denied"
	);
	let err = result.unwrap_err();
	let err_msg = format!("{}", err).to_lowercase();
	assert!(
		err_msg.contains("permission"),
		"Error should mention permission denial, got: {}",
		err
	);
}

/// Verify get_fields returns permission error when view permission is denied
#[rstest]
#[tokio::test]
async fn test_get_fields_denied_when_view_false(
	#[future] server_fn_context_deny_all: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context_deny_all.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Act
	let result = get_fields(
		"TestModel".to_string(),
		None,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(
		result.is_err(),
		"get_fields should fail when view permission is denied"
	);
	let err = result.unwrap_err();
	let err_msg = format!("{}", err).to_lowercase();
	assert!(
		err_msg.contains("permission"),
		"Error should mention permission denial, got: {}",
		err
	);
}

/// Verify create_record returns permission error when add permission is denied
#[rstest]
#[tokio::test]
async fn test_create_record_denied_when_add_false(
	#[future] server_fn_context_deny_all: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context_deny_all.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data: HashMap::new(),
	};

	// Act
	let result = create_record(
		"TestModel".to_string(),
		request,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(
		result.is_err(),
		"create_record should fail when add permission is denied"
	);
	let err = result.unwrap_err();
	let err_msg = format!("{}", err).to_lowercase();
	assert!(
		err_msg.contains("permission"),
		"Error should mention permission denial, got: {}",
		err
	);
}

/// Verify update_record returns permission error when change permission is denied
#[rstest]
#[tokio::test]
async fn test_update_record_denied_when_change_false(
	#[future] server_fn_context_deny_all: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context_deny_all.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data: HashMap::new(),
	};

	// Act
	let result = update_record(
		"TestModel".to_string(),
		"1".to_string(),
		request,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(
		result.is_err(),
		"update_record should fail when change permission is denied"
	);
	let err = result.unwrap_err();
	let err_msg = format!("{}", err).to_lowercase();
	assert!(
		err_msg.contains("permission"),
		"Error should mention permission denial, got: {}",
		err
	);
}

/// Verify delete_record returns permission error when delete permission is denied
#[rstest]
#[tokio::test]
async fn test_delete_record_denied_when_delete_false(
	#[future] server_fn_context_deny_all: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context_deny_all.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Act
	let result = delete_record(
		"TestModel".to_string(),
		"1".to_string(),
		TEST_CSRF_TOKEN.to_string(),
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(
		result.is_err(),
		"delete_record should fail when delete permission is denied"
	);
	let err = result.unwrap_err();
	let err_msg = format!("{}", err).to_lowercase();
	assert!(
		err_msg.contains("permission"),
		"Error should mention permission denial, got: {}",
		err
	);
}

/// Verify bulk_delete_records returns permission error when delete permission is denied
#[rstest]
#[tokio::test]
async fn test_bulk_delete_denied_when_delete_false(
	#[future] server_fn_context_deny_all: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context_deny_all.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let request = BulkDeleteRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		ids: vec!["1".to_string()],
	};

	// Act
	let result = bulk_delete_records(
		"TestModel".to_string(),
		request,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(
		result.is_err(),
		"bulk_delete_records should fail when delete permission is denied"
	);
	let err = result.unwrap_err();
	let err_msg = format!("{}", err).to_lowercase();
	assert!(
		err_msg.contains("permission"),
		"Error should mention permission denial, got: {}",
		err
	);
}

/// Verify export_data returns permission error when view permission is denied
#[rstest]
#[tokio::test]
async fn test_export_denied_when_view_false(
	#[future] server_fn_context_deny_all: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context_deny_all.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Act
	let result = export_data(
		"TestModel".to_string(),
		ExportFormat::JSON,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(
		result.is_err(),
		"export_data should fail when view permission is denied"
	);
	let err = result.unwrap_err();
	let err_msg = format!("{}", err).to_lowercase();
	assert!(
		err_msg.contains("permission"),
		"Error should mention permission denial, got: {}",
		err
	);
}

/// Verify import_data returns permission error when add permission is denied
#[rstest]
#[tokio::test]
async fn test_import_denied_when_add_false(
	#[future] server_fn_context_deny_all: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context_deny_all.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let json_data = serde_json::to_vec(&serde_json::json!([
		{"name": "Import Item", "status": "active"}
	]))
	.expect("JSON serialization should succeed");

	// Act
	let result = import_data(
		"TestModel".to_string(),
		ImportFormat::JSON,
		json_data,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(
		result.is_err(),
		"import_data should fail when add permission is denied"
	);
	let err = result.unwrap_err();
	let err_msg = format!("{}", err).to_lowercase();
	assert!(
		err_msg.contains("permission"),
		"Error should mention permission denial, got: {}",
		err
	);
}

// ==================== View-Only Permission Tests ====================

/// Verify view-only user can list records but cannot create new ones
#[rstest]
#[tokio::test]
async fn test_view_only_can_list_but_not_create(
	#[future] server_fn_context_view_only: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context_view_only.await;
	let params = ListQueryParams::default();

	// Act — view operation should succeed
	let list_result = get_list(
		"TestModel".to_string(),
		params,
		site.clone(),
		db.clone(),
		make_auth_user(),
	)
	.await;

	// Assert — list succeeds
	assert!(
		list_result.is_ok(),
		"get_list should succeed with view permission: {:?}",
		list_result
	);
	let response = list_result.unwrap();
	assert!(
		response.count >= 1,
		"Should have at least the seeded record"
	);

	// Act — mutation operation should fail
	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data: HashMap::new(),
	};
	let create_result = create_record(
		"TestModel".to_string(),
		request,
		site,
		db,
		make_staff_request(),
		make_auth_user(),
	)
	.await;

	// Assert — create fails with permission error
	assert!(
		create_result.is_err(),
		"create_record should fail with view-only permission"
	);
	let err = create_result.unwrap_err();
	let err_msg = format!("{}", err).to_lowercase();
	assert!(
		err_msg.contains("permission"),
		"Error should mention permission denial, got: {}",
		err
	);
}

/// Verify view-only user can view detail but cannot update records
#[rstest]
#[tokio::test]
async fn test_view_only_can_detail_but_not_update(
	#[future] server_fn_context_view_only: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context_view_only.await;

	// Act — view operation should succeed (seeded record has id=1)
	let detail_result = get_detail(
		"TestModel".to_string(),
		"1".to_string(),
		site.clone(),
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await;

	// Assert — detail succeeds
	assert!(
		detail_result.is_ok(),
		"get_detail should succeed with view permission: {:?}",
		detail_result
	);

	// Act — mutation operation should fail
	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data: HashMap::new(),
	};
	let update_result = update_record(
		"TestModel".to_string(),
		"1".to_string(),
		request,
		site,
		db,
		make_staff_request(),
		make_auth_user(),
	)
	.await;

	// Assert — update fails with permission error
	assert!(
		update_result.is_err(),
		"update_record should fail with view-only permission"
	);
	let err = update_result.unwrap_err();
	let err_msg = format!("{}", err).to_lowercase();
	assert!(
		err_msg.contains("permission"),
		"Error should mention permission denial, got: {}",
		err
	);
}

/// Verify view-only user can export data but cannot import
#[rstest]
#[tokio::test]
async fn test_view_only_can_export_but_not_import(
	#[future] server_fn_context_view_only: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context_view_only.await;

	// Act — view operation (export) should succeed
	let export_result = export_data(
		"TestModel".to_string(),
		ExportFormat::JSON,
		site.clone(),
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await;

	// Assert — export succeeds
	assert!(
		export_result.is_ok(),
		"export_data should succeed with view permission: {:?}",
		export_result
	);
	let response = export_result.unwrap();
	assert!(
		!response.data.is_empty(),
		"Exported data should not be empty"
	);

	// Act — mutation operation (import) should fail
	let json_data = serde_json::to_vec(&serde_json::json!([
		{"name": "Import Attempt", "status": "active"}
	]))
	.expect("JSON serialization should succeed");

	let import_result = import_data(
		"TestModel".to_string(),
		ImportFormat::JSON,
		json_data,
		site,
		db,
		make_staff_request(),
		make_auth_user(),
	)
	.await;

	// Assert — import fails with permission error
	assert!(
		import_result.is_err(),
		"import_data should fail with view-only permission"
	);
	let err = import_result.unwrap_err();
	let err_msg = format!("{}", err).to_lowercase();
	assert!(
		err_msg.contains("permission"),
		"Error should mention permission denial, got: {}",
		err
	);
}

// ==================== Decision Table: Parametric Test ====================

/// Permission denial decision table for all mutation endpoints.
///
/// Verifies that every mutation server function rejects operations
/// when the corresponding permission is denied (deny-all context).
#[rstest]
#[case::create("create")]
#[case::update("update")]
#[case::delete("delete")]
#[case::bulk_delete("bulk_delete")]
#[case::import("import")]
#[tokio::test]
async fn test_permission_denial_for_all_mutation_endpoints(
	#[future] server_fn_context_deny_all: (Depends<AdminSite>, Depends<AdminDatabase>),
	#[case] endpoint: &str,
) {
	// Arrange
	let (site, db) = server_fn_context_deny_all.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Act
	let result: Result<(), String> = match endpoint {
		"create" => {
			let request = MutationRequest {
				csrf_token: TEST_CSRF_TOKEN.to_string(),
				data: HashMap::new(),
			};
			create_record(
				"TestModel".to_string(),
				request,
				site,
				db,
				http_request,
				auth_user,
			)
			.await
			.map(|_| ())
			.map_err(|e| format!("{}", e))
		}
		"update" => {
			let request = MutationRequest {
				csrf_token: TEST_CSRF_TOKEN.to_string(),
				data: HashMap::new(),
			};
			update_record(
				"TestModel".to_string(),
				"1".to_string(),
				request,
				site,
				db,
				http_request,
				auth_user,
			)
			.await
			.map(|_| ())
			.map_err(|e| format!("{}", e))
		}
		"delete" => delete_record(
			"TestModel".to_string(),
			"1".to_string(),
			TEST_CSRF_TOKEN.to_string(),
			site,
			db,
			http_request,
			auth_user,
		)
		.await
		.map(|_| ())
		.map_err(|e| format!("{}", e)),
		"bulk_delete" => {
			let request = BulkDeleteRequest {
				csrf_token: TEST_CSRF_TOKEN.to_string(),
				ids: vec!["1".to_string()],
			};
			bulk_delete_records(
				"TestModel".to_string(),
				request,
				site,
				db,
				http_request,
				auth_user,
			)
			.await
			.map(|_| ())
			.map_err(|e| format!("{}", e))
		}
		"import" => {
			let json_data = serde_json::to_vec(&serde_json::json!([
				{"name": "Import Item", "status": "active"}
			]))
			.expect("JSON serialization should succeed");
			import_data(
				"TestModel".to_string(),
				ImportFormat::JSON,
				json_data,
				site,
				db,
				http_request,
				auth_user,
			)
			.await
			.map(|_| ())
			.map_err(|e| format!("{}", e))
		}
		_ => unreachable!("Unknown endpoint: {}", endpoint),
	};

	// Assert
	assert!(
		result.is_err(),
		"{} should fail when permission is denied",
		endpoint
	);
	let err_msg = result.unwrap_err().to_lowercase();
	assert!(
		err_msg.contains("permission"),
		"{} error should mention permission denial, got: {}",
		endpoint,
		err_msg
	);
}

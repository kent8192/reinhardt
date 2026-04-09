//! Integration tests for server functions with UUID primary keys
//!
//! Tests that exercise server functions with UUID PKs to catch "id" hardcoding bugs.
//! Covers regression for Issue #2946 (PK field handling with non-integer types).

use super::server_fn_helpers::uuid_pk_context;
use reinhardt_admin::adapters::{BulkDeleteRequest, ListQueryParams, MutationRequest};
use reinhardt_admin::core::{AdminDatabase, AdminSite, ExportFormat};
use reinhardt_admin::server::{
	bulk_delete_records, create_record, delete_record, export_data, get_detail, get_list,
	update_record,
};
use reinhardt_di::Depends;
use rstest::*;
use serde_json::json;
use std::collections::HashMap;

use reinhardt_query::prelude::{Alias, PostgresQueryBuilder, Query, QueryStatementBuilder, Value};

use super::server_fn_helpers::{TEST_CSRF_TOKEN, make_auth_user, make_staff_request};

// ==================== Helper ====================

/// Inserts a UUID PK record via SeaQuery and returns its UUID as a string.
async fn insert_uuid_record(pool: &sqlx::PgPool, name: &str, status: &str) -> String {
	use sqlx::{Executor, Row};

	let sql = Query::insert()
		.into_table(Alias::new("uuid_test_models"))
		.columns([Alias::new("name"), Alias::new("status")])
		.values_panic([Value::from(name), Value::from(status)])
		.returning([Alias::new("id")])
		.to_string(PostgresQueryBuilder);

	let row = pool
		.fetch_one(sqlx::query(&sql))
		.await
		.expect("insert failed");
	let id: uuid::Uuid = row.get("id");
	id.to_string()
}

// ==================== List ====================

/// Verify get_list works with UUID PK model
#[rstest]
#[tokio::test]
async fn test_list_uuid_pk_model(
	#[future] uuid_pk_context: (Depends<AdminSite>, Depends<AdminDatabase>, sqlx::PgPool),
) {
	// Arrange
	let (site, db, pool) = uuid_pk_context.await;
	let auth_user = make_auth_user();

	insert_uuid_record(&pool, "UUID List Item 1", "active").await;
	insert_uuid_record(&pool, "UUID List Item 2", "draft").await;

	let params = ListQueryParams::default();

	// Act
	let result = get_list("UuidModel".to_string(), params, site, db, auth_user).await;

	// Assert
	assert!(result.is_ok(), "get_list should succeed: {:?}", result);
	let response = result.unwrap();
	assert_eq!(response.model_name, "UuidModel");
	assert!(
		response.count >= 2,
		"Should have at least 2 records, got {}",
		response.count
	);
	assert!(response.page_size > 0);

	// Verify that results contain UUID-formatted IDs
	for record in &response.results {
		let id_value = record.get("id").expect("Record should have an 'id' field");
		let id_str = id_value.as_str().expect("UUID id should be a string");
		assert!(
			uuid::Uuid::parse_str(id_str).is_ok(),
			"ID should be a valid UUID: {}",
			id_str
		);
	}
}

// ==================== Detail ====================

/// Verify get_detail returns correct record for UUID PK
#[rstest]
#[tokio::test]
async fn test_detail_uuid_pk_model(
	#[future] uuid_pk_context: (Depends<AdminSite>, Depends<AdminDatabase>, sqlx::PgPool),
) {
	// Arrange
	let (site, db, pool) = uuid_pk_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let uuid_id = insert_uuid_record(&pool, "UUID Detail Test", "active").await;

	// Act
	let result = get_detail(
		"UuidModel".to_string(),
		uuid_id.clone(),
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(result.is_ok(), "get_detail should succeed: {:?}", result);
	let response = result.unwrap();
	assert_eq!(response.model_name, "UuidModel");
	assert_eq!(response.data.get("name"), Some(&json!("UUID Detail Test")));
	assert_eq!(response.data.get("status"), Some(&json!("active")));
}

// ==================== Create ====================

/// Verify create_record returns success with a UUID id
#[rstest]
#[tokio::test]
async fn test_create_uuid_pk_model(
	#[future] uuid_pk_context: (Depends<AdminSite>, Depends<AdminDatabase>, sqlx::PgPool),
) {
	// Arrange
	let (site, db, _pool) = uuid_pk_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("UUID Created Item"));
	data.insert("status".to_string(), json!("active"));

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};

	// Act
	let result = create_record(
		"UuidModel".to_string(),
		request,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(
		result.is_ok(),
		"create_record should succeed for UUID PK model: {:?}",
		result
	);
	let response = result.unwrap();
	assert!(response.success);
	assert!(
		response.affected.is_some(),
		"Should return affected count for UUID PK create"
	);
}

// ==================== Update ====================

/// Verify update_record works with UUID PK
#[rstest]
#[tokio::test]
async fn test_update_uuid_pk_model(
	#[future] uuid_pk_context: (Depends<AdminSite>, Depends<AdminDatabase>, sqlx::PgPool),
) {
	// Arrange
	let (site, db, pool) = uuid_pk_context.await;
	let uuid_id = insert_uuid_record(&pool, "UUID Before Update", "active").await;

	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("UUID After Update"));

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};

	// Act
	let result = update_record(
		"UuidModel".to_string(),
		uuid_id.clone(),
		request,
		site.clone(),
		db.clone(),
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(
		result.is_ok(),
		"update_record should succeed for UUID PK: {:?}",
		result
	);
	let response = result.unwrap();
	assert!(response.success);

	// Verify the update persisted
	let detail_request = make_staff_request();
	let detail_user = make_auth_user();
	let detail = get_detail(
		"UuidModel".to_string(),
		uuid_id,
		site,
		db,
		detail_request,
		detail_user,
	)
	.await
	.expect("get_detail after update should succeed");
	assert_eq!(detail.data.get("name"), Some(&json!("UUID After Update")));
}

// ==================== Delete ====================

/// Verify delete_record works with UUID PK
#[rstest]
#[tokio::test]
async fn test_delete_uuid_pk_model(
	#[future] uuid_pk_context: (Depends<AdminSite>, Depends<AdminDatabase>, sqlx::PgPool),
) {
	// Arrange
	let (site, db, pool) = uuid_pk_context.await;
	let uuid_id = insert_uuid_record(&pool, "UUID To Delete", "active").await;

	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Act
	let result = delete_record(
		"UuidModel".to_string(),
		uuid_id,
		TEST_CSRF_TOKEN.to_string(),
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(
		result.is_ok(),
		"delete_record should succeed for UUID PK: {:?}",
		result
	);
	let response = result.unwrap();
	assert!(response.success);
	assert_eq!(response.affected, Some(1));
}

// ==================== Bulk Delete ====================

/// Verify bulk_delete_records works with UUID PKs
#[rstest]
#[tokio::test]
async fn test_bulk_delete_uuid_pk_model(
	#[future] uuid_pk_context: (Depends<AdminSite>, Depends<AdminDatabase>, sqlx::PgPool),
) {
	// Arrange
	let (site, db, pool) = uuid_pk_context.await;

	let id1 = insert_uuid_record(&pool, "UUID Bulk 1", "active").await;
	let id2 = insert_uuid_record(&pool, "UUID Bulk 2", "draft").await;
	let id3 = insert_uuid_record(&pool, "UUID Bulk 3", "active").await;

	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let request = BulkDeleteRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		ids: vec![id1, id2, id3],
	};

	// Act
	let result = bulk_delete_records(
		"UuidModel".to_string(),
		request,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(
		result.is_ok(),
		"bulk_delete should succeed for UUID PKs: {:?}",
		result
	);
	let response = result.unwrap();
	assert_eq!(response.deleted, 3, "Should delete all 3 UUID records");
	assert!(response.success);
}

// ==================== Export ====================

/// Verify export_data returns data with UUID ids
#[rstest]
#[tokio::test]
async fn test_export_uuid_pk_model(
	#[future] uuid_pk_context: (Depends<AdminSite>, Depends<AdminDatabase>, sqlx::PgPool),
) {
	// Arrange
	let (site, db, pool) = uuid_pk_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let uuid_id = insert_uuid_record(&pool, "UUID Export Item", "active").await;

	// Act
	let result = export_data(
		"UuidModel".to_string(),
		ExportFormat::JSON,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(
		result.is_ok(),
		"export_data should succeed for UUID PK: {:?}",
		result
	);
	let response = result.unwrap();
	assert!(!response.data.is_empty(), "Export data should not be empty");

	// Parse the exported JSON and verify UUID IDs are present
	let exported: Vec<HashMap<String, serde_json::Value>> =
		serde_json::from_slice(&response.data).expect("Export data should be valid JSON");
	assert!(!exported.is_empty(), "Exported records should not be empty");

	// Verify at least one record has the UUID we inserted
	let has_our_record = exported.iter().any(|record| {
		record
			.get("id")
			.and_then(|v| v.as_str())
			.is_some_and(|id| id == uuid_id)
	});
	assert!(
		has_our_record,
		"Exported data should contain the inserted UUID record"
	);
}

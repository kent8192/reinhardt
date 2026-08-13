//! Integration tests for import_data server function
//!
//! Tests the import server function with various formats and boundary conditions.

use super::server_fn_helpers::{ServerFnContext, server_fn_context};
use reinhardt_admin::core::ImportFormat;
use reinhardt_admin::server::{get_history, import_data};
use reinhardt_admin::types::HistoryResponse;
use reinhardt_db::orm::{OrmExecutor, QueryValue};
use rstest::*;

use super::server_fn_helpers::{make_auth_user, make_staff_request};

async fn query_history(context: &ServerFnContext, object_id: &str) -> HistoryResponse {
	let (site, db, _) = context;
	get_history(
		"testmodel".to_string(),
		object_id.to_string(),
		1,
		site.clone(),
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("authorized history query must succeed")
}

async fn object_ids_by_name(
	db: &super::server_fn_helpers::AdminDatabaseDepends,
	name: &str,
) -> Vec<String> {
	let mut connection = *db.connection();
	OrmExecutor::fetch_all(
		&mut connection,
		"SELECT id FROM test_models WHERE name = $1 ORDER BY id",
		vec![QueryValue::String(name.to_string())],
	)
	.await
	.expect("imported rows must be queryable")
	.into_iter()
	.map(|row| {
		let id: i64 = row.get("id").expect("imported row must have an id");
		id.to_string()
	})
	.collect()
}

async fn history_row_count(db: &super::server_fn_helpers::AdminDatabaseDepends) -> i64 {
	let mut connection = *db.connection();
	let row = OrmExecutor::fetch_one(
		&mut connection,
		"SELECT COUNT(*) AS count FROM reinhardt_admin_history",
		Vec::new(),
	)
	.await
	.expect("history count must be queryable");
	row.get("count").expect("history count must be returned")
}

// ==================== Happy path tests ====================

/// Verify JSON import succeeds with valid data
#[rstest]
#[tokio::test]
async fn test_import_json_happy_path(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let context = server_fn_context.await;
	let (site, db, _connection_lease) = &context;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let json_data = serde_json::to_vec(&serde_json::json!([
		{"name": "Import Item 1", "status": "active"},
		{"name": "Import Item 2", "status": "draft"}
	]))
	.expect("JSON serialization should succeed");

	// Act
	let result = import_data(
		"TestModel".to_string(),
		ImportFormat::JSON,
		json_data,
		site.clone(),
		db.clone(),
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(result.is_ok(), "JSON import should succeed: {:?}", result);
	let response = result.unwrap();
	assert_eq!(response.imported, 2, "Should import 2 records");
	assert_eq!(response.failed, 0, "No records should fail");
	assert!(response.success);
	for name in ["Import Item 1", "Import Item 2"] {
		let object_ids = object_ids_by_name(db, name).await;
		assert_eq!(object_ids.len(), 1);
		let history = query_history(&context, &object_ids[0]).await;
		assert_eq!(history.count, 1);
		assert_eq!(history.results.len(), 1);
		let event = &history.results[0];
		assert_eq!(event.action_name, "IMPORT");
		assert_eq!(event.actor, "test_staff");
		assert_eq!(event.model_name, "TestModel");
		assert_eq!(event.object_id, object_ids[0]);
		assert_eq!(event.changed_fields, vec!["name", "status"]);
		assert_eq!(event.affected_count, 1);
		assert!(event.success);
	}
}

#[rstest]
#[tokio::test]
async fn test_import_history_failure_rolls_back_record(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let context = server_fn_context.await;
	let (site, db, _connection_lease) = &context;
	assert_eq!(query_history(&context, "0").await.count, 0);
	let mut connection = *db.connection();
	OrmExecutor::execute(
		&mut connection,
		"ALTER TABLE reinhardt_admin_history \
		 ADD CONSTRAINT import_history_test_reject_insert CHECK (FALSE) NOT VALID",
		Vec::new(),
	)
	.await
	.expect("history fault constraint must install");
	let json_data = serde_json::to_vec(&serde_json::json!([
		{"name": "Import Must Roll Back", "status": "active"}
	]))
	.expect("JSON serialization should succeed");

	// Act
	let response = import_data(
		"TestModel".to_string(),
		ImportFormat::JSON,
		json_data,
		site.clone(),
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("record-local import failure must return statistics");

	// Assert
	assert_eq!(response.imported, 0);
	assert_eq!(response.failed, 1);
	assert!(!response.success);
	assert_eq!(
		object_ids_by_name(db, "Import Must Roll Back").await.len(),
		0
	);
	assert_eq!(history_row_count(db).await, 0);
}

/// Verify CSV import succeeds with valid data
#[rstest]
#[tokio::test]
async fn test_import_csv_happy_path(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let csv_data = b"name,status\nCSV Item 1,active\nCSV Item 2,draft".to_vec();

	// Act
	let result = import_data(
		"TestModel".to_string(),
		ImportFormat::CSV,
		csv_data,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(result.is_ok(), "CSV import should succeed: {:?}", result);
	let response = result.unwrap();
	assert_eq!(response.imported, 2, "Should import 2 CSV records");
}

/// Verify TSV import succeeds with valid data
#[rstest]
#[tokio::test]
async fn test_import_tsv_happy_path(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let tsv_data = b"name\tstatus\nTSV Item 1\tactive\nTSV Item 2\tdraft".to_vec();

	// Act
	let result = import_data(
		"TestModel".to_string(),
		ImportFormat::TSV,
		tsv_data,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(result.is_ok(), "TSV import should succeed: {:?}", result);
	let response = result.unwrap();
	assert_eq!(response.imported, 2, "Should import 2 TSV records");
}

// ==================== Boundary tests ====================

/// Verify that import rejects file exceeding MAX_IMPORT_FILE_SIZE (10MB)
#[rstest]
#[tokio::test]
async fn test_import_file_size_limit(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// MAX_IMPORT_FILE_SIZE = 10 * 1024 * 1024 = 10_485_760 bytes
	let oversized_data = vec![b'x'; 10 * 1024 * 1024 + 1];

	// Act
	let result = import_data(
		"TestModel".to_string(),
		ImportFormat::JSON,
		oversized_data,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(
		result.is_err(),
		"Should reject import exceeding MAX_IMPORT_FILE_SIZE"
	);
	let err = format!("{}", result.unwrap_err());
	assert!(
		err.contains("size") || err.contains("exceeds") || err.contains("maximum"),
		"Error should mention file size limit: {}",
		err
	);
}

/// Verify that import rejects data with more than MAX_IMPORT_RECORDS (1000)
#[rstest]
#[tokio::test]
async fn test_import_record_count_limit(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Create JSON with 1001 records
	let records: Vec<serde_json::Value> = (0..1001)
		.map(|i| {
			serde_json::json!({
				"name": format!("Record {}", i),
				"status": "active"
			})
		})
		.collect();
	let json_data = serde_json::to_vec(&records).expect("JSON serialization");

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
		"Should reject import exceeding MAX_IMPORT_RECORDS"
	);
	let err = format!("{}", result.unwrap_err());
	assert!(
		err.contains("count") || err.contains("exceeds") || err.contains("1000"),
		"Error should mention record count limit: {}",
		err
	);
}

// ==================== Error path tests ====================

/// Verify that invalid JSON returns deserialization error
#[rstest]
#[tokio::test]
async fn test_import_invalid_json(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let invalid_json = b"{ this is not valid json }".to_vec();

	// Act
	let result = import_data(
		"TestModel".to_string(),
		ImportFormat::JSON,
		invalid_json,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(result.is_err(), "Should return error for invalid JSON");
}

/// Verify that invalid CSV returns error
#[rstest]
#[tokio::test]
async fn test_import_invalid_csv(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// CSV with mismatched field count (header has 2 fields, row has 3)
	let bad_csv = b"name,status\n\"unclosed quote,value1,value2,value3".to_vec();

	// Act
	let result = import_data(
		"TestModel".to_string(),
		ImportFormat::CSV,
		bad_csv,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert - either error or 0 imports due to parse failure
	if let Ok(response) = &result {
		// If the CSV parser handles it gracefully, verify no records imported
		assert!(
			response.imported == 0 || response.failed > 0,
			"Bad CSV should result in 0 imports or failures"
		);
	}
	// Error is also acceptable
}

// ==================== Edge case tests ====================

/// Verify import of empty data
#[rstest]
#[tokio::test]
async fn test_import_empty_data(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let empty_json = serde_json::to_vec(&serde_json::json!([])).expect("JSON serialization");

	// Act
	let result = import_data(
		"TestModel".to_string(),
		ImportFormat::JSON,
		empty_json,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(result.is_ok(), "Empty import should succeed: {:?}", result);
	let response = result.unwrap();
	assert_eq!(response.imported, 0, "Should import 0 records");
	assert!(response.success, "Empty import should be success");
}

/// Verify import returns error for non-registered model
#[rstest]
#[tokio::test]
async fn test_import_model_not_registered(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let json_data = serde_json::to_vec(&serde_json::json!([
		{"name": "Test", "status": "active"}
	]))
	.expect("JSON serialization");

	// Act
	let result = import_data(
		"NonExistentModel".to_string(),
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
		"Should return error for unregistered model"
	);
}

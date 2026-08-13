//! Integration tests for update_record server function
//!
//! Tests the update operation server function.
//! Covers Issue #3047 (missing update_record test coverage).

use super::server_fn_create_tests::setup_many_to_many_context;
use super::server_fn_helpers::server_fn_context;
use reinhardt_admin::adapters::MutationRequest;
use reinhardt_admin::core::AdminRecord;
use reinhardt_admin::server::{create_record, update_record};
use reinhardt_test::fixtures::shared_postgres::shared_db_pool;
use rstest::*;
use serde_json::json;
use serial_test::serial;
use sqlx::Executor;
use std::collections::HashMap;

use super::server_fn_helpers::{TEST_CSRF_TOKEN, make_auth_user, make_staff_request};

async fn seed_many_to_many_article(pool: &sqlx::PgPool) {
	pool.execute("INSERT INTO admin_persistence_articles (id, title) VALUES (1, 'Before update')")
		.await
		.unwrap();
	pool.execute(
		"INSERT INTO admin_persistence_articles_tags \
		 (admin_persistence_articles_id, admin_persistence_tags_id) VALUES (1, 1), (1, 2)",
	)
	.await
	.unwrap();
}

async fn many_to_many_article_state(pool: &sqlx::PgPool, id: i32) -> (String, Vec<(i32, i64)>) {
	let title = sqlx::query_scalar("SELECT title FROM admin_persistence_articles WHERE id = $1")
		.bind(id)
		.fetch_one(pool)
		.await
		.unwrap();
	let joins = sqlx::query_as(
		"SELECT admin_persistence_tags_id, marker \
		 FROM admin_persistence_articles_tags \
		 WHERE admin_persistence_articles_id = $1 ORDER BY admin_persistence_tags_id",
	)
	.bind(id)
	.fetch_all(pool)
	.await
	.unwrap();
	(title, joins)
}

// ==================== Helper ====================

/// Creates a test record and returns its ID as a string.
async fn create_test_record(
	site: &super::server_fn_helpers::AdminSiteDepends,
	db: &super::server_fn_helpers::AdminDatabaseDepends,
	name: &str,
	status: &str,
) -> String {
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!(name));
	data.insert("status".to_string(), json!(status));

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};

	let result = create_record(
		"TestModel".to_string(),
		request,
		site.clone(),
		db.clone(),
		http_request,
		auth_user,
	)
	.await
	.expect("Failed to create test record");

	result
		.affected
		.expect("Create should return affected count")
		.to_string()
}

// ==================== Happy path tests ====================

/// Verify that update_record succeeds with valid data
#[rstest]
#[tokio::test]
async fn test_update_record_happy_path(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let id = create_test_record(&site, &db, "Original Name", "active").await;

	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("Updated Name"));

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};

	// Act
	let result = update_record(
		"TestModel".to_string(),
		id,
		request,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(result.is_ok(), "update_record should succeed: {:?}", result);
	let response = result.unwrap();
	assert!(response.success);
	assert_eq!(response.affected, Some(1));
}

/// Verify update_record returns valid response metadata
#[rstest]
#[tokio::test]
async fn test_update_record_returns_valid_response(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let id = create_test_record(&site, &db, "Metadata Test", "active").await;

	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("Metadata Updated"));

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};

	// Act
	let result = update_record(
		"TestModel".to_string(),
		id,
		request,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	let response = result.expect("update_record should succeed");
	assert!(response.success);
	assert!(
		response.message.contains("TestModel"),
		"Message should contain model name: {}",
		response.message
	);
	assert!(
		response.message.contains("updated"),
		"Message should indicate update: {}",
		response.message
	);
}

/// Verify updated record persists to database
#[rstest]
#[tokio::test]
async fn test_update_record_persists_to_database(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let id = create_test_record(&site, &db, "Before Update", "active").await;

	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("After Update"));
	data.insert("status".to_string(), json!("inactive"));

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};

	// Act
	update_record(
		"TestModel".to_string(),
		id.clone(),
		request,
		site,
		db.clone(),
		http_request,
		auth_user,
	)
	.await
	.expect("update_record should succeed");

	// Assert - verify changes persisted in DB
	let record = db
		.get::<AdminRecord>("test_models", "id", &id)
		.await
		.expect("Should read updated record");
	let record = record.expect("Record should exist");
	// Sanitized values have HTML entities escaped, so compare accordingly
	let name = record.get("name").expect("Should have name field");
	assert_eq!(name, &json!("After Update"));
}

/// Verify update_record works with multiple fields
#[rstest]
#[tokio::test]
async fn test_update_record_multiple_fields(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let id = create_test_record(&site, &db, "Multi Field", "active").await;

	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("Updated Multi"));
	data.insert("status".to_string(), json!("draft"));
	data.insert("description".to_string(), json!("New description"));

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};

	// Act
	let result = update_record(
		"TestModel".to_string(),
		id,
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
		"Should handle multiple fields: {:?}",
		result
	);
}

/// Verify update_record handles special characters and Unicode
#[rstest]
#[tokio::test]
async fn test_update_record_special_characters(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let id = create_test_record(&site, &db, "Original", "active").await;

	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert(
		"name".to_string(),
		json!("Special: <>&\"' \u{00e9}\u{00f1}\u{00fc} \u{65e5}\u{672c}\u{8a9e}"),
	);

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};

	// Act
	let result = update_record(
		"TestModel".to_string(),
		id,
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
		"Should handle special characters: {:?}",
		result
	);
}

/// Verify update_record with partial fields leaves other fields unchanged
#[rstest]
#[tokio::test]
async fn test_update_record_partial_fields(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let id = create_test_record(&site, &db, "Partial Original", "active").await;

	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Only update status, leave name unchanged
	let mut data = HashMap::new();
	data.insert("status".to_string(), json!("archived"));

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};

	// Act
	update_record(
		"TestModel".to_string(),
		id.clone(),
		request,
		site,
		db.clone(),
		http_request,
		auth_user,
	)
	.await
	.expect("Partial update should succeed");

	// Assert - name should be unchanged, status should be updated
	let record = db
		.get::<AdminRecord>("test_models", "id", &id)
		.await
		.expect("Should read record")
		.expect("Record should exist");
	let status = record.get("status").expect("Should have status field");
	assert_eq!(status, &json!("archived"));
}

// ==================== Error path tests ====================

/// Verify update_record returns error for non-existent ID
#[rstest]
#[tokio::test]
async fn test_update_record_not_found(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("Ghost Update"));

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};

	// Act
	let result = update_record(
		"TestModel".to_string(),
		"999999".to_string(),
		request,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(result.is_err(), "Should return error for non-existent ID");
}

/// Verify update_record returns error for non-registered model
#[rstest]
#[tokio::test]
async fn test_update_record_model_not_registered(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data: HashMap::new(),
	};

	// Act
	let result = update_record(
		"NonExistentModel".to_string(),
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
		"Should return error for unregistered model"
	);
}

#[rstest]
#[tokio::test]
#[serial(admin_m2m_persistence)]
async fn many_to_many_update_preserves_retained_join_and_changes_only_difference(
	#[future] shared_db_pool: (sqlx::PgPool, String),
) {
	// Arrange
	let (pool, _) = shared_db_pool.await;
	let context = setup_many_to_many_context(pool, true).await;
	seed_many_to_many_article(&context.pool).await;
	let retained_marker = many_to_many_article_state(&context.pool, 1).await.1[1].1;
	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data: HashMap::from([
			("title".to_string(), json!("After update")),
			("tags".to_string(), json!([2, " 2 ", 3, "3"])),
		]),
	};

	// Act
	let response = update_record(
		"PersistenceArticle".to_string(),
		"1".to_string(),
		request,
		context.site.clone(),
		context.db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.unwrap();

	// Assert
	assert_eq!(response.affected, Some(1));
	let state = many_to_many_article_state(&context.pool, 1).await;
	assert_eq!(state.0, "After update");
	assert_eq!(
		state.1.iter().map(|row| row.0).collect::<Vec<_>>(),
		vec![2, 3]
	);
	assert_eq!(state.1[0].1, retained_marker);
}

#[rstest]
#[tokio::test]
#[serial(admin_m2m_persistence)]
async fn many_to_many_update_missing_target_rolls_back_parent_and_joins(
	#[future] shared_db_pool: (sqlx::PgPool, String),
) {
	// Arrange
	let (pool, _) = shared_db_pool.await;
	let context = setup_many_to_many_context(pool, true).await;
	seed_many_to_many_article(&context.pool).await;
	let before = many_to_many_article_state(&context.pool, 1).await;
	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data: HashMap::from([
			("title".to_string(), json!("Must roll back")),
			("tags".to_string(), json!([2, 404])),
		]),
	};

	// Act
	let result = update_record(
		"PersistenceArticle".to_string(),
		"1".to_string(),
		request,
		context.site.clone(),
		context.db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await;

	// Assert
	assert!(result.is_err());
	assert_eq!(many_to_many_article_state(&context.pool, 1).await, before);
}

#[rstest]
#[tokio::test]
#[serial(admin_m2m_persistence)]
async fn many_to_many_update_denied_target_view_makes_no_sql_mutation(
	#[future] shared_db_pool: (sqlx::PgPool, String),
) {
	// Arrange
	let (pool, _) = shared_db_pool.await;
	let context = setup_many_to_many_context(pool, false).await;
	seed_many_to_many_article(&context.pool).await;
	let before = many_to_many_article_state(&context.pool, 1).await;
	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data: HashMap::from([
			("title".to_string(), json!("Denied")),
			("tags".to_string(), json!([2])),
		]),
	};

	// Act
	let result = update_record(
		"PersistenceArticle".to_string(),
		"1".to_string(),
		request,
		context.site.clone(),
		context.db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await;

	// Assert
	assert!(result.is_err());
	assert_eq!(many_to_many_article_state(&context.pool, 1).await, before);
}

#[rstest]
#[tokio::test]
#[serial(admin_m2m_persistence)]
async fn many_to_many_update_join_error_rolls_back_parent_and_removed_join(
	#[future] shared_db_pool: (sqlx::PgPool, String),
) {
	// Arrange
	let (pool, _) = shared_db_pool.await;
	let context = setup_many_to_many_context(pool, true).await;
	seed_many_to_many_article(&context.pool).await;
	let before = many_to_many_article_state(&context.pool, 1).await;
	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data: HashMap::from([
			("title".to_string(), json!("Must roll back")),
			("tags".to_string(), json!([2, 99])),
		]),
	};

	// Act
	let result = update_record(
		"PersistenceArticle".to_string(),
		"1".to_string(),
		request,
		context.site.clone(),
		context.db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await;

	// Assert
	assert!(result.is_err());
	assert_eq!(many_to_many_article_state(&context.pool, 1).await, before);
}

#[rstest]
#[tokio::test]
#[serial(admin_m2m_persistence)]
async fn many_to_many_update_zero_affected_rows_leaves_joins_unchanged(
	#[future] shared_db_pool: (sqlx::PgPool, String),
) {
	// Arrange
	let (pool, _) = shared_db_pool.await;
	let context = setup_many_to_many_context(pool, true).await;
	context
		.pool
		.execute(
			"INSERT INTO admin_persistence_articles_tags \
			 (admin_persistence_articles_id, admin_persistence_tags_id) VALUES (404, 1)",
		)
		.await
		.unwrap();
	let before: Vec<(i32, i64)> = sqlx::query_as(
		"SELECT admin_persistence_tags_id, marker FROM admin_persistence_articles_tags \
		 WHERE admin_persistence_articles_id = 404",
	)
	.fetch_all(&context.pool)
	.await
	.unwrap();
	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data: HashMap::from([
			("title".to_string(), json!("Missing parent")),
			("tags".to_string(), json!([2, 3])),
		]),
	};

	// Act
	let result = update_record(
		"PersistenceArticle".to_string(),
		"404".to_string(),
		request,
		context.site.clone(),
		context.db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await;

	// Assert
	assert!(result.is_err());
	let after: Vec<(i32, i64)> = sqlx::query_as(
		"SELECT admin_persistence_tags_id, marker FROM admin_persistence_articles_tags \
		 WHERE admin_persistence_articles_id = 404",
	)
	.fetch_all(&context.pool)
	.await
	.unwrap();
	assert_eq!(after, before);
}

//! Integration tests for atomic changelist inline edits.

use super::server_fn_helpers::{
	ServerFnContext, TEST_CSRF_TOKEN, make_auth_user, make_staff_request, server_fn_context,
	server_fn_context_deny_all,
};
use reinhardt_admin::core::AdminRecord;
use reinhardt_admin::server::{get_history, update_inline_edits};
use reinhardt_admin::types::{HistoryResponse, InlineEditMutation, InlineEditRequest};
use reinhardt_db::orm::OrmExecutor;
use rstest::rstest;
use serde_json::json;
use std::collections::HashMap;

async fn create_record(db: &super::server_fn_helpers::AdminDatabaseDepends, name: &str) -> String {
	db.create::<AdminRecord>(
		"test_models",
		Some("id"),
		HashMap::from([("name".to_string(), json!(name))]),
	)
	.await
	.expect("create inline-edit fixture")
	.to_string()
}

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

async fn assert_record_name(
	db: &super::server_fn_helpers::AdminDatabaseDepends,
	object_id: &str,
	expected: &str,
) {
	let record = db
		.get::<AdminRecord>("test_models", "id", object_id)
		.await
		.expect("read inline-edit fixture")
		.expect("inline-edit fixture exists");
	assert_eq!(record.get("name"), Some(&json!(expected)));
}

fn request(updates: Vec<InlineEditMutation>) -> InlineEditRequest {
	InlineEditRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		updates,
	}
}

fn mutation(
	object_id: impl Into<String>,
	field: &str,
	value: serde_json::Value,
) -> InlineEditMutation {
	InlineEditMutation {
		object_id: object_id.into(),
		changes: HashMap::from([(field.to_string(), value)]),
	}
}

#[rstest]
#[tokio::test]
async fn inline_edit_commits_rows_and_returns_sorted_outcomes(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let context = server_fn_context.await;
	let (site, db, _lease) = &context;
	let first_id = create_record(db, "first").await;
	let second_id = create_record(db, "second").await;

	// Act
	let response = update_inline_edits(
		"TestModel".to_string(),
		request(vec![
			mutation(&first_id, "name", json!("first changed")),
			mutation(&second_id, "name", json!("second changed")),
		]),
		site.clone(),
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("commit inline edits");

	// Assert
	assert_eq!(response.updated, 2);
	assert_eq!(response.errors, vec![]);
	assert_eq!(response.outcomes.len(), 2);
	assert_eq!(response.outcomes[0].object_id, first_id);
	assert_eq!(response.outcomes[0].changed_fields, vec!["name"]);
	assert_eq!(response.outcomes[1].object_id, second_id);
	assert_eq!(response.outcomes[1].changed_fields, vec!["name"]);
	let first = db
		.get::<AdminRecord>("test_models", "id", &response.outcomes[0].object_id)
		.await
		.expect("read first edited row")
		.expect("first edited row exists");
	assert_eq!(first.get("name"), Some(&json!("first changed")));
	let second = db
		.get::<AdminRecord>("test_models", "id", &response.outcomes[1].object_id)
		.await
		.expect("read second edited row")
		.expect("second edited row exists");
	assert_eq!(second.get("name"), Some(&json!("second changed")));
	for object_id in [&first_id, &second_id] {
		let history = query_history(&context, object_id).await;
		assert_eq!(history.count, 1);
		assert_eq!(history.results.len(), 1);
		let event = &history.results[0];
		assert_eq!(event.action_name, "UPDATE");
		assert_eq!(event.actor, "test_staff");
		assert_eq!(event.model_name, "TestModel");
		assert_eq!(event.object_id.as_str(), object_id.as_str());
		assert_eq!(event.changed_fields, vec!["name"]);
		assert_eq!(event.affected_count, 1);
		assert!(event.success);
	}
}

#[rstest]
#[tokio::test]
async fn history_insert_failure_rolls_back_all_inline_edits(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let context = server_fn_context.await;
	let (site, db, _lease) = &context;
	let first_id = create_record(db, "first before rollback").await;
	let second_id = create_record(db, "second before rollback").await;
	assert_eq!(query_history(&context, &first_id).await.count, 0);
	let mut connection = *db.connection();
	OrmExecutor::execute(
		&mut connection,
		"ALTER TABLE reinhardt_admin_history \
		 ADD CONSTRAINT inline_history_test_reject_insert CHECK (FALSE) NOT VALID",
		Vec::new(),
	)
	.await
	.expect("history fault constraint must install");

	// Act
	let result = update_inline_edits(
		"TestModel".to_string(),
		request(vec![
			mutation(&first_id, "name", json!("first must roll back")),
			mutation(&second_id, "name", json!("second must roll back")),
		]),
		site.clone(),
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await;
	let first = db
		.get::<AdminRecord>("test_models", "id", &first_id)
		.await
		.expect("read first rolled-back row")
		.expect("first rolled-back row exists");
	let second = db
		.get::<AdminRecord>("test_models", "id", &second_id)
		.await
		.expect("read second rolled-back row")
		.expect("second rolled-back row exists");
	let first_history = query_history(&context, &first_id).await;
	let second_history = query_history(&context, &second_id).await;

	// Assert
	result.expect_err("history insert failure must fail all inline edits");
	assert_eq!(first.get("name"), Some(&json!("first before rollback")));
	assert_eq!(second.get("name"), Some(&json!("second before rollback")));
	assert_eq!(first_history.count, 0);
	assert_eq!(first_history.results.len(), 0);
	assert_eq!(second_history.count, 0);
	assert_eq!(second_history.results.len(), 0);
}

#[rstest]
#[tokio::test]
async fn inline_edit_returns_typed_error_for_non_editable_field(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _lease) = server_fn_context.await;
	let object_id = create_record(&db, "unchanged").await;

	// Act
	let response = update_inline_edits(
		"TestModel".to_string(),
		request(vec![mutation(&object_id, "status", json!("inactive"))]),
		site,
		db,
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("return typed validation response");

	// Assert
	assert_eq!(response.updated, 0);
	assert_eq!(response.outcomes, vec![]);
	assert_eq!(response.errors.len(), 1);
	assert_eq!(response.errors[0].object_id, object_id);
	assert_eq!(response.errors[0].field.as_deref(), Some("status"));
	assert_eq!(
		response.errors[0].message,
		"Field is not editable in the changelist"
	);
}

#[rstest]
#[tokio::test]
async fn inline_edit_canonicalizes_numeric_identity_before_update_and_outcome(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let context = server_fn_context.await;
	let (site, db, _lease) = &context;
	let object_id = create_record(db, "before canonical update").await;
	let padded_id = format!("0{object_id}");

	// Act
	let response = update_inline_edits(
		"TestModel".to_string(),
		request(vec![mutation(
			padded_id,
			"name",
			json!("after canonical update"),
		)]),
		site.clone(),
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("update canonical numeric object ID");

	// Assert
	assert_eq!(response.updated, 1);
	assert_eq!(response.errors, vec![]);
	assert_eq!(response.outcomes.len(), 1);
	assert_eq!(response.outcomes[0].object_id, object_id);
	assert_eq!(response.outcomes[0].changed_fields, vec!["name"]);
	assert_record_name(
		db,
		&response.outcomes[0].object_id,
		"after canonical update",
	)
	.await;
	let history = query_history(&context, &object_id).await;
	assert_eq!(history.count, 1);
	assert_eq!(history.results.len(), 1);
	assert_eq!(history.results[0].object_id, object_id);
}

#[rstest]
#[tokio::test]
async fn inline_edit_rejects_canonical_numeric_duplicate_without_writes(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _lease) = server_fn_context.await;
	let object_id = create_record(&db, "before duplicate").await;
	let padded_id = format!("0{object_id}");

	// Act
	let response = update_inline_edits(
		"TestModel".to_string(),
		request(vec![
			mutation(&object_id, "name", json!("first duplicate")),
			mutation(&padded_id, "name", json!("second duplicate")),
		]),
		site,
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("return canonical duplicate response");

	// Assert
	assert_eq!(response.updated, 0);
	assert_eq!(response.outcomes, vec![]);
	assert_eq!(response.errors.len(), 1);
	assert_eq!(response.errors[0].object_id, padded_id);
	assert_eq!(response.errors[0].field, None);
	assert_eq!(response.errors[0].message, "Duplicate object ID");
	assert_record_name(&db, &object_id, "before duplicate").await;
}

#[rstest]
#[tokio::test]
async fn inline_edit_rejects_empty_batch_with_typed_error(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _lease) = server_fn_context.await;

	// Act
	let response = update_inline_edits(
		"TestModel".to_string(),
		request(vec![]),
		site,
		db,
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("return empty-batch response");

	// Assert
	assert_eq!(response.updated, 0);
	assert_eq!(response.outcomes, vec![]);
	assert_eq!(response.errors.len(), 1);
	assert_eq!(response.errors[0].object_id, "");
	assert_eq!(response.errors[0].field, None);
	assert_eq!(
		response.errors[0].message,
		"At least one row update is required"
	);
}

#[rstest]
#[tokio::test]
async fn inline_edit_rejects_aggregate_payload_before_writes(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _lease) = server_fn_context.await;
	let object_id = create_record(&db, "before oversized batch").await;
	let oversized_name = "x".repeat(10_000_000);

	// Act
	let response = update_inline_edits(
		"TestModel".to_string(),
		request(vec![mutation(&object_id, "name", json!(oversized_name))]),
		site,
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("return oversized-batch response");

	// Assert
	assert_eq!(response.updated, 0);
	assert_eq!(response.outcomes, vec![]);
	assert_eq!(response.errors.len(), 1);
	assert_eq!(response.errors[0].object_id, "");
	assert_eq!(response.errors[0].field, None);
	assert_eq!(
		response.errors[0].message,
		"Payload too large (max 10000000 bytes)"
	);
	assert_record_name(&db, &object_id, "before oversized batch").await;
}

#[rstest]
#[tokio::test]
async fn inline_edit_rejects_typed_primary_key_mismatch_before_writes(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _lease) = server_fn_context.await;
	let existing_id = create_record(&db, "before invalid identity").await;

	// Act
	let response = update_inline_edits(
		"TestModel".to_string(),
		request(vec![mutation(
			"not-an-integer",
			"name",
			json!("must not persist"),
		)]),
		site,
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("return invalid-primary-key response");

	// Assert
	assert_eq!(response.updated, 0);
	assert_eq!(response.outcomes, vec![]);
	assert_eq!(response.errors.len(), 1);
	assert_eq!(response.errors[0].object_id, "not-an-integer");
	assert_eq!(response.errors[0].field, None);
	assert_eq!(response.errors[0].message, "Invalid primary key value");
	assert_record_name(&db, &existing_id, "before invalid identity").await;
}

#[rstest]
#[tokio::test]
async fn inline_edit_rejects_value_type_mismatch_before_writes(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _lease) = server_fn_context.await;
	let object_id = create_record(&db, "before type mismatch").await;

	// Act
	let response = update_inline_edits(
		"TestModel".to_string(),
		request(vec![mutation(&object_id, "name", json!(42))]),
		site,
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("return value-type response");

	// Assert
	assert_eq!(response.updated, 0);
	assert_eq!(response.outcomes, vec![]);
	assert_eq!(response.errors.len(), 1);
	assert_eq!(response.errors[0].object_id, object_id);
	assert_eq!(response.errors[0].field.as_deref(), Some("name"));
	assert_eq!(response.errors[0].message, "Invalid value type");
	assert_record_name(&db, &response.errors[0].object_id, "before type mismatch").await;
}

#[rstest]
#[tokio::test]
async fn inline_edit_aggregates_multiple_row_errors_without_writes(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _lease) = server_fn_context.await;
	let first_id = create_record(&db, "first before errors").await;
	let second_id = create_record(&db, "second before errors").await;

	// Act
	let response = update_inline_edits(
		"TestModel".to_string(),
		request(vec![
			mutation(&first_id, "status", json!("inactive")),
			mutation(&second_id, "status", json!("inactive")),
		]),
		site,
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("return all row errors");

	// Assert
	assert_eq!(response.updated, 0);
	assert_eq!(response.outcomes, vec![]);
	assert_eq!(response.errors.len(), 2);
	assert_eq!(response.errors[0].object_id, first_id);
	assert_eq!(response.errors[1].object_id, second_id);
	assert_eq!(
		response
			.errors
			.iter()
			.map(|error| error.field.as_deref())
			.collect::<Vec<_>>(),
		vec![Some("status"), Some("status")]
	);
	assert_record_name(&db, &response.errors[0].object_id, "first before errors").await;
	assert_record_name(&db, &response.errors[1].object_id, "second before errors").await;
}

#[rstest]
#[tokio::test]
async fn inline_edit_rejects_invalid_csrf(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _lease) = server_fn_context.await;
	let object_id = create_record(&db, "before invalid csrf").await;
	let mut edit_request = request(vec![mutation(
		&object_id,
		"name",
		json!("must not persist"),
	)]);
	edit_request.csrf_token = "wrong-token".to_string();

	// Act
	let result = update_inline_edits(
		"TestModel".to_string(),
		edit_request,
		site,
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await;

	// Assert
	assert!(result.is_err());
	assert_record_name(&db, &object_id, "before invalid csrf").await;
}

#[rstest]
#[tokio::test]
async fn inline_edit_rejects_change_permission_denial(
	#[future] server_fn_context_deny_all: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _lease) = server_fn_context_deny_all.await;
	let object_id = create_record(&db, "before permission denial").await;

	// Act
	let result = update_inline_edits(
		"TestModel".to_string(),
		request(vec![mutation(
			&object_id,
			"name",
			json!("must not persist"),
		)]),
		site,
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await;

	// Assert
	assert!(result.is_err());
	assert_record_name(&db, &object_id, "before permission denial").await;
}

#[rstest]
#[tokio::test]
async fn missing_later_row_rolls_back_earlier_update(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _lease) = server_fn_context.await;
	let first_id = create_record(&db, "before rollback").await;
	let missing_id = "02147483647";

	// Act
	let response = update_inline_edits(
		"TestModel".to_string(),
		request(vec![
			mutation(&first_id, "name", json!("must roll back")),
			mutation(missing_id, "name", json!("missing")),
		]),
		site,
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("return missing-row response");

	// Assert
	assert_eq!(response.updated, 0);
	assert_eq!(response.outcomes, vec![]);
	assert_eq!(response.errors.len(), 1);
	assert_eq!(response.errors[0].object_id, missing_id);
	assert_eq!(response.errors[0].field, None);
	assert_eq!(response.errors[0].message, "Object was not found");
	let first = db
		.get::<AdminRecord>("test_models", "id", &first_id)
		.await
		.expect("read rolled-back row")
		.expect("rolled-back row exists");
	assert_eq!(first.get("name"), Some(&json!("before rollback")));
}

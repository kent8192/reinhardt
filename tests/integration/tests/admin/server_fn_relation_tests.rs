//! Integration tests for permission-aware admin relation lookups.

use super::server_fn_helpers::{
	AdminDatabaseDepends, ServerFnContext, TEST_CSRF_TOKEN, make_auth_user, make_staff_request,
	relation_invalid_config_context, relation_logical_fields_context,
	relation_logical_readonly_context, relation_physical_readonly_context,
	relation_pk_fallback_context, relation_server_fn_context, relation_source_denied_context,
	relation_target_denied_context,
};
use reinhardt_admin::adapters::MutationRequest;
use reinhardt_admin::core::AdminRecord;
use reinhardt_admin::server::{create_record, get_fields, get_relation_options, update_record};
use reinhardt_admin::types::{FieldType, RelationLookupRequest, RelationOption, RelationWidget};
use reinhardt_pages::server_fn::ServerFnErrorKind;
use rstest::*;
use serde_json::{Value, json};
use serial_test::serial;
use std::collections::HashMap;

const RELATION_UUID: &str = "5f7278bc-9669-4fdf-8492-b57d5fd908ce";

fn relation_mutation_data(target: Value) -> HashMap<String, Value> {
	HashMap::from([
		("title".to_string(), json!("Created relation source")),
		("target_key".to_string(), target),
		("reviewer_key".to_string(), json!(2)),
		("text_target_key".to_string(), json!("001")),
		("uuid_target_key".to_string(), json!(RELATION_UUID)),
		("optional_target_key".to_string(), Value::Null),
	])
}

async fn relation_source_rows(
	db: &AdminDatabaseDepends,
) -> Vec<HashMap<String, serde_json::Value>> {
	db.list::<AdminRecord>("admin_relation_sources", Vec::new(), 0, 10)
		.await
		.expect("relation source rows should load")
}

async fn relation_source_record(db: &AdminDatabaseDepends) -> HashMap<String, serde_json::Value> {
	db.get::<AdminRecord>("admin_relation_sources", "id", "1")
		.await
		.expect("relation source row lookup should succeed")
		.expect("seeded relation source row should exist")
}

#[rstest]
#[case("Alpha", "1", "Alpha Writer (writer-001)")]
#[case("special-code", "2", "Beta Editor (special-code)")]
#[tokio::test]
#[serial(admin_relation_server_fn)]
async fn server_fn_relation_searches_every_related_admin_search_field(
	#[future] relation_server_fn_context: ServerFnContext,
	#[case] query: &str,
	#[case] expected_id: &str,
	#[case] expected_label: &str,
) {
	// Arrange
	let (site, db, _connection_lease) = relation_server_fn_context.await;

	// Act
	let response = get_relation_options(
		"AdminRelationSourceModel".to_string(),
		"target".to_string(),
		RelationLookupRequest::Search {
			query: query.to_string(),
			page: Some(1),
			page_size: Some(20),
		},
		site,
		db,
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("relation search should succeed");

	// Assert
	assert_eq!(
		response.results,
		vec![RelationOption {
			id: expected_id.to_string(),
			label: expected_label.to_string(),
		}]
	);
	assert_eq!(response.page, 1);
	assert_eq!(response.has_next, false);
}

#[rstest]
#[case(1, vec!["1", "2"], true)]
#[case(53, vec!["105"], false)]
#[tokio::test]
#[serial(admin_relation_server_fn)]
async fn server_fn_relation_returns_strict_bounded_pagination_metadata(
	#[future] relation_server_fn_context: ServerFnContext,
	#[case] page: u64,
	#[case] expected_ids: Vec<&str>,
	#[case] expected_has_next: bool,
) {
	// Arrange
	let (site, db, _connection_lease) = relation_server_fn_context.await;

	// Act
	let response = get_relation_options(
		"AdminRelationSourceModel".to_string(),
		"target".to_string(),
		RelationLookupRequest::Search {
			query: String::new(),
			page: Some(page),
			page_size: Some(2),
		},
		site,
		db,
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("paginated relation search should succeed");

	// Assert
	assert_eq!(
		response
			.results
			.iter()
			.map(|option| option.id.as_str())
			.collect::<Vec<_>>(),
		expected_ids
	);
	assert_eq!(response.page, page);
	assert_eq!(response.has_next, expected_has_next);
}

#[rstest]
#[tokio::test]
#[serial(admin_relation_server_fn)]
async fn server_fn_relation_accepts_the_maximum_page(
	#[future] relation_server_fn_context: ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = relation_server_fn_context.await;

	// Act
	let response = get_relation_options(
		"AdminRelationSourceModel".to_string(),
		"target".to_string(),
		RelationLookupRequest::Search {
			query: String::new(),
			page: Some(10_000),
			page_size: Some(1),
		},
		site,
		db,
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("the maximum relation page should succeed");

	// Assert
	assert_eq!(response.results, Vec::<RelationOption>::new());
	assert_eq!(response.page, 10_000);
	assert_eq!(response.has_next, false);
}

#[rstest]
#[tokio::test]
#[serial(admin_relation_server_fn)]
async fn server_fn_relation_rejects_an_unbounded_page(
	#[future] relation_server_fn_context: ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = relation_server_fn_context.await;

	// Act
	let error = get_relation_options(
		"AdminRelationSourceModel".to_string(),
		"target".to_string(),
		RelationLookupRequest::Search {
			query: String::new(),
			page: Some(u64::MAX),
			page_size: Some(100),
		},
		site,
		db,
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect_err("a relation page above the maximum must be rejected");

	// Assert
	assert_eq!(error.kind(), ServerFnErrorKind::Application);
	assert_eq!(error.status(), None);
	assert_eq!(
		error.user_message(),
		"Relation page exceeds maximum of 10000"
	);
}

#[rstest]
#[case(0, None, 1, 20, true)]
#[case(1, Some(1_000), 1, 100, true)]
#[tokio::test]
#[serial(admin_relation_server_fn)]
async fn server_fn_relation_normalizes_page_and_caps_page_size(
	#[future] relation_server_fn_context: ServerFnContext,
	#[case] page: u64,
	#[case] page_size: Option<u64>,
	#[case] expected_page: u64,
	#[case] expected_count: usize,
	#[case] expected_has_next: bool,
) {
	// Arrange
	let (site, db, _connection_lease) = relation_server_fn_context.await;

	// Act
	let response = get_relation_options(
		"AdminRelationSourceModel".to_string(),
		"target".to_string(),
		RelationLookupRequest::Search {
			query: String::new(),
			page: Some(page),
			page_size,
		},
		site,
		db,
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("bounded relation search should succeed");

	// Assert
	assert_eq!(response.page, expected_page);
	assert_eq!(response.results.len(), expected_count);
	assert_eq!(response.has_next, expected_has_next);
}

#[rstest]
#[tokio::test]
#[serial(admin_relation_server_fn)]
async fn server_fn_relation_accepts_a_two_hundred_byte_query(
	#[future] relation_server_fn_context: ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = relation_server_fn_context.await;
	let query = "x".repeat(200);

	// Act
	let response = get_relation_options(
		"AdminRelationSourceModel".to_string(),
		"target".to_string(),
		RelationLookupRequest::Search {
			query,
			page: Some(1),
			page_size: Some(20),
		},
		site,
		db,
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("a query at the byte limit should succeed");

	// Assert
	assert_eq!(response.results, Vec::<RelationOption>::new());
	assert_eq!(response.page, 1);
	assert_eq!(response.has_next, false);
}

#[rstest]
#[tokio::test]
#[serial(admin_relation_server_fn)]
async fn server_fn_relation_rejects_a_two_hundred_and_one_byte_query(
	#[future] relation_server_fn_context: ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = relation_server_fn_context.await;
	let query = "x".repeat(201);

	// Act
	let error = get_relation_options(
		"AdminRelationSourceModel".to_string(),
		"target".to_string(),
		RelationLookupRequest::Search {
			query,
			page: Some(1),
			page_size: Some(20),
		},
		site,
		db,
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect_err("a query above the byte limit must be rejected");

	// Assert
	assert_eq!(error.kind(), ServerFnErrorKind::Application);
	assert_eq!(error.status(), None);
	assert_eq!(
		error.user_message(),
		"Relation query exceeds maximum length of 200 bytes"
	);
}

#[rstest]
#[tokio::test]
#[serial(admin_relation_server_fn)]
async fn server_fn_relation_resolves_an_explicit_object_label(
	#[future] relation_server_fn_context: ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = relation_server_fn_context.await;

	// Act
	let response = get_relation_options(
		"AdminRelationSourceModel".to_string(),
		"reviewer_key".to_string(),
		RelationLookupRequest::Resolve {
			id: "2".to_string(),
		},
		site,
		db,
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("exact relation resolution should succeed");

	// Assert
	assert_eq!(
		response.results,
		vec![RelationOption {
			id: "2".to_string(),
			label: "Beta Editor (special-code)".to_string(),
		}]
	);
	assert_eq!(response.page, 1);
	assert_eq!(response.has_next, false);
}

#[rstest]
#[tokio::test]
#[serial(admin_relation_server_fn)]
async fn server_fn_relation_resolves_a_text_primary_key_exactly(
	#[future] relation_server_fn_context: ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = relation_server_fn_context.await;

	// Act
	let response = get_relation_options(
		"AdminRelationSourceModel".to_string(),
		"text_target".to_string(),
		RelationLookupRequest::Resolve {
			id: "001".to_string(),
		},
		site,
		db,
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("text primary key resolution should preserve leading zeroes");

	// Assert
	assert_eq!(
		response.results,
		vec![RelationOption {
			id: "001".to_string(),
			label: "001".to_string(),
		}]
	);
	assert_eq!(response.page, 1);
	assert_eq!(response.has_next, false);
}

#[rstest]
#[tokio::test]
#[serial(admin_relation_server_fn)]
async fn server_fn_relation_rejects_a_malformed_uuid_before_database_execution(
	#[future] relation_server_fn_context: ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = relation_server_fn_context.await;

	// Act
	let error = get_relation_options(
		"AdminRelationSourceModel".to_string(),
		"uuid_target".to_string(),
		RelationLookupRequest::Resolve {
			id: "not-a-uuid".to_string(),
		},
		site,
		db,
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect_err("malformed registered UUID must fail before a database type error");

	// Assert
	assert_eq!(error.kind(), ServerFnErrorKind::Application);
	assert_eq!(error.status(), None);
	assert_eq!(
		error.user_message(),
		"Invalid UUID primary key value 'not-a-uuid' for field 'id'"
	);
}

#[rstest]
#[tokio::test]
#[serial(admin_relation_server_fn)]
async fn server_fn_relation_falls_back_to_the_primary_key_label(
	#[future] relation_pk_fallback_context: ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = relation_pk_fallback_context.await;

	// Act
	let response = get_relation_options(
		"AdminRelationSourceModel".to_string(),
		"target".to_string(),
		RelationLookupRequest::Resolve {
			id: "1".to_string(),
		},
		site,
		db,
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("primary-key fallback resolution should succeed");

	// Assert
	assert_eq!(
		response.results,
		vec![RelationOption {
			id: "1".to_string(),
			label: "1".to_string(),
		}]
	);
	assert_eq!(response.page, 1);
	assert_eq!(response.has_next, false);
}

#[rstest]
#[tokio::test]
#[serial(admin_relation_server_fn)]
async fn server_fn_relation_checks_source_permission_before_field_configuration(
	#[future] relation_source_denied_context: ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = relation_source_denied_context.await;

	// Act
	let error = get_relation_options(
		"AdminRelationSourceModel".to_string(),
		"not_configured".to_string(),
		RelationLookupRequest::Resolve {
			id: "1".to_string(),
		},
		site,
		db,
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect_err("source permission denial must precede field validation");

	// Assert
	assert_eq!(error.kind(), ServerFnErrorKind::Server);
	assert_eq!(error.status(), Some(403));
	assert_eq!(error.user_message(), "Permission denied");
}

#[rstest]
#[tokio::test]
#[serial(admin_relation_server_fn)]
async fn server_fn_relation_checks_target_permission_before_row_resolution(
	#[future] relation_target_denied_context: ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = relation_target_denied_context.await;

	// Act
	let error = get_relation_options(
		"AdminRelationSourceModel".to_string(),
		"target".to_string(),
		RelationLookupRequest::Resolve {
			id: "999999".to_string(),
		},
		site,
		db,
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect_err("target permission denial must precede row resolution");

	// Assert
	assert_eq!(error.kind(), ServerFnErrorKind::Server);
	assert_eq!(error.status(), Some(403));
	assert_eq!(error.user_message(), "Permission denied");
}

#[rstest]
#[case::missing(json!(999_999), ServerFnErrorKind::Application, None, "Related object 'AdminRelationTargetModel' with id '999999' does not exist")]
#[case::boolean(json!(true), ServerFnErrorKind::Application, None, "Relation primary keys must be scalar values")]
#[case::array(json!([1]), ServerFnErrorKind::Application, None, "Relation primary keys must be scalar values")]
#[case::object(json!({"id": 1}), ServerFnErrorKind::Application, None, "Relation primary keys must be scalar values")]
#[case::required_null(
	Value::Null,
	ServerFnErrorKind::Validation,
	Some(422),
	"Validation failed"
)]
#[tokio::test]
#[serial(admin_relation_server_fn)]
async fn server_fn_relation_create_rejects_invalid_target_without_writing(
	#[future] relation_server_fn_context: ServerFnContext,
	#[case] target: Value,
	#[case] expected_kind: ServerFnErrorKind,
	#[case] expected_status: Option<u16>,
	#[case] expected_message: &str,
) {
	// Arrange
	let (site, db, _connection_lease) = relation_server_fn_context.await;
	let before = relation_source_rows(&db).await;
	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data: relation_mutation_data(target),
	};

	// Act
	let error = create_record(
		"AdminRelationSourceModel".to_string(),
		request,
		site,
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect_err("invalid relation values must be rejected before create");
	let after = relation_source_rows(&db).await;

	// Assert
	assert_eq!(error.kind(), expected_kind);
	assert_eq!(error.status(), expected_status);
	assert_eq!(error.user_message(), expected_message);
	assert_eq!(after, before);
}

#[rstest]
#[case::existing(json!(1))]
#[case::missing(json!(999_999))]
#[tokio::test]
#[serial(admin_relation_server_fn)]
async fn server_fn_relation_create_checks_target_permission_before_existence_without_writing(
	#[future] relation_target_denied_context: ServerFnContext,
	#[case] target: Value,
) {
	// Arrange
	let (site, db, _connection_lease) = relation_target_denied_context.await;
	let before = relation_source_rows(&db).await;
	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data: relation_mutation_data(target),
	};

	// Act
	let error = create_record(
		"AdminRelationSourceModel".to_string(),
		request,
		site,
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect_err("target permission denial must reject relation create");
	let after = relation_source_rows(&db).await;

	// Assert
	assert_eq!(error.kind(), ServerFnErrorKind::Server);
	assert_eq!(error.status(), Some(403));
	assert_eq!(error.user_message(), "Permission denied");
	assert_eq!(after, before);
}

#[rstest]
#[case::number(json!(1))]
#[case::string(json!("1"))]
#[tokio::test]
#[serial(admin_relation_server_fn)]
async fn server_fn_relation_create_accepts_scalar_ids_and_nullable_null(
	#[future] relation_server_fn_context: ServerFnContext,
	#[case] target: Value,
) {
	// Arrange
	let (site, db, _connection_lease) = relation_server_fn_context.await;
	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data: relation_mutation_data(target),
	};

	// Act
	let response = create_record(
		"AdminRelationSourceModel".to_string(),
		request,
		site,
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("permitted scalar relations and nullable null should create");
	let rows = relation_source_rows(&db).await;

	// Assert
	assert_eq!(response.success, true);
	assert_eq!(rows.len(), 2);
	assert_eq!(
		rows.iter()
			.find(|row| row.get("title") == Some(&json!("Created relation source")))
			.and_then(|row| row.get("optional_target_key")),
		Some(&Value::Null)
	);
}

#[rstest]
#[tokio::test]
#[serial(admin_relation_server_fn)]
async fn server_fn_relation_create_accepts_physical_names_for_logical_field_allowlist(
	#[future] relation_logical_fields_context: ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = relation_logical_fields_context.await;
	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data: relation_mutation_data(json!(2)),
	};

	// Act
	let response = create_record(
		"AdminRelationSourceModel".to_string(),
		request,
		site,
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("physical form relation names should satisfy a logical field allowlist");
	let rows = relation_source_rows(&db).await;
	let created = rows
		.iter()
		.find(|row| row.get("title") == Some(&json!("Created relation source")))
		.expect("the relation source should be created");

	// Assert
	assert_eq!(response.success, true);
	assert_eq!(rows.len(), 2);
	assert_eq!(created.get("target_key"), Some(&json!(2)));
}

#[rstest]
#[tokio::test]
#[serial(admin_relation_server_fn)]
async fn server_fn_relation_create_rejects_physical_alias_of_logical_readonly_without_writing(
	#[future] relation_logical_readonly_context: ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = relation_logical_readonly_context.await;
	let before = relation_source_rows(&db).await;
	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data: HashMap::from([("target_key".to_string(), json!(2))]),
	};

	// Act
	let error = create_record(
		"AdminRelationSourceModel".to_string(),
		request,
		site,
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect_err("a physical alias must not bypass a logical readonly field");
	let after = relation_source_rows(&db).await;

	// Assert
	assert_eq!(error.kind(), ServerFnErrorKind::Application);
	assert_eq!(error.status(), None);
	assert_eq!(
		error.user_message(),
		"Field 'target_key' is read-only and cannot be modified"
	);
	assert_eq!(after, before);
}

#[rstest]
#[tokio::test]
#[serial(admin_relation_server_fn)]
async fn server_fn_relation_create_preserves_the_exact_validated_text_primary_key(
	#[future] relation_server_fn_context: ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = relation_server_fn_context.await;
	let mut data = relation_mutation_data(json!(1));
	data.insert("text_target_key".to_string(), json!("raw<&"));
	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};

	// Act
	create_record(
		"AdminRelationSourceModel".to_string(),
		request,
		site,
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("the exact validated text relation ID should create");
	let rows = relation_source_rows(&db).await;
	let created = rows
		.iter()
		.find(|row| row.get("title") == Some(&json!("Created relation source")))
		.expect("created relation source should exist");

	// Assert
	assert_eq!(created.get("text_target_key"), Some(&json!("raw<&")));
}

#[rstest]
#[case::missing(json!(999_999), ServerFnErrorKind::Application, None, "Related object 'AdminRelationTargetModel' with id '999999' does not exist")]
#[case::boolean(json!(true), ServerFnErrorKind::Application, None, "Relation primary keys must be scalar values")]
#[case::array(json!([1]), ServerFnErrorKind::Application, None, "Relation primary keys must be scalar values")]
#[case::object(json!({"id": 1}), ServerFnErrorKind::Application, None, "Relation primary keys must be scalar values")]
#[case::required_null(
	Value::Null,
	ServerFnErrorKind::Validation,
	Some(422),
	"Validation failed"
)]
#[tokio::test]
#[serial(admin_relation_server_fn)]
async fn server_fn_relation_update_rejects_invalid_target_without_changing_row(
	#[future] relation_server_fn_context: ServerFnContext,
	#[case] target: Value,
	#[case] expected_kind: ServerFnErrorKind,
	#[case] expected_status: Option<u16>,
	#[case] expected_message: &str,
) {
	// Arrange
	let (site, db, _connection_lease) = relation_server_fn_context.await;
	let before = relation_source_record(&db).await;
	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data: HashMap::from([("target_key".to_string(), target)]),
	};

	// Act
	let error = update_record(
		"AdminRelationSourceModel".to_string(),
		"1".to_string(),
		request,
		site,
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect_err("invalid relation values must be rejected before update");
	let after = relation_source_record(&db).await;

	// Assert
	assert_eq!(error.kind(), expected_kind);
	assert_eq!(error.status(), expected_status);
	assert_eq!(error.user_message(), expected_message);
	assert_eq!(after, before);
}

#[rstest]
#[case::existing(json!(2))]
#[case::missing(json!(999_999))]
#[tokio::test]
#[serial(admin_relation_server_fn)]
async fn server_fn_relation_update_checks_target_permission_before_existence_without_changing_row(
	#[future] relation_target_denied_context: ServerFnContext,
	#[case] target: Value,
) {
	// Arrange
	let (site, db, _connection_lease) = relation_target_denied_context.await;
	let before = relation_source_record(&db).await;
	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data: HashMap::from([("target_key".to_string(), target)]),
	};

	// Act
	let error = update_record(
		"AdminRelationSourceModel".to_string(),
		"1".to_string(),
		request,
		site,
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect_err("target permission denial must reject relation update");
	let after = relation_source_record(&db).await;

	// Assert
	assert_eq!(error.kind(), ServerFnErrorKind::Server);
	assert_eq!(error.status(), Some(403));
	assert_eq!(error.user_message(), "Permission denied");
	assert_eq!(after, before);
}

#[rstest]
#[case::number(json!(2))]
#[case::string(json!("2"))]
#[tokio::test]
#[serial(admin_relation_server_fn)]
async fn server_fn_relation_update_accepts_scalar_ids_and_nullable_null(
	#[future] relation_server_fn_context: ServerFnContext,
	#[case] target: Value,
) {
	// Arrange
	let (site, db, _connection_lease) = relation_server_fn_context.await;
	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data: HashMap::from([
			("target_key".to_string(), target),
			("optional_target_key".to_string(), Value::Null),
		]),
	};

	// Act
	let response = update_record(
		"AdminRelationSourceModel".to_string(),
		"1".to_string(),
		request,
		site,
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("permitted scalar relations and nullable null should update");
	let row = relation_source_record(&db).await;

	// Assert
	assert_eq!(response.success, true);
	assert_eq!(row.get("target_key"), Some(&json!(2)));
	assert_eq!(row.get("optional_target_key"), Some(&Value::Null));
}

#[rstest]
#[tokio::test]
#[serial(admin_relation_server_fn)]
async fn server_fn_relation_update_accepts_logical_name_for_physical_field_allowlist(
	#[future] relation_server_fn_context: ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = relation_server_fn_context.await;
	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data: HashMap::from([
			("title".to_string(), json!("Logical alias update")),
			("target".to_string(), json!(2)),
		]),
	};

	// Act
	let response = update_record(
		"AdminRelationSourceModel".to_string(),
		"1".to_string(),
		request,
		site,
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("a logical relation name should satisfy its physical field allowlist");
	let row = relation_source_record(&db).await;

	// Assert
	assert_eq!(response.success, true);
	assert_eq!(row.get("title"), Some(&json!("Logical alias update")));
	assert_eq!(row.get("target_key"), Some(&json!(2)));
}

#[rstest]
#[tokio::test]
#[serial(admin_relation_server_fn)]
async fn server_fn_relation_update_rejects_logical_alias_of_physical_readonly_without_writing(
	#[future] relation_physical_readonly_context: ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = relation_physical_readonly_context.await;
	let before = relation_source_record(&db).await;
	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data: HashMap::from([("target".to_string(), json!(2))]),
	};

	// Act
	let error = update_record(
		"AdminRelationSourceModel".to_string(),
		"1".to_string(),
		request,
		site,
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect_err("a logical alias must not bypass a physical readonly field");
	let after = relation_source_record(&db).await;

	// Assert
	assert_eq!(error.kind(), ServerFnErrorKind::Application);
	assert_eq!(error.status(), None);
	assert_eq!(
		error.user_message(),
		"Field 'target' is read-only and cannot be modified"
	);
	assert_eq!(after, before);
}

#[rstest]
#[tokio::test]
#[serial(admin_relation_server_fn)]
async fn server_fn_relation_update_ignores_absent_relation_fields(
	#[future] relation_server_fn_context: ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = relation_server_fn_context.await;
	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data: HashMap::from([("title".to_string(), json!("Partial update"))]),
	};

	// Act
	let response = update_record(
		"AdminRelationSourceModel".to_string(),
		"1".to_string(),
		request,
		site,
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("partial updates should not require absent relation values");
	let row = relation_source_record(&db).await;

	// Assert
	assert_eq!(response.success, true);
	assert_eq!(row.get("title"), Some(&json!("Partial update")));
	assert_eq!(row.get("target_key"), Some(&json!(1)));
}

#[rstest]
#[tokio::test]
#[serial(admin_relation_server_fn)]
async fn server_fn_relation_get_fields_uses_physical_names_and_permission_aware_labels(
	#[future] relation_server_fn_context: ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = relation_server_fn_context.await;

	// Act
	let response = get_fields(
		"AdminRelationSourceModel".to_string(),
		Some("1".to_string()),
		site,
		db,
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("relation edit fields should resolve initial options");
	let relation_fields = response
		.fields
		.into_iter()
		.filter_map(|field| match field.field_type {
			FieldType::Relation {
				field_name,
				widget,
				selected,
				readonly: _,
			} => Some((field.name, field_name, widget, selected, field.required)),
			_ => None,
		})
		.collect::<Vec<_>>();

	// Assert
	assert_eq!(
		relation_fields,
		vec![
			(
				"target_key".to_string(),
				"target".to_string(),
				RelationWidget::Autocomplete,
				Some(RelationOption {
					id: "1".to_string(),
					label: "Alpha Writer (writer-001)".to_string(),
				}),
				true,
			),
			(
				"reviewer_key".to_string(),
				"reviewer".to_string(),
				RelationWidget::RawId,
				Some(RelationOption {
					id: "2".to_string(),
					label: "Beta Editor (special-code)".to_string(),
				}),
				true,
			),
			(
				"text_target_key".to_string(),
				"text_target".to_string(),
				RelationWidget::RawId,
				Some(RelationOption {
					id: "001".to_string(),
					label: "001".to_string(),
				}),
				true,
			),
			(
				"uuid_target_key".to_string(),
				"uuid_target".to_string(),
				RelationWidget::RawId,
				Some(RelationOption {
					id: RELATION_UUID.to_string(),
					label: RELATION_UUID.to_string(),
				}),
				true,
			),
			(
				"optional_target_key".to_string(),
				"optional_target".to_string(),
				RelationWidget::RawId,
				None,
				false,
			),
		]
	);
}

#[rstest]
#[tokio::test]
#[serial(admin_relation_server_fn)]
async fn server_fn_relation_get_fields_rejects_invalid_full_configuration(
	#[future] relation_invalid_config_context: ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = relation_invalid_config_context.await;

	// Act
	let error = get_fields(
		"AdminRelationSourceModel".to_string(),
		None,
		site,
		db,
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect_err("get_fields must validate the complete relation configuration");

	// Assert
	assert_eq!(error.kind(), ServerFnErrorKind::Application);
	assert_eq!(error.status(), None);
	assert_eq!(
		error.user_message(),
		"Related admin 'AdminRelationTargetModel' for field 'target' must configure search_fields for autocomplete"
	);
}

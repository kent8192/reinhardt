//! Integration tests for permission-aware admin relation lookups.

use super::server_fn_helpers::{
	RelationServerFnContext, make_auth_user, make_staff_request, relation_invalid_config_context,
	relation_pk_fallback_context, relation_server_fn_context, relation_source_denied_context,
	relation_target_denied_context,
};
use reinhardt_admin::server::{get_fields, get_relation_options};
use reinhardt_admin::types::{FieldType, RelationLookupRequest, RelationOption, RelationWidget};
use reinhardt_pages::server_fn::ServerFnErrorKind;
use rstest::*;
use serial_test::serial;

#[rstest]
#[case("Alpha", "1", "Alpha Writer (writer-001)")]
#[case("special-code", "2", "Beta Editor (special-code)")]
#[tokio::test]
#[serial(admin_relation_server_fn)]
async fn server_fn_relation_searches_every_related_admin_search_field(
	#[future] relation_server_fn_context: RelationServerFnContext,
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
	#[future] relation_server_fn_context: RelationServerFnContext,
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
#[case(0, None, 1, 20, true)]
#[case(1, Some(1_000), 1, 100, true)]
#[tokio::test]
#[serial(admin_relation_server_fn)]
async fn server_fn_relation_normalizes_page_and_caps_page_size(
	#[future] relation_server_fn_context: RelationServerFnContext,
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
	#[future] relation_server_fn_context: RelationServerFnContext,
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
	#[future] relation_server_fn_context: RelationServerFnContext,
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
	#[future] relation_server_fn_context: RelationServerFnContext,
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
async fn server_fn_relation_falls_back_to_the_primary_key_label(
	#[future] relation_pk_fallback_context: RelationServerFnContext,
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
	#[future] relation_source_denied_context: RelationServerFnContext,
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
	#[future] relation_target_denied_context: RelationServerFnContext,
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
#[tokio::test]
#[serial(admin_relation_server_fn)]
async fn server_fn_relation_get_fields_uses_physical_names_and_permission_aware_labels(
	#[future] relation_server_fn_context: RelationServerFnContext,
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
		]
	);
}

#[rstest]
#[tokio::test]
#[serial(admin_relation_server_fn)]
async fn server_fn_relation_get_fields_rejects_invalid_full_configuration(
	#[future] relation_invalid_config_context: RelationServerFnContext,
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

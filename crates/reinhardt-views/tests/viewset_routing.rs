// Integration tests for ViewSet routing in reinhardt-views crate.
// Covers Action definition, ViewSet registration, URL generation, builder validation,
// pagination config, filtering config, ordering config, and nested resources.

use bytes::Bytes;
use hyper::{HeaderMap, Method, StatusCode, Version};
use reinhardt_http::Request;
use reinhardt_views::viewset_actions;
use reinhardt_views::viewsets::{
	Action, ActionMetadata, ActionType, FilterConfig, GenericViewSet, ModelViewSet, NestedResource,
	NestedResourcePath, NestedViewSet, OrderingConfig, PaginationConfig, ReadOnlyModelViewSet,
	ViewSet, action, clear_actions, get_registered_actions, nested_detail_url, nested_url,
	register_action,
};
use rstest::rstest;
use serial_test::serial;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Helper types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
#[allow(dead_code)] // test helper struct with derived traits
struct TestModel {
	id: i64,
	name: String,
}

#[derive(Debug, Clone)]
struct TestSerializer;

fn make_request(method: Method, uri: &str) -> Request {
	Request::builder()
		.method(method)
		.uri(uri)
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap()
}

fn make_request_with_body(method: Method, uri: &str, body: &'static str) -> Request {
	Request::builder()
		.method(method)
		.uri(uri)
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::from(body))
		.build()
		.unwrap()
}

// ===========================================================================
// Action definition tests
// ===========================================================================

#[rstest]
fn action_list_is_not_detail() {
	// Act
	let action = Action::list();

	// Assert
	assert_eq!(action.action_type, ActionType::List);
	assert!(!action.detail, "list action must not be a detail action");
}

#[rstest]
fn action_retrieve_is_detail() {
	// Act
	let action = Action::retrieve();

	// Assert
	assert_eq!(action.action_type, ActionType::Retrieve);
	assert!(action.detail, "retrieve action must be a detail action");
}

#[rstest]
fn action_create_is_not_detail() {
	// Act
	let action = Action::create();

	// Assert
	assert_eq!(action.action_type, ActionType::Create);
	assert!(!action.detail, "create action must not be a detail action");
}

#[rstest]
fn action_update_is_detail() {
	// Act
	let action = Action::update();

	// Assert
	assert_eq!(action.action_type, ActionType::Update);
	assert!(action.detail, "update action must be a detail action");
}

#[rstest]
fn action_partial_update_is_detail() {
	// Act
	let action = Action::partial_update();

	// Assert
	assert_eq!(action.action_type, ActionType::PartialUpdate);
	assert!(
		action.detail,
		"partial_update action must be a detail action"
	);
}

#[rstest]
fn action_destroy_is_detail() {
	// Act
	let action = Action::destroy();

	// Assert
	assert_eq!(action.action_type, ActionType::Destroy);
	assert!(action.detail, "destroy action must be a detail action");
}

#[rstest]
fn action_custom_list_type() {
	// Act
	let action = Action::custom("recent", false);

	// Assert
	assert!(!action.detail, "custom list-type action must not be detail");
	match action.action_type {
		ActionType::Custom(name) => assert_eq!(name.as_ref(), "recent"),
		other => panic!("expected Custom variant, got {:?}", other),
	}
}

#[rstest]
fn action_custom_detail_type() {
	// Act
	let action = Action::custom("activate", true);

	// Assert
	assert!(action.detail, "custom detail-type action must be detail");
	match action.action_type {
		ActionType::Custom(name) => assert_eq!(name.as_ref(), "activate"),
		other => panic!("expected Custom variant, got {:?}", other),
	}
}

#[rstest]
fn action_from_name_standard_actions() {
	// Arrange
	let cases = [
		("list", false),
		("retrieve", true),
		("create", false),
		("update", true),
		("partial_update", true),
		("destroy", true),
	];

	for (name, expected_detail) in cases {
		// Act
		let action = Action::from_name(name);

		// Assert
		assert_eq!(
			action.detail, expected_detail,
			"action '{}' detail mismatch",
			name
		);
	}
}

#[rstest]
fn action_from_name_unknown_defaults_to_list() {
	// Act
	let action = Action::from_name("unknown_custom_action");

	// Assert
	assert!(!action.detail);
	match action.action_type {
		ActionType::Custom(name) => assert_eq!(name.as_ref(), "unknown_custom_action"),
		other => panic!("expected Custom variant, got {:?}", other),
	}
}

// ===========================================================================
// GenericViewSet basename tests
// ===========================================================================

#[rstest]
fn generic_viewset_basename() {
	// Act
	let viewset = GenericViewSet::new("articles", ());

	// Assert
	assert_eq!(viewset.get_basename(), "articles");
}

#[rstest]
fn generic_viewset_default_lookup_field() {
	// Act
	let viewset = GenericViewSet::new("articles", ());

	// Assert
	assert_eq!(viewset.get_lookup_field(), "id");
}

// ===========================================================================
// ModelViewSet routing tests
// ===========================================================================

#[rstest]
#[tokio::test]
async fn model_viewset_list_returns_ok() {
	// Arrange
	let viewset: ModelViewSet<TestModel, TestSerializer> = ModelViewSet::new("users");
	let request = make_request(Method::GET, "/users/");

	// Act
	let response = viewset.dispatch(request, Action::list()).await.unwrap();

	// Assert
	assert_eq!(response.status, StatusCode::OK);
	assert!(
		!response.body.is_empty(),
		"list should return a response body"
	);
}

#[rstest]
#[tokio::test]
async fn model_viewset_retrieve_returns_ok() {
	// Arrange
	let viewset: ModelViewSet<TestModel, TestSerializer> = ModelViewSet::new("users");
	let request = make_request(Method::GET, "/users/42/");

	// Act
	let response = viewset.dispatch(request, Action::retrieve()).await.unwrap();

	// Assert
	assert_eq!(response.status, StatusCode::OK);
	assert!(
		!response.body.is_empty(),
		"retrieve should return a response body"
	);
}

#[rstest]
#[tokio::test]
async fn model_viewset_create_returns_created() {
	// Arrange
	let viewset: ModelViewSet<TestModel, TestSerializer> = ModelViewSet::new("users");
	let request = make_request_with_body(Method::POST, "/users/", r#"{"name":"Alice"}"#);

	// Act
	let response = viewset.dispatch(request, Action::create()).await.unwrap();

	// Assert
	assert_eq!(response.status, StatusCode::CREATED);
	assert!(
		!response.body.is_empty(),
		"create should return a response body"
	);
}

#[rstest]
#[tokio::test]
async fn model_viewset_update_returns_ok() {
	// Arrange
	let viewset: ModelViewSet<TestModel, TestSerializer> = ModelViewSet::new("users");
	let request = make_request_with_body(Method::PUT, "/users/1/", r#"{"name":"Bob"}"#);

	// Act
	let response = viewset.dispatch(request, Action::update()).await.unwrap();

	// Assert
	assert_eq!(response.status, StatusCode::OK);
	assert!(
		!response.body.is_empty(),
		"update should return a response body"
	);
}

#[rstest]
#[tokio::test]
async fn model_viewset_partial_update_returns_ok() {
	// Arrange
	let viewset: ModelViewSet<TestModel, TestSerializer> = ModelViewSet::new("users");
	let request = make_request_with_body(Method::PATCH, "/users/1/", r#"{"name":"Carol"}"#);

	// Act
	let response = viewset
		.dispatch(request, Action::partial_update())
		.await
		.unwrap();

	// Assert
	assert_eq!(response.status, StatusCode::OK);
	assert!(
		!response.body.is_empty(),
		"partial_update should return a response body"
	);
}

#[rstest]
#[tokio::test]
async fn model_viewset_destroy_returns_no_content() {
	// Arrange
	let viewset: ModelViewSet<TestModel, TestSerializer> = ModelViewSet::new("users");
	let request = make_request(Method::DELETE, "/users/1/");

	// Act
	let response = viewset.dispatch(request, Action::destroy()).await.unwrap();

	// Assert
	assert_eq!(response.status, StatusCode::NO_CONTENT);
	assert!(
		response.body.is_empty(),
		"destroy should return empty body for 204"
	);
}

#[rstest]
fn model_viewset_default_lookup_field_is_id() {
	// Act
	let viewset: ModelViewSet<TestModel, TestSerializer> = ModelViewSet::new("users");

	// Assert
	assert_eq!(viewset.get_lookup_field(), "id");
}

#[rstest]
fn model_viewset_custom_lookup_field() {
	// Act
	let viewset: ModelViewSet<TestModel, TestSerializer> =
		ModelViewSet::new("users").with_lookup_field("username");

	// Assert
	assert_eq!(viewset.get_lookup_field(), "username");
}

// ===========================================================================
// ReadOnlyModelViewSet routing tests
// ===========================================================================

#[rstest]
#[tokio::test]
async fn readonly_viewset_list_allowed() {
	// Arrange
	let viewset: ReadOnlyModelViewSet<TestModel, TestSerializer> =
		ReadOnlyModelViewSet::new("posts");
	let request = make_request(Method::GET, "/posts/");

	// Act
	let response = viewset.dispatch(request, Action::list()).await.unwrap();

	// Assert
	assert_eq!(response.status, StatusCode::OK);
}

#[rstest]
#[tokio::test]
async fn readonly_viewset_retrieve_allowed() {
	// Arrange
	let viewset: ReadOnlyModelViewSet<TestModel, TestSerializer> =
		ReadOnlyModelViewSet::new("posts");
	let request = make_request(Method::GET, "/posts/7/");

	// Act
	let response = viewset.dispatch(request, Action::retrieve()).await.unwrap();

	// Assert
	assert_eq!(response.status, StatusCode::OK);
}

#[rstest]
#[tokio::test]
async fn readonly_viewset_create_is_denied() {
	// Arrange
	let viewset: ReadOnlyModelViewSet<TestModel, TestSerializer> =
		ReadOnlyModelViewSet::new("posts");
	let request = make_request_with_body(Method::POST, "/posts/", r#"{"title":"new post"}"#);

	// Act
	let result = viewset.dispatch(request, Action::create()).await;

	// Assert
	assert!(
		result.is_err(),
		"create must be rejected by ReadOnlyModelViewSet"
	);
}

#[rstest]
#[tokio::test]
async fn readonly_viewset_destroy_is_denied() {
	// Arrange
	let viewset: ReadOnlyModelViewSet<TestModel, TestSerializer> =
		ReadOnlyModelViewSet::new("posts");
	let request = make_request(Method::DELETE, "/posts/5/");

	// Act
	let result = viewset.dispatch(request, Action::destroy()).await;

	// Assert
	assert!(
		result.is_err(),
		"destroy must be rejected by ReadOnlyModelViewSet"
	);
}

// ===========================================================================
// ViewSetBuilder tests
// ===========================================================================

#[rstest]
fn viewset_builder_empty_actions_returns_error() {
	// Arrange
	let viewset: ModelViewSet<TestModel, TestSerializer> = ModelViewSet::new("items");

	// Act
	let result = viewset.as_view().build();

	// Assert
	assert!(result.is_err());
	let err = result.err().unwrap();
	let message = err.to_string();
	assert!(
		message.contains("actions"),
		"error message should mention 'actions', got: {message}"
	);
}

#[rstest]
fn viewset_builder_name_and_suffix_are_mutually_exclusive() {
	// Arrange
	let viewset: ModelViewSet<TestModel, TestSerializer> = ModelViewSet::new("items");

	// Act – add name first, then suffix (must fail)
	let result = viewset
		.as_view()
		.with_name("custom_name")
		.and_then(|b| b.with_suffix("Suffix"));

	// Assert
	assert!(result.is_err());
	let err = result.err().unwrap();
	let message = err.to_string();
	assert!(
		message.contains("mutually exclusive"),
		"error should mention mutual exclusivity, got: {message}"
	);
}

#[rstest]
fn viewset_builder_builds_successfully_with_actions() {
	// Arrange
	let viewset: ModelViewSet<TestModel, TestSerializer> = ModelViewSet::new("items");
	let mut actions = HashMap::new();
	actions.insert(Method::GET, "list".to_string());

	// Act
	let result = viewset.as_view().with_actions(actions).build();

	// Assert
	assert!(
		result.is_ok(),
		"builder must succeed when actions are provided"
	);
}

#[rstest]
fn viewset_actions_macro_creates_correct_mapping() {
	// Act
	let actions = viewset_actions!(GET => "list", POST => "create");

	// Assert
	assert_eq!(actions.get(&Method::GET).map(String::as_str), Some("list"));
	assert_eq!(
		actions.get(&Method::POST).map(String::as_str),
		Some("create")
	);
}

#[rstest]
fn viewset_builder_with_name_succeeds() {
	// Arrange
	let viewset: ModelViewSet<TestModel, TestSerializer> = ModelViewSet::new("items");
	let actions = viewset_actions!(GET => "list");

	// Act
	let result = viewset
		.as_view()
		.with_actions(actions)
		.with_name("item_list")
		.and_then(|b| b.build());

	// Assert
	assert!(result.is_ok());
}

#[rstest]
fn viewset_builder_with_suffix_succeeds() {
	// Arrange
	let viewset: ModelViewSet<TestModel, TestSerializer> = ModelViewSet::new("items");
	let actions = viewset_actions!(GET => "list");

	// Act
	let result = viewset
		.as_view()
		.with_actions(actions)
		.with_suffix("List")
		.and_then(|b| b.build());

	// Assert
	assert!(result.is_ok());
}

// ===========================================================================
// Pagination config tests
// ===========================================================================

#[rstest]
fn pagination_config_page_number() {
	// Act
	let config = PaginationConfig::page_number(20, Some(200));

	// Assert
	match config {
		PaginationConfig::PageNumber {
			page_size,
			max_page_size,
		} => {
			assert_eq!(page_size, 20);
			assert_eq!(max_page_size, Some(200));
		}
		other => panic!("expected PageNumber, got {:?}", other),
	}
}

#[rstest]
fn pagination_config_limit_offset() {
	// Act
	let config = PaginationConfig::limit_offset(25, Some(500));

	// Assert
	match config {
		PaginationConfig::LimitOffset {
			default_limit,
			max_limit,
		} => {
			assert_eq!(default_limit, 25);
			assert_eq!(max_limit, Some(500));
		}
		other => panic!("expected LimitOffset, got {:?}", other),
	}
}

#[rstest]
fn pagination_config_cursor() {
	// Act
	let config = PaginationConfig::cursor(50, "created_at");

	// Assert
	match config {
		PaginationConfig::Cursor {
			page_size,
			ordering_field,
		} => {
			assert_eq!(page_size, 50);
			assert_eq!(ordering_field, "created_at");
		}
		other => panic!("expected Cursor, got {:?}", other),
	}
}

#[rstest]
fn pagination_config_none() {
	// Act
	let config = PaginationConfig::none();

	// Assert
	assert!(matches!(config, PaginationConfig::None));
}

#[rstest]
fn model_viewset_without_pagination() {
	// Act
	let viewset: ModelViewSet<TestModel, TestSerializer> =
		ModelViewSet::new("items").without_pagination();

	// Assert
	assert_eq!(viewset.get_basename(), "items");
}

// ===========================================================================
// FilterConfig and OrderingConfig tests
// ===========================================================================

#[rstest]
fn filter_config_filterable_fields() {
	// Act
	let config = FilterConfig::new()
		.with_filterable_fields(vec!["status", "category"])
		.with_search_fields(vec!["title", "description"]);

	// Assert
	assert_eq!(config.filterable_fields, vec!["status", "category"]);
	assert_eq!(config.search_fields, vec!["title", "description"]);
	assert!(
		config.case_insensitive_search,
		"default must be case-insensitive"
	);
}

#[rstest]
fn filter_config_case_sensitive_override() {
	// Act
	let config = FilterConfig::new().case_insensitive(false);

	// Assert
	assert!(!config.case_insensitive_search);
}

#[rstest]
fn ordering_config_fields_and_default() {
	// Act
	let config = OrderingConfig::new()
		.with_ordering_fields(vec!["created_at", "title", "id"])
		.with_default_ordering(vec!["-created_at"]);

	// Assert
	assert_eq!(config.ordering_fields, vec!["created_at", "title", "id"]);
	assert_eq!(config.default_ordering, vec!["-created_at"]);
}

#[rstest]
fn model_viewset_with_filters_and_ordering() {
	// Act
	let viewset: ModelViewSet<TestModel, TestSerializer> = ModelViewSet::new("articles")
		.with_filters(
			FilterConfig::new()
				.with_filterable_fields(vec!["status"])
				.with_search_fields(vec!["title"]),
		)
		.with_ordering(
			OrderingConfig::new()
				.with_ordering_fields(vec!["created_at"])
				.with_default_ordering(vec!["-created_at"]),
		);

	// Assert
	assert_eq!(viewset.get_basename(), "articles");
}

// ===========================================================================
// Custom action registration tests
// ===========================================================================

#[rstest]
#[serial(viewset_registry)]
fn register_and_get_custom_action() {
	// Arrange
	let viewset_type = "test_register_get_custom::CustomViewSet";
	clear_actions();
	let metadata = ActionMetadata::new("activate").with_detail(true);

	// Act
	register_action(viewset_type, metadata);
	let actions = get_registered_actions(viewset_type);

	// Assert
	assert_eq!(actions.len(), 1);
	assert_eq!(actions[0].name, "activate");
	assert!(actions[0].detail);

	// Cleanup
	clear_actions();
}

#[rstest]
fn action_helper_creates_correct_metadata() {
	// Act
	let metadata = action("recent", false, |_req| async {
		reinhardt_http::Response::ok()
			.with_json(&serde_json::json!([]))
			.map_err(|e| reinhardt_core::exception::Error::Http(e.to_string()))
	});

	// Assert
	assert_eq!(metadata.name, "recent");
	assert!(!metadata.detail);
}

#[rstest]
fn action_metadata_display_name_default() {
	// Act
	let metadata = ActionMetadata::new("activate_user");

	// Assert
	assert_eq!(metadata.display_name(), "Activate User");
}

#[rstest]
fn action_metadata_custom_name_takes_priority() {
	// Act
	let metadata = ActionMetadata::new("activate_user").with_custom_name("Activate Now");

	// Assert
	assert_eq!(metadata.display_name(), "Activate Now");
}

#[rstest]
fn action_metadata_url_name_replaces_underscores() {
	// Act
	let metadata = ActionMetadata::new("recent_posts");

	// Assert
	assert_eq!(metadata.get_url_name(), "recent-posts");
}

#[rstest]
fn action_metadata_custom_url_name() {
	// Act
	let metadata = ActionMetadata::new("recent_posts").with_url_name("latest");

	// Assert
	assert_eq!(metadata.get_url_name(), "latest");
}

#[rstest]
fn action_metadata_url_path_default() {
	// Act
	let metadata = ActionMetadata::new("bulk_delete");

	// Assert
	assert_eq!(metadata.get_url_path(), "bulk-delete");
}

#[rstest]
fn action_metadata_custom_url_path() {
	// Act
	let metadata = ActionMetadata::new("bulk_delete").with_url_path("bulk/delete");

	// Assert
	assert_eq!(metadata.get_url_path(), "bulk/delete");
}

// ===========================================================================
// Nested resource URL generation tests
// ===========================================================================

#[rstest]
fn nested_url_helper() {
	// Act
	let url = nested_url("users", "42", "posts");

	// Assert
	assert_eq!(url, "users/42/posts/");
}

#[rstest]
fn nested_detail_url_helper() {
	// Act
	let url = nested_detail_url("users", "42", "posts", "7");

	// Assert
	assert_eq!(url, "users/42/posts/7/");
}

#[rstest]
fn nested_resource_path_single_segment() {
	// Act
	let path = NestedResourcePath::new().add_segment("users", "user_id");

	// Assert
	assert_eq!(path.build_url(), "users/{user_id}/");
	assert_eq!(path.build_list_url(), "users/");
}

#[rstest]
fn nested_resource_path_two_segments() {
	// Act
	let path = NestedResourcePath::new()
		.add_segment("users", "user_id")
		.add_segment("posts", "post_id");

	// Assert
	assert_eq!(path.build_url(), "users/{user_id}/posts/{post_id}/");
	assert_eq!(path.build_list_url(), "users/posts/");
}

#[rstest]
fn nested_resource_creation() {
	// Act
	let nested = NestedResource::new("user", "user_id", "author_id");

	// Assert
	assert_eq!(nested.parent, "user");
	assert_eq!(nested.parent_id_param, "user_id");
	assert_eq!(nested.lookup_field, "author_id");
}

#[rstest]
fn nested_viewset_get_parent_id_from_request() {
	// Arrange
	let inner: ModelViewSet<TestModel, TestSerializer> = ModelViewSet::new("comments");
	let config = NestedResource::new("posts", "post_id", "post_id");
	let nested_viewset = NestedViewSet::new(inner, config);
	let request = make_request(Method::GET, "/posts/99/comments/");

	// Act
	let parent_id = nested_viewset.get_parent_id(&request);

	// Assert
	assert_eq!(parent_id, Some("99".to_string()));
}

#[rstest]
fn nested_resource_path_extract_parent_ids() {
	// Arrange
	let path = NestedResourcePath::new()
		.add_segment("users", "user_id")
		.add_segment("posts", "post_id");
	let request = make_request(Method::GET, "/users/10/posts/20/");

	// Act
	let ids = path.extract_parent_ids(&request);

	// Assert
	assert_eq!(ids.get("user_id"), Some(&"10".to_string()));
	assert_eq!(ids.get("post_id"), Some(&"20".to_string()));
}

#[rstest]
fn nested_resource_path_empty_returns_root_list_url() {
	// Act
	let path = NestedResourcePath::new();

	// Assert
	assert_eq!(path.build_list_url(), "/");
}

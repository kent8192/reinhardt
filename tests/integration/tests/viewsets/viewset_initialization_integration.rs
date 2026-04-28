use bytes::Bytes;
use hyper::{HeaderMap, Method, StatusCode, Version};
use reinhardt_http::Request;
use reinhardt_macros::model;
use reinhardt_views::viewsets::{
	Action, GenericViewSet, ModelViewSet, ReadOnlyModelViewSet, ViewSet,
};
use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[model(table_name = "test_models")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestModel {
	#[field(primary_key = true)]
	id: i64,
	#[field(max_length = 255)]
	name: String,
}

#[derive(Debug, Clone)]
struct TestSerializer;

/// Test initializing viewset with action mapping
#[tokio::test]
async fn test_initialize_view_set_with_actions() {
	let viewset: ModelViewSet<TestModel, TestSerializer> = ModelViewSet::new("test");
	let request = Request::builder()
		.method(Method::GET)
		.uri("/test/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();
	let action = Action::list();

	let response = viewset.dispatch(request, action).await;

	// Should successfully handle the request
	assert!(response.is_ok(), "ViewSet dispatch should succeed");
	let resp = response.unwrap();
	assert_eq!(
		resp.status,
		StatusCode::OK,
		"List action should return 200 OK"
	);

	// Verify JSON response structure
	let json_value: serde_json::Value =
		serde_json::from_slice(&resp.body).expect("Response should be valid JSON");
	assert!(
		json_value.is_array(),
		"List response should be an array, but got: {:?}",
		json_value
	);
}

/// Test HEAD request handling with viewset
#[tokio::test]
async fn test_head_request_against_viewset() {
	let viewset: ModelViewSet<TestModel, TestSerializer> = ModelViewSet::new("test");
	let request = Request::builder()
		.method(Method::HEAD)
		.uri("/test/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();
	let action = Action::list();

	// HEAD requests should be handled (or properly rejected)
	let result = viewset.dispatch(request, action).await;

	// Currently this will fail with "Method not allowed" which is correct behavior
	// for a ModelViewSet that doesn't explicitly handle HEAD
	assert!(result.is_err());
}

/// Test viewset action attribute is set correctly
#[tokio::test]
async fn test_viewset_action_attr() {
	let viewset: ModelViewSet<TestModel, TestSerializer> = ModelViewSet::new("test");

	// Verify basename is set correctly (this is the closest we have to action tracking)
	assert_eq!(viewset.get_basename(), "test");
}

/// Test viewset has correct basename
#[tokio::test]
async fn test_viewset_get_basename() {
	let viewset = GenericViewSet::new("users", ());
	assert_eq!(viewset.get_basename(), "users");

	let model_viewset: ModelViewSet<TestModel, TestSerializer> = ModelViewSet::new("posts");
	assert_eq!(model_viewset.get_basename(), "posts");

	let readonly_viewset: ReadOnlyModelViewSet<TestModel, TestSerializer> =
		ReadOnlyModelViewSet::new("comments");
	assert_eq!(readonly_viewset.get_basename(), "comments");
}

/// Test different action types.
///
/// Provides an in-memory queryset and populates `path_params` so detail-route
/// actions resolve correctly through the embedded `ModelViewSetHandler`
/// (issue #3985).
#[tokio::test]
async fn test_viewset_action_types() {
	use std::collections::HashMap;
	let viewset: ModelViewSet<TestModel, TestSerializer> = ModelViewSet::new("items")
		.with_queryset(vec![TestModel {
			id: 1,
			name: "alpha".into(),
		}]);

	// Test list action
	let list_request = Request::builder()
		.method(Method::GET)
		.uri("/items/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();
	let list_response = viewset.dispatch(list_request, Action::list()).await;
	assert!(list_response.is_ok(), "List action should succeed");
	let list_resp = list_response.unwrap();
	assert_eq!(
		list_resp.status,
		StatusCode::OK,
		"List should return 200 OK"
	);

	// Verify JSON response structure
	let json_value: serde_json::Value =
		serde_json::from_slice(&list_resp.body).expect("List response should be valid JSON");
	assert!(
		json_value.is_array(),
		"List response should be an array, but got: {:?}",
		json_value
	);

	// Test retrieve action
	let mut params = HashMap::new();
	params.insert("id".to_string(), "1".to_string());
	let retrieve_request = Request::builder()
		.method(Method::GET)
		.uri("/items/1/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.path_params(params.clone())
		.build()
		.unwrap();
	let retrieve_response = viewset.dispatch(retrieve_request, Action::retrieve()).await;
	assert!(retrieve_response.is_ok(), "Retrieve action should succeed");
	let retrieve_resp = retrieve_response.unwrap();
	assert_eq!(
		retrieve_resp.status,
		StatusCode::OK,
		"Retrieve should return 200 OK"
	);

	// Verify JSON response structure
	let json_value: serde_json::Value = serde_json::from_slice(&retrieve_resp.body)
		.expect("Retrieve response should be valid JSON");
	assert!(
		json_value.is_object(),
		"Retrieve response should be an object, but got: {:?}",
		json_value
	);

	// Test create action
	let create_request = Request::builder()
		.method(Method::POST)
		.uri("/items/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::from(r#"{"id": 2, "name": "test"}"#))
		.build()
		.unwrap();
	let create_response = viewset.dispatch(create_request, Action::create()).await;
	assert!(create_response.is_ok(), "Create action should succeed");
	let create_resp = create_response.unwrap();
	assert_eq!(
		create_resp.status,
		StatusCode::CREATED,
		"Create should return 201 Created"
	);

	// Verify JSON response structure
	let json_value: serde_json::Value =
		serde_json::from_slice(&create_resp.body).expect("Create response should be valid JSON");
	assert!(
		json_value.is_object(),
		"Create response should be an object, but got: {:?}",
		json_value
	);

	// Test update action
	let update_request = Request::builder()
		.method(Method::PUT)
		.uri("/items/1/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::from(r#"{"id": 1, "name": "updated"}"#))
		.path_params(params.clone())
		.build()
		.unwrap();
	let update_response = viewset.dispatch(update_request, Action::update()).await;
	assert!(update_response.is_ok(), "Update action should succeed");
	let update_resp = update_response.unwrap();
	assert_eq!(
		update_resp.status,
		StatusCode::OK,
		"Update should return 200 OK"
	);

	// Verify JSON response structure
	let json_value: serde_json::Value =
		serde_json::from_slice(&update_resp.body).expect("Update response should be valid JSON");
	assert!(
		json_value.is_object(),
		"Update response should be an object, but got: {:?}",
		json_value
	);

	// Test destroy action
	let destroy_request = Request::builder()
		.method(Method::DELETE)
		.uri("/items/1/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.path_params(params)
		.build()
		.unwrap();
	let destroy_response = viewset.dispatch(destroy_request, Action::destroy()).await;
	assert!(destroy_response.is_ok(), "Destroy action should succeed");
	let destroy_resp = destroy_response.unwrap();
	assert_eq!(
		destroy_resp.status,
		StatusCode::NO_CONTENT,
		"Destroy should return 204 No Content"
	);

	// Verify no content in body for destroy action
	assert!(
		destroy_resp.body.is_empty(),
		"Destroy response should have empty body, but got {} bytes: {:?}",
		destroy_resp.body.len(),
		destroy_resp.body
	);
}

/// Test readonly viewset restrictions.
///
/// Provides an in-memory queryset and populates `path_params` so that
/// retrieve actually resolves to a row (issue #3985: dispatch now flows
/// through the real `ModelViewSetHandler`).
#[tokio::test]
async fn test_readonly_viewset_restrictions() {
	use std::collections::HashMap;
	let viewset: ReadOnlyModelViewSet<TestModel, TestSerializer> =
		ReadOnlyModelViewSet::new("readonly").with_queryset(vec![TestModel {
			id: 1,
			name: "alpha".into(),
		}]);

	// List should work
	let list_request = Request::builder()
		.method(Method::GET)
		.uri("/readonly/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();
	let list_response = viewset.dispatch(list_request, Action::list()).await;
	assert!(
		list_response.is_ok(),
		"List action should succeed on readonly ViewSet"
	);
	let list_resp = list_response.unwrap();
	assert_eq!(
		list_resp.status,
		StatusCode::OK,
		"List should return 200 OK"
	);

	// Verify JSON response structure
	let json_value: serde_json::Value =
		serde_json::from_slice(&list_resp.body).expect("List response should be valid JSON");
	assert!(
		json_value.is_array(),
		"List response should be an array, but got: {:?}",
		json_value
	);

	// Retrieve should work — must populate path_params so dispatch can resolve pk.
	let mut path_params = HashMap::new();
	path_params.insert("id".to_string(), "1".to_string());
	let retrieve_request = Request::builder()
		.method(Method::GET)
		.uri("/readonly/1/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.path_params(path_params)
		.build()
		.unwrap();
	let retrieve_response = viewset.dispatch(retrieve_request, Action::retrieve()).await;
	assert!(
		retrieve_response.is_ok(),
		"Retrieve action should succeed on readonly ViewSet"
	);
	let retrieve_resp = retrieve_response.unwrap();
	assert_eq!(
		retrieve_resp.status,
		StatusCode::OK,
		"Retrieve should return 200 OK"
	);

	// Verify JSON response structure
	let json_value: serde_json::Value = serde_json::from_slice(&retrieve_resp.body)
		.expect("Retrieve response should be valid JSON");
	assert!(
		json_value.is_object(),
		"Retrieve response should be an object, but got: {:?}",
		json_value
	);

	// Create should fail
	let create_request = Request::builder()
		.method(Method::POST)
		.uri("/readonly/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::from(r#"{"name": "test"}"#))
		.build()
		.unwrap();
	let create_response = viewset.dispatch(create_request, Action::create()).await;
	assert!(
		create_response.is_err(),
		"Create action should fail on readonly ViewSet"
	);

	// Delete should fail
	let delete_request = Request::builder()
		.method(Method::DELETE)
		.uri("/readonly/1/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();
	let delete_response = viewset.dispatch(delete_request, Action::destroy()).await;
	assert!(
		delete_response.is_err(),
		"Destroy action should fail on readonly ViewSet"
	);
}

/// Test custom actions
#[tokio::test]
async fn test_custom_actions() {
	let list_action = Action::custom("custom_list", false);
	assert!(!list_action.detail);

	let detail_action = Action::custom("custom_detail", true);
	assert!(detail_action.detail);
}

/// Test action metadata
#[tokio::test]
async fn test_action_metadata() {
	let list = Action::list();
	assert!(!list.detail);

	let retrieve = Action::retrieve();
	assert!(retrieve.detail);

	let create = Action::create();
	assert!(!create.detail);

	let update = Action::update();
	assert!(update.detail);

	let partial_update = Action::partial_update();
	assert!(partial_update.detail);

	let destroy = Action::destroy();
	assert!(destroy.detail);
}

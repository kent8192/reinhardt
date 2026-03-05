use async_trait::async_trait;
use bytes::Bytes;
use hyper::{HeaderMap, Method, StatusCode, Version};
use reinhardt_http::{Request, Response};
use reinhardt_views::viewsets::{
	ActionMetadata, FunctionActionHandler, ViewSet, action, clear_actions, register_action,
};
use serial_test::serial;
use std::collections::HashSet;

#[derive(Debug, Clone)]
struct TestViewSet {
	basename: String,
}

impl TestViewSet {
	fn new(basename: impl Into<String>) -> Self {
		Self {
			basename: basename.into(),
		}
	}
}

#[async_trait]
impl ViewSet for TestViewSet {
	fn get_basename(&self) -> &str {
		&self.basename
	}

	async fn dispatch(
		&self,
		_request: Request,
		_action: reinhardt_views::viewsets::Action,
	) -> reinhardt_http::Result<Response> {
		Response::ok().with_json(&serde_json::json!({"test": true}))
	}
}

/// Test manual action registration with proper registry cleanup
#[tokio::test]
#[serial(action_registry)]
async fn test_manual_action_registration() {
	// Clear registry before test
	clear_actions();

	// Register actions manually
	let viewset_type = std::any::type_name::<TestViewSet>();

	let action1 = ActionMetadata::new("custom_list")
		.with_detail(false)
		.with_handler(FunctionActionHandler::new(|_req| {
			Box::pin(async { Response::ok().with_json(&serde_json::json!({"action": "list"})) })
		}));

	let action2 = ActionMetadata::new("custom_detail")
		.with_detail(true)
		.with_custom_name("Custom Detail Action")
		.with_handler(FunctionActionHandler::new(|_req| {
			Box::pin(async { Response::ok().with_json(&serde_json::json!({"action": "detail"})) })
		}));

	register_action(viewset_type, action1);
	register_action(viewset_type, action2);

	// Get actions via ViewSet trait
	let viewset = TestViewSet::new("test");
	let actions = viewset.get_extra_actions();

	assert_eq!(actions.len(), 2, "Should have exactly 2 registered actions");

	// Use HashSet for order-independent comparison
	let actual_names: HashSet<String> = actions.iter().map(|a| a.name.clone()).collect();
	let expected_names: HashSet<String> = ["custom_list", "custom_detail"]
		.iter()
		.map(|s| s.to_string())
		.collect();
	assert_eq!(
		actual_names, expected_names,
		"Action name does not match expected value. Expected: {:?}, Actual: {:?}",
		expected_names, actual_names
	);

	// Cleanup after test
	clear_actions();
}

/// Test action metadata properties
#[tokio::test]
async fn test_action_metadata_properties() {
	let action = ActionMetadata::new("test_action")
		.with_detail(true)
		.with_custom_name("My Custom Action")
		.with_suffix("Extra")
		.with_url_path("custom/path")
		.with_url_name("custom-action");

	assert_eq!(action.name, "test_action");
	assert!(action.detail);
	assert_eq!(action.custom_name, Some("My Custom Action".to_string()));
	assert_eq!(action.display_name(), "My Custom Action"); // custom_name takes precedence
	assert_eq!(action.get_url_path(), "custom/path");
	assert_eq!(action.get_url_name(), "custom-action");
}

/// Test display name formatting
#[tokio::test]
async fn test_display_name_formatting() {
	// Without custom_name or suffix - should format snake_case
	let action1 = ActionMetadata::new("list_all_items");
	assert_eq!(action1.display_name(), "List All Items");

	// With custom_name - should use it
	let action2 = ActionMetadata::new("list_all_items").with_custom_name("All Items");
	assert_eq!(action2.display_name(), "All Items");

	// With suffix - should append it
	let action3 = ActionMetadata::new("list_items").with_suffix("Custom");
	assert_eq!(action3.display_name(), "List Items Custom");
}

/// Test action helper function
#[tokio::test]
async fn test_action_helper() {
	let my_action = action("test", false, |_req| async {
		Response::ok().with_json(&serde_json::json!({"status": "ok"}))
	});

	assert_eq!(my_action.name, "test");
	assert!(!my_action.detail);

	// Test handler execution
	let request = Request::builder()
		.method(Method::GET)
		.uri("/test/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = my_action.handler.handle(request).await;
	assert!(response.is_ok());
	assert_eq!(response.unwrap().status, StatusCode::OK);
}

/// Test ViewSet without actions - registry should be empty after clear
#[tokio::test]
#[serial(action_registry)]
async fn test_viewset_no_actions() {
	// Clear registry to ensure isolation from other tests
	clear_actions();

	let viewset = TestViewSet::new("empty");
	let actions = viewset.get_extra_actions();

	// After clearing, there should be no actions registered
	assert_eq!(
		actions.len(),
		0,
		"After clearing registry, ViewSet should have no extra actions"
	);
}

/// Test that clearing actions works correctly
#[tokio::test]
#[serial(action_registry)]
async fn test_clear_actions_functionality() {
	// Start clean
	clear_actions();

	let viewset_type = std::any::type_name::<TestViewSet>();

	// Register some actions
	register_action(
		viewset_type,
		ActionMetadata::new("action1").with_handler(FunctionActionHandler::new(|_req| {
			Box::pin(async { Ok(Response::ok()) })
		})),
	);
	register_action(
		viewset_type,
		ActionMetadata::new("action2").with_handler(FunctionActionHandler::new(|_req| {
			Box::pin(async { Ok(Response::ok()) })
		})),
	);

	// Verify actions are registered
	let viewset = TestViewSet::new("test");
	let actions_before = viewset.get_extra_actions();
	assert_eq!(
		actions_before.len(),
		2,
		"Should have 2 actions before clear"
	);

	// Clear all actions
	clear_actions();

	// Verify actions are cleared
	let actions_after = viewset.get_extra_actions();
	assert_eq!(actions_after.len(), 0, "Should have 0 actions after clear");
}

/// Test get_extra_action_url_map (should return empty for base ViewSet)
#[tokio::test]
async fn test_uninitialized_view_url_map() {
	let viewset = TestViewSet::new("test");
	let url_map = viewset.get_extra_action_url_map();

	// Uninitialized ViewSet should return empty map
	assert_eq!(url_map.len(), 0);
}

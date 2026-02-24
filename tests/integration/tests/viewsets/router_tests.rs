use bytes::Bytes;
use hyper::{HeaderMap, Method, StatusCode, Uri, Version};
use reinhardt_apps::{Handler, Request};
use reinhardt_urls::routers::{DefaultRouter, Router};
use reinhardt_views::viewsets::{GenericViewSet, ModelViewSet, ViewSet};
use std::sync::Arc;

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct TestModel {
	id: i64,
	name: String,
}

#[derive(Debug, Clone)]
struct TestSerializer;

/// Test viewset registration with router
#[tokio::test]
async fn test_register_viewset_with_router() {
	let mut router = DefaultRouter::new();
	let viewset: Arc<ModelViewSet<TestModel, TestSerializer>> = Arc::new(ModelViewSet::new("test"));

	router.register_viewset("test", viewset);

	// Test list endpoint is accessible
	let list_request = Request::builder()
		.method(Method::GET)
		.uri("/test/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = router.route(list_request).await;
	assert!(response.is_ok());
	assert_eq!(response.unwrap().status, StatusCode::OK);
}

/// Test viewset detail endpoint with router
#[tokio::test]
async fn test_viewset_detail_endpoint_with_router() {
	let mut router = DefaultRouter::new();
	let viewset: Arc<ModelViewSet<TestModel, TestSerializer>> =
		Arc::new(ModelViewSet::new("items"));

	router.register_viewset("items", viewset);

	// Test detail endpoint
	let detail_request = Request::builder()
		.method(Method::GET)
		.uri("/items/1/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = router.route(detail_request).await;
	assert!(response.is_ok());
	assert_eq!(response.unwrap().status, StatusCode::OK);
}

/// Test URL reversing with router (default basename)
#[tokio::test]
async fn test_reverse_action_default_basename() {
	let mut router = DefaultRouter::new();
	let viewset: Arc<ModelViewSet<TestModel, TestSerializer>> =
		Arc::new(ModelViewSet::new("users"));

	router.register_viewset("users", viewset);

	// Test reversing list action
	let list_url = router.reverse("users-list", &std::collections::HashMap::new());
	assert!(list_url.is_ok());
	assert_eq!(list_url.unwrap(), "/users/");

	// Test reversing detail action
	let mut detail_params = std::collections::HashMap::new();
	detail_params.insert("id".to_string(), "123".to_string());
	let detail_url = router.reverse("users-detail", &detail_params);
	assert!(detail_url.is_ok());
	assert_eq!(detail_url.unwrap(), "/users/123/");
}

/// Test URL reversing with custom basename
#[tokio::test]
async fn test_reverse_action_custom_basename() {
	let mut router = DefaultRouter::new();

	// Create a custom viewset with different basename
	let custom_viewset: Arc<GenericViewSet<()>> = Arc::new(GenericViewSet::new("custom", ()));

	router.register_viewset("api/items", custom_viewset);

	// Test reversing with custom basename
	let list_url = router.reverse("custom-list", &std::collections::HashMap::new());
	assert!(list_url.is_ok());
	assert_eq!(list_url.unwrap(), "/api/items/");
}

/// Test multiple viewset registration
#[tokio::test]
async fn test_multiple_viewset_registration() {
	let mut router = DefaultRouter::new();

	let users_viewset: Arc<ModelViewSet<TestModel, TestSerializer>> =
		Arc::new(ModelViewSet::new("users"));
	let posts_viewset: Arc<ModelViewSet<TestModel, TestSerializer>> =
		Arc::new(ModelViewSet::new("posts"));

	router.register_viewset("users", users_viewset);
	router.register_viewset("posts", posts_viewset);

	// Test users endpoint
	let users_request = Request::builder()
		.method(Method::GET)
		.uri("/users/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();
	let users_response = router.route(users_request).await;
	assert!(users_response.is_ok());

	// Test posts endpoint
	let posts_request = Request::builder()
		.method(Method::GET)
		.uri("/posts/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();
	let posts_response = router.route(posts_request).await;
	assert!(posts_response.is_ok());
}

/// Test viewset with router handles different HTTP methods
#[tokio::test]
async fn test_viewset_router_http_methods() {
	let mut router = DefaultRouter::new();
	let viewset: Arc<ModelViewSet<TestModel, TestSerializer>> =
		Arc::new(ModelViewSet::new("resources"));

	router.register_viewset("resources", viewset);

	// GET (list)
	let get_request = Request::builder()
		.method(Method::GET)
		.uri("/resources/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();
	let get_response = router.route(get_request).await;
	assert!(get_response.is_ok());
	assert_eq!(get_response.unwrap().status, StatusCode::OK);

	// POST (create)
	let post_request = Request::builder()
		.method(Method::POST)
		.uri("/resources/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::from(r#"{"name": "test"}"#))
		.build()
		.unwrap();
	let post_response = router.route(post_request).await;
	assert!(post_response.is_ok());
	assert_eq!(post_response.unwrap().status, StatusCode::CREATED);

	// PUT (update)
	let put_request = Request::builder()
		.method(Method::PUT)
		.uri("/resources/1/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::from(r#"{"name": "updated"}"#))
		.build()
		.unwrap();
	let put_response = router.route(put_request).await;
	assert!(put_response.is_ok());
	assert_eq!(put_response.unwrap().status, StatusCode::OK);

	// DELETE (destroy)
	let delete_request = Request::builder()
		.method(Method::DELETE)
		.uri("/resources/1/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();
	let delete_response = router.route(delete_request).await;
	assert!(delete_response.is_ok());
	assert_eq!(delete_response.unwrap().status, StatusCode::NO_CONTENT);
}

/// Test extra actions (custom actions) with router
#[tokio::test]
async fn test_extra_actions_with_router() {
	use async_trait::async_trait;
	use reinhardt_views::viewsets::{ActionMetadata, FunctionActionHandler, register_action};

	// Define a custom ViewSet type
	#[derive(Debug, Clone)]
	struct CustomViewSet {
		basename: String,
	}

	#[async_trait]
	impl ViewSet for CustomViewSet {
		fn get_basename(&self) -> &str {
			&self.basename
		}

		async fn dispatch(
			&self,
			_request: Request,
			_action: reinhardt_views::viewsets::Action,
		) -> reinhardt_apps::Result<reinhardt_apps::Response> {
			reinhardt_apps::Response::ok().with_json(&serde_json::json!({"test": true}))
		}
	}

	// Register custom actions manually
	let viewset_type = std::any::type_name::<CustomViewSet>();

	let list_action = ActionMetadata::new("custom_list")
		.with_detail(false)
		.with_handler(FunctionActionHandler::new(|_req| {
			Box::pin(async {
				reinhardt_apps::Response::ok()
					.with_json(&serde_json::json!({"action": "custom_list"}))
			})
		}));

	let detail_action = ActionMetadata::new("custom_detail")
		.with_detail(true)
		.with_handler(FunctionActionHandler::new(|_req| {
			Box::pin(async {
				reinhardt_apps::Response::ok()
					.with_json(&serde_json::json!({"action": "custom_detail"}))
			})
		}));

	register_action(viewset_type, list_action);
	register_action(viewset_type, detail_action);

	// Register ViewSet with router
	let viewset = Arc::new(CustomViewSet {
		basename: "custom".to_string(),
	});
	let mut router = DefaultRouter::new();
	router.register_viewset("custom", viewset);

	// Router should now have 4 routes: list, detail, custom_list, custom_detail
	assert_eq!(router.get_routes().len(), 4);

	// Test custom list action endpoint
	let custom_list_request = Request::builder()
		.method(Method::GET)
		.uri("/custom/custom_list/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = router.route(custom_list_request).await;
	assert!(response.is_ok(), "Custom list action should be accessible");
	assert_eq!(response.unwrap().status, StatusCode::OK);

	// Test custom detail action endpoint
	let custom_detail_request = Request::builder()
		.method(Method::GET)
		.uri("/custom/1/custom_detail/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = router.route(custom_detail_request).await;
	assert!(
		response.is_ok(),
		"Custom detail action should be accessible"
	);
	assert_eq!(response.unwrap().status, StatusCode::OK);
}

/// Test get_extra_actions method
#[tokio::test]
async fn test_get_extra_actions() {
	use async_trait::async_trait;
	use reinhardt_views::viewsets::{ActionMetadata, FunctionActionHandler, register_action};

	// Define a custom ViewSet type
	#[derive(Debug, Clone)]
	struct CustomViewSet {
		basename: String,
	}

	#[async_trait]
	impl ViewSet for CustomViewSet {
		fn get_basename(&self) -> &str {
			&self.basename
		}

		async fn dispatch(
			&self,
			_request: Request,
			_action: reinhardt_views::viewsets::Action,
		) -> reinhardt_apps::Result<reinhardt_apps::Response> {
			reinhardt_apps::Response::ok().with_json(&serde_json::json!({"test": true}))
		}
	}

	// Register custom actions manually
	let viewset_type = std::any::type_name::<CustomViewSet>();

	let action1 = ActionMetadata::new("custom_list_action")
		.with_detail(false)
		.with_handler(FunctionActionHandler::new(|_req| {
			Box::pin(async {
				reinhardt_apps::Response::ok().with_json(&serde_json::json!({"action": "list"}))
			})
		}));

	let action2 = ActionMetadata::new("custom_detail_action")
		.with_detail(true)
		.with_handler(FunctionActionHandler::new(|_req| {
			Box::pin(async {
				reinhardt_apps::Response::ok().with_json(&serde_json::json!({"action": "detail"}))
			})
		}));

	register_action(viewset_type, action1);
	register_action(viewset_type, action2);

	// Test get_extra_actions
	let viewset = CustomViewSet {
		basename: "test".to_string(),
	};
	let extra_actions = viewset.get_extra_actions();

	assert_eq!(extra_actions.len(), 2);
	assert!(extra_actions.iter().any(|a| a.name == "custom_list_action"));
	assert!(
		extra_actions
			.iter()
			.any(|a| a.name == "custom_detail_action")
	);
}

/// Test extra action URL map generation
#[tokio::test]
async fn test_extra_action_url_map() {
	use async_trait::async_trait;
	use reinhardt_views::viewsets::{ActionMetadata, FunctionActionHandler, register_action};

	// Define a custom ViewSet type
	#[derive(Debug, Clone)]
	struct CustomViewSet {
		basename: String,
	}

	#[async_trait]
	impl ViewSet for CustomViewSet {
		fn get_basename(&self) -> &str {
			&self.basename
		}

		async fn dispatch(
			&self,
			_request: Request,
			_action: reinhardt_views::viewsets::Action,
		) -> reinhardt_apps::Result<reinhardt_apps::Response> {
			reinhardt_apps::Response::ok().with_json(&serde_json::json!({"test": true}))
		}
	}

	// Register custom actions
	let viewset_type = std::any::type_name::<CustomViewSet>();

	let list_action = ActionMetadata::new("custom_list")
		.with_detail(false)
		.with_handler(FunctionActionHandler::new(|_req| {
			Box::pin(async {
				reinhardt_apps::Response::ok()
					.with_json(&serde_json::json!({"action": "custom_list"}))
			})
		}));

	register_action(viewset_type, list_action);

	// Register ViewSet with router
	let viewset = Arc::new(CustomViewSet {
		basename: "test".to_string(),
	});
	let mut router = DefaultRouter::new();
	router.register_viewset("test", viewset.clone());

	// Get action URL map from router
	let url_map = router.get_action_url_map(viewset.as_ref(), "http://testserver");

	// Verify URL map contains custom action
	assert!(url_map.contains_key("custom_list"));
	assert_eq!(
		url_map.get("custom_list").unwrap(),
		"http://testserver/test/custom_list/"
	);
}

/// Test action names with name and suffix kwargs
#[tokio::test]
async fn test_action_names_with_kwargs() {
	use async_trait::async_trait;
	use reinhardt_views::viewsets::{ActionMetadata, FunctionActionHandler, register_action};

	// Define a custom ViewSet type
	#[derive(Debug, Clone)]
	struct CustomViewSet {
		basename: String,
	}

	#[async_trait]
	impl ViewSet for CustomViewSet {
		fn get_basename(&self) -> &str {
			&self.basename
		}

		async fn dispatch(
			&self,
			_request: Request,
			_action: reinhardt_views::viewsets::Action,
		) -> reinhardt_apps::Result<reinhardt_apps::Response> {
			reinhardt_apps::Response::ok().with_json(&serde_json::json!({"test": true}))
		}
	}

	// Register custom actions with name and suffix
	let viewset_type = std::any::type_name::<CustomViewSet>();

	let named_action = ActionMetadata::new("custom_action")
		.with_detail(true)
		.with_custom_name("Custom Name")
		.with_handler(FunctionActionHandler::new(|_req| {
			Box::pin(async {
				reinhardt_apps::Response::ok()
					.with_json(&serde_json::json!({"action": "named_action"}))
			})
		}));

	let suffixed_action = ActionMetadata::new("another_action")
		.with_detail(false)
		.with_suffix("Custom Suffix")
		.with_handler(FunctionActionHandler::new(|_req| {
			Box::pin(async {
				reinhardt_apps::Response::ok()
					.with_json(&serde_json::json!({"action": "suffixed_action"}))
			})
		}));

	register_action(viewset_type, named_action);
	register_action(viewset_type, suffixed_action);

	// Register ViewSet with router
	let viewset = Arc::new(CustomViewSet {
		basename: "test".to_string(),
	});
	let mut router = DefaultRouter::new();
	router.register_viewset("test", viewset.clone());

	// Test that custom name is used in URL generation
	let url_map = router.get_action_url_map(viewset.as_ref(), "http://testserver");

	// The action should be accessible by its custom name
	assert!(url_map.contains_key("custom_action"));
	assert_eq!(
		url_map.get("custom_action").unwrap(),
		"http://testserver/test/1/custom_action/"
	);

	// Test that suffixed action is accessible
	assert!(url_map.contains_key("another_action"));
	assert_eq!(
		url_map.get("another_action").unwrap(),
		"http://testserver/test/another_action/"
	);

	// Test that the actions are actually accessible via HTTP
	let named_request = Request::builder()
		.method(Method::GET)
		.uri("/test/1/custom_action/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = router.route(named_request).await;
	assert!(response.is_ok(), "Named action should be accessible");
	assert_eq!(response.unwrap().status, StatusCode::OK);

	let suffixed_request = Request::builder()
		.method(Method::GET)
		.uri("/test/another_action/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = router.route(suffixed_request).await;
	assert!(response.is_ok(), "Suffixed action should be accessible");
	assert_eq!(response.unwrap().status, StatusCode::OK);
}

/// Test as_view() pattern for viewset initialization
#[tokio::test]
async fn test_initialize_view_set_with_empty_actions() {
	// Test that as_view() without actions produces an error
	let viewset = GenericViewSet::new("test", ());
	let builder = viewset.as_view();
	let result = builder.build();

	assert!(result.is_err());
	if let Err(e) = result {
		let err_msg = e.to_string();
		assert_eq!(
			err_msg,
			"HTTP error: The `actions` argument must be provided when calling `.as_view()` on a ViewSet. For example `.as_view({'get': 'list'})`"
		);
	}
}

/// Test as_view() with both name and suffix raises error
#[tokio::test]
async fn test_initialize_view_set_with_both_name_and_suffix() {
	use reinhardt_views::viewset_actions;

	// Test that providing both name and suffix produces an error
	let viewset = GenericViewSet::new("test", ());
	let actions = viewset_actions!(GET => "list");

	let result = viewset
		.as_view()
		.with_actions(actions)
		.with_name("test_name")
		.and_then(|builder| builder.with_suffix("test_suffix"));

	assert!(result.is_err());
	if let Err(e) = result {
		let err_msg = e.to_string();
		// Exact error message from builder.rs
		assert_eq!(
			err_msg,
			"HTTP error: reinhardt_views::viewsets::viewset::GenericViewSet<()>() received both `name` and `suffix`, which are mutually exclusive arguments."
		);
	}
}

/// Test viewset has required attributes after as_view()
#[tokio::test]
async fn test_args_kwargs_request_action_map_on_self() {
	use reinhardt_views::viewset_actions;

	// Test that ViewSetHandler has the expected behavior:
	// - It can be called as a handler
	// - It has access to request, action_map, args, kwargs

	let viewset = GenericViewSet::new("test", ());
	let actions = viewset_actions!(GET => "list");
	let handler = viewset.as_view().with_actions(actions).build().unwrap();

	// Create a test request
	let request = Request::builder()
		.method(Method::GET)
		.uri(Uri::from_static("/test/"))
		.version(Version::HTTP_11)
		.body(Bytes::new())
		.build()
		.unwrap();

	// Handler should be able to process the request
	// This implicitly tests that args, kwargs, request, and action_map are being set
	let response = handler.handle(request).await;

	// We expect an error because GenericViewSet doesn't implement the list action
	// but the important thing is that the handler accepted and processed the request
	assert!(response.is_err());
	if let Err(e) = response {
		// Should get "Action not implemented" error, not a method routing error
		let err_msg = e.to_string();
		assert_eq!(err_msg, "Not found: Action not implemented");
	}
}

/// Test login_required middleware compatibility
#[tokio::test]
async fn test_login_required_middleware_compat() {
	use hyper::{Method, Version};
	use reinhardt_test::TestViewSet;
	use reinhardt_views::viewset_actions;

	// Test ViewSet without login required
	let viewset = TestViewSet::new("test");
	assert!(!viewset.requires_login());
	assert!(viewset.get_middleware().is_none());

	// Test ViewSet with login required
	let viewset_with_login = TestViewSet::new("test").with_login_required(true);
	assert!(viewset_with_login.requires_login());
	assert!(viewset_with_login.get_middleware().is_some());

	// Test ViewSet with permissions
	let permissions = vec!["read".to_string(), "write".to_string()];
	let viewset_with_permissions = TestViewSet::new("test").with_permissions(permissions.clone());
	assert_eq!(
		viewset_with_permissions.get_required_permissions(),
		permissions
	);
	assert!(viewset_with_permissions.get_middleware().is_some());

	// Test that middleware is properly integrated
	let viewset = TestViewSet::new("test").with_login_required(true);
	let actions = viewset_actions!(GET => "list");

	// Build the handler
	let result = viewset.as_view().with_actions(actions).build();
	let handler = result.unwrap();

	// Create a request without authentication
	let request = Request::builder()
		.method(Method::GET)
		.uri("/test/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(bytes::Bytes::new())
		.build()
		.unwrap();

	// Handler should return 401 due to authentication middleware
	let response = handler.handle(request).await;
	let response = response.unwrap();
	assert_eq!(response.status, hyper::StatusCode::UNAUTHORIZED);
}

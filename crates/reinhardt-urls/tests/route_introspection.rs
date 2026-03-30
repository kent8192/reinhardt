use hyper::Method;
use reinhardt_urls::routers::introspection::{RouteInfo, RouteInspector};
use rstest::rstest;
use std::collections::HashMap;

// ============================================================
// RouteInfo construction tests
// ============================================================

#[rstest]
fn route_info_basic_path_stored() {
	// Act
	let info = RouteInfo::new("/users/", vec![Method::GET], None::<String>);

	// Assert
	assert_eq!(info.path, "/users/");
}

#[rstest]
fn route_info_methods_stored_as_strings() {
	// Act
	let info = RouteInfo::new("/users/", vec![Method::GET, Method::POST], None::<String>);

	// Assert
	assert!(info.methods.iter().any(|m| m == "GET"));
	assert!(info.methods.iter().any(|m| m == "POST"));
}

#[rstest]
fn route_info_name_stored() {
	// Act
	let info = RouteInfo::new("/users/", vec![Method::GET], Some("api:users:list"));

	// Assert
	assert_eq!(info.name, Some("api:users:list".to_string()));
}

#[rstest]
fn route_info_no_name() {
	// Act
	let info = RouteInfo::new("/users/", vec![Method::GET], None::<String>);

	// Assert
	assert!(info.name.is_none());
	assert!(info.namespace.is_none());
	assert!(info.route_name.is_none());
}

#[rstest]
fn route_info_single_part_name_has_no_namespace() {
	// Act
	let info = RouteInfo::new("/users/", vec![Method::GET], Some("list"));

	// Assert
	assert!(info.namespace.is_none());
	assert_eq!(info.route_name, Some("list".to_string()));
}

#[rstest]
fn route_info_two_part_name_splits_namespace_and_route() {
	// Act
	let info = RouteInfo::new("/users/", vec![Method::GET], Some("users:list"));

	// Assert
	assert_eq!(info.namespace, Some("users".to_string()));
	assert_eq!(info.route_name, Some("list".to_string()));
}

#[rstest]
fn route_info_deep_namespace_splits_correctly() {
	// Act
	let info = RouteInfo::new(
		"/users/{id}/",
		vec![Method::GET],
		Some("api:v1:users:detail"),
	);

	// Assert
	assert_eq!(info.namespace, Some("api:v1:users".to_string()));
	assert_eq!(info.route_name, Some("detail".to_string()));
}

#[rstest]
fn route_info_params_extracted_from_path() {
	// Act
	let info = RouteInfo::new(
		"/users/{id}/posts/{post_id}/",
		vec![Method::GET],
		None::<String>,
	);

	// Assert
	assert!(info.params.iter().any(|p| p == "id"));
	assert!(info.params.iter().any(|p| p == "post_id"));
}

#[rstest]
fn route_info_no_params_for_static_path() {
	// Act
	let info = RouteInfo::new("/users/", vec![Method::GET], None::<String>);

	// Assert
	assert!(info.params.is_empty());
}

#[rstest]
fn route_info_metadata_empty_by_default() {
	// Act
	let info = RouteInfo::new("/users/", vec![Method::GET], None::<String>);

	// Assert
	assert!(info.metadata.is_empty());
}

// ============================================================
// RouteInfo::add_metadata tests
// ============================================================

#[rstest]
fn route_info_add_metadata_stores_key_value() {
	// Arrange
	let mut info = RouteInfo::new("/users/", vec![Method::GET], None::<String>);

	// Act
	info.add_metadata("description", "List all users");

	// Assert
	assert_eq!(
		info.metadata.get("description"),
		Some(&"List all users".to_string())
	);
}

#[rstest]
fn route_info_add_multiple_metadata_entries() {
	// Arrange
	let mut info = RouteInfo::new("/users/", vec![Method::GET], None::<String>);

	// Act
	info.add_metadata("description", "List all users");
	info.add_metadata("tags", "users,api");
	info.add_metadata("auth", "required");

	// Assert
	assert_eq!(info.metadata.len(), 3);
	assert_eq!(info.metadata.get("tags"), Some(&"users,api".to_string()));
	assert_eq!(info.metadata.get("auth"), Some(&"required".to_string()));
}

// ============================================================
// RouteInfo::supports_method tests
// ============================================================

#[rstest]
#[case(Method::GET, true)]
#[case(Method::POST, true)]
#[case(Method::DELETE, false)]
#[case(Method::PUT, false)]
fn route_info_supports_method(#[case] method: Method, #[case] expected: bool) {
	// Arrange
	let info = RouteInfo::new("/users/", vec![Method::GET, Method::POST], None::<String>);

	// Act
	let result = info.supports_method(&method);

	// Assert
	assert_eq!(result, expected);
}

// ============================================================
// RouteInspector creation tests
// ============================================================

#[rstest]
fn inspector_new_has_zero_routes() {
	// Act
	let inspector = RouteInspector::new();

	// Assert
	assert_eq!(inspector.route_count(), 0);
}

#[rstest]
fn inspector_default_has_zero_routes() {
	// Act
	let inspector = RouteInspector::default();

	// Assert
	assert_eq!(inspector.route_count(), 0);
}

#[rstest]
fn inspector_all_routes_empty_on_new() {
	// Act
	let inspector = RouteInspector::new();

	// Assert
	assert!(inspector.all_routes().is_empty());
}

// ============================================================
// RouteInspector::add_route tests
// ============================================================

#[rstest]
fn inspector_add_route_increments_count() {
	// Arrange
	let mut inspector = RouteInspector::new();

	// Act
	inspector.add_route("/users/", vec![Method::GET], None::<String>, None);

	// Assert
	assert_eq!(inspector.route_count(), 1);
}

#[rstest]
fn inspector_add_multiple_routes_counts_all() {
	// Arrange
	let mut inspector = RouteInspector::new();

	// Act
	inspector.add_route("/users/", vec![Method::GET], None::<String>, None);
	inspector.add_route("/posts/", vec![Method::POST], None::<String>, None);
	inspector.add_route("/comments/", vec![Method::DELETE], None::<String>, None);

	// Assert
	assert_eq!(inspector.route_count(), 3);
}

#[rstest]
fn inspector_add_route_with_metadata() {
	// Arrange
	let mut inspector = RouteInspector::new();
	let mut meta = HashMap::new();
	meta.insert("version".to_string(), "v1".to_string());

	// Act
	inspector.add_route("/users/", vec![Method::GET], Some("users:list"), Some(meta));

	// Assert
	let route = inspector.find_by_path("/users/").unwrap();
	assert_eq!(route.metadata.get("version"), Some(&"v1".to_string()));
}

// ============================================================
// RouteInspector::all_routes tests
// ============================================================

#[rstest]
fn inspector_all_routes_returns_all_registered() {
	// Arrange
	let mut inspector = RouteInspector::new();
	inspector.add_route("/users/", vec![Method::GET], None::<String>, None);
	inspector.add_route("/posts/", vec![Method::GET], None::<String>, None);

	// Act
	let routes = inspector.all_routes();

	// Assert
	assert_eq!(routes.len(), 2);
	let paths: Vec<&str> = routes.iter().map(|r| r.path.as_str()).collect();
	assert!(paths.contains(&"/users/"), "should contain /users/ route");
	assert!(paths.contains(&"/posts/"), "should contain /posts/ route");
}

// ============================================================
// RouteInspector::find_by_path tests
// ============================================================

#[rstest]
fn inspector_find_by_path_returns_correct_route() {
	// Arrange
	let mut inspector = RouteInspector::new();
	inspector.add_route("/users/", vec![Method::GET], Some("users:list"), None);

	// Act
	let route = inspector.find_by_path("/users/");

	// Assert
	assert!(route.is_some());
	assert_eq!(route.unwrap().path, "/users/");
}

#[rstest]
fn inspector_find_by_path_returns_none_for_unknown_path() {
	// Arrange
	let mut inspector = RouteInspector::new();
	inspector.add_route("/users/", vec![Method::GET], None::<String>, None);

	// Act
	let route = inspector.find_by_path("/nonexistent/");

	// Assert
	assert!(route.is_none());
}

#[rstest]
fn inspector_find_by_path_returns_none_on_empty_inspector() {
	// Arrange
	let inspector = RouteInspector::new();

	// Act
	let route = inspector.find_by_path("/users/");

	// Assert
	assert!(route.is_none());
}

// ============================================================
// RouteInspector::find_by_name tests
// ============================================================

#[rstest]
fn inspector_find_by_name_returns_correct_route() {
	// Arrange
	let mut inspector = RouteInspector::new();
	inspector.add_route("/users/", vec![Method::GET], Some("users:list"), None);

	// Act
	let route = inspector.find_by_name("users:list");

	// Assert
	assert!(route.is_some());
	assert_eq!(route.unwrap().path, "/users/");
}

#[rstest]
fn inspector_find_by_name_returns_none_for_unknown_name() {
	// Arrange
	let mut inspector = RouteInspector::new();
	inspector.add_route("/users/", vec![Method::GET], Some("users:list"), None);

	// Act
	let route = inspector.find_by_name("nonexistent:route");

	// Assert
	assert!(route.is_none());
}

#[rstest]
fn inspector_find_by_name_returns_none_for_unnamed_routes() {
	// Arrange
	let mut inspector = RouteInspector::new();
	inspector.add_route("/users/", vec![Method::GET], None::<String>, None);

	// Act
	let route = inspector.find_by_name("any:name");

	// Assert
	assert!(route.is_none());
}

// ============================================================
// RouteInspector::find_by_path_prefix tests
// ============================================================

#[rstest]
fn inspector_find_by_path_prefix_matches_multiple() {
	// Arrange
	let mut inspector = RouteInspector::new();
	inspector.add_route("/api/v1/users/", vec![Method::GET], None::<String>, None);
	inspector.add_route("/api/v1/posts/", vec![Method::GET], None::<String>, None);
	inspector.add_route("/api/v2/users/", vec![Method::GET], None::<String>, None);

	// Act
	let routes = inspector.find_by_path_prefix("/api/v1");

	// Assert
	assert_eq!(routes.len(), 2);
	assert!(
		routes.iter().all(|r| r.path.starts_with("/api/v1")),
		"all matched routes should have /api/v1 prefix"
	);
}

#[rstest]
fn inspector_find_by_path_prefix_returns_empty_for_no_match() {
	// Arrange
	let mut inspector = RouteInspector::new();
	inspector.add_route("/users/", vec![Method::GET], None::<String>, None);

	// Act
	let routes = inspector.find_by_path_prefix("/api");

	// Assert
	assert!(routes.is_empty());
}

#[rstest]
fn inspector_find_by_path_prefix_exact_match_included() {
	// Arrange
	let mut inspector = RouteInspector::new();
	inspector.add_route("/api/", vec![Method::GET], None::<String>, None);
	inspector.add_route("/api/users/", vec![Method::GET], None::<String>, None);

	// Act
	let routes = inspector.find_by_path_prefix("/api/");

	// Assert
	assert_eq!(routes.len(), 2);
	let paths: Vec<&str> = routes.iter().map(|r| r.path.as_str()).collect();
	assert!(
		paths.contains(&"/api/"),
		"should include exact prefix match"
	);
	assert!(paths.contains(&"/api/users/"), "should include sub-path");
}

#[rstest]
fn inspector_find_by_path_prefix_empty_inspector_returns_empty() {
	// Arrange
	let inspector = RouteInspector::new();

	// Act
	let routes = inspector.find_by_path_prefix("/api");

	// Assert
	assert!(routes.is_empty());
}

// ============================================================
// RouteInspector::find_by_namespace tests
// ============================================================

#[rstest]
fn inspector_find_by_namespace_exact_match() {
	// Arrange
	let mut inspector = RouteInspector::new();
	inspector.add_route(
		"/users/",
		vec![Method::GET],
		Some("api:v1:users:list"),
		None,
	);
	inspector.add_route(
		"/posts/",
		vec![Method::GET],
		Some("api:v1:posts:list"),
		None,
	);
	inspector.add_route(
		"/users/",
		vec![Method::GET],
		Some("api:v2:users:list"),
		None,
	);

	// Act
	let routes = inspector.find_by_namespace("api:v1");

	// Assert
	assert_eq!(routes.len(), 2);
	assert!(
		routes
			.iter()
			.all(|r| r.name.as_deref().unwrap_or("").starts_with("api:v1:")),
		"all matched routes should be in api:v1 namespace"
	);
}

#[rstest]
fn inspector_find_by_namespace_returns_empty_for_unknown_namespace() {
	// Arrange
	let mut inspector = RouteInspector::new();
	inspector.add_route(
		"/users/",
		vec![Method::GET],
		Some("api:v1:users:list"),
		None,
	);

	// Act
	let routes = inspector.find_by_namespace("admin");

	// Assert
	assert!(routes.is_empty());
}

// ============================================================
// RouteInspector::find_by_method tests
// ============================================================

#[rstest]
fn inspector_find_by_method_returns_matching_routes() {
	// Arrange
	let mut inspector = RouteInspector::new();
	inspector.add_route(
		"/users/",
		vec![Method::GET, Method::POST],
		None::<String>,
		None,
	);
	inspector.add_route("/posts/", vec![Method::GET], None::<String>, None);
	inspector.add_route("/comments/", vec![Method::POST], None::<String>, None);

	// Act
	let get_routes = inspector.find_by_method(&Method::GET);
	let post_routes = inspector.find_by_method(&Method::POST);
	let delete_routes = inspector.find_by_method(&Method::DELETE);

	// Assert
	assert_eq!(get_routes.len(), 2);
	assert_eq!(post_routes.len(), 2);
	assert!(delete_routes.is_empty());
}

// ============================================================
// RouteInspector::all_namespaces tests
// ============================================================

#[rstest]
fn inspector_all_namespaces_returns_all_hierarchy_levels() {
	// Arrange
	let mut inspector = RouteInspector::new();
	inspector.add_route(
		"/users/",
		vec![Method::GET],
		Some("api:v1:users:list"),
		None,
	);
	inspector.add_route(
		"/posts/",
		vec![Method::GET],
		Some("api:v2:posts:list"),
		None,
	);

	// Act
	let namespaces = inspector.all_namespaces();

	// Assert
	assert!(namespaces.iter().any(|n| n == "api"));
	assert!(namespaces.iter().any(|n| n == "api:v1"));
	assert!(namespaces.iter().any(|n| n == "api:v1:users"));
	assert!(namespaces.iter().any(|n| n == "api:v2"));
	assert!(namespaces.iter().any(|n| n == "api:v2:posts"));
	assert_eq!(namespaces.len(), 5);
}

#[rstest]
fn inspector_all_namespaces_empty_when_no_named_routes() {
	// Arrange
	let mut inspector = RouteInspector::new();
	inspector.add_route("/users/", vec![Method::GET], None::<String>, None);

	// Act
	let namespaces = inspector.all_namespaces();

	// Assert
	assert!(namespaces.is_empty());
}

#[rstest]
fn inspector_all_namespaces_sorted() {
	// Arrange
	let mut inspector = RouteInspector::new();
	inspector.add_route("/z/", vec![Method::GET], Some("zzz:route"), None);
	inspector.add_route("/a/", vec![Method::GET], Some("aaa:route"), None);

	// Act
	let namespaces = inspector.all_namespaces();

	// Assert
	assert_eq!(namespaces[0], "aaa");
	assert_eq!(namespaces[1], "zzz");
}

// ============================================================
// RouteInspector::all_methods tests
// ============================================================

#[rstest]
fn inspector_all_methods_returns_unique_methods() {
	// Arrange
	let mut inspector = RouteInspector::new();
	inspector.add_route(
		"/users/",
		vec![Method::GET, Method::POST],
		None::<String>,
		None,
	);
	inspector.add_route(
		"/posts/",
		vec![Method::GET, Method::DELETE],
		None::<String>,
		None,
	);

	// Act
	let methods = inspector.all_methods();

	// Assert
	assert!(methods.contains(&Method::GET));
	assert!(methods.contains(&Method::POST));
	assert!(methods.contains(&Method::DELETE));
	// GET appears twice in routes but should be deduplicated
	assert_eq!(methods.iter().filter(|m| **m == Method::GET).count(), 1);
}

#[rstest]
fn inspector_all_methods_empty_when_no_routes() {
	// Arrange
	let inspector = RouteInspector::new();

	// Act
	let methods = inspector.all_methods();

	// Assert
	assert!(methods.is_empty());
}

// ============================================================
// RouteInspector::statistics tests
// ============================================================

#[rstest]
fn inspector_statistics_total_routes() {
	// Arrange
	let mut inspector = RouteInspector::new();
	inspector.add_route("/users/", vec![Method::GET], Some("api:users:list"), None);
	inspector.add_route(
		"/users/{id}/",
		vec![Method::GET],
		Some("api:users:detail"),
		None,
	);

	// Act
	let stats = inspector.statistics();

	// Assert
	assert_eq!(stats.total_routes, 2);
	assert_eq!(stats.routes_with_names, 2, "both routes have names");
}

#[rstest]
fn inspector_statistics_routes_with_params() {
	// Arrange
	let mut inspector = RouteInspector::new();
	inspector.add_route("/users/", vec![Method::GET], None::<String>, None);
	inspector.add_route("/users/{id}/", vec![Method::GET], None::<String>, None);
	inspector.add_route(
		"/users/{id}/posts/{post_id}/",
		vec![Method::GET],
		None::<String>,
		None,
	);

	// Act
	let stats = inspector.statistics();

	// Assert
	assert_eq!(stats.routes_with_params, 2);
	assert_eq!(stats.total_routes, 3, "total routes should be 3");
}

#[rstest]
fn inspector_statistics_routes_with_names() {
	// Arrange
	let mut inspector = RouteInspector::new();
	inspector.add_route("/users/", vec![Method::GET], Some("users:list"), None);
	inspector.add_route("/posts/", vec![Method::GET], None::<String>, None);

	// Act
	let stats = inspector.statistics();

	// Assert
	assert_eq!(stats.routes_with_names, 1);
	assert_eq!(stats.total_routes, 2, "total routes should be 2");
}

#[rstest]
fn inspector_statistics_zero_on_empty() {
	// Arrange
	let inspector = RouteInspector::new();

	// Act
	let stats = inspector.statistics();

	// Assert
	assert_eq!(stats.total_routes, 0);
	assert_eq!(stats.total_namespaces, 0);
	assert_eq!(stats.total_methods, 0);
	assert_eq!(stats.routes_with_params, 0);
	assert_eq!(stats.routes_with_names, 0);
}

// ============================================================
// RouteInspector serialization tests
// ============================================================

#[rstest]
fn inspector_to_json_contains_route_name() {
	// Arrange
	let mut inspector = RouteInspector::new();
	inspector.add_route("/users/", vec![Method::GET], Some("users:list"), None);

	// Act
	let json = inspector.to_json().unwrap();
	let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

	// Assert
	let routes = parsed.as_array().unwrap();
	assert_eq!(routes.len(), 1);
	assert_eq!(routes[0]["name"], "users:list");
}

#[rstest]
fn inspector_to_json_contains_path() {
	// Arrange
	let mut inspector = RouteInspector::new();
	inspector.add_route("/api/v1/users/", vec![Method::GET], None::<String>, None);

	// Act
	let json = inspector.to_json().unwrap();
	let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

	// Assert
	let routes = parsed.as_array().unwrap();
	assert_eq!(routes.len(), 1);
	assert_eq!(routes[0]["path"], "/api/v1/users/");
}

#[rstest]
fn inspector_to_json_empty_inspector_produces_empty_array() {
	// Arrange
	let inspector = RouteInspector::new();

	// Act
	let json = inspector.to_json().unwrap();
	let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

	// Assert
	let routes = parsed.as_array().unwrap();
	assert!(routes.is_empty());
}

#[rstest]
fn inspector_to_yaml_contains_route_name() {
	// Arrange
	let mut inspector = RouteInspector::new();
	inspector.add_route("/users/", vec![Method::GET], Some("users:list"), None);

	// Act
	let yaml = inspector.to_yaml().unwrap();
	let parsed: serde_yaml::Value = serde_yaml::from_str(&yaml).unwrap();

	// Assert
	let routes = parsed.as_sequence().unwrap();
	assert_eq!(routes.len(), 1);
	assert_eq!(routes[0]["name"].as_str().unwrap(), "users:list");
}

// ============================================================
// Edge case tests
// ============================================================

#[rstest]
fn inspector_duplicate_paths_last_write_wins_in_index() {
	// Arrange
	let mut inspector = RouteInspector::new();

	// Act — add two routes with the same path
	inspector.add_route("/users/", vec![Method::GET], Some("users:list:v1"), None);
	inspector.add_route("/users/", vec![Method::POST], Some("users:create:v2"), None);

	// Assert — route_count reflects all additions, but path_index stores last
	assert_eq!(inspector.route_count(), 2);
	// The path index stores the last inserted index, so find_by_path returns the last one
	let found = inspector.find_by_path("/users/").unwrap();
	assert_eq!(found.name, Some("users:create:v2".to_string()));
}

#[rstest]
fn inspector_route_with_all_http_methods() {
	// Arrange
	let mut inspector = RouteInspector::new();

	// Act
	inspector.add_route(
		"/resource/",
		vec![
			Method::GET,
			Method::POST,
			Method::PUT,
			Method::PATCH,
			Method::DELETE,
			Method::HEAD,
			Method::OPTIONS,
		],
		None::<String>,
		None,
	);

	// Assert
	let route = inspector.find_by_path("/resource/").unwrap();
	assert_eq!(route.methods.len(), 7);
	assert!(route.supports_method(&Method::PATCH));
	assert!(route.supports_method(&Method::HEAD));
	assert!(route.supports_method(&Method::OPTIONS));
}

#[rstest]
fn inspector_find_by_path_prefix_with_root_prefix() {
	// Arrange
	let mut inspector = RouteInspector::new();
	inspector.add_route("/users/", vec![Method::GET], None::<String>, None);
	inspector.add_route("/posts/", vec![Method::GET], None::<String>, None);

	// Act
	let routes = inspector.find_by_path_prefix("/");

	// Assert — all routes start with "/"
	assert_eq!(routes.len(), 2);
}

#[rstest]
fn inspector_route_info_namespace_object_present_when_namespace_set() {
	// Arrange
	let info = RouteInfo::new("/users/", vec![Method::GET], Some("api:v1:users:list"));

	// Act
	let ns_obj = info.namespace_object();

	// Assert
	assert!(ns_obj.is_some());
}

#[rstest]
fn inspector_route_info_namespace_object_absent_when_no_namespace() {
	// Arrange
	let info = RouteInfo::new("/users/", vec![Method::GET], None::<String>);

	// Act
	let ns_obj = info.namespace_object();

	// Assert
	assert!(ns_obj.is_none());
}

#[rstest]
fn inspector_route_info_namespace_object_absent_for_single_part_name() {
	// Arrange
	let info = RouteInfo::new("/users/", vec![Method::GET], Some("list"));

	// Act
	let ns_obj = info.namespace_object();

	// Assert — single-part name has no namespace
	assert!(ns_obj.is_none());
}

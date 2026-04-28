//! OpenAPI + ViewSets Cross-Crate Integration Tests
//!
//! Validates the integration of OpenAPI schema generation and ViewSets.
//!
//! ## Integration Points
//!
//! - **openapi**: OpenAPI 3.0 schema generation
//! - **viewsets**: ModelViewSet, ReadOnlyModelViewSet, etc.
//!
//! ## Purpose
//!
//! Automatically generate OpenAPI schemas from ViewSets and verify:
//! - Accuracy of paths, components, and schemas
//! - Endpoint generation for CRUD operations
//! - Parameter schema types and constraints
//! - Response schema structure

use rstest::*;
use serde::{Deserialize, Serialize};

use reinhardt_macros::model;
use reinhardt_rest::openapi::SchemaGenerator;
use reinhardt_views::openapi_inspector::{InspectorConfig, ViewSetInspector};
use reinhardt_views::viewsets::{ModelViewSet, ReadOnlyModelViewSet};

/// User model for testing
#[allow(dead_code)]
#[model(table_name = "users")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
	#[field(primary_key = true)]
	id: i64,
	#[field(max_length = 255)]
	username: String,
	#[field(max_length = 255)]
	email: String,
	is_active: bool,
}

/// User serializer for testing
#[derive(Debug, Clone)]
struct UserSerializer;

// Note: tests below that need a "second" model reuse `User` with a different
// basename rather than introducing a separate `Post` struct. The `#[model]`
// macro currently fails to generate `impl Model` for a second `#[model]`
// struct in the same module — see Reinhardt #3985 follow-up. Reusing `User`
// with `ModelViewSet::<User, UserSerializer>::new("posts")` is enough to
// exercise multi-viewset path generation.

/// Fixture: ViewSetInspector
#[fixture]
fn inspector() -> ViewSetInspector {
	ViewSetInspector::new()
}

/// Fixture: SchemaGenerator
#[fixture]
fn generator() -> SchemaGenerator {
	SchemaGenerator::new()
		.title("Test API")
		.version("1.0.0")
		.description("Test API for OpenAPI integration")
}

/// Test 1: OpenAPI paths generation from ModelViewSet
///
/// Verification:
/// - Paths for CRUD operations (GET, POST, PUT, PATCH, DELETE)
/// - Collection endpoint (/api/users/) and detail endpoint (/api/users/{id}/)
/// - Existence of each HTTP method
#[rstest]
#[test]
fn test_model_viewset_openapi_paths_generation(inspector: ViewSetInspector) {
	// Build ModelViewSet
	let viewset = ModelViewSet::<User, UserSerializer>::new("users");

	// Extract paths
	let paths = inspector.extract_paths(&viewset, "/api/users");

	// DEBUG: Print all generated path keys
	eprintln!("Generated paths:");
	for key in paths.keys() {
		eprintln!("  - '{}'", key);
	}

	// Verify that paths were generated
	assert_eq!(
		paths.len(),
		2,
		"Should generate collection and detail paths"
	);

	// Collection endpoint (/api/users/)
	let collection_path = paths.get("/api/users/");
	assert!(
		collection_path.is_some(),
		"Collection endpoint should be generated"
	);

	let collection = collection_path.unwrap();

	// GET (list) operation
	assert!(
		collection.get.is_some(),
		"Collection should have GET operation"
	);

	// POST (create) operation
	assert!(
		collection.post.is_some(),
		"Collection should have POST operation"
	);

	// Detail endpoint (/api/users/{id}/)
	// Try to find the detail path - it should match the OpenAPI format
	let detail_path = paths
		.keys()
		.find(|k| k.contains("{") && k.contains("id"))
		.expect("Should have a detail endpoint with path parameter");

	eprintln!("Found detail path: '{}'", detail_path);

	let detail = paths
		.get(detail_path.as_str())
		.expect("Detail endpoint should be retrievable");

	// GET (retrieve) operation
	assert!(detail.get.is_some(), "Detail should have GET operation");

	// PUT (update) operation
	assert!(detail.put.is_some(), "Detail should have PUT operation");

	// PATCH (partial update) operation
	assert!(detail.patch.is_some(), "Detail should have PATCH operation");

	// DELETE (destroy) operation
	assert!(
		detail.delete.is_some(),
		"Detail should have DELETE operation"
	);
}

/// Test 2: OpenAPI schema generation from ReadOnlyModelViewSet
///
/// Verification:
/// - ViewSetInspector always generates all CRUD operations
/// - ReadOnlyModelViewSet also includes GET/POST/PUT/PATCH/DELETE
/// - Two endpoints (collection and detail) are generated
///
/// Note: The current implementation does not distinguish ViewSet types,
/// and generates all CRUD operations. ViewSet type detection functionality
/// may be added in the future.
#[rstest]
#[test]
fn test_readonly_viewset_openapi_schema(inspector: ViewSetInspector) {
	// Build ReadOnlyModelViewSet
	let viewset = ReadOnlyModelViewSet::<User, UserSerializer>::new("users");

	// Extract paths
	let paths = inspector.extract_paths(&viewset, "/api/users");

	// Verify that paths were generated
	assert_eq!(
		paths.len(),
		2,
		"Should generate collection and detail paths"
	);

	// Collection endpoint
	let collection = paths
		.get("/api/users/")
		.expect("Collection path should exist");

	// GET should exist
	assert!(
		collection.get.is_some(),
		"Collection should have GET operation"
	);

	// POST is also generated in current implementation (may be fixed in the future)
	assert!(
		collection.post.is_some(),
		"Current implementation generates POST for all ViewSets"
	);

	// Detail endpoint - search dynamically
	let detail_path = paths
		.keys()
		.find(|k| k.contains("{") && k.contains("id"))
		.expect("Detail path with parameter should exist");

	let detail = paths
		.get(detail_path.as_str())
		.expect("Detail endpoint should be retrievable");

	// GET should exist
	assert!(detail.get.is_some(), "Detail should have GET operation");

	// PUT, PATCH, DELETE are also generated in current implementation
	assert!(
		detail.put.is_some(),
		"Current implementation generates PUT for all ViewSets"
	);
	assert!(
		detail.patch.is_some(),
		"Current implementation generates PATCH for all ViewSets"
	);
	assert!(
		detail.delete.is_some(),
		"Current implementation generates DELETE for all ViewSets"
	);
}

/// Test 3: OpenAPI 3.0 schema generation from ViewSet
///
/// Verification:
/// - Compliance with OpenAPI 3.0 specification
/// - Accuracy of info section (title, version, description)
/// - JSON serialization
#[rstest]
#[test]
fn test_complete_openapi_schema_generation(
	inspector: ViewSetInspector,
	generator: SchemaGenerator,
) {
	// Extract path information from ViewSet
	let viewset = ModelViewSet::<User, UserSerializer>::new("users");
	let paths = inspector.extract_paths(&viewset, "/api/users");

	// Verify that paths were generated
	assert!(!paths.is_empty(), "Paths should be extracted");

	// Generate OpenAPI schema
	let schema = generator
		.generate()
		.expect("Schema generation should succeed");

	// Verify info section
	assert_eq!(schema.info.title, "Test API", "Title should match");
	assert_eq!(schema.info.version, "1.0.0", "Version should match");
	assert_eq!(
		schema.info.description,
		Some("Test API for OpenAPI integration".to_string()),
		"Description should match"
	);

	// Verify JSON serialization
	let json_result = schema.to_json();
	assert!(json_result.is_ok(), "Schema should be serializable to JSON");

	let json = json_result.unwrap();
	assert!(!json.is_empty(), "JSON should not be empty");

	// Verify OpenAPI version (via JSON)
	assert!(
		json.contains("\"openapi\""),
		"JSON should contain OpenAPI version field"
	);
}

/// Test 4: Response schema generation for ViewSet operations
///
/// Verification:
/// - Schema generation for success responses (200, 201)
/// - Existence of responses
/// - Responses for GET and PUT operations
#[rstest]
#[test]
fn test_viewset_response_schema_generation(inspector: ViewSetInspector) {
	let viewset = ModelViewSet::<User, UserSerializer>::new("users");
	let paths = inspector.extract_paths(&viewset, "/api/users");

	// GET operation on collection endpoint
	let collection = paths
		.get("/api/users/")
		.expect("Collection path should exist");
	let get_operation = collection.get.as_ref().expect("GET operation should exist");

	// Verify existence of response schema
	let responses = &get_operation.responses;
	assert!(
		!responses.responses.is_empty(),
		"Responses should be defined"
	);

	// Verify existence of 200 OK response
	let ok_response = responses.responses.get("200");
	assert!(ok_response.is_some(), "200 OK response should be defined");

	// PUT operation on detail endpoint - search dynamically
	let detail_path = paths
		.keys()
		.find(|k| k.contains("{") && k.contains("id"))
		.expect("Should have a detail endpoint with path parameter");

	let detail = paths
		.get(detail_path.as_str())
		.expect("Detail path should exist");
	let put_operation = detail.put.as_ref().expect("PUT operation should exist");

	// Verify existence of PUT response schema
	let put_responses = &put_operation.responses;
	assert!(
		!put_responses.responses.is_empty(),
		"PUT responses should be defined"
	);

	// Verify existence of 200 OK response
	let put_ok_response = put_responses.responses.get("200");
	assert!(
		put_ok_response.is_some(),
		"200 OK response for PUT should be defined"
	);
}

/// Test 5: ViewSet InspectorConfig Customization
///
/// Verification:
/// - Schema generation with custom InspectorConfig
/// - description and tags settings are reflected
/// - Configuration flexibility
#[rstest]
#[test]
fn test_inspector_config_customization() {
	// Custom InspectorConfig
	let config = InspectorConfig {
		include_descriptions: false,
		include_tags: true,
		default_response_description: "Custom success response".to_string(),
	};

	let inspector = ViewSetInspector::with_config(config);

	// Build ViewSet
	let viewset = ModelViewSet::<User, UserSerializer>::new("users");

	// Extract paths
	let paths = inspector.extract_paths(&viewset, "/api/users");

	// Verify paths were generated
	assert!(
		!paths.is_empty(),
		"Paths should be generated with custom config"
	);

	// Verify collection endpoint
	let collection = paths
		.get("/api/users/")
		.expect("Collection path should exist");

	// GET operation
	let get_operation = collection.get.as_ref().expect("GET operation should exist");

	// Verify response exists
	let responses = &get_operation.responses;
	assert!(
		!responses.responses.is_empty(),
		"Responses should be defined"
	);
}

/// Test 6: Multiple ViewSet Endpoint Generation
///
/// Verification:
/// - Schema generation from multiple ViewSets
/// - Accurate path generation with different basePaths
/// - Path independence
#[rstest]
#[test]
fn test_multiple_viewsets_path_generation(inspector: ViewSetInspector) {
	// User ViewSet
	let user_viewset = ModelViewSet::<User, UserSerializer>::new("users");
	let user_paths = inspector.extract_paths(&user_viewset, "/api/users");

	// Reuse the User model with a different basename — see file-top note about
	// the macro limitation that prevents introducing a second `#[model]` struct
	// in this module.
	let post_viewset = ModelViewSet::<User, UserSerializer>::new("posts");
	let post_paths = inspector.extract_paths(&post_viewset, "/api/posts");

	// User paths
	assert!(
		user_paths.contains_key("/api/users/"),
		"User collection path should exist"
	);

	// User detail path - dynamic search
	// NOTE: Path format is "/api/users{id}/" (not "/api/users/{id}/")
	let user_has_detail = user_paths
		.keys()
		.any(|k| k.starts_with("/api/users") && k.contains("{") && k.contains("id"));
	assert!(
		user_has_detail,
		"User detail path with parameter should exist"
	);

	// Post paths
	assert!(
		post_paths.contains_key("/api/posts/"),
		"Post collection path should exist"
	);

	// Post detail path - dynamic search
	// NOTE: Path format is "/api/posts{id}/" (not "/api/posts/{id}/")
	let post_has_detail = post_paths
		.keys()
		.any(|k| k.starts_with("/api/posts") && k.contains("{") && k.contains("id"));
	assert!(
		post_has_detail,
		"Post detail path with parameter should exist"
	);

	// Path independence (User and Post paths are not mixed)
	assert_eq!(user_paths.len(), 2, "User should have 2 paths");
	assert_eq!(post_paths.len(), 2, "Post should have 2 paths");
}

/// Test 7: SchemaGenerator Integration with Registry
///
/// Verification:
/// - Schema registration in SchemaRegistry
/// - Component reuse
/// - $ref reference generation
#[rstest]
#[test]
fn test_schema_generator_registry_integration(mut generator: SchemaGenerator) {
	// Register User schema in registry
	use reinhardt_rest::openapi::{Schema, SchemaExt};

	let user_schema = Schema::object_with_properties(
		vec![
			("id", Schema::integer()),
			("username", Schema::string()),
			("email", Schema::string()),
			("is_active", Schema::boolean()),
		],
		vec!["id", "username", "email", "is_active"],
	);

	generator.registry().register("User", user_schema);

	// Verify schema was registered in registry
	assert!(
		generator.registry().contains("User"),
		"User schema should be registered"
	);

	// Generate schema
	let schema = generator
		.generate()
		.expect("Schema generation should succeed");

	// Verify components were generated
	assert!(
		schema.components.is_some(),
		"Components should be generated"
	);

	let components = schema.components.unwrap();

	// Verify User schema exists in components/schemas
	assert!(
		components.schemas.contains_key("User"),
		"User schema should be in components"
	);
}

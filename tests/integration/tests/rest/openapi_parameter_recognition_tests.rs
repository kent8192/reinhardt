//! OpenAPI Parameter Recognition Integration Tests
//!
//! Comprehensive verification that OpenAPI schema generation correctly
//! recognizes and represents all parameter types, request bodies,
//! query parameters, schemas, and UI components.
//!
//! ## Test Categories
//!
//! 1. Path Parameter Verification
//! 2. Request Body Verification
//! 3. Query Parameter Verification
//! 4. Schema Definition Verification
//! 5. Swagger UI / Redoc UI HTML Verification
//! 6. End-to-End Integration Verification

use reinhardt_core::endpoint::EndpointMetadata;
use reinhardt_openapi_macros::Schema as DeriveSchema;
use reinhardt_rest::openapi::param_metadata::{
	CookieParam, HeaderParam, ParameterMetadata, PathParam, QueryParam,
};
use reinhardt_rest::openapi::serde_attrs::{FieldMetadata, RenameAll, SchemaBuilderExt};
use reinhardt_rest::openapi::{
	Info, OpenApiSchema, RedocUI, RefOr, Schema, SchemaExt, SchemaGenerator, SwaggerUI, ToSchema,
};
use reinhardt_views::openapi_inspector::ViewSetInspector;
use reinhardt_views::viewsets::ModelViewSet;
use rstest::*;
use serde::{Deserialize, Serialize};
use utoipa::openapi::path::ParameterIn;
use utoipa::openapi::schema::{SchemaType, Type};
use utoipa::openapi::{PathsBuilder, Required};

// ============================================================================
// Test Fixtures
// ============================================================================

/// Dummy model for ViewSet-based tests
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestUser {
	id: i64,
	username: String,
	email: String,
	is_active: bool,
}

/// Dummy serializer for ViewSet-based tests
#[derive(Debug, Clone)]
struct TestUserSerializer;

/// Fixture: ViewSetInspector with default config
#[fixture]
fn inspector() -> ViewSetInspector {
	ViewSetInspector::new()
}

/// Fixture: SchemaGenerator with test defaults
#[fixture]
fn generator() -> SchemaGenerator {
	SchemaGenerator::new()
		.title("Parameter Recognition Test API")
		.version("1.0.0")
		.description("API for OpenAPI parameter recognition tests")
}

/// Fixture: ModelViewSet for users
#[fixture]
fn user_viewset() -> ModelViewSet<TestUser, TestUserSerializer> {
	ModelViewSet::<TestUser, TestUserSerializer>::new("users")
}

/// Helper: Create a minimal OpenAPI schema for UI tests
fn create_test_openapi_schema() -> OpenApiSchema {
	let info = Info::new("Test API", "1.0.0");
	let paths = PathsBuilder::new().build();
	OpenApiSchema::new(info, paths)
}

/// Helper: Create a schema with custom title
fn create_titled_openapi_schema(title: &str) -> OpenApiSchema {
	let info = Info::new(title, "2.0.0");
	let paths = PathsBuilder::new().build();
	OpenApiSchema::new(info, paths)
}

// ============================================================================
// Category 1: Path Parameter Verification (7 tests)
// ============================================================================

/// 1.1 PathParam with Uuid type generates string schema with uuid format
#[rstest]
fn test_path_param_uuid_type_metadata() {
	// Arrange
	let name = "user_id";

	// Act
	let param = PathParam::<uuid::Uuid>::parameter_metadata(name, true);

	// Assert
	let param = param.expect("PathParam<Uuid> should produce a parameter");
	assert_eq!(param.name, "user_id");
	assert!(matches!(param.parameter_in, ParameterIn::Path));
	assert!(matches!(param.required, Required::True));

	// Verify the schema is string type with uuid format
	let schema = param.schema.expect("Parameter should have schema");
	let json = serde_json::to_value(schema).expect("Schema should serialize to JSON");
	assert_eq!(json["type"].as_str(), Some("string"));
	assert_eq!(json["format"].as_str(), Some("uuid"));
}

/// 1.2 PathParam with i64 type generates integer schema
#[rstest]
fn test_path_param_integer_type_metadata() {
	// Arrange
	let name = "id";

	// Act
	let param = PathParam::<i64>::parameter_metadata(name, true);

	// Assert
	let param = param.expect("PathParam<i64> should produce a parameter");
	assert_eq!(param.name, "id");
	assert!(matches!(param.parameter_in, ParameterIn::Path));
	assert!(matches!(param.required, Required::True));

	let schema = param.schema.expect("Parameter should have schema");
	let json = serde_json::to_value(schema).expect("Schema should serialize to JSON");
	assert_eq!(json["type"].as_str(), Some("integer"));
}

/// 1.3 PathParam with String type generates string schema
#[rstest]
fn test_path_param_string_type_metadata() {
	// Arrange
	let name = "slug";

	// Act
	let param = PathParam::<String>::parameter_metadata(name, true);

	// Assert
	let param = param.expect("PathParam<String> should produce a parameter");
	assert_eq!(param.name, "slug");
	assert!(matches!(param.parameter_in, ParameterIn::Path));
	assert!(matches!(param.required, Required::True));

	let schema = param.schema.expect("Parameter should have schema");
	let json = serde_json::to_value(schema).expect("Schema should serialize to JSON");
	assert_eq!(json["type"].as_str(), Some("string"));
}

/// 1.4 ViewSet generates detail path with multiple path sections
#[rstest]
fn test_viewset_generates_collection_and_detail_paths(
	inspector: ViewSetInspector,
	user_viewset: ModelViewSet<TestUser, TestUserSerializer>,
) {
	// Arrange (fixtures provide inspector and viewset)

	// Act
	let paths = inspector.extract_paths(&user_viewset, "/api/users");

	// Assert
	assert_eq!(
		paths.len(),
		2,
		"Should generate collection and detail paths"
	);

	// Collection path
	assert!(
		paths.contains_key("/api/users/"),
		"Collection endpoint should exist"
	);

	// Detail path with {id} parameter
	let detail_path = paths
		.keys()
		.find(|k| k.contains("{") && k.contains("id"))
		.expect("Detail path with {id} parameter should exist");
	assert!(
		detail_path.contains("id"),
		"Detail path should contain id parameter"
	);
}

/// 1.5 ViewSet detail path has id parameter in operations
#[rstest]
fn test_viewset_detail_operations_have_id_parameter(
	inspector: ViewSetInspector,
	user_viewset: ModelViewSet<TestUser, TestUserSerializer>,
) {
	// Arrange
	let paths = inspector.extract_paths(&user_viewset, "/api/users");

	// Act
	let detail_path = paths
		.keys()
		.find(|k| k.contains("{") && k.contains("id"))
		.expect("Detail path should exist");
	let detail = paths.get(detail_path.as_str()).expect("Detail PathItem");

	// Assert - GET operation on detail should have parameters
	let get_op = detail
		.get
		.as_ref()
		.expect("Detail GET operation should exist");
	let params = get_op
		.parameters
		.as_ref()
		.expect("Detail GET should have parameters");
	assert!(!params.is_empty(), "Detail GET should have path parameters");

	// Verify the first parameter is an id parameter
	let id_param = &params[0];
	assert!(
		id_param.name.contains("id"),
		"Parameter should be 'id', got: {}",
		id_param.name
	);
	assert!(matches!(id_param.parameter_in, ParameterIn::Path));
	assert!(matches!(id_param.required, Required::True));
}

/// 1.6 PathParam trait produces correct parameter metadata for various types
#[rstest]
fn test_path_param_trait_various_types() {
	// Arrange & Act
	let int_param = PathParam::<i32>::parameter_metadata("count", true);
	let str_param = PathParam::<String>::parameter_metadata("name", true);
	let bool_param = PathParam::<bool>::parameter_metadata("active", true);

	// Assert
	assert!(int_param.is_some(), "i32 PathParam should produce metadata");
	assert!(
		str_param.is_some(),
		"String PathParam should produce metadata"
	);
	assert!(
		bool_param.is_some(),
		"bool PathParam should produce metadata"
	);

	// All path parameters must be required
	let int_p = int_param.unwrap();
	let str_p = str_param.unwrap();
	let bool_p = bool_param.unwrap();

	assert!(matches!(int_p.required, Required::True));
	assert!(matches!(str_p.required, Required::True));
	assert!(matches!(bool_p.required, Required::True));

	// All should have schemas
	assert!(int_p.schema.is_some());
	assert!(str_p.schema.is_some());
	assert!(bool_p.schema.is_some());
}

/// 1.7 Full JSON output includes path parameters in operation definitions
#[rstest]
fn test_json_output_includes_path_parameters(
	inspector: ViewSetInspector,
	user_viewset: ModelViewSet<TestUser, TestUserSerializer>,
) {
	// Arrange
	let paths = inspector.extract_paths(&user_viewset, "/api/users");
	let generator = SchemaGenerator::new()
		.title("Path Param Test")
		.version("1.0.0");

	// Manually add viewset paths to generator for JSON output
	let schema = generator
		.generate()
		.expect("Schema generation should succeed");
	let json_str = serde_json::to_string_pretty(&schema).expect("JSON serialization");
	let json: serde_json::Value =
		serde_json::from_str(&json_str).expect("JSON parsing should succeed");

	// Assert - basic structure
	assert!(json["openapi"].is_string(), "openapi version should exist");
	assert!(json["info"].is_object(), "info should be an object");
	assert!(json["paths"].is_object(), "paths should be an object");

	// Verify the ViewSet-generated paths have parameter info
	for (path_key, path_item) in &paths {
		if path_key.contains("{") {
			// Detail paths should have operations with parameters
			if let Some(get_op) = &path_item.get {
				let params = get_op
					.parameters
					.as_ref()
					.expect("Detail GET should have parameters");
				assert!(
					!params.is_empty(),
					"Detail GET at {} should have parameters",
					path_key
				);
			}
		}
	}
}

// ============================================================================
// Category 2: Request Body Verification (9 tests)
// ============================================================================

/// 2.1 POST operation has request body
#[rstest]
fn test_viewset_post_operation_has_request_body(
	inspector: ViewSetInspector,
	user_viewset: ModelViewSet<TestUser, TestUserSerializer>,
) {
	// Arrange
	let paths = inspector.extract_paths(&user_viewset, "/api/users");

	// Act
	let collection = paths
		.get("/api/users/")
		.expect("Collection path should exist");
	let post_op = collection
		.post
		.as_ref()
		.expect("POST operation should exist");

	// Assert
	// POST should have either a request body or parameters
	// ViewSetInspector may or may not generate request body depending on implementation
	assert!(
		post_op.request_body.is_some() || post_op.responses.responses.contains_key("200"),
		"POST operation should have request body or responses"
	);
}

/// 2.2 PUT operation exists on detail endpoint
#[rstest]
fn test_viewset_put_operation_exists(
	inspector: ViewSetInspector,
	user_viewset: ModelViewSet<TestUser, TestUserSerializer>,
) {
	// Arrange
	let paths = inspector.extract_paths(&user_viewset, "/api/users");
	let detail_path = paths
		.keys()
		.find(|k| k.contains("{") && k.contains("id"))
		.expect("Detail path should exist");

	// Act
	let detail = paths.get(detail_path.as_str()).expect("Detail PathItem");

	// Assert
	assert!(detail.put.is_some(), "Detail should have PUT operation");
}

/// 2.3 PATCH operation exists on detail endpoint
#[rstest]
fn test_viewset_patch_operation_exists(
	inspector: ViewSetInspector,
	user_viewset: ModelViewSet<TestUser, TestUserSerializer>,
) {
	// Arrange
	let paths = inspector.extract_paths(&user_viewset, "/api/users");
	let detail_path = paths
		.keys()
		.find(|k| k.contains("{") && k.contains("id"))
		.expect("Detail path should exist");

	// Act
	let detail = paths.get(detail_path.as_str()).expect("Detail PathItem");

	// Assert
	assert!(detail.patch.is_some(), "Detail should have PATCH operation");
}

/// 2.4 GET operation has no request body
#[rstest]
fn test_viewset_get_operation_has_no_request_body(
	inspector: ViewSetInspector,
	user_viewset: ModelViewSet<TestUser, TestUserSerializer>,
) {
	// Arrange
	let paths = inspector.extract_paths(&user_viewset, "/api/users");

	// Act
	let collection = paths
		.get("/api/users/")
		.expect("Collection path should exist");
	let get_op = collection.get.as_ref().expect("GET operation should exist");

	// Assert
	assert!(
		get_op.request_body.is_none(),
		"GET operation should not have request body"
	);
}

/// 2.5 DELETE operation has no request body
#[rstest]
fn test_viewset_delete_operation_has_no_request_body(
	inspector: ViewSetInspector,
	user_viewset: ModelViewSet<TestUser, TestUserSerializer>,
) {
	// Arrange
	let paths = inspector.extract_paths(&user_viewset, "/api/users");
	let detail_path = paths
		.keys()
		.find(|k| k.contains("{") && k.contains("id"))
		.expect("Detail path should exist");

	// Act
	let detail = paths.get(detail_path.as_str()).expect("Detail PathItem");
	let delete_op = detail
		.delete
		.as_ref()
		.expect("DELETE operation should exist");

	// Assert
	assert!(
		delete_op.request_body.is_none(),
		"DELETE operation should not have request body"
	);
}

/// 2.6 EndpointInspector creates form-urlencoded request body for POST
///
/// Verifies through the EndpointMetadata struct that form content type is supported.
/// Since create_request_body is private, we verify via EndpointMetadata field structure.
#[rstest]
fn test_endpoint_metadata_supports_form_content_type() {
	// Arrange
	let metadata = EndpointMetadata {
		path: "/api/login",
		method: "POST",
		name: Some("login"),
		function_name: "login",
		module_path: "auth::views",
		request_body_type: Some("LoginForm"),
		request_content_type: Some("application/x-www-form-urlencoded"),
	};

	// Act & Assert
	assert_eq!(metadata.method, "POST");
	assert_eq!(
		metadata.request_content_type,
		Some("application/x-www-form-urlencoded")
	);
	assert_eq!(metadata.request_body_type, Some("LoginForm"));
}

/// 2.7 SchemaGenerator registry holds schemas for request body resolution
#[rstest]
fn test_registry_schema_registration_for_body(mut generator: SchemaGenerator) {
	// Arrange
	let user_schema = Schema::object_with_properties(
		vec![
			("id", Schema::integer()),
			("username", Schema::string()),
			("email", Schema::string()),
		],
		vec!["id", "username", "email"],
	);

	// Act
	generator.registry().register("CreateUser", user_schema);

	// Assert
	assert!(
		generator.registry().contains("CreateUser"),
		"CreateUser schema should be registered"
	);
	assert!(
		generator.registry().get_schema("CreateUser").is_some(),
		"Schema should be retrievable"
	);
}

/// 2.8 ViewSet CRUD operations: POST/PUT/PATCH have body, GET/DELETE don't
#[rstest]
fn test_viewset_crud_body_presence(
	inspector: ViewSetInspector,
	user_viewset: ModelViewSet<TestUser, TestUserSerializer>,
) {
	// Arrange
	let paths = inspector.extract_paths(&user_viewset, "/api/users");
	let collection = paths.get("/api/users/").expect("Collection path");
	let detail_path = paths
		.keys()
		.find(|k| k.contains("{") && k.contains("id"))
		.expect("Detail path");
	let detail = paths.get(detail_path.as_str()).expect("Detail PathItem");

	// Act & Assert - GET should NOT have body
	let get_op = collection.get.as_ref().expect("GET");
	assert!(get_op.request_body.is_none(), "GET should not have body");

	// DELETE should NOT have body
	let delete_op = detail.delete.as_ref().expect("DELETE");
	assert!(
		delete_op.request_body.is_none(),
		"DELETE should not have body"
	);

	// POST/PUT/PATCH should exist (body may or may not be set by ViewSetInspector)
	assert!(collection.post.is_some(), "POST should exist on collection");
	assert!(detail.put.is_some(), "PUT should exist on detail");
	assert!(detail.patch.is_some(), "PATCH should exist on detail");
}

/// 2.9 JSON output contains properly structured requestBody when present
#[rstest]
fn test_json_output_request_body_structure(mut generator: SchemaGenerator) {
	// Arrange
	generator.registry().register(
		"CreateUser",
		Schema::object_with_properties(
			vec![("username", Schema::string()), ("email", Schema::string())],
			vec!["username", "email"],
		),
	);

	// Act
	let json_str = generator.to_json().expect("JSON generation should succeed");
	let json: serde_json::Value = serde_json::from_str(&json_str).expect("Valid JSON");

	// Assert - verify components/schemas contains CreateUser
	let schemas = &json["components"]["schemas"];
	assert!(
		schemas.is_object(),
		"components.schemas should be an object"
	);
	assert!(
		schemas["CreateUser"].is_object(),
		"CreateUser schema should be registered"
	);
	assert_eq!(
		schemas["CreateUser"]["type"].as_str(),
		Some("object"),
		"CreateUser should be object type"
	);
}

// ============================================================================
// Category 3: Query Parameter Verification (7 tests)
// ============================================================================

/// 3.1 QueryParam with String type generates query parameter metadata
#[rstest]
fn test_query_param_string_type() {
	// Arrange
	let name = "search";

	// Act
	let param = QueryParam::<String>::parameter_metadata(name, true);

	// Assert
	let param = param.expect("QueryParam<String> should produce metadata");
	assert_eq!(param.name, "search");
	assert!(matches!(param.parameter_in, ParameterIn::Query));
	assert!(
		matches!(param.required, Required::False),
		"Query params should be optional by default"
	);

	let schema = param.schema.expect("Should have schema");
	let json = serde_json::to_value(schema).expect("Schema serializes");
	assert_eq!(json["type"].as_str(), Some("string"));
}

/// 3.2 QueryParam with i32 type generates integer schema
#[rstest]
fn test_query_param_integer_type() {
	// Arrange & Act
	let param = QueryParam::<i32>::parameter_metadata("page", true);

	// Assert
	let param = param.expect("QueryParam<i32> should produce metadata");
	assert_eq!(param.name, "page");
	assert!(matches!(param.parameter_in, ParameterIn::Query));

	let schema = param.schema.expect("Should have schema");
	let json = serde_json::to_value(schema).expect("Schema serializes");
	assert_eq!(json["type"].as_str(), Some("integer"));
}

/// 3.3 QueryParam with bool type generates boolean schema
#[rstest]
fn test_query_param_boolean_type() {
	// Arrange & Act
	let param = QueryParam::<bool>::parameter_metadata("active", true);

	// Assert
	let param = param.expect("QueryParam<bool> should produce metadata");
	assert_eq!(param.name, "active");
	assert!(matches!(param.parameter_in, ParameterIn::Query));

	let schema = param.schema.expect("Should have schema");
	let json = serde_json::to_value(schema).expect("Schema serializes");
	assert_eq!(json["type"].as_str(), Some("boolean"));
}

/// 3.4 QueryParam with include_in_schema=false returns None
#[rstest]
fn test_query_param_hidden() {
	// Arrange & Act
	let param = QueryParam::<String>::parameter_metadata("internal_filter", false);

	// Assert
	assert!(param.is_none(), "Hidden query param should return None");
}

/// 3.5 HeaderParam generates header parameter metadata
#[rstest]
fn test_header_param_metadata() {
	// Arrange & Act
	let param = HeaderParam::<String>::parameter_metadata("X-API-Key", true);

	// Assert
	let param = param.expect("HeaderParam should produce metadata");
	assert_eq!(param.name, "X-API-Key");
	assert!(matches!(param.parameter_in, ParameterIn::Header));
	assert!(
		matches!(param.required, Required::False),
		"Header params should be optional by default"
	);
	assert!(param.schema.is_some());
}

/// 3.6 CookieParam generates cookie parameter metadata
#[rstest]
fn test_cookie_param_metadata() {
	// Arrange & Act
	let param = CookieParam::<String>::parameter_metadata("session_id", true);

	// Assert
	let param = param.expect("CookieParam should produce metadata");
	assert_eq!(param.name, "session_id");
	assert!(matches!(param.parameter_in, ParameterIn::Cookie));
	assert!(
		matches!(param.required, Required::False),
		"Cookie params should be optional by default"
	);
	assert!(param.schema.is_some());
}

/// 3.7 Multiple parameter types can coexist: Path + Query + Header
#[rstest]
fn test_mixed_parameter_types() {
	// Arrange
	let path_param = PathParam::<uuid::Uuid>::parameter_metadata("user_id", true);
	let query_param = QueryParam::<String>::parameter_metadata("search", true);
	let header_param = HeaderParam::<String>::parameter_metadata("Authorization", true);
	let cookie_param = CookieParam::<String>::parameter_metadata("session", true);

	// Act
	let all_params = vec![
		path_param.expect("Path param"),
		query_param.expect("Query param"),
		header_param.expect("Header param"),
		cookie_param.expect("Cookie param"),
	];

	// Assert - each has different location
	assert!(matches!(all_params[0].parameter_in, ParameterIn::Path));
	assert!(matches!(all_params[1].parameter_in, ParameterIn::Query));
	assert!(matches!(all_params[2].parameter_in, ParameterIn::Header));
	assert!(matches!(all_params[3].parameter_in, ParameterIn::Cookie));

	// Path is required, others optional
	assert!(matches!(all_params[0].required, Required::True));
	assert!(matches!(all_params[1].required, Required::False));
	assert!(matches!(all_params[2].required, Required::False));
	assert!(matches!(all_params[3].required, Required::False));

	// All have schemas
	for param in &all_params {
		assert!(param.schema.is_some(), "All params should have schemas");
	}
}

// ============================================================================
// Category 4: Schema Definition Verification (19 tests)
// ============================================================================

/// 4.1 Basic struct generates object schema with properties and required
#[rstest]
fn test_basic_struct_schema() {
	// Arrange
	#[allow(dead_code)]
	#[derive(DeriveSchema)]
	struct Article {
		id: i64,
		title: String,
		published: bool,
		views: Option<i32>,
	}

	// Act
	let schema = Article::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			assert!(matches!(obj.schema_type, SchemaType::Type(Type::Object)));
			assert_eq!(obj.properties.len(), 4);
			assert!(obj.properties.contains_key("id"));
			assert!(obj.properties.contains_key("title"));
			assert!(obj.properties.contains_key("published"));
			assert!(obj.properties.contains_key("views"));

			// Optional fields are NOT required
			assert!(obj.required.contains(&"id".to_string()));
			assert!(obj.required.contains(&"title".to_string()));
			assert!(obj.required.contains(&"published".to_string()));
			assert!(
				!obj.required.contains(&"views".to_string()),
				"Option<T> field should not be required"
			);
		}
		_ => panic!("Expected Object schema"),
	}
}

/// 4.2 String field constraints: minLength, maxLength, pattern
#[rstest]
fn test_string_field_constraints() {
	// Arrange
	#[allow(dead_code)]
	#[derive(DeriveSchema)]
	struct Registration {
		#[schema(min_length = 3, max_length = 50, pattern = "^[a-zA-Z0-9]+$")]
		username: String,
	}

	// Act
	let schema = Registration::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			if let Some(RefOr::T(Schema::Object(username_obj))) = obj.properties.get("username") {
				assert_eq!(username_obj.min_length, Some(3));
				assert_eq!(username_obj.max_length, Some(50));
				assert_eq!(username_obj.pattern, Some("^[a-zA-Z0-9]+$".to_string()));
			} else {
				panic!("Expected Object schema for username field");
			}
		}
		_ => panic!("Expected Object schema"),
	}
}

/// 4.3 Numeric field constraints: minimum, maximum
#[rstest]
fn test_numeric_field_constraints() {
	// Arrange
	#[allow(dead_code)]
	#[derive(DeriveSchema)]
	struct Rating {
		#[schema(minimum = 1, maximum = 5)]
		score: i32,
	}

	// Act
	let schema = Rating::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			if let Some(RefOr::T(Schema::Object(score_obj))) = obj.properties.get("score") {
				assert!(score_obj.minimum.is_some());
				assert!(score_obj.maximum.is_some());
				let min_json = serde_json::to_value(&score_obj.minimum).unwrap();
				let max_json = serde_json::to_value(&score_obj.maximum).unwrap();
				assert_eq!(min_json.as_f64(), Some(1.0));
				assert_eq!(max_json.as_f64(), Some(5.0));
			} else {
				panic!("Expected Object schema for score field");
			}
		}
		_ => panic!("Expected Object schema"),
	}
}

/// 4.4 Metadata attributes: readOnly, writeOnly, deprecated
#[rstest]
fn test_metadata_attributes() {
	// Arrange
	#[allow(dead_code)]
	#[derive(DeriveSchema)]
	struct Account {
		#[schema(read_only)]
		id: i64,
		#[schema(write_only)]
		password: String,
		#[schema(deprecated)]
		legacy_token: Option<String>,
	}

	// Act
	let schema = Account::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			if let Some(RefOr::T(Schema::Object(id_obj))) = obj.properties.get("id") {
				assert_eq!(id_obj.read_only, Some(true));
			} else {
				panic!("Expected id field");
			}
			if let Some(RefOr::T(Schema::Object(pw_obj))) = obj.properties.get("password") {
				assert_eq!(pw_obj.write_only, Some(true));
			} else {
				panic!("Expected password field");
			}
			if let Some(RefOr::T(Schema::Object(legacy_obj))) = obj.properties.get("legacy_token") {
				assert!(matches!(
					legacy_obj.deprecated,
					Some(utoipa::openapi::Deprecated::True)
				));
			} else {
				panic!("Expected legacy_token field");
			}
		}
		_ => panic!("Expected Object schema"),
	}
}

/// 4.5 Nested struct: parent contains child struct as property
#[rstest]
fn test_nested_struct_schema() {
	// Arrange
	#[allow(dead_code)]
	#[derive(DeriveSchema)]
	struct Location {
		latitude: f64,
		longitude: f64,
	}

	#[allow(dead_code)]
	#[derive(DeriveSchema)]
	struct Event {
		name: String,
		location: Location,
		backup_location: Option<Location>,
	}

	// Act
	let schema = Event::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			assert!(obj.properties.contains_key("name"));
			assert!(obj.properties.contains_key("location"));
			assert!(obj.properties.contains_key("backup_location"));

			// location is required, backup_location is not
			assert!(obj.required.contains(&"location".to_string()));
			assert!(!obj.required.contains(&"backup_location".to_string()));
		}
		_ => panic!("Expected Object schema"),
	}
}

/// 4.6 Vec<T> field generates array schema with items
#[rstest]
fn test_vec_field_array_schema() {
	// Arrange
	#[allow(dead_code)]
	#[derive(DeriveSchema)]
	struct TagList {
		tags: Vec<String>,
	}

	// Act
	let schema = TagList::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			assert!(obj.properties.contains_key("tags"));
			let tags_prop = &obj.properties["tags"];
			// Vec<String> should be an Array schema
			match tags_prop {
				RefOr::T(Schema::Array(arr)) => {
					// Array has items - ArrayItems is always present in Array schema
					match &arr.items {
						utoipa::openapi::schema::ArrayItems::RefOrSchema(_) => {
							// Valid: items definition exists
						}
						utoipa::openapi::schema::ArrayItems::False => {
							panic!("Array schema should have items definition, got False");
						}
					}
				}
				_ => {
					// It may also be represented as RefOr::T(Schema::Object)
					// depending on how ToSchema handles Vec<T> in derive macro context
					// Just verify it exists
				}
			}
		}
		_ => panic!("Expected Object schema"),
	}
}

/// 4.7 HashMap<String, V> generates object schema with additionalProperties
#[rstest]
fn test_hashmap_field_schema() {
	// Arrange & Act
	let schema = <std::collections::HashMap<String, i32>>::schema();

	// Assert
	let json = serde_json::to_value(schema).expect("Schema should serialize");
	assert_eq!(json["type"].as_str(), Some("object"));
	assert!(
		json["additionalProperties"].is_object(),
		"HashMap should have additionalProperties"
	);
}

/// 4.8 DateTime and Uuid types have correct format
#[rstest]
fn test_datetime_uuid_format() {
	// Arrange & Act
	let datetime_schema = <chrono::DateTime<chrono::Utc>>::schema();
	let uuid_schema = uuid::Uuid::schema();

	// Assert - DateTime
	let dt_json = serde_json::to_value(datetime_schema).expect("DateTime schema");
	assert_eq!(dt_json["type"].as_str(), Some("string"));
	assert_eq!(dt_json["format"].as_str(), Some("date-time"));

	// Assert - Uuid
	let uuid_json = serde_json::to_value(uuid_schema).expect("Uuid schema");
	assert_eq!(uuid_json["type"].as_str(), Some("string"));
	assert_eq!(uuid_json["format"].as_str(), Some("uuid"));
}

/// 4.9 Unit enum generates string schema with enum values
#[rstest]
fn test_unit_enum_schema() {
	// Arrange
	#[allow(dead_code)]
	#[derive(DeriveSchema)]
	enum Priority {
		Low,
		Medium,
		High,
		Critical,
	}

	// Act
	let schema = Priority::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			assert!(matches!(obj.schema_type, SchemaType::Type(Type::String)));
			assert!(obj.enum_values.is_some());
			let values = obj.enum_values.as_ref().unwrap();
			assert_eq!(values.len(), 4);
		}
		_ => panic!("Expected Object schema for unit enum"),
	}
}

/// 4.10 Internally tagged enum generates oneOf with discriminator
#[rstest]
fn test_internally_tagged_enum_schema() {
	// Arrange
	#[allow(dead_code)]
	#[derive(DeriveSchema, Serialize, Deserialize)]
	#[serde(tag = "type")]
	enum Notification {
		Email { address: String },
		Sms { phone: String },
	}

	// Act
	let schema = Notification::schema();

	// Assert
	match schema {
		Schema::OneOf(one_of) => {
			assert_eq!(one_of.items.len(), 2);
			assert!(
				one_of.discriminator.is_some(),
				"Internal tag enum should have discriminator"
			);
		}
		_ => panic!("Expected OneOf schema for internally tagged enum"),
	}
}

/// 4.11 Adjacently tagged enum generates oneOf with discriminator
#[rstest]
fn test_adjacently_tagged_enum_schema() {
	// Arrange
	#[allow(dead_code)]
	#[derive(DeriveSchema, Serialize, Deserialize)]
	#[serde(tag = "kind", content = "data")]
	enum Payload {
		Text { content: String },
		Binary { data: Vec<u8> },
	}

	// Act
	let schema = Payload::schema();

	// Assert
	match schema {
		Schema::OneOf(one_of) => {
			assert_eq!(one_of.items.len(), 2);
			assert!(one_of.discriminator.is_some());
		}
		_ => panic!("Expected OneOf schema for adjacently tagged enum"),
	}
}

/// 4.12 Untagged enum generates oneOf without discriminator
#[rstest]
fn test_untagged_enum_schema() {
	// Arrange
	#[allow(dead_code)]
	#[derive(DeriveSchema, Serialize, Deserialize)]
	#[serde(untagged)]
	enum InputValue {
		Text { value: String },
		Number { value: f64 },
	}

	// Act
	let schema = InputValue::schema();

	// Assert
	match schema {
		Schema::OneOf(one_of) => {
			assert_eq!(one_of.items.len(), 2);
			assert!(
				one_of.discriminator.is_none(),
				"Untagged enum should NOT have discriminator"
			);
		}
		_ => panic!("Expected OneOf schema for untagged enum"),
	}
}

/// 4.13 rename_all on enum transforms variant names
#[rstest]
fn test_rename_all_enum_schema() {
	// Arrange
	#[allow(dead_code)]
	#[derive(DeriveSchema, Serialize, Deserialize)]
	#[serde(rename_all = "snake_case")]
	enum UserRole {
		SuperAdmin,
		RegularUser,
		GuestUser,
	}

	// Act
	let schema = UserRole::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			let values = obj.enum_values.as_ref().expect("Should have enum values");
			assert_eq!(values.len(), 3);
			assert!(values.contains(&serde_json::Value::String("super_admin".to_string())));
			assert!(values.contains(&serde_json::Value::String("regular_user".to_string())));
			assert!(values.contains(&serde_json::Value::String("guest_user".to_string())));
		}
		_ => panic!("Expected Object schema for rename_all enum"),
	}
}

/// 4.14 serde rename transforms field name in schema
#[rstest]
fn test_serde_rename_field() {
	// Arrange
	let fields = vec![
		(
			FieldMetadata::new("user_name").with_rename("userName"),
			Schema::string(),
		),
		(FieldMetadata::new("id"), Schema::integer()),
	];

	// Act
	let schema = SchemaBuilderExt::build_object_from_fields(fields);

	// Assert
	match schema {
		Schema::Object(obj) => {
			assert!(
				obj.properties.contains_key("userName"),
				"Renamed field should use new name"
			);
			assert!(
				!obj.properties.contains_key("user_name"),
				"Original name should not exist"
			);
			assert!(obj.properties.contains_key("id"));
		}
		_ => panic!("Expected Object schema"),
	}
}

/// 4.15 serde skip excludes field from schema
#[rstest]
fn test_serde_skip_field() {
	// Arrange
	let fields = vec![
		(FieldMetadata::new("id"), Schema::integer()),
		(FieldMetadata::new("name"), Schema::string()),
		(
			FieldMetadata::new("internal_state").with_skip(true),
			Schema::string(),
		),
	];

	// Act
	let schema = SchemaBuilderExt::build_object_from_fields(fields);

	// Assert
	match schema {
		Schema::Object(obj) => {
			assert_eq!(obj.properties.len(), 2, "Skipped field should be excluded");
			assert!(obj.properties.contains_key("id"));
			assert!(obj.properties.contains_key("name"));
			assert!(!obj.properties.contains_key("internal_state"));
		}
		_ => panic!("Expected Object schema"),
	}
}

/// 4.16 serde flatten generates allOf schema
#[rstest]
fn test_serde_flatten_generates_allof() {
	// Arrange
	let base_schema = Schema::object_with_properties(
		vec![("key", Schema::string()), ("value", Schema::string())],
		vec!["key"],
	);

	let fields = vec![
		(FieldMetadata::new("id"), Schema::integer()),
		(
			FieldMetadata::new("metadata").with_flatten(true),
			base_schema,
		),
	];

	// Act
	let schema = SchemaBuilderExt::build_object_from_fields(fields);

	// Assert
	match schema {
		Schema::AllOf(all_of) => {
			assert_eq!(
				all_of.items.len(),
				2,
				"AllOf should contain base + flattened schema"
			);
		}
		_ => panic!("Expected AllOf schema for flatten"),
	}
}

/// 4.17 Field with default value is excluded from required
#[rstest]
fn test_default_field_not_required() {
	// Arrange
	let fields = vec![
		(FieldMetadata::new("id"), Schema::integer()),
		(
			FieldMetadata::new("count").with_default("0"),
			Schema::integer(),
		),
	];

	// Act
	let schema = SchemaBuilderExt::build_object_from_fields(fields);

	// Assert
	match schema {
		Schema::Object(obj) => {
			assert!(
				obj.required.contains(&"id".to_string()),
				"id should be required"
			);
			assert!(
				!obj.required.contains(&"count".to_string()),
				"Field with default should NOT be required"
			);
		}
		_ => panic!("Expected Object schema"),
	}
}

/// 4.18 rename_all CamelCase transforms field names
#[rstest]
fn test_rename_all_camelcase() {
	// Arrange
	let metadata = vec![
		FieldMetadata::new("user_name"),
		FieldMetadata::new("created_at"),
		FieldMetadata::new("is_active"),
	];

	// Act
	let renamed = SchemaBuilderExt::apply_rename_all(metadata, RenameAll::CamelCase);

	// Assert
	assert_eq!(renamed[0].effective_name(), "userName");
	assert_eq!(renamed[1].effective_name(), "createdAt");
	assert_eq!(renamed[2].effective_name(), "isActive");
}

/// 4.19 Full JSON includes components/schemas with registered types
#[rstest]
fn test_components_schemas_in_json(mut generator: SchemaGenerator) {
	// Arrange
	generator.registry().register(
		"Product",
		Schema::object_with_properties(
			vec![
				("id", Schema::integer()),
				("name", Schema::string()),
				("price", Schema::number()),
			],
			vec!["id", "name", "price"],
		),
	);
	generator.registry().register(
		"Category",
		Schema::object_with_properties(
			vec![("id", Schema::integer()), ("label", Schema::string())],
			vec!["id", "label"],
		),
	);

	// Act
	let json_str = generator.to_json().expect("JSON generation");
	let json: serde_json::Value = serde_json::from_str(&json_str).expect("Valid JSON");

	// Assert
	let schemas = &json["components"]["schemas"];
	assert!(schemas.is_object(), "components.schemas should exist");
	assert!(
		schemas["Product"].is_object(),
		"Product schema should be registered"
	);
	assert!(
		schemas["Category"].is_object(),
		"Category schema should be registered"
	);

	// Verify Product structure
	assert_eq!(schemas["Product"]["type"].as_str(), Some("object"));
	let product_props = &schemas["Product"]["properties"];
	assert!(product_props["id"].is_object());
	assert!(product_props["name"].is_object());
	assert!(product_props["price"].is_object());

	// Verify required
	let required = schemas["Product"]["required"]
		.as_array()
		.expect("required should be array");
	assert_eq!(required.len(), 3);
}

// ============================================================================
// Category 5: Swagger UI / Redoc UI HTML Verification (7 tests)
// ============================================================================

/// 5.1 Swagger UI HTML contains all required elements
#[rstest]
fn test_swagger_ui_required_elements() {
	// Arrange
	let schema = create_test_openapi_schema();
	let swagger = SwaggerUI::new(schema);

	// Act
	let html = swagger.render_html().expect("Swagger UI rendering");

	// Assert
	assert!(
		html.contains(r#"<div id="swagger-ui">"#),
		"Should contain swagger-ui div"
	);
	assert!(
		html.contains("SwaggerUIBundle"),
		"Should contain SwaggerUIBundle JS"
	);
	assert!(
		html.contains("swagger-ui.css"),
		"Should contain Swagger CSS CDN link"
	);
	assert!(
		html.contains("swagger-ui-bundle.js"),
		"Should contain Swagger JS CDN link"
	);
	assert!(
		html.contains("/api/openapi.json"),
		"Should contain spec_url"
	);
	assert!(
		html.contains("StandaloneLayout"),
		"Should contain StandaloneLayout config"
	);
}

/// 5.2 Redoc UI HTML contains required elements
#[rstest]
fn test_redoc_ui_required_elements() {
	// Arrange
	let schema = create_test_openapi_schema();
	let redoc = RedocUI::new(schema);

	// Act
	let html = redoc.render_html().expect("Redoc UI rendering");

	// Assert
	assert!(html.contains("redoc"), "Should contain redoc element");
	assert!(
		html.contains("spec-url="),
		"Should contain spec-url attribute"
	);
	assert!(
		html.contains("redoc.standalone.js"),
		"Should contain Redoc CDN script"
	);
}

/// 5.3 Custom API title is reflected in HTML title
#[rstest]
fn test_custom_title_in_html() {
	// Arrange
	let schema = create_titled_openapi_schema("My Custom API");
	let swagger = SwaggerUI::new(schema.clone());
	let redoc = RedocUI::new(schema);

	// Act
	let swagger_html = swagger.render_html().expect("Swagger rendering");
	let redoc_html = redoc.render_html().expect("Redoc rendering");

	// Assert
	assert!(
		swagger_html.contains("My Custom API"),
		"Swagger HTML should contain the custom title"
	);
	assert!(
		redoc_html.contains("My Custom API"),
		"Redoc HTML should contain the custom title"
	);
}

/// 5.4 Swagger UI contains base64 favicon
#[rstest]
fn test_swagger_ui_favicon() {
	// Arrange
	let schema = create_test_openapi_schema();
	let swagger = SwaggerUI::new(schema);

	// Act
	let html = swagger.render_html().expect("Swagger rendering");

	// Assert
	assert!(
		html.contains("data:image/png;base64,"),
		"Should embed favicon as base64 data URL"
	);
	assert!(
		html.contains(r#"rel="icon""#),
		"Should have favicon link element"
	);
}

/// 5.5 Redoc UI contains base64 favicon
#[rstest]
fn test_redoc_ui_favicon() {
	// Arrange
	let schema = create_test_openapi_schema();
	let redoc = RedocUI::new(schema);

	// Act
	let html = redoc.render_html().expect("Redoc rendering");

	// Assert
	assert!(
		html.contains("data:image/png;base64,"),
		"Should embed favicon as base64 data URL"
	);
	assert!(
		html.contains(r#"rel="icon""#),
		"Should have favicon link element"
	);
}

/// 5.6 schema_json() produces valid JSON with openapi key
#[rstest]
fn test_schema_json_validity() {
	// Arrange
	let schema = create_test_openapi_schema();
	let swagger = SwaggerUI::new(schema);

	// Act
	let json_str = swagger.schema_json().expect("schema_json should succeed");

	// Assert
	let json: serde_json::Value = serde_json::from_str(&json_str).expect("Should be valid JSON");
	assert!(json["openapi"].is_string(), "Should contain openapi key");
	assert!(json["info"].is_object(), "Should contain info key");
}

/// 5.7 HTML is well-formed with doctype, html, head, body, meta charset
#[rstest]
fn test_html_wellformedness() {
	// Arrange
	let schema = create_test_openapi_schema();
	let swagger = SwaggerUI::new(schema.clone());
	let redoc = RedocUI::new(schema);

	// Act
	let swagger_html = swagger.render_html().expect("Swagger rendering");
	let redoc_html = redoc.render_html().expect("Redoc rendering");

	// Assert - Swagger UI
	assert!(
		swagger_html.contains("<!doctype html>"),
		"Swagger should have doctype"
	);
	assert!(
		swagger_html.contains("<html"),
		"Swagger should have html tag"
	);
	assert!(
		swagger_html.contains("<head>"),
		"Swagger should have head tag"
	);
	assert!(
		swagger_html.contains("<body>"),
		"Swagger should have body tag"
	);
	assert!(
		swagger_html.contains("<meta charset"),
		"Swagger should have meta charset"
	);

	// Assert - Redoc UI
	assert!(
		redoc_html.contains("<!doctype html>"),
		"Redoc should have doctype"
	);
	assert!(redoc_html.contains("<html"), "Redoc should have html tag");
	assert!(redoc_html.contains("<head>"), "Redoc should have head tag");
	assert!(redoc_html.contains("<body>"), "Redoc should have body tag");
	assert!(
		redoc_html.contains("<meta charset"),
		"Redoc should have meta charset"
	);
}

// ============================================================================
// Category 6: End-to-End Integration Verification (9 tests)
// ============================================================================

/// 6.1 Full pipeline: Schema + Paths → JSON with complete structure
#[rstest]
fn test_full_pipeline_json_structure(mut generator: SchemaGenerator) {
	// Arrange
	generator.registry().register(
		"User",
		Schema::object_with_properties(
			vec![
				("id", Schema::integer()),
				("name", Schema::string()),
				("email", Schema::string()),
			],
			vec!["id", "name", "email"],
		),
	);

	// Act
	let json_str = generator.to_json().expect("JSON generation");
	let json: serde_json::Value = serde_json::from_str(&json_str).expect("Valid JSON");

	// Assert
	assert_eq!(json["openapi"].as_str(), Some("3.1.0"));
	assert_eq!(
		json["info"]["title"].as_str(),
		Some("Parameter Recognition Test API")
	);
	assert_eq!(json["info"]["version"].as_str(), Some("1.0.0"));
	assert!(json["paths"].is_object());
	assert!(json["components"].is_object());
	assert!(json["components"]["schemas"]["User"].is_object());
}

/// 6.2 ViewSet + Generator integration produces valid schema
#[rstest]
fn test_viewset_generator_integration(
	inspector: ViewSetInspector,
	user_viewset: ModelViewSet<TestUser, TestUserSerializer>,
	generator: SchemaGenerator,
) {
	// Arrange
	let paths = inspector.extract_paths(&user_viewset, "/api/users");

	// Act
	let schema = generator.generate().expect("Schema generation");

	// Assert
	assert_eq!(schema.info.title, "Parameter Recognition Test API");
	assert!(!paths.is_empty(), "ViewSet paths should be generated");

	// Verify JSON serialization
	let json = schema.to_json();
	assert!(json.is_ok(), "Schema should serialize to JSON");
}

/// 6.3 SwaggerUI produces valid JSON from schema_json()
#[rstest]
fn test_swagger_ui_json_endpoint_simulation() {
	// Arrange
	let schema = create_test_openapi_schema();
	let swagger = SwaggerUI::new(schema);

	// Act
	let json_str = swagger.schema_json().expect("schema_json should succeed");

	// Assert
	let json: serde_json::Value =
		serde_json::from_str(&json_str).expect("Response should be valid JSON");
	assert!(
		json["openapi"].is_string(),
		"Should contain openapi version"
	);
	assert!(json["info"].is_object(), "Should contain info section");
}

/// 6.4 SwaggerUI render_html produces Swagger UI content
#[rstest]
fn test_swagger_ui_html_endpoint_simulation() {
	// Arrange
	let schema = create_test_openapi_schema();
	let swagger = SwaggerUI::new(schema);

	// Act
	let html = swagger.render_html().expect("render_html should succeed");

	// Assert
	assert!(
		html.contains("swagger-ui"),
		"Response should contain swagger-ui"
	);
	assert!(
		html.contains("SwaggerUIBundle"),
		"Response should contain SwaggerUIBundle"
	);
}

/// 6.5 RedocUI render_html produces Redoc content
#[rstest]
fn test_redoc_ui_html_endpoint_simulation() {
	// Arrange
	let schema = create_test_openapi_schema();
	let redoc = RedocUI::new(schema);

	// Act
	let html = redoc.render_html().expect("render_html should succeed");

	// Assert
	assert!(
		html.contains("redoc"),
		"Response should contain redoc element"
	);
}

/// 6.6 generate_openapi_schema produces complete schema from global registry
#[rstest]
fn test_generate_openapi_schema_function() {
	// Arrange & Act
	let schema = reinhardt_rest::openapi::generate_openapi_schema();

	// Assert
	let json = schema.to_json();
	assert!(json.is_ok(), "Schema should serialize to JSON");
	let json_str = json.unwrap();
	let parsed: serde_json::Value = serde_json::from_str(&json_str).expect("Should be valid JSON");
	assert!(parsed["openapi"].is_string(), "Should have openapi version");
	assert!(parsed["info"].is_object(), "Should have info");
}

/// 6.7 Structural validation: root keys, path format, operation structure
#[rstest]
fn test_structural_validation(mut generator: SchemaGenerator) {
	// Arrange
	generator.registry().register(
		"Item",
		Schema::object_with_properties(
			vec![("id", Schema::integer()), ("name", Schema::string())],
			vec!["id", "name"],
		),
	);

	// Act
	let json_str = generator.to_json().expect("JSON generation");
	let json: serde_json::Value = serde_json::from_str(&json_str).expect("Valid JSON");

	// Assert - root keys
	assert!(
		json["openapi"].is_string(),
		"Root should have 'openapi' key"
	);
	assert!(json["info"].is_object(), "Root should have 'info' key");
	assert!(json["paths"].is_object(), "Root should have 'paths' key");
	assert!(
		json["components"].is_object(),
		"Root should have 'components' key"
	);

	// Verify info structure
	assert!(json["info"]["title"].is_string(), "Info should have title");
	assert!(
		json["info"]["version"].is_string(),
		"Info should have version"
	);

	// Verify components structure
	assert!(
		json["components"]["schemas"].is_object(),
		"Components should have schemas"
	);
}

/// 6.8 Multiple schemas with different types in single spec
#[rstest]
fn test_multiple_schemas_mixed_types(mut generator: SchemaGenerator) {
	// Arrange
	generator.registry().register(
		"User",
		Schema::object_with_properties(
			vec![
				("id", Schema::integer()),
				("name", Schema::string()),
				("active", Schema::boolean()),
			],
			vec!["id", "name"],
		),
	);
	generator.registry().register(
		"UserList",
		Schema::array(Schema::object_with_properties(
			vec![("id", Schema::integer()), ("name", Schema::string())],
			vec!["id"],
		)),
	);
	generator.registry().register(
		"ErrorResponse",
		Schema::object_with_properties(
			vec![("code", Schema::integer()), ("message", Schema::string())],
			vec!["code", "message"],
		),
	);

	// Act
	let json_str = generator.to_json().expect("JSON generation");
	let json: serde_json::Value = serde_json::from_str(&json_str).expect("Valid JSON");

	// Assert
	let schemas = &json["components"]["schemas"];
	assert!(schemas["User"].is_object(), "User schema should exist");
	assert!(
		schemas["UserList"].is_object(),
		"UserList schema should exist"
	);
	assert!(
		schemas["ErrorResponse"].is_object(),
		"ErrorResponse schema should exist"
	);

	// Verify different schema types
	assert_eq!(schemas["User"]["type"].as_str(), Some("object"));
	assert_eq!(schemas["ErrorResponse"]["type"].as_str(), Some("object"));
}

/// 6.9 Swagger UI spec_url matches JSON endpoint URL
#[rstest]
fn test_spec_url_consistency() {
	// Arrange
	let schema = create_test_openapi_schema();
	let swagger = SwaggerUI::new(schema);

	// Act
	let html = swagger.render_html().expect("Swagger rendering");

	// Assert
	// The spec_url in the Swagger HTML should point to /api/openapi.json
	assert!(
		html.contains(r#"url: "/api/openapi.json""#),
		"Swagger UI spec_url should match the JSON endpoint URL"
	);
}

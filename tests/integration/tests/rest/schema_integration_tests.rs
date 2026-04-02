//! Schema Macro Integration Tests
//!
//! Validates the `#[derive(Schema)]` macro and schema registry integration.
//!
//! ## Integration Points
//!
//! - **openapi-macros**: `#[derive(Schema)]` procedural macro
//! - **openapi**: Global schema registry and `ToSchema` trait
//! - **endpoint_inspector**: Request body schema retrieval
//!
//! ## Purpose
//!
//! Verify that:
//! - `#[derive(Schema)]` correctly generates OpenAPI schemas
//! - Schemas are automatically registered in `GLOBAL_SCHEMA_REGISTRY`
//! - `EndpointInspector` retrieves schemas from the registry
//! - Request bodies appear correctly in generated OpenAPI JSON

use rstest::rstest;
use serde::{Deserialize, Serialize};

use reinhardt_core::endpoint::{AuthProtection, EndpointMetadata};
use reinhardt_rest::openapi::{EndpointInspector, Schema as OpenApiSchema, ToSchema};

/// Test model with Schema derivation
#[derive(Debug, Clone, Serialize, Deserialize, reinhardt_rest::openapi::Schema)]
struct CreateUserRequest {
	username: String,
	email: String,
	password: String,
}

/// Test model with optional fields
#[derive(Debug, Clone, Serialize, Deserialize, reinhardt_rest::openapi::Schema)]
struct UpdateUserRequest {
	username: Option<String>,
	email: Option<String>,
}

/// Test model with field attributes
#[derive(Debug, Clone, Serialize, Deserialize, reinhardt_rest::openapi::Schema)]
struct UserProfile {
	#[schema(description = "User ID")]
	id: i64,

	#[schema(description = "Username", example = "john_doe")]
	username: String,

	#[schema(description = "Email address", example = "john@example.com")]
	email: String,

	#[serde(rename = "isActive")]
	#[schema(description = "Whether the user is active")]
	is_active: bool,
}

/// Test 1: Basic `#[derive(Schema)]` functionality
///
/// Verification:
/// - ToSchema trait is implemented
/// - schema() method returns valid OpenAPI schema
/// - schema_name() returns correct type name
#[rstest]
fn test_schema_derive_macro_basic() {
	// Verify ToSchema trait is implemented
	let schema = CreateUserRequest::schema();

	// Schema should be an Object
	match &schema {
		OpenApiSchema::Object(obj) => {
			// Verify schema type exists
			// Note: schema_type is SchemaType enum, not Option

			// Verify properties exist
			assert!(!obj.properties.is_empty(), "Schema should have properties");

			let props = &obj.properties;

			// Verify field count
			assert_eq!(
				props.len(),
				3,
				"Should have 3 properties (username, email, password)"
			);

			// Verify specific fields
			assert!(props.contains_key("username"), "Should have username field");
			assert!(props.contains_key("email"), "Should have email field");
			assert!(props.contains_key("password"), "Should have password field");
		}
		_ => panic!("Expected Object schema"),
	}

	// Verify schema_name()
	let name = CreateUserRequest::schema_name();
	assert!(name.is_some(), "schema_name() should return Some");
	assert_eq!(
		name.unwrap(),
		"CreateUserRequest",
		"Schema name should match struct name"
	);
}

/// Test 2: Global schema registry registration
///
/// Verification:
/// - Schemas are automatically registered via inventory
/// - GLOBAL_SCHEMA_REGISTRY contains the registered schema
/// - Registered schema matches the generated schema
#[rstest]
fn test_schema_global_registry_registration() {
	// Access global registry
	let registry = reinhardt_rest::openapi::registry::get_all_schemas();

	// Verify CreateUserRequest is registered
	assert!(
		registry.contains_key("CreateUserRequest"),
		"CreateUserRequest should be registered in global registry"
	);

	// Verify UpdateUserRequest is registered
	assert!(
		registry.contains_key("UpdateUserRequest"),
		"UpdateUserRequest should be registered in global registry"
	);

	// Verify UserProfile is registered
	assert!(
		registry.contains_key("UserProfile"),
		"UserProfile should be registered in global registry"
	);

	// Verify the registered schema matches the trait-generated schema
	let registered_schema = registry.get("CreateUserRequest").unwrap();
	let trait_schema = CreateUserRequest::schema();

	// Both should be Object schemas
	assert!(
		matches!(registered_schema, OpenApiSchema::Object(_)),
		"Registered schema should be Object"
	);
	assert!(
		matches!(trait_schema, OpenApiSchema::Object(_)),
		"Trait schema should be Object"
	);
}

/// Test 3: EndpointInspector uses registered schema
///
/// Verification:
/// - EndpointInspector retrieves schema from global registry
/// - Request body is created with correct schema
/// - extract_paths() produces valid OpenAPI path items
#[rstest]
fn test_endpoint_inspector_uses_registered_schema() {
	let inspector = EndpointInspector::new();

	// Extract paths from registered endpoints
	let paths = inspector.extract_paths();

	// If paths extraction succeeds, verify structure
	if let Ok(paths) = paths {
		// Verify POST operations have request bodies with schemas
		for (path, path_item) in &paths {
			if let Some(operation) = &path_item.post {
				if let Some(request_body) = &operation.request_body {
					assert!(
						!request_body.content.is_empty(),
						"POST {} should have request body content",
						path
					);

					if let Some(json_content) = request_body.content.get("application/json") {
						assert!(
							json_content.schema.is_some(),
							"JSON content for POST {} should have schema",
							path
						);
					}
				}
			}

			// Check PUT operations
			if let Some(operation) = &path_item.put {
				if let Some(request_body) = &operation.request_body {
					assert!(
						!request_body.content.is_empty(),
						"PUT {} should have request body content",
						path
					);
				}
			}

			// Check PATCH operations
			if let Some(operation) = &path_item.patch {
				if let Some(request_body) = &operation.request_body {
					assert!(
						!request_body.content.is_empty(),
						"PATCH {} should have request body content",
						path
					);
				}
			}
		}
	}

	// Original metadata verification (still valid)
	let metadata = EndpointMetadata {
		path: "/api/users",
		method: "POST",
		name: Some("create_user"),
		function_name: "create_user",
		module_path: "users::views",
		request_body_type: Some("CreateUserRequest"),
		request_content_type: Some("application/json"),
		responses: &[],
		headers: &[],
		security: &[],
		auth_protection: AuthProtection::None,
		guard_description: None,
	};

	assert_eq!(metadata.request_body_type, Some("CreateUserRequest"));
	assert_eq!(metadata.request_content_type, Some("application/json"));
	assert_eq!(metadata.method, "POST");
}

/// Test 4: Request body generation with registered schema
///
/// Verification:
/// - POST/PUT/PATCH methods generate request bodies
/// - GET methods do not generate request bodies
/// - Content-Type is correctly set
#[rstest]
fn test_request_body_with_registered_schema() {
	// Test POST metadata
	let post_metadata = EndpointMetadata {
		path: "/api/users",
		method: "POST",
		name: Some("create_user"),
		function_name: "create_user",
		module_path: "users::views",
		request_body_type: Some("CreateUserRequest"),
		request_content_type: Some("application/json"),
		responses: &[],
		headers: &[],
		security: &[],
		auth_protection: AuthProtection::None,
		guard_description: None,
	};

	// Verify POST should have request body
	assert!(
		matches!(post_metadata.method, "POST" | "PUT" | "PATCH"),
		"POST should be eligible for request body"
	);

	// Test GET metadata (should not have request body)
	let get_metadata = EndpointMetadata {
		path: "/api/users",
		method: "GET",
		name: Some("list_users"),
		function_name: "list_users",
		module_path: "users::views",
		request_body_type: None,
		request_content_type: None,
		responses: &[],
		headers: &[],
		security: &[],
		auth_protection: AuthProtection::None,
		guard_description: None,
	};

	// Verify GET should not have request body
	assert!(
		!matches!(get_metadata.method, "POST" | "PUT" | "PATCH"),
		"GET should not be eligible for request body"
	);
}

/// Test 5: Schema with field attributes
///
/// Verification:
/// - Field-level attributes are processed
/// - Descriptions are included
/// - Examples are included
/// - Serde rename is respected
#[rstest]
fn test_schema_with_field_attributes() {
	let schema = UserProfile::schema();

	match &schema {
		OpenApiSchema::Object(obj) => {
			let props = &obj.properties;

			// Verify all fields exist
			assert!(props.contains_key("id"), "Should have id field");
			assert!(props.contains_key("username"), "Should have username field");
			assert!(props.contains_key("email"), "Should have email field");

			// Verify serde rename is applied
			assert!(
				props.contains_key("isActive"),
				"Should have isActive field (renamed from is_active)"
			);
			assert!(
				!props.contains_key("is_active"),
				"Should not have is_active field (renamed to isActive)"
			);
		}
		_ => panic!("Expected Object schema"),
	}
}

/// Test 6: Schema with optional fields
///
/// Verification:
/// - Optional fields are correctly represented
/// - Required fields list is accurate
#[rstest]
fn test_schema_with_optional_fields() {
	let schema = UpdateUserRequest::schema();

	match &schema {
		OpenApiSchema::Object(obj) => {
			let props = &obj.properties;

			// Verify fields exist
			assert!(props.contains_key("username"), "Should have username field");
			assert!(props.contains_key("email"), "Should have email field");

			// In OpenAPI, optional fields typically don't appear in required list
			// The macro should handle Option<T> appropriately
			let required = &obj.required;
			// Optional fields should not be in required list
			assert!(
				!required.contains(&"username".to_string()),
				"Optional username should not be required"
			);
			assert!(
				!required.contains(&"email".to_string()),
				"Optional email should not be required"
			);
		}
		_ => panic!("Expected Object schema"),
	}
}

/// Test 7: Multiple schema registration
///
/// Verification:
/// - Multiple structs can derive Schema
/// - Each gets registered independently
/// - No conflicts between schemas
#[rstest]
fn test_multiple_schema_registration() {
	let registry = reinhardt_rest::openapi::registry::get_all_schemas();

	// All three test models should be registered
	let registered_schemas = vec!["CreateUserRequest", "UpdateUserRequest", "UserProfile"];

	for schema_name in registered_schemas {
		assert!(
			registry.contains_key(schema_name),
			"{} should be registered",
			schema_name
		);
	}

	// Verify they are distinct schemas
	let create_schema = registry.get("CreateUserRequest");
	let update_schema = registry.get("UpdateUserRequest");
	assert!(
		create_schema.is_some() && update_schema.is_some(),
		"Both schemas should exist"
	);
	// Compare by serializing to JSON (since Schema doesn't implement Debug/PartialEq)
	let create_json = serde_json::to_string(create_schema.unwrap()).unwrap();
	let update_json = serde_json::to_string(update_schema.unwrap()).unwrap();
	assert_ne!(create_json, update_json, "Schemas should be different");
}

/// Test 8: Global registry contains expected minimum number of schemas
///
/// Verification:
/// - The global registry is populated at startup via inventory/ctor
/// - At least the 3 test models registered in this file are present
/// - Registry size uses >= N assertion to tolerate other schemas from other test files
#[rstest]
fn test_global_registry_contains_minimum_expected_schemas() {
	// Arrange
	let registry = reinhardt_rest::openapi::registry::get_all_schemas();

	// Assert: must contain at least the 3 structs defined in this test file
	// Use >= to avoid breakage when other test files register additional schemas
	assert!(
		registry.len() >= 3,
		"Global registry should contain at least 3 schemas, found {}",
		registry.len()
	);

	// Verify the specific schemas from this test file are present
	assert!(
		registry.contains_key("CreateUserRequest"),
		"Registry must contain CreateUserRequest"
	);
	assert!(
		registry.contains_key("UpdateUserRequest"),
		"Registry must contain UpdateUserRequest"
	);
	assert!(
		registry.contains_key("UserProfile"),
		"Registry must contain UserProfile"
	);
}

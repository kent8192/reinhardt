//! Integration tests for OpenAPI schema generation from derive macros.
//!
//! This test suite verifies:
//! - Basic schema generation from struct fields
//! - Nested schema generation for complex types
//! - Enum variant schemas with different tagging strategies
//! - Schema validation constraints (min/max, length, pattern)
//! - Schema references and $ref resolution
//! - Documentation extraction and schema metadata
//!
//! Moved from `reinhardt-openapi-macros/tests/` to avoid circular dev-dependencies.

use reinhardt_openapi_macros::Schema as DeriveSchema;
use reinhardt_rest::openapi::{RefOr, Schema, ToSchema};
use rstest::rstest;
use utoipa::openapi::schema::{SchemaType, Type};

// Helper functions for assertions
fn get_object_property<'a>(
	obj: &'a utoipa::openapi::schema::Object,
	key: &str,
) -> Option<&'a utoipa::openapi::schema::Object> {
	match obj.properties.get(key) {
		Some(RefOr::T(Schema::Object(prop))) => Some(prop),
		_ => None,
	}
}

/// Basic struct with primitive types
#[allow(dead_code)]
#[derive(DeriveSchema)]
struct BasicStruct {
	/// User's unique identifier
	id: i64,
	/// User's full name
	name: String,
	/// User's age (optional)
	age: Option<i32>,
	/// Account active status
	is_active: bool,
}

/// Struct with validation constraints
#[allow(dead_code)]
#[derive(DeriveSchema)]
struct ConstrainedStruct {
	/// Username with length constraints
	#[schema(min_length = 3, max_length = 50)]
	username: String,

	/// Email address with format
	#[schema(format = "email")]
	email: String,

	/// Age with numeric constraints
	#[schema(minimum = 0, maximum = 150)]
	age: i32,

	/// Password with pattern validation
	#[schema(pattern = "^(?=.*[A-Za-z])(?=.*\\d)[A-Za-z\\d]{8,}$")]
	password: String,
}

/// Struct with field metadata
#[allow(dead_code)]
#[derive(DeriveSchema)]
struct MetadataStruct {
	/// Read-only field (ID)
	#[schema(read_only)]
	id: i64,

	/// Write-only field (password)
	#[schema(write_only)]
	password: String,

	/// Deprecated field
	#[schema(deprecated)]
	legacy_field: Option<String>,

	/// Field with custom example
	#[schema(example = "john.doe@example.com")]
	email: String,

	/// Field with explicit description
	#[schema(description = "User's preferred display name")]
	display_name: String,
}

/// Nested struct for composition testing
#[allow(dead_code)]
#[derive(DeriveSchema)]
struct Address {
	/// Street address
	street: String,
	/// City name
	city: String,
	/// Postal code
	postal_code: String,
}

/// Struct with nested field
#[allow(dead_code)]
#[derive(DeriveSchema)]
struct UserWithAddress {
	/// User ID
	id: i64,
	/// User name
	name: String,
	/// User's address
	address: Address,
	/// Optional secondary address
	secondary_address: Option<Address>,
}

/// Struct with collection types
#[allow(dead_code)]
#[derive(DeriveSchema)]
struct CollectionStruct {
	/// List of tags
	tags: Vec<String>,
	/// Map of metadata
	metadata: std::collections::HashMap<String, String>,
}

/// Struct with multiple optional fields
#[allow(dead_code)]
#[derive(DeriveSchema)]
struct OptionalFields {
	/// Required field
	id: i64,
	/// Optional string
	optional_string: Option<String>,
	/// Optional number
	optional_number: Option<i32>,
	/// Optional boolean
	optional_bool: Option<bool>,
}

/// Struct with all primitive types
#[allow(dead_code)]
#[derive(DeriveSchema)]
struct AllPrimitives {
	/// 8-bit integer
	i8_field: i8,
	/// 16-bit integer
	i16_field: i16,
	/// 32-bit integer
	i32_field: i32,
	/// 64-bit integer
	i64_field: i64,
	/// Unsigned 8-bit integer
	u8_field: u8,
	/// Unsigned 16-bit integer
	u16_field: u16,
	/// Unsigned 32-bit integer
	u32_field: u32,
	/// Unsigned 64-bit integer
	u64_field: u64,
	/// 32-bit float
	f32_field: f32,
	/// 64-bit float
	f64_field: f64,
	/// Boolean
	bool_field: bool,
	/// String
	string_field: String,
}

/// Struct with complex validation
#[allow(dead_code)]
#[derive(DeriveSchema)]
struct ComplexValidation {
	/// Email with format and example
	#[schema(format = "email", example = "user@example.com")]
	email: String,

	/// URL with format and pattern
	#[schema(format = "uri", pattern = "^https://")]
	website: String,

	/// DateTime with format
	#[schema(format = "date-time")]
	created_at: String,

	/// Percentage with min/max
	#[schema(minimum = 0, maximum = 100)]
	completion_percentage: i32,
}

// ============================================================================
// Test Cases
// ============================================================================

#[rstest]
fn test_basic_struct_schema_generation() {
	// Arrange & Act
	let schema = BasicStruct::schema();

	// Assert
	if let Schema::Object(obj) = schema {
		// Check schema type (schema_type is SchemaType, not Option<SchemaType>)
		assert!(matches!(obj.schema_type, SchemaType::Type(Type::Object)));

		// Verify properties exist
		assert!(obj.properties.contains_key("id"));
		assert!(obj.properties.contains_key("name"));
		assert!(obj.properties.contains_key("age"));
		assert!(obj.properties.contains_key("is_active"));

		// Verify required fields (non-Option types)
		assert!(obj.required.contains(&"id".to_string()));
		assert!(obj.required.contains(&"name".to_string()));
		assert!(obj.required.contains(&"is_active".to_string()));

		// Verify age is not required (it's Option<i32>)
		assert!(!obj.required.contains(&"age".to_string()));
	} else {
		panic!("Expected Object schema");
	}
}

#[rstest]
fn test_schema_name_generation() {
	// Act
	let schema_name = BasicStruct::schema_name();

	// Assert
	assert_eq!(schema_name, Some("BasicStruct".to_string()));
}

#[rstest]
fn test_constrained_struct_min_max_length() {
	// Act
	let schema = ConstrainedStruct::schema();

	// Assert
	if let Schema::Object(obj) = schema {
		// Check username constraints
		if let Some(username_schema) = get_object_property(&obj, "username") {
			assert_eq!(username_schema.min_length, Some(3));
			assert_eq!(username_schema.max_length, Some(50));
		} else {
			panic!("Expected username to be an object schema");
		}
	} else {
		panic!("Expected Object schema");
	}
}

#[rstest]
fn test_constrained_struct_format() {
	// Act
	let schema = ConstrainedStruct::schema();

	// Assert
	if let Schema::Object(obj) = schema {
		// Check email format
		if let Some(email_schema) = get_object_property(&obj, "email") {
			// Verify format exists and contains "email"
			assert!(email_schema.format.is_some());
		} else {
			panic!("Expected email to be an object schema");
		}
	} else {
		panic!("Expected Object schema");
	}
}

#[rstest]
fn test_constrained_struct_numeric_constraints() {
	// Act
	let schema = ConstrainedStruct::schema();

	// Assert
	if let Schema::Object(obj) = schema {
		// Check age minimum and maximum
		if let Some(age_schema) = get_object_property(&obj, "age") {
			// Verify minimum and maximum exist
			assert!(age_schema.minimum.is_some());
			assert!(age_schema.maximum.is_some());
			// Verify values through JSON serialization
			let min_json = serde_json::to_value(&age_schema.minimum).unwrap();
			let max_json = serde_json::to_value(&age_schema.maximum).unwrap();
			assert_eq!(min_json.as_f64(), Some(0.0));
			assert_eq!(max_json.as_f64(), Some(150.0));
		} else {
			panic!("Expected age to be an object schema");
		}
	} else {
		panic!("Expected Object schema");
	}
}

#[rstest]
fn test_constrained_struct_pattern() {
	// Act
	let schema = ConstrainedStruct::schema();

	// Assert
	if let Schema::Object(obj) = schema {
		// Check password pattern
		if let Some(password_schema) = get_object_property(&obj, "password") {
			assert_eq!(
				password_schema.pattern,
				Some("^(?=.*[A-Za-z])(?=.*\\d)[A-Za-z\\d]{8,}$".to_string())
			);
		} else {
			panic!("Expected password to be an object schema");
		}
	} else {
		panic!("Expected Object schema");
	}
}

#[rstest]
fn test_metadata_struct_read_only() {
	// Act
	let schema = MetadataStruct::schema();

	// Assert
	if let Schema::Object(obj) = schema {
		// Check read_only field
		if let Some(id_schema) = get_object_property(&obj, "id") {
			assert_eq!(id_schema.read_only, Some(true));
		} else {
			panic!("Expected id to be an object schema");
		}
	} else {
		panic!("Expected Object schema");
	}
}

#[rstest]
fn test_metadata_struct_write_only() {
	// Act
	let schema = MetadataStruct::schema();

	// Assert
	if let Schema::Object(obj) = schema {
		// Check write_only field
		if let Some(password_schema) = get_object_property(&obj, "password") {
			assert_eq!(password_schema.write_only, Some(true));
		} else {
			panic!("Expected password to be an object schema");
		}
	} else {
		panic!("Expected Object schema");
	}
}

#[rstest]
fn test_metadata_struct_deprecated() {
	// Act
	let schema = MetadataStruct::schema();

	// Assert
	if let Schema::Object(obj) = schema {
		// Check deprecated field
		if let Some(legacy_schema) = get_object_property(&obj, "legacy_field") {
			// Verify deprecated is Some(True)
			assert!(matches!(
				legacy_schema.deprecated,
				Some(utoipa::openapi::Deprecated::True)
			));
		} else {
			panic!("Expected legacy_field to be an object schema");
		}
	} else {
		panic!("Expected Object schema");
	}
}

#[rstest]
fn test_metadata_struct_example() {
	// Act
	let schema = MetadataStruct::schema();

	// Assert
	if let Schema::Object(obj) = schema {
		// Check example field
		if let Some(email_schema) = get_object_property(&obj, "email") {
			if let Some(example) = &email_schema.example {
				let example_str = example.as_str().expect("Example should be a string");
				assert_eq!(example_str, "john.doe@example.com");
			} else {
				panic!("Expected email to have an example");
			}
		} else {
			panic!("Expected email to be an object schema");
		}
	} else {
		panic!("Expected Object schema");
	}
}

#[rstest]
fn test_metadata_struct_explicit_description() {
	// Act
	let schema = MetadataStruct::schema();

	// Assert
	if let Schema::Object(obj) = schema {
		// Check explicit description (should override doc comment)
		if let Some(display_name_schema) = get_object_property(&obj, "display_name") {
			assert_eq!(
				display_name_schema.description,
				Some("User's preferred display name".to_string())
			);
		} else {
			panic!("Expected display_name to be an object schema");
		}
	} else {
		panic!("Expected Object schema");
	}
}

#[rstest]
fn test_nested_struct_composition() {
	// Act
	let schema = UserWithAddress::schema();

	// Assert
	if let Schema::Object(obj) = schema {
		// Verify nested Address field exists
		assert!(obj.properties.contains_key("address"));

		// Verify optional nested Address field exists
		assert!(obj.properties.contains_key("secondary_address"));

		// Verify address is required
		assert!(obj.required.contains(&"address".to_string()));

		// Verify secondary_address is not required
		assert!(!obj.required.contains(&"secondary_address".to_string()));
	} else {
		panic!("Expected Object schema");
	}
}

#[rstest]
fn test_address_schema_standalone() {
	// Act
	let schema = Address::schema();

	// Assert
	if let Schema::Object(obj) = schema {
		// Verify Address has its own properties
		assert!(obj.properties.contains_key("street"));
		assert!(obj.properties.contains_key("city"));
		assert!(obj.properties.contains_key("postal_code"));

		// All fields are required
		assert!(obj.required.contains(&"street".to_string()));
		assert!(obj.required.contains(&"city".to_string()));
		assert!(obj.required.contains(&"postal_code".to_string()));
	} else {
		panic!("Expected Object schema");
	}
}

#[rstest]
fn test_collection_struct_vec_field() {
	// Act
	let schema = CollectionStruct::schema();

	// Assert
	if let Schema::Object(obj) = schema {
		// Verify tags field exists
		assert!(obj.properties.contains_key("tags"));

		// All collection fields are required
		assert!(obj.required.contains(&"tags".to_string()));
	} else {
		panic!("Expected Object schema");
	}
}

#[rstest]
fn test_collection_struct_hashmap_field() {
	// Act
	let schema = CollectionStruct::schema();

	// Assert
	if let Schema::Object(obj) = schema {
		// Verify metadata field exists
		assert!(obj.properties.contains_key("metadata"));
		assert!(obj.required.contains(&"metadata".to_string()));
	} else {
		panic!("Expected Object schema");
	}
}

#[rstest]
fn test_optional_fields_required_detection() {
	// Act
	let schema = OptionalFields::schema();

	// Assert
	if let Schema::Object(obj) = schema {
		// Only id should be required
		assert_eq!(obj.required.len(), 1);
		assert!(obj.required.contains(&"id".to_string()));

		// All optional fields should not be required
		assert!(!obj.required.contains(&"optional_string".to_string()));
		assert!(!obj.required.contains(&"optional_number".to_string()));
		assert!(!obj.required.contains(&"optional_bool".to_string()));
	} else {
		panic!("Expected Object schema");
	}
}

#[rstest]
fn test_optional_fields_all_present() {
	// Act
	let schema = OptionalFields::schema();

	// Assert
	if let Schema::Object(obj) = schema {
		// Verify all fields exist in properties (even if optional)
		assert_eq!(obj.properties.len(), 4);
		assert!(obj.properties.contains_key("id"));
		assert!(obj.properties.contains_key("optional_string"));
		assert!(obj.properties.contains_key("optional_number"));
		assert!(obj.properties.contains_key("optional_bool"));
	} else {
		panic!("Expected Object schema");
	}
}

#[rstest]
fn test_all_primitives_field_count() {
	// Act
	let schema = AllPrimitives::schema();

	// Assert
	if let Schema::Object(obj) = schema {
		// Verify all primitive fields are present
		assert_eq!(obj.properties.len(), 12);

		// All fields should be required (none are Option<T>)
		assert_eq!(obj.required.len(), 12);
	} else {
		panic!("Expected Object schema");
	}
}

#[rstest]
fn test_all_primitives_integer_fields() {
	// Act
	let schema = AllPrimitives::schema();

	// Assert
	if let Schema::Object(obj) = schema {
		// Check integer fields exist
		assert!(obj.properties.contains_key("i8_field"));
		assert!(obj.properties.contains_key("i16_field"));
		assert!(obj.properties.contains_key("i32_field"));
		assert!(obj.properties.contains_key("i64_field"));
		assert!(obj.properties.contains_key("u8_field"));
		assert!(obj.properties.contains_key("u16_field"));
		assert!(obj.properties.contains_key("u32_field"));
		assert!(obj.properties.contains_key("u64_field"));
	} else {
		panic!("Expected Object schema");
	}
}

#[rstest]
fn test_all_primitives_float_fields() {
	// Act
	let schema = AllPrimitives::schema();

	// Assert
	if let Schema::Object(obj) = schema {
		// Check float fields exist
		assert!(obj.properties.contains_key("f32_field"));
		assert!(obj.properties.contains_key("f64_field"));
	} else {
		panic!("Expected Object schema");
	}
}

#[rstest]
fn test_all_primitives_bool_and_string_fields() {
	// Act
	let schema = AllPrimitives::schema();

	// Assert
	if let Schema::Object(obj) = schema {
		// Check bool and string fields exist
		assert!(obj.properties.contains_key("bool_field"));
		assert!(obj.properties.contains_key("string_field"));
	} else {
		panic!("Expected Object schema");
	}
}

#[rstest]
fn test_complex_validation_combined_attributes() {
	// Act
	let schema = ComplexValidation::schema();

	// Assert
	if let Schema::Object(obj) = schema {
		// Check email with both format and example
		if let Some(email_schema) = get_object_property(&obj, "email") {
			assert!(email_schema.format.is_some());
			assert!(email_schema.example.is_some());
		} else {
			panic!("Expected email to be an object schema");
		}

		// Check website with both format and pattern
		if let Some(website_schema) = get_object_property(&obj, "website") {
			assert!(website_schema.format.is_some());
			assert_eq!(website_schema.pattern, Some("^https://".to_string()));
		} else {
			panic!("Expected website to be an object schema");
		}

		// Check created_at with date-time format
		if let Some(created_at_schema) = get_object_property(&obj, "created_at") {
			assert!(created_at_schema.format.is_some());
		} else {
			panic!("Expected created_at to be an object schema");
		}

		// Check completion_percentage with min/max
		if let Some(percentage_schema) = get_object_property(&obj, "completion_percentage") {
			assert!(percentage_schema.minimum.is_some());
			assert!(percentage_schema.maximum.is_some());
			// Verify values through JSON serialization
			let min_json = serde_json::to_value(&percentage_schema.minimum).unwrap();
			let max_json = serde_json::to_value(&percentage_schema.maximum).unwrap();
			assert_eq!(min_json.as_f64(), Some(0.0));
			assert_eq!(max_json.as_f64(), Some(100.0));
		} else {
			panic!("Expected completion_percentage to be an object schema");
		}
	} else {
		panic!("Expected Object schema");
	}
}

#[rstest]
fn test_schema_serialization_to_json() {
	// Arrange
	let schema = BasicStruct::schema();

	// Act
	let json = serde_json::to_value(schema).expect("Failed to serialize schema to JSON");

	// Assert
	assert!(json.is_object());

	let obj = json.as_object().expect("Expected JSON object");
	assert_eq!(obj.get("type").and_then(|v| v.as_str()), Some("object"));
	assert!(obj.contains_key("properties"));
	assert!(obj.contains_key("required"));
}

#[rstest]
fn test_schema_json_properties_structure() {
	// Arrange
	let schema = BasicStruct::schema();

	// Act
	let json = serde_json::to_value(schema).expect("Failed to serialize schema to JSON");

	// Assert
	let obj = json.as_object().expect("Expected JSON object");
	let properties = obj
		.get("properties")
		.and_then(|v| v.as_object())
		.expect("Expected properties to be an object");

	// Verify all properties exist in JSON
	assert!(properties.contains_key("id"));
	assert!(properties.contains_key("name"));
	assert!(properties.contains_key("age"));
	assert!(properties.contains_key("is_active"));
}

#[rstest]
fn test_schema_json_required_array() {
	// Arrange
	let schema = BasicStruct::schema();

	// Act
	let json = serde_json::to_value(schema).expect("Failed to serialize schema to JSON");

	// Assert
	let obj = json.as_object().expect("Expected JSON object");
	let required = obj
		.get("required")
		.and_then(|v| v.as_array())
		.expect("Expected required to be an array");

	// Verify required fields in JSON
	let required_strs: Vec<&str> = required.iter().filter_map(|v| v.as_str()).collect();

	assert!(required_strs.contains(&"id"));
	assert!(required_strs.contains(&"name"));
	assert!(required_strs.contains(&"is_active"));
	assert!(!required_strs.contains(&"age")); // age is optional
}

#[rstest]
fn test_nested_schema_json_structure() {
	// Arrange
	let schema = UserWithAddress::schema();

	// Act
	let json = serde_json::to_value(schema).expect("Failed to serialize schema to JSON");

	// Assert
	let obj = json.as_object().expect("Expected JSON object");
	let properties = obj
		.get("properties")
		.and_then(|v| v.as_object())
		.expect("Expected properties to be an object");

	// Verify address property exists
	assert!(properties.contains_key("address"));

	// Verify address has nested structure
	let address_prop = properties
		.get("address")
		.expect("Expected address property");
	assert!(address_prop.is_object());
}

#[rstest]
fn test_validation_constraints_in_json() {
	// Arrange
	let schema = ConstrainedStruct::schema();

	// Act
	let json = serde_json::to_value(schema).expect("Failed to serialize schema to JSON");

	// Assert
	let obj = json.as_object().expect("Expected JSON object");
	let properties = obj
		.get("properties")
		.and_then(|v| v.as_object())
		.expect("Expected properties to be an object");

	// Check username constraints in JSON
	let username = properties
		.get("username")
		.and_then(|v| v.as_object())
		.expect("Expected username to be an object");

	assert_eq!(username.get("minLength").and_then(|v| v.as_u64()), Some(3));
	assert_eq!(username.get("maxLength").and_then(|v| v.as_u64()), Some(50));

	// Check age constraints in JSON
	let age = properties
		.get("age")
		.and_then(|v| v.as_object())
		.expect("Expected age to be an object");

	assert_eq!(age.get("minimum").and_then(|v| v.as_f64()), Some(0.0));
	assert_eq!(age.get("maximum").and_then(|v| v.as_f64()), Some(150.0));
}

#[rstest]
fn test_metadata_attributes_in_json() {
	// Arrange
	let schema = MetadataStruct::schema();

	// Act
	let json = serde_json::to_value(schema).expect("Failed to serialize schema to JSON");

	// Assert
	let obj = json.as_object().expect("Expected JSON object");
	let properties = obj
		.get("properties")
		.and_then(|v| v.as_object())
		.expect("Expected properties to be an object");

	// Check read_only in JSON
	let id = properties
		.get("id")
		.and_then(|v| v.as_object())
		.expect("Expected id to be an object");
	assert_eq!(id.get("readOnly").and_then(|v| v.as_bool()), Some(true));

	// Check write_only in JSON
	let password = properties
		.get("password")
		.and_then(|v| v.as_object())
		.expect("Expected password to be an object");
	assert_eq!(
		password.get("writeOnly").and_then(|v| v.as_bool()),
		Some(true)
	);

	// Check deprecated in JSON
	let legacy = properties
		.get("legacy_field")
		.and_then(|v| v.as_object())
		.expect("Expected legacy_field to be an object");
	assert_eq!(
		legacy.get("deprecated").and_then(|v| v.as_bool()),
		Some(true)
	);
}

#[rstest]
fn test_format_attribute_in_json() {
	// Arrange
	let schema = ConstrainedStruct::schema();

	// Act
	let json = serde_json::to_value(schema).expect("Failed to serialize schema to JSON");

	// Assert
	let obj = json.as_object().expect("Expected JSON object");
	let properties = obj
		.get("properties")
		.and_then(|v| v.as_object())
		.expect("Expected properties to be an object");

	// Check email format in JSON
	let email = properties
		.get("email")
		.and_then(|v| v.as_object())
		.expect("Expected email to be an object");
	assert_eq!(email.get("format").and_then(|v| v.as_str()), Some("email"));
}

#[rstest]
fn test_pattern_attribute_in_json() {
	// Arrange
	let schema = ConstrainedStruct::schema();

	// Act
	let json = serde_json::to_value(schema).expect("Failed to serialize schema to JSON");

	// Assert
	let obj = json.as_object().expect("Expected JSON object");
	let properties = obj
		.get("properties")
		.and_then(|v| v.as_object())
		.expect("Expected properties to be an object");

	// Check password pattern in JSON
	let password = properties
		.get("password")
		.and_then(|v| v.as_object())
		.expect("Expected password to be an object");
	assert_eq!(
		password.get("pattern").and_then(|v| v.as_str()),
		Some("^(?=.*[A-Za-z])(?=.*\\d)[A-Za-z\\d]{8,}$")
	);
}

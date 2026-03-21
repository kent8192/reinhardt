//! Tests for `#[derive(Schema)]` macro
//!
//! These tests validate the OpenAPI schema derivation macro functionality
//! including struct schemas, enum schemas, field attributes, and serde integration.
//!
//! Moved from `reinhardt-openapi-macros/tests/` to avoid circular dev-dependencies.

use reinhardt_rest::openapi::{Schema, ToSchema};
use rstest::rstest;
use serde::{Deserialize, Serialize};
use utoipa::openapi::{
	Deprecated,
	schema::{SchemaFormat, SchemaType, Type},
};

// ============================================================================
// Struct Schema Tests
// ============================================================================

#[rstest]
fn test_simple_struct_schema_generation() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema)]
	struct User {
		id: i64,
		name: String,
	}

	// Act
	let schema = User::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			assert!(matches!(obj.schema_type, SchemaType::Type(Type::Object)));
			assert_eq!(obj.properties.len(), 2);
			assert!(obj.properties.contains_key("id"));
			assert!(obj.properties.contains_key("name"));
			assert_eq!(obj.required, vec!["id", "name"]);
		}
		_ => panic!("Expected Object schema"),
	}

	assert_eq!(User::schema_name(), Some("User".to_string()));
}

#[rstest]
fn test_optional_fields_not_required() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema)]
	struct User {
		id: i64,
		name: String,
		email: Option<String>,
		age: Option<i32>,
	}

	// Act
	let schema = User::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			// Only non-Option fields should be required
			assert_eq!(obj.required, vec!["id", "name"]);
			assert_eq!(obj.properties.len(), 4);
		}
		_ => panic!("Expected Object schema"),
	}
}

#[rstest]
fn test_field_with_description() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema)]
	struct User {
		#[schema(description = "Unique user identifier")]
		id: i64,

		name: String,
	}

	// Act
	let schema = User::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			if let Some(id_schema) = obj.properties.get("id") {
				match id_schema {
					utoipa::openapi::RefOr::T(Schema::Object(id_obj)) => {
						assert_eq!(
							id_obj.description,
							Some("Unique user identifier".to_string())
						);
					}
					_ => panic!("Expected Object schema for id field"),
				}
			} else {
				panic!("id field not found in properties");
			}
		}
		_ => panic!("Expected Object schema"),
	}
}

#[rstest]
fn test_field_with_example() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema)]
	struct User {
		#[schema(example = "42")]
		id: i64,

		#[schema(example = "John Doe")]
		name: String,
	}

	// Act
	let schema = User::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			if let Some(id_schema) = obj.properties.get("id") {
				match id_schema {
					utoipa::openapi::RefOr::T(Schema::Object(id_obj)) => {
						assert!(id_obj.example.is_some());
					}
					_ => panic!("Expected Object schema for id field"),
				}
			}
		}
		_ => panic!("Expected Object schema"),
	}
}

#[rstest]
fn test_field_with_format() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema)]
	struct User {
		#[schema(format = "email")]
		email: String,

		#[schema(format = "uri")]
		website: String,
	}

	// Act
	let schema = User::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			if let Some(email_schema) = obj.properties.get("email") {
				match email_schema {
					utoipa::openapi::RefOr::T(Schema::Object(email_obj)) => {
						assert!(matches!(
							email_obj.format,
							Some(SchemaFormat::Custom(ref s)) if s == "email"
						));
					}
					_ => panic!("Expected Object schema for email field"),
				}
			}
		}
		_ => panic!("Expected Object schema"),
	}
}

#[rstest]
fn test_field_with_read_only() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema)]
	struct User {
		#[schema(read_only)]
		id: i64,

		name: String,
	}

	// Act
	let schema = User::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			if let Some(id_schema) = obj.properties.get("id") {
				match id_schema {
					utoipa::openapi::RefOr::T(Schema::Object(id_obj)) => {
						assert_eq!(id_obj.read_only, Some(true));
					}
					_ => panic!("Expected Object schema for id field"),
				}
			}
		}
		_ => panic!("Expected Object schema"),
	}
}

#[rstest]
fn test_field_with_write_only() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema)]
	struct CreateUser {
		name: String,

		#[schema(write_only)]
		password: String,
	}

	// Act
	let schema = CreateUser::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			if let Some(password_schema) = obj.properties.get("password") {
				match password_schema {
					utoipa::openapi::RefOr::T(Schema::Object(password_obj)) => {
						assert_eq!(password_obj.write_only, Some(true));
					}
					_ => panic!("Expected Object schema for password field"),
				}
			}
		}
		_ => panic!("Expected Object schema"),
	}
}

#[rstest]
fn test_field_with_deprecated() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema)]
	struct User {
		id: i64,
		name: String,

		#[schema(deprecated)]
		old_field: String,
	}

	// Act
	let schema = User::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			if let Some(old_field_schema) = obj.properties.get("old_field") {
				match old_field_schema {
					utoipa::openapi::RefOr::T(Schema::Object(old_field_obj)) => {
						assert!(matches!(old_field_obj.deprecated, Some(Deprecated::True)));
					}
					_ => panic!("Expected Object schema for old_field"),
				}
			}
		}
		_ => panic!("Expected Object schema"),
	}
}

#[rstest]
fn test_field_with_numeric_constraints() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema)]
	struct Product {
		#[schema(minimum = 0, maximum = 100)]
		rating: i32,

		#[schema(minimum = 0)]
		price: f64,
	}

	// Act
	let schema = Product::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			if let Some(rating_schema) = obj.properties.get("rating") {
				match rating_schema {
					utoipa::openapi::RefOr::T(Schema::Object(rating_obj)) => {
						assert!(rating_obj.minimum.is_some());
						assert!(rating_obj.maximum.is_some());
					}
					_ => panic!("Expected Object schema for rating field"),
				}
			}
		}
		_ => panic!("Expected Object schema"),
	}
}

#[rstest]
fn test_field_with_string_length_constraints() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema)]
	struct User {
		#[schema(min_length = 3, max_length = 50)]
		username: String,

		#[schema(min_length = 8)]
		password: String,
	}

	// Act
	let schema = User::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			if let Some(username_schema) = obj.properties.get("username") {
				match username_schema {
					utoipa::openapi::RefOr::T(Schema::Object(username_obj)) => {
						assert_eq!(username_obj.min_length, Some(3));
						assert_eq!(username_obj.max_length, Some(50));
					}
					_ => panic!("Expected Object schema for username field"),
				}
			}
		}
		_ => panic!("Expected Object schema"),
	}
}

#[rstest]
fn test_field_with_pattern() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema)]
	struct User {
		#[schema(pattern = "^[a-zA-Z0-9_]+$")]
		username: String,
	}

	// Act
	let schema = User::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			if let Some(username_schema) = obj.properties.get("username") {
				match username_schema {
					utoipa::openapi::RefOr::T(Schema::Object(username_obj)) => {
						assert_eq!(username_obj.pattern, Some("^[a-zA-Z0-9_]+$".to_string()));
					}
					_ => panic!("Expected Object schema for username field"),
				}
			}
		}
		_ => panic!("Expected Object schema"),
	}
}

#[rstest]
fn test_doc_comments_as_description() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema)]
	struct User {
		/// User's unique identifier
		id: i64,

		/// Full name of the user
		name: String,
	}

	// Act
	let schema = User::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			if let Some(id_schema) = obj.properties.get("id") {
				match id_schema {
					utoipa::openapi::RefOr::T(Schema::Object(id_obj)) => {
						assert_eq!(
							id_obj.description,
							Some("User's unique identifier".to_string())
						);
					}
					_ => panic!("Expected Object schema for id field"),
				}
			}
		}
		_ => panic!("Expected Object schema"),
	}
}

#[rstest]
fn test_explicit_description_overrides_doc_comment() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema)]
	struct User {
		/// This doc comment will be overridden
		#[schema(description = "Explicit description")]
		id: i64,
	}

	// Act
	let schema = User::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			if let Some(id_schema) = obj.properties.get("id") {
				match id_schema {
					utoipa::openapi::RefOr::T(Schema::Object(id_obj)) => {
						assert_eq!(id_obj.description, Some("Explicit description".to_string()));
					}
					_ => panic!("Expected Object schema for id field"),
				}
			}
		}
		_ => panic!("Expected Object schema"),
	}
}

#[rstest]
fn test_combined_attributes() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema)]
	struct User {
		/// User's email address
		#[schema(
			format = "email",
			example = "user@example.com",
			min_length = 5,
			max_length = 255
		)]
		email: String,
	}

	// Act
	let schema = User::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			if let Some(email_schema) = obj.properties.get("email") {
				match email_schema {
					utoipa::openapi::RefOr::T(Schema::Object(email_obj)) => {
						assert_eq!(
							email_obj.description,
							Some("User's email address".to_string())
						);
						assert!(matches!(
							email_obj.format,
							Some(SchemaFormat::Custom(ref s)) if s == "email"
						));
						assert!(email_obj.example.is_some());
						assert_eq!(email_obj.min_length, Some(5));
						assert_eq!(email_obj.max_length, Some(255));
					}
					_ => panic!("Expected Object schema for email field"),
				}
			}
		}
		_ => panic!("Expected Object schema"),
	}
}

#[rstest]
fn test_complex_struct_with_all_features() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema)]
	struct User {
		/// Unique identifier
		#[schema(read_only, example = "123")]
		id: i64,

		/// Username for login
		#[schema(
			min_length = 3,
			max_length = 50,
			pattern = "^[a-zA-Z0-9_]+$",
			example = "john_doe"
		)]
		username: String,

		/// User's email address
		#[schema(format = "email", example = "john@example.com")]
		email: String,

		/// User's age (optional)
		#[schema(minimum = 0, maximum = 150)]
		age: Option<i32>,

		/// Password (write-only)
		#[schema(write_only, min_length = 8)]
		password: String,

		/// Deprecated field
		#[schema(deprecated)]
		legacy_field: Option<String>,
	}

	// Act
	let schema = User::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			// Verify structure
			assert_eq!(obj.properties.len(), 6);

			// Only non-Option and non-write-only fields should be required
			assert!(obj.required.contains(&"id".to_string()));
			assert!(obj.required.contains(&"username".to_string()));
			assert!(obj.required.contains(&"email".to_string()));
			assert!(obj.required.contains(&"password".to_string()));

			// Verify id field (read-only)
			if let Some(utoipa::openapi::RefOr::T(Schema::Object(id_obj))) =
				obj.properties.get("id")
			{
				assert_eq!(id_obj.read_only, Some(true));
				assert!(id_obj.example.is_some());
			}

			// Verify username field (with pattern and length constraints)
			if let Some(utoipa::openapi::RefOr::T(Schema::Object(username_obj))) =
				obj.properties.get("username")
			{
				assert_eq!(username_obj.min_length, Some(3));
				assert_eq!(username_obj.max_length, Some(50));
				assert_eq!(username_obj.pattern, Some("^[a-zA-Z0-9_]+$".to_string()));
			}

			// Verify email field (with format)
			if let Some(utoipa::openapi::RefOr::T(Schema::Object(email_obj))) =
				obj.properties.get("email")
			{
				assert!(matches!(
					email_obj.format,
					Some(SchemaFormat::Custom(ref s)) if s == "email"
				));
			}

			// Verify password field (write-only)
			if let Some(utoipa::openapi::RefOr::T(Schema::Object(password_obj))) =
				obj.properties.get("password")
			{
				assert_eq!(password_obj.write_only, Some(true));
				assert_eq!(password_obj.min_length, Some(8));
			}

			// Verify deprecated field
			if let Some(utoipa::openapi::RefOr::T(Schema::Object(legacy_obj))) =
				obj.properties.get("legacy_field")
			{
				assert!(matches!(legacy_obj.deprecated, Some(Deprecated::True)));
			}
		}
		_ => panic!("Expected Object schema"),
	}
}

// ============================================================================
// Enum Schema Tests
// ============================================================================

#[rstest]
fn test_simple_unit_enum_generates_string_schema() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema)]
	enum Status {
		Active,
		Inactive,
		Pending,
	}

	// Act
	let schema = Status::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			assert!(matches!(obj.schema_type, SchemaType::Type(Type::String)));
			assert!(obj.enum_values.is_some());
			let values = obj.enum_values.as_ref().unwrap();
			assert_eq!(values.len(), 3);
		}
		_ => panic!("Expected Object schema with string type for simple enum"),
	}

	assert_eq!(Status::schema_name(), Some("Status".to_string()));
}

#[rstest]
fn test_internally_tagged_enum() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema, Serialize, Deserialize)]
	#[serde(tag = "type")]
	enum Event {
		Created { id: i64 },
		Updated { id: i64, changes: Vec<String> },
	}

	// Act
	let schema = Event::schema();

	// Assert
	match schema {
		Schema::OneOf(one_of) => {
			assert_eq!(one_of.items.len(), 2);
			assert!(one_of.discriminator.is_some());
			assert_eq!(one_of.title, Some("Event".to_string()));
		}
		_ => panic!("Expected OneOf schema for internally tagged enum"),
	}
}

#[rstest]
fn test_adjacently_tagged_enum() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema, Serialize, Deserialize)]
	#[serde(tag = "t", content = "c")]
	enum Message {
		Text { content: String },
		Image { url: String },
	}

	// Act
	let schema = Message::schema();

	// Assert
	match schema {
		Schema::OneOf(one_of) => {
			assert_eq!(one_of.items.len(), 2);
			assert!(one_of.discriminator.is_some());
		}
		_ => panic!("Expected OneOf schema for adjacently tagged enum"),
	}
}

#[rstest]
fn test_untagged_enum() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema, Serialize, Deserialize)]
	#[serde(untagged)]
	enum Value {
		Str { text: String },
		Num { value: i64 },
	}

	// Act
	let schema = Value::schema();

	// Assert
	match schema {
		Schema::OneOf(one_of) => {
			assert_eq!(one_of.items.len(), 2);
			// Untagged enums should not have a discriminator
			assert!(one_of.discriminator.is_none());
		}
		_ => panic!("Expected OneOf schema for untagged enum"),
	}
}

#[rstest]
fn test_externally_tagged_enum_with_struct_variants() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema)]
	enum Shape {
		Circle { radius: f64 },
		Rectangle { width: f64, height: f64 },
	}

	// Act
	let schema = Shape::schema();

	// Assert
	match schema {
		Schema::OneOf(one_of) => {
			assert_eq!(one_of.items.len(), 2);
			assert_eq!(one_of.title, Some("Shape".to_string()));
			// External tagging should not have discriminator
			assert!(one_of.discriminator.is_none());
		}
		_ => panic!("Expected OneOf schema for externally tagged enum with struct variants"),
	}
}

#[rstest]
fn test_enum_with_newtype_variant() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema)]
	enum Wrapper {
		Int(i32),
		Str(String),
	}

	// Act
	let schema = Wrapper::schema();

	// Assert
	match schema {
		Schema::OneOf(one_of) => {
			assert_eq!(one_of.items.len(), 2);
		}
		_ => panic!("Expected OneOf schema for enum with newtype variants"),
	}
}

#[rstest]
fn test_enum_with_serde_rename() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema, Serialize, Deserialize)]
	enum Status {
		#[serde(rename = "ACTIVE")]
		Active,
		#[serde(rename = "INACTIVE")]
		Inactive,
	}

	// Act
	let schema = Status::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			assert!(obj.enum_values.is_some());
			let values = obj.enum_values.as_ref().unwrap();
			assert_eq!(values.len(), 2);
			// Values should be renamed
			assert!(values.contains(&serde_json::Value::String("ACTIVE".to_string())));
			assert!(values.contains(&serde_json::Value::String("INACTIVE".to_string())));
		}
		_ => panic!("Expected Object schema with renamed enum values"),
	}
}

#[rstest]
fn test_enum_with_serde_rename_all() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema, Serialize, Deserialize)]
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
			assert!(obj.enum_values.is_some());
			let values = obj.enum_values.as_ref().unwrap();
			assert_eq!(values.len(), 3);
			assert!(values.contains(&serde_json::Value::String("super_admin".to_string())));
			assert!(values.contains(&serde_json::Value::String("regular_user".to_string())));
			assert!(values.contains(&serde_json::Value::String("guest_user".to_string())));
		}
		_ => panic!("Expected Object schema with snake_case enum values"),
	}
}

#[rstest]
fn test_enum_with_skip_variant() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema, Serialize, Deserialize)]
	enum Mode {
		Normal,
		#[serde(skip)]
		Internal,
		Debug,
	}

	// Act
	let schema = Mode::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			assert!(obj.enum_values.is_some());
			let values = obj.enum_values.as_ref().unwrap();
			// Internal should be skipped
			assert_eq!(values.len(), 2);
			assert!(values.contains(&serde_json::Value::String("Normal".to_string())));
			assert!(values.contains(&serde_json::Value::String("Debug".to_string())));
			assert!(!values.contains(&serde_json::Value::String("Internal".to_string())));
		}
		_ => panic!("Expected Object schema with skipped variant excluded"),
	}
}

#[rstest]
fn test_mixed_variant_types() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema)]
	enum Data {
		Empty,
		Single(i32),
		Pair(i32, i32),
		Named { x: i32, y: i32 },
	}

	// Act
	let schema = Data::schema();

	// Assert
	match schema {
		Schema::OneOf(one_of) => {
			// All 4 variants should be present
			assert_eq!(one_of.items.len(), 4);
			assert_eq!(one_of.title, Some("Data".to_string()));
		}
		_ => panic!("Expected OneOf schema for mixed variant enum"),
	}
}

#[rstest]
fn test_internally_tagged_with_unit_variant() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema, Serialize, Deserialize)]
	#[serde(tag = "kind")]
	enum Action {
		Start,
		Stop,
		Pause { duration: i32 },
	}

	// Act
	let schema = Action::schema();

	// Assert
	match schema {
		Schema::OneOf(one_of) => {
			assert_eq!(one_of.items.len(), 3);
			assert!(one_of.discriminator.is_some());
		}
		_ => panic!("Expected OneOf schema for internally tagged enum with unit variants"),
	}
}

// ============================================================================
// Container Attribute Tests (Fixes #2441)
// ============================================================================

#[rstest]
fn test_container_title_attribute() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema)]
	#[schema(title = "User Account")]
	struct UserAccount {
		id: i64,
		name: String,
	}

	// Act
	let schema = UserAccount::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			assert_eq!(obj.title, Some("User Account".to_string()));
		}
		_ => panic!("Expected Object schema"),
	}
}

#[rstest]
fn test_container_description_attribute() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema)]
	#[schema(description = "A user account in the system")]
	struct UserProfile {
		id: i64,
		name: String,
	}

	// Act
	let schema = UserProfile::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			assert_eq!(
				obj.description,
				Some("A user account in the system".to_string())
			);
		}
		_ => panic!("Expected Object schema"),
	}
}

#[rstest]
fn test_container_description_overrides_doc_comment() {
	// Arrange
	/// This doc comment should be overridden
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema)]
	#[schema(description = "Explicit container description")]
	struct OverrideDoc {
		id: i64,
	}

	// Act
	let schema = OverrideDoc::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			assert_eq!(
				obj.description,
				Some("Explicit container description".to_string())
			);
		}
		_ => panic!("Expected Object schema"),
	}
}

#[rstest]
fn test_container_example_attribute() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema)]
	#[schema(example = "{\"id\": 1, \"name\": \"Alice\"}")]
	struct ExampleUser {
		id: i64,
		name: String,
	}

	// Act
	let schema = ExampleUser::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			assert!(obj.example.is_some());
		}
		_ => panic!("Expected Object schema"),
	}
}

#[rstest]
fn test_container_deprecated_attribute() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema)]
	#[schema(deprecated)]
	struct LegacyData {
		id: i64,
	}

	// Act
	let schema = LegacyData::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			assert!(matches!(obj.deprecated, Some(Deprecated::True)));
		}
		_ => panic!("Expected Object schema"),
	}
}

#[rstest]
fn test_container_nullable_attribute() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema)]
	#[schema(nullable)]
	struct NullableData {
		id: i64,
	}

	// Act
	let schema = NullableData::schema();
	let json = serde_json::to_value(&schema).expect("Failed to serialize");

	// Assert: type should be ["object", "null"]
	let type_value = json.get("type").expect("Expected 'type' in schema");
	assert!(
		type_value.is_array(),
		"nullable schema should have array type"
	);
	let types: Vec<&str> = type_value
		.as_array()
		.unwrap()
		.iter()
		.filter_map(|v| v.as_str())
		.collect();
	assert!(types.contains(&"object"), "Should contain 'object' type");
	assert!(types.contains(&"null"), "Should contain 'null' type");
}

#[rstest]
fn test_container_combined_attributes() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema)]
	#[schema(
		title = "Combined User",
		description = "A user with all container attributes",
		deprecated
	)]
	struct CombinedUser {
		id: i64,
		name: String,
	}

	// Act
	let schema = CombinedUser::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			assert_eq!(obj.title, Some("Combined User".to_string()));
			assert_eq!(
				obj.description,
				Some("A user with all container attributes".to_string())
			);
			assert!(matches!(obj.deprecated, Some(Deprecated::True)));
		}
		_ => panic!("Expected Object schema"),
	}
}

// ============================================================================
// New Field Attribute Tests (Fixes #2441)
// ============================================================================

#[rstest]
fn test_field_exclusive_minimum_maximum() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema)]
	struct NumericBounds {
		#[schema(exclusive_minimum = 0, exclusive_maximum = 100)]
		value: i32,
	}

	// Act
	let schema = NumericBounds::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			if let Some(utoipa::openapi::RefOr::T(Schema::Object(value_obj))) =
				obj.properties.get("value")
			{
				assert!(
					value_obj.exclusive_minimum.is_some(),
					"exclusive_minimum should be set"
				);
				assert!(
					value_obj.exclusive_maximum.is_some(),
					"exclusive_maximum should be set"
				);
			} else {
				panic!("Expected Object schema for value field");
			}
		}
		_ => panic!("Expected Object schema"),
	}
}

#[rstest]
fn test_field_multiple_of() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema)]
	struct MultipleOfTest {
		#[schema(multiple_of = 5)]
		count: i32,
	}

	// Act
	let schema = MultipleOfTest::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			if let Some(utoipa::openapi::RefOr::T(Schema::Object(count_obj))) =
				obj.properties.get("count")
			{
				assert!(count_obj.multiple_of.is_some(), "multiple_of should be set");
				let mul_json = serde_json::to_value(&count_obj.multiple_of).unwrap();
				assert_eq!(mul_json.as_f64(), Some(5.0));
			} else {
				panic!("Expected Object schema for count field");
			}
		}
		_ => panic!("Expected Object schema"),
	}
}

#[rstest]
fn test_field_min_max_items() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema)]
	struct ItemBounds {
		#[schema(min_items = 1, max_items = 10)]
		tags: Vec<String>,
	}

	// Act
	let schema = ItemBounds::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			if let Some(utoipa::openapi::RefOr::T(Schema::Array(arr))) = obj.properties.get("tags")
			{
				assert_eq!(arr.min_items, Some(1));
				assert_eq!(arr.max_items, Some(10));
			} else {
				panic!("Expected Array schema for tags field");
			}
		}
		_ => panic!("Expected Object schema"),
	}
}

#[rstest]
fn test_field_unique_items() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema)]
	struct UniqueSet {
		#[schema(unique_items)]
		items: Vec<String>,
	}

	// Act
	let schema = UniqueSet::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			if let Some(utoipa::openapi::RefOr::T(Schema::Array(arr))) = obj.properties.get("items")
			{
				assert!(arr.unique_items, "unique_items should be true");
			} else {
				panic!("Expected Array schema for items field");
			}
		}
		_ => panic!("Expected Object schema"),
	}
}

#[rstest]
fn test_field_nullable() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema)]
	struct NullableField {
		#[schema(nullable)]
		value: String,
	}

	// Act
	let schema = NullableField::schema();
	let json = serde_json::to_value(&schema).expect("Failed to serialize");

	// Assert: the value field should have type ["string", "null"]
	let properties = json.get("properties").expect("Expected properties");
	let value_prop = properties.get("value").expect("Expected value property");
	let type_value = value_prop
		.get("type")
		.expect("Expected 'type' in value property");
	assert!(
		type_value.is_array(),
		"nullable field should have array type"
	);
	let types: Vec<&str> = type_value
		.as_array()
		.unwrap()
		.iter()
		.filter_map(|v| v.as_str())
		.collect();
	assert!(types.contains(&"null"), "Should contain 'null' type");
}

#[rstest]
fn test_field_default_value() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema)]
	struct WithDefault {
		#[schema(default_value = "active")]
		status: String,
	}

	// Act
	let schema = WithDefault::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			if let Some(utoipa::openapi::RefOr::T(Schema::Object(status_obj))) =
				obj.properties.get("status")
			{
				assert!(status_obj.default.is_some(), "default should be set");
			} else {
				panic!("Expected Object schema for status field");
			}
		}
		_ => panic!("Expected Object schema"),
	}
}

#[rstest]
fn test_field_title() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema)]
	struct WithTitle {
		#[schema(title = "User Identifier")]
		id: i64,
	}

	// Act
	let schema = WithTitle::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			if let Some(utoipa::openapi::RefOr::T(Schema::Object(id_obj))) =
				obj.properties.get("id")
			{
				assert_eq!(id_obj.title, Some("User Identifier".to_string()));
			} else {
				panic!("Expected Object schema for id field");
			}
		}
		_ => panic!("Expected Object schema"),
	}
}

#[rstest]
fn test_field_attributes_in_json_output() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema)]
	struct JsonVerification {
		#[schema(exclusive_minimum = 0, exclusive_maximum = 100, multiple_of = 5)]
		score: i32,

		#[schema(title = "Tag List", min_items = 1, max_items = 20, unique_items)]
		tags: Vec<String>,

		#[schema(default_value = "pending", title = "Status")]
		status: String,
	}

	// Act
	let schema = JsonVerification::schema();
	let json = serde_json::to_value(&schema).expect("Failed to serialize");

	// Assert
	let props = json.get("properties").unwrap().as_object().unwrap();

	// Check score field
	let score = props.get("score").unwrap().as_object().unwrap();
	assert_eq!(
		score.get("exclusiveMinimum").and_then(|v| v.as_f64()),
		Some(0.0)
	);
	assert_eq!(
		score.get("exclusiveMaximum").and_then(|v| v.as_f64()),
		Some(100.0)
	);
	assert_eq!(score.get("multipleOf").and_then(|v| v.as_f64()), Some(5.0));

	// Check tags field
	let tags = props.get("tags").unwrap().as_object().unwrap();
	assert_eq!(tags.get("minItems").and_then(|v| v.as_u64()), Some(1));
	assert_eq!(tags.get("maxItems").and_then(|v| v.as_u64()), Some(20));
	assert_eq!(
		tags.get("uniqueItems").and_then(|v| v.as_bool()),
		Some(true)
	);

	// Check status field
	let status = props.get("status").unwrap().as_object().unwrap();
	assert_eq!(status.get("title").and_then(|v| v.as_str()), Some("Status"));
	assert!(status.get("default").is_some());
}

#[rstest]
fn test_enum_container_attributes() {
	// Arrange
	// Allow: compile-time-only test fixture for derive macro verification
	#[allow(dead_code)]
	#[derive(Schema)]
	#[schema(title = "User Status", description = "The status of a user account")]
	enum UserStatus {
		Active,
		Inactive,
		Suspended,
	}

	// Act
	let schema = UserStatus::schema();

	// Assert
	match schema {
		Schema::Object(obj) => {
			assert_eq!(obj.title, Some("User Status".to_string()));
			assert_eq!(
				obj.description,
				Some("The status of a user account".to_string())
			);
		}
		_ => panic!("Expected Object schema for simple enum with container attrs"),
	}
}

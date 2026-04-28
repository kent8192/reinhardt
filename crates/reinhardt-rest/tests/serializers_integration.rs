// Integration tests for reinhardt-rest serializers module
//
// Tests cover:
// - Field validation (CharField, IntegerField, FloatField, BooleanField, EmailField, URLField)
// - Data conversion and serialization (JsonSerializer)
// - Serializer error types
// - Nested serializer configuration
// - ModelSerializer meta configuration
// - Relation fields
// - Validator types (UniqueValidator, UniqueTogetherValidator)
// - SerializerMethodField and MethodFieldRegistry
// - DatabaseValidatorError conversion

use reinhardt_db::orm::{FieldSelector, Model};
use reinhardt_rest::serializers::{
	BooleanField, CharField, EmailField, FieldError, FloatField, HyperlinkedRelatedField,
	IntegerField, JsonSerializer, ManyRelatedField, ModelLevelValidator, ModelSerializer,
	PrimaryKeyRelatedField, RelationField, Serializer, SerializerError, SerializerMethodField,
	SlugRelatedField, StringRelatedField, URLField, UniqueTogetherValidator, UniqueValidator,
	ValidationError, ValidatorError, WritableNestedSerializer,
	introspection::{FieldInfo, FieldIntrospector},
	meta::{DefaultMeta, MetaConfig, SerializerMeta},
	method_field::{MethodFieldError, MethodFieldRegistry},
	nested_config::{NestedFieldConfig, NestedSerializerConfig},
	validators::DatabaseValidatorError,
};
use rstest::rstest;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashMap};

// ============================================================
// Helper types for tests
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestUser {
	id: Option<i64>,
	username: String,
	email: String,
}

#[derive(Debug, Clone)]
struct TestUserFields;

impl FieldSelector for TestUserFields {
	fn with_alias(self, _alias: &str) -> Self {
		self
	}
}

impl Model for TestUser {
	type PrimaryKey = i64;
	type Fields = TestUserFields;

	fn table_name() -> &'static str {
		"test_users"
	}

	fn new_fields() -> Self::Fields {
		TestUserFields
	}

	fn primary_key(&self) -> Option<Self::PrimaryKey> {
		self.id
	}

	fn set_primary_key(&mut self, value: Self::PrimaryKey) {
		self.id = Some(value);
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestPost {
	id: Option<i64>,
	title: String,
	author_id: i64,
}

#[derive(Debug, Clone)]
struct TestPostFields;

impl FieldSelector for TestPostFields {
	fn with_alias(self, _alias: &str) -> Self {
		self
	}
}

impl Model for TestPost {
	type PrimaryKey = i64;
	type Fields = TestPostFields;

	fn table_name() -> &'static str {
		"test_posts"
	}

	fn new_fields() -> Self::Fields {
		TestPostFields
	}

	fn primary_key(&self) -> Option<Self::PrimaryKey> {
		self.id
	}

	fn set_primary_key(&mut self, value: Self::PrimaryKey) {
		self.id = Some(value);
	}
}

// ============================================================
// CharField tests
// ============================================================

#[rstest]
fn char_field_valid_string() {
	// Arrange
	let field = CharField::new().min_length(3).max_length(10);

	// Act
	let result = field.validate("hello");

	// Assert
	assert_eq!(result, Ok(()));
}

#[rstest]
fn char_field_too_short() {
	// Arrange
	let field = CharField::new().min_length(5);

	// Act
	let result = field.validate("hi");

	// Assert
	assert!(matches!(result, Err(FieldError::TooShort(5))));
}

#[rstest]
fn char_field_too_long() {
	// Arrange
	let field = CharField::new().max_length(5);

	// Act
	let result = field.validate("hello world");

	// Assert
	assert!(matches!(result, Err(FieldError::TooLong(5))));
}

#[rstest]
fn char_field_blank_rejected_by_default() {
	// Arrange
	let field = CharField::new();

	// Act
	let result = field.validate("");

	// Assert
	assert!(result.is_err());
}

#[rstest]
fn char_field_blank_allowed_when_configured() {
	// Arrange
	let field = CharField::new().allow_blank(true);

	// Act
	let result = field.validate("");

	// Assert
	assert_eq!(result, Ok(()));
}

#[rstest]
fn char_field_at_min_boundary() {
	// Arrange
	let field = CharField::new().min_length(3);

	// Act
	let result = field.validate("abc");

	// Assert
	assert_eq!(result, Ok(()));
}

#[rstest]
fn char_field_at_max_boundary() {
	// Arrange
	let field = CharField::new().max_length(5);

	// Act
	let result = field.validate("hello");

	// Assert
	assert_eq!(result, Ok(()));
}

// ============================================================
// IntegerField tests
// ============================================================

#[rstest]
fn integer_field_valid_value() {
	// Arrange
	let field = IntegerField::new().min_value(0).max_value(100);

	// Act
	let result = field.validate(50);

	// Assert
	assert_eq!(result, Ok(()));
}

#[rstest]
fn integer_field_below_min() {
	// Arrange
	let field = IntegerField::new().min_value(0);

	// Act
	let result = field.validate(-1);

	// Assert
	assert!(matches!(result, Err(FieldError::TooSmall(0))));
}

#[rstest]
fn integer_field_above_max() {
	// Arrange
	let field = IntegerField::new().max_value(100);

	// Act
	let result = field.validate(101);

	// Assert
	assert!(matches!(result, Err(FieldError::TooLarge(100))));
}

#[rstest]
fn integer_field_at_boundary_values() {
	// Arrange
	let field = IntegerField::new().min_value(-10).max_value(10);

	// Assert
	assert!(field.validate(-10).is_ok());
	assert!(field.validate(10).is_ok());
	assert!(field.validate(-11).is_err());
	assert!(field.validate(11).is_err());
}

// ============================================================
// FloatField tests
// ============================================================

#[rstest]
fn float_field_valid_value() {
	// Arrange
	let field = FloatField::new().min_value(0.0).max_value(1.0);

	// Act
	let result = field.validate(0.5);

	// Assert
	assert_eq!(result, Ok(()));
}

#[rstest]
fn float_field_below_min() {
	// Arrange
	let field = FloatField::new().min_value(0.0);

	// Act
	let result = field.validate(-0.1);

	// Assert
	assert!(matches!(result, Err(FieldError::TooSmallFloat(_))));
}

#[rstest]
fn float_field_above_max() {
	// Arrange
	let field = FloatField::new().max_value(1.0);

	// Act
	let result = field.validate(1.1);

	// Assert
	assert!(matches!(result, Err(FieldError::TooLargeFloat(_))));
}

// ============================================================
// EmailField tests
// ============================================================

#[rstest]
fn email_field_valid_email() {
	// Arrange
	let field = EmailField::new();

	// Act
	let result = field.validate("user@example.com");

	// Assert
	assert_eq!(result, Ok(()));
}

#[rstest]
fn email_field_invalid_email() {
	// Arrange
	let field = EmailField::new();

	// Act
	let result = field.validate("not-an-email");

	// Assert
	assert!(matches!(result, Err(FieldError::InvalidEmail)));
}

#[rstest]
fn email_field_missing_domain() {
	// Arrange
	let field = EmailField::new();

	// Act
	let result = field.validate("user@");

	// Assert
	assert!(result.is_err());
}

// ============================================================
// URLField tests
// ============================================================

#[rstest]
fn url_field_valid_http_url() {
	// Arrange
	let field = URLField::new();

	// Act
	let result = field.validate("http://example.com");

	// Assert
	assert_eq!(result, Ok(()));
}

#[rstest]
fn url_field_valid_https_url() {
	// Arrange
	let field = URLField::new();

	// Act
	let result = field.validate("https://example.com/path?query=1");

	// Assert
	assert_eq!(result, Ok(()));
}

#[rstest]
fn url_field_invalid_url() {
	// Arrange
	let field = URLField::new();

	// Act
	let result = field.validate("not-a-url");

	// Assert
	assert!(matches!(result, Err(FieldError::InvalidUrl)));
}

// ============================================================
// BooleanField tests
// ============================================================

#[rstest]
fn boolean_field_true_value() {
	// Arrange
	let field = BooleanField::new();

	// Act
	let result = field.validate(true);

	// Assert
	assert_eq!(result, Ok(()));
}

#[rstest]
fn boolean_field_false_value() {
	// Arrange
	let field = BooleanField::new();

	// Act
	let result = field.validate(false);

	// Assert
	assert_eq!(result, Ok(()));
}

// ============================================================
// JsonSerializer tests
// ============================================================

#[rstest]
fn json_serializer_serialize_struct() {
	// Arrange
	let user = TestUser {
		id: Some(1),
		username: "alice".to_string(),
		email: "alice@example.com".to_string(),
	};
	let serializer = JsonSerializer::<TestUser>::new();

	// Act
	let json = serializer.serialize(&user).unwrap();

	// Assert
	let parsed: Value = serde_json::from_str(&json).unwrap();
	assert_eq!(parsed["username"], "alice");
	assert_eq!(parsed["email"], "alice@example.com");
	assert_eq!(parsed["id"], 1);
}

#[rstest]
fn json_serializer_deserialize_struct() {
	// Arrange
	let json = r#"{"id":1,"username":"alice","email":"alice@example.com"}"#.to_string();
	let serializer = JsonSerializer::<TestUser>::new();

	// Act
	let user = serializer.deserialize(&json).unwrap();

	// Assert
	assert_eq!(user.username, "alice");
	assert_eq!(user.email, "alice@example.com");
	assert_eq!(user.id, Some(1));
}

#[rstest]
fn json_serializer_roundtrip() {
	// Arrange
	let original = TestUser {
		id: Some(42),
		username: "bob".to_string(),
		email: "bob@example.com".to_string(),
	};
	let serializer = JsonSerializer::<TestUser>::new();

	// Act
	let json = serializer.serialize(&original).unwrap();
	let restored = serializer.deserialize(&json).unwrap();

	// Assert
	assert_eq!(original, restored);
}

#[rstest]
fn json_serializer_invalid_json_returns_error() {
	// Arrange
	let invalid_json = "{invalid json}".to_string();
	let serializer = JsonSerializer::<TestUser>::new();

	// Act
	let result = serializer.deserialize(&invalid_json);

	// Assert
	assert!(result.is_err());
	assert!(matches!(result.unwrap_err(), SerializerError::Serde { .. }));
}

// ============================================================
// SerializerError tests
// ============================================================

#[rstest]
fn serializer_error_unique_violation() {
	// Arrange
	let error = SerializerError::unique_violation(
		"email".to_string(),
		"alice@example.com".to_string(),
		"Email already exists".to_string(),
	);

	// Act
	let error_str = format!("{}", error);

	// Assert
	assert!(error_str.contains("email") || error_str.contains("alice@example.com"));
}

#[rstest]
fn serializer_error_required_field() {
	// Arrange
	let error = SerializerError::required_field(
		"username".to_string(),
		"This field is required".to_string(),
	);

	// Act
	let error_str = format!("{}", error);

	// Assert
	assert!(error_str.contains("username"));
}

// ============================================================
// MetaConfig tests
// ============================================================

#[rstest]
fn meta_config_includes_specified_fields() {
	// Arrange
	let config = MetaConfig::new().with_fields(vec!["id".to_string(), "username".to_string()]);

	// Assert
	assert!(config.is_field_included("id"));
	assert!(config.is_field_included("username"));
	assert!(!config.is_field_included("email"));
}

#[rstest]
fn meta_config_excludes_specified_fields() {
	// Arrange
	let config = MetaConfig::new().with_exclude(vec!["password_hash".to_string()]);

	// Assert
	assert!(config.is_field_included("username"));
	assert!(!config.is_field_included("password_hash"));
}

#[rstest]
fn meta_config_read_only_fields() {
	// Arrange
	let config =
		MetaConfig::new().with_read_only_fields(vec!["id".to_string(), "created_at".to_string()]);

	// Assert
	assert!(config.is_read_only("id"));
	assert!(config.is_read_only("created_at"));
	assert!(!config.is_read_only("username"));
}

#[rstest]
fn meta_config_write_only_fields() {
	// Arrange
	let config = MetaConfig::new().with_write_only_fields(vec!["password_hash".to_string()]);

	// Assert
	assert!(config.is_write_only("password_hash"));
	assert!(!config.is_write_only("username"));
}

#[rstest]
fn meta_config_effective_fields_with_exclude() {
	// Arrange
	let all_fields = vec![
		"id".to_string(),
		"username".to_string(),
		"email".to_string(),
		"password_hash".to_string(),
	];
	let config = MetaConfig::new().with_exclude(vec!["password_hash".to_string()]);

	// Act
	let effective = config.effective_fields(&all_fields);

	// Assert
	assert_eq!(effective.len(), 3);
	assert!(effective.contains("id"));
	assert!(effective.contains("username"));
	assert!(effective.contains("email"));
	assert!(!effective.contains("password_hash"));
}

#[rstest]
fn default_meta_includes_all_fields() {
	// Arrange
	let all_fields = vec![
		"id".to_string(),
		"username".to_string(),
		"email".to_string(),
	];

	// Act
	let effective = DefaultMeta::effective_fields(&all_fields);

	// Assert
	assert_eq!(effective.len(), 3);
	assert!(effective.contains("id"));
	assert!(effective.contains("username"));
	assert!(effective.contains("email"));
}

// ============================================================
// NestedFieldConfig tests
// ============================================================

#[rstest]
fn nested_field_config_default_depth() {
	// Arrange
	let config = NestedFieldConfig::new("author");

	// Assert
	assert_eq!(config.field_name, "author");
	assert_eq!(config.depth, 1);
	assert!(!config.read_only);
	assert!(!config.allow_create);
	assert!(!config.allow_update);
}

#[rstest]
fn nested_field_config_custom_depth() {
	// Arrange
	let config = NestedFieldConfig::new("author").depth(3);

	// Assert
	assert_eq!(config.depth, 3);
}

#[rstest]
fn nested_field_config_read_only() {
	// Arrange
	let config = NestedFieldConfig::new("author").read_only();

	// Assert
	assert!(config.read_only);
	assert!(!config.allow_create);
	assert!(!config.allow_update);
}

#[rstest]
fn nested_field_config_writable() {
	// Arrange
	let config = NestedFieldConfig::new("author").writable();

	// Assert
	assert!(config.allow_create);
	assert!(config.allow_update);
}

#[rstest]
fn nested_serializer_config_add_and_get() {
	// Arrange
	let mut config = NestedSerializerConfig::new();

	// Act
	config.add_nested_field(NestedFieldConfig::new("author").depth(2));

	// Assert
	assert!(config.is_nested_field("author"));
	assert!(!config.is_nested_field("category"));
	let field = config.get_nested_field("author").unwrap();
	assert_eq!(field.depth, 2);
}

#[rstest]
fn nested_serializer_config_multiple_fields() {
	// Arrange
	let mut config = NestedSerializerConfig::new();

	// Act
	config.add_nested_field(NestedFieldConfig::new("author"));
	config.add_nested_field(NestedFieldConfig::new("category"));
	config.add_nested_field(NestedFieldConfig::new("tags"));

	// Assert
	let names = config.nested_field_names();
	assert_eq!(names.len(), 3);
	assert!(names.contains(&"author".to_string()));
	assert!(names.contains(&"category".to_string()));
	assert!(names.contains(&"tags".to_string()));
}

#[rstest]
fn nested_serializer_config_remove_field() {
	// Arrange
	let mut config = NestedSerializerConfig::new();
	config.add_nested_field(NestedFieldConfig::new("author"));
	assert!(config.is_nested_field("author"));

	// Act
	let removed = config.remove_nested_field("author");

	// Assert
	assert!(removed.is_some());
	assert!(!config.is_nested_field("author"));
}

#[rstest]
fn nested_serializer_config_get_depth() {
	// Arrange
	let mut config = NestedSerializerConfig::new();
	config.add_nested_field(NestedFieldConfig::new("author").depth(3));

	// Assert
	assert_eq!(config.get_depth("author"), Some(3));
	assert_eq!(config.get_depth("unknown"), None);
}

// ============================================================
// FieldIntrospector tests
// ============================================================

#[rstest]
fn field_introspector_register_and_get() {
	// Arrange
	let mut introspector = FieldIntrospector::new();

	// Act
	introspector.register_field(FieldInfo::new("id", "i64").optional().primary_key());
	introspector.register_field(FieldInfo::new("username", "String"));

	// Assert
	let fields = introspector.get_fields();
	assert_eq!(fields.len(), 2);
}

#[rstest]
fn field_introspector_field_names() {
	// Arrange
	let mut introspector = FieldIntrospector::new();
	introspector.register_field(FieldInfo::new("id", "i64"));
	introspector.register_field(FieldInfo::new("name", "String"));

	// Act
	let names = introspector.field_names();

	// Assert
	assert_eq!(names.len(), 2);
	assert!(names.contains(&"id".to_string()));
	assert!(names.contains(&"name".to_string()));
}

// ============================================================
// ModelSerializer tests
// ============================================================

#[rstest]
fn model_serializer_new() {
	// Act
	let serializer = ModelSerializer::<TestUser>::new();

	// Assert
	assert!(!serializer.is_nested_field("any_field"));
}

#[rstest]
fn model_serializer_with_fields() {
	// Act
	let serializer = ModelSerializer::<TestUser>::new()
		.with_fields(vec!["id".to_string(), "username".to_string()]);

	// Assert
	let meta = serializer.meta();
	assert!(meta.is_field_included("id"));
	assert!(meta.is_field_included("username"));
	assert!(!meta.is_field_included("email"));
}

#[rstest]
fn model_serializer_with_exclude() {
	// Act
	let serializer = ModelSerializer::<TestUser>::new().with_exclude(vec!["email".to_string()]);

	// Assert
	let meta = serializer.meta();
	assert!(meta.is_field_included("username"));
	assert!(!meta.is_field_included("email"));
}

#[rstest]
fn model_serializer_with_read_only_fields() {
	// Act
	let serializer =
		ModelSerializer::<TestUser>::new().with_read_only_fields(vec!["id".to_string()]);

	// Assert
	let meta = serializer.meta();
	assert!(meta.is_read_only("id"));
	assert!(!meta.is_read_only("username"));
}

#[rstest]
fn model_serializer_with_nested_field() {
	// Act
	let serializer = ModelSerializer::<TestPost>::new()
		.with_nested_field(NestedFieldConfig::new("author").depth(2));

	// Assert
	assert!(serializer.is_nested_field("author"));
	assert!(!serializer.is_nested_field("title"));
}

#[rstest]
fn model_serializer_field_names_from_introspector() {
	// Arrange
	let mut introspector = FieldIntrospector::new();
	introspector.register_field(FieldInfo::new("id", "i64").primary_key());
	introspector.register_field(FieldInfo::new("username", "String"));
	introspector.register_field(FieldInfo::new("email", "String"));

	// Act
	let serializer = ModelSerializer::<TestUser>::new().with_introspector(introspector);
	let fields = serializer.field_names();

	// Assert
	assert_eq!(fields.len(), 3);
}

// ============================================================
// UniqueValidator tests
// ============================================================

#[rstest]
fn unique_validator_new() {
	// Act
	let validator = UniqueValidator::<TestUser>::new("username");

	// Assert
	assert_eq!(validator.field_name(), "username");
}

#[rstest]
fn unique_validator_with_message() {
	// Act
	let validator = UniqueValidator::<TestUser>::new("username")
		.with_message("Username must be unique across all users");

	// Assert
	assert_eq!(validator.field_name(), "username");
}

// ============================================================
// UniqueTogetherValidator tests
// ============================================================

#[rstest]
fn unique_together_validator_new() {
	// Act
	let validator = UniqueTogetherValidator::<TestUser>::new(vec!["username", "email"]);

	// Assert
	let field_names = validator.field_names();
	assert_eq!(field_names.len(), 2);
	assert_eq!(field_names[0], "username");
	assert_eq!(field_names[1], "email");
}

#[rstest]
fn unique_together_validator_single_field() {
	// Act
	let validator = UniqueTogetherValidator::<TestUser>::new(vec!["username"]);

	// Assert
	assert_eq!(validator.field_names().len(), 1);
	assert_eq!(validator.field_names()[0], "username");
}

// ============================================================
// DatabaseValidatorError tests
// ============================================================

#[rstest]
fn database_validator_error_unique_constraint_display() {
	// Arrange
	let error = DatabaseValidatorError::UniqueConstraintViolation {
		field: "email".to_string(),
		value: "alice@example.com".to_string(),
		table: "users".to_string(),
		message: None,
	};

	// Act
	let error_str = format!("{}", error);

	// Assert
	assert!(error_str.contains("email"));
	assert!(error_str.contains("alice@example.com"));
	assert!(error_str.contains("users"));
}

#[rstest]
fn database_validator_error_unique_together_display() {
	// Arrange
	let error = DatabaseValidatorError::UniqueTogetherViolation {
		fields: vec!["username".to_string(), "email".to_string()],
		values: vec!["alice".to_string(), "alice@example.com".to_string()],
		table: "users".to_string(),
		message: None,
	};

	// Act
	let error_str = format!("{}", error);

	// Assert
	assert!(error_str.contains("username"));
	assert!(error_str.contains("email"));
}

#[rstest]
fn database_validator_error_field_not_found() {
	// Arrange
	let error = DatabaseValidatorError::FieldNotFound {
		field: "missing_field".to_string(),
	};

	// Act
	let error_str = format!("{}", error);

	// Assert
	assert!(error_str.contains("missing_field"));
}

#[rstest]
fn database_validator_error_converts_to_serializer_error() {
	// Arrange
	let db_error = DatabaseValidatorError::UniqueConstraintViolation {
		field: "email".to_string(),
		value: "user@example.com".to_string(),
		table: "users".to_string(),
		message: Some("Custom message".to_string()),
	};

	// Act
	let serializer_error: SerializerError = db_error.into();

	// Assert
	assert!(matches!(serializer_error, SerializerError::Other { .. }));
}

// ============================================================
// SerializerMethodField tests
// ============================================================

#[rstest]
fn serializer_method_field_new() {
	// Act
	let field = SerializerMethodField::new("get_full_name");

	// Assert
	assert_eq!(field.method_name, "get_full_name");
	assert!(field.custom_method_name.is_none());
	assert!(field.read_only);
}

#[rstest]
fn serializer_method_field_get_value_success() {
	// Arrange
	let mut context = HashMap::new();
	context.insert("full_name".to_string(), json!("Alice Smith"));
	let field = SerializerMethodField::new("full_name");

	// Act
	let value = field.get_value(&context).unwrap();

	// Assert
	assert_eq!(value, json!("Alice Smith"));
}

#[rstest]
fn serializer_method_field_get_value_missing() {
	// Arrange
	let context: HashMap<String, Value> = HashMap::new();
	let field = SerializerMethodField::new("missing_method");

	// Act
	let result = field.get_value(&context);

	// Assert
	assert!(result.is_err());
	assert!(matches!(
		result.unwrap_err(),
		MethodFieldError::MethodNotFound(_)
	));
}

#[rstest]
fn serializer_method_field_custom_method_name() {
	// Arrange
	let mut context = HashMap::new();
	context.insert("compute_name".to_string(), json!("Bob Jones"));
	let field = SerializerMethodField::new("full_name").method_name("compute_name");

	// Act
	let value = field.get_value(&context).unwrap();

	// Assert
	assert_eq!(value, json!("Bob Jones"));
	assert_eq!(field.get_method_name(), "compute_name");
}

#[rstest]
fn serializer_method_field_complex_value() {
	// Arrange
	let mut context = HashMap::new();
	context.insert(
		"stats".to_string(),
		json!({
			"post_count": 10,
			"follower_count": 200,
			"is_verified": true,
		}),
	);
	let field = SerializerMethodField::new("stats");

	// Act
	let value = field.get_value(&context).unwrap();

	// Assert
	assert_eq!(value["post_count"], 10);
	assert_eq!(value["follower_count"], 200);
	assert_eq!(value["is_verified"], true);
}

// ============================================================
// MethodFieldRegistry tests
// ============================================================

#[rstest]
fn method_field_registry_register_and_get() {
	// Arrange
	let mut registry = MethodFieldRegistry::new();
	let field = SerializerMethodField::new("full_name");

	// Act
	registry.register("full_name", field);

	// Assert
	assert!(registry.contains("full_name"));
	let retrieved = registry.get("full_name").unwrap();
	assert_eq!(retrieved.method_name, "full_name");
}

#[rstest]
fn method_field_registry_not_found() {
	// Arrange
	let registry = MethodFieldRegistry::new();

	// Assert
	assert!(!registry.contains("nonexistent"));
	assert!(registry.get("nonexistent").is_none());
}

#[rstest]
fn method_field_registry_multiple_fields() {
	// Arrange
	let mut registry = MethodFieldRegistry::new();

	// Act
	registry.register("full_name", SerializerMethodField::new("full_name"));
	registry.register("age", SerializerMethodField::new("age"));
	registry.register("is_admin", SerializerMethodField::new("is_admin"));

	// Assert
	assert!(registry.contains("full_name"));
	assert!(registry.contains("age"));
	assert!(registry.contains("is_admin"));
	assert_eq!(registry.all().len(), 3);
}

// ============================================================
// Relation field tests
// ============================================================

#[rstest]
fn relation_field_new() {
	// Act
	let field = RelationField::<TestUser>::new();

	// Assert - field can be serialized
	let json = serde_json::to_string(&field).unwrap();
	assert_eq!(json, r#"{"_phantom":null}"#);
}

#[rstest]
fn primary_key_related_field_new() {
	// Act
	let field = PrimaryKeyRelatedField::<TestUser>::new();

	// Assert - type alias works, field is serializable
	let json = serde_json::to_string(&field).unwrap();
	assert_eq!(json, r#"{"_phantom":null}"#);
}

#[rstest]
fn slug_related_field_new() {
	// Act
	let field = SlugRelatedField::<TestUser>::new();

	// Assert
	let json = serde_json::to_string(&field).unwrap();
	assert_eq!(json, r#"{"_phantom":null}"#);
}

#[rstest]
fn string_related_field_new() {
	// Act
	let field = StringRelatedField::<TestUser>::new();

	// Assert
	let json = serde_json::to_string(&field).unwrap();
	assert_eq!(json, r#"{"_phantom":null}"#);
}

#[rstest]
fn hyperlinked_related_field_new() {
	// Act
	let field = HyperlinkedRelatedField::<TestUser>::new();

	// Assert
	let json = serde_json::to_string(&field).unwrap();
	assert_eq!(json, r#"{"_phantom":null}"#);
}

#[rstest]
fn many_related_field_new() {
	// Act
	let field = ManyRelatedField::<TestUser>::new();

	// Assert - field is serializable
	let json = serde_json::to_string(&field).unwrap();
	assert_eq!(json, r#"{"_phantom":null}"#);
}

#[rstest]
fn many_related_field_default() {
	// Act
	let field = ManyRelatedField::<TestUser>::default();

	// Assert
	let json = serde_json::to_string(&field).unwrap();
	assert_eq!(json, r#"{"_phantom":null}"#);
}

// ============================================================
// ValidationError tests
// ============================================================

#[rstest]
fn validation_error_field_error() {
	// Act
	let error = ValidationError::field_error("email", "Invalid email format");

	// Assert
	let error_str = format!("{}", error);
	assert!(error_str.contains("email"));
	assert!(error_str.contains("Invalid email format"));
}

#[rstest]
fn validation_error_object_error() {
	// Act
	let error = ValidationError::object_error("Passwords do not match");

	// Assert
	let error_str = format!("{}", error);
	assert!(error_str.contains("Passwords do not match"));
}

#[rstest]
fn validation_error_multiple() {
	// Arrange
	let errors = vec![
		ValidationError::field_error("email", "Required"),
		ValidationError::field_error("username", "Too short"),
	];

	// Act
	let combined = ValidationError::multiple(errors);

	// Assert
	let error_str = format!("{}", combined);
	assert!(!error_str.is_empty());
}

// ============================================================
// Issue #3992 — ModelSerializer / NestedSerializer honesty
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestAuthor {
	id: Option<i64>,
	name: String,
}

#[derive(Debug, Clone)]
struct TestAuthorFields;

impl FieldSelector for TestAuthorFields {
	fn with_alias(self, _alias: &str) -> Self {
		self
	}
}

impl Model for TestAuthor {
	type PrimaryKey = i64;
	type Fields = TestAuthorFields;

	fn table_name() -> &'static str {
		"test_authors"
	}

	fn new_fields() -> Self::Fields {
		TestAuthorFields
	}

	fn primary_key(&self) -> Option<Self::PrimaryKey> {
		self.id
	}

	fn set_primary_key(&mut self, value: Self::PrimaryKey) {
		self.id = Some(value);
	}
}

fn sample_user() -> TestUser {
	TestUser {
		id: Some(7),
		username: "alice".to_string(),
		email: "alice@example.com".to_string(),
	}
}

// ---- Track 1: MetaConfig is applied during serialize/deserialize ----

#[rstest]
fn model_serializer_serialize_with_no_meta_keeps_all_fields() {
	// Arrange
	let serializer = ModelSerializer::<TestUser>::new();
	let user = sample_user();

	// Act
	let json = serializer.serialize(&user).unwrap();
	let parsed: Value = serde_json::from_str(&json).unwrap();

	// Assert: backward compatibility — empty MetaConfig keeps every field
	assert_eq!(parsed["id"], 7);
	assert_eq!(parsed["username"], "alice");
	assert_eq!(parsed["email"], "alice@example.com");
}

#[rstest]
fn model_serializer_deserialize_with_no_meta_round_trips() {
	// Arrange
	let serializer = ModelSerializer::<TestUser>::new();
	let json = r#"{"id":7,"username":"alice","email":"alice@example.com"}"#.to_string();

	// Act
	let restored = serializer.deserialize(&json).unwrap();

	// Assert
	assert_eq!(restored, sample_user());
}

#[rstest]
fn model_serializer_serialize_with_fields_allowlist_drops_others() {
	// Arrange
	let serializer = ModelSerializer::<TestUser>::new()
		.with_fields(vec!["id".to_string(), "username".to_string()]);
	let user = sample_user();

	// Act
	let parsed: Value = serde_json::from_str(&serializer.serialize(&user).unwrap()).unwrap();

	// Assert
	assert_eq!(parsed["id"], 7);
	assert_eq!(parsed["username"], "alice");
	assert!(parsed.get("email").is_none(), "email must be filtered out");
}

#[rstest]
fn model_serializer_serialize_with_exclude_drops_listed_field() {
	// Arrange
	let serializer = ModelSerializer::<TestUser>::new().with_exclude(vec!["email".to_string()]);
	let user = sample_user();

	// Act
	let parsed: Value = serde_json::from_str(&serializer.serialize(&user).unwrap()).unwrap();

	// Assert
	assert!(parsed.get("email").is_none());
	assert_eq!(parsed["username"], "alice");
}

#[rstest]
fn model_serializer_serialize_strips_write_only_fields() {
	// Arrange: write-only fields must never leak to API responses.
	let serializer =
		ModelSerializer::<TestUser>::new().with_write_only_fields(vec!["email".to_string()]);
	let user = sample_user();

	// Act
	let parsed: Value = serde_json::from_str(&serializer.serialize(&user).unwrap()).unwrap();

	// Assert
	assert!(parsed.get("email").is_none());
	assert_eq!(parsed["id"], 7);
}

#[rstest]
fn model_serializer_deserialize_keeps_write_only_fields() {
	// Arrange: write_only blocks output only; input is unaffected.
	let serializer =
		ModelSerializer::<TestUser>::new().with_write_only_fields(vec!["email".to_string()]);
	let json = r#"{"id":7,"username":"alice","email":"alice@example.com"}"#.to_string();

	// Act
	let restored = serializer.deserialize(&json).unwrap();

	// Assert
	assert_eq!(restored.email, "alice@example.com");
}

#[rstest]
fn model_serializer_deserialize_strips_read_only_fields() {
	// Arrange: server-set fields must not be accepted from clients.
	let serializer =
		ModelSerializer::<TestUser>::new().with_read_only_fields(vec!["id".to_string()]);
	let json = r#"{"id":999,"username":"alice","email":"alice@example.com"}"#.to_string();

	// Act
	let restored = serializer.deserialize(&json).unwrap();

	// Assert: incoming "id":999 was dropped before deserialization
	assert_eq!(restored.id, None);
	assert_eq!(restored.username, "alice");
	assert_eq!(restored.email, "alice@example.com");
}

#[rstest]
fn model_serializer_serialize_keeps_read_only_fields() {
	// Arrange
	let serializer =
		ModelSerializer::<TestUser>::new().with_read_only_fields(vec!["id".to_string()]);
	let user = sample_user();

	// Act
	let parsed: Value = serde_json::from_str(&serializer.serialize(&user).unwrap()).unwrap();

	// Assert: read-only fields are still present in the response
	assert_eq!(parsed["id"], 7);
}

#[rstest]
fn model_serializer_combined_meta_serialize_filters_correctly() {
	// Arrange: fields allowlist limits to id+username+email; exclude drops
	// email; write_only drops username from output -> only id remains.
	let serializer = ModelSerializer::<TestUser>::new()
		.with_fields(vec!["id".into(), "username".into(), "email".into()])
		.with_exclude(vec!["email".into()])
		.with_write_only_fields(vec!["username".into()]);
	let user = sample_user();

	// Act
	let parsed: Value = serde_json::from_str(&serializer.serialize(&user).unwrap()).unwrap();

	// Assert
	assert_eq!(parsed["id"], 7);
	assert!(parsed.get("username").is_none());
	assert!(parsed.get("email").is_none());
}

#[rstest]
#[case::no_config(
	ModelSerializer::<TestUser>::new(),
	BTreeSet::from(["id".to_string(), "username".to_string(), "email".to_string()]),
)]
#[case::fields_only(
	ModelSerializer::<TestUser>::new()
		.with_fields(vec!["id".to_string(), "username".to_string()]),
	BTreeSet::from(["id".to_string(), "username".to_string()]),
)]
#[case::exclude_only(
	ModelSerializer::<TestUser>::new().with_exclude(vec!["email".to_string()]),
	BTreeSet::from(["id".to_string(), "username".to_string()]),
)]
#[case::write_only_drops_field(
	ModelSerializer::<TestUser>::new()
		.with_write_only_fields(vec!["email".to_string()]),
	BTreeSet::from(["id".to_string(), "username".to_string()]),
)]
#[case::fields_overrides_default(
	ModelSerializer::<TestUser>::new().with_fields(vec!["id".to_string()]),
	BTreeSet::from(["id".to_string()]),
)]
fn model_serializer_serialize_keys_match_expected(
	#[case] serializer: ModelSerializer<TestUser>,
	#[case] expected: BTreeSet<String>,
) {
	// Arrange
	let user = sample_user();

	// Act
	let parsed: Value = serde_json::from_str(&serializer.serialize(&user).unwrap()).unwrap();
	let actual: BTreeSet<String> = parsed.as_object().unwrap().keys().cloned().collect();

	// Assert: exact key set, no extras and no missing
	assert_eq!(actual, expected);
}

#[rstest]
fn model_serializer_serialize_uses_introspector_when_fields_unset() {
	// Arrange: introspector defines only id+username; email is unknown.
	let mut introspector = FieldIntrospector::new();
	introspector.register_field(FieldInfo::new("id", "i64").primary_key());
	introspector.register_field(FieldInfo::new("username", "String"));

	let serializer = ModelSerializer::<TestUser>::new().with_introspector(introspector);
	let user = sample_user();

	// Act
	let parsed: Value = serde_json::from_str(&serializer.serialize(&user).unwrap()).unwrap();

	// Assert: email is dropped because it is not in the introspector allowlist
	assert_eq!(parsed["id"], 7);
	assert_eq!(parsed["username"], "alice");
	assert!(parsed.get("email").is_none());
}

// ---- Track 2: ModelSerializer::validate runs registered sync validators ----

#[derive(Debug)]
struct UsernameNonEmpty;

impl ModelLevelValidator<TestUser> for UsernameNonEmpty {
	fn validate(&self, instance: &TestUser) -> Result<(), ValidatorError> {
		if instance.username.is_empty() {
			Err(ValidatorError::Custom {
				message: "username must not be empty".to_string(),
			})
		} else {
			Ok(())
		}
	}
}

#[rstest]
fn model_serializer_validate_with_no_validators_is_ok() {
	// Arrange: empty config preserves the historical behavior of validate.
	let serializer = ModelSerializer::<TestUser>::new();
	let user = sample_user();

	// Act
	let result = serializer.validate(&user);

	// Assert
	assert!(result.is_ok());
}

#[rstest]
fn model_serializer_validate_runs_registered_model_validator_pass() {
	// Arrange: validator passes because username is non-empty.
	let serializer = ModelSerializer::<TestUser>::new().with_model_validator(UsernameNonEmpty);
	let user = sample_user();

	// Act
	let result = serializer.validate(&user);

	// Assert
	assert!(result.is_ok());
}

#[rstest]
fn model_serializer_validate_runs_registered_model_validator_fail() {
	// Arrange: validator should reject an empty username.
	let serializer = ModelSerializer::<TestUser>::new().with_model_validator(UsernameNonEmpty);
	let user = TestUser {
		id: Some(1),
		username: String::new(),
		email: "ghost@example.com".to_string(),
	};

	// Act
	let result = serializer.validate(&user);

	// Assert: error must be wrapped as `SerializerError::Validation`
	let err = result.expect_err("expected sync validator failure");
	match err {
		SerializerError::Validation(ValidatorError::Custom { ref message }) => {
			assert_eq!(message, "username must not be empty");
		}
		other => panic!("unexpected error variant: {:?}", other),
	}
}

#[rstest]
fn validator_config_has_validators_includes_sync_model() {
	// Arrange: empty config is empty; adding a sync model validator flips it.
	let bare = ModelSerializer::<TestUser>::new();
	let configured = ModelSerializer::<TestUser>::new().with_model_validator(UsernameNonEmpty);

	// Act + Assert
	assert!(!bare.validators().has_validators());
	assert!(configured.validators().has_validators());
}

// ---- Track 3: WritableNestedSerializer is honest ----

#[rstest]
fn writable_nested_serializer_serialize_emits_full_parent_object() {
	// Arrange: with the no-op gone, serialize must still produce a complete
	// parent JSON object regardless of relationship loading state.
	let serializer = WritableNestedSerializer::<TestPost, TestAuthor>::new("author");
	let post = TestPost {
		id: Some(42),
		title: "hello".to_string(),
		author_id: 7,
	};

	// Act
	let json = serializer.serialize(&post).expect("serialize must succeed");
	let parsed: Value = serde_json::from_str(&json).unwrap();

	// Assert
	assert_eq!(parsed["id"], 42);
	assert_eq!(parsed["title"], "hello");
	assert_eq!(parsed["author_id"], 7);
}

#[rstest]
fn writable_nested_serializer_extract_nested_data_after_deserialize() {
	// Arrange: trait method returns only the parent; the helper exposes
	// nested data for the caller to dispatch to the ORM.
	let serializer = WritableNestedSerializer::<TestPost, TestAuthor>::new("author")
		.allow_create(true)
		.allow_update(true);
	let json = json!({
		"id": 1,
		"title": "post",
		"author_id": 9,
		"author": { "id": null, "name": "Alice" }
	})
	.to_string();

	// Act
	let parent = serializer
		.deserialize(&json)
		.expect("deserialize must validate structure");
	let nested = serializer
		.extract_nested_data(&json)
		.expect("extract_nested_data must succeed");

	// Assert: the trait method is honest about returning only the parent,
	// while the helper surfaces the nested payload for ORM dispatch.
	assert_eq!(parent.id, Some(1));
	assert_eq!(parent.title, "post");
	let nested = nested.expect("nested data must be present in JSON");
	assert_eq!(nested["name"], "Alice");
	assert!(nested["id"].is_null());
}

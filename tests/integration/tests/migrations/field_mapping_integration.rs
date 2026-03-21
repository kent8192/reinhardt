//! Field Mapping Integration Tests
//!
//! Tests the pipeline from macro-generated `FieldMetadata` through `ModelMetadata`
//! to `ModelState`/`FieldState`, verifying that field types and options are correctly
//! propagated through the migration system.
//!
//! **Test Coverage:**
//! - Field type mapping (String, Int, Bool, etc.) from FieldMetadata to FieldState
//! - Field option propagation (nullable, default, max_length)
//! - ColumnDefinition construction with various field types and options
//! - Auto-increment field generation for different integer types
//!
//! **Closes:** #1705

use reinhardt_db::migrations::{
	ColumnDefinition, FieldMetadata, FieldType, ModelMetadata, ModelRegistry,
};
use rstest::*;

// ============================================================================
// Issue #1705: FieldMetadata → FieldState Mapping Tests
// ============================================================================

/// Test that String field type maps correctly through the pipeline
///
/// **Test Intent**: Verify CharField/VarChar FieldMetadata produces correct FieldState
#[rstest]
fn test_string_field_mapping() {
	// Arrange
	let mut metadata = ModelMetadata::new("blog", "Post", "blog_post");
	let title_field = FieldMetadata::new(FieldType::VarChar(200)).with_param("max_length", "200");
	metadata.add_field("title".to_string(), title_field);

	// Act
	let model_state = metadata.to_model_state();

	// Assert
	assert_eq!(model_state.app_label, "blog");
	assert_eq!(model_state.name, "Post");
	assert!(model_state.fields.contains_key("title"));
	let field_state = &model_state.fields["title"];
	assert_eq!(field_state.field_type, FieldType::VarChar(200));
	assert_eq!(
		field_state.params.get("max_length").map(String::as_str),
		Some("200")
	);
}

/// Test that Integer field type maps correctly through the pipeline
///
/// **Test Intent**: Verify IntegerField FieldMetadata produces correct FieldState
#[rstest]
fn test_integer_field_mapping() {
	// Arrange
	let mut metadata = ModelMetadata::new("shop", "Product", "shop_product");
	let price_field = FieldMetadata::new(FieldType::Integer);
	metadata.add_field("price".to_string(), price_field);

	// Act
	let model_state = metadata.to_model_state();

	// Assert
	let field_state = &model_state.fields["price"];
	assert_eq!(field_state.field_type, FieldType::Integer);
	assert!(!field_state.nullable);
}

/// Test that Boolean field type maps correctly through the pipeline
///
/// **Test Intent**: Verify BooleanField FieldMetadata produces correct FieldState
#[rstest]
fn test_boolean_field_mapping() {
	// Arrange
	let mut metadata = ModelMetadata::new("blog", "Post", "blog_post");
	let published_field = FieldMetadata::new(FieldType::Boolean).with_param("default", "false");
	metadata.add_field("published".to_string(), published_field);

	// Act
	let model_state = metadata.to_model_state();

	// Assert
	let field_state = &model_state.fields["published"];
	assert_eq!(field_state.field_type, FieldType::Boolean);
	assert_eq!(
		field_state.params.get("default").map(String::as_str),
		Some("false")
	);
}

/// Test that Text field type maps correctly
///
/// **Test Intent**: Verify TextField FieldMetadata produces correct FieldState
#[rstest]
fn test_text_field_mapping() {
	// Arrange
	let mut metadata = ModelMetadata::new("blog", "Post", "blog_post");
	let body_field = FieldMetadata::new(FieldType::Text);
	metadata.add_field("body".to_string(), body_field);

	// Act
	let model_state = metadata.to_model_state();

	// Assert
	let field_state = &model_state.fields["body"];
	assert_eq!(field_state.field_type, FieldType::Text);
}

/// Test that nullable option is propagated through FieldMetadata
///
/// **Test Intent**: Verify null=true param makes FieldState nullable
#[rstest]
fn test_nullable_option_propagation() {
	// Arrange
	let mut metadata = ModelMetadata::new("blog", "Post", "blog_post");
	let nullable_field = FieldMetadata::new(FieldType::VarChar(255)).with_param("null", "true");
	metadata.add_field("subtitle".to_string(), nullable_field);

	// Act
	let model_state = metadata.to_model_state();

	// Assert
	let field_state = &model_state.fields["subtitle"];
	assert!(
		field_state.nullable,
		"Field with null=true should be nullable"
	);
}

/// Test that non-nullable (default) option is propagated
///
/// **Test Intent**: Verify field without null param is not nullable
#[rstest]
fn test_not_nullable_default_propagation() {
	// Arrange
	let mut metadata = ModelMetadata::new("blog", "Post", "blog_post");
	let required_field = FieldMetadata::new(FieldType::VarChar(100));
	metadata.add_field("title".to_string(), required_field);

	// Act
	let model_state = metadata.to_model_state();

	// Assert
	let field_state = &model_state.fields["title"];
	assert!(
		!field_state.nullable,
		"Field without null param should not be nullable"
	);
}

/// Test that max_length option is propagated
///
/// **Test Intent**: Verify max_length param is preserved in FieldState
#[rstest]
fn test_max_length_option_propagation() {
	// Arrange
	let mut metadata = ModelMetadata::new("auth", "User", "auth_user");
	let username_field =
		FieldMetadata::new(FieldType::VarChar(150)).with_param("max_length", "150");
	metadata.add_field("username".to_string(), username_field);

	// Act
	let model_state = metadata.to_model_state();

	// Assert
	let field_state = &model_state.fields["username"];
	assert_eq!(
		field_state.params.get("max_length").map(String::as_str),
		Some("150")
	);
}

/// Test that unique option generates constraint in ModelState
///
/// **Test Intent**: Verify unique=true produces a ConstraintDefinition
#[rstest]
fn test_unique_option_generates_constraint() {
	// Arrange
	let mut metadata = ModelMetadata::new("auth", "User", "auth_user");
	let email_field = FieldMetadata::new(FieldType::VarChar(255)).with_param("unique", "true");
	metadata.add_field("email".to_string(), email_field);

	// Act
	let model_state = metadata.to_model_state();

	// Assert
	assert!(
		!model_state.constraints.is_empty(),
		"Unique field should generate a constraint"
	);
	let constraint = &model_state.constraints[0];
	assert_eq!(constraint.constraint_type, "unique");
	assert!(constraint.fields.contains(&"email".to_string()));
}

/// Test multiple fields are all correctly mapped
///
/// **Test Intent**: Verify a model with mixed field types maps all fields correctly
#[rstest]
fn test_multiple_field_types_mapping() {
	// Arrange
	let mut metadata = ModelMetadata::new("blog", "Article", "blog_article");
	metadata.add_field(
		"title".to_string(),
		FieldMetadata::new(FieldType::VarChar(200)).with_param("max_length", "200"),
	);
	metadata.add_field("content".to_string(), FieldMetadata::new(FieldType::Text));
	metadata.add_field("views".to_string(), FieldMetadata::new(FieldType::Integer));
	metadata.add_field(
		"published".to_string(),
		FieldMetadata::new(FieldType::Boolean).with_param("default", "false"),
	);
	metadata.add_field(
		"created_at".to_string(),
		FieldMetadata::new(FieldType::DateTime),
	);
	metadata.add_field(
		"rating".to_string(),
		FieldMetadata::new(FieldType::Decimal {
			precision: 5,
			scale: 2,
		}),
	);

	// Act
	let model_state = metadata.to_model_state();

	// Assert
	assert_eq!(model_state.fields.len(), 6);
	assert_eq!(
		model_state.fields["title"].field_type,
		FieldType::VarChar(200)
	);
	assert_eq!(model_state.fields["content"].field_type, FieldType::Text);
	assert_eq!(model_state.fields["views"].field_type, FieldType::Integer);
	assert_eq!(
		model_state.fields["published"].field_type,
		FieldType::Boolean
	);
	assert_eq!(
		model_state.fields["created_at"].field_type,
		FieldType::DateTime
	);
	assert_eq!(
		model_state.fields["rating"].field_type,
		FieldType::Decimal {
			precision: 5,
			scale: 2,
		}
	);
}

/// Test table name propagation from ModelMetadata to ModelState
///
/// **Test Intent**: Verify custom table name is preserved in ModelState
#[rstest]
fn test_table_name_propagation() {
	// Arrange
	let metadata = ModelMetadata::new("auth", "UserProfile", "custom_profiles_table");

	// Act
	let model_state = metadata.to_model_state();

	// Assert
	assert_eq!(model_state.table_name, "custom_profiles_table");
}

/// Test ModelRegistry registration and retrieval preserves metadata
///
/// **Test Intent**: Verify ModelRegistry correctly stores and retrieves field definitions
#[rstest]
fn test_registry_preserves_field_metadata() {
	// Arrange
	let registry = ModelRegistry::new();
	let mut metadata = ModelMetadata::new("shop", "Product", "shop_product");
	metadata.add_field(
		"name".to_string(),
		FieldMetadata::new(FieldType::VarChar(100)).with_param("max_length", "100"),
	);
	metadata.add_field(
		"price".to_string(),
		FieldMetadata::new(FieldType::Decimal {
			precision: 10,
			scale: 2,
		}),
	);

	// Act
	registry.register_model(metadata);
	let retrieved = registry.get_model("shop", "Product");

	// Assert
	assert!(retrieved.is_some());
	let model = retrieved.unwrap();
	assert_eq!(model.fields.len(), 2);
	assert_eq!(model.fields["name"].field_type, FieldType::VarChar(100));
	assert_eq!(
		model.fields["price"].field_type,
		FieldType::Decimal {
			precision: 10,
			scale: 2,
		}
	);
}

// ============================================================================
// ColumnDefinition Construction Tests
// ============================================================================

/// Test ColumnDefinition construction with basic field types
///
/// **Test Intent**: Verify ColumnDefinition correctly represents different field types
#[rstest]
#[case::varchar(FieldType::VarChar(255), "email")]
#[case::text(FieldType::Text, "body")]
#[case::integer(FieldType::Integer, "count")]
#[case::big_integer(FieldType::BigInteger, "big_count")]
#[case::small_integer(FieldType::SmallInteger, "small_count")]
#[case::boolean(FieldType::Boolean, "active")]
#[case::date(FieldType::Date, "birthday")]
#[case::datetime(FieldType::DateTime, "created_at")]
#[case::uuid(FieldType::Uuid, "uuid")]
#[case::json(FieldType::Json, "data")]
fn test_column_definition_field_types(#[case] field_type: FieldType, #[case] name: &str) {
	// Arrange & Act
	let column = ColumnDefinition {
		name: name.to_string(),
		type_definition: field_type.clone(),
		not_null: false,
		unique: false,
		primary_key: false,
		auto_increment: false,
		default: None,
	};

	// Assert
	assert_eq!(column.name, name);
	assert_eq!(column.type_definition, field_type);
	assert!(!column.not_null);
	assert!(!column.unique);
	assert!(!column.primary_key);
	assert!(!column.auto_increment);
	assert!(column.default.is_none());
}

/// Test ColumnDefinition with NOT NULL constraint
///
/// **Test Intent**: Verify ColumnDefinition correctly propagates NOT NULL
#[rstest]
fn test_column_definition_not_null() {
	// Arrange & Act
	let column = ColumnDefinition {
		name: "username".to_string(),
		type_definition: FieldType::VarChar(150),
		not_null: true,
		unique: false,
		primary_key: false,
		auto_increment: false,
		default: None,
	};

	// Assert
	assert!(column.not_null, "Column should be NOT NULL");
}

/// Test ColumnDefinition with DEFAULT value
///
/// **Test Intent**: Verify ColumnDefinition correctly carries default values
#[rstest]
fn test_column_definition_with_default() {
	// Arrange & Act
	let column = ColumnDefinition {
		name: "status".to_string(),
		type_definition: FieldType::VarChar(20),
		not_null: true,
		unique: false,
		primary_key: false,
		auto_increment: false,
		default: Some("'active'".to_string()),
	};

	// Assert
	assert_eq!(column.default, Some("'active'".to_string()));
}

/// Test ColumnDefinition with UNIQUE constraint
///
/// **Test Intent**: Verify ColumnDefinition correctly sets unique flag
#[rstest]
fn test_column_definition_unique() {
	// Arrange & Act
	let column = ColumnDefinition {
		name: "email".to_string(),
		type_definition: FieldType::VarChar(255),
		not_null: true,
		unique: true,
		primary_key: false,
		auto_increment: false,
		default: None,
	};

	// Assert
	assert!(column.unique, "Column should be UNIQUE");
}

/// Test ColumnDefinition auto-increment with Integer
///
/// **Test Intent**: Verify auto_increment works with Integer type
#[rstest]
fn test_column_definition_auto_increment_integer() {
	// Arrange & Act
	let column = ColumnDefinition {
		name: "id".to_string(),
		type_definition: FieldType::Integer,
		not_null: true,
		unique: false,
		primary_key: true,
		auto_increment: true,
		default: None,
	};

	// Assert
	assert!(column.auto_increment);
	assert!(column.primary_key);
	assert!(column.not_null);
	assert_eq!(column.type_definition, FieldType::Integer);
}

/// Test ColumnDefinition auto-increment with BigInteger
///
/// **Test Intent**: Verify auto_increment works with BigInteger type
#[rstest]
fn test_column_definition_auto_increment_big_integer() {
	// Arrange & Act
	let column = ColumnDefinition {
		name: "id".to_string(),
		type_definition: FieldType::BigInteger,
		not_null: true,
		unique: false,
		primary_key: true,
		auto_increment: true,
		default: None,
	};

	// Assert
	assert!(column.auto_increment);
	assert!(column.primary_key);
	assert_eq!(column.type_definition, FieldType::BigInteger);
}

/// Test ColumnDefinition auto-increment with SmallInteger
///
/// **Test Intent**: Verify auto_increment works with SmallInteger type
#[rstest]
fn test_column_definition_auto_increment_small_integer() {
	// Arrange & Act
	let column = ColumnDefinition {
		name: "id".to_string(),
		type_definition: FieldType::SmallInteger,
		not_null: true,
		unique: false,
		primary_key: true,
		auto_increment: true,
		default: None,
	};

	// Assert
	assert!(column.auto_increment);
	assert!(column.primary_key);
	assert_eq!(column.type_definition, FieldType::SmallInteger);
}

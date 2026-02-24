//! Field Attribute Tests
//!
//! Tests for the `#[field(...)]` helper attribute used within `#[document]`.
//! Compilation behavior is validated via trybuild UI tests in `tests/ui/`.

use bson::oid::ObjectId;
use reinhardt_db::nosql::document::{Document, IndexOrder};
use reinhardt_db::nosql::error::{OdmError, ValidationError};
use reinhardt_db_macros::document;
use rstest::rstest;
use serde::{Deserialize, Serialize};

#[document(collection = "test_fields", backend = "mongodb")]
#[derive(Serialize, Deserialize, Debug)]
struct FieldTest {
	#[field(primary_key)]
	id: Option<ObjectId>,

	#[field(required, unique)]
	email: String,

	#[field(index)]
	username: String,

	#[field(min = 0, max = 100)]
	score: i32,

	#[field(rename = "display_name")]
	name: String,

	#[field(default = "Anonymous")]
	nickname: String,

	#[field(references = "companies")]
	company_id: Option<ObjectId>,
}

#[rstest]
fn test_indexes_generation() {
	// Act
	let indexes = FieldTest::indexes();

	// Assert: 2 indexes (email unique + username non-unique)
	assert_eq!(indexes.len(), 2);

	// First index: email (unique)
	assert_eq!(indexes[0].keys.len(), 1);
	assert_eq!(indexes[0].keys[0].field, "email");
	assert_eq!(indexes[0].keys[0].order, IndexOrder::Ascending);
	assert!(indexes[0].options.unique);

	// Second index: username (non-unique)
	assert_eq!(indexes[1].keys.len(), 1);
	assert_eq!(indexes[1].keys[0].field, "username");
	assert_eq!(indexes[1].keys[0].order, IndexOrder::Ascending);
	assert!(!indexes[1].options.unique);
}

#[rstest]
fn test_validate_required() {
	// Arrange: empty email triggers Required error
	let ft = FieldTest {
		id: None,
		email: String::new(),
		username: "testuser".to_string(),
		score: 50,
		name: "Test".to_string(),
		nickname: "nick".to_string(),
		company_id: None,
	};

	// Act
	let result = ft.validate();

	// Assert
	match result {
		Err(OdmError::Validation(ValidationError::Required(field))) => {
			assert_eq!(field, "email");
		}
		other => panic!("Expected Required(\"email\") error, got {:?}", other),
	}
}

#[rstest]
fn test_validate_min_max() {
	// Arrange: score below minimum
	let ft_low = FieldTest {
		id: None,
		email: "test@example.com".to_string(),
		username: "testuser".to_string(),
		score: -1,
		name: "Test".to_string(),
		nickname: "nick".to_string(),
		company_id: None,
	};

	// Act & Assert: score = -1 is out of range
	match ft_low.validate() {
		Err(OdmError::Validation(ValidationError::OutOfRange { field, min, max })) => {
			assert_eq!(field, "score");
			assert_eq!(min, 0);
			assert_eq!(max, 100);
		}
		other => panic!("Expected OutOfRange error for score = -1, got {:?}", other),
	}

	// Arrange: score above maximum
	let ft_high = FieldTest {
		id: None,
		email: "test@example.com".to_string(),
		username: "testuser".to_string(),
		score: 101,
		name: "Test".to_string(),
		nickname: "nick".to_string(),
		company_id: None,
	};

	// Act & Assert: score = 101 is out of range
	match ft_high.validate() {
		Err(OdmError::Validation(ValidationError::OutOfRange { field, min, max })) => {
			assert_eq!(field, "score");
			assert_eq!(min, 0);
			assert_eq!(max, 100);
		}
		other => panic!("Expected OutOfRange error for score = 101, got {:?}", other),
	}
}

#[rstest]
fn test_validate_passes() {
	// Arrange: all fields are valid
	let ft = FieldTest {
		id: None,
		email: "valid@example.com".to_string(),
		username: "testuser".to_string(),
		score: 50,
		name: "Test User".to_string(),
		nickname: "nick".to_string(),
		company_id: None,
	};

	// Act
	let result = ft.validate();

	// Assert
	assert!(result.is_ok());
}

#[rstest]
fn test_rename_serialization() {
	// Arrange
	let ft = FieldTest {
		id: None,
		email: "test@example.com".to_string(),
		username: "testuser".to_string(),
		score: 50,
		name: "Display Name".to_string(),
		nickname: "nick".to_string(),
		company_id: None,
	};

	// Act: Serialize to BSON document
	let doc = bson::serialize_to_document(&ft).unwrap();

	// Assert: field is renamed to "display_name"
	assert!(doc.contains_key("display_name"));
	assert!(!doc.contains_key("name"));
}

#[rstest]
fn test_default_deserialization() {
	// Arrange: BSON document without nickname field
	let doc = bson::doc! {
		"email": "test@example.com",
		"username": "testuser",
		"score": 50,
		"display_name": "Test User",
	};

	// Act: Deserialize without nickname (should use default)
	let ft: FieldTest = bson::deserialize_from_document(doc).unwrap();

	// Assert: nickname defaults to "Anonymous"
	assert_eq!(ft.nickname, "Anonymous");
}

#[rstest]
fn test_references_metadata() {
	// Act
	let refs = FieldTest::references();

	// Assert
	assert_eq!(refs.len(), 1);
	assert_eq!(refs[0], ("company_id", "companies"));
}

#[rstest]
fn test_validation_schema_generated() {
	// Act
	let schema = FieldTest::validation_schema();

	// Assert: schema is Some and contains expected keys
	assert!(schema.is_some());
	let schema = schema.unwrap();
	assert!(schema.contains_key("properties"));
	assert!(schema.contains_key("required"));
}

//! Document Macro Tests
//!
//! Tests for the `#[document(...)]` attribute macro.
//! Compilation behavior is validated via trybuild UI tests in `tests/ui/`.

use bson::oid::ObjectId;
use reinhardt_db::nosql::document::Document;
use reinhardt_db_macros::document;
use rstest::rstest;
use serde::{Deserialize, Serialize};

#[document(collection = "test_users", backend = "mongodb")]
#[derive(Serialize, Deserialize, Debug)]
struct TestUser {
	#[field(primary_key)]
	id: Option<ObjectId>,
	name: String,
	email: String,
}

#[document(collection = "test_items", backend = "mongodb", database = "custom_db")]
#[derive(Serialize, Deserialize, Debug)]
struct TestItem {
	#[field(primary_key)]
	id: Option<ObjectId>,
	title: String,
}

#[rstest]
fn test_document_collection_name() {
	// Assert
	assert_eq!(TestUser::COLLECTION_NAME, "test_users");
	assert_eq!(TestItem::COLLECTION_NAME, "test_items");
}

#[rstest]
fn test_document_database_name() {
	// Assert
	assert_eq!(TestUser::DATABASE_NAME, "default");
	assert_eq!(TestItem::DATABASE_NAME, "custom_db");
}

#[rstest]
fn test_document_id_operations() {
	// Arrange
	let mut user = TestUser {
		id: None,
		name: "Alice".to_string(),
		email: "alice@test.com".to_string(),
	};

	// Assert: ID is initially None
	assert!(user.id().is_none());

	// Act: Set ID
	let oid = ObjectId::new();
	user.set_id(oid);

	// Assert: ID is now set
	assert!(user.id().is_some());
	assert_eq!(user.id().unwrap(), &oid);
}

#[rstest]
fn test_document_backend_type() {
	// Act & Assert
	assert_eq!(
		TestUser::backend_type(),
		reinhardt_db::nosql::types::NoSQLBackendType::MongoDB
	);
}

#[rstest]
fn test_primary_key_serialized_as_id() {
	// Arrange
	let oid = ObjectId::new();
	let user = TestUser {
		id: Some(oid),
		name: "Bob".to_string(),
		email: "bob@test.com".to_string(),
	};

	// Act: Serialize to BSON document
	let doc = bson::serialize_to_document(&user).unwrap();

	// Assert: Primary key field is renamed to "_id"
	assert!(doc.contains_key("_id"));
	assert!(!doc.contains_key("id"));
}

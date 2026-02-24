//! Index Tests
//!
//! Tests index metadata generation, ensure_indexes, and unique enforcement
//! via the Repository API.

use crate::mongodb_fixtures::mongodb;
use bson::{doc, oid::ObjectId};
use futures::stream::TryStreamExt;
use reinhardt_db::nosql::document::Document;
use reinhardt_db_macros::document;
use rstest::rstest;
use serde::{Deserialize, Serialize};

/// Test document with indexes
#[document(collection = "test_indexes", backend = "mongodb")]
#[derive(Serialize, Deserialize, Debug)]
struct IndexTest {
	#[field(primary_key)]
	id: Option<ObjectId>,
	#[field(index)]
	indexed_field: String,
	#[field(unique)]
	unique_field: String,
}

/// Test that index metadata is generated correctly by the macro
///
/// Verifies that:
/// 1. indexes() returns the correct number of index definitions
/// 2. Field names and uniqueness flags are correct
#[rstest]
fn test_document_indexes_metadata() {
	// Arrange & Act
	let indexes = IndexTest::indexes();

	// Assert
	assert_eq!(indexes.len(), 2);

	// First index: indexed_field (non-unique)
	assert_eq!(indexes[0].keys.len(), 1);
	assert_eq!(indexes[0].keys[0].field, "indexed_field");
	assert!(!indexes[0].options.unique);

	// Second index: unique_field (unique)
	assert_eq!(indexes[1].keys.len(), 1);
	assert_eq!(indexes[1].keys[0].field, "unique_field");
	assert!(indexes[1].options.unique);
}

/// Test that ensure_indexes creates indexes in MongoDB
///
/// Verifies that:
/// 1. ensure_indexes() succeeds without error
/// 2. Indexes are actually created in MongoDB
#[rstest]
#[tokio::test]
async fn test_ensure_indexes(
	#[future] mongodb: (
		testcontainers::ContainerAsync<testcontainers::GenericImage>,
		reinhardt_db::nosql::backends::mongodb::MongoDBBackend,
	),
) {
	// Arrange
	let (_container, db) = mongodb.await;
	let repo = reinhardt_db::nosql::Repository::<IndexTest>::new(db);

	// Act
	repo.ensure_indexes().await.unwrap();

	// Assert: query MongoDB for actual index list
	let collection = repo
		.backend()
		.database()
		.collection::<bson::Document>("test_indexes");
	let cursor = collection.list_indexes().await.unwrap();
	let indexes: Vec<_> = cursor.try_collect().await.unwrap();

	// MongoDB always creates a default _id index, plus our 2 custom indexes
	assert!(indexes.len() >= 3);

	// Verify our custom indexes exist
	let index_names: Vec<String> = indexes
		.iter()
		.filter_map(|idx| idx.options.as_ref().and_then(|o| o.name.clone()))
		.collect();
	assert!(index_names.iter().any(|n| n.contains("indexed_field")));
	assert!(index_names.iter().any(|n| n.contains("unique_field")));

	// Cleanup: drop collection
	collection.drop().await.ok();
}

/// Test that unique index enforcement works through the Repository
///
/// Verifies that:
/// 1. After ensure_indexes, unique constraints are enforced
/// 2. Inserting a duplicate unique_field returns a DuplicateKey error
#[rstest]
#[tokio::test]
async fn test_unique_enforcement_via_repository(
	#[future] mongodb: (
		testcontainers::ContainerAsync<testcontainers::GenericImage>,
		reinhardt_db::nosql::backends::mongodb::MongoDBBackend,
	),
) {
	// Arrange
	let (_container, db) = mongodb.await;
	let repo = reinhardt_db::nosql::Repository::<IndexTest>::new(db);
	repo.ensure_indexes().await.unwrap();

	let mut doc1 = IndexTest {
		id: None,
		indexed_field: "value1".to_string(),
		unique_field: "unique_value".to_string(),
	};
	repo.insert(&mut doc1).await.unwrap();

	// Act: attempt to insert another document with the same unique_field
	let mut doc2 = IndexTest {
		id: None,
		indexed_field: "value2".to_string(),
		unique_field: "unique_value".to_string(),
	};
	let result = repo.insert(&mut doc2).await;

	// Assert: should fail with a duplicate key / serialization error
	assert!(result.is_err());

	// Cleanup: drop collection
	repo.backend()
		.database()
		.collection::<bson::Document>("test_indexes")
		.drop()
		.await
		.ok();
}

//! CRUD Operations Tests
//!
//! Tests Create, Read, Update, Delete operations using the Repository API.

use crate::mongodb_fixtures::mongodb;
use bson::{doc, oid::ObjectId};
use reinhardt_db::nosql::Repository;
use reinhardt_db::nosql::backends::mongodb::MongoDBBackend;
use reinhardt_db::nosql::document::Document;
use reinhardt_db::nosql::error::OdmError;
use reinhardt_db::nosql::types::FindOptions;
use reinhardt_db_macros::document;
use rstest::rstest;
use serde::{Deserialize, Serialize};
use testcontainers::{ContainerAsync, GenericImage};

/// Test document structure
#[document(collection = "test_users", backend = "mongodb")]
#[derive(Serialize, Deserialize, Debug)]
struct TestUser {
	#[field(primary_key)]
	id: Option<ObjectId>,
	#[field(required, unique)]
	email: String,
	name: String,
}

impl TestUser {
	fn new(email: &str, name: &str) -> Self {
		Self {
			id: None,
			email: email.to_string(),
			name: name.to_string(),
		}
	}
}

/// Test inserting a document via Repository
///
/// Verifies that:
/// 1. Documents can be inserted via Repository
/// 2. The document's ID is automatically set after insertion
#[rstest]
#[tokio::test]
async fn test_repository_insert(#[future] mongodb: (ContainerAsync<GenericImage>, MongoDBBackend)) {
	// Arrange
	let (_container, db) = mongodb.await;
	let repo = Repository::<TestUser>::new(db);
	let mut user = TestUser::new("test@example.com", "Test User");

	// Act
	repo.insert(&mut user).await.unwrap();

	// Assert
	assert!(user.id().is_some());

	// Cleanup
	repo.delete_by_id(user.id().unwrap()).await.ok();
}

/// Test finding a document by ID via Repository
///
/// Verifies that:
/// 1. Inserted documents can be retrieved by ID
/// 2. Retrieved data matches inserted data
#[rstest]
#[tokio::test]
async fn test_repository_find_by_id(
	#[future] mongodb: (ContainerAsync<GenericImage>, MongoDBBackend),
) {
	// Arrange
	let (_container, db) = mongodb.await;
	let repo = Repository::<TestUser>::new(db);
	let mut user = TestUser::new("find@example.com", "Find User");
	repo.insert(&mut user).await.unwrap();
	let id = user.id().unwrap().clone();

	// Act
	let found = repo.find_by_id(&id).await.unwrap();

	// Assert
	assert!(found.is_some());
	let found_user = found.unwrap();
	assert_eq!(found_user.email, "find@example.com");
	assert_eq!(found_user.name, "Find User");

	// Cleanup
	repo.delete_by_id(&id).await.ok();
}

/// Test updating a document via Repository
///
/// Verifies that:
/// 1. Documents can be updated via Repository
/// 2. Updated data is persisted correctly
#[rstest]
#[tokio::test]
async fn test_repository_update(#[future] mongodb: (ContainerAsync<GenericImage>, MongoDBBackend)) {
	// Arrange
	let (_container, db) = mongodb.await;
	let repo = Repository::<TestUser>::new(db);
	let mut user = TestUser::new("update@example.com", "Original Name");
	repo.insert(&mut user).await.unwrap();
	let id = user.id().unwrap().clone();

	// Act
	user.name = "Updated Name".to_string();
	repo.update(&user).await.unwrap();

	// Assert
	let found = repo.find_by_id(&id).await.unwrap().unwrap();
	assert_eq!(found.name, "Updated Name");
	assert_eq!(found.email, "update@example.com");

	// Cleanup
	repo.delete_by_id(&id).await.ok();
}

/// Test deleting a document via Repository
///
/// Verifies that:
/// 1. Documents can be deleted by ID
/// 2. Deleted documents cannot be found
#[rstest]
#[tokio::test]
async fn test_repository_delete(#[future] mongodb: (ContainerAsync<GenericImage>, MongoDBBackend)) {
	// Arrange
	let (_container, db) = mongodb.await;
	let repo = Repository::<TestUser>::new(db);
	let mut user = TestUser::new("delete@example.com", "Delete User");
	repo.insert(&mut user).await.unwrap();
	let id = user.id().unwrap().clone();

	// Act
	repo.delete_by_id(&id).await.unwrap();

	// Assert
	let found = repo.find_by_id(&id).await.unwrap();
	assert!(found.is_none());
}

/// Test finding multiple documents via Repository
///
/// Verifies that:
/// 1. Multiple documents can be retrieved with find_many
/// 2. Limit option restricts the number of results
#[rstest]
#[tokio::test]
async fn test_repository_find_many(
	#[future] mongodb: (ContainerAsync<GenericImage>, MongoDBBackend),
) {
	// Arrange
	let (_container, db) = mongodb.await;
	let repo = Repository::<TestUser>::new(db);
	let mut inserted_ids = Vec::new();

	for i in 0..5 {
		let mut user = TestUser::new(&format!("many{}@example.com", i), &format!("User {}", i));
		repo.insert(&mut user).await.unwrap();
		inserted_ids.push(user.id().unwrap().clone());
	}

	// Act
	let options = FindOptions::new().limit(3);
	let results = repo.find_many(doc! {}, options).await.unwrap();

	// Assert
	assert_eq!(results.len(), 3);

	// Cleanup
	for id in &inserted_ids {
		repo.delete_by_id(id).await.ok();
	}
}

/// Test finding a single document by filter via Repository
///
/// Verifies that:
/// 1. Documents can be retrieved by arbitrary BSON filter
/// 2. The correct document is returned
#[rstest]
#[tokio::test]
async fn test_repository_find_one_by_filter(
	#[future] mongodb: (ContainerAsync<GenericImage>, MongoDBBackend),
) {
	// Arrange
	let (_container, db) = mongodb.await;
	let repo = Repository::<TestUser>::new(db);
	let mut user = TestUser::new("filter@example.com", "Filter User");
	repo.insert(&mut user).await.unwrap();
	let id = user.id().unwrap().clone();

	// Act
	let found = repo
		.find_one(doc! { "email": "filter@example.com" })
		.await
		.unwrap();

	// Assert
	assert!(found.is_some());
	let found_user = found.unwrap();
	assert_eq!(found_user.email, "filter@example.com");
	assert_eq!(found_user.name, "Filter User");

	// Cleanup
	repo.delete_by_id(&id).await.ok();
}

/// Test updating a non-existent document returns NotFound
///
/// Verifies that:
/// 1. Updating a document with a non-existent ID returns OdmError::NotFound
#[rstest]
#[tokio::test]
async fn test_repository_update_not_found(
	#[future] mongodb: (ContainerAsync<GenericImage>, MongoDBBackend),
) {
	// Arrange
	let (_container, db) = mongodb.await;
	let repo = Repository::<TestUser>::new(db);
	let user = TestUser {
		id: Some(ObjectId::new()),
		email: "nonexistent@example.com".to_string(),
		name: "Nonexistent".to_string(),
	};

	// Act
	let result = repo.update(&user).await;

	// Assert
	assert!(matches!(result, Err(OdmError::NotFound)));
}

/// Test deleting a non-existent document returns NotFound
///
/// Verifies that:
/// 1. Deleting by a non-existent ID returns OdmError::NotFound
#[rstest]
#[tokio::test]
async fn test_repository_delete_not_found(
	#[future] mongodb: (ContainerAsync<GenericImage>, MongoDBBackend),
) {
	// Arrange
	let (_container, db) = mongodb.await;
	let repo = Repository::<TestUser>::new(db);
	let fake_id = ObjectId::new();

	// Act
	let result = repo.delete_by_id(&fake_id).await;

	// Assert
	assert!(matches!(result, Err(OdmError::NotFound)));
}

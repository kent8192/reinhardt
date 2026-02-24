//! Document-oriented database trait
//!
//! This module defines the trait for document-oriented NoSQL databases
//! like MongoDB, CouchDB, etc.

use async_trait::async_trait;

use crate::nosql::error::Result;
use crate::nosql::traits::NoSQLBackend;
use crate::nosql::types::{Document, FindOptions, UpdateResult};

/// Trait for document-oriented NoSQL databases
///
/// This trait provides methods for working with document databases,
/// which store data as semi-structured documents (typically JSON-like).
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_db::nosql::traits::DocumentBackend;
/// use bson::doc;
///
/// async fn find_user(db: &dyn DocumentBackend, email: &str) -> Result<Option<Document>> {
///     db.find_one("users", doc! { "email": email }).await
/// }
/// ```
#[async_trait]
pub trait DocumentBackend: NoSQLBackend {
	/// Finds a single document matching the filter
	///
	/// # Arguments
	///
	/// * `collection` - The name of the collection to search
	/// * `filter` - The filter criteria as a document
	///
	/// # Returns
	///
	/// Returns `Some(Document)` if a matching document is found, `None` otherwise.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let user = db.find_one("users", doc! { "id": 1 }).await?;
	/// ```
	async fn find_one(&self, collection: &str, filter: Document) -> Result<Option<Document>>;

	/// Finds multiple documents matching the filter
	///
	/// # Arguments
	///
	/// * `collection` - The name of the collection to search
	/// * `filter` - The filter criteria as a document
	/// * `options` - Optional query options (limit, skip, sort, etc.)
	///
	/// # Returns
	///
	/// Returns a vector of matching documents.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let options = FindOptions::new().limit(10).sort(doc! { "created_at": -1 });
	/// let users = db.find_many("users", doc! { "active": true }, options).await?;
	/// ```
	async fn find_many(
		&self,
		collection: &str,
		filter: Document,
		options: FindOptions,
	) -> Result<Vec<Document>>;

	/// Inserts a single document into the collection
	///
	/// # Arguments
	///
	/// * `collection` - The name of the collection
	/// * `document` - The document to insert
	///
	/// # Returns
	///
	/// Returns the ID of the inserted document as a string.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let id = db.insert_one("users", doc! {
	///     "name": "Alice",
	///     "email": "alice@example.com"
	/// }).await?;
	/// ```
	async fn insert_one(&self, collection: &str, document: Document) -> Result<String>;

	/// Inserts multiple documents into the collection
	///
	/// # Arguments
	///
	/// * `collection` - The name of the collection
	/// * `documents` - The documents to insert
	///
	/// # Returns
	///
	/// Returns a vector of IDs of the inserted documents.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let ids = db.insert_many("users", vec![
	///     doc! { "name": "Alice" },
	///     doc! { "name": "Bob" }
	/// ]).await?;
	/// ```
	async fn insert_many(&self, collection: &str, documents: Vec<Document>) -> Result<Vec<String>>;

	/// Updates a single document matching the filter
	///
	/// # Arguments
	///
	/// * `collection` - The name of the collection
	/// * `filter` - The filter criteria
	/// * `update` - The update operations to apply
	///
	/// # Returns
	///
	/// Returns an `UpdateResult` with information about the update operation.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let result = db.update_one(
	///     "users",
	///     doc! { "id": 1 },
	///     doc! { "$set": { "active": false } }
	/// ).await?;
	/// ```
	async fn update_one(
		&self,
		collection: &str,
		filter: Document,
		update: Document,
	) -> Result<UpdateResult>;

	/// Updates multiple documents matching the filter
	///
	/// # Arguments
	///
	/// * `collection` - The name of the collection
	/// * `filter` - The filter criteria
	/// * `update` - The update operations to apply
	///
	/// # Returns
	///
	/// Returns an `UpdateResult` with information about the update operation.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let result = db.update_many(
	///     "users",
	///     doc! { "status": "pending" },
	///     doc! { "$set": { "status": "active" } }
	/// ).await?;
	/// ```
	async fn update_many(
		&self,
		collection: &str,
		filter: Document,
		update: Document,
	) -> Result<UpdateResult>;

	/// Deletes a single document matching the filter
	///
	/// # Arguments
	///
	/// * `collection` - The name of the collection
	/// * `filter` - The filter criteria
	///
	/// # Returns
	///
	/// Returns the number of documents deleted (0 or 1).
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let count = db.delete_one("users", doc! { "id": 1 }).await?;
	/// ```
	async fn delete_one(&self, collection: &str, filter: Document) -> Result<u64>;

	/// Deletes multiple documents matching the filter
	///
	/// # Arguments
	///
	/// * `collection` - The name of the collection
	/// * `filter` - The filter criteria
	///
	/// # Returns
	///
	/// Returns the number of documents deleted.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let count = db.delete_many("users", doc! { "active": false }).await?;
	/// ```
	async fn delete_many(&self, collection: &str, filter: Document) -> Result<u64>;

	/// Executes an aggregation pipeline
	///
	/// # Arguments
	///
	/// * `collection` - The name of the collection
	/// * `pipeline` - The aggregation pipeline stages
	///
	/// # Returns
	///
	/// Returns a vector of result documents.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let results = db.aggregate("orders", vec![
	///     doc! { "$match": { "status": "completed" } },
	///     doc! { "$group": {
	///         "_id": "$customer_id",
	///         "total": { "$sum": "$amount" }
	///     }}
	/// ]).await?;
	/// ```
	async fn aggregate(&self, collection: &str, pipeline: Vec<Document>) -> Result<Vec<Document>>;
}

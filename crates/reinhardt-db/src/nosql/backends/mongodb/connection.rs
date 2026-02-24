//! MongoDB connection and backend implementation
//!
//! This module provides the MongoDB database backend that implements
//! the `DocumentBackend` and `NoSQLBackend` traits.
//!
//! # Example
//!
//! ```rust,no_run
//! use reinhardt_db::nosql::backends::mongodb::MongoDBBackend;
//! use reinhardt_db::nosql::traits::DocumentBackend;
//! use bson::doc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Connect to MongoDB
//! let backend = MongoDBBackend::connect("mongodb://localhost:27017").await?;
//!
//! // Use a specific database
//! let backend_with_db = backend.with_database("myapp");
//!
//! // Insert a document
//! let id = backend_with_db.insert_one("users", doc! {
//!     "name": "Alice",
//!     "email": "alice@example.com"
//! }).await?;
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use bson::{Bson, Document};
use mongodb::{Client, ClientSession, Database};
use std::sync::Arc;

use crate::nosql::error::{NoSQLError, Result};
use crate::nosql::traits::{DocumentBackend, NoSQLBackend};
use crate::nosql::types::{FindOptions, NoSQLBackendType, UpdateResult};

/// MongoDB backend implementation
///
/// Supports connection pooling, replica sets, and sharded clusters.
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_db::nosql::backends::mongodb::MongoDBBackend;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Basic connection
/// let backend = MongoDBBackend::connect("mongodb://localhost:27017").await?;
/// let backend = backend.with_database("mydb");
///
/// // Connection with options
/// let backend = MongoDBBackend::builder()
///     .url("mongodb://localhost:27017")
///     .database("mydb")
///     .max_pool_size(100)
///     .min_pool_size(10)
///     .build()
///     .await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct MongoDBBackend {
	client: Arc<Client>,
	database_name: String,
}

/// MongoDB transaction executor
///
/// This struct wraps a MongoDB `ClientSession` to ensure all operations
/// within a transaction run within the same session context.
///
/// # Note
///
/// MongoDB transactions require a replica set or sharded cluster.
/// Standalone MongoDB instances do not support transactions.
pub struct MongoDBTransactionExecutor {
	/// The MongoDB session (Option for consume-on-commit/rollback pattern)
	session: Option<ClientSession>,
	/// Reference to the client for accessing database/collections
	#[allow(dead_code)]
	client: Arc<Client>,
	/// Database name for operations
	#[allow(dead_code)]
	database_name: String,
}

impl MongoDBTransactionExecutor {
	/// Create a new MongoDB transaction executor
	pub fn new(session: ClientSession, client: Arc<Client>, database_name: String) -> Self {
		Self {
			session: Some(session),
			client,
			database_name,
		}
	}

	/// Commit the transaction
	pub async fn commit(mut self) -> Result<()> {
		let mut session = self
			.session
			.take()
			.ok_or_else(|| NoSQLError::DatabaseError("Transaction already consumed".to_string()))?;

		session.commit_transaction().await.map_err(|e| {
			NoSQLError::DatabaseError(format!("Failed to commit MongoDB transaction: {}", e))
		})
	}

	/// Rollback the transaction
	pub async fn rollback(mut self) -> Result<()> {
		let mut session = self
			.session
			.take()
			.ok_or_else(|| NoSQLError::DatabaseError("Transaction already consumed".to_string()))?;

		session.abort_transaction().await.map_err(|e| {
			NoSQLError::DatabaseError(format!("Failed to rollback MongoDB transaction: {}", e))
		})
	}
}

/// Builder for configuring MongoDB connections
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_db::nosql::backends::mongodb::MongoDBBackendBuilder;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let backend = MongoDBBackendBuilder::new()
///     .url("mongodb://localhost:27017")
///     .database("mydb")
///     .max_pool_size(100)
///     .min_pool_size(10)
///     .max_idle_time_secs(300)
///     .build()
///     .await?;
/// # Ok(())
/// # }
/// ```
pub struct MongoDBBackendBuilder {
	url: String,
	database: String,
	max_pool_size: Option<u32>,
	min_pool_size: Option<u32>,
	max_idle_time_secs: Option<u64>,
}

impl Default for MongoDBBackendBuilder {
	fn default() -> Self {
		Self::new()
	}
}

impl MongoDBBackendBuilder {
	/// Create a new builder with default settings
	///
	/// # Example
	///
	/// ```rust
	/// # use reinhardt_db::nosql::backends::mongodb::MongoDBBackendBuilder;
	/// let builder = MongoDBBackendBuilder::new();
	/// // Builder successfully created with default settings
	/// ```
	pub fn new() -> Self {
		Self {
			url: "mongodb://localhost:27017".to_string(),
			database: "test".to_string(),
			max_pool_size: None,
			min_pool_size: None,
			max_idle_time_secs: None,
		}
	}

	/// Set the MongoDB connection URL
	///
	/// # Example
	///
	/// ```rust
	/// # use reinhardt_db::nosql::backends::mongodb::MongoDBBackendBuilder;
	/// let builder = MongoDBBackendBuilder::new()
	///     .url("mongodb://localhost:27017");
	/// // URL successfully set
	/// ```
	pub fn url(mut self, url: impl Into<String>) -> Self {
		self.url = url.into();
		self
	}

	/// Set the database name
	///
	/// # Example
	///
	/// ```rust
	/// # use reinhardt_db::nosql::backends::mongodb::MongoDBBackendBuilder;
	/// let builder = MongoDBBackendBuilder::new()
	///     .database("mydb");
	/// // Database name successfully set
	/// ```
	pub fn database(mut self, database: impl Into<String>) -> Self {
		self.database = database.into();
		self
	}

	/// Set the maximum connection pool size
	///
	/// # Example
	///
	/// ```rust
	/// # use reinhardt_db::nosql::backends::mongodb::MongoDBBackendBuilder;
	/// let builder = MongoDBBackendBuilder::new()
	///     .max_pool_size(100);
	/// // Max pool size successfully set
	/// ```
	pub fn max_pool_size(mut self, size: u32) -> Self {
		self.max_pool_size = Some(size);
		self
	}

	/// Set the minimum connection pool size
	///
	/// # Example
	///
	/// ```rust
	/// # use reinhardt_db::nosql::backends::mongodb::MongoDBBackendBuilder;
	/// let builder = MongoDBBackendBuilder::new()
	///     .min_pool_size(10);
	/// // Min pool size successfully set
	/// ```
	pub fn min_pool_size(mut self, size: u32) -> Self {
		self.min_pool_size = Some(size);
		self
	}

	/// Set the maximum idle time for connections in seconds
	///
	/// # Example
	///
	/// ```rust
	/// # use reinhardt_db::nosql::backends::mongodb::MongoDBBackendBuilder;
	/// let builder = MongoDBBackendBuilder::new()
	///     .max_idle_time_secs(300);
	/// // Max idle time successfully set
	/// ```
	pub fn max_idle_time_secs(mut self, secs: u64) -> Self {
		self.max_idle_time_secs = Some(secs);
		self
	}

	/// Build the MongoDB backend
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_db::nosql::backends::mongodb::MongoDBBackendBuilder;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let backend = MongoDBBackendBuilder::new()
	///     .url("mongodb://localhost:27017")
	///     .database("mydb")
	///     .build()
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn build(self) -> Result<MongoDBBackend> {
		use mongodb::options::ClientOptions;
		use std::time::Duration;

		let mut options = ClientOptions::parse(&self.url)
			.await
			.map_err(|e| NoSQLError::ConnectionError(e.to_string()))?;

		// Configure connection pool
		if let Some(max_size) = self.max_pool_size {
			options.max_pool_size = Some(max_size);
		}

		if let Some(min_size) = self.min_pool_size {
			options.min_pool_size = Some(min_size);
		}

		if let Some(idle_time) = self.max_idle_time_secs {
			options.max_idle_time = Some(Duration::from_secs(idle_time));
		}

		let client = Client::with_options(options)
			.map_err(|e| NoSQLError::ConnectionError(e.to_string()))?;

		Ok(MongoDBBackend {
			client: Arc::new(client),
			database_name: self.database,
		})
	}
}

impl MongoDBBackend {
	/// Connect to MongoDB using a connection string
	///
	/// # Arguments
	///
	/// * `url` - MongoDB connection string (e.g., "mongodb://localhost:27017")
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_db::nosql::backends::mongodb::MongoDBBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let backend = MongoDBBackend::connect("mongodb://localhost:27017").await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn connect(url: &str) -> Result<Self> {
		let client = Client::with_uri_str(url)
			.await
			.map_err(|e| NoSQLError::ConnectionError(e.to_string()))?;

		Ok(Self {
			client: Arc::new(client),
			database_name: "test".to_string(), // Default database
		})
	}

	/// Create a builder for configuring the MongoDB connection
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_db::nosql::backends::mongodb::MongoDBBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let backend = MongoDBBackend::builder()
	///     .url("mongodb://localhost:27017")
	///     .database("mydb")
	///     .max_pool_size(100)
	///     .build()
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub fn builder() -> MongoDBBackendBuilder {
		MongoDBBackendBuilder::new()
	}

	/// Set the database name to use
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_db::nosql::backends::mongodb::MongoDBBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let backend = MongoDBBackend::connect("mongodb://localhost:27017").await?;
	/// let backend = backend.with_database("myapp");
	/// # Ok(())
	/// # }
	/// ```
	pub fn with_database(mut self, database_name: &str) -> Self {
		self.database_name = database_name.to_string();
		self
	}

	/// Get the MongoDB database instance
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_db::nosql::backends::mongodb::MongoDBBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let backend = MongoDBBackend::connect("mongodb://localhost:27017").await?;
	/// let db = backend.database();
	/// # Ok(())
	/// # }
	/// ```
	pub fn database(&self) -> Database {
		self.client.database(&self.database_name)
	}

	/// Begin a transaction
	///
	/// # Note
	///
	/// MongoDB transactions require a replica set or sharded cluster.
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_db::nosql::backends::mongodb::MongoDBBackend;
	/// use bson::doc;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let backend = MongoDBBackend::connect("mongodb://localhost:27017").await?
	///     .with_database("mydb");
	///
	/// // Begin transaction
	/// let mut tx = backend.begin_transaction().await?;
	///
	/// // Perform operations...
	///
	/// // Commit or rollback
	/// tx.commit().await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn begin_transaction(&self) -> Result<MongoDBTransactionExecutor> {
		// Start a new session from the client
		let mut session = self.client.start_session().await.map_err(|e| {
			NoSQLError::ConnectionError(format!("Failed to start MongoDB session: {}", e))
		})?;

		// Begin the transaction on the session
		session.start_transaction().await.map_err(|e| {
			NoSQLError::DatabaseError(format!("Failed to start MongoDB transaction: {}", e))
		})?;

		Ok(MongoDBTransactionExecutor::new(
			session,
			Arc::clone(&self.client),
			self.database_name.clone(),
		))
	}
}

#[async_trait]
impl NoSQLBackend for MongoDBBackend {
	fn backend_type(&self) -> NoSQLBackendType {
		NoSQLBackendType::MongoDB
	}

	async fn health_check(&self) -> Result<()> {
		// Perform a simple ping to check database connectivity
		let db = self.database();
		db.run_command(bson::doc! { "ping": 1 })
			.await
			.map_err(|e| NoSQLError::ConnectionError(format!("Health check failed: {}", e)))?;
		Ok(())
	}

	fn as_any(&self) -> &dyn std::any::Any {
		self
	}
}

#[async_trait]
impl DocumentBackend for MongoDBBackend {
	async fn find_one(&self, collection: &str, filter: Document) -> Result<Option<Document>> {
		let db = self.database();
		let coll = db.collection::<Document>(collection);

		coll.find_one(filter)
			.await
			.map_err(|e| NoSQLError::ExecutionError(e.to_string()))
	}

	async fn find_many(
		&self,
		collection: &str,
		filter: Document,
		options: FindOptions,
	) -> Result<Vec<Document>> {
		use futures::stream::TryStreamExt;

		let db = self.database();
		let coll = db.collection::<Document>(collection);

		// Convert FindOptions to MongoDB's FindOptions
		let mut mongo_options = mongodb::options::FindOptions::default();
		mongo_options.limit = options.limit;
		mongo_options.skip = options.skip;
		mongo_options.sort = options.sort;
		mongo_options.projection = options.projection;
		mongo_options.batch_size = options.batch_size;

		let cursor = coll
			.find(filter)
			.with_options(mongo_options)
			.await
			.map_err(|e| NoSQLError::ExecutionError(e.to_string()))?;

		cursor
			.try_collect()
			.await
			.map_err(|e| NoSQLError::ExecutionError(e.to_string()))
	}

	async fn insert_one(&self, collection: &str, document: Document) -> Result<String> {
		let db = self.database();
		let coll = db.collection::<Document>(collection);

		let result = coll
			.insert_one(document)
			.await
			.map_err(|e| NoSQLError::ExecutionError(e.to_string()))?;

		// Convert Bson to String
		match result.inserted_id {
			Bson::ObjectId(oid) => Ok(oid.to_hex()),
			Bson::String(s) => Ok(s),
			other => Ok(other.to_string()),
		}
	}

	async fn insert_many(&self, collection: &str, documents: Vec<Document>) -> Result<Vec<String>> {
		let db = self.database();
		let coll = db.collection::<Document>(collection);

		let result = coll
			.insert_many(documents)
			.await
			.map_err(|e| NoSQLError::ExecutionError(e.to_string()))?;

		// Convert Bson IDs to Strings
		let ids = result
			.inserted_ids
			.into_values()
			.map(|bson| match bson {
				Bson::ObjectId(oid) => oid.to_hex(),
				Bson::String(s) => s,
				other => other.to_string(),
			})
			.collect();

		Ok(ids)
	}

	async fn update_one(
		&self,
		collection: &str,
		filter: Document,
		update: Document,
	) -> Result<UpdateResult> {
		let db = self.database();
		let coll = db.collection::<Document>(collection);

		let result = coll
			.update_one(filter, update)
			.await
			.map_err(|e| NoSQLError::ExecutionError(e.to_string()))?;

		let upserted_id = result.upserted_id.map(|bson| match bson {
			Bson::ObjectId(oid) => oid.to_hex(),
			Bson::String(s) => s,
			other => other.to_string(),
		});

		Ok(UpdateResult::new(
			result.matched_count,
			result.modified_count,
			if upserted_id.is_some() { 1 } else { 0 },
			upserted_id,
		))
	}

	async fn update_many(
		&self,
		collection: &str,
		filter: Document,
		update: Document,
	) -> Result<UpdateResult> {
		let db = self.database();
		let coll = db.collection::<Document>(collection);

		let result = coll
			.update_many(filter, update)
			.await
			.map_err(|e| NoSQLError::ExecutionError(e.to_string()))?;

		let upserted_id = result.upserted_id.map(|bson| match bson {
			Bson::ObjectId(oid) => oid.to_hex(),
			Bson::String(s) => s,
			other => other.to_string(),
		});

		Ok(UpdateResult::new(
			result.matched_count,
			result.modified_count,
			if upserted_id.is_some() { 1 } else { 0 },
			upserted_id,
		))
	}

	async fn delete_one(&self, collection: &str, filter: Document) -> Result<u64> {
		let db = self.database();
		let coll = db.collection::<Document>(collection);

		let result = coll
			.delete_one(filter)
			.await
			.map_err(|e| NoSQLError::ExecutionError(e.to_string()))?;

		Ok(result.deleted_count)
	}

	async fn delete_many(&self, collection: &str, filter: Document) -> Result<u64> {
		let db = self.database();
		let coll = db.collection::<Document>(collection);

		let result = coll
			.delete_many(filter)
			.await
			.map_err(|e| NoSQLError::ExecutionError(e.to_string()))?;

		Ok(result.deleted_count)
	}

	async fn aggregate(&self, collection: &str, pipeline: Vec<Document>) -> Result<Vec<Document>> {
		use futures::stream::TryStreamExt;

		let db = self.database();
		let coll = db.collection::<Document>(collection);

		let cursor = coll
			.aggregate(pipeline)
			.await
			.map_err(|e| NoSQLError::ExecutionError(e.to_string()))?;

		cursor
			.try_collect()
			.await
			.map_err(|e| NoSQLError::ExecutionError(e.to_string()))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_builder_default() {
		let builder = MongoDBBackendBuilder::new();
		assert_eq!(builder.url, "mongodb://localhost:27017");
		assert_eq!(builder.database, "test");
		assert_eq!(builder.max_pool_size, None);
		assert_eq!(builder.min_pool_size, None);
	}

	#[test]
	fn test_builder_configuration() {
		let builder = MongoDBBackendBuilder::new()
			.url("mongodb://example.com:27017")
			.database("mydb")
			.max_pool_size(100)
			.min_pool_size(10)
			.max_idle_time_secs(300);

		assert_eq!(builder.url, "mongodb://example.com:27017");
		assert_eq!(builder.database, "mydb");
		assert_eq!(builder.max_pool_size, Some(100));
		assert_eq!(builder.min_pool_size, Some(10));
		assert_eq!(builder.max_idle_time_secs, Some(300));
	}

	#[test]
	fn test_backend_builder_method() {
		let builder = MongoDBBackend::builder();
		assert_eq!(builder.url, "mongodb://localhost:27017");
	}
}

//! # Reinhardt NoSQL
//!
//! NoSQL database abstractions for the Reinhardt framework.
//!
//! This crate provides a unified interface for working with various NoSQL databases,
//! organized by paradigm (Document, Key-Value, Column-Family, Graph).
//!
//! ## Features
//!
//! - **Document Databases**: MongoDB, CouchDB (planned)
//! - **Key-Value Stores**: Redis (planned), DynamoDB (planned)
//! - **Column-Family Stores**: Cassandra (planned)
//! - **Graph Databases**: Neo4j (planned)
//!
//! ## Architecture
//!
//! The crate is organized around a trait hierarchy.
//! See [`NoSQLBackend`] for the architecture diagram.
//!
//! ## Feature Flags
//!
//! Individual database backends can be enabled via feature flags:
//!
//! - `mongodb` - MongoDB support
//! - `redis` - Redis support (planned)
//! - `cassandra` - Cassandra support (planned)
//! - `dynamodb` - DynamoDB support (planned)
//! - `neo4j` - Neo4j support (planned)
//!
//! Convenience feature groups:
//!
//! - `nosql-all` - Enable all NoSQL backends
//! - `nosql-document` - Enable all document-oriented databases
//! - `nosql-key-value` - Enable all key-value stores
//! - `nosql-column` - Enable all column-family stores
//! - `nosql-graph` - Enable all graph databases
//! - `full` - Enable all features
//!
//! ## Example
//!
//! ```rust,ignore
//! use reinhardt_db::nosql::{
//!     backends::mongodb::MongoDBBackend,
//!     traits::DocumentBackend,
//! };
//! use bson::doc;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Connect to MongoDB
//!     let db = MongoDBBackend::new("mongodb://localhost:27017", "mydb").await?;
//!
//!     // Insert a document
//!     let id = db.insert_one("users", doc! {
//!         "name": "Alice",
//!         "email": "alice@example.com"
//!     }).await?;
//!
//!     // Find the document
//!     let user = db.find_one("users", doc! { "_id": id }).await?;
//!
//!     Ok(())
//! }
//! ```

pub mod backends;
pub mod document;
pub mod error;
pub mod repository;
pub mod traits;
pub mod types;

// Re-export commonly used types
pub use error::{NoSQLError, OdmError, OdmResult, Result, ValidationError};
pub use traits::{DocumentBackend, NoSQLBackend};
pub use types::{
	Document as BsonDocument, FindOptions, NoSQLBackendType, NoSQLType, QueryValue, Ttl,
	UpdateResult,
};

// Re-export ODM types
pub use document::{Document, IndexKey, IndexModel, IndexModelBuilder, IndexOptions, IndexOrder};

// Re-export Repository
#[cfg(feature = "mongodb")]
pub use repository::Repository;

/// Prelude module for convenient imports
///
/// Import everything from this module to get started quickly:
///
/// ```rust,ignore
/// use reinhardt_db::nosql::prelude::*;
/// ```
pub mod prelude {
	pub use super::document::{
		Document, IndexKey, IndexModel, IndexModelBuilder, IndexOptions, IndexOrder,
	};
	pub use super::error::{NoSQLError, OdmError, OdmResult, Result, ValidationError};
	pub use super::traits::{DocumentBackend, NoSQLBackend};
	pub use super::types::{
		Document as BsonDocument, FindOptions, NoSQLBackendType, NoSQLType, QueryValue, Ttl,
		UpdateResult,
	};

	#[cfg(feature = "mongodb")]
	pub use super::backends::mongodb;
	#[cfg(feature = "mongodb")]
	pub use super::repository::Repository;
}

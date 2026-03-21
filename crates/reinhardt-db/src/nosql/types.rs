//! Common types for NoSQL operations
//!
//! This module provides common types used across different NoSQL backends.

use serde::{Deserialize, Serialize};
use std::time::Duration;

#[cfg(not(feature = "mongodb"))]
use std::collections::HashMap;

/// NoSQL database types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NoSQLType {
	/// Document-oriented databases (MongoDB, CouchDB, etc.)
	Document,
	/// Key-Value stores (Redis, DynamoDB, etc.)
	KeyValue,
	/// Column-family stores (Cassandra, HBase, etc.)
	Column,
	/// Graph databases (Neo4j, etc.)
	Graph,
}

/// NoSQL backend identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NoSQLBackendType {
	/// MongoDB
	#[cfg(feature = "mongodb")]
	MongoDB,
	/// Redis
	#[cfg(feature = "redis")]
	Redis,
	/// Cassandra
	#[cfg(feature = "cassandra")]
	Cassandra,
	/// DynamoDB
	#[cfg(feature = "dynamodb")]
	DynamoDB,
	/// Neo4j
	#[cfg(feature = "neo4j")]
	Neo4j,
	/// Placeholder variant to prevent empty enum when no backend features are enabled.
	/// This variant is never constructed and exists only for compilation purposes.
	#[doc(hidden)]
	#[cfg(not(any(
		feature = "mongodb",
		feature = "redis",
		feature = "cassandra",
		feature = "dynamodb",
		feature = "neo4j"
	)))]
	__UnusedPlaceholder,
}

impl NoSQLBackendType {
	/// Returns the NoSQL paradigm type for this backend
	pub fn nosql_type(&self) -> NoSQLType {
		match self {
			#[cfg(feature = "mongodb")]
			NoSQLBackendType::MongoDB => NoSQLType::Document,
			#[cfg(feature = "redis")]
			NoSQLBackendType::Redis => NoSQLType::KeyValue,
			#[cfg(feature = "cassandra")]
			NoSQLBackendType::Cassandra => NoSQLType::Column,
			#[cfg(feature = "dynamodb")]
			NoSQLBackendType::DynamoDB => NoSQLType::KeyValue,
			#[cfg(feature = "neo4j")]
			NoSQLBackendType::Neo4j => NoSQLType::Graph,
			#[cfg(not(any(
				feature = "mongodb",
				feature = "redis",
				feature = "cassandra",
				feature = "dynamodb",
				feature = "neo4j"
			)))]
			NoSQLBackendType::__UnusedPlaceholder => {
				unreachable!("__UnusedPlaceholder should never be constructed")
			}
		}
	}
}

/// Generic document type for document-oriented databases
///
/// This is a re-export of the underlying document type used by MongoDB.
/// For document databases, this provides a common interface.
#[cfg(feature = "mongodb")]
pub use bson::Document;

/// Fallback document type when MongoDB is not enabled
#[cfg(not(feature = "mongodb"))]
pub type Document = HashMap<String, serde_json::Value>;

/// Result of an update operation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateResult {
	/// Number of documents matched by the filter
	pub matched_count: u64,
	/// Number of documents actually modified
	pub modified_count: u64,
	/// Number of documents upserted
	pub upserted_count: u64,
	/// ID of the upserted document (if any)
	pub upserted_id: Option<String>,
}

impl UpdateResult {
	/// Creates a new UpdateResult
	pub fn new(
		matched_count: u64,
		modified_count: u64,
		upserted_count: u64,
		upserted_id: Option<String>,
	) -> Self {
		Self {
			matched_count,
			modified_count,
			upserted_count,
			upserted_id,
		}
	}
}

/// Options for find operations in document databases
#[non_exhaustive]
#[derive(Debug, Clone, Default)]
pub struct FindOptions {
	/// Maximum number of documents to return
	pub limit: Option<i64>,
	/// Number of documents to skip
	pub skip: Option<u64>,
	/// Sort specification
	pub sort: Option<Document>,
	/// Projection specification
	pub projection: Option<Document>,
	/// Batch size for cursor operations
	pub batch_size: Option<u32>,
}

impl FindOptions {
	/// Creates a new FindOptions with default values
	pub fn new() -> Self {
		Self::default()
	}

	/// Sets the limit
	pub fn limit(mut self, limit: i64) -> Self {
		self.limit = Some(limit);
		self
	}

	/// Sets the skip
	pub fn skip(mut self, skip: u64) -> Self {
		self.skip = Some(skip);
		self
	}

	/// Sets the sort
	pub fn sort(mut self, sort: Document) -> Self {
		self.sort = Some(sort);
		self
	}

	/// Sets the projection
	pub fn projection(mut self, projection: Document) -> Self {
		self.projection = Some(projection);
		self
	}

	/// Sets the batch size
	pub fn batch_size(mut self, batch_size: u32) -> Self {
		self.batch_size = Some(batch_size);
		self
	}
}

/// Query value type for parameterized queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueryValue {
	/// Null value
	Null,
	/// Boolean value
	Bool(bool),
	/// Integer value
	Int(i64),
	/// Floating point value
	Float(f64),
	/// String value
	String(String),
	/// Binary data
	Bytes(Vec<u8>),
}

/// Time-to-live duration for key-value operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ttl(pub Duration);

impl From<Duration> for Ttl {
	fn from(duration: Duration) -> Self {
		Ttl(duration)
	}
}

impl From<Ttl> for Duration {
	fn from(ttl: Ttl) -> Self {
		ttl.0
	}
}

impl Ttl {
	/// Creates a new TTL from seconds
	pub fn from_secs(secs: u64) -> Self {
		Ttl(Duration::from_secs(secs))
	}

	/// Creates a new TTL from milliseconds
	pub fn from_millis(millis: u64) -> Self {
		Ttl(Duration::from_millis(millis))
	}

	/// Returns the TTL as seconds
	pub fn as_secs(&self) -> u64 {
		self.0.as_secs()
	}

	/// Returns the TTL as milliseconds
	pub fn as_millis(&self) -> u128 {
		self.0.as_millis()
	}
}

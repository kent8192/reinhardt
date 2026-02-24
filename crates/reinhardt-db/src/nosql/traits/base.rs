//! Base trait for all NoSQL backends
//!
//! This module defines the fundamental trait that all NoSQL backend
//! implementations must implement.

use async_trait::async_trait;

use crate::nosql::error::Result;
use crate::nosql::types::{NoSQLBackendType, NoSQLType};

#[cfg_attr(doc, aquamarine::aquamarine)]
/// Base trait for all NoSQL database backends
///
/// This trait defines the minimum interface that all NoSQL backends must implement.
/// Specific NoSQL paradigms (Document, KeyValue, Column, Graph) extend this trait
/// with their own specialized methods.
///
/// # Architecture
///
/// ```mermaid
/// classDiagram
///     class NoSQLBackend {
///         <<trait>>
///         +backend_type() NoSQLBackendType
///         +nosql_type() NoSQLType
///         +health_check() Result
///     }
///     class DocumentBackend {
///         <<trait>>
///         MongoDB
///         CouchDB
///     }
///     class KeyValueBackend {
///         <<trait>>
///         Redis
///         DynamoDB
///     }
///     class ColumnBackend {
///         <<trait>>
///         Cassandra
///     }
///     class GraphBackend {
///         <<trait>>
///         Neo4j
///     }
///
///     NoSQLBackend <|-- DocumentBackend
///     NoSQLBackend <|-- KeyValueBackend
///     NoSQLBackend <|-- ColumnBackend
///     NoSQLBackend <|-- GraphBackend
/// ```
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_db::nosql::traits::NoSQLBackend;
///
/// async fn check_backend_health(backend: &dyn NoSQLBackend) -> Result<()> {
///     backend.health_check().await
/// }
/// ```
#[async_trait]
pub trait NoSQLBackend: Send + Sync {
	/// Returns the specific backend type (MongoDB, Redis, etc.)
	fn backend_type(&self) -> NoSQLBackendType;

	/// Returns the NoSQL paradigm type (Document, KeyValue, Column, Graph)
	fn nosql_type(&self) -> NoSQLType {
		self.backend_type().nosql_type()
	}

	/// Performs a health check on the database connection
	///
	/// This method verifies that the database is accessible and operational.
	/// The exact implementation depends on the specific backend.
	///
	/// # Errors
	///
	/// Returns an error if the health check fails, indicating the database
	/// is not accessible or not functioning properly.
	async fn health_check(&self) -> Result<()>;

	/// Returns self as &dyn std::any::Any for downcasting
	///
	/// This enables runtime type checking and downcasting to concrete backend types.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// use reinhardt_db::nosql::backends::mongodb::MongoDBBackend;
	///
	/// if let Some(mongodb) = backend.as_any().downcast_ref::<MongoDBBackend>() {
	///     // Use MongoDB-specific methods
	/// }
	/// ```
	fn as_any(&self) -> &dyn std::any::Any;
}

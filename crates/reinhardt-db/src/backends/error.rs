//! Error types for database operations

use thiserror::Error;

/// Database operation errors
#[non_exhaustive]
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum DatabaseError {
	/// Feature not supported by this database
	#[error("Feature '{feature}' is not supported by {database}")]
	UnsupportedFeature { database: String, feature: String },

	/// Operation not supported by this backend
	#[error("Not supported: {0}")]
	NotSupported(String),

	/// SQL syntax error
	#[error("SQL syntax error: {0}")]
	SyntaxError(String),

	/// Type conversion error
	#[error("Type conversion error: {0}")]
	TypeError(String),

	/// Connection error
	#[error("Connection error: {0}")]
	ConnectionError(String),

	/// Query execution error
	#[error("Query execution error: {0}")]
	QueryError(String),

	/// Serialization/deserialization error
	#[error("Serialization error: {0}")]
	SerializationError(String),

	/// Configuration error
	#[error("Configuration error: {0}")]
	ConfigError(String),

	/// Column not found error
	#[error("Column not found: {0}")]
	ColumnNotFound(String),

	/// Transaction error
	#[error("Transaction error: {0}")]
	TransactionError(String),

	/// Generic database error
	#[error("Database error: {0}")]
	Other(String),
}

/// Result type for database operations
pub type Result<T> = std::result::Result<T, DatabaseError>;

impl From<serde_json::Error> for DatabaseError {
	fn from(err: serde_json::Error) -> Self {
		DatabaseError::SerializationError(err.to_string())
	}
}

impl From<sqlx::Error> for DatabaseError {
	fn from(err: sqlx::Error) -> Self {
		use sqlx::Error::*;
		match err {
			Configuration(msg) => DatabaseError::ConfigError(msg.to_string()),
			Database(e) => DatabaseError::QueryError(e.to_string()),
			Io(e) => DatabaseError::ConnectionError(e.to_string()),
			Tls(e) => DatabaseError::ConnectionError(e.to_string()),
			Protocol(msg) => DatabaseError::QueryError(msg),
			RowNotFound => DatabaseError::QueryError("Row not found".to_string()),
			TypeNotFound { type_name } => {
				DatabaseError::TypeError(format!("Type not found: {}", type_name))
			}
			ColumnIndexOutOfBounds { index, len } => DatabaseError::QueryError(format!(
				"Column index {} out of bounds (len: {})",
				index, len
			)),
			ColumnNotFound(name) => {
				DatabaseError::QueryError(format!("Column not found: {}", name))
			}
			ColumnDecode { index, source } => {
				DatabaseError::TypeError(format!("Failed to decode column {}: {}", index, source))
			}
			Decode(e) => DatabaseError::TypeError(e.to_string()),
			PoolTimedOut => DatabaseError::ConnectionError("Pool timed out".to_string()),
			PoolClosed => DatabaseError::ConnectionError("Pool closed".to_string()),
			WorkerCrashed => DatabaseError::ConnectionError("Worker crashed".to_string()),
			Migrate(e) => DatabaseError::QueryError(format!("Migration error: {}", e)),
			_ => DatabaseError::Other(err.to_string()),
		}
	}
}

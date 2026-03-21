//! Error types for connection pooling

use thiserror::Error;

#[non_exhaustive]
#[derive(Error, Debug)]
/// Defines possible pool error values.
pub enum PoolError {
	#[error("Pool is closed")]
	/// PoolClosed variant.
	PoolClosed,

	#[error("Connection timeout")]
	/// Timeout variant.
	Timeout,

	#[error("Pool exhausted (max connections reached)")]
	/// PoolExhausted variant.
	PoolExhausted,

	#[error("Invalid connection")]
	/// InvalidConnection variant.
	InvalidConnection,

	#[error("Database error: {0}")]
	/// Database variant.
	Database(#[from] sqlx::Error),

	#[error("Configuration error: {0}")]
	/// Config variant.
	Config(String),

	#[error("Connection error: {0}")]
	/// Connection variant.
	Connection(String),

	#[error("Pool not found: {0}")]
	/// PoolNotFound variant.
	PoolNotFound(String),
}

/// Type alias for pool result.
pub type PoolResult<T> = Result<T, PoolError>;

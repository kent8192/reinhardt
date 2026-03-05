//! Error types for connection pooling

use thiserror::Error;

#[non_exhaustive]
#[derive(Error, Debug)]
pub enum PoolError {
	#[error("Pool is closed")]
	PoolClosed,

	#[error("Connection timeout")]
	Timeout,

	#[error("Pool exhausted (max connections reached)")]
	PoolExhausted,

	#[error("Invalid connection")]
	InvalidConnection,

	#[error("Database error: {0}")]
	Database(#[from] sqlx::Error),

	#[error("Configuration error: {0}")]
	Config(String),

	#[error("Connection error: {0}")]
	Connection(String),

	#[error("Pool not found: {0}")]
	PoolNotFound(String),
}

pub type PoolResult<T> = Result<T, PoolError>;

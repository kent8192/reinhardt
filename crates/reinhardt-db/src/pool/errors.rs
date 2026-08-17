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

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn display_preserves_every_pool_error_message() {
		let cases = [
			(PoolError::PoolClosed, "Pool is closed"),
			(PoolError::Timeout, "Connection timeout"),
			(
				PoolError::PoolExhausted,
				"Pool exhausted (max connections reached)",
			),
			(PoolError::InvalidConnection, "Invalid connection"),
			(
				PoolError::Config("max must be positive".to_string()),
				"Configuration error: max must be positive",
			),
			(
				PoolError::Connection("refused".to_string()),
				"Connection error: refused",
			),
			(
				PoolError::PoolNotFound("analytics".to_string()),
				"Pool not found: analytics",
			),
		];

		for (error, expected) in cases {
			assert_eq!(error.to_string(), expected);
		}
	}

	#[test]
	fn sqlx_pool_closed_error_is_preserved_as_database_error() {
		let error = PoolError::from(sqlx::Error::PoolClosed);

		assert!(matches!(
			error,
			PoolError::Database(sqlx::Error::PoolClosed)
		));
	}
}

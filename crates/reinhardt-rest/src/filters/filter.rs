use async_trait::async_trait;
use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur during query filtering.
#[derive(Debug, Error)]
pub enum FilterError {
	/// A filter parameter provided by the client is invalid.
	#[error("Invalid filter parameter: {0}")]
	InvalidParameter(String),
	/// A general filtering error.
	#[error("Filter error: {0}")]
	FilterError(String),
	/// The resulting query is invalid.
	#[error("Invalid query: {0}")]
	InvalidQuery(String),
}

/// A convenience type alias for filter operation results.
pub type FilterResult<T> = Result<T, FilterError>;

/// A backend that applies query parameter filters to a SQL query string.
#[async_trait]
pub trait FilterBackend: Send + Sync {
	/// Applies filters from query parameters to the given SQL string and returns the modified SQL.
	async fn filter_queryset(
		&self,
		query_params: &HashMap<String, String>,
		sql: String,
	) -> FilterResult<String>;
}

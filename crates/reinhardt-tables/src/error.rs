//! Error types for reinhardt-tables

use thiserror::Error;

/// Errors that can occur when working with tables
#[derive(Debug, Error)]
pub enum TableError {
	/// Column with the specified name was not found
	#[error("Column '{0}' not found")]
	ColumnNotFound(String),

	/// Invalid sort order specified
	#[error("Invalid sort order: {0}")]
	InvalidSortOrder(String),

	/// Column is not filterable
	#[error("Column '{0}' is not filterable")]
	ColumnNotFilterable(String),

	/// Invalid page number specified
	#[error("Invalid page number: {0}")]
	InvalidPageNumber(usize),

	/// Invalid per-page value specified
	#[error("Invalid per-page value: {0}")]
	InvalidPerPage(usize),

	/// Export operation failed
	#[error("Export failed: {0}")]
	ExportError(#[from] ExportError),
}

/// Errors that can occur during export operations
#[derive(Debug, Error)]
pub enum ExportError {
	/// CSV serialization failed
	#[cfg(feature = "export")]
	#[error("CSV serialization failed: {0}")]
	CsvError(#[from] csv::Error),

	/// JSON serialization failed
	#[cfg(feature = "export")]
	#[error("JSON serialization failed: {0}")]
	JsonError(#[from] serde_json::Error),

	/// Export feature not enabled
	#[cfg(not(feature = "export"))]
	#[error("Export feature not enabled. Enable 'export' feature to use this functionality")]
	FeatureNotEnabled,
}

/// Result type for table operations
pub type Result<T> = std::result::Result<T, TableError>;

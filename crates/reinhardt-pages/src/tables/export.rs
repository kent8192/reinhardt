//! Export functionality for tables

pub mod csv;
pub mod json;

use std::io::Write;

/// Export format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
	/// CSV format
	CSV,
	/// JSON format
	JSON,
	/// Excel format (XLSX)
	Excel,
	/// YAML format
	YAML,
}

/// Trait for exportable tables
pub trait Exportable {
	/// Exports the table to the specified format
	fn export<W: Write>(&self, writer: &mut W, format: ExportFormat) -> Result<(), ExportError>;
}

/// Export error
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
	/// I/O error
	#[error("I/O error: {0}")]
	Io(#[from] std::io::Error),
	/// Serialization error
	#[error("Serialization error: {0}")]
	Serialization(String),
	/// Unsupported format
	#[error("Unsupported format: {0:?}")]
	UnsupportedFormat(ExportFormat),
}

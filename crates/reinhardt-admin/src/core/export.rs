//! Export functionality for admin data
//!
//! This module provides export capabilities for admin data in various formats
//! including CSV, JSON, and Excel.

use crate::types::{AdminError, AdminResult};
use csv::Writer;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Export format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportFormat {
	/// Comma-separated values
	CSV,
	/// JSON format
	JSON,
	/// Excel format (XLSX)
	Excel,
	/// Tab-separated values
	TSV,
	/// XML format
	XML,
}

impl ExportFormat {
	/// Get file extension for this format
	pub fn extension(&self) -> &'static str {
		match self {
			ExportFormat::CSV => "csv",
			ExportFormat::JSON => "json",
			ExportFormat::Excel => "xlsx",
			ExportFormat::TSV => "tsv",
			ExportFormat::XML => "xml",
		}
	}

	/// Get MIME type for this format
	pub fn mime_type(&self) -> &'static str {
		match self {
			ExportFormat::CSV => "text/csv",
			ExportFormat::JSON => "application/json",
			ExportFormat::Excel => {
				"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
			}
			ExportFormat::TSV => "text/tab-separated-values",
			ExportFormat::XML => "application/xml",
		}
	}
}

/// Export configuration
///
/// # Examples
///
/// ```
/// use reinhardt_admin::core::ExportConfig;
/// use reinhardt_admin::core::export::ExportFormat;
///
/// let config = ExportConfig::new("User", ExportFormat::CSV)
///     .with_field("id")
///     .with_field("username")
///     .with_field("email");
///
/// assert_eq!(config.model_name(), "User");
/// assert_eq!(config.field_count(), 3);
/// ```
#[derive(Debug, Clone)]
pub struct ExportConfig {
	/// Model name
	model_name: String,
	/// Export format
	format: ExportFormat,
	/// Fields to export (empty means all fields)
	fields: Vec<String>,
	/// Field labels (for headers)
	field_labels: HashMap<String, String>,
	/// Filter conditions
	filters: HashMap<String, String>,
	/// Sort order
	ordering: Vec<String>,
	/// Maximum number of rows to export
	max_rows: Option<usize>,
	/// Include column headers
	include_headers: bool,
}

impl ExportConfig {
	/// Create a new export configuration
	pub fn new(model_name: impl Into<String>, format: ExportFormat) -> Self {
		Self {
			model_name: model_name.into(),
			format,
			fields: Vec::new(),
			field_labels: HashMap::new(),
			filters: HashMap::new(),
			ordering: Vec::new(),
			max_rows: None,
			include_headers: true,
		}
	}

	/// Get model name
	pub fn model_name(&self) -> &str {
		&self.model_name
	}

	/// Get export format
	pub fn format(&self) -> ExportFormat {
		self.format
	}

	/// Add a field to export
	pub fn with_field(mut self, field: impl Into<String>) -> Self {
		self.fields.push(field.into());
		self
	}

	/// Set fields to export
	pub fn with_fields(mut self, fields: Vec<String>) -> Self {
		self.fields = fields;
		self
	}

	/// Get fields
	pub fn fields(&self) -> &[String] {
		&self.fields
	}

	/// Get field count
	pub fn field_count(&self) -> usize {
		self.fields.len()
	}

	/// Set field label
	pub fn with_field_label(mut self, field: impl Into<String>, label: impl Into<String>) -> Self {
		self.field_labels.insert(field.into(), label.into());
		self
	}

	/// Get field label
	pub fn get_field_label(&self, field: &str) -> Option<&String> {
		self.field_labels.get(field)
	}

	/// Add a filter
	pub fn with_filter(mut self, field: impl Into<String>, value: impl Into<String>) -> Self {
		self.filters.insert(field.into(), value.into());
		self
	}

	/// Get filters
	pub fn filters(&self) -> &HashMap<String, String> {
		&self.filters
	}

	/// Set ordering
	pub fn with_ordering(mut self, ordering: Vec<String>) -> Self {
		self.ordering = ordering;
		self
	}

	/// Get ordering
	pub fn ordering(&self) -> &[String] {
		&self.ordering
	}

	/// Set maximum rows
	pub fn with_max_rows(mut self, max: usize) -> Self {
		self.max_rows = Some(max);
		self
	}

	/// Get maximum rows
	pub fn max_rows(&self) -> Option<usize> {
		self.max_rows
	}

	/// Set whether to include headers
	pub fn with_headers(mut self, include: bool) -> Self {
		self.include_headers = include;
		self
	}

	/// Check if headers should be included
	pub fn include_headers(&self) -> bool {
		self.include_headers
	}
}

/// Export result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
	/// Exported data as bytes
	pub data: Vec<u8>,
	/// MIME type
	pub mime_type: String,
	/// Suggested filename
	pub filename: String,
	/// Number of rows exported
	pub row_count: usize,
}

impl ExportResult {
	/// Create a new export result
	pub fn new(
		data: Vec<u8>,
		mime_type: impl Into<String>,
		filename: impl Into<String>,
		row_count: usize,
	) -> Self {
		Self {
			data,
			mime_type: mime_type.into(),
			filename: filename.into(),
			row_count,
		}
	}

	/// Get data size in bytes
	pub fn size_bytes(&self) -> usize {
		self.data.len()
	}

	/// Get data size in kilobytes
	pub fn size_kb(&self) -> f64 {
		self.data.len() as f64 / 1024.0
	}
}

/// CSV exporter
pub struct CsvExporter;

impl CsvExporter {
	/// Export data to CSV format
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::core::CsvExporter;
	/// use std::collections::HashMap;
	///
	/// let fields = vec!["id".to_string(), "name".to_string()];
	/// let mut row1 = HashMap::new();
	/// row1.insert("id".to_string(), "1".to_string());
	/// row1.insert("name".to_string(), "Alice".to_string());
	///
	/// let data = vec![row1];
	/// let result = CsvExporter::export(&fields, &data, true);
	///
	/// assert!(result.is_ok());
	/// ```
	pub fn export(
		fields: &[String],
		data: &[HashMap<String, String>],
		include_headers: bool,
	) -> AdminResult<Vec<u8>> {
		// Use csv crate for RFC 4180 compliant CSV writing
		let mut writer = Writer::from_writer(Vec::new());

		// Write headers
		if include_headers {
			writer.write_record(fields).map_err(|e| {
				AdminError::ValidationError(format!("Failed to write CSV headers: {}", e))
			})?;
		}

		// Write data rows
		for row in data {
			let values: Vec<&str> = fields
				.iter()
				.map(|field| row.get(field).map(|v| v.as_str()).unwrap_or(""))
				.collect();

			writer.write_record(&values).map_err(|e| {
				AdminError::ValidationError(format!("Failed to write CSV row: {}", e))
			})?;
		}

		// Flush and get the output
		writer.flush().map_err(|e| {
			AdminError::ValidationError(format!("Failed to flush CSV writer: {}", e))
		})?;

		let output = writer
			.into_inner()
			.map_err(|e| AdminError::ValidationError(format!("Failed to get CSV output: {}", e)))?;

		Ok(output)
	}
}

/// JSON exporter
pub struct JsonExporter;

impl JsonExporter {
	/// Export data to JSON format
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::core::JsonExporter;
	/// use std::collections::HashMap;
	///
	/// let mut row1 = HashMap::new();
	/// row1.insert("id".to_string(), "1".to_string());
	/// row1.insert("name".to_string(), "Alice".to_string());
	///
	/// let data = vec![row1];
	/// let result = JsonExporter::export(&data);
	///
	/// assert!(result.is_ok());
	/// ```
	pub fn export(data: &[HashMap<String, String>]) -> AdminResult<Vec<u8>> {
		serde_json::to_vec_pretty(data)
			.map_err(|e| AdminError::ValidationError(format!("JSON export failed: {}", e)))
	}
}

/// TSV (Tab-Separated Values) exporter
pub struct TsvExporter;

impl TsvExporter {
	/// Export data to TSV format
	pub fn export(
		fields: &[String],
		data: &[HashMap<String, String>],
		include_headers: bool,
	) -> AdminResult<Vec<u8>> {
		let mut output = Vec::new();

		// Write headers
		if include_headers {
			let header_line = fields.join("\t");
			output.extend_from_slice(header_line.as_bytes());
			output.extend_from_slice(b"\r\n");
		}

		// Write data rows
		for row in data {
			let values: Vec<String> = fields
				.iter()
				.map(|field| {
					// Escape tabs, newlines, and carriage returns to prevent field corruption
					// Replace \r\n first to avoid double-space from individual \r and \n replacements
					row.get(field)
						.map(|v| v.replace("\r\n", " ").replace(['\t', '\n', '\r'], " "))
						.unwrap_or_default()
				})
				.collect();
			let line = values.join("\t");
			output.extend_from_slice(line.as_bytes());
			output.extend_from_slice(b"\r\n");
		}

		Ok(output)
	}
}

/// Export builder for fluent API
///
/// # Examples
///
/// ```
/// use reinhardt_admin::core::ExportBuilder;
/// use reinhardt_admin::core::export::ExportFormat;
/// use std::collections::HashMap;
///
/// let mut row = HashMap::new();
/// row.insert("id".to_string(), "1".to_string());
///
/// let result = ExportBuilder::new("User", ExportFormat::CSV)
///     .field("id")
///     .field("username")
///     .data(vec![row])
///     .build();
///
/// assert!(result.is_ok());
/// ```
pub struct ExportBuilder {
	config: ExportConfig,
	data: Vec<HashMap<String, String>>,
}

impl ExportBuilder {
	/// Create a new export builder
	pub fn new(model_name: impl Into<String>, format: ExportFormat) -> Self {
		Self {
			config: ExportConfig::new(model_name, format),
			data: Vec::new(),
		}
	}

	/// Add a field
	pub fn field(mut self, field: impl Into<String>) -> Self {
		self.config = self.config.with_field(field);
		self
	}

	/// Add fields
	pub fn fields(mut self, fields: Vec<String>) -> Self {
		self.config = self.config.with_fields(fields);
		self
	}

	/// Set field label
	pub fn field_label(mut self, field: impl Into<String>, label: impl Into<String>) -> Self {
		self.config = self.config.with_field_label(field, label);
		self
	}

	/// Set data
	pub fn data(mut self, data: Vec<HashMap<String, String>>) -> Self {
		self.data = data;
		self
	}

	/// Set maximum rows
	pub fn max_rows(mut self, max: usize) -> Self {
		self.config = self.config.with_max_rows(max);
		self
	}

	/// Build and export
	pub fn build(self) -> AdminResult<ExportResult> {
		let fields = if self.config.fields().is_empty() {
			// Extract all unique field names from data
			let mut all_fields: Vec<String> = self
				.data
				.iter()
				.flat_map(|row| row.keys().cloned())
				.collect::<std::collections::HashSet<_>>()
				.into_iter()
				.collect();
			all_fields.sort();
			all_fields
		} else {
			self.config.fields().to_vec()
		};

		// Apply max_rows limit if configured
		let effective_data = if let Some(max) = self.config.max_rows() {
			&self.data[..self.data.len().min(max)]
		} else {
			&self.data
		};

		let exported = match self.config.format() {
			ExportFormat::CSV => {
				CsvExporter::export(&fields, effective_data, self.config.include_headers())?
			}
			ExportFormat::JSON => JsonExporter::export(effective_data)?,
			ExportFormat::TSV => {
				TsvExporter::export(&fields, effective_data, self.config.include_headers())?
			}
			ExportFormat::Excel | ExportFormat::XML => {
				return Err(AdminError::ValidationError(format!(
					"{:?} export not yet implemented",
					self.config.format()
				)));
			}
		};

		let filename = format!(
			"{}_{}.{}",
			self.config.model_name(),
			chrono::Utc::now().format("%Y%m%d_%H%M%S"),
			self.config.format().extension()
		);

		Ok(ExportResult::new(
			exported,
			self.config.format().mime_type().to_string(),
			filename,
			effective_data.len(),
		))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[test]
	fn test_export_format_extension() {
		assert_eq!(ExportFormat::CSV.extension(), "csv");
		assert_eq!(ExportFormat::JSON.extension(), "json");
		assert_eq!(ExportFormat::Excel.extension(), "xlsx");
		assert_eq!(ExportFormat::TSV.extension(), "tsv");
	}

	#[test]
	fn test_export_format_mime_type() {
		assert_eq!(ExportFormat::CSV.mime_type(), "text/csv");
		assert_eq!(ExportFormat::JSON.mime_type(), "application/json");
	}

	#[test]
	fn test_export_config_new() {
		let config = ExportConfig::new("User", ExportFormat::CSV);
		assert_eq!(config.model_name(), "User");
		assert_eq!(config.format(), ExportFormat::CSV);
		assert!(config.include_headers());
	}

	#[test]
	fn test_export_config_with_field() {
		let config = ExportConfig::new("User", ExportFormat::CSV)
			.with_field("id")
			.with_field("username");

		assert_eq!(config.field_count(), 2);
	}

	#[test]
	fn test_csv_exporter_basic() {
		let fields = vec!["id".to_string(), "name".to_string()];
		let mut row1 = HashMap::new();
		row1.insert("id".to_string(), "1".to_string());
		row1.insert("name".to_string(), "Alice".to_string());

		let mut row2 = HashMap::new();
		row2.insert("id".to_string(), "2".to_string());
		row2.insert("name".to_string(), "Bob".to_string());

		let data = vec![row1, row2];
		let result = CsvExporter::export(&fields, &data, true);

		assert!(result.is_ok());
		let output = String::from_utf8(result.unwrap()).unwrap();
		assert!(output.contains("id,name"));
		assert!(output.contains("1,Alice"));
		assert!(output.contains("2,Bob"));
	}

	#[test]
	fn test_csv_exporter_escape() {
		let fields = vec!["id".to_string(), "name".to_string()];
		let mut row = HashMap::new();
		row.insert("id".to_string(), "1".to_string());
		row.insert("name".to_string(), "Smith, John".to_string());

		let data = vec![row];
		let result = CsvExporter::export(&fields, &data, true);

		assert!(result.is_ok());
		let output = String::from_utf8(result.unwrap()).unwrap();
		assert!(output.contains("\"Smith, John\""));
	}

	#[test]
	fn test_json_exporter() {
		let mut row1 = HashMap::new();
		row1.insert("id".to_string(), "1".to_string());
		row1.insert("name".to_string(), "Alice".to_string());

		let data = vec![row1];
		let result = JsonExporter::export(&data);

		assert!(result.is_ok());
		let output = String::from_utf8(result.unwrap()).unwrap();
		assert!(output.contains("\"id\""));
		assert!(output.contains("\"Alice\""));
	}

	#[test]
	fn test_tsv_exporter() {
		let fields = vec!["id".to_string(), "name".to_string()];
		let mut row = HashMap::new();
		row.insert("id".to_string(), "1".to_string());
		row.insert("name".to_string(), "Alice".to_string());

		let data = vec![row];
		let result = TsvExporter::export(&fields, &data, true);

		assert!(result.is_ok());
		let output = String::from_utf8(result.unwrap()).unwrap();
		assert!(output.contains("id\tname\r\n"));
		assert!(output.contains("1\tAlice\r\n"));
	}

	#[rstest]
	fn test_tsv_exporter_escapes_newlines() {
		let fields = vec!["id".to_string(), "bio".to_string()];
		let mut row = HashMap::new();
		row.insert("id".to_string(), "1".to_string());
		row.insert("bio".to_string(), "line1\nline2\r\nline3".to_string());

		let data = vec![row];
		let result = TsvExporter::export(&fields, &data, true);

		assert!(result.is_ok());
		let output = String::from_utf8(result.unwrap()).unwrap();
		// Newlines and carriage returns in field values must be escaped
		assert_eq!(output, "id\tbio\r\n1\tline1 line2 line3\r\n");
	}

	#[test]
	fn test_export_builder() {
		let mut row = HashMap::new();
		row.insert("id".to_string(), "1".to_string());
		row.insert("username".to_string(), "alice".to_string());

		let result = ExportBuilder::new("User", ExportFormat::CSV)
			.field("id")
			.field("username")
			.data(vec![row])
			.build();

		let export = result.unwrap();
		assert_eq!(export.row_count, 1);
		assert!(export.filename.starts_with("User_"));
		assert!(export.filename.ends_with(".csv"));
	}

	#[test]
	fn test_export_result() {
		let data = vec![1, 2, 3, 4, 5];
		let result = ExportResult::new(data, "text/csv".to_string(), "test.csv".to_string(), 10);

		assert_eq!(result.row_count, 10);
		assert_eq!(result.size_bytes(), 5);
		assert!((result.size_kb() - 0.00488).abs() < 0.001);
	}

	#[test]
	fn test_export_config_filters() {
		let config = ExportConfig::new("User", ExportFormat::CSV)
			.with_filter("status", "active")
			.with_filter("role", "admin");

		assert_eq!(config.filters().len(), 2);
		assert_eq!(config.filters().get("status"), Some(&"active".to_string()));
	}

	#[test]
	fn test_export_config_ordering() {
		let config = ExportConfig::new("User", ExportFormat::CSV)
			.with_ordering(vec!["name".to_string(), "-created_at".to_string()]);

		assert_eq!(config.ordering().len(), 2);
	}

	#[rstest]
	fn test_export_builder_max_rows_truncates_output() {
		// Arrange
		let mut row1 = HashMap::new();
		row1.insert("id".to_string(), "1".to_string());
		row1.insert("name".to_string(), "Alice".to_string());

		let mut row2 = HashMap::new();
		row2.insert("id".to_string(), "2".to_string());
		row2.insert("name".to_string(), "Bob".to_string());

		let mut row3 = HashMap::new();
		row3.insert("id".to_string(), "3".to_string());
		row3.insert("name".to_string(), "Carol".to_string());

		// Act
		let result = ExportBuilder::new("User", ExportFormat::CSV)
			.field("id")
			.field("name")
			.data(vec![row1, row2, row3])
			.max_rows(2)
			.build();

		// Assert
		let export = result.unwrap();
		assert_eq!(export.row_count, 2);
		let output = String::from_utf8(export.data).unwrap();
		assert!(output.contains("1,Alice"));
		assert!(output.contains("2,Bob"));
		assert!(!output.contains("3,Carol"));
	}
}

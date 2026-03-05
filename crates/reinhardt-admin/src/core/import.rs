//! Import functionality for admin data
//!
//! This module provides import capabilities for admin data from various formats
//! including CSV and JSON.

use crate::types::{AdminError, AdminResult};
use csv::ReaderBuilder;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Cursor;

/// Import format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImportFormat {
	/// Comma-separated values
	CSV,
	/// JSON format
	JSON,
	/// Tab-separated values
	TSV,
}

impl ImportFormat {
	/// Get file extensions for this format
	pub fn extensions(&self) -> &[&'static str] {
		match self {
			ImportFormat::CSV => &["csv"],
			ImportFormat::JSON => &["json"],
			ImportFormat::TSV => &["tsv", "tab"],
		}
	}

	/// Detect format from filename
	pub fn from_filename(filename: &str) -> Option<Self> {
		let ext = filename.split('.').next_back()?.to_lowercase();
		match ext.as_str() {
			"csv" => Some(ImportFormat::CSV),
			"json" => Some(ImportFormat::JSON),
			"tsv" | "tab" => Some(ImportFormat::TSV),
			_ => None,
		}
	}

	/// Detect format from Content-Type header
	///
	/// Supports standard MIME types and common variations.
	pub fn from_content_type(content_type: &str) -> Option<Self> {
		// Extract the main MIME type (ignore charset and other parameters)
		let mime_type = content_type.split(';').next()?.trim().to_lowercase();

		match mime_type.as_str() {
			// JSON formats
			"application/json" | "text/json" => Some(ImportFormat::JSON),
			// CSV formats
			"text/csv" | "application/csv" => Some(ImportFormat::CSV),
			// TSV formats
			"text/tab-separated-values" | "text/tsv" => Some(ImportFormat::TSV),
			_ => None,
		}
	}
}

/// Import configuration
///
/// # Examples
///
/// ```
/// use reinhardt_admin::core::{ImportConfig, ImportFormat};
///
/// let config = ImportConfig::new("User", ImportFormat::CSV)
///     .with_field_mapping("username", "login")
///     .skip_duplicates(true)
///     .update_existing(false);
///
/// assert_eq!(config.model_name(), "User");
/// ```
#[derive(Debug, Clone)]
pub struct ImportConfig {
	/// Model name
	model_name: String,
	/// Import format
	format: ImportFormat,
	/// Field mappings (import_field -> model_field)
	field_mappings: HashMap<String, String>,
	/// Fields to skip during import
	skip_fields: Vec<String>,
	/// Skip duplicate records
	skip_duplicates: bool,
	/// Update existing records
	update_existing: bool,
	/// Key field for duplicate detection
	key_field: Option<String>,
	/// Maximum records to import
	max_records: Option<usize>,
	/// Skip header row (for CSV/TSV)
	skip_header: bool,
	/// Validate before import
	validate_first: bool,
}

impl ImportConfig {
	/// Create a new import configuration
	pub fn new(model_name: impl Into<String>, format: ImportFormat) -> Self {
		Self {
			model_name: model_name.into(),
			format,
			field_mappings: HashMap::new(),
			skip_fields: Vec::new(),
			skip_duplicates: false,
			update_existing: false,
			key_field: None,
			max_records: None,
			skip_header: true,
			validate_first: true,
		}
	}

	/// Get model name
	pub fn model_name(&self) -> &str {
		&self.model_name
	}

	/// Get import format
	pub fn format(&self) -> ImportFormat {
		self.format
	}

	/// Add field mapping
	pub fn with_field_mapping(
		mut self,
		import_field: impl Into<String>,
		model_field: impl Into<String>,
	) -> Self {
		self.field_mappings
			.insert(import_field.into(), model_field.into());
		self
	}

	/// Get field mappings
	pub fn field_mappings(&self) -> &HashMap<String, String> {
		&self.field_mappings
	}

	/// Map import field to model field
	pub fn map_field<'a>(&'a self, import_field: &'a str) -> &'a str {
		self.field_mappings
			.get(import_field)
			.map(|s| s.as_str())
			.unwrap_or(import_field)
	}

	/// Add field to skip
	pub fn skip_field(mut self, field: impl Into<String>) -> Self {
		self.skip_fields.push(field.into());
		self
	}

	/// Get skip fields
	pub fn skip_fields(&self) -> &[String] {
		&self.skip_fields
	}

	/// Set whether to skip duplicates
	pub fn skip_duplicates(mut self, skip: bool) -> Self {
		self.skip_duplicates = skip;
		self
	}

	/// Check if duplicates should be skipped
	pub fn should_skip_duplicates(&self) -> bool {
		self.skip_duplicates
	}

	/// Set whether to update existing records
	pub fn update_existing(mut self, update: bool) -> Self {
		self.update_existing = update;
		self
	}

	/// Check if existing records should be updated
	pub fn should_update_existing(&self) -> bool {
		self.update_existing
	}

	/// Set key field for duplicate detection
	pub fn with_key_field(mut self, field: impl Into<String>) -> Self {
		self.key_field = Some(field.into());
		self
	}

	/// Get key field
	pub fn key_field(&self) -> Option<&String> {
		self.key_field.as_ref()
	}

	/// Set maximum records to import
	pub fn with_max_records(mut self, max: usize) -> Self {
		self.max_records = Some(max);
		self
	}

	/// Get maximum records
	pub fn max_records(&self) -> Option<usize> {
		self.max_records
	}

	/// Set whether to skip header row
	pub fn with_skip_header(mut self, skip: bool) -> Self {
		self.skip_header = skip;
		self
	}

	/// Check if header should be skipped
	pub fn should_skip_header(&self) -> bool {
		self.skip_header
	}

	/// Set whether to validate before import
	pub fn with_validation(mut self, validate: bool) -> Self {
		self.validate_first = validate;
		self
	}

	/// Check if validation should be performed
	pub fn should_validate(&self) -> bool {
		self.validate_first
	}
}

/// Import result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
	/// Number of records imported
	pub imported_count: usize,
	/// Number of records updated
	pub updated_count: usize,
	/// Number of records skipped
	pub skipped_count: usize,
	/// Number of records failed
	pub failed_count: usize,
	/// Error messages
	pub errors: Vec<ImportError>,
}

impl ImportResult {
	/// Create a new import result
	pub fn new() -> Self {
		Self {
			imported_count: 0,
			updated_count: 0,
			skipped_count: 0,
			failed_count: 0,
			errors: Vec::new(),
		}
	}

	/// Get total processed count
	pub fn total_processed(&self) -> usize {
		self.imported_count + self.updated_count + self.skipped_count + self.failed_count
	}

	/// Check if import was successful (no failures)
	pub fn is_successful(&self) -> bool {
		self.failed_count == 0
	}

	/// Add imported record
	pub fn add_imported(&mut self) {
		self.imported_count += 1;
	}

	/// Add updated record
	pub fn add_updated(&mut self) {
		self.updated_count += 1;
	}

	/// Add skipped record
	pub fn add_skipped(&mut self) {
		self.skipped_count += 1;
	}

	/// Add failed record
	pub fn add_failed(&mut self, error: ImportError) {
		self.failed_count += 1;
		self.errors.push(error);
	}
}

impl Default for ImportResult {
	fn default() -> Self {
		Self::new()
	}
}

/// Import error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportError {
	/// Row number (1-indexed)
	pub row_number: usize,
	/// Error message
	pub message: String,
	/// Failed data (optional)
	pub data: Option<HashMap<String, String>>,
}

impl ImportError {
	/// Create a new import error
	pub fn new(row_number: usize, message: impl Into<String>) -> Self {
		Self {
			row_number,
			message: message.into(),
			data: None,
		}
	}

	/// Create import error with data
	pub fn with_data(
		row_number: usize,
		message: impl Into<String>,
		data: HashMap<String, String>,
	) -> Self {
		Self {
			row_number,
			message: message.into(),
			data: Some(data),
		}
	}
}

/// CSV importer
pub struct CsvImporter;

impl CsvImporter {
	/// Import data from CSV format
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::core::CsvImporter;
	///
	/// let csv_data = b"id,name\n1,Alice\n2,Bob";
	/// let result = CsvImporter::import(csv_data, true);
	///
	/// assert!(result.is_ok());
	/// ```
	pub fn import(data: &[u8], skip_header: bool) -> AdminResult<Vec<HashMap<String, String>>> {
		// Use csv crate for RFC 4180 compliant parsing
		let mut reader = ReaderBuilder::new()
			.has_headers(true) // Always expect headers
			.flexible(false) // Strict column count validation
			.trim(csv::Trim::All) // Trim whitespace
			.from_reader(Cursor::new(data));

		// Get headers
		let headers = reader
			.headers()
			.map_err(|e| AdminError::ValidationError(format!("Failed to read CSV headers: {}", e)))?
			.iter()
			.map(|h| h.to_string())
			.collect::<Vec<_>>();

		if headers.is_empty() {
			return Err(AdminError::ValidationError(
				"CSV header is empty".to_string(),
			));
		}

		// Parse records
		let mut records = Vec::new();
		let mut row_num = 1; // Start from 1 (after header)

		for result in reader.records() {
			row_num += 1;

			let record = result.map_err(|e| {
				AdminError::ValidationError(format!("Row {}: CSV parse error: {}", row_num, e))
			})?;

			// Validate column count
			if record.len() != headers.len() {
				return Err(AdminError::ValidationError(format!(
					"Row {}: Expected {} columns, got {}",
					row_num,
					headers.len(),
					record.len()
				)));
			}

			// Convert to HashMap
			let mut map = HashMap::new();
			for (header, value) in headers.iter().zip(record.iter()) {
				map.insert(header.clone(), value.to_string());
			}

			records.push(map);
		}

		// If skip_header is false, we need to include the header as a data row
		// (but csv crate already skipped it as headers)
		// This is unusual but matches the original behavior
		if !skip_header && !headers.is_empty() {
			// Insert header row at the beginning
			let mut header_record = HashMap::new();
			for header in &headers {
				header_record.insert(header.clone(), header.clone());
			}
			records.insert(0, header_record);
		}

		Ok(records)
	}
}

/// JSON importer
pub struct JsonImporter;

impl JsonImporter {
	/// Import data from JSON format
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::core::JsonImporter;
	///
	/// let json_data = br#"[{"id":"1","name":"Alice"}]"#;
	/// let result = JsonImporter::import(json_data);
	///
	/// assert!(result.is_ok());
	/// ```
	pub fn import(data: &[u8]) -> AdminResult<Vec<HashMap<String, String>>> {
		let value: serde_json::Value = serde_json::from_slice(data)
			.map_err(|e| AdminError::ValidationError(format!("Invalid JSON: {}", e)))?;

		let array = value
			.as_array()
			.ok_or_else(|| AdminError::ValidationError("JSON must be an array".to_string()))?;

		// Validate all items are objects before processing.
		// This ensures consistent error behavior regardless of array size.
		for (idx, item) in array.iter().enumerate() {
			if !item.is_object() {
				return Err(AdminError::ValidationError(format!(
					"Item {} is not an object",
					idx
				)));
			}
		}

		// Use parallel processing for large JSON arrays (1000+ items)
		let records: Vec<HashMap<String, String>> = if array.len() > 1000 {
			// Parallel processing with rayon (all items are validated as objects above)
			array
				.par_iter()
				.map(|item| {
					// Safe: validated above that all items are objects
					let obj = item.as_object().expect("validated as object above");

					let mut record = HashMap::new();
					for (key, value) in obj {
						let value_str = match value {
							serde_json::Value::String(s) => s.clone(),
							serde_json::Value::Number(n) => n.to_string(),
							serde_json::Value::Bool(b) => b.to_string(),
							serde_json::Value::Null => String::new(),
							_ => value.to_string(),
						};
						record.insert(key.clone(), value_str);
					}

					record
				})
				.collect()
		} else {
			// Sequential processing for small arrays (all items validated above)
			let mut records = Vec::new();

			for item in array.iter() {
				// Safe: validated above that all items are objects
				let obj = item.as_object().expect("validated as object above");

				let mut record = HashMap::new();
				for (key, value) in obj {
					let value_str = match value {
						serde_json::Value::String(s) => s.clone(),
						serde_json::Value::Number(n) => n.to_string(),
						serde_json::Value::Bool(b) => b.to_string(),
						serde_json::Value::Null => String::new(),
						_ => value.to_string(),
					};
					record.insert(key.clone(), value_str);
				}

				records.push(record);
			}

			records
		};

		Ok(records)
	}
}

/// TSV (Tab-Separated Values) importer
pub struct TsvImporter;

impl TsvImporter {
	/// Import data from TSV format
	pub fn import(data: &[u8], skip_header: bool) -> AdminResult<Vec<HashMap<String, String>>> {
		let content = String::from_utf8(data.to_vec())
			.map_err(|e| AdminError::ValidationError(format!("Invalid UTF-8: {}", e)))?;

		let lines: Vec<&str> = content.lines().collect();

		if lines.is_empty() {
			return Ok(Vec::new());
		}

		// Parse header
		let headers: Vec<String> = lines[0].split('\t').map(|s| s.to_string()).collect();

		if headers.is_empty() {
			return Err(AdminError::ValidationError(
				"TSV header is empty".to_string(),
			));
		}

		let start_row = if skip_header { 1 } else { 0 };
		let mut records = Vec::new();

		for (idx, line) in lines.iter().enumerate().skip(start_row) {
			if line.trim().is_empty() {
				continue;
			}

			let values: Vec<String> = line.split('\t').map(|s| s.to_string()).collect();

			if values.len() != headers.len() {
				return Err(AdminError::ValidationError(format!(
					"Row {}: Expected {} columns, got {}",
					idx + 1,
					headers.len(),
					values.len()
				)));
			}

			let mut record = HashMap::new();
			for (header, value) in headers.iter().zip(values.iter()) {
				record.insert(header.clone(), value.clone());
			}

			records.push(record);
		}

		Ok(records)
	}
}

/// Import builder for fluent API
///
/// # Examples
///
/// ```
/// use reinhardt_admin::core::{ImportBuilder, ImportFormat};
///
/// let csv_data = b"id,name\n1,Alice\n2,Bob";
///
/// let result = ImportBuilder::new("User", ImportFormat::CSV)
///     .data(csv_data.to_vec())
///     .skip_duplicates(true)
///     .parse();
///
/// assert!(result.is_ok());
/// ```
pub struct ImportBuilder {
	config: ImportConfig,
	data: Vec<u8>,
}

impl ImportBuilder {
	/// Create a new import builder
	pub fn new(model_name: impl Into<String>, format: ImportFormat) -> Self {
		Self {
			config: ImportConfig::new(model_name, format),
			data: Vec::new(),
		}
	}

	/// Set data
	pub fn data(mut self, data: Vec<u8>) -> Self {
		self.data = data;
		self
	}

	/// Add field mapping
	pub fn field_mapping(
		mut self,
		import_field: impl Into<String>,
		model_field: impl Into<String>,
	) -> Self {
		self.config = self.config.with_field_mapping(import_field, model_field);
		self
	}

	/// Skip duplicates
	pub fn skip_duplicates(mut self, skip: bool) -> Self {
		self.config = self.config.skip_duplicates(skip);
		self
	}

	/// Update existing
	pub fn update_existing(mut self, update: bool) -> Self {
		self.config = self.config.update_existing(update);
		self
	}

	/// Set key field
	pub fn key_field(mut self, field: impl Into<String>) -> Self {
		self.config = self.config.with_key_field(field);
		self
	}

	/// Set maximum records
	pub fn max_records(mut self, max: usize) -> Self {
		self.config = self.config.with_max_records(max);
		self
	}

	/// Parse data
	pub fn parse(self) -> AdminResult<Vec<HashMap<String, String>>> {
		let mut records = match self.config.format() {
			ImportFormat::CSV => CsvImporter::import(&self.data, self.config.should_skip_header())?,
			ImportFormat::JSON => JsonImporter::import(&self.data)?,
			ImportFormat::TSV => TsvImporter::import(&self.data, self.config.should_skip_header())?,
		};

		// Apply field mappings
		if !self.config.field_mappings().is_empty() {
			records = records
				.into_iter()
				.map(|mut record| {
					let mut mapped_record = HashMap::new();
					for (key, value) in record.drain() {
						let mapped_key = self.config.map_field(&key).to_string();
						mapped_record.insert(mapped_key, value);
					}
					mapped_record
				})
				.collect();
		}

		// Apply max records limit
		if let Some(max) = self.config.max_records() {
			records.truncate(max);
		}

		Ok(records)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_import_format_from_filename() {
		assert_eq!(
			ImportFormat::from_filename("data.csv"),
			Some(ImportFormat::CSV)
		);
		assert_eq!(
			ImportFormat::from_filename("data.json"),
			Some(ImportFormat::JSON)
		);
		assert_eq!(
			ImportFormat::from_filename("data.tsv"),
			Some(ImportFormat::TSV)
		);
		assert_eq!(ImportFormat::from_filename("data.txt"), None);
	}

	#[test]
	fn test_import_config_new() {
		let config = ImportConfig::new("User", ImportFormat::CSV);
		assert_eq!(config.model_name(), "User");
		assert_eq!(config.format(), ImportFormat::CSV);
		assert!(config.should_skip_header());
		assert!(config.should_validate());
	}

	#[test]
	fn test_import_config_field_mapping() {
		let config =
			ImportConfig::new("User", ImportFormat::CSV).with_field_mapping("username", "login");

		assert_eq!(config.map_field("username"), "login");
		assert_eq!(config.map_field("email"), "email");
	}

	#[test]
	fn test_csv_importer_basic() {
		let csv_data = b"id,name\n1,Alice\n2,Bob";
		let result = CsvImporter::import(csv_data, true);

		let records = result.unwrap();
		assert_eq!(records.len(), 2);
		assert_eq!(records[0].get("id"), Some(&"1".to_string()));
		assert_eq!(records[0].get("name"), Some(&"Alice".to_string()));
	}

	#[test]
	fn test_csv_importer_quoted() {
		let csv_data = b"id,name\n1,\"Smith, John\"\n2,\"Doe, Jane\"";
		let result = CsvImporter::import(csv_data, true);

		let records = result.unwrap();
		assert_eq!(records.len(), 2);
		assert_eq!(records[0].get("name"), Some(&"Smith, John".to_string()));
	}

	#[test]
	fn test_json_importer() {
		let json_data = br#"[{"id":"1","name":"Alice"},{"id":"2","name":"Bob"}]"#;
		let result = JsonImporter::import(json_data);

		let records = result.unwrap();
		assert_eq!(records.len(), 2);
		assert_eq!(records[0].get("id"), Some(&"1".to_string()));
		assert_eq!(records[0].get("name"), Some(&"Alice".to_string()));
	}

	#[test]
	fn test_tsv_importer() {
		let tsv_data = b"id\tname\n1\tAlice\n2\tBob";
		let result = TsvImporter::import(tsv_data, true);

		let records = result.unwrap();
		assert_eq!(records.len(), 2);
		assert_eq!(records[0].get("id"), Some(&"1".to_string()));
		assert_eq!(records[0].get("name"), Some(&"Alice".to_string()));
	}

	#[test]
	fn test_import_builder() {
		let csv_data = b"id,name\n1,Alice\n2,Bob";

		let result = ImportBuilder::new("User", ImportFormat::CSV)
			.data(csv_data.to_vec())
			.parse();

		let records = result.unwrap();
		assert_eq!(records.len(), 2);
	}

	#[test]
	fn test_import_builder_with_mapping() {
		let csv_data = b"id,username\n1,alice\n2,bob";

		let result = ImportBuilder::new("User", ImportFormat::CSV)
			.data(csv_data.to_vec())
			.field_mapping("username", "login")
			.parse();

		let records = result.unwrap();
		assert_eq!(records[0].get("login"), Some(&"alice".to_string()));
		assert_eq!(records[0].get("username"), None);
	}

	#[test]
	fn test_import_builder_max_records() {
		let csv_data = b"id,name\n1,Alice\n2,Bob\n3,Charlie";

		let result = ImportBuilder::new("User", ImportFormat::CSV)
			.data(csv_data.to_vec())
			.max_records(2)
			.parse();

		let records = result.unwrap();
		assert_eq!(records.len(), 2);
	}

	#[test]
	fn test_import_result() {
		let mut result = ImportResult::new();
		assert_eq!(result.total_processed(), 0);
		assert!(result.is_successful());

		result.add_imported();
		result.add_updated();
		assert_eq!(result.imported_count, 1);
		assert_eq!(result.updated_count, 1);
		assert_eq!(result.total_processed(), 2);

		result.add_failed(ImportError::new(1, "Test error".to_string()));
		assert!(!result.is_successful());
		assert_eq!(result.failed_count, 1);
	}

	// ==================== from_content_type tests ====================

	#[test]
	fn test_from_content_type_json() {
		assert_eq!(
			ImportFormat::from_content_type("application/json"),
			Some(ImportFormat::JSON)
		);
	}

	#[test]
	fn test_from_content_type_csv() {
		assert_eq!(
			ImportFormat::from_content_type("text/csv"),
			Some(ImportFormat::CSV)
		);
	}

	#[test]
	fn test_from_content_type_tsv() {
		assert_eq!(
			ImportFormat::from_content_type("text/tab-separated-values"),
			Some(ImportFormat::TSV)
		);
	}

	#[test]
	fn test_from_content_type_with_charset() {
		// Content-Type with charset parameter should still be parsed correctly
		assert_eq!(
			ImportFormat::from_content_type("application/json; charset=utf-8"),
			Some(ImportFormat::JSON)
		);
		assert_eq!(
			ImportFormat::from_content_type("text/csv; charset=utf-8"),
			Some(ImportFormat::CSV)
		);
		assert_eq!(
			ImportFormat::from_content_type("text/tab-separated-values; charset=utf-8"),
			Some(ImportFormat::TSV)
		);
	}

	#[test]
	fn test_from_content_type_unknown() {
		assert_eq!(ImportFormat::from_content_type("text/html"), None);
		assert_eq!(ImportFormat::from_content_type("application/xml"), None);
		assert_eq!(ImportFormat::from_content_type("image/png"), None);
	}

	#[test]
	fn test_from_content_type_empty() {
		assert_eq!(ImportFormat::from_content_type(""), None);
	}

	#[test]
	fn test_from_content_type_case_insensitive() {
		// Content-Type header values should be case-insensitive per RFC
		assert_eq!(
			ImportFormat::from_content_type("Application/JSON"),
			Some(ImportFormat::JSON)
		);
		assert_eq!(
			ImportFormat::from_content_type("TEXT/CSV"),
			Some(ImportFormat::CSV)
		);
		assert_eq!(
			ImportFormat::from_content_type("Text/Tab-Separated-Values"),
			Some(ImportFormat::TSV)
		);
	}

	#[test]
	fn test_from_content_type_text_json() {
		// text/json is an alternative MIME type for JSON
		assert_eq!(
			ImportFormat::from_content_type("text/json"),
			Some(ImportFormat::JSON)
		);
	}

	#[test]
	fn test_from_content_type_application_csv() {
		// application/csv is an alternative MIME type for CSV
		assert_eq!(
			ImportFormat::from_content_type("application/csv"),
			Some(ImportFormat::CSV)
		);
	}

	#[test]
	fn test_from_content_type_text_tsv() {
		// text/tsv is an alternative MIME type for TSV
		assert_eq!(
			ImportFormat::from_content_type("text/tsv"),
			Some(ImportFormat::TSV)
		);
	}

	#[test]
	fn test_from_content_type_with_extra_parameters() {
		// Content-Type with multiple parameters
		assert_eq!(
			ImportFormat::from_content_type("application/json; charset=utf-8; boundary=something"),
			Some(ImportFormat::JSON)
		);
	}

	#[test]
	fn test_from_content_type_whitespace() {
		// Content-Type with whitespace variations
		assert_eq!(
			ImportFormat::from_content_type("  application/json  "),
			Some(ImportFormat::JSON)
		);
		assert_eq!(
			ImportFormat::from_content_type("application/json ;charset=utf-8"),
			Some(ImportFormat::JSON)
		);
	}
}

//! ContentType serialization support
//!
//! This module provides utilities for serializing and deserializing content types,
//! useful for fixtures, data migration, and management commands.
//!
//! # Example
//!
//! ```rust
//! use reinhardt_db::contenttypes::serialization::{ContentTypeSerializer, SerializationFormat};
//! use reinhardt_db::contenttypes::{ContentType, ContentTypeRegistry};
//!
//! let registry = ContentTypeRegistry::new();
//! registry.register(ContentType::new("blog", "article"));
//! registry.register(ContentType::new("auth", "user"));
//!
//! let serializer = ContentTypeSerializer::new();
//!
//! // Export to JSON
//! let json = serializer.dump_to_json(&registry).unwrap();
//! println!("{}", json);
//!
//! // Using natural keys (app_label.model format)
//! let natural_keys = serializer.dump_with_natural_keys(&registry);
//! assert!(natural_keys.contains(&"blog.article".to_string()));
//! ```

use super::{ContentType, ContentTypeRegistry};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Serialization format options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SerializationFormat {
	/// JSON format (default)
	#[default]
	Json,
	/// Pretty-printed JSON
	JsonPretty,
}

/// Error type for serialization operations
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SerializationError {
	/// JSON serialization/deserialization error
	JsonError(String),
	/// Invalid data format
	InvalidFormat(String),
	/// Content type already exists during import
	DuplicateEntry { app_label: String, model: String },
}

impl std::fmt::Display for SerializationError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::JsonError(msg) => write!(f, "JSON error: {}", msg),
			Self::InvalidFormat(msg) => write!(f, "Invalid format: {}", msg),
			Self::DuplicateEntry { app_label, model } => {
				write!(f, "Duplicate entry: {}.{}", app_label, model)
			}
		}
	}
}

impl std::error::Error for SerializationError {}

/// Serializable representation of a ContentType
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SerializableContentType {
	/// The application label (e.g., "blog", "auth")
	pub app_label: String,
	/// The model name (e.g., "article", "user")
	pub model: String,
	/// Optional ID (may be None if using natural keys)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub id: Option<i64>,
}

impl SerializableContentType {
	/// Creates a new serializable content type
	#[must_use]
	pub fn new(app_label: impl Into<String>, model: impl Into<String>) -> Self {
		Self {
			app_label: app_label.into(),
			model: model.into(),
			id: None,
		}
	}

	/// Creates a serializable content type with ID
	#[must_use]
	pub fn with_id(app_label: impl Into<String>, model: impl Into<String>, id: i64) -> Self {
		Self {
			app_label: app_label.into(),
			model: model.into(),
			id: Some(id),
		}
	}

	/// Returns the natural key (app_label.model)
	#[must_use]
	pub fn natural_key(&self) -> String {
		format!("{}.{}", self.app_label, self.model)
	}

	/// Creates from a ContentType
	#[must_use]
	pub fn from_content_type(ct: &ContentType) -> Self {
		Self {
			app_label: ct.app_label.clone(),
			model: ct.model.clone(),
			id: ct.id,
		}
	}

	/// Converts to a ContentType
	#[must_use]
	pub fn to_content_type(&self) -> ContentType {
		ContentType::new(&self.app_label, &self.model)
	}
}

/// Export data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentTypeExport {
	/// Format version for future compatibility
	pub version: String,
	/// Export timestamp (Unix timestamp)
	pub timestamp: u64,
	/// List of content types
	pub content_types: Vec<SerializableContentType>,
	/// Optional metadata
	#[serde(skip_serializing_if = "Option::is_none")]
	pub metadata: Option<HashMap<String, String>>,
}

impl ContentTypeExport {
	/// Creates a new export with the current timestamp
	#[must_use]
	pub fn new(content_types: Vec<SerializableContentType>) -> Self {
		Self {
			version: "1.0".to_string(),
			timestamp: std::time::SystemTime::now()
				.duration_since(std::time::UNIX_EPOCH)
				.map(|d| d.as_secs())
				.unwrap_or(0),
			content_types,
			metadata: None,
		}
	}

	/// Sets optional metadata
	#[must_use]
	pub fn with_metadata(mut self, metadata: HashMap<String, String>) -> Self {
		self.metadata = Some(metadata);
		self
	}

	/// Returns the number of content types in the export
	#[must_use]
	pub fn len(&self) -> usize {
		self.content_types.len()
	}

	/// Returns true if there are no content types
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.content_types.is_empty()
	}
}

/// Import options
#[non_exhaustive]
#[derive(Debug, Clone, Default)]
pub struct ImportOptions {
	/// Whether to skip existing entries instead of returning an error
	pub skip_existing: bool,
	/// Whether to update existing entries with new data
	pub update_existing: bool,
	/// Optional filter for app_label (only import matching)
	pub filter_app_label: Option<String>,
}

impl ImportOptions {
	/// Creates default import options
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	/// Sets skip_existing option
	#[must_use]
	pub fn skip_existing(mut self, skip: bool) -> Self {
		self.skip_existing = skip;
		self
	}

	/// Sets update_existing option
	#[must_use]
	pub fn update_existing(mut self, update: bool) -> Self {
		self.update_existing = update;
		self
	}

	/// Sets filter_app_label option
	#[must_use]
	pub fn filter_app_label(mut self, app_label: impl Into<String>) -> Self {
		self.filter_app_label = Some(app_label.into());
		self
	}
}

/// Result of an import operation
#[derive(Debug, Clone, Default)]
pub struct ImportResult {
	/// Number of content types created
	pub created: usize,
	/// Number of content types updated
	pub updated: usize,
	/// Number of content types skipped
	pub skipped: usize,
	/// Errors encountered during import
	pub errors: Vec<String>,
}

impl ImportResult {
	/// Creates a new empty import result
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	/// Returns true if any content types were imported
	#[must_use]
	pub fn has_changes(&self) -> bool {
		self.created > 0 || self.updated > 0
	}

	/// Returns true if there were any errors
	#[must_use]
	pub fn has_errors(&self) -> bool {
		!self.errors.is_empty()
	}

	/// Returns the total number of entries processed
	#[must_use]
	pub fn total_processed(&self) -> usize {
		self.created + self.updated + self.skipped
	}
}

/// Handles serialization and deserialization of content types
#[derive(Debug, Clone, Default)]
pub struct ContentTypeSerializer {
	/// Serialization format
	format: SerializationFormat,
}

impl ContentTypeSerializer {
	/// Creates a new serializer with default format
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	/// Creates a new serializer with specified format
	#[must_use]
	pub fn with_format(format: SerializationFormat) -> Self {
		Self { format }
	}

	/// Exports content types to JSON string
	///
	/// # Errors
	///
	/// Returns an error if JSON serialization fails.
	pub fn dump_to_json(
		&self,
		registry: &ContentTypeRegistry,
	) -> Result<String, SerializationError> {
		let content_types: Vec<SerializableContentType> = registry
			.all()
			.into_iter()
			.map(|ct| SerializableContentType::from_content_type(&ct))
			.collect();

		let export = ContentTypeExport::new(content_types);

		match self.format {
			SerializationFormat::Json => serde_json::to_string(&export)
				.map_err(|e| SerializationError::JsonError(e.to_string())),
			SerializationFormat::JsonPretty => serde_json::to_string_pretty(&export)
				.map_err(|e| SerializationError::JsonError(e.to_string())),
		}
	}

	/// Exports content types with metadata
	///
	/// # Errors
	///
	/// Returns an error if JSON serialization fails.
	pub fn dump_to_json_with_metadata(
		&self,
		registry: &ContentTypeRegistry,
		metadata: HashMap<String, String>,
	) -> Result<String, SerializationError> {
		let content_types: Vec<SerializableContentType> = registry
			.all()
			.into_iter()
			.map(|ct| SerializableContentType::from_content_type(&ct))
			.collect();

		let export = ContentTypeExport::new(content_types).with_metadata(metadata);

		match self.format {
			SerializationFormat::Json => serde_json::to_string(&export)
				.map_err(|e| SerializationError::JsonError(e.to_string())),
			SerializationFormat::JsonPretty => serde_json::to_string_pretty(&export)
				.map_err(|e| SerializationError::JsonError(e.to_string())),
		}
	}

	/// Imports content types from JSON string
	///
	/// # Errors
	///
	/// Returns an error if JSON parsing fails or duplicate entries are found.
	pub fn load_from_json(
		&self,
		registry: &ContentTypeRegistry,
		json: &str,
	) -> Result<ImportResult, SerializationError> {
		self.load_from_json_with_options(registry, json, ImportOptions::default())
	}

	/// Imports content types from JSON string with options
	///
	/// # Errors
	///
	/// Returns an error if JSON parsing fails.
	pub fn load_from_json_with_options(
		&self,
		registry: &ContentTypeRegistry,
		json: &str,
		options: ImportOptions,
	) -> Result<ImportResult, SerializationError> {
		let export: ContentTypeExport =
			serde_json::from_str(json).map_err(|e| SerializationError::JsonError(e.to_string()))?;

		let mut result = ImportResult::new();

		for serializable in export.content_types {
			// Apply app_label filter if set
			if let Some(ref filter) = options.filter_app_label
				&& &serializable.app_label != filter
			{
				result.skipped += 1;
				continue;
			}

			let existing = registry.get(&serializable.app_label, &serializable.model);

			match existing {
				Some(_) => {
					if options.update_existing {
						// ContentType has only app_label and model fields, which form the natural key.
						// Since there are no additional mutable fields to update, existing records
						// are treated as already up-to-date and counted as updated.
						result.updated += 1;
					} else if options.skip_existing {
						result.skipped += 1;
					} else {
						return Err(SerializationError::DuplicateEntry {
							app_label: serializable.app_label,
							model: serializable.model,
						});
					}
				}
				None => {
					registry.register(serializable.to_content_type());
					result.created += 1;
				}
			}
		}

		Ok(result)
	}

	/// Returns content types as natural keys (app_label.model format)
	///
	/// This is useful for Django-compatible fixtures and human-readable exports.
	#[must_use]
	pub fn dump_with_natural_keys(&self, registry: &ContentTypeRegistry) -> Vec<String> {
		registry
			.all()
			.into_iter()
			.map(|ct| format!("{}.{}", ct.app_label, ct.model))
			.collect()
	}

	/// Exports only specific app's content types
	///
	/// # Errors
	///
	/// Returns an error if JSON serialization fails.
	pub fn dump_app_to_json(
		&self,
		registry: &ContentTypeRegistry,
		app_label: &str,
	) -> Result<String, SerializationError> {
		let content_types: Vec<SerializableContentType> = registry
			.all()
			.into_iter()
			.filter(|ct| ct.app_label == app_label)
			.map(|ct| SerializableContentType::from_content_type(&ct))
			.collect();

		let export = ContentTypeExport::new(content_types);

		match self.format {
			SerializationFormat::Json => serde_json::to_string(&export)
				.map_err(|e| SerializationError::JsonError(e.to_string())),
			SerializationFormat::JsonPretty => serde_json::to_string_pretty(&export)
				.map_err(|e| SerializationError::JsonError(e.to_string())),
		}
	}

	/// Parses a ContentTypeExport from JSON without importing
	///
	/// # Errors
	///
	/// Returns an error if JSON parsing fails.
	pub fn parse_json(&self, json: &str) -> Result<ContentTypeExport, SerializationError> {
		serde_json::from_str(json).map_err(|e| SerializationError::JsonError(e.to_string()))
	}

	/// Validates JSON structure without importing
	#[must_use]
	pub fn validate_json(&self, json: &str) -> bool {
		self.parse_json(json).is_ok()
	}
}

/// Convenience function to dump registry to JSON
///
/// # Errors
///
/// Returns an error if JSON serialization fails.
pub fn dump_to_json(registry: &ContentTypeRegistry) -> Result<String, SerializationError> {
	ContentTypeSerializer::new().dump_to_json(registry)
}

/// Convenience function to dump registry to pretty JSON
///
/// # Errors
///
/// Returns an error if JSON serialization fails.
pub fn dump_to_json_pretty(registry: &ContentTypeRegistry) -> Result<String, SerializationError> {
	ContentTypeSerializer::with_format(SerializationFormat::JsonPretty).dump_to_json(registry)
}

/// Convenience function to load content types from JSON
///
/// # Errors
///
/// Returns an error if JSON parsing fails or duplicate entries are found.
pub fn load_from_json(
	registry: &ContentTypeRegistry,
	json: &str,
) -> Result<ImportResult, SerializationError> {
	ContentTypeSerializer::new().load_from_json(registry, json)
}

/// Convenience function to get natural keys
#[must_use]
pub fn dump_with_natural_keys(registry: &ContentTypeRegistry) -> Vec<String> {
	ContentTypeSerializer::new().dump_with_natural_keys(registry)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_serializable_content_type_new() {
		let sct = SerializableContentType::new("blog", "article");
		assert_eq!(sct.app_label, "blog");
		assert_eq!(sct.model, "article");
		assert!(sct.id.is_none());
	}

	#[test]
	fn test_serializable_content_type_with_id() {
		let sct = SerializableContentType::with_id("blog", "article", 42);
		assert_eq!(sct.app_label, "blog");
		assert_eq!(sct.model, "article");
		assert_eq!(sct.id, Some(42));
	}

	#[test]
	fn test_serializable_content_type_natural_key() {
		let sct = SerializableContentType::new("blog", "article");
		assert_eq!(sct.natural_key(), "blog.article");
	}

	#[test]
	fn test_from_content_type() {
		// Unregistered ContentType has no ID
		let ct = ContentType::new("auth", "user");
		let sct = SerializableContentType::from_content_type(&ct);
		assert_eq!(sct.app_label, "auth");
		assert_eq!(sct.model, "user");
		assert!(sct.id.is_none()); // ID is None until registered
	}

	#[test]
	fn test_from_registered_content_type() {
		// Registered ContentType has an ID
		let registry = ContentTypeRegistry::new();
		let ct = registry.register(ContentType::new("auth", "user"));
		let sct = SerializableContentType::from_content_type(&ct);
		assert_eq!(sct.app_label, "auth");
		assert_eq!(sct.model, "user");
		assert!(sct.id.is_some()); // ID is assigned after registration
	}

	#[test]
	fn test_to_content_type() {
		let sct = SerializableContentType::new("blog", "article");
		let ct = sct.to_content_type();
		assert_eq!(ct.app_label, "blog");
		assert_eq!(ct.model, "article");
	}

	#[test]
	fn test_content_type_export_new() {
		let content_types = vec![
			SerializableContentType::new("blog", "article"),
			SerializableContentType::new("auth", "user"),
		];
		let export = ContentTypeExport::new(content_types);

		assert_eq!(export.version, "1.0");
		assert!(export.timestamp > 0);
		assert_eq!(export.len(), 2);
		assert!(!export.is_empty());
		assert!(export.metadata.is_none());
	}

	#[test]
	fn test_content_type_export_with_metadata() {
		let content_types = vec![SerializableContentType::new("blog", "article")];
		let mut metadata = HashMap::new();
		metadata.insert("source".to_string(), "test".to_string());

		let export = ContentTypeExport::new(content_types).with_metadata(metadata);

		assert!(export.metadata.is_some());
		assert_eq!(
			export.metadata.as_ref().unwrap().get("source"),
			Some(&"test".to_string())
		);
	}

	#[test]
	fn test_import_options_builder() {
		let options = ImportOptions::new()
			.skip_existing(true)
			.update_existing(false)
			.filter_app_label("blog");

		assert!(options.skip_existing);
		assert!(!options.update_existing);
		assert_eq!(options.filter_app_label, Some("blog".to_string()));
	}

	#[test]
	fn test_import_result() {
		let mut result = ImportResult::new();
		assert!(!result.has_changes());
		assert!(!result.has_errors());
		assert_eq!(result.total_processed(), 0);

		result.created = 2;
		result.updated = 1;
		result.skipped = 1;

		assert!(result.has_changes());
		assert_eq!(result.total_processed(), 4);
	}

	#[test]
	fn test_dump_to_json() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "article"));

		let serializer = ContentTypeSerializer::new();
		let json = serializer.dump_to_json(&registry).unwrap();

		assert!(json.contains("blog"));
		assert!(json.contains("article"));
		assert!(json.contains("version"));
	}

	#[test]
	fn test_dump_to_json_pretty() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "article"));

		let serializer = ContentTypeSerializer::with_format(SerializationFormat::JsonPretty);
		let json = serializer.dump_to_json(&registry).unwrap();

		// Pretty format should have newlines
		assert!(json.contains('\n'));
	}

	#[test]
	fn test_load_from_json() {
		let registry = ContentTypeRegistry::new();

		let json = r#"{
			"version": "1.0",
			"timestamp": 1234567890,
			"content_types": [
				{"app_label": "blog", "model": "article"},
				{"app_label": "auth", "model": "user"}
			]
		}"#;

		let serializer = ContentTypeSerializer::new();
		let result = serializer.load_from_json(&registry, json).unwrap();

		assert_eq!(result.created, 2);
		assert_eq!(result.skipped, 0);
		assert!(registry.get("blog", "article").is_some());
		assert!(registry.get("auth", "user").is_some());
	}

	#[test]
	fn test_load_from_json_skip_existing() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "article"));

		let json = r#"{
			"version": "1.0",
			"timestamp": 1234567890,
			"content_types": [
				{"app_label": "blog", "model": "article"},
				{"app_label": "auth", "model": "user"}
			]
		}"#;

		let serializer = ContentTypeSerializer::new();
		let result = serializer
			.load_from_json_with_options(&registry, json, ImportOptions::new().skip_existing(true))
			.unwrap();

		assert_eq!(result.created, 1);
		assert_eq!(result.skipped, 1);
	}

	#[test]
	fn test_load_from_json_duplicate_error() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "article"));

		let json = r#"{
			"version": "1.0",
			"timestamp": 1234567890,
			"content_types": [
				{"app_label": "blog", "model": "article"}
			]
		}"#;

		let serializer = ContentTypeSerializer::new();
		let result = serializer.load_from_json(&registry, json);

		assert!(result.is_err());
		assert!(matches!(
			result,
			Err(SerializationError::DuplicateEntry { .. })
		));
	}

	#[test]
	fn test_load_from_json_with_filter() {
		let registry = ContentTypeRegistry::new();

		let json = r#"{
			"version": "1.0",
			"timestamp": 1234567890,
			"content_types": [
				{"app_label": "blog", "model": "article"},
				{"app_label": "blog", "model": "comment"},
				{"app_label": "auth", "model": "user"}
			]
		}"#;

		let serializer = ContentTypeSerializer::new();
		let result = serializer
			.load_from_json_with_options(
				&registry,
				json,
				ImportOptions::new().filter_app_label("blog"),
			)
			.unwrap();

		assert_eq!(result.created, 2);
		assert_eq!(result.skipped, 1);
		assert!(registry.get("blog", "article").is_some());
		assert!(registry.get("auth", "user").is_none());
	}

	#[test]
	fn test_dump_with_natural_keys() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "article"));
		registry.register(ContentType::new("auth", "user"));

		let serializer = ContentTypeSerializer::new();
		let keys = serializer.dump_with_natural_keys(&registry);

		assert!(keys.contains(&"blog.article".to_string()));
		assert!(keys.contains(&"auth.user".to_string()));
	}

	#[test]
	fn test_dump_app_to_json() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "article"));
		registry.register(ContentType::new("blog", "comment"));
		registry.register(ContentType::new("auth", "user"));

		let serializer = ContentTypeSerializer::new();
		let json = serializer.dump_app_to_json(&registry, "blog").unwrap();

		assert!(json.contains("article"));
		assert!(json.contains("comment"));
		assert!(!json.contains("user"));
	}

	#[test]
	fn test_validate_json() {
		let serializer = ContentTypeSerializer::new();

		let valid_json = r#"{
			"version": "1.0",
			"timestamp": 1234567890,
			"content_types": []
		}"#;
		assert!(serializer.validate_json(valid_json));

		let invalid_json = r#"{ invalid }"#;
		assert!(!serializer.validate_json(invalid_json));
	}

	#[test]
	fn test_roundtrip() {
		let registry1 = ContentTypeRegistry::new();
		registry1.register(ContentType::new("blog", "article"));
		registry1.register(ContentType::new("auth", "user"));

		let serializer = ContentTypeSerializer::new();
		let json = serializer.dump_to_json(&registry1).unwrap();

		let registry2 = ContentTypeRegistry::new();
		let result = serializer.load_from_json(&registry2, &json).unwrap();

		assert_eq!(result.created, 2);
		assert!(registry2.get("blog", "article").is_some());
		assert!(registry2.get("auth", "user").is_some());
	}

	#[test]
	fn test_convenience_functions() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "article"));

		// dump_to_json
		let json = dump_to_json(&registry).unwrap();
		assert!(json.contains("blog"));

		// dump_to_json_pretty
		let pretty_json = dump_to_json_pretty(&registry).unwrap();
		assert!(pretty_json.contains('\n'));

		// dump_with_natural_keys
		let keys = dump_with_natural_keys(&registry);
		assert!(keys.contains(&"blog.article".to_string()));

		// load_from_json
		let registry2 = ContentTypeRegistry::new();
		let json_import = r#"{
			"version": "1.0",
			"timestamp": 1234567890,
			"content_types": [{"app_label": "auth", "model": "user"}]
		}"#;
		let result = load_from_json(&registry2, json_import).unwrap();
		assert_eq!(result.created, 1);
	}

	#[test]
	fn test_serialization_error_display() {
		let json_error = SerializationError::JsonError("parse error".to_string());
		assert!(json_error.to_string().contains("JSON error"));

		let format_error = SerializationError::InvalidFormat("bad format".to_string());
		assert!(format_error.to_string().contains("Invalid format"));

		let duplicate_error = SerializationError::DuplicateEntry {
			app_label: "blog".to_string(),
			model: "article".to_string(),
		};
		assert!(duplicate_error.to_string().contains("blog.article"));
	}

	#[test]
	fn test_dump_to_json_with_metadata() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "article"));

		let mut metadata = HashMap::new();
		metadata.insert("source".to_string(), "test_export".to_string());
		metadata.insert("version".to_string(), "1.0.0".to_string());

		let serializer = ContentTypeSerializer::with_format(SerializationFormat::JsonPretty);
		let json = serializer
			.dump_to_json_with_metadata(&registry, metadata)
			.unwrap();

		assert!(json.contains("test_export"));
		assert!(json.contains("1.0.0"));
	}
}

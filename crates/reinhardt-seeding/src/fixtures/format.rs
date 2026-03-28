//! Fixture format definitions.
//!
//! This module defines the data structures for Django-compatible fixture format.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

/// Django-compatible fixture record.
///
/// Each record represents a single model instance with its field values.
///
/// # Example
///
/// ```json
/// {
///   "model": "auth.User",
///   "pk": 1,
///   "fields": {
///     "username": "admin",
///     "email": "admin@example.com"
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FixtureRecord {
	/// Model identifier in format "app.Model" (e.g., "auth.User").
	pub model: String,

	/// Primary key value. Optional for auto-increment fields.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub pk: Option<Value>,

	/// Field values as a JSON object.
	pub fields: Value,
}

impl FixtureRecord {
	/// Creates a new fixture record.
	pub fn new(model: impl Into<String>, fields: Value) -> Self {
		Self {
			model: model.into(),
			pk: None,
			fields,
		}
	}

	/// Creates a new fixture record with a primary key.
	pub fn with_pk(model: impl Into<String>, pk: Value, fields: Value) -> Self {
		Self {
			model: model.into(),
			pk: Some(pk),
			fields,
		}
	}

	/// Returns the app label portion of the model identifier.
	///
	/// # Example
	///
	/// ```
	/// # use reinhardt_seeding::fixtures::FixtureRecord;
	/// # use serde_json::json;
	/// let record = FixtureRecord::new("auth.User", json!({}));
	/// assert_eq!(record.app_label(), Some("auth"));
	/// ```
	pub fn app_label(&self) -> Option<&str> {
		self.model.split('.').next()
	}

	/// Returns the model name portion of the model identifier.
	///
	/// # Example
	///
	/// ```
	/// # use reinhardt_seeding::fixtures::FixtureRecord;
	/// # use serde_json::json;
	/// let record = FixtureRecord::new("auth.User", json!({}));
	/// assert_eq!(record.model_name(), Some("User"));
	/// ```
	pub fn model_name(&self) -> Option<&str> {
		self.model.split('.').nth(1)
	}
}

/// Supported fixture file formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum FixtureFormat {
	/// JSON format (default).
	#[default]
	Json,

	/// YAML format (requires `yaml` feature).
	Yaml,
}

impl FixtureFormat {
	/// Determines the fixture format from a file extension.
	///
	/// # Arguments
	///
	/// * `ext` - File extension (e.g., "json", "yaml", "yml")
	///
	/// # Returns
	///
	/// Returns `Some(FixtureFormat)` if the extension is recognized, `None` otherwise.
	///
	/// # Example
	///
	/// ```
	/// # use reinhardt_seeding::fixtures::FixtureFormat;
	/// assert_eq!(FixtureFormat::from_extension("json"), Some(FixtureFormat::Json));
	/// assert_eq!(FixtureFormat::from_extension("yaml"), Some(FixtureFormat::Yaml));
	/// assert_eq!(FixtureFormat::from_extension("yml"), Some(FixtureFormat::Yaml));
	/// assert_eq!(FixtureFormat::from_extension("xml"), None);
	/// ```
	pub fn from_extension(ext: &str) -> Option<Self> {
		match ext.to_lowercase().as_str() {
			"json" => Some(Self::Json),
			"yaml" | "yml" => Some(Self::Yaml),
			_ => None,
		}
	}

	/// Determines the fixture format from a file path.
	///
	/// # Arguments
	///
	/// * `path` - Path to the fixture file
	///
	/// # Returns
	///
	/// Returns `Some(FixtureFormat)` if the file extension is recognized, `None` otherwise.
	pub fn from_path(path: &Path) -> Option<Self> {
		path.extension()
			.and_then(|ext| ext.to_str())
			.and_then(Self::from_extension)
	}

	/// Returns the default file extension for this format.
	pub fn extension(&self) -> &'static str {
		match self {
			Self::Json => "json",
			Self::Yaml => "yaml",
		}
	}

	/// Returns the MIME type for this format.
	pub fn mime_type(&self) -> &'static str {
		match self {
			Self::Json => "application/json",
			Self::Yaml => "application/x-yaml",
		}
	}
}

impl std::fmt::Display for FixtureFormat {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Json => write!(f, "JSON"),
			Self::Yaml => write!(f, "YAML"),
		}
	}
}

/// Parsed fixture data containing multiple records.
#[derive(Debug, Clone)]
pub struct FixtureData {
	/// Collection of fixture records.
	pub records: Vec<FixtureRecord>,

	/// Format the data was parsed from.
	pub format: FixtureFormat,

	/// Optional source file path.
	pub source: Option<String>,
}

impl FixtureData {
	/// Creates a new empty fixture data container.
	pub fn new(format: FixtureFormat) -> Self {
		Self {
			records: Vec::new(),
			format,
			source: None,
		}
	}

	/// Creates fixture data from a vector of records.
	pub fn from_records(records: Vec<FixtureRecord>, format: FixtureFormat) -> Self {
		Self {
			records,
			format,
			source: None,
		}
	}

	/// Sets the source file path.
	pub fn with_source(mut self, source: impl Into<String>) -> Self {
		self.source = Some(source.into());
		self
	}

	/// Returns the number of records.
	pub fn len(&self) -> usize {
		self.records.len()
	}

	/// Returns true if there are no records.
	pub fn is_empty(&self) -> bool {
		self.records.is_empty()
	}

	/// Adds a record to the collection.
	pub fn push(&mut self, record: FixtureRecord) {
		self.records.push(record);
	}

	/// Returns an iterator over the records.
	pub fn iter(&self) -> impl Iterator<Item = &FixtureRecord> {
		self.records.iter()
	}

	/// Groups records by model identifier.
	pub fn group_by_model(&self) -> std::collections::HashMap<&str, Vec<&FixtureRecord>> {
		let mut groups = std::collections::HashMap::new();
		for record in &self.records {
			groups
				.entry(record.model.as_str())
				.or_insert_with(Vec::new)
				.push(record);
		}
		groups
	}

	/// Filters records by app label.
	pub fn filter_by_app(&self, app_labels: &[&str]) -> Vec<&FixtureRecord> {
		self.records
			.iter()
			.filter(|record| {
				record
					.app_label()
					.map(|app| app_labels.contains(&app))
					.unwrap_or(false)
			})
			.collect()
	}
}

impl IntoIterator for FixtureData {
	type Item = FixtureRecord;
	type IntoIter = std::vec::IntoIter<FixtureRecord>;

	fn into_iter(self) -> Self::IntoIter {
		self.records.into_iter()
	}
}

impl<'a> IntoIterator for &'a FixtureData {
	type Item = &'a FixtureRecord;
	type IntoIter = std::slice::Iter<'a, FixtureRecord>;

	fn into_iter(self) -> Self::IntoIter {
		self.records.iter()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use serde_json::json;

	#[rstest]
	fn test_fixture_record_new() {
		let record = FixtureRecord::new("auth.User", json!({"username": "test"}));
		assert_eq!(record.model, "auth.User");
		assert!(record.pk.is_none());
		assert_eq!(record.fields, json!({"username": "test"}));
	}

	#[rstest]
	fn test_fixture_record_with_pk() {
		let record = FixtureRecord::with_pk("auth.User", json!(1), json!({"username": "test"}));
		assert_eq!(record.model, "auth.User");
		assert_eq!(record.pk, Some(json!(1)));
	}

	#[rstest]
	fn test_fixture_record_app_label() {
		let record = FixtureRecord::new("auth.User", json!({}));
		assert_eq!(record.app_label(), Some("auth"));
	}

	#[rstest]
	fn test_fixture_record_model_name() {
		let record = FixtureRecord::new("auth.User", json!({}));
		assert_eq!(record.model_name(), Some("User"));
	}

	#[rstest]
	fn test_fixture_format_from_extension() {
		assert_eq!(
			FixtureFormat::from_extension("json"),
			Some(FixtureFormat::Json)
		);
		assert_eq!(
			FixtureFormat::from_extension("JSON"),
			Some(FixtureFormat::Json)
		);
		assert_eq!(
			FixtureFormat::from_extension("yaml"),
			Some(FixtureFormat::Yaml)
		);
		assert_eq!(
			FixtureFormat::from_extension("yml"),
			Some(FixtureFormat::Yaml)
		);
		assert_eq!(FixtureFormat::from_extension("xml"), None);
	}

	#[rstest]
	fn test_fixture_format_from_path() {
		use std::path::PathBuf;
		assert_eq!(
			FixtureFormat::from_path(&PathBuf::from("fixtures.json")),
			Some(FixtureFormat::Json)
		);
		assert_eq!(
			FixtureFormat::from_path(&PathBuf::from("fixtures.yaml")),
			Some(FixtureFormat::Yaml)
		);
		assert_eq!(
			FixtureFormat::from_path(&PathBuf::from("no_extension")),
			None
		);
	}

	#[rstest]
	fn test_fixture_format_extension() {
		assert_eq!(FixtureFormat::Json.extension(), "json");
		assert_eq!(FixtureFormat::Yaml.extension(), "yaml");
	}

	#[rstest]
	fn test_fixture_data_operations() {
		let mut data = FixtureData::new(FixtureFormat::Json);
		assert!(data.is_empty());
		assert_eq!(data.len(), 0);

		data.push(FixtureRecord::new("auth.User", json!({"id": 1})));
		data.push(FixtureRecord::new("auth.User", json!({"id": 2})));
		data.push(FixtureRecord::new("blog.Post", json!({"id": 1})));

		assert!(!data.is_empty());
		assert_eq!(data.len(), 3);

		let groups = data.group_by_model();
		assert_eq!(groups.len(), 2);
		assert_eq!(groups["auth.User"].len(), 2);
		assert_eq!(groups["blog.Post"].len(), 1);
	}

	#[rstest]
	fn test_fixture_data_filter_by_app() {
		let data = FixtureData::from_records(
			vec![
				FixtureRecord::new("auth.User", json!({})),
				FixtureRecord::new("blog.Post", json!({})),
				FixtureRecord::new("auth.Group", json!({})),
			],
			FixtureFormat::Json,
		);

		let auth_records = data.filter_by_app(&["auth"]);
		assert_eq!(auth_records.len(), 2);
	}

	#[rstest]
	fn test_fixture_record_serialization() {
		let record = FixtureRecord::with_pk("auth.User", json!(1), json!({"username": "admin"}));
		let json = serde_json::to_string(&record).unwrap();
		let deserialized: FixtureRecord = serde_json::from_str(&json).unwrap();
		assert_eq!(record, deserialized);
	}
}

//! Fixture parsing functionality.
//!
//! This module handles parsing of fixture files in JSON and YAML formats.

use std::path::Path;

use super::{FixtureData, FixtureFormat, FixtureRecord};
use crate::error::{SeedingError, SeedingResult};

/// Parser for fixture files.
///
/// Supports both JSON and YAML formats (YAML requires the `yaml` feature).
#[derive(Debug, Default)]
pub struct FixtureParser;

impl FixtureParser {
	/// Creates a new fixture parser.
	pub fn new() -> Self {
		Self
	}

	/// Parses a fixture file from the given path.
	///
	/// The format is automatically detected from the file extension.
	///
	/// # Arguments
	///
	/// * `path` - Path to the fixture file
	///
	/// # Returns
	///
	/// Returns parsed fixture data on success.
	///
	/// # Errors
	///
	/// Returns an error if:
	/// - The file cannot be read
	/// - The file extension is not recognized
	/// - The file content is invalid
	pub fn parse_file(&self, path: &Path) -> SeedingResult<FixtureData> {
		let format = FixtureFormat::from_path(path).ok_or_else(|| {
			SeedingError::UnsupportedExtension(
				path.extension()
					.and_then(|e| e.to_str())
					.unwrap_or("(none)")
					.to_string(),
			)
		})?;

		let content = std::fs::read_to_string(path).map_err(|e| {
			if e.kind() == std::io::ErrorKind::NotFound {
				SeedingError::FileNotFound(path.display().to_string())
			} else {
				SeedingError::IoError(e)
			}
		})?;

		let mut data = self.parse_string(&content, format)?;
		data.source = Some(path.display().to_string());
		Ok(data)
	}

	/// Parses fixture data from a string.
	///
	/// # Arguments
	///
	/// * `content` - String content to parse
	/// * `format` - Format of the content
	///
	/// # Returns
	///
	/// Returns parsed fixture data on success.
	pub fn parse_string(&self, content: &str, format: FixtureFormat) -> SeedingResult<FixtureData> {
		let records = match format {
			FixtureFormat::Json => self.parse_json(content)?,
			FixtureFormat::Yaml => self.parse_yaml(content)?,
		};

		Ok(FixtureData::from_records(records, format))
	}

	/// Parses JSON fixture content.
	fn parse_json(&self, content: &str) -> SeedingResult<Vec<FixtureRecord>> {
		// Handle both single object and array formats
		let value: serde_json::Value = serde_json::from_str(content)?;

		match value {
			serde_json::Value::Array(arr) => {
				let mut records = Vec::with_capacity(arr.len());
				for (idx, item) in arr.into_iter().enumerate() {
					let record: FixtureRecord = serde_json::from_value(item).map_err(|e| {
						SeedingError::ParseError(format!("Invalid record at index {}: {}", idx, e))
					})?;
					self.validate_record(&record)?;
					records.push(record);
				}
				Ok(records)
			}
			serde_json::Value::Object(_) => {
				// Single object format
				let record: FixtureRecord = serde_json::from_value(value)?;
				self.validate_record(&record)?;
				Ok(vec![record])
			}
			_ => Err(SeedingError::ParseError(
				"Expected array or object".to_string(),
			)),
		}
	}

	/// Parses YAML fixture content.
	#[cfg(feature = "yaml")]
	fn parse_yaml(&self, content: &str) -> SeedingResult<Vec<FixtureRecord>> {
		let value: serde_yaml::Value = serde_yaml::from_str(content)?;

		match value {
			serde_yaml::Value::Sequence(seq) => {
				let mut records = Vec::with_capacity(seq.len());
				for (idx, item) in seq.into_iter().enumerate() {
					let record: FixtureRecord = serde_yaml::from_value(item).map_err(|e| {
						SeedingError::ParseError(format!("Invalid record at index {}: {}", idx, e))
					})?;
					self.validate_record(&record)?;
					records.push(record);
				}
				Ok(records)
			}
			serde_yaml::Value::Mapping(_) => {
				// Single object format
				let record: FixtureRecord = serde_yaml::from_value(value)?;
				self.validate_record(&record)?;
				Ok(vec![record])
			}
			_ => Err(SeedingError::ParseError(
				"Expected sequence or mapping".to_string(),
			)),
		}
	}

	/// Stub for YAML parsing when the feature is not enabled.
	#[cfg(not(feature = "yaml"))]
	fn parse_yaml(&self, _content: &str) -> SeedingResult<Vec<FixtureRecord>> {
		Err(SeedingError::UnsupportedExtension(
			"YAML support requires the 'yaml' feature".to_string(),
		))
	}

	/// Validates a fixture record.
	fn validate_record(&self, record: &FixtureRecord) -> SeedingResult<()> {
		// Model identifier must contain a dot separator
		if !record.model.contains('.') {
			return Err(SeedingError::ValidationError {
				field: "model".to_string(),
				message: format!(
					"Model identifier '{}' must be in 'app.Model' format",
					record.model
				),
			});
		}

		// Fields must be an object
		if !record.fields.is_object() {
			return Err(SeedingError::ValidationError {
				field: "fields".to_string(),
				message: "Fields must be a JSON object".to_string(),
			});
		}

		Ok(())
	}

	/// Parses multiple fixture files.
	///
	/// # Arguments
	///
	/// * `paths` - Paths to fixture files
	///
	/// # Returns
	///
	/// Returns combined fixture data from all files.
	pub fn parse_files(&self, paths: &[&Path]) -> SeedingResult<FixtureData> {
		let mut all_records = Vec::new();
		let format = paths
			.first()
			.and_then(|p| FixtureFormat::from_path(p))
			.unwrap_or_default();

		for path in paths {
			let data = self.parse_file(path)?;
			all_records.extend(data.records);
		}

		Ok(FixtureData::from_records(all_records, format))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use std::io::Write;
	use tempfile::NamedTempFile;

	#[rstest]
	fn test_parse_json_array() {
		let parser = FixtureParser::new();
		let content = r#"[
            {
                "model": "auth.User",
                "pk": 1,
                "fields": {"username": "admin"}
            },
            {
                "model": "auth.User",
                "pk": 2,
                "fields": {"username": "user"}
            }
        ]"#;

		let data = parser.parse_string(content, FixtureFormat::Json).unwrap();
		assert_eq!(data.len(), 2);
		assert_eq!(data.records[0].model, "auth.User");
		assert_eq!(data.records[0].pk, Some(serde_json::json!(1)));
	}

	#[rstest]
	fn test_parse_json_single_object() {
		let parser = FixtureParser::new();
		let content = r#"{
            "model": "auth.User",
            "pk": 1,
            "fields": {"username": "admin"}
        }"#;

		let data = parser.parse_string(content, FixtureFormat::Json).unwrap();
		assert_eq!(data.len(), 1);
	}

	#[rstest]
	fn test_parse_json_without_pk() {
		let parser = FixtureParser::new();
		let content = r#"[
            {
                "model": "auth.User",
                "fields": {"username": "admin"}
            }
        ]"#;

		let data = parser.parse_string(content, FixtureFormat::Json).unwrap();
		assert_eq!(data.len(), 1);
		assert!(data.records[0].pk.is_none());
	}

	#[rstest]
	fn test_parse_invalid_model_format() {
		let parser = FixtureParser::new();
		let content = r#"[
            {
                "model": "User",
                "fields": {"username": "admin"}
            }
        ]"#;

		let result = parser.parse_string(content, FixtureFormat::Json);
		assert!(result.is_err());
		if let Err(SeedingError::ValidationError { field, .. }) = result {
			assert_eq!(field, "model");
		} else {
			panic!("Expected ValidationError");
		}
	}

	#[rstest]
	fn test_parse_invalid_fields_type() {
		let parser = FixtureParser::new();
		let content = r#"[
            {
                "model": "auth.User",
                "fields": "not an object"
            }
        ]"#;

		let result = parser.parse_string(content, FixtureFormat::Json);
		assert!(result.is_err());
	}

	#[rstest]
	fn test_parse_file() {
		let parser = FixtureParser::new();
		let mut file = NamedTempFile::with_suffix(".json").unwrap();
		writeln!(
			file,
			r#"[{{"model": "auth.User", "fields": {{"username": "test"}}}}]"#
		)
		.unwrap();

		let data = parser.parse_file(file.path()).unwrap();
		assert_eq!(data.len(), 1);
		assert!(data.source.is_some());
	}

	#[rstest]
	fn test_parse_file_not_found() {
		let parser = FixtureParser::new();
		let result = parser.parse_file(Path::new("/nonexistent/file.json"));
		assert!(matches!(result, Err(SeedingError::FileNotFound(_))));
	}

	#[rstest]
	fn test_parse_unsupported_extension() {
		let parser = FixtureParser::new();
		let result = parser.parse_file(Path::new("file.xml"));
		assert!(matches!(result, Err(SeedingError::UnsupportedExtension(_))));
	}

	#[cfg(feature = "yaml")]
	#[rstest]
	fn test_parse_yaml() {
		let parser = FixtureParser::new();
		let content = r#"
- model: auth.User
  pk: 1
  fields:
    username: admin
- model: auth.User
  pk: 2
  fields:
    username: user
"#;

		let data = parser.parse_string(content, FixtureFormat::Yaml).unwrap();
		assert_eq!(data.len(), 2);
	}

	#[rstest]
	fn test_parse_multiple_files() {
		let parser = FixtureParser::new();

		let mut file1 = NamedTempFile::with_suffix(".json").unwrap();
		writeln!(
			file1,
			r#"[{{"model": "auth.User", "fields": {{"id": 1}}}}]"#
		)
		.unwrap();

		let mut file2 = NamedTempFile::with_suffix(".json").unwrap();
		writeln!(
			file2,
			r#"[{{"model": "auth.User", "fields": {{"id": 2}}}}]"#
		)
		.unwrap();

		let data = parser.parse_files(&[file1.path(), file2.path()]).unwrap();
		assert_eq!(data.len(), 2);
	}
}

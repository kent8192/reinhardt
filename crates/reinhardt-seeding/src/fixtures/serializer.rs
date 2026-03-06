//! Model serialization for fixture output.
//!
//! This module handles converting model data to fixture format.

use serde::Serialize;

use super::{FixtureFormat, FixtureRecord};
use crate::error::{SeedingError, SeedingResult};

/// Trait for serializing models to fixture format.
///
/// Implement this trait for models that should support fixture dumping.
pub trait ModelSerializer: Send + Sync {
	/// Returns the model identifier (e.g., "auth.User").
	fn model_id(&self) -> &str;

	/// Returns the app label for this model.
	fn app_label(&self) -> &str {
		self.model_id().split('.').next().unwrap_or("")
	}

	/// Returns the model name (without app label).
	fn model_name(&self) -> &str {
		self.model_id().split('.').nth(1).unwrap_or("")
	}
}

/// Fixture serializer for exporting model data.
#[derive(Debug, Clone)]
pub struct FixtureSerializer {
	/// Output format.
	format: FixtureFormat,

	/// Indentation level for pretty printing.
	indent: usize,

	/// Whether to use natural keys instead of primary keys.
	use_natural_keys: bool,
}

impl FixtureSerializer {
	/// Creates a new fixture serializer with default settings.
	pub fn new() -> Self {
		Self {
			format: FixtureFormat::Json,
			indent: 2,
			use_natural_keys: false,
		}
	}

	/// Sets the output format.
	pub fn with_format(mut self, format: FixtureFormat) -> Self {
		self.format = format;
		self
	}

	/// Sets the indentation level.
	pub fn with_indent(mut self, indent: usize) -> Self {
		self.indent = indent;
		self
	}

	/// Sets whether to use natural keys.
	pub fn with_natural_keys(mut self, use_natural_keys: bool) -> Self {
		self.use_natural_keys = use_natural_keys;
		self
	}

	/// Serializes fixture records to a string.
	///
	/// # Arguments
	///
	/// * `records` - Fixture records to serialize
	///
	/// # Returns
	///
	/// Returns the serialized string.
	pub fn serialize(&self, records: &[FixtureRecord]) -> SeedingResult<String> {
		match self.format {
			FixtureFormat::Json => self.serialize_json(records),
			FixtureFormat::Yaml => self.serialize_yaml(records),
		}
	}

	/// Serializes a single model instance to a fixture record.
	///
	/// # Type Parameters
	///
	/// * `T` - Model type that implements `Serialize`
	///
	/// # Arguments
	///
	/// * `model_id` - Model identifier (e.g., "auth.User")
	/// * `pk` - Primary key value
	/// * `instance` - Model instance to serialize
	pub fn serialize_model<T: Serialize>(
		&self,
		model_id: &str,
		pk: Option<serde_json::Value>,
		instance: &T,
	) -> SeedingResult<FixtureRecord> {
		let fields = serde_json::to_value(instance)?;
		Ok(FixtureRecord {
			model: model_id.to_string(),
			pk,
			fields,
		})
	}

	/// Serializes multiple model instances to fixture records.
	///
	/// # Type Parameters
	///
	/// * `T` - Model type that implements `Serialize`
	/// * `F` - Function to extract primary key from model
	///
	/// # Arguments
	///
	/// * `model_id` - Model identifier
	/// * `instances` - Model instances to serialize
	/// * `pk_fn` - Function to extract primary key from each instance
	pub fn serialize_models<T: Serialize, F>(
		&self,
		model_id: &str,
		instances: &[T],
		mut pk_fn: F,
	) -> SeedingResult<Vec<FixtureRecord>>
	where
		F: FnMut(&T) -> Option<serde_json::Value>,
	{
		instances
			.iter()
			.map(|instance| {
				let pk = pk_fn(instance);
				self.serialize_model(model_id, pk, instance)
			})
			.collect()
	}

	/// Serializes to JSON format.
	fn serialize_json(&self, records: &[FixtureRecord]) -> SeedingResult<String> {
		if self.indent > 0 {
			serde_json::to_string_pretty(records)
				.map_err(|e| SeedingError::SerializationError(e.to_string()))
		} else {
			serde_json::to_string(records)
				.map_err(|e| SeedingError::SerializationError(e.to_string()))
		}
	}

	/// Serializes to YAML format.
	#[cfg(feature = "yaml")]
	fn serialize_yaml(&self, records: &[FixtureRecord]) -> SeedingResult<String> {
		serde_yaml::to_string(records).map_err(|e| SeedingError::SerializationError(e.to_string()))
	}

	/// Stub for YAML serialization when feature is not enabled.
	#[cfg(not(feature = "yaml"))]
	fn serialize_yaml(&self, _records: &[FixtureRecord]) -> SeedingResult<String> {
		Err(SeedingError::UnsupportedExtension(
			"YAML support requires the 'yaml' feature".to_string(),
		))
	}

	/// Writes serialized fixtures to a file.
	///
	/// # Arguments
	///
	/// * `records` - Fixture records to write
	/// * `path` - Output file path
	pub fn write_to_file(
		&self,
		records: &[FixtureRecord],
		path: &std::path::Path,
	) -> SeedingResult<()> {
		let content = self.serialize(records)?;
		std::fs::write(path, content)?;
		Ok(())
	}

	/// Returns the configured output format.
	pub fn format(&self) -> FixtureFormat {
		self.format
	}

	/// Returns the configured indentation level.
	pub fn indent(&self) -> usize {
		self.indent
	}
}

impl Default for FixtureSerializer {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use serde::Serialize;
	use serde_json::json;
	use tempfile::tempdir;

	#[derive(Serialize)]
	struct TestUser {
		username: String,
		email: String,
	}

	#[rstest]
	fn test_serialize_json_pretty() {
		let serializer = FixtureSerializer::new().with_format(FixtureFormat::Json);
		let records = vec![FixtureRecord::with_pk(
			"auth.User",
			json!(1),
			json!({"username": "admin"}),
		)];

		let output = serializer.serialize(&records).unwrap();
		assert!(output.contains("\"model\": \"auth.User\""));
		assert!(output.contains('\n')); // Pretty printed
	}

	#[rstest]
	fn test_serialize_json_compact() {
		let serializer = FixtureSerializer::new()
			.with_format(FixtureFormat::Json)
			.with_indent(0);
		let records = vec![FixtureRecord::new(
			"auth.User",
			json!({"username": "admin"}),
		)];

		let output = serializer.serialize(&records).unwrap();
		assert!(!output.contains("\n  ")); // Not pretty printed
	}

	#[rstest]
	fn test_serialize_model() {
		let serializer = FixtureSerializer::new();
		let user = TestUser {
			username: "admin".to_string(),
			email: "admin@example.com".to_string(),
		};

		let record = serializer
			.serialize_model("auth.User", Some(json!(1)), &user)
			.unwrap();

		assert_eq!(record.model, "auth.User");
		assert_eq!(record.pk, Some(json!(1)));
		assert_eq!(record.fields["username"], "admin");
		assert_eq!(record.fields["email"], "admin@example.com");
	}

	#[rstest]
	fn test_serialize_models() {
		let serializer = FixtureSerializer::new();
		let users = vec![
			TestUser {
				username: "alice".to_string(),
				email: "alice@example.com".to_string(),
			},
			TestUser {
				username: "bob".to_string(),
				email: "bob@example.com".to_string(),
			},
		];

		let mut pk = 0;
		let records = serializer
			.serialize_models("auth.User", &users, |_| {
				pk += 1;
				Some(json!(pk))
			})
			.unwrap();

		assert_eq!(records.len(), 2);
		assert_eq!(records[0].pk, Some(json!(1)));
		assert_eq!(records[1].pk, Some(json!(2)));
	}

	#[rstest]
	fn test_write_to_file() {
		let serializer = FixtureSerializer::new();
		let records = vec![FixtureRecord::new("auth.User", json!({"username": "test"}))];

		let dir = tempdir().unwrap();
		let path = dir.path().join("fixtures.json");

		serializer.write_to_file(&records, &path).unwrap();

		let content = std::fs::read_to_string(&path).unwrap();
		assert!(content.contains("auth.User"));
	}

	#[cfg(feature = "yaml")]
	#[rstest]
	fn test_serialize_yaml() {
		let serializer = FixtureSerializer::new().with_format(FixtureFormat::Yaml);
		let records = vec![FixtureRecord::with_pk(
			"auth.User",
			json!(1),
			json!({"username": "admin"}),
		)];

		let output = serializer.serialize(&records).unwrap();
		assert!(output.contains("model: auth.User"));
	}

	#[rstest]
	fn test_model_serializer_trait() {
		struct TestSerializer;

		impl ModelSerializer for TestSerializer {
			fn model_id(&self) -> &str {
				"test.Model"
			}
		}

		let serializer = TestSerializer;
		assert_eq!(serializer.app_label(), "test");
		assert_eq!(serializer.model_name(), "Model");
	}
}

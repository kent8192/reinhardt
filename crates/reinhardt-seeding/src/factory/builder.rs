//! Factory builder for fluent factory configuration.
//!
//! This module provides a builder pattern for configuring factory behavior.

use std::collections::HashMap;

use super::faker::FakerType;
use super::sequence::Sequence;

/// Value generator for factory fields.
#[derive(Debug, Clone)]
pub enum FieldGenerator {
	/// Static value.
	Static(serde_json::Value),

	/// Generated using faker.
	Faker(FakerType),

	/// Generated using a sequence with format.
	Sequence {
		/// Sequence name identifier.
		name: String,
		/// Format string with `{n}` placeholder.
		format: String,
	},

	/// Custom generator function.
	Custom(String),
}

impl FieldGenerator {
	/// Generates a value using this generator.
	pub fn generate(&self) -> serde_json::Value {
		match self {
			Self::Static(value) => value.clone(),
			Self::Faker(faker) => serde_json::Value::String(faker.generate()),
			Self::Sequence { name, format } => {
				let seq = Sequence::new(name);
				serde_json::Value::String(seq.next_formatted(format))
			}
			Self::Custom(_) => {
				// Custom generators would need to be resolved through a registry
				serde_json::Value::Null
			}
		}
	}
}

/// Builder for configuring factory field generation.
///
/// This builder allows fluent configuration of how each field should
/// be generated.
///
/// # Example
///
/// ```ignore
/// let builder = FactoryBuilder::<User>::new("auth.User")
///     .field("username", FieldGenerator::Faker(FakerType::Username))
///     .field("email", FieldGenerator::Faker(FakerType::Email))
///     .field("code", FieldGenerator::Sequence {
///         name: "user_code".to_string(),
///         format: "USR-{n}".to_string(),
///     })
///     .field("is_active", FieldGenerator::Static(json!(true)));
/// ```
#[derive(Debug, Clone)]
pub struct FactoryBuilder<M> {
	/// Model identifier.
	model_id: String,

	/// Field generators.
	fields: HashMap<String, FieldGenerator>,

	/// Phantom marker for model type.
	_marker: std::marker::PhantomData<M>,
}

impl<M> FactoryBuilder<M> {
	/// Creates a new factory builder.
	///
	/// # Arguments
	///
	/// * `model_id` - Model identifier (e.g., "auth.User")
	pub fn new(model_id: impl Into<String>) -> Self {
		Self {
			model_id: model_id.into(),
			fields: HashMap::new(),
			_marker: std::marker::PhantomData,
		}
	}

	/// Adds a field generator.
	///
	/// # Arguments
	///
	/// * `name` - Field name
	/// * `generator` - Value generator for this field
	pub fn field(mut self, name: impl Into<String>, generator: FieldGenerator) -> Self {
		self.fields.insert(name.into(), generator);
		self
	}

	/// Adds a static value field.
	///
	/// # Arguments
	///
	/// * `name` - Field name
	/// * `value` - Static value
	pub fn static_field(self, name: impl Into<String>, value: serde_json::Value) -> Self {
		self.field(name, FieldGenerator::Static(value))
	}

	/// Adds a faker field.
	///
	/// # Arguments
	///
	/// * `name` - Field name
	/// * `faker` - Faker type
	pub fn faker_field(self, name: impl Into<String>, faker: FakerType) -> Self {
		self.field(name, FieldGenerator::Faker(faker))
	}

	/// Adds a sequence field.
	///
	/// # Arguments
	///
	/// * `name` - Field name
	/// * `seq_name` - Sequence name
	/// * `format` - Format string with `{n}` placeholder
	pub fn sequence_field(
		self,
		name: impl Into<String>,
		seq_name: impl Into<String>,
		format: impl Into<String>,
	) -> Self {
		self.field(
			name,
			FieldGenerator::Sequence {
				name: seq_name.into(),
				format: format.into(),
			},
		)
	}

	/// Returns the model identifier.
	pub fn model_id(&self) -> &str {
		&self.model_id
	}

	/// Returns the configured field generators.
	pub fn fields(&self) -> &HashMap<String, FieldGenerator> {
		&self.fields
	}

	/// Generates field values as a JSON object.
	pub fn generate_fields(&self) -> serde_json::Value {
		let mut obj = serde_json::Map::new();
		for (name, generator) in &self.fields {
			obj.insert(name.clone(), generator.generate());
		}
		serde_json::Value::Object(obj)
	}

	/// Checks if a field is configured.
	pub fn has_field(&self, name: &str) -> bool {
		self.fields.contains_key(name)
	}
}

/// Configuration for field overrides during build/create.
#[derive(Debug, Clone, Default)]
pub struct BuildConfig {
	/// Field overrides (name -> value).
	pub overrides: HashMap<String, serde_json::Value>,

	/// Whether to skip certain fields.
	pub skip_fields: Vec<String>,
}

impl BuildConfig {
	/// Creates a new empty build configuration.
	pub fn new() -> Self {
		Self::default()
	}

	/// Adds a field override.
	pub fn with_override(mut self, name: impl Into<String>, value: serde_json::Value) -> Self {
		self.overrides.insert(name.into(), value);
		self
	}

	/// Adds fields to skip.
	pub fn skip(mut self, fields: Vec<String>) -> Self {
		self.skip_fields = fields;
		self
	}

	/// Applies overrides to generated fields.
	pub fn apply(&self, mut fields: serde_json::Value) -> serde_json::Value {
		if let Some(obj) = fields.as_object_mut() {
			// Remove skipped fields
			for field in &self.skip_fields {
				obj.remove(field);
			}

			// Apply overrides
			for (name, value) in &self.overrides {
				obj.insert(name.clone(), value.clone());
			}
		}
		fields
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use serde_json::json;

	#[rstest]
	fn test_field_generator_static() {
		let generator = FieldGenerator::Static(json!("test_value"));
		assert_eq!(generator.generate(), json!("test_value"));
	}

	#[rstest]
	fn test_field_generator_faker() {
		let generator = FieldGenerator::Faker(FakerType::Email);
		let value = generator.generate();
		if let serde_json::Value::String(s) = value {
			assert!(s.contains('@'));
		} else {
			panic!("Expected string value");
		}
	}

	#[rstest]
	fn test_field_generator_sequence() {
		super::super::sequence::clear_sequences();

		let generator = FieldGenerator::Sequence {
			name: "test_builder_seq".to_string(),
			format: "CODE-{n}".to_string(),
		};

		assert_eq!(generator.generate(), json!("CODE-1"));
		assert_eq!(generator.generate(), json!("CODE-2"));
	}

	#[derive(Debug)]
	struct TestModel;

	#[rstest]
	fn test_factory_builder() {
		let builder = FactoryBuilder::<TestModel>::new("test.Model")
			.static_field("name", json!("test"))
			.faker_field("email", FakerType::Email)
			.sequence_field("code", "test_seq", "CODE-{n}");

		assert_eq!(builder.model_id(), "test.Model");
		assert!(builder.has_field("name"));
		assert!(builder.has_field("email"));
		assert!(builder.has_field("code"));
		assert!(!builder.has_field("other"));
	}

	#[rstest]
	fn test_factory_builder_generate_fields() {
		super::super::sequence::clear_sequences();

		let builder = FactoryBuilder::<TestModel>::new("test.Model")
			.static_field("name", json!("test"))
			.faker_field("email", FakerType::Email);

		let fields = builder.generate_fields();
		assert!(fields.is_object());
		assert_eq!(fields["name"], json!("test"));
		assert!(fields["email"].as_str().unwrap().contains('@'));
	}

	#[rstest]
	fn test_build_config() {
		let config = BuildConfig::new()
			.with_override("name", json!("override"))
			.skip(vec!["skip_me".to_string()]);

		let fields = json!({
			"name": "original",
			"email": "test@example.com",
			"skip_me": "should be removed"
		});

		let result = config.apply(fields);
		assert_eq!(result["name"], json!("override"));
		assert_eq!(result["email"], json!("test@example.com"));
		assert!(result.get("skip_me").is_none());
	}
}

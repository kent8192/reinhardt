//! Metadata configuration options

use std::collections::HashMap;

/// Field information from serializer introspection
#[derive(Debug, Clone)]
pub struct SerializerFieldInfo {
	/// Field name
	pub name: String,
	/// Rust type name
	pub type_name: String,
	/// Whether the field is optional (`Option<T>`)
	pub is_optional: bool,
	/// Whether the field is read-only
	pub is_read_only: bool,
	/// Whether the field is write-only
	pub is_write_only: bool,
}

/// Options for configuring metadata
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct MetadataOptions {
	pub name: String,
	pub description: String,
	pub allowed_methods: Vec<String>,
	pub renders: Vec<String>,
	pub parses: Vec<String>,
	/// Serializer field information for generating action metadata
	pub serializer_fields: Option<HashMap<String, SerializerFieldInfo>>,
}

impl Default for MetadataOptions {
	fn default() -> Self {
		Self {
			name: "API View".to_string(),
			description: "API endpoint".to_string(),
			allowed_methods: vec!["GET".to_string()],
			renders: vec!["application/json".to_string()],
			parses: vec!["application/json".to_string()],
			serializer_fields: None,
		}
	}
}

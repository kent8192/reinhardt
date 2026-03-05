//! Nested serializer configuration
//!
//! This module provides configuration for nested serializers in ModelSerializer.

use std::collections::HashMap;

/// Configuration for a single nested field
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct NestedFieldConfig {
	/// Field name that contains the relationship
	pub field_name: String,
	/// Maximum depth for nested serialization
	pub depth: usize,
	/// Whether to include this nested field in serialization
	pub read_only: bool,
	/// Whether to allow creating nested instances during deserialization
	pub allow_create: bool,
	/// Whether to allow updating nested instances during deserialization
	pub allow_update: bool,
}

impl NestedFieldConfig {
	/// Create a new nested field configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::serializers::nested_config::NestedFieldConfig;
	///
	/// let config = NestedFieldConfig::new("author");
	/// // Verify the config is created with correct defaults
	/// assert_eq!(config.field_name, "author");
	/// assert_eq!(config.depth, 1);
	/// let _: NestedFieldConfig = config;
	/// ```
	pub fn new(field_name: impl Into<String>) -> Self {
		Self {
			field_name: field_name.into(),
			depth: 1,
			read_only: false,
			allow_create: false,
			allow_update: false,
		}
	}

	/// Set the nesting depth
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::serializers::nested_config::NestedFieldConfig;
	///
	/// let config = NestedFieldConfig::new("author").depth(2);
	/// // Verify the depth is set correctly
	/// assert_eq!(config.depth, 2);
	/// let _: NestedFieldConfig = config;
	/// ```
	pub fn depth(mut self, depth: usize) -> Self {
		self.depth = depth;
		self
	}

	/// Mark this field as read-only
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::serializers::nested_config::NestedFieldConfig;
	///
	/// let config = NestedFieldConfig::new("author").read_only();
	/// // Verify read_only flag is set
	/// assert!(config.read_only);
	/// let _: NestedFieldConfig = config;
	/// ```
	pub fn read_only(mut self) -> Self {
		self.read_only = true;
		self
	}

	/// Allow creating nested instances
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::serializers::nested_config::NestedFieldConfig;
	///
	/// let config = NestedFieldConfig::new("author").allow_create();
	/// // Verify allow_create flag is set
	/// assert!(config.allow_create);
	/// let _: NestedFieldConfig = config;
	/// ```
	pub fn allow_create(mut self) -> Self {
		self.allow_create = true;
		self
	}

	/// Allow updating nested instances
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::serializers::nested_config::NestedFieldConfig;
	///
	/// let config = NestedFieldConfig::new("author").allow_update();
	/// // Verify allow_update flag is set
	/// assert!(config.allow_update);
	/// let _: NestedFieldConfig = config;
	/// ```
	pub fn allow_update(mut self) -> Self {
		self.allow_update = true;
		self
	}

	/// Allow both creating and updating nested instances
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::serializers::nested_config::NestedFieldConfig;
	///
	/// let config = NestedFieldConfig::new("author").writable();
	/// // Verify both allow_create and allow_update flags are set
	/// assert!(config.allow_create);
	/// assert!(config.allow_update);
	/// let _: NestedFieldConfig = config;
	/// ```
	pub fn writable(mut self) -> Self {
		self.allow_create = true;
		self.allow_update = true;
		self
	}
}

/// Configuration manager for nested serializers
#[non_exhaustive]
#[derive(Debug, Clone, Default)]
pub struct NestedSerializerConfig {
	/// Map of field names to their nested configurations
	nested_fields: HashMap<String, NestedFieldConfig>,
}

impl NestedSerializerConfig {
	/// Create a new empty nested serializer configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::serializers::nested_config::NestedSerializerConfig;
	///
	/// let config = NestedSerializerConfig::new();
	/// // Verify the config is created successfully
	/// let _: NestedSerializerConfig = config;
	/// ```
	pub fn new() -> Self {
		Self {
			nested_fields: HashMap::new(),
		}
	}

	/// Add a nested field configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::serializers::nested_config::{NestedSerializerConfig, NestedFieldConfig};
	///
	/// let mut config = NestedSerializerConfig::new();
	/// config.add_nested_field(NestedFieldConfig::new("author").depth(2));
	/// // Verify the nested field is added successfully
	/// assert!(config.is_nested_field("author"));
	/// ```
	pub fn add_nested_field(&mut self, field_config: NestedFieldConfig) {
		self.nested_fields
			.insert(field_config.field_name.clone(), field_config);
	}

	/// Get a nested field configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::serializers::nested_config::{NestedSerializerConfig, NestedFieldConfig};
	///
	/// let mut config = NestedSerializerConfig::new();
	/// config.add_nested_field(NestedFieldConfig::new("author"));
	///
	/// // Verify the nested field is retrieved correctly
	/// let author_config = config.get_nested_field("author");
	/// assert!(author_config.is_some());
	/// ```
	pub fn get_nested_field(&self, field_name: &str) -> Option<&NestedFieldConfig> {
		self.nested_fields.get(field_name)
	}

	/// Check if a field is configured as nested
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::serializers::nested_config::{NestedSerializerConfig, NestedFieldConfig};
	///
	/// let mut config = NestedSerializerConfig::new();
	/// config.add_nested_field(NestedFieldConfig::new("author"));
	///
	/// // Verify nested field presence
	/// assert!(config.is_nested_field("author"));
	/// assert!(!config.is_nested_field("title"));
	/// ```
	pub fn is_nested_field(&self, field_name: &str) -> bool {
		self.nested_fields.contains_key(field_name)
	}

	/// Get all nested field names
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::serializers::nested_config::{NestedSerializerConfig, NestedFieldConfig};
	///
	/// let mut config = NestedSerializerConfig::new();
	/// config.add_nested_field(NestedFieldConfig::new("author"));
	/// config.add_nested_field(NestedFieldConfig::new("category"));
	///
	/// // Verify all nested field names are retrieved
	/// let fields = config.nested_field_names();
	/// assert_eq!(fields.len(), 2);
	/// ```
	pub fn nested_field_names(&self) -> Vec<String> {
		self.nested_fields.keys().cloned().collect()
	}

	/// Remove a nested field configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::serializers::nested_config::{NestedSerializerConfig, NestedFieldConfig};
	///
	/// let mut config = NestedSerializerConfig::new();
	/// config.add_nested_field(NestedFieldConfig::new("author"));
	/// assert!(config.is_nested_field("author"));
	///
	/// // Verify nested field is removed correctly
	/// config.remove_nested_field("author");
	/// assert!(!config.is_nested_field("author"));
	/// ```
	pub fn remove_nested_field(&mut self, field_name: &str) -> Option<NestedFieldConfig> {
		self.nested_fields.remove(field_name)
	}

	/// Get the depth for a nested field
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::serializers::nested_config::{NestedSerializerConfig, NestedFieldConfig};
	///
	/// let mut config = NestedSerializerConfig::new();
	/// config.add_nested_field(NestedFieldConfig::new("author").depth(3));
	///
	/// // Verify depth is retrieved correctly
	/// assert_eq!(config.get_depth("author"), Some(3));
	/// assert_eq!(config.get_depth("title"), None);
	/// ```
	pub fn get_depth(&self, field_name: &str) -> Option<usize> {
		self.nested_fields.get(field_name).map(|c| c.depth)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_nested_field_config_new() {
		let config = NestedFieldConfig::new("author");
		assert_eq!(config.field_name, "author");
		assert_eq!(config.depth, 1);
		assert!(!config.read_only);
		assert!(!config.allow_create);
		assert!(!config.allow_update);
	}

	#[test]
	fn test_nested_field_config_depth() {
		let config = NestedFieldConfig::new("author").depth(3);
		assert_eq!(config.depth, 3);
	}

	#[test]
	fn test_nested_field_config_read_only() {
		let config = NestedFieldConfig::new("author").read_only();
		assert!(config.read_only);
	}

	#[test]
	fn test_nested_field_config_allow_create() {
		let config = NestedFieldConfig::new("author").allow_create();
		assert!(config.allow_create);
		assert!(!config.allow_update);
	}

	#[test]
	fn test_nested_field_config_allow_update() {
		let config = NestedFieldConfig::new("author").allow_update();
		assert!(!config.allow_create);
		assert!(config.allow_update);
	}

	#[test]
	fn test_nested_field_config_writable() {
		let config = NestedFieldConfig::new("author").writable();
		assert!(config.allow_create);
		assert!(config.allow_update);
	}

	#[test]
	fn test_nested_serializer_config_new() {
		let config = NestedSerializerConfig::new();
		assert_eq!(config.nested_field_names().len(), 0);
	}

	#[test]
	fn test_add_and_get_nested_field() {
		let mut config = NestedSerializerConfig::new();
		config.add_nested_field(NestedFieldConfig::new("author").depth(2));

		assert!(config.is_nested_field("author"));
		assert!(!config.is_nested_field("title"));

		let author_config = config.get_nested_field("author").unwrap();
		assert_eq!(author_config.field_name, "author");
		assert_eq!(author_config.depth, 2);
	}

	#[test]
	fn test_remove_nested_field() {
		let mut config = NestedSerializerConfig::new();
		config.add_nested_field(NestedFieldConfig::new("author"));

		assert!(config.is_nested_field("author"));

		let removed = config.remove_nested_field("author");
		assert!(removed.is_some());
		assert!(!config.is_nested_field("author"));
	}

	#[test]
	fn test_nested_field_names() {
		let mut config = NestedSerializerConfig::new();
		config.add_nested_field(NestedFieldConfig::new("author"));
		config.add_nested_field(NestedFieldConfig::new("category"));

		let names = config.nested_field_names();
		assert_eq!(names.len(), 2);
		assert!(names.contains(&"author".to_string()));
		assert!(names.contains(&"category".to_string()));
	}

	#[test]
	fn test_get_depth() {
		let mut config = NestedSerializerConfig::new();
		config.add_nested_field(NestedFieldConfig::new("author").depth(3));

		assert_eq!(config.get_depth("author"), Some(3));
		assert_eq!(config.get_depth("unknown"), None);
	}
}

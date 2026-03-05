/// Meta configuration for serializers
///
/// This module provides Django REST Framework-style Meta configuration
/// for customizing serializer behavior.
use std::collections::HashSet;

/// Meta configuration trait for serializers
///
/// Inspired by Django REST Framework's Meta class, this trait allows
/// serializers to declare configuration options such as which fields
/// to include/exclude and which fields are read-only.
///
/// # Examples
///
/// ```
/// use reinhardt_rest::serializers::meta::SerializerMeta;
/// use std::collections::HashSet;
///
/// struct UserSerializerMeta;
///
/// impl SerializerMeta for UserSerializerMeta {
///     fn fields() -> Option<Vec<String>> {
///         Some(vec!["id".to_string(), "username".to_string(), "email".to_string()])
///     }
///
///     fn read_only_fields() -> Vec<String> {
///         vec!["id".to_string()]
///     }
/// }
/// ```
pub trait SerializerMeta {
	/// Specify which fields to include in serialization
	///
	/// If None, all fields are included. If Some, only specified fields are included.
	fn fields() -> Option<Vec<String>> {
		None
	}

	/// Specify which fields to exclude from serialization
	///
	/// These fields will not be included in the output, even if specified in fields().
	fn exclude() -> Vec<String> {
		vec![]
	}

	/// Specify which fields are read-only
	///
	/// Read-only fields are included in serialization but not in deserialization.
	fn read_only_fields() -> Vec<String> {
		vec![]
	}

	/// Specify which fields are write-only
	///
	/// Write-only fields are included in deserialization but not in serialization.
	fn write_only_fields() -> Vec<String> {
		vec![]
	}

	/// Get the effective field set after applying all configuration
	///
	/// This method computes the final set of fields to be used for serialization
	/// by applying the fields, exclude, and other filters.
	fn effective_fields(all_fields: &[String]) -> HashSet<String> {
		let mut fields: HashSet<String> = if let Some(included) = Self::fields() {
			included.into_iter().collect()
		} else {
			all_fields.iter().cloned().collect()
		};

		// Remove excluded fields
		for field in Self::exclude() {
			fields.remove(&field);
		}

		fields
	}

	/// Check if a field is read-only
	fn is_read_only(field_name: &str) -> bool {
		Self::read_only_fields().contains(&field_name.to_string())
	}

	/// Check if a field is write-only
	fn is_write_only(field_name: &str) -> bool {
		Self::write_only_fields().contains(&field_name.to_string())
	}
}

/// Default meta configuration that includes all fields
pub struct DefaultMeta;

impl SerializerMeta for DefaultMeta {
	// Uses default trait implementations
}

/// Configuration builder for serializers
///
/// Provides a fluent interface for configuring serializer behavior
/// without requiring a custom Meta type.
///
/// # Examples
///
/// ```
/// use reinhardt_rest::serializers::meta::MetaConfig;
///
/// let config = MetaConfig::new()
///     .with_fields(vec!["id".to_string(), "username".to_string()])
///     .with_read_only_fields(vec!["id".to_string()]);
///
/// assert!(config.is_field_included("username"));
/// assert!(config.is_read_only("id"));
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, Default)]
pub struct MetaConfig {
	fields: Option<Vec<String>>,
	exclude: Vec<String>,
	read_only_fields: Vec<String>,
	write_only_fields: Vec<String>,
}

impl MetaConfig {
	/// Create a new meta configuration
	pub fn new() -> Self {
		Self::default()
	}

	/// Specify which fields to include
	pub fn with_fields(mut self, fields: Vec<String>) -> Self {
		self.fields = Some(fields);
		self
	}

	/// Specify which fields to exclude
	pub fn with_exclude(mut self, exclude: Vec<String>) -> Self {
		self.exclude = exclude;
		self
	}

	/// Specify which fields are read-only
	pub fn with_read_only_fields(mut self, fields: Vec<String>) -> Self {
		self.read_only_fields = fields;
		self
	}

	/// Specify which fields are write-only
	pub fn with_write_only_fields(mut self, fields: Vec<String>) -> Self {
		self.write_only_fields = fields;
		self
	}

	/// Get effective field set
	pub fn effective_fields(&self, all_fields: &[String]) -> HashSet<String> {
		let mut fields: HashSet<String> = if let Some(included) = &self.fields {
			included.iter().cloned().collect()
		} else {
			all_fields.iter().cloned().collect()
		};

		// Remove excluded fields
		for field in &self.exclude {
			fields.remove(field);
		}

		fields
	}

	/// Check if a field should be included in serialization
	pub fn is_field_included(&self, field_name: &str) -> bool {
		if self.exclude.contains(&field_name.to_string()) {
			return false;
		}

		if let Some(fields) = &self.fields {
			fields.contains(&field_name.to_string())
		} else {
			true
		}
	}

	/// Check if a field is read-only
	pub fn is_read_only(&self, field_name: &str) -> bool {
		self.read_only_fields.contains(&field_name.to_string())
	}

	/// Check if a field is write-only
	pub fn is_write_only(&self, field_name: &str) -> bool {
		self.write_only_fields.contains(&field_name.to_string())
	}

	/// Get the list of included fields
	pub fn fields(&self) -> Option<&Vec<String>> {
		self.fields.as_ref()
	}

	/// Get the list of excluded fields
	pub fn excluded_fields(&self) -> &Vec<String> {
		&self.exclude
	}

	/// Get the list of read-only fields
	pub fn read_only_fields(&self) -> &Vec<String> {
		&self.read_only_fields
	}

	/// Get the list of write-only fields
	pub fn write_only_fields(&self) -> &Vec<String> {
		&self.write_only_fields
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_default_meta() {
		let all_fields = vec!["id".to_string(), "name".to_string(), "email".to_string()];
		let effective = DefaultMeta::effective_fields(&all_fields);

		assert_eq!(effective.len(), 3);
		assert!(effective.contains("id"));
		assert!(effective.contains("name"));
		assert!(effective.contains("email"));
	}

	#[test]
	fn test_meta_config_with_fields() {
		let config = MetaConfig::new().with_fields(vec!["id".to_string(), "name".to_string()]);

		assert!(config.is_field_included("id"));
		assert!(config.is_field_included("name"));
		assert!(!config.is_field_included("email"));
	}

	#[test]
	fn test_meta_config_with_exclude() {
		let config = MetaConfig::new().with_exclude(vec!["password".to_string()]);

		assert!(config.is_field_included("id"));
		assert!(config.is_field_included("name"));
		assert!(!config.is_field_included("password"));
	}

	#[test]
	fn test_meta_config_read_only() {
		let config = MetaConfig::new().with_read_only_fields(vec!["id".to_string()]);

		assert!(config.is_read_only("id"));
		assert!(!config.is_read_only("name"));
	}

	#[test]
	fn test_meta_config_write_only() {
		let config = MetaConfig::new().with_write_only_fields(vec!["password".to_string()]);

		assert!(config.is_write_only("password"));
		assert!(!config.is_write_only("email"));
	}

	#[test]
	fn test_meta_config_effective_fields() {
		let all_fields = vec![
			"id".to_string(),
			"name".to_string(),
			"email".to_string(),
			"password".to_string(),
		];

		let config = MetaConfig::new()
			.with_fields(vec![
				"id".to_string(),
				"name".to_string(),
				"password".to_string(),
			])
			.with_exclude(vec!["password".to_string()]);

		let effective = config.effective_fields(&all_fields);

		assert_eq!(effective.len(), 2);
		assert!(effective.contains("id"));
		assert!(effective.contains("name"));
		assert!(!effective.contains("password"));
		assert!(!effective.contains("email"));
	}
}

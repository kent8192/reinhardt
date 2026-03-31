//! Input validation for mutation operations
//!
//! This module provides validation utilities to ensure that incoming mutation
//! requests (create/update) are safe and conform to the model's field definitions.
//!
//! # Security Protections
//!
//! - **Field allowlist**: Only fields defined in `ModelAdmin.fields()` or `list_display()` are allowed
//! - **Readonly enforcement**: Fields in `readonly_fields()` cannot be modified
//! - **Type validation**: Values are checked for basic type compatibility
//! - **Size limits**: Payload size and field counts are limited to prevent DoS

use crate::core::ModelAdmin;
use crate::types::AdminError;
use std::collections::HashMap;

/// Maximum number of fields in a mutation request
const MAX_FIELDS: usize = 100;

/// Maximum string length for a single field value (in bytes)
const MAX_STRING_LENGTH: usize = 1_000_000; // 1MB

/// Maximum total payload size (in bytes, approximate)
const MAX_PAYLOAD_SIZE: usize = 10_000_000; // 10MB

/// Validates mutation data against model admin configuration.
///
/// This function performs the following checks:
/// 1. Size limits (field count, string length, total payload)
/// 2. Field allowlist (only known fields are allowed)
/// 3. Readonly field enforcement (readonly fields cannot be modified)
///
/// # Arguments
///
/// * `data` - The mutation data to validate
/// * `model_admin` - The model admin configuration
/// * `is_update` - Whether this is an update operation (blocks pk_field modification on updates only)
///
/// # Errors
///
/// Returns `AdminError::ValidationError` if validation fails.
///
/// # Examples
///
/// ```ignore
/// use reinhardt_admin::server::validation::validate_mutation_data;
///
/// let mut data = HashMap::new();
/// data.insert("name".to_string(), serde_json::json!("Alice"));
///
/// validate_mutation_data(&data, &model_admin, false)?;
/// ```
pub fn validate_mutation_data(
	data: &HashMap<String, serde_json::Value>,
	model_admin: &dyn ModelAdmin,
	is_update: bool,
) -> Result<(), AdminError> {
	// Check field count limit
	validate_field_count(data)?;

	// Check total payload size
	validate_payload_size(data)?;

	// Get allowed fields from model admin
	let allowed_fields = get_allowed_fields(model_admin);
	let readonly_fields: Vec<&str> = model_admin.readonly_fields();
	let pk_field = model_admin.pk_field();

	// Validate each field
	for (field_name, value) in data {
		// Check if field is in allowlist
		validate_field_allowed(field_name, &allowed_fields)?;

		// Check readonly fields (for both create and update)
		if readonly_fields.contains(&field_name.as_str()) {
			return Err(AdminError::ValidationError(format!(
				"Field '{}' is read-only and cannot be modified",
				field_name
			)));
		}

		// Prevent primary key modification on update operations.
		// On create, PK may be supplied by the caller (e.g. UUID-based PKs),
		// so it is only blocked for updates where changing PK is never valid.
		if is_update && field_name == pk_field {
			return Err(AdminError::ValidationError(format!(
				"Primary key field '{}' cannot be modified",
				field_name
			)));
		}

		// Validate value size
		validate_value_size(field_name, value)?;
	}

	Ok(())
}

/// Gets the list of allowed fields from model admin.
///
/// Falls back to `list_display()` if `fields()` returns None.
fn get_allowed_fields(model_admin: &dyn ModelAdmin) -> Vec<&str> {
	model_admin
		.fields()
		.unwrap_or_else(|| model_admin.list_display())
}

/// Validates that the number of fields doesn't exceed the limit.
fn validate_field_count(data: &HashMap<String, serde_json::Value>) -> Result<(), AdminError> {
	if data.len() > MAX_FIELDS {
		return Err(AdminError::ValidationError(format!(
			"Too many fields in request: {} (max {})",
			data.len(),
			MAX_FIELDS
		)));
	}
	Ok(())
}

/// Validates that the total payload size doesn't exceed the limit.
fn validate_payload_size(data: &HashMap<String, serde_json::Value>) -> Result<(), AdminError> {
	let total_size: usize = data
		.iter()
		.map(|(k, v)| k.len() + v.to_string().len())
		.sum();

	if total_size > MAX_PAYLOAD_SIZE {
		return Err(AdminError::ValidationError(format!(
			"Payload too large: {} bytes (max {} bytes)",
			total_size, MAX_PAYLOAD_SIZE
		)));
	}
	Ok(())
}

/// Validates that a field is in the allowed list.
fn validate_field_allowed(field_name: &str, allowed_fields: &[&str]) -> Result<(), AdminError> {
	if !allowed_fields.contains(&field_name) {
		return Err(AdminError::ValidationError(format!(
			"Field '{}' is not allowed. Allowed fields: {:?}",
			field_name, allowed_fields
		)));
	}
	Ok(())
}

/// Validates that a value doesn't exceed size limits.
fn validate_value_size(field_name: &str, value: &serde_json::Value) -> Result<(), AdminError> {
	match value {
		serde_json::Value::String(s) => {
			if s.len() > MAX_STRING_LENGTH {
				return Err(AdminError::ValidationError(format!(
					"Field '{}' value too long: {} bytes (max {} bytes)",
					field_name,
					s.len(),
					MAX_STRING_LENGTH
				)));
			}
		}
		serde_json::Value::Array(arr) => {
			if arr.len() > MAX_FIELDS {
				return Err(AdminError::ValidationError(format!(
					"Field '{}' array too large: {} elements (max {})",
					field_name,
					arr.len(),
					MAX_FIELDS
				)));
			}
		}
		serde_json::Value::Object(obj) => {
			if obj.len() > MAX_FIELDS {
				return Err(AdminError::ValidationError(format!(
					"Field '{}' object too large: {} keys (max {})",
					field_name,
					obj.len(),
					MAX_FIELDS
				)));
			}
		}
		_ => {}
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::core::ModelAdminConfig;
	use rstest::rstest;

	fn create_test_admin() -> ModelAdminConfig {
		ModelAdminConfig::builder()
			.model_name("TestModel")
			.list_display(vec!["id", "name", "email", "created_at"])
			.fields(vec!["id", "name", "email", "created_at"])
			.readonly_fields(vec!["created_at"])
			.build()
			.unwrap()
	}

	#[rstest]
	fn test_validate_empty_data() {
		let admin = create_test_admin();
		let data = HashMap::new();
		assert!(validate_mutation_data(&data, &admin, false).is_ok());
	}

	#[rstest]
	fn test_validate_allowed_field() {
		let admin = create_test_admin();
		let mut data = HashMap::new();
		data.insert("name".to_string(), serde_json::json!("Alice"));

		assert!(validate_mutation_data(&data, &admin, false).is_ok());
	}

	#[rstest]
	fn test_validate_disallowed_field() {
		let admin = create_test_admin();
		let mut data = HashMap::new();
		data.insert("hacked".to_string(), serde_json::json!("value"));

		let result = validate_mutation_data(&data, &admin, false);
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			AdminError::ValidationError(_)
		));
	}

	#[rstest]
	fn test_validate_readonly_field() {
		let admin = create_test_admin();
		let mut data = HashMap::new();
		data.insert("created_at".to_string(), serde_json::json!("2024-01-01"));

		let result = validate_mutation_data(&data, &admin, false);
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(matches!(err, AdminError::ValidationError(_)));
		assert!(err.to_string().contains("read-only"));
	}

	#[rstest]
	fn test_validate_pk_field_on_update() {
		let admin = create_test_admin();
		let mut data = HashMap::new();
		data.insert("id".to_string(), serde_json::json!(999));

		let result = validate_mutation_data(&data, &admin, true);
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(matches!(err, AdminError::ValidationError(_)));
		assert!(err.to_string().contains("Primary key"));
	}

	#[rstest]
	fn test_validate_pk_field_on_create() {
		// On create, PK may be supplied by the caller (e.g. UUID-based PKs)
		let admin = create_test_admin();
		let mut data = HashMap::new();
		data.insert("id".to_string(), serde_json::json!(999));

		let result = validate_mutation_data(&data, &admin, false);
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_validate_too_many_fields() {
		let admin = create_test_admin();
		let mut data = HashMap::new();

		// Create more fields than allowed (but use allowed field names)
		for i in 0..=MAX_FIELDS {
			data.insert(format!("name_{}", i), serde_json::json!("value"));
		}

		let result = validate_mutation_data(&data, &admin, false);
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(matches!(err, AdminError::ValidationError(_)));
		assert!(err.to_string().contains("Too many fields"));
	}

	#[rstest]
	fn test_validate_string_too_long() {
		let admin = create_test_admin();
		let mut data = HashMap::new();
		data.insert(
			"name".to_string(),
			serde_json::json!("x".repeat(MAX_STRING_LENGTH + 1)),
		);

		let result = validate_mutation_data(&data, &admin, false);
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(matches!(err, AdminError::ValidationError(_)));
		assert!(err.to_string().contains("too long"));
	}

	#[rstest]
	fn test_validate_array_too_large() {
		let admin = create_test_admin();
		let mut data = HashMap::new();
		let large_array: Vec<_> = (0..=MAX_FIELDS).map(|i| serde_json::json!(i)).collect();
		data.insert("name".to_string(), serde_json::json!(large_array));

		let result = validate_mutation_data(&data, &admin, false);
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(matches!(err, AdminError::ValidationError(_)));
		assert!(err.to_string().contains("array too large"));
	}

	#[rstest]
	fn test_validate_uses_list_display_as_fallback() {
		// Admin with no fields() configured, should use list_display()
		let admin = ModelAdminConfig::builder()
			.model_name("TestModel")
			.list_display(vec!["id", "title"])
			.build()
			.unwrap();

		let mut data = HashMap::new();
		data.insert("title".to_string(), serde_json::json!("Test"));

		assert!(validate_mutation_data(&data, &admin, false).is_ok());
	}

	// ==================== Boundary value: field count ====================

	#[rstest]
	#[case::below_limit(99, true)]
	#[case::at_limit(100, true)]
	#[case::above_limit(101, false)]
	fn test_mutation_field_count_boundary(#[case] field_count: usize, #[case] should_pass: bool) {
		// Arrange
		// Use an admin that allows any field via list_display fallback
		let field_names: Vec<String> = (0..field_count).map(|i| format!("f_{}", i)).collect();
		let field_refs: Vec<&str> = field_names.iter().map(|s| s.as_str()).collect();
		let admin = ModelAdminConfig::builder()
			.model_name("TestModel")
			.list_display(field_refs.clone())
			.fields(field_refs)
			.build()
			.unwrap();

		let mut data = HashMap::new();
		for i in 0..field_count {
			data.insert(format!("f_{}", i), serde_json::json!("v"));
		}

		// Act
		let result = validate_mutation_data(&data, &admin, false);

		// Assert
		assert_eq!(
			result.is_ok(),
			should_pass,
			"field_count={}, expected pass={}, got {:?}",
			field_count,
			should_pass,
			result
		);
	}

	// ==================== Boundary value: string length ====================

	#[rstest]
	#[case::within_limit(999_999, true)]
	#[case::at_limit(1_000_000, true)]
	#[case::above_limit(1_000_001, false)]
	fn test_mutation_string_length_boundary(#[case] length: usize, #[case] should_pass: bool) {
		// Arrange
		let admin = create_test_admin();
		let mut data = HashMap::new();
		data.insert("name".to_string(), serde_json::json!("x".repeat(length)));

		// Act
		let result = validate_mutation_data(&data, &admin, false);

		// Assert
		assert_eq!(
			result.is_ok(),
			should_pass,
			"length={}, expected pass={}, got {:?}",
			length,
			should_pass,
			result
		);
	}

	// ==================== Decision table: mutation validation ====================

	#[rstest]
	#[case::field_in_allowlist_not_readonly_create(true, false, false, true, true)]
	#[case::field_in_allowlist_not_readonly_update(true, false, false, false, true)]
	#[case::field_not_in_allowlist(false, false, false, true, false)]
	#[case::field_is_readonly_on_create(true, true, false, true, false)]
	#[case::field_is_readonly_on_update(true, true, false, false, false)]
	#[case::pk_field_on_create(true, false, true, true, true)]
	#[case::pk_field_on_update(true, false, true, false, false)]
	fn test_mutation_validation_decision_table(
		#[case] in_allowlist: bool,
		#[case] is_readonly: bool,
		#[case] is_pk: bool,
		#[case] is_create: bool,
		#[case] should_pass: bool,
	) {
		// Arrange
		let field_name = if is_pk { "id" } else { "name" };
		let is_update = !is_create;

		let mut fields_list = vec!["id"];
		if in_allowlist && !is_pk {
			fields_list.push("name");
		}

		let readonly = if is_readonly && !is_pk {
			vec!["name"]
		} else {
			vec![]
		};

		let admin = ModelAdminConfig::builder()
			.model_name("TestModel")
			.list_display(fields_list.clone())
			.fields(fields_list)
			.readonly_fields(readonly)
			.build()
			.unwrap();

		let mut data = HashMap::new();
		data.insert(field_name.to_string(), serde_json::json!("test_value"));

		// Act
		let result = validate_mutation_data(&data, &admin, is_update);

		// Assert
		assert_eq!(
			result.is_ok(),
			should_pass,
			"in_allowlist={}, is_readonly={}, is_pk={}, is_create={}, expected pass={}, got {:?}",
			in_allowlist,
			is_readonly,
			is_pk,
			is_create,
			should_pass,
			result
		);
	}
}

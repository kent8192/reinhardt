//! Input validation for mutation operations
//!
//! This module provides validation utilities to ensure that incoming mutation
//! requests (create/update) are safe and conform to the model's field definitions.
//!
//! # Security Protections
//!
//! - **Field allowlist**: Only fields defined in `ModelAdmin.fields()`, `fieldsets()`, or `list_display()` are allowed
//! - **Readonly enforcement**: Fields in `readonly_fields()` cannot be modified
//! - **Type validation**: Values are checked for basic type compatibility
//! - **Size limits**: Payload size and field counts are limited to prevent DoS

use super::limits::MAX_RELATION_SELECTIONS;
use crate::core::ModelAdmin;
use crate::types::AdminError;
use std::collections::HashMap;

pub(crate) fn retain_allowed_fields<T: AsRef<str>>(
	data: &mut HashMap<String, serde_json::Value>,
	allowed_fields: &[T],
) {
	data.retain(|field, _| {
		allowed_fields
			.iter()
			.any(|allowed| field == allowed.as_ref())
	});
}

pub(crate) fn retain_allowed_fields_with_aliases<T: AsRef<str>>(
	data: &mut HashMap<String, serde_json::Value>,
	allowed_fields: &[T],
	aliases: &[(String, String)],
) {
	data.retain(|field, _| {
		allowed_fields.iter().any(|allowed| {
			let allowed = allowed.as_ref();
			field == allowed
				|| aliases.iter().any(|(logical, physical)| {
					(field == logical && allowed == physical)
						|| (field == physical && allowed == logical)
				})
		})
	});
}

/// Maximum number of fields in a mutation request
const MAX_FIELDS: usize = 100;

/// Maximum string length for a single field value (in bytes)
const MAX_STRING_LENGTH: usize = 1_000_000; // 1MB

/// Maximum total payload size (in bytes, approximate)
pub(super) const MAX_PAYLOAD_SIZE: usize = 10_000_000; // 10MB

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
	validate_mutation_data_with_aliases(data, model_admin, is_update, &[])
}

/// Validates mutation data while treating configured field aliases as equivalent.
pub(crate) fn validate_mutation_data_with_aliases(
	data: &HashMap<String, serde_json::Value>,
	model_admin: &dyn ModelAdmin,
	is_update: bool,
	field_aliases: &[(String, String)],
) -> Result<(), AdminError> {
	let allowed_fields = get_allowed_fields(model_admin)?;
	validate_mutation_data_inner(data, model_admin, is_update, &allowed_fields, field_aliases)
}

pub(super) fn validate_mutation_data_with_allowed_fields(
	data: &HashMap<String, serde_json::Value>,
	model_admin: &dyn ModelAdmin,
	is_update: bool,
	allowed_fields: &[&str],
) -> Result<(), AdminError> {
	let allowed_fields = allowed_fields
		.iter()
		.map(|field| (*field).to_string())
		.collect::<Vec<_>>();
	validate_mutation_data_inner(data, model_admin, is_update, &allowed_fields, &[])
}

fn validate_mutation_data_inner(
	data: &HashMap<String, serde_json::Value>,
	model_admin: &dyn ModelAdmin,
	is_update: bool,
	allowed_fields: &[String],
	field_aliases: &[(String, String)],
) -> Result<(), AdminError> {
	// Check field count limit
	validate_field_count(data)?;

	// Check total payload size
	validate_payload_size(data)?;

	let readonly_fields: Vec<&str> = model_admin.readonly_fields();
	let pk_field = model_admin.pk_field();
	let relation_fields = model_admin
		.filter_horizontal()
		.into_iter()
		.chain(model_admin.filter_vertical())
		.collect::<Vec<_>>();

	// Validate each field
	for (field_name, value) in data {
		// Check if field is in allowlist
		validate_field_allowed(field_name, allowed_fields, field_aliases)?;

		// Check readonly fields (for both create and update)
		if readonly_field_is_configured(field_name, &readonly_fields, field_aliases) {
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

		if relation_fields.contains(&field_name.as_str()) {
			if !is_update {
				validate_relation_selection_size(field_name, value)?;
			}
		} else {
			validate_value_size(field_name, value)?;
		}
	}

	Ok(())
}

fn validate_relation_selection_size(
	field_name: &str,
	value: &serde_json::Value,
) -> Result<(), AdminError> {
	if let serde_json::Value::Array(values) = value
		&& values.len() > MAX_RELATION_SELECTIONS
	{
		return Err(AdminError::ValidationError(format!(
			"Field '{}' relation selection too large: {} elements (max {})",
			field_name,
			values.len(),
			MAX_RELATION_SELECTIONS
		)));
	}
	Ok(())
}

/// Gets the list of allowed fields from model admin.
///
/// Falls back to `list_display()` if neither `fields()` nor `fieldsets()` is configured.
fn get_allowed_fields(model_admin: &dyn ModelAdmin) -> Result<Vec<String>, AdminError> {
	let (mut fields, _) = crate::core::resolve_form_fields(model_admin)?;
	for relation in model_admin
		.filter_horizontal()
		.into_iter()
		.chain(model_admin.filter_vertical())
		.chain(model_admin.autocomplete_fields())
		.chain(model_admin.raw_id_fields())
	{
		if !fields.iter().any(|field| field == relation) {
			fields.push(relation.to_string());
		}
	}
	Ok(fields)
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
fn validate_field_allowed(
	field_name: &str,
	allowed_fields: &[String],
	field_aliases: &[(String, String)],
) -> Result<(), AdminError> {
	if !field_or_alias_is_configured(field_name, allowed_fields, field_aliases) {
		return Err(AdminError::ValidationError(format!(
			"Field '{}' is not allowed. Allowed fields: {:?}",
			field_name, allowed_fields
		)));
	}
	Ok(())
}

fn field_or_alias_is_configured(
	field_name: &str,
	configured_fields: &[String],
	field_aliases: &[(String, String)],
) -> bool {
	configured_fields.iter().any(|field| field == field_name)
		|| field_aliases.iter().any(|(logical_name, column_name)| {
			(logical_name == field_name
				&& configured_fields.iter().any(|field| field == column_name))
				|| (column_name == field_name
					&& configured_fields.iter().any(|field| field == logical_name))
		})
}

fn readonly_field_is_configured(
	field_name: &str,
	readonly_fields: &[&str],
	field_aliases: &[(String, String)],
) -> bool {
	readonly_fields.contains(&field_name)
		|| field_aliases.iter().any(|(logical_name, column_name)| {
			(logical_name == field_name && readonly_fields.contains(&column_name.as_str()))
				|| (column_name == field_name && readonly_fields.contains(&logical_name.as_str()))
		})
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
		serde_json::Value::Array(arr) if arr.len() > MAX_FIELDS => {
			return Err(AdminError::ValidationError(format!(
				"Field '{}' array too large: {} elements (max {})",
				field_name,
				arr.len(),
				MAX_FIELDS
			)));
		}
		serde_json::Value::Object(obj) if obj.len() > MAX_FIELDS => {
			return Err(AdminError::ValidationError(format!(
				"Field '{}' object too large: {} keys (max {})",
				field_name,
				obj.len(),
				MAX_FIELDS
			)));
		}
		_ => {}
	}
	Ok(())
}

#[cfg(all(test, server))]
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
	fn retain_allowed_fields_removes_unconfigured_columns() {
		let mut data = HashMap::from([
			("id".to_string(), serde_json::json!(1)),
			("name".to_string(), serde_json::json!("visible")),
			("Name".to_string(), serde_json::json!("hidden")),
			("reset_token".to_string(), serde_json::json!("secret")),
		]);

		retain_allowed_fields(&mut data, &["name"]);

		assert_eq!(
			data,
			HashMap::from([("name".to_string(), serde_json::json!("visible"))])
		);
	}

	#[rstest]
	fn retain_allowed_fields_with_aliases_preserves_configured_database_column() {
		let mut data = HashMap::from([
			("headline_col".to_string(), serde_json::json!("Visible")),
			("secret_col".to_string(), serde_json::json!("hidden")),
		]);
		let aliases = vec![("headline".to_string(), "headline_col".to_string())];

		retain_allowed_fields_with_aliases(&mut data, &["headline"], &aliases);

		assert_eq!(
			data,
			HashMap::from([("headline_col".to_string(), serde_json::json!("Visible"))])
		);
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
	fn test_validate_relation_selection_limit() {
		// Arrange
		let admin = ModelAdminConfig::builder()
			.model_name("TestModel")
			.list_display(vec!["id"])
			.filter_horizontal(vec!["tags"])
			.build()
			.unwrap();
		let values = (0..=MAX_RELATION_SELECTIONS)
			.map(|value| serde_json::json!(value))
			.collect::<Vec<_>>();
		let data = HashMap::from([("tags".to_string(), serde_json::json!(values))]);

		// Act
		let result = validate_mutation_data(&data, &admin, false);

		// Assert
		let error = result.expect_err("oversized relation selections must be rejected");
		assert!(error.to_string().contains("relation selection too large"));
	}

	#[rstest]
	fn test_validate_large_relation_selection_on_update() {
		// Arrange
		let admin = ModelAdminConfig::builder()
			.model_name("TestModel")
			.list_display(vec!["id"])
			.filter_horizontal(vec!["tags"])
			.build()
			.unwrap();
		let values = (0..=MAX_RELATION_SELECTIONS)
			.map(|value| serde_json::json!(value))
			.collect::<Vec<_>>();
		let data = HashMap::from([("tags".to_string(), serde_json::json!(values))]);

		// Act
		let result = validate_mutation_data(&data, &admin, true);

		// Assert
		assert_eq!(result.map_err(|error| error.to_string()), Ok(()));
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

	#[rstest]
	fn test_validate_allows_relations_omitted_from_default_form_fields() {
		// Arrange
		let admin = ModelAdminConfig::builder()
			.model_name("TestModel")
			.list_display(vec!["id"])
			.autocomplete_fields(vec!["author"])
			.raw_id_fields(vec!["editor"])
			.build()
			.unwrap();
		let data = HashMap::from([
			("author".to_string(), serde_json::json!(1)),
			("editor".to_string(), serde_json::json!(2)),
		]);

		// Act
		let result = validate_mutation_data(&data, &admin, false);

		// Assert
		assert_eq!(result.map_err(|error| error.to_string()), Ok(()));
	}

	#[rstest]
	fn test_validate_allows_configured_fieldset_field() {
		// Arrange
		let admin = ModelAdminConfig::builder()
			.model_name("TestModel")
			.list_display(vec!["id"])
			.fieldsets(vec![
				crate::core::Fieldset::new(Some("Main"), &["title", "body"]),
				crate::core::Fieldset::new(Some("Publishing"), &["published_at"]),
			])
			.build()
			.unwrap();
		let mut data = HashMap::new();
		data.insert("body".to_string(), serde_json::json!("Draft"));

		// Act
		let result = validate_mutation_data(&data, &admin, false);

		// Assert
		assert_eq!(result.map_err(|error| error.to_string()), Ok(()));
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

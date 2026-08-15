//! Type inference for admin field metadata
//!
//! This module provides conversion utilities between database field types and
//! admin UI field types. It also infers whether fields are required based on
//! database constraints.
//!
//! # Architecture
//!
//! ```text
//! Database Layer              →  Admin Layer
//! ─────────────────────────────────────────────────
//! reinhardt_db::migrations::FieldType  →  admin_types::FieldType
//! FieldMetadata params (null/blank) →  required: bool
//! admin_types::FieldType           →  admin_types::FilterType
//! ```

use crate::core::database::{AdminRelatedField, SENSITIVE_FIELDS};
use crate::types::{
	AdminError, AdminResult, FieldType as AdminFieldType, FilterChoice, FilterType,
};
use reinhardt_apps::RelationshipMetadata as AppRelationshipMetadata;
use reinhardt_apps::registry::{RelationshipType, get_relationships_for_model};
use reinhardt_db::migrations::{
	FieldMetadata, FieldType as DbFieldType, ModelMetadata, ModelRegistry, global_registry,
};
#[cfg(test)]
use rust_decimal::Decimal;
use std::collections::{HashMap, HashSet};

/// Infers the admin UI field type from a database field type.
///
/// This conversion maps database-specific types to UI-appropriate form field types.
/// For example, `VARCHAR` becomes `Text`, while `TEXT` or `LONGTEXT` become `TextArea`.
///
/// # Examples
///
/// ```
/// use reinhardt_admin::server::type_inference::infer_admin_field_type;
/// use reinhardt_db::migrations::FieldType as DbFieldType;
/// use reinhardt_admin::types::FieldType as AdminFieldType;
///
/// assert_eq!(infer_admin_field_type(&DbFieldType::VarChar(255)), AdminFieldType::Text);
/// assert_eq!(infer_admin_field_type(&DbFieldType::Boolean), AdminFieldType::Boolean);
/// assert_eq!(infer_admin_field_type(&DbFieldType::Date), AdminFieldType::Date);
/// ```
pub fn infer_admin_field_type(db_type: &DbFieldType) -> AdminFieldType {
	match db_type {
		// Integer types → Number input
		DbFieldType::BigInteger
		| DbFieldType::Integer
		| DbFieldType::SmallInteger
		| DbFieldType::TinyInt
		| DbFieldType::MediumInt => AdminFieldType::Number,

		// Short string types → Text input
		DbFieldType::VarChar(_) | DbFieldType::Char(_) => AdminFieldType::Text,

		// Long text types → TextArea
		DbFieldType::Text
		| DbFieldType::TinyText
		| DbFieldType::MediumText
		| DbFieldType::LongText => AdminFieldType::TextArea,

		// Boolean → Boolean checkbox
		DbFieldType::Boolean => AdminFieldType::Boolean,

		// Date → Date picker
		DbFieldType::Date => AdminFieldType::Date,

		// Date/Time types → DateTime picker
		DbFieldType::DateTime | DbFieldType::TimestampTz | DbFieldType::Time => {
			AdminFieldType::DateTime
		}

		// Numeric types → Number input
		DbFieldType::Decimal { .. }
		| DbFieldType::Float
		| DbFieldType::Double
		| DbFieldType::Real => AdminFieldType::Number,

		// Enum → Select dropdown
		DbFieldType::Enum { values } => {
			let choices = values
				.iter()
				.map(|v| (v.clone(), humanize_value(v)))
				.collect();
			AdminFieldType::Select { choices }
		}

		// Set → MultiSelect
		DbFieldType::Set { values } => {
			let choices = values
				.iter()
				.map(|v| (v.clone(), humanize_value(v)))
				.collect();
			AdminFieldType::MultiSelect { choices }
		}

		// UUID → Text input (special format)
		DbFieldType::Uuid => AdminFieldType::Text,

		// Binary/Blob types → File upload
		DbFieldType::Binary
		| DbFieldType::Blob
		| DbFieldType::TinyBlob
		| DbFieldType::MediumBlob
		| DbFieldType::LongBlob
		| DbFieldType::Bytea => AdminFieldType::File,

		// JSON types → TextArea (for JSON editing)
		DbFieldType::Json | DbFieldType::JsonBinary => AdminFieldType::TextArea,

		// Year → Number input
		DbFieldType::Year => AdminFieldType::Number,

		// Relationship types → Hidden (handled separately)
		DbFieldType::OneToOne { .. }
		| DbFieldType::ManyToMany { .. }
		| DbFieldType::ForeignKey { .. } => AdminFieldType::Hidden,

		// Custom types → Text input as fallback
		DbFieldType::Custom(_) => AdminFieldType::Text,

		// PostgreSQL-specific types
		// Array types → MultiSelect for simple arrays, TextArea for complex
		DbFieldType::Array(inner) => match inner.as_ref() {
			DbFieldType::VarChar(_) | DbFieldType::Text | DbFieldType::CIText => {
				// String arrays can use MultiSelect
				AdminFieldType::MultiSelect {
					choices: Vec::new(), // Choices would be populated dynamically
				}
			}
			_ => AdminFieldType::TextArea, // Complex arrays use TextArea (JSON-like editing)
		},

		// HStore (key-value store) → TextArea for JSON-like editing
		DbFieldType::HStore => AdminFieldType::TextArea,

		// CIText (case-insensitive text) → Text input
		DbFieldType::CIText => AdminFieldType::Text,

		// Vectors → TextArea for structured multi-value editing
		#[cfg(feature = "pgvector")]
		DbFieldType::Vector { .. } => AdminFieldType::TextArea,

		// Range types → TextArea for range editing (e.g., "[1,10)" format)
		DbFieldType::Int4Range
		| DbFieldType::Int8Range
		| DbFieldType::NumRange
		| DbFieldType::DateRange
		| DbFieldType::TsRange
		| DbFieldType::TsTzRange => AdminFieldType::TextArea,

		// Full-text search types → TextArea
		DbFieldType::TsVector | DbFieldType::TsQuery => AdminFieldType::TextArea,
	}
}

/// Returns whether model metadata identifies a semantic file field.
pub(crate) fn is_semantic_file_field(metadata: &FieldMetadata) -> bool {
	metadata
		.params
		.get("model_field_type")
		.is_some_and(|field_type| matches!(field_type.as_str(), "file" | "image"))
}

/// Infers the admin field type while honoring semantic model field metadata.
pub(crate) fn infer_admin_field_type_from_metadata(metadata: &FieldMetadata) -> AdminFieldType {
	if is_semantic_file_field(metadata) {
		AdminFieldType::File
	} else {
		infer_admin_field_type(&metadata.field_type)
	}
}

/// Infers whether a field is required based on its metadata.
///
/// A field is considered required when:
/// - `null` parameter is NOT "true" (field cannot be NULL in database)
/// - `blank` parameter is NOT "true" (field cannot be empty in forms)
///
/// # Examples
///
/// ```
/// use reinhardt_admin::server::type_inference::infer_required;
/// use reinhardt_db::migrations::{FieldMetadata, FieldType};
///
/// // Field with null=false, blank=false is required
/// let meta = FieldMetadata::new(FieldType::VarChar(255));
/// assert!(infer_required(&meta));
///
/// // Field with null=true is not required
/// let meta = FieldMetadata::new(FieldType::VarChar(255))
///     .with_nullable(true);
/// assert!(!infer_required(&meta));
/// ```
pub fn infer_required(meta: &FieldMetadata) -> bool {
	let is_null = meta.nullable;
	let is_blank = meta
		.params
		.get("blank")
		.map(|v| v == "true")
		.unwrap_or(false);

	// Required if both null and blank are false (or not specified)
	!is_null && !is_blank
}

/// Infers the appropriate filter type for a given admin field type.
///
/// This determines how the field should be filtered in list views:
/// - Boolean fields get Yes/No filters
/// - Date/DateTime fields get date range filters
/// - Number fields get number range filters
/// - Enum/Select fields get choice filters
///
/// # Examples
///
/// ```
/// use reinhardt_admin::server::type_inference::infer_filter_type;
/// use reinhardt_admin::types::{FieldType, FilterType};
///
/// assert!(matches!(
///     infer_filter_type(&FieldType::Boolean),
///     FilterType::Boolean
/// ));
///
/// assert!(matches!(
///     infer_filter_type(&FieldType::Date),
///     FilterType::DateRange { .. }
/// ));
/// ```
pub fn infer_filter_type(admin_type: &AdminFieldType) -> FilterType {
	match admin_type {
		AdminFieldType::Boolean => FilterType::Boolean,

		AdminFieldType::Date | AdminFieldType::DateTime => FilterType::DateRange {
			ranges: default_date_ranges(),
		},

		AdminFieldType::Number => FilterType::NumberRange {
			ranges: default_number_ranges(),
		},

		AdminFieldType::Select { choices } => FilterType::Choice {
			choices: choices
				.iter()
				.map(|(value, label)| FilterChoice {
					value: value.clone(),
					label: label.clone(),
				})
				.collect(),
		},

		// For text fields and others, use a simple choice filter with common options
		_ => FilterType::Choice {
			choices: vec![
				FilterChoice {
					value: "all".to_string(),
					label: "All".to_string(),
				},
				FilterChoice {
					value: "empty".to_string(),
					label: "Empty".to_string(),
				},
				FilterChoice {
					value: "not_empty".to_string(),
					label: "Not Empty".to_string(),
				},
			],
		},
	}
}

/// Creates default date range filter choices.
fn default_date_ranges() -> Vec<FilterChoice> {
	vec![
		FilterChoice {
			value: "today".to_string(),
			label: "Today".to_string(),
		},
		FilterChoice {
			value: "past_7_days".to_string(),
			label: "Past 7 days".to_string(),
		},
		FilterChoice {
			value: "this_month".to_string(),
			label: "This month".to_string(),
		},
		FilterChoice {
			value: "this_year".to_string(),
			label: "This year".to_string(),
		},
	]
}

/// Creates default number range filter choices.
fn default_number_ranges() -> Vec<FilterChoice> {
	vec![
		FilterChoice {
			value: "0".to_string(),
			label: "Zero".to_string(),
		},
		FilterChoice {
			value: "positive".to_string(),
			label: "Positive".to_string(),
		},
		FilterChoice {
			value: "negative".to_string(),
			label: "Negative".to_string(),
		},
	]
}

/// Converts a database enum value to a human-readable label.
///
/// Example: "active_user" → "Active User"
fn humanize_value(value: &str) -> String {
	reinhardt_utils::utils_core::text::humanize_field_name(value)
}

/// Finds model metadata by table name from the global registry.
///
/// This is useful when you have a table name from ModelAdmin but need
/// to access field metadata from the migration registry.
///
/// # Examples
///
/// ```no_run
/// use reinhardt_admin::server::type_inference::find_model_by_table_name;
///
/// if let Some(metadata) = find_model_by_table_name("auth_user") {
///     for (field_name, field_meta) in &metadata.fields {
///         println!("Field: {}", field_name);
///     }
/// }
/// ```
pub fn find_model_by_table_name(table_name: &str) -> Option<ModelMetadata> {
	let registry = global_registry();
	registry
		.get_models()
		.into_iter()
		.find(|m| m.table_name == table_name)
}

/// Gets field metadata for a specific field from a model.
///
/// This combines table lookup and field extraction into a single helper.
///
/// # Examples
///
/// ```ignore
/// use reinhardt_admin::server::type_inference::get_field_metadata;
///
/// if let Some(field_meta) = get_field_metadata("auth_user", "email") {
///     let admin_type = infer_admin_field_type(&field_meta.field_type);
///     let required = infer_required(&field_meta);
/// }
/// ```
pub fn get_field_metadata(table_name: &str, field_name: &str) -> Option<FieldMetadata> {
	find_model_by_table_name(table_name).and_then(|model| find_field_metadata(&model, field_name))
}

pub(crate) fn translate_logical_field_names(
	table_name: &str,
	data: &mut HashMap<String, serde_json::Value>,
) -> Result<(), AdminError> {
	let Some(model) = find_model_by_table_name(table_name) else {
		return Ok(());
	};
	translate_logical_field_names_in_model(&model, data)
}

pub(crate) fn translate_physical_field_names_to_logical(
	table_name: &str,
	data: &mut HashMap<String, serde_json::Value>,
) -> Result<(), AdminError> {
	let Some(model) = find_model_by_table_name(table_name) else {
		return Ok(());
	};
	translate_physical_field_names_to_logical_in_model(&model, data)
}

fn translate_logical_field_names_in_model(
	model: &ModelMetadata,
	data: &mut HashMap<String, serde_json::Value>,
) -> Result<(), AdminError> {
	let mut translated = HashMap::with_capacity(data.len());
	for (field_name, value) in data.drain() {
		let physical_name = find_field_entry(model, &field_name)
			.map(|(column_name, metadata)| physical_field_name(column_name, metadata))
			.unwrap_or(field_name);
		if translated.insert(physical_name.clone(), value).is_some() {
			return Err(AdminError::ValidationError(format!(
				"Multiple form fields map to database column '{}'",
				physical_name
			)));
		}
	}
	*data = translated;
	Ok(())
}

fn translate_physical_field_names_to_logical_in_model(
	model: &ModelMetadata,
	data: &mut HashMap<String, serde_json::Value>,
) -> Result<(), AdminError> {
	let mut translated = HashMap::with_capacity(data.len());
	for (column_name, value) in data.drain() {
		let logical_name = find_physical_field_entry(model, &column_name)
			.map(|(registered_name, metadata)| logical_field_name(registered_name, metadata))
			.unwrap_or(column_name);
		if translated.insert(logical_name.clone(), value).is_some() {
			return Err(AdminError::ValidationError(format!(
				"Multiple database columns map to form field '{}'",
				logical_name
			)));
		}
	}
	*data = translated;
	Ok(())
}

fn find_field_entry<'a>(
	model: &'a ModelMetadata,
	field_name: &str,
) -> Option<(&'a String, &'a FieldMetadata)> {
	if let Some((column_name, metadata)) = model.fields.get_key_value(field_name) {
		return Some((column_name, metadata));
	}
	model
		.fields
		.iter()
		.find(|(_, metadata)| {
			metadata
				.params
				.get("db_column")
				.is_some_and(|column| column == field_name)
		})
		.or_else(|| {
			model.fields.iter().find(|(_, metadata)| {
				metadata
					.params
					.get("rust_field_name")
					.is_some_and(|name| name == field_name)
			})
		})
}

fn find_physical_field_entry<'a>(
	model: &'a ModelMetadata,
	column_name: &str,
) -> Option<(&'a String, &'a FieldMetadata)> {
	if let Some((registered_name, metadata)) = model.fields.get_key_value(column_name) {
		return Some((registered_name, metadata));
	}
	model.fields.iter().find(|(_, metadata)| {
		metadata
			.params
			.get("db_column")
			.is_some_and(|column| column == column_name)
	})
}

/// Resolved migration metadata for one configured foreign-key field.
#[derive(Debug, Clone)]
pub struct ForeignKeyFieldMetadata {
	/// Logical relation name declared on the model.
	pub logical_name: String,
	/// Persisted database column used for form submission.
	pub column_name: String,
	/// Logical target field configured by `#[rel(to_field = ...)]`.
	pub target_field: Option<String>,
	/// Raw migration field metadata for the persisted column.
	pub field_metadata: FieldMetadata,
	/// Qualified target model metadata.
	pub target_model: ModelMetadata,
}

/// Resolves a configured logical or physical field name to foreign-key metadata.
pub fn resolve_foreign_key_field_metadata(
	source_model: &ModelMetadata,
	configured_field_name: &str,
	relationships: &[&AppRelationshipMetadata],
	registry: &ModelRegistry,
) -> AdminResult<ForeignKeyFieldMetadata> {
	let relationship = relationships
		.iter()
		.copied()
		.find(|relationship| {
			relationship.field_name == configured_field_name
				|| relationship.db_column == Some(configured_field_name)
		})
		.ok_or_else(|| {
			AdminError::ValidationError(format!(
				"Field '{}' on model '{}' must be a foreign key",
				configured_field_name, source_model.model_name
			))
		})?;

	if relationship.relationship_type != RelationshipType::ForeignKey {
		return Err(AdminError::ValidationError(format!(
			"Field '{}' on model '{}' must be a foreign key",
			configured_field_name, source_model.model_name
		)));
	}

	let column_name = relationship.db_column.ok_or_else(|| {
		AdminError::ValidationError(format!(
			"Foreign key field '{}' has no persisted database column",
			relationship.field_name
		))
	})?;
	let field_metadata = find_field_metadata(source_model, column_name).ok_or_else(|| {
		AdminError::ValidationError(format!(
			"Foreign key field '{}' is missing migration metadata for column '{}'",
			relationship.field_name, column_name
		))
	})?;
	let target = field_metadata.params.get("fk_target").ok_or_else(|| {
		AdminError::ValidationError(format!(
			"Foreign key field '{}' is missing target model metadata",
			relationship.field_name
		))
	})?;
	let (qualified_app, target_model_name) = target
		.split_once('.')
		.map_or((None, target.as_str()), |(app, model)| (Some(app), model));
	let target_app = field_metadata
		.params
		.get("fk_target_app")
		.map(String::as_str)
		.or(qualified_app);
	let target_model = match target_app {
		Some(app) => registry.find_model_qualified(app, target_model_name),
		None => registry.find_model_by_name(target_model_name),
	}
	.ok_or_else(|| {
		let qualified_target = target_app
			.map(|app| format!("{app}.{target_model_name}"))
			.unwrap_or_else(|| target_model_name.to_string());
		AdminError::ValidationError(format!(
			"Target model '{}' for field '{}' is not registered",
			qualified_target, relationship.field_name
		))
	})?;

	Ok(ForeignKeyFieldMetadata {
		logical_name: relationship.field_name.to_string(),
		column_name: column_name.to_string(),
		target_field: field_metadata.params.get("fk_target_field").cloned(),
		field_metadata,
		target_model,
	})
}

/// Validates record IDs against their registered primary-key type.
#[cfg(test)]
pub(crate) fn validate_primary_key_ids(
	primary_key_type: &DbFieldType,
	ids: &[String],
) -> AdminResult<()> {
	for id in ids {
		let valid = match primary_key_type {
			DbFieldType::BigInteger => id.parse::<i64>().is_ok(),
			DbFieldType::Integer => id.parse::<i32>().is_ok(),
			DbFieldType::SmallInteger => id.parse::<i16>().is_ok(),
			DbFieldType::TinyInt => id.parse::<i8>().is_ok(),
			DbFieldType::MediumInt => id
				.parse::<i32>()
				.is_ok_and(|value| (-8_388_608..=8_388_607).contains(&value)),
			DbFieldType::Uuid => uuid::Uuid::parse_str(id).is_ok(),
			DbFieldType::Char(limit) | DbFieldType::VarChar(limit) => {
				!id.is_empty()
					&& !id.chars().any(char::is_control)
					&& id.chars().count() <= *limit as usize
			}
			DbFieldType::Text
			| DbFieldType::TinyText
			| DbFieldType::MediumText
			| DbFieldType::LongText
			| DbFieldType::CIText => !id.is_empty() && !id.chars().any(char::is_control),
			DbFieldType::Date => id.parse::<chrono::NaiveDate>().is_ok(),
			DbFieldType::Time => id.parse::<chrono::NaiveTime>().is_ok(),
			DbFieldType::DateTime => id.parse::<chrono::NaiveDateTime>().is_ok(),
			DbFieldType::TimestampTz => chrono::DateTime::parse_from_rfc3339(id).is_ok(),
			DbFieldType::Decimal { precision, scale } => id.parse::<Decimal>().is_ok_and(|value| {
				let mantissa_digits = value.mantissa().unsigned_abs().to_string().len();
				let integer_digits = mantissa_digits.saturating_sub(value.scale() as usize);
				value.scale() <= *scale
					&& integer_digits <= precision.saturating_sub(*scale) as usize
			}),
			DbFieldType::Float | DbFieldType::Real => id.parse::<f32>().is_ok_and(f32::is_finite),
			DbFieldType::Double => id.parse::<f64>().is_ok_and(f64::is_finite),
			DbFieldType::Boolean => id.parse::<bool>().is_ok(),
			DbFieldType::Year => {
				id == "0000"
					|| id
						.parse::<u16>()
						.is_ok_and(|year| (1901..=2155).contains(&year))
			}
			DbFieldType::Enum { values } => values.iter().any(|value| value == id),
			DbFieldType::Json | DbFieldType::JsonBinary => {
				serde_json::from_str::<serde_json::Value>(id).is_ok()
			}
			DbFieldType::ForeignKey { .. } | DbFieldType::OneToOne { .. } => {
				id.parse::<i64>().is_ok()
			}
			_ => false,
		};

		if !valid {
			return Err(AdminError::ValidationError("Invalid record ID".to_string()));
		}
	}

	Ok(())
}

/// Canonicalizes validated primary-key IDs before action execution.
#[cfg(test)]
pub(crate) fn canonicalize_primary_key_ids(
	primary_key_type: &DbFieldType,
	ids: &[String],
) -> AdminResult<Vec<String>> {
	validate_primary_key_ids(primary_key_type, ids)?;
	Ok(ids
		.iter()
		.map(|id| match primary_key_type {
			DbFieldType::BigInteger => id
				.parse::<i64>()
				.map_or_else(|_| id.clone(), |value| value.to_string()),
			DbFieldType::Integer | DbFieldType::MediumInt => id
				.parse::<i32>()
				.map_or_else(|_| id.clone(), |value| value.to_string()),
			DbFieldType::SmallInteger => id
				.parse::<i16>()
				.map_or_else(|_| id.clone(), |value| value.to_string()),
			DbFieldType::TinyInt => id
				.parse::<i8>()
				.map_or_else(|_| id.clone(), |value| value.to_string()),
			DbFieldType::ForeignKey { .. } | DbFieldType::OneToOne { .. } => id
				.parse::<i64>()
				.map_or_else(|_| id.clone(), |value| value.to_string()),
			DbFieldType::Uuid => {
				uuid::Uuid::parse_str(id).map_or_else(|_| id.clone(), |value| value.to_string())
			}
			_ => id.clone(),
		})
		.collect())
}

fn physical_field_name(column_name: &str, metadata: &FieldMetadata) -> String {
	metadata
		.params
		.get("db_column")
		.cloned()
		.unwrap_or_else(|| column_name.to_string())
}

fn logical_field_name(column_name: &str, metadata: &FieldMetadata) -> String {
	metadata
		.params
		.get("rust_field_name")
		.cloned()
		.unwrap_or_else(|| column_name.to_string())
}

fn find_field_metadata(model: &ModelMetadata, field_name: &str) -> Option<FieldMetadata> {
	if let Some((_, meta)) = find_field_entry(model, field_name) {
		return Some(meta.clone());
	}

	let relation_name = field_name.strip_suffix("_id")?;
	let mut meta = model.fields.get(relation_name)?.clone();
	match meta.field_type {
		DbFieldType::ForeignKey { .. } | DbFieldType::OneToOne { .. } => {
			meta.field_type = DbFieldType::BigInteger;
			Some(meta)
		}
		_ => None,
	}
}

fn is_loadable_related_column(column: &str, metadata: &FieldMetadata) -> bool {
	let logical_name = metadata
		.params
		.get("field_name")
		.map(String::as_str)
		.unwrap_or(column);
	!SENSITIVE_FIELDS.contains(&column)
		&& !SENSITIVE_FIELDS.contains(&logical_name)
		&& metadata.params.get("skip_info").map(String::as_str) != Some("true")
}

pub(crate) fn resolve_list_select_related(
	table_name: &str,
	relation_names: &[&str],
) -> AdminResult<Vec<AdminRelatedField>> {
	if relation_names.is_empty() {
		return Ok(Vec::new());
	}

	let source_model = find_model_by_table_name(table_name).ok_or_else(|| {
		AdminError::ValidationError(format!(
			"list_select_related cannot resolve model metadata for table '{table_name}'"
		))
	})?;
	let qualified_source = format!("{}.{}", source_model.app_label, source_model.model_name);
	let relationships = get_relationships_for_model(&qualified_source);
	let mut seen = HashSet::new();
	let mut resolved = Vec::with_capacity(relation_names.len());

	for relation_name in relation_names {
		if !seen.insert(*relation_name) {
			return Err(AdminError::ValidationError(format!(
				"list_select_related contains duplicate relationship '{relation_name}'"
			)));
		}

		let relationship = relationships
			.iter()
			.find(|relationship| relationship.field_name == *relation_name)
			.ok_or_else(|| {
				AdminError::ValidationError(format!(
					"list_select_related field '{relation_name}' is not a declared relationship on '{}'",
					source_model.model_name
				))
			})?;
		if relationship.relationship_type != RelationshipType::ForeignKey {
			return Err(AdminError::ValidationError(format!(
				"list_select_related field '{relation_name}' is not a declared foreign key"
			)));
		}

		let source_column = relationship.db_column.ok_or_else(|| {
			AdminError::ValidationError(format!(
				"list_select_related foreign key '{relation_name}' has no source column metadata"
			))
		})?;
		let source_field = source_model.fields.get(source_column).ok_or_else(|| {
			AdminError::ValidationError(format!(
				"list_select_related foreign key '{relation_name}' cannot resolve source column '{source_column}'"
			))
		})?;

		let target_app = source_field.params.get("fk_target_app").ok_or_else(|| {
			AdminError::ValidationError(format!(
				"list_select_related foreign key '{relation_name}' has no target app metadata"
			))
		})?;
		let target_model_name = source_field.params.get("fk_target").ok_or_else(|| {
			AdminError::ValidationError(format!(
				"list_select_related foreign key '{relation_name}' has no target model metadata"
			))
		})?;
		let target_model = global_registry()
			.find_model_qualified(target_app, target_model_name)
			.ok_or_else(|| {
				AdminError::ValidationError(format!(
					"list_select_related foreign key '{relation_name}' cannot resolve target model '{target_app}.{target_model_name}'"
				))
			})?;

		let target_column = source_field
			.params
			.get("fk_target_column")
			.cloned()
			.or_else(|| {
				source_field
					.foreign_key
					.as_ref()
					.map(|foreign_key| foreign_key.referenced_column.clone())
			})
			.ok_or_else(|| {
				AdminError::ValidationError(format!(
					"list_select_related foreign key '{relation_name}' has no target column metadata"
				))
			})?;
		if !target_model.fields.contains_key(&target_column) {
			return Err(AdminError::ValidationError(format!(
				"list_select_related foreign key '{relation_name}' targets unknown column '{}.{}'",
				target_model.table_name, target_column
			)));
		}
		let presence_column = target_model
			.fields
			.iter()
			.find(|(_, metadata)| {
				metadata.params.get("primary_key").map(String::as_str) == Some("true")
			})
			.map(|(column, _)| column.clone())
			.ok_or_else(|| {
				AdminError::ValidationError(format!(
					"list_select_related target model '{}' has no primary key metadata",
					target_model.table_name
				))
			})?;

		let mut columns = target_model
			.fields
			.iter()
			.filter(|(column, metadata)| is_loadable_related_column(column, metadata))
			.map(|(column, _)| column.clone())
			.collect::<Vec<_>>();
		columns.sort();
		if columns.is_empty() {
			return Err(AdminError::ValidationError(format!(
				"list_select_related foreign key '{relation_name}' has no loadable target columns"
			)));
		}

		resolved.push(AdminRelatedField {
			relation_name: (*relation_name).to_string(),
			source_column: source_column.to_string(),
			target_table: target_model.table_name,
			target_column,
			presence_column,
			columns,
		});
	}

	Ok(resolved)
}

#[cfg(all(test, server))]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_find_field_metadata_resolves_logical_name_to_custom_column() {
		// Arrange
		let mut model = ModelMetadata::new("admin", "Article", "articles");
		model.add_field(
			"email_address".to_string(),
			FieldMetadata::new(DbFieldType::VarChar(255))
				.with_param("rust_field_name", "email")
				.with_param("db_column", "email_address"),
		);

		// Act
		let metadata = find_field_metadata(&model, "email").expect("logical field should resolve");

		// Assert
		assert_eq!(
			metadata.params.get("db_column").map(String::as_str),
			Some("email_address")
		);
	}

	#[rstest]
	fn test_translate_logical_field_names_to_custom_columns() {
		// Arrange
		let mut model = ModelMetadata::new("admin", "Article", "articles");
		model.add_field(
			"email_address".to_string(),
			FieldMetadata::new(DbFieldType::VarChar(255))
				.with_param("rust_field_name", "email")
				.with_param("db_column", "email_address"),
		);
		let mut data = HashMap::from([(
			String::from("email"),
			serde_json::json!("alice@example.com"),
		)]);

		// Act
		translate_logical_field_names_in_model(&model, &mut data).expect("field names should map");

		// Assert
		assert_eq!(
			data.get("email_address"),
			Some(&serde_json::json!("alice@example.com"))
		);
		assert!(!data.contains_key("email"));
	}

	#[rstest]
	fn test_translate_physical_field_names_to_logical_names() {
		// Arrange
		let mut model = ModelMetadata::new("admin", "Article", "articles");
		model.add_field(
			"email_address".to_string(),
			FieldMetadata::new(DbFieldType::VarChar(255))
				.with_param("rust_field_name", "email")
				.with_param("db_column", "email_address"),
		);
		let mut data = HashMap::from([(
			String::from("email_address"),
			serde_json::json!("alice@example.com"),
		)]);

		// Act
		translate_physical_field_names_to_logical_in_model(&model, &mut data)
			.expect("field names should map");

		// Assert
		assert_eq!(
			data.get("email"),
			Some(&serde_json::json!("alice@example.com"))
		);
		assert!(!data.contains_key("email_address"));
	}

	#[test]
	fn test_related_columns_exclude_sensitive_metadata() {
		let password =
			FieldMetadata::new(DbFieldType::VarChar(255)).with_param("skip_info", "true");
		let username = FieldMetadata::new(DbFieldType::VarChar(255));

		assert!(!is_loadable_related_column("pwd_hash", &password));
		assert!(is_loadable_related_column("username", &username));
	}

	#[test]
	fn test_related_columns_exclude_sensitive_logical_name_with_custom_db_column() {
		let password = FieldMetadata::new(DbFieldType::VarChar(255))
			.with_param("field_name", "password_hash")
			.with_param("db_column", "credential_blob");

		assert!(!is_loadable_related_column("credential_blob", &password));
	}

	#[test]
	fn test_resolve_list_select_related_rejects_unknown_source_model() {
		// Act
		let error = resolve_list_select_related("missing_admin_table", &["owner"])
			.expect_err("missing source model must be rejected");

		// Assert
		let AdminError::ValidationError(message) = error else {
			panic!("expected validation error");
		};
		assert_eq!(
			message,
			"list_select_related cannot resolve model metadata for table 'missing_admin_table'"
		);
	}

	#[test]
	fn test_infer_admin_field_type_integers() {
		assert_eq!(
			infer_admin_field_type(&DbFieldType::Integer),
			AdminFieldType::Number
		);
		assert_eq!(
			infer_admin_field_type(&DbFieldType::BigInteger),
			AdminFieldType::Number
		);
		assert_eq!(
			infer_admin_field_type(&DbFieldType::SmallInteger),
			AdminFieldType::Number
		);
	}

	#[rstest]
	fn semantic_file_metadata_is_detected_without_changing_physical_inference() {
		let file =
			FieldMetadata::new(DbFieldType::VarChar(255)).with_param("model_field_type", "file");
		let image =
			FieldMetadata::new(DbFieldType::VarChar(255)).with_param("model_field_type", "image");
		let varchar = FieldMetadata::new(DbFieldType::VarChar(255));
		let binary = FieldMetadata::new(DbFieldType::Binary);

		assert!(is_semantic_file_field(&file));
		assert!(is_semantic_file_field(&image));
		assert!(!is_semantic_file_field(&varchar));
		assert!(!is_semantic_file_field(&binary));
		assert_eq!(
			infer_admin_field_type_from_metadata(&file),
			AdminFieldType::File
		);
		assert_eq!(
			infer_admin_field_type_from_metadata(&image),
			AdminFieldType::File
		);
		assert_eq!(
			infer_admin_field_type_from_metadata(&varchar),
			AdminFieldType::Text
		);
		assert_eq!(
			infer_admin_field_type_from_metadata(&binary),
			AdminFieldType::File
		);
	}

	#[test]
	fn validate_primary_key_ids_rejects_values_outside_the_registered_type() {
		assert!(validate_primary_key_ids(&DbFieldType::Integer, &["42".to_string()]).is_ok());
		assert!(
			validate_primary_key_ids(&DbFieldType::Integer, &["not-an-id".to_string()]).is_err()
		);
		assert!(
			validate_primary_key_ids(
				&DbFieldType::Uuid,
				&["00000000-0000-0000-0000-000000000001".to_string()],
			)
			.is_ok()
		);
		assert!(validate_primary_key_ids(&DbFieldType::Uuid, &["not-a-uuid".to_string()]).is_err());
		assert!(
			validate_primary_key_ids(&DbFieldType::VarChar(32), &["slug-42".to_string()]).is_ok()
		);
		assert!(
			validate_primary_key_ids(&DbFieldType::VarChar(32), &["\u{0000}".to_string()]).is_err()
		);
		assert!(matches!(
			validate_primary_key_ids(&DbFieldType::VarChar(32), &[String::new()]),
			Err(AdminError::ValidationError(_))
		));
	}

	#[rstest]
	#[case(DbFieldType::TinyInt, "-128", true)]
	#[case(DbFieldType::TinyInt, "127", true)]
	#[case(DbFieldType::TinyInt, "-129", false)]
	#[case(DbFieldType::TinyInt, "128", false)]
	#[case(DbFieldType::SmallInteger, "-32768", true)]
	#[case(DbFieldType::SmallInteger, "32767", true)]
	#[case(DbFieldType::SmallInteger, "-32769", false)]
	#[case(DbFieldType::SmallInteger, "32768", false)]
	#[case(DbFieldType::MediumInt, "-8388608", true)]
	#[case(DbFieldType::MediumInt, "8388607", true)]
	#[case(DbFieldType::MediumInt, "-8388609", false)]
	#[case(DbFieldType::MediumInt, "8388608", false)]
	fn validate_primary_key_ids_enforces_integer_storage_ranges(
		#[case] field_type: DbFieldType,
		#[case] value: &str,
		#[case] expected_valid: bool,
	) {
		let result = validate_primary_key_ids(&field_type, &[value.to_string()]);

		assert_eq!(result.is_ok(), expected_valid);
	}

	#[rstest]
	#[case::char_bound(DbFieldType::Char(3), "abcd", false)]
	#[case::varchar_bound(DbFieldType::VarChar(3), "abcd", false)]
	#[case::valid_date(DbFieldType::Date, "2026-08-11", true)]
	#[case::invalid_date(DbFieldType::Date, "not-a-date", false)]
	#[case::valid_decimal(
		DbFieldType::Decimal {
			precision: 5,
			scale: 2,
		},
		"123.45",
		true
	)]
	#[case::decimal_precision_overflow(
		DbFieldType::Decimal {
			precision: 5,
			scale: 2,
		},
		"1234.56",
		false
	)]
	#[case::decimal_integer_digit_overflow(
		DbFieldType::Decimal {
			precision: 5,
			scale: 2,
		},
		"1234",
		false
	)]
	#[case::decimal_scale_overflow(
		DbFieldType::Decimal {
			precision: 5,
			scale: 2,
		},
		"123.456",
		false
	)]
	#[case::valid_float(DbFieldType::Float, "1.25", true)]
	#[case::invalid_float(DbFieldType::Float, "not-a-number", false)]
	fn validate_primary_key_ids_enforces_registered_scalar_formats(
		#[case] field_type: DbFieldType,
		#[case] value: &str,
		#[case] expected_valid: bool,
	) {
		// Act
		let result = validate_primary_key_ids(&field_type, &[value.to_string()]);

		// Assert
		assert_eq!(result.is_ok(), expected_valid);
	}

	#[test]
	fn canonicalize_primary_key_ids_collapses_equivalent_integer_and_uuid_values() {
		assert_eq!(
			canonicalize_primary_key_ids(&DbFieldType::BigInteger, &["+007".to_string()]).unwrap(),
			["7"]
		);
		assert_eq!(
			canonicalize_primary_key_ids(
				&DbFieldType::Uuid,
				&["550E8400-E29B-41D4-A716-446655440000".to_string()]
			)
			.unwrap(),
			["550e8400-e29b-41d4-a716-446655440000"]
		);
	}

	#[test]
	fn test_infer_admin_field_type_strings() {
		assert_eq!(
			infer_admin_field_type(&DbFieldType::VarChar(255)),
			AdminFieldType::Text
		);
		assert_eq!(
			infer_admin_field_type(&DbFieldType::Char(10)),
			AdminFieldType::Text
		);
		assert_eq!(
			infer_admin_field_type(&DbFieldType::Text),
			AdminFieldType::TextArea
		);
		assert_eq!(
			infer_admin_field_type(&DbFieldType::LongText),
			AdminFieldType::TextArea
		);
	}

	#[test]
	fn test_infer_admin_field_type_datetime() {
		assert_eq!(
			infer_admin_field_type(&DbFieldType::Boolean),
			AdminFieldType::Boolean
		);
		assert_eq!(
			infer_admin_field_type(&DbFieldType::Date),
			AdminFieldType::Date
		);
		assert_eq!(
			infer_admin_field_type(&DbFieldType::DateTime),
			AdminFieldType::DateTime
		);
	}

	#[test]
	fn test_infer_admin_field_type_enum() {
		let db_type = DbFieldType::Enum {
			values: vec!["active".to_string(), "inactive".to_string()],
		};
		let admin_type = infer_admin_field_type(&db_type);

		match admin_type {
			AdminFieldType::Select { choices } => {
				assert_eq!(choices.len(), 2);
				assert_eq!(choices[0].0, "active");
				assert_eq!(choices[1].0, "inactive");
			}
			_ => panic!("Expected Select variant"),
		}
	}

	#[test]
	fn test_infer_required_default() {
		let meta = FieldMetadata::new(DbFieldType::VarChar(255));
		assert!(infer_required(&meta));
	}

	#[test]
	fn test_infer_required_null_true() {
		let meta = FieldMetadata::new(DbFieldType::VarChar(255)).with_nullable(true);
		assert!(!infer_required(&meta));
	}

	#[test]
	fn test_infer_required_blank_true() {
		let meta = FieldMetadata::new(DbFieldType::VarChar(255)).with_param("blank", "true");
		assert!(!infer_required(&meta));
	}

	#[test]
	fn test_infer_required_both_false() {
		let meta = FieldMetadata::new(DbFieldType::VarChar(255))
			.with_nullable(false)
			.with_param("blank", "false");
		assert!(infer_required(&meta));
	}

	#[test]
	fn test_infer_filter_type_boolean() {
		assert!(matches!(
			infer_filter_type(&AdminFieldType::Boolean),
			FilterType::Boolean
		));
	}

	#[test]
	fn test_infer_filter_type_date() {
		let filter = infer_filter_type(&AdminFieldType::Date);
		match filter {
			FilterType::DateRange { ranges } => {
				assert!(!ranges.is_empty());
				assert!(ranges.iter().any(|r| r.value == "today"));
			}
			_ => panic!("Expected DateRange variant"),
		}
	}

	#[test]
	fn test_infer_filter_type_number() {
		let filter = infer_filter_type(&AdminFieldType::Number);
		match filter {
			FilterType::NumberRange { ranges } => {
				assert!(!ranges.is_empty());
			}
			_ => panic!("Expected NumberRange variant"),
		}
	}

	#[test]
	fn test_infer_filter_type_select() {
		let admin_type = AdminFieldType::Select {
			choices: vec![
				("active".to_string(), "Active".to_string()),
				("inactive".to_string(), "Inactive".to_string()),
			],
		};
		let filter = infer_filter_type(&admin_type);

		match filter {
			FilterType::Choice { choices } => {
				assert_eq!(choices.len(), 2);
				assert_eq!(choices[0].value, "active");
				assert_eq!(choices[0].label, "Active");
			}
			_ => panic!("Expected Choice variant"),
		}
	}

	// ──────────────────────────────────────────────────────────────
	// Additional type inference tests
	// ──────────────────────────────────────────────────────────────

	#[test]
	fn test_infer_admin_field_type_decimal() {
		assert_eq!(
			infer_admin_field_type(&DbFieldType::Decimal {
				precision: 10,
				scale: 2
			}),
			AdminFieldType::Number
		);
	}

	#[test]
	fn test_infer_admin_field_type_float_double_real() {
		assert_eq!(
			infer_admin_field_type(&DbFieldType::Float),
			AdminFieldType::Number
		);
		assert_eq!(
			infer_admin_field_type(&DbFieldType::Double),
			AdminFieldType::Number
		);
		assert_eq!(
			infer_admin_field_type(&DbFieldType::Real),
			AdminFieldType::Number
		);
	}

	#[test]
	fn test_infer_admin_field_type_uuid() {
		assert_eq!(
			infer_admin_field_type(&DbFieldType::Uuid),
			AdminFieldType::Text
		);
	}

	#[test]
	fn test_infer_admin_field_type_binary() {
		assert_eq!(
			infer_admin_field_type(&DbFieldType::Binary),
			AdminFieldType::File
		);
		assert_eq!(
			infer_admin_field_type(&DbFieldType::Blob),
			AdminFieldType::File
		);
		assert_eq!(
			infer_admin_field_type(&DbFieldType::TinyBlob),
			AdminFieldType::File
		);
		assert_eq!(
			infer_admin_field_type(&DbFieldType::MediumBlob),
			AdminFieldType::File
		);
		assert_eq!(
			infer_admin_field_type(&DbFieldType::LongBlob),
			AdminFieldType::File
		);
		assert_eq!(
			infer_admin_field_type(&DbFieldType::Bytea),
			AdminFieldType::File
		);
	}

	#[test]
	fn test_infer_admin_field_type_json() {
		assert_eq!(
			infer_admin_field_type(&DbFieldType::Json),
			AdminFieldType::TextArea
		);
		assert_eq!(
			infer_admin_field_type(&DbFieldType::JsonBinary),
			AdminFieldType::TextArea
		);
	}

	#[test]
	fn test_infer_admin_field_type_year() {
		assert_eq!(
			infer_admin_field_type(&DbFieldType::Year),
			AdminFieldType::Number
		);
	}

	#[test]
	fn test_infer_admin_field_type_time() {
		assert_eq!(
			infer_admin_field_type(&DbFieldType::Time),
			AdminFieldType::DateTime
		);
		assert_eq!(
			infer_admin_field_type(&DbFieldType::TimestampTz),
			AdminFieldType::DateTime
		);
	}

	#[test]
	fn test_infer_admin_field_type_set() {
		let db_type = DbFieldType::Set {
			values: vec![
				"read".to_string(),
				"write".to_string(),
				"delete".to_string(),
			],
		};
		let admin_type = infer_admin_field_type(&db_type);

		match admin_type {
			AdminFieldType::MultiSelect { choices } => {
				assert_eq!(choices.len(), 3);
				assert_eq!(choices[0].0, "read");
				assert_eq!(choices[1].0, "write");
				assert_eq!(choices[2].0, "delete");
			}
			_ => panic!("Expected MultiSelect variant"),
		}
	}

	#[test]
	fn test_infer_admin_field_type_relationship() {
		use reinhardt_db::migrations::ForeignKeyAction;

		assert_eq!(
			infer_admin_field_type(&DbFieldType::OneToOne {
				to: "user".to_string(),
				on_delete: ForeignKeyAction::Cascade,
				on_update: ForeignKeyAction::Cascade,
			}),
			AdminFieldType::Hidden
		);
		assert_eq!(
			infer_admin_field_type(&DbFieldType::ManyToMany {
				to: "roles".to_string(),
				through: None,
			}),
			AdminFieldType::Hidden
		);
	}

	#[test]
	fn test_infer_admin_field_type_custom() {
		assert_eq!(
			infer_admin_field_type(&DbFieldType::Custom("geometry".to_string())),
			AdminFieldType::Text
		);
	}

	#[test]
	fn test_infer_admin_field_type_text_variants() {
		assert_eq!(
			infer_admin_field_type(&DbFieldType::TinyText),
			AdminFieldType::TextArea
		);
		assert_eq!(
			infer_admin_field_type(&DbFieldType::MediumText),
			AdminFieldType::TextArea
		);
	}

	#[test]
	fn test_infer_admin_field_type_integer_variants() {
		assert_eq!(
			infer_admin_field_type(&DbFieldType::TinyInt),
			AdminFieldType::Number
		);
		assert_eq!(
			infer_admin_field_type(&DbFieldType::MediumInt),
			AdminFieldType::Number
		);
	}

	// ──────────────────────────────────────────────────────────────
	// Additional filter type tests
	// ──────────────────────────────────────────────────────────────

	#[test]
	fn test_infer_filter_type_datetime() {
		let filter = infer_filter_type(&AdminFieldType::DateTime);
		match filter {
			FilterType::DateRange { ranges } => {
				assert!(!ranges.is_empty());
				assert!(ranges.iter().any(|r| r.value == "today"));
				assert!(ranges.iter().any(|r| r.value == "past_7_days"));
				assert!(ranges.iter().any(|r| r.value == "this_month"));
				assert!(ranges.iter().any(|r| r.value == "this_year"));
			}
			_ => panic!("Expected DateRange variant"),
		}
	}

	#[test]
	fn test_infer_filter_type_text() {
		let filter = infer_filter_type(&AdminFieldType::Text);
		match filter {
			FilterType::Choice { choices } => {
				assert_eq!(choices.len(), 3);
				assert!(choices.iter().any(|c| c.value == "all"));
				assert!(choices.iter().any(|c| c.value == "empty"));
				assert!(choices.iter().any(|c| c.value == "not_empty"));
			}
			_ => panic!("Expected Choice variant"),
		}
	}

	#[test]
	fn test_infer_filter_type_textarea() {
		let filter = infer_filter_type(&AdminFieldType::TextArea);
		match filter {
			FilterType::Choice { choices } => {
				assert_eq!(choices.len(), 3);
				assert_eq!(choices[0].label, "All");
				assert_eq!(choices[1].label, "Empty");
				assert_eq!(choices[2].label, "Not Empty");
			}
			_ => panic!("Expected Choice variant"),
		}
	}

	#[test]
	fn test_infer_filter_type_file() {
		let filter = infer_filter_type(&AdminFieldType::File);
		match filter {
			FilterType::Choice { choices } => {
				assert_eq!(choices.len(), 3);
			}
			_ => panic!("Expected Choice variant"),
		}
	}

	#[test]
	fn test_infer_filter_type_hidden() {
		let filter = infer_filter_type(&AdminFieldType::Hidden);
		match filter {
			FilterType::Choice { choices } => {
				assert!(!choices.is_empty());
			}
			_ => panic!("Expected Choice variant"),
		}
	}

	#[test]
	fn test_infer_filter_type_number_ranges() {
		let filter = infer_filter_type(&AdminFieldType::Number);
		match filter {
			FilterType::NumberRange { ranges } => {
				assert_eq!(ranges.len(), 3);
				assert!(ranges.iter().any(|r| r.value == "0" && r.label == "Zero"));
				assert!(
					ranges
						.iter()
						.any(|r| r.value == "positive" && r.label == "Positive")
				);
				assert!(
					ranges
						.iter()
						.any(|r| r.value == "negative" && r.label == "Negative")
				);
			}
			_ => panic!("Expected NumberRange variant"),
		}
	}

	// ──────────────────────────────────────────────────────────────
	// Required inference edge cases
	// ──────────────────────────────────────────────────────────────

	#[test]
	fn test_infer_required_null_false_explicit() {
		let meta = FieldMetadata::new(DbFieldType::Integer).with_nullable(false);
		assert!(infer_required(&meta));
	}

	#[test]
	fn test_infer_required_blank_false_explicit() {
		let meta = FieldMetadata::new(DbFieldType::Integer).with_param("blank", "false");
		assert!(infer_required(&meta));
	}

	#[test]
	fn test_infer_required_null_true_blank_false() {
		let meta = FieldMetadata::new(DbFieldType::Integer)
			.with_nullable(true)
			.with_param("blank", "false");
		assert!(!infer_required(&meta));
	}

	#[test]
	fn test_infer_required_null_false_blank_true() {
		let meta = FieldMetadata::new(DbFieldType::Integer)
			.with_nullable(false)
			.with_param("blank", "true");
		assert!(!infer_required(&meta));
	}

	#[test]
	fn test_infer_required_both_true() {
		let meta = FieldMetadata::new(DbFieldType::Integer)
			.with_nullable(true)
			.with_param("blank", "true");
		assert!(!infer_required(&meta));
	}

	// ──────────────────────────────────────────────────────────────
	// PostgreSQL-specific field type tests
	// ──────────────────────────────────────────────────────────────

	#[test]
	fn test_infer_admin_field_type_postgres_array_string() {
		// String array → MultiSelect
		let db_type = DbFieldType::Array(Box::new(DbFieldType::VarChar(255)));
		let admin_type = infer_admin_field_type(&db_type);
		assert!(matches!(admin_type, AdminFieldType::MultiSelect { .. }));
	}

	#[test]
	fn test_infer_admin_field_type_postgres_array_integer() {
		// Integer array → TextArea (complex array)
		let db_type = DbFieldType::Array(Box::new(DbFieldType::Integer));
		let admin_type = infer_admin_field_type(&db_type);
		assert_eq!(admin_type, AdminFieldType::TextArea);
	}

	#[test]
	fn test_infer_admin_field_type_postgres_hstore() {
		assert_eq!(
			infer_admin_field_type(&DbFieldType::HStore),
			AdminFieldType::TextArea
		);
	}

	#[test]
	fn test_infer_admin_field_type_postgres_citext() {
		assert_eq!(
			infer_admin_field_type(&DbFieldType::CIText),
			AdminFieldType::Text
		);
	}

	#[test]
	#[cfg(feature = "pgvector")]
	fn test_infer_admin_field_type_postgres_vector() {
		assert_eq!(
			infer_admin_field_type(&DbFieldType::Vector { dimensions: 1536 }),
			AdminFieldType::TextArea
		);
	}

	#[test]
	fn test_infer_admin_field_type_postgres_ranges() {
		assert_eq!(
			infer_admin_field_type(&DbFieldType::Int4Range),
			AdminFieldType::TextArea
		);
		assert_eq!(
			infer_admin_field_type(&DbFieldType::Int8Range),
			AdminFieldType::TextArea
		);
		assert_eq!(
			infer_admin_field_type(&DbFieldType::NumRange),
			AdminFieldType::TextArea
		);
		assert_eq!(
			infer_admin_field_type(&DbFieldType::DateRange),
			AdminFieldType::TextArea
		);
		assert_eq!(
			infer_admin_field_type(&DbFieldType::TsRange),
			AdminFieldType::TextArea
		);
		assert_eq!(
			infer_admin_field_type(&DbFieldType::TsTzRange),
			AdminFieldType::TextArea
		);
	}

	#[test]
	fn test_infer_admin_field_type_postgres_fulltext() {
		assert_eq!(
			infer_admin_field_type(&DbFieldType::TsVector),
			AdminFieldType::TextArea
		);
		assert_eq!(
			infer_admin_field_type(&DbFieldType::TsQuery),
			AdminFieldType::TextArea
		);
	}
}

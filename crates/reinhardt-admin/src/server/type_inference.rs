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

use crate::types::{FieldType as AdminFieldType, FilterChoice, FilterType};
use reinhardt_db::migrations::{
	FieldMetadata, FieldType as DbFieldType, ModelMetadata, global_registry,
};

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
///     .with_param("null", "true");
/// assert!(!infer_required(&meta));
/// ```
pub fn infer_required(meta: &FieldMetadata) -> bool {
	let is_null = meta
		.params
		.get("null")
		.map(|v| v == "true")
		.unwrap_or(false);
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
	find_model_by_table_name(table_name).and_then(|m| m.fields.get(field_name).cloned())
}

#[cfg(test)]
mod tests {
	use super::*;

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
		let meta = FieldMetadata::new(DbFieldType::VarChar(255)).with_param("null", "true");
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
			.with_param("null", "false")
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
		let meta = FieldMetadata::new(DbFieldType::Integer).with_param("null", "false");
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
			.with_param("null", "true")
			.with_param("blank", "false");
		assert!(!infer_required(&meta));
	}

	#[test]
	fn test_infer_required_null_false_blank_true() {
		let meta = FieldMetadata::new(DbFieldType::Integer)
			.with_param("null", "false")
			.with_param("blank", "true");
		assert!(!infer_required(&meta));
	}

	#[test]
	fn test_infer_required_both_true() {
		let meta = FieldMetadata::new(DbFieldType::Integer)
			.with_param("null", "true")
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

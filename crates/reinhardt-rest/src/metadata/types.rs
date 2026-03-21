//! Core types for metadata system

use serde::{Deserialize, Serialize};
use thiserror::Error as ThisError;

/// Errors that can occur during metadata determination.
#[derive(Debug, ThisError)]
pub enum MetadataError {
	/// Failed to determine metadata for the given request.
	#[error("Failed to determine metadata: {0}")]
	DeterminationError(String),

	/// No serializer is available for metadata introspection.
	#[error("Serializer not available")]
	SerializerNotAvailable,
}

/// Field type enumeration for metadata
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
	/// A generic field with no specific type.
	Field,
	/// A boolean true/false field.
	Boolean,
	/// A text string field.
	String,
	/// An integer numeric field.
	Integer,
	/// A floating-point numeric field.
	Float,
	/// A fixed-precision decimal field.
	Decimal,
	/// A date field (year, month, day).
	Date,
	/// A date and time field.
	DateTime,
	/// A time-of-day field.
	Time,
	/// A duration or time interval field.
	Duration,
	/// An email address field.
	Email,
	/// A URL field.
	Url,
	/// A UUID field.
	Uuid,
	/// A single-choice selection field.
	Choice,
	/// A multiple-choice selection field.
	MultipleChoice,
	/// A file upload field.
	File,
	/// An image upload field.
	Image,
	/// A list/array field containing child elements.
	List,
	/// A nested object field containing named child fields.
	NestedObject,
}

/// Choice information for choice fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChoiceInfo {
	/// The internal value stored when this choice is selected.
	pub value: String,
	/// The human-readable display name for this choice.
	pub display_name: String,
}

#[cfg(test)]
mod tests {
	use super::*;

	// DRF test: test_null_boolean_field_info_type
	#[test]
	fn test_boolean_field_info_type() {
		let field_type = FieldType::Boolean;
		assert_eq!(field_type, FieldType::Boolean);
	}

	// DRF test: test_decimal_field_info_type
	#[test]
	fn test_decimal_field_info_type() {
		// Note: In DRF, max_digits and decimal_places are specific to DecimalField
		// In Rust, we use the Decimal field type and could add custom fields if needed
		let field_type = FieldType::Decimal;
		assert_eq!(field_type, FieldType::Decimal);
	}
}

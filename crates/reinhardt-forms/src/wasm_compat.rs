//! WASM Compatibility Layer for Forms (Week 5 Day 1-2)
//!
//! This module provides serializable metadata structures that allow Django-style
//! Forms to be rendered on the client-side (WASM) without requiring the full
//! `Form` struct with its trait objects and non-serializable closures.
//!
//! ## Architecture
//!
//! The metadata extraction follows this pattern:
//!
//! ```mermaid
//! flowchart LR
//!     subgraph Server["Server-side"]
//!         Form["Form<br/>(traits, closures)"]
//!     end
//!
//!     subgraph Client["Client-side (WASM)"]
//!         FormMetadata["FormMetadata<br/>(plain data, serializable)"]
//!         FormComponent["FormComponent<br/>(WASM UI)"]
//!     end
//!
//!     Form -->|"to_metadata()"| FormMetadata
//!     FormMetadata --> FormComponent
//! ```
//!
//! ## Example
//!
//! ```
//! use reinhardt_forms::{Form, CharField, Field};
//! use reinhardt_forms::wasm_compat::{FormMetadata, FormExt};
//!
//! // Server-side: Create form
//! let mut form = Form::new();
//! form.add_field(Box::new(CharField::new("username".to_string())));
//!
//! // Extract metadata for client
//! let metadata: FormMetadata = form.to_metadata();
//!
//! // Serialize and send to WASM
//! let json = serde_json::to_string(&metadata).unwrap();
//! ```

use crate::field::Widget;
use crate::form::Form;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Validation rule types for client-side validation (Phase 2-A)
///
/// These rules enable client-side validation for better UX, while
/// server-side validation remains mandatory for security.
///
/// ## Security Note
///
/// Client-side validation is for UX enhancement only and MUST NOT
/// be relied upon for security. Server-side validation is always required.
///
/// ## Design Principle
///
/// All validation rules are declarative and type-safe. We do NOT use
/// JavaScript expressions to prevent arbitrary code execution vulnerabilities.
/// Instead, each rule type has specific parameters that are validated in Rust.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ValidationRule {
	/// Minimum length validation for string fields
	MinLength {
		/// Field name to validate
		field_name: String,
		/// Minimum required length
		min: usize,
		/// Error message to display when validation fails
		error_message: String,
	},
	/// Maximum length validation for string fields
	MaxLength {
		/// Field name to validate
		field_name: String,
		/// Maximum allowed length
		max: usize,
		/// Error message to display when validation fails
		error_message: String,
	},
	/// Regex pattern validation for string fields
	Pattern {
		/// Field name to validate
		field_name: String,
		/// Regex pattern to match (must be valid regex)
		pattern: String,
		/// Error message to display when validation fails
		error_message: String,
	},
	/// Minimum value validation for numeric fields
	MinValue {
		/// Field name to validate
		field_name: String,
		/// Minimum allowed value
		min: f64,
		/// Error message to display when validation fails
		error_message: String,
	},
	/// Maximum value validation for numeric fields
	MaxValue {
		/// Field name to validate
		field_name: String,
		/// Maximum allowed value
		max: f64,
		/// Error message to display when validation fails
		error_message: String,
	},
	/// Email format validation
	Email {
		/// Field name to validate
		field_name: String,
		/// Error message to display when validation fails
		error_message: String,
	},
	/// URL format validation
	Url {
		/// Field name to validate
		field_name: String,
		/// Error message to display when validation fails
		error_message: String,
	},
	/// Cross-field equality validation (e.g., password confirmation)
	FieldsEqual {
		/// Field names to compare for equality
		field_names: Vec<String>,
		/// Error message to display when validation fails
		error_message: String,
		/// Target field for error display (None = non-field error)
		target_field: Option<String>,
	},
	/// Date range validation (end_date >= start_date)
	DateRange {
		/// Start date field name
		start_field: String,
		/// End date field name
		end_field: String,
		/// Error message to display when validation fails
		error_message: String,
		/// Target field for error display
		target_field: Option<String>,
	},
	/// Numeric range validation (max >= min)
	NumericRange {
		/// Minimum value field name
		min_field: String,
		/// Maximum value field name
		max_field: String,
		/// Error message to display when validation fails
		error_message: String,
		/// Target field for error display
		target_field: Option<String>,
	},
	/// Reference to reinhardt-validators Validator
	ValidatorRef {
		/// Field name to validate
		field_name: String,
		/// Validator identifier (e.g., "email", "url", "min_length")
		validator_id: String,
		/// Validator parameters as JSON
		/// Example: {"min": 8, "max": 20} for MinMaxLengthValidator
		params: serde_json::Value,
		/// Error message to display when validation fails
		error_message: String,
	},
}

/// Serializable form metadata for client-side rendering (Week 5 Day 1)
///
/// This structure contains all information needed to render a form on the
/// client-side without requiring the full `Form` struct with its trait objects.
///
/// ## Fields
///
/// - `fields`: Metadata for each form field
/// - `initial`: Initial values for the form (form-level)
/// - `prefix`: Field name prefix (for multiple forms on same page)
/// - `is_bound`: Whether the form has been bound with data
/// - `errors`: Validation errors (if any)
/// - `validation_rules`: Client-side validation rules (Phase 2-A)
/// - `non_field_errors`: Form-level validation errors (Phase 2-A)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormMetadata {
	/// Field metadata list
	pub fields: Vec<FieldMetadata>,

	/// Initial values (form-level)
	pub initial: HashMap<String, serde_json::Value>,

	/// Field name prefix
	pub prefix: String,

	/// Whether the form has been bound with data
	pub is_bound: bool,

	/// Validation errors (field name -> error messages)
	pub errors: HashMap<String, Vec<String>>,

	/// Client-side validation rules (Phase 2-A)
	/// These rules enable immediate feedback to users without server round-trips.
	/// Server-side validation is still mandatory for security.
	#[serde(default)]
	pub validation_rules: Vec<ValidationRule>,

	/// Non-field errors (form-level errors) (Phase 2-A)
	/// These are errors that don't belong to a specific field (e.g., "Passwords don't match")
	#[serde(default)]
	pub non_field_errors: Vec<String>,
}

/// Serializable field metadata for client-side rendering (Week 5 Day 1)
///
/// This structure contains all information needed to render a single form field
/// on the client-side.
///
/// ## Fields
///
/// - `name`: Field name (used as form data key)
/// - `label`: Human-readable label (defaults to field name if None)
/// - `required`: Whether the field is required
/// - `help_text`: Help text displayed below the field
/// - `widget`: Widget type for rendering (TextInput, Select, etc.)
/// - `initial`: Initial value for this field
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldMetadata {
	/// Field name
	pub name: String,

	/// Human-readable label (optional)
	pub label: Option<String>,

	/// Whether the field is required
	pub required: bool,

	/// Help text (optional)
	pub help_text: Option<String>,

	/// Widget type for rendering
	pub widget: Widget,

	/// Initial value (optional)
	pub initial: Option<serde_json::Value>,
}

/// Extension trait for Form to extract metadata (Week 5 Day 1)
///
/// This trait provides the `to_metadata()` method that converts a `Form`
/// into a serializable `FormMetadata` structure.
pub trait FormExt {
	/// Extract serializable metadata from the form
	///
	/// This method creates a `FormMetadata` structure containing all
	/// information needed to render the form on the client-side.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::{Form, CharField, Field};
	/// use reinhardt_forms::wasm_compat::{FormMetadata, FormExt};
	///
	/// let mut form = Form::new();
	/// form.add_field(Box::new(CharField::new("email".to_string())));
	///
	/// let metadata: FormMetadata = form.to_metadata();
	/// assert_eq!(metadata.fields.len(), 1);
	/// assert_eq!(metadata.fields[0].name, "email");
	/// ```
	fn to_metadata(&self) -> FormMetadata;
}

impl FormExt for Form {
	fn to_metadata(&self) -> FormMetadata {
		// Extract field metadata
		let fields = self
			.fields()
			.iter()
			.map(|field| FieldMetadata {
				name: field.name().to_string(),
				label: field.label().map(|s| s.to_string()),
				required: field.required(),
				help_text: field.help_text().map(|s| s.to_string()),
				widget: field.widget().clone(),
				initial: field.initial().cloned(),
			})
			.collect();

		FormMetadata {
			fields,
			initial: self.initial().clone(),
			prefix: self.prefix().to_string(),
			is_bound: self.is_bound(),
			errors: self.errors().clone(),
			// Phase 2-A: Clone validation rules from Form
			validation_rules: self.validation_rules().to_vec(),
			non_field_errors: self
				.errors()
				.get(crate::form::ALL_FIELDS_KEY)
				.cloned()
				.unwrap_or_default(),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::fields::CharField;

	#[test]
	fn test_form_metadata_extraction() {
		let mut form = Form::new();
		form.add_field(Box::new(CharField::new("username".to_string())));
		form.add_field(Box::new(CharField::new("email".to_string())));

		let metadata = form.to_metadata();

		assert_eq!(metadata.fields.len(), 2);
		assert_eq!(metadata.fields[0].name, "username");
		assert_eq!(metadata.fields[1].name, "email");
		assert!(!metadata.is_bound);
	}

	#[test]
	fn test_form_metadata_with_prefix() {
		let mut form = Form::with_prefix("user".to_string());
		form.add_field(Box::new(CharField::new("name".to_string())));

		let metadata = form.to_metadata();

		assert_eq!(metadata.prefix, "user");
		assert_eq!(metadata.fields.len(), 1);
	}

	#[test]
	fn test_form_metadata_serialization() {
		let mut form = Form::new();
		form.add_field(Box::new(CharField::new("test".to_string())));

		let metadata = form.to_metadata();

		// Test JSON serialization
		let json = serde_json::to_string(&metadata).expect("Failed to serialize");
		assert!(json.contains("\"name\":\"test\""));

		// Test deserialization
		let deserialized: FormMetadata =
			serde_json::from_str(&json).expect("Failed to deserialize");
		assert_eq!(deserialized.fields[0].name, "test");
	}

	#[test]
	fn test_field_metadata_with_all_attributes() {
		use crate::fields::CharField;

		let field = CharField::new("bio".to_string())
			.with_label("Biography")
			.with_help_text("Tell us about yourself")
			.required();

		let mut form = Form::new();
		form.add_field(Box::new(field));

		let metadata = form.to_metadata();
		let field_meta = &metadata.fields[0];

		assert_eq!(field_meta.name, "bio");
		assert_eq!(field_meta.label, Some("Biography".to_string()));
		assert_eq!(
			field_meta.help_text,
			Some("Tell us about yourself".to_string())
		);
		assert!(field_meta.required);
	}

	#[test]
	fn test_form_metadata_with_initial_values() {
		use serde_json::json;

		let mut initial = HashMap::new();
		initial.insert("username".to_string(), json!("john_doe"));
		initial.insert("age".to_string(), json!(25));

		let mut form = Form::with_initial(initial);
		form.add_field(Box::new(CharField::new("username".to_string())));

		let metadata = form.to_metadata();

		assert_eq!(metadata.initial.get("username"), Some(&json!("john_doe")));
		assert_eq!(metadata.initial.get("age"), Some(&json!(25)));
	}

	#[test]
	fn test_form_metadata_with_errors() {
		use serde_json::json;

		let mut form = Form::new();
		// Create a required field - empty value should fail validation
		form.add_field(Box::new(CharField::new("email".to_string()).required()));

		// Bind with invalid data to generate errors (empty string for required field)
		let mut data = HashMap::new();
		data.insert("email".to_string(), json!("")); // Empty required field should fail
		form.bind(data);

		// Validate to populate errors
		let is_valid = form.is_valid();

		let metadata = form.to_metadata();

		// Should have validation error for the required email field
		assert!(!is_valid);
		assert!(!metadata.errors.is_empty());
		assert!(metadata.errors.contains_key("email"));
	}
}

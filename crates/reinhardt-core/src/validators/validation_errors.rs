//! Aggregate validation errors by field name

use std::collections::BTreeMap;
use std::fmt;

use super::errors::ValidationError;

/// Aggregates validation errors by field name.
///
/// Collects per-field [`ValidationError`]s and provides access
/// to the accumulated errors for structured error responses.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ValidationErrors {
	errors: BTreeMap<&'static str, Vec<ValidationError>>,
}

impl ValidationErrors {
	/// Create an empty error collection.
	pub fn new() -> Self {
		Self {
			errors: BTreeMap::new(),
		}
	}

	/// Add a validation error for a specific field.
	pub fn add(&mut self, field: &'static str, error: ValidationError) {
		self.errors.entry(field).or_default().push(error);
	}

	/// Get all field errors as a map.
	pub fn field_errors(&self) -> &BTreeMap<&'static str, Vec<ValidationError>> {
		&self.errors
	}

	/// Returns `true` if no errors have been added.
	pub fn is_empty(&self) -> bool {
		self.errors.is_empty()
	}
}

impl Default for ValidationErrors {
	fn default() -> Self {
		Self::new()
	}
}

impl fmt::Display for ValidationErrors {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let mut first = true;
		for (field, errors) in &self.errors {
			for error in errors {
				if !first {
					write!(f, ", ")?;
				}
				write!(f, "{}: {}", field, error)?;
				first = false;
			}
		}
		Ok(())
	}
}

impl std::error::Error for ValidationErrors {}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_new_is_empty() {
		let errors = ValidationErrors::new();
		assert!(errors.is_empty());
		assert!(errors.field_errors().is_empty());
	}

	#[test]
	fn test_add_single_error() {
		let mut errors = ValidationErrors::new();
		errors.add("email", ValidationError::InvalidEmail("bad".to_string()));
		assert!(!errors.is_empty());
		assert!(errors.field_errors().contains_key("email"));
		assert_eq!(errors.field_errors()["email"].len(), 1);
	}

	#[test]
	fn test_add_multiple_errors_same_field() {
		let mut errors = ValidationErrors::new();
		errors.add("name", ValidationError::TooShort { length: 0, min: 1 });
		errors.add(
			"name",
			ValidationError::TooLong {
				length: 200,
				max: 100,
			},
		);
		assert_eq!(errors.field_errors()["name"].len(), 2);
	}

	#[test]
	fn test_add_errors_different_fields() {
		let mut errors = ValidationErrors::new();
		errors.add("name", ValidationError::TooShort { length: 0, min: 1 });
		errors.add("email", ValidationError::InvalidEmail("bad".to_string()));
		assert_eq!(errors.field_errors().len(), 2);
	}

	#[test]
	fn test_display_format() {
		let mut errors = ValidationErrors::new();
		errors.add("email", ValidationError::InvalidEmail("bad".to_string()));
		let display = format!("{}", errors);
		assert!(display.contains("email"));
		assert!(display.contains("Invalid email"));
	}

	#[test]
	fn test_default_is_empty() {
		let errors = ValidationErrors::default();
		assert!(errors.is_empty());
	}
}

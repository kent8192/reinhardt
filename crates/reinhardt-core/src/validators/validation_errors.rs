//! Aggregate validation errors by field name

use std::borrow::Cow;
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
	errors: BTreeMap<Cow<'static, str>, Vec<ValidationError>>,
	#[cfg_attr(feature = "serde", serde(skip))]
	order: Vec<Cow<'static, str>>,
}

impl ValidationErrors {
	/// Create an empty error collection.
	pub fn new() -> Self {
		Self {
			errors: BTreeMap::new(),
			order: Vec::new(),
		}
	}

	/// Add a validation error for a specific field.
	pub fn add(&mut self, field: impl Into<Cow<'static, str>>, error: ValidationError) {
		let field = field.into();
		if !self.errors.contains_key(&field) {
			self.order.push(field.clone());
		}
		self.errors.entry(field).or_default().push(error);
	}

	/// Get all field errors as a map.
	pub fn field_errors(&self) -> &BTreeMap<Cow<'static, str>, Vec<ValidationError>> {
		&self.errors
	}

	/// Get field errors in first-insertion order, with non-field errors last.
	///
	/// **Parity: P2.** Native and WASM targets preserve the same insertion and
	/// non-field ordering semantics.
	pub fn ordered_field_errors(&self) -> impl Iterator<Item = (&str, &[ValidationError])> {
		self.order
			.iter()
			.filter(|field| field.as_ref() != "_all")
			.chain(self.order.iter().filter(|field| field.as_ref() == "_all"))
			.filter_map(|field| {
				self.errors
					.get(field)
					.map(|errors| (field.as_ref(), errors.as_slice()))
			})
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
	use rstest::rstest;

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

	#[rstest]
	fn ordered_field_errors_preserve_insertion_order_with_non_field_errors_last() {
		let mut errors = ValidationErrors::new();
		errors.add(
			"_all",
			ValidationError::Custom("Invalid combination".to_string()),
		);
		errors.add("name", ValidationError::TooShort { length: 0, min: 1 });
		errors.add(
			"api_url",
			ValidationError::InvalidUrl("invalid".to_string()),
		);

		let fields = errors
			.ordered_field_errors()
			.map(|(field, _)| field)
			.collect::<Vec<_>>();

		assert_eq!(fields, ["name", "api_url", "_all"]);
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

//! Protobuf message constraint validation
//!
//! This module provides validation utilities for protobuf messages
//! to enforce field constraints such as required fields, value ranges,
//! and string length limits.
//!
//! # Security
//!
//! Without validation, malformed or malicious protobuf messages may
//! bypass application-level constraints. This module provides a
//! [`ProtoValidator`] trait and [`ValidationRuleSet`] for declaring
//! and enforcing constraints on decoded messages.
//!
//! # Example
//!
//! ```rust
//! use reinhardt_grpc::validation::{ProtoValidator, ValidationError, FieldRule};
//!
//! struct MyRequest {
//!     name: String,
//!     count: i32,
//! }
//!
//! impl ProtoValidator for MyRequest {
//!     fn validate(&self) -> Result<(), ValidationError> {
//!         let mut errors = Vec::new();
//!
//!         if self.name.is_empty() {
//!             errors.push(FieldRule::required("name"));
//!         }
//!         if self.count < 0 || self.count > 1000 {
//!             errors.push(FieldRule::range("count", 0, 1000));
//!         }
//!
//!         if errors.is_empty() {
//!             Ok(())
//!         } else {
//!             Err(ValidationError::constraint_violations(errors))
//!         }
//!     }
//! }
//! ```

use std::borrow::Cow;

/// Trait for validating protobuf messages against declared constraints.
///
/// Implement this trait for protobuf message types that require
/// field-level validation beyond what the protobuf schema enforces.
pub trait ProtoValidator {
	/// Validate the message against its declared constraints.
	///
	/// Returns `Ok(())` if all constraints pass, or a [`ValidationError`]
	/// describing which fields failed validation.
	fn validate(&self) -> Result<(), ValidationError>;
}

/// A field-level validation rule violation.
///
/// Describes a single constraint that a field failed to satisfy.
/// Uses `Cow<'static, str>` to avoid heap allocations for static messages.
#[derive(Debug, Clone)]
pub struct FieldRule {
	/// The field name that violated the constraint.
	pub field: Cow<'static, str>,
	/// A description of the violated constraint.
	pub constraint: Cow<'static, str>,
}

impl FieldRule {
	/// Create a rule violation for a required field that is missing or empty.
	pub fn required(field: &str) -> Self {
		Self {
			field: Cow::Owned(field.to_string()),
			constraint: Cow::Borrowed("field is required"),
		}
	}

	/// Create a rule violation for a numeric field outside its allowed range.
	pub fn range(field: &str, min: i64, max: i64) -> Self {
		Self {
			field: Cow::Owned(field.to_string()),
			constraint: Cow::Owned(format!("value must be between {min} and {max}")),
		}
	}

	/// Create a rule violation for a string field exceeding its maximum length.
	pub fn max_length(field: &str, max: usize) -> Self {
		Self {
			field: Cow::Owned(field.to_string()),
			constraint: Cow::Owned(format!("length must not exceed {max}")),
		}
	}

	/// Create a rule violation for a string field below its minimum length.
	pub fn min_length(field: &str, min: usize) -> Self {
		Self {
			field: Cow::Owned(field.to_string()),
			constraint: Cow::Owned(format!("length must be at least {min}")),
		}
	}

	/// Create a rule violation for a repeated field exceeding its maximum count.
	pub fn max_items(field: &str, max: usize) -> Self {
		Self {
			field: Cow::Owned(field.to_string()),
			constraint: Cow::Owned(format!("number of items must not exceed {max}")),
		}
	}

	/// Create a custom rule violation.
	pub fn custom(field: &str, constraint: &str) -> Self {
		Self {
			field: Cow::Owned(field.to_string()),
			constraint: Cow::Owned(constraint.to_string()),
		}
	}
}

impl std::fmt::Display for FieldRule {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}: {}", self.field, self.constraint)
	}
}

/// Error type for protobuf constraint validation failures.
///
/// Contains one or more field-level violations that describe
/// which constraints were not satisfied.
#[derive(Debug, thiserror::Error)]
#[error("protobuf validation failed: {}", format_violations(&self.violations))]
pub struct ValidationError {
	violations: Vec<FieldRule>,
}

impl ValidationError {
	/// Create a validation error from a list of constraint violations.
	pub fn constraint_violations(violations: Vec<FieldRule>) -> Self {
		Self { violations }
	}

	/// Returns the list of field constraint violations.
	pub fn violations(&self) -> &[FieldRule] {
		&self.violations
	}

	/// Convert this validation error into a `tonic::Status` with
	/// `InvalidArgument` code.
	///
	/// The error message includes field names and constraint descriptions
	/// so clients can fix their requests.
	pub fn into_status(self) -> tonic::Status {
		let message = format_violations(&self.violations);
		tonic::Status::invalid_argument(message)
	}
}

/// Format violation list into a human-readable string.
fn format_violations(violations: &[FieldRule]) -> String {
	violations
		.iter()
		.map(|v| v.to_string())
		.collect::<Vec<_>>()
		.join("; ")
}

/// A rule set builder for declaring validation constraints on fields.
///
/// Provides a fluent API for building validation logic.
///
/// # Example
///
/// ```rust
/// use reinhardt_grpc::validation::ValidationRuleSet;
///
/// let result = ValidationRuleSet::new()
///     .require_non_empty("name", "Alice")
///     .require_range("count", 5, 0, 100)
///     .require_max_length("description", "hello", 1000)
///     .validate();
///
/// assert!(result.is_ok());
///
/// let result = ValidationRuleSet::new()
///     .require_non_empty("name", "")
///     .validate();
///
/// assert!(result.is_err());
/// ```
#[derive(Debug, Default)]
pub struct ValidationRuleSet {
	violations: Vec<FieldRule>,
}

impl ValidationRuleSet {
	/// Create a new empty rule set.
	pub fn new() -> Self {
		Self::default()
	}

	/// Add a required-non-empty check for a string field.
	///
	/// If the value is empty, a violation is recorded.
	pub fn require_non_empty(mut self, field: &str, value: &str) -> Self {
		if value.is_empty() {
			self.violations.push(FieldRule::required(field));
		}
		self
	}

	/// Add a numeric range check for a field.
	///
	/// If the value is outside `[min, max]`, a violation is recorded.
	pub fn require_range(mut self, field: &str, value: i64, min: i64, max: i64) -> Self {
		if value < min || value > max {
			self.violations.push(FieldRule::range(field, min, max));
		}
		self
	}

	/// Add a maximum length check for a string field.
	///
	/// If the string length exceeds `max`, a violation is recorded.
	pub fn require_max_length(mut self, field: &str, value: &str, max: usize) -> Self {
		if value.len() > max {
			self.violations.push(FieldRule::max_length(field, max));
		}
		self
	}

	/// Add a minimum length check for a string field.
	///
	/// If the string length is below `min`, a violation is recorded.
	pub fn require_min_length(mut self, field: &str, value: &str, min: usize) -> Self {
		if value.len() < min {
			self.violations.push(FieldRule::min_length(field, min));
		}
		self
	}

	/// Add a maximum items check for a repeated field.
	///
	/// If the count exceeds `max`, a violation is recorded.
	pub fn require_max_items(mut self, field: &str, count: usize, max: usize) -> Self {
		if count > max {
			self.violations.push(FieldRule::max_items(field, max));
		}
		self
	}

	/// Add a custom constraint check.
	///
	/// If the `condition` is `false`, the violation is recorded.
	pub fn require(mut self, field: &str, constraint: &str, condition: bool) -> Self {
		if !condition {
			self.violations.push(FieldRule::custom(field, constraint));
		}
		self
	}

	/// Consume the rule set and return a validation result.
	///
	/// Returns `Ok(())` if no violations were recorded, or
	/// `Err(ValidationError)` with all recorded violations.
	pub fn validate(self) -> Result<(), ValidationError> {
		if self.violations.is_empty() {
			Ok(())
		} else {
			Err(ValidationError::constraint_violations(self.violations))
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn field_rule_required_display() {
		// Arrange
		let rule = FieldRule::required("name");

		// Act
		let display = rule.to_string();

		// Assert
		assert_eq!(display, "name: field is required");
	}

	#[rstest]
	fn field_rule_range_display() {
		// Arrange
		let rule = FieldRule::range("count", 0, 100);

		// Act
		let display = rule.to_string();

		// Assert
		assert_eq!(display, "count: value must be between 0 and 100");
	}

	#[rstest]
	fn field_rule_max_length_display() {
		// Arrange
		let rule = FieldRule::max_length("description", 255);

		// Act
		let display = rule.to_string();

		// Assert
		assert_eq!(display, "description: length must not exceed 255");
	}

	#[rstest]
	fn field_rule_min_length_display() {
		// Arrange
		let rule = FieldRule::min_length("password", 8);

		// Act
		let display = rule.to_string();

		// Assert
		assert_eq!(display, "password: length must be at least 8");
	}

	#[rstest]
	fn field_rule_max_items_display() {
		// Arrange
		let rule = FieldRule::max_items("errors", 50);

		// Act
		let display = rule.to_string();

		// Assert
		assert_eq!(display, "errors: number of items must not exceed 50");
	}

	#[rstest]
	fn field_rule_custom_display() {
		// Arrange
		let rule = FieldRule::custom("email", "must be a valid email address");

		// Act
		let display = rule.to_string();

		// Assert
		assert_eq!(display, "email: must be a valid email address");
	}

	#[rstest]
	fn validation_error_single_violation() {
		// Arrange
		let error = ValidationError::constraint_violations(vec![FieldRule::required("name")]);

		// Act
		let message = error.to_string();
		let violations = error.violations();

		// Assert
		assert_eq!(
			message,
			"protobuf validation failed: name: field is required"
		);
		assert_eq!(violations.len(), 1);
	}

	#[rstest]
	fn validation_error_multiple_violations() {
		// Arrange
		let error = ValidationError::constraint_violations(vec![
			FieldRule::required("name"),
			FieldRule::range("count", 0, 100),
		]);

		// Act
		let message = error.to_string();

		// Assert
		assert_eq!(
			message,
			"protobuf validation failed: name: field is required; count: value must be between 0 and 100"
		);
	}

	#[rstest]
	fn validation_error_into_status() {
		// Arrange
		let error = ValidationError::constraint_violations(vec![FieldRule::required("query")]);

		// Act
		let status = error.into_status();

		// Assert
		assert_eq!(status.code(), tonic::Code::InvalidArgument);
		assert!(status.message().contains("query: field is required"));
	}

	#[rstest]
	fn rule_set_passes_when_all_valid() {
		// Arrange & Act
		let result = ValidationRuleSet::new()
			.require_non_empty("name", "Alice")
			.require_range("age", 25, 0, 150)
			.require_max_length("bio", "Short bio", 1000)
			.validate();

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn rule_set_fails_on_empty_required() {
		// Arrange & Act
		let result = ValidationRuleSet::new()
			.require_non_empty("name", "")
			.validate();

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert_eq!(err.violations().len(), 1);
		assert_eq!(err.violations()[0].field, "name");
	}

	#[rstest]
	fn rule_set_fails_on_out_of_range() {
		// Arrange & Act
		let result = ValidationRuleSet::new()
			.require_range("page", -1, 0, 1000)
			.validate();

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert_eq!(err.violations()[0].field, "page");
	}

	#[rstest]
	fn rule_set_fails_on_excessive_length() {
		// Arrange
		let long_string = "x".repeat(300);

		// Act
		let result = ValidationRuleSet::new()
			.require_max_length("name", &long_string, 255)
			.validate();

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn rule_set_fails_on_insufficient_length() {
		// Arrange & Act
		let result = ValidationRuleSet::new()
			.require_min_length("password", "abc", 8)
			.validate();

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn rule_set_fails_on_excessive_items() {
		// Arrange & Act
		let result = ValidationRuleSet::new()
			.require_max_items("errors", 100, 50)
			.validate();

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn rule_set_custom_constraint() {
		// Arrange & Act
		let result = ValidationRuleSet::new()
			.require("email", "must contain @", "invalid".contains('@'))
			.validate();

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert_eq!(err.violations()[0].field, "email");
		assert_eq!(err.violations()[0].constraint, "must contain @");
	}

	#[rstest]
	fn rule_set_collects_multiple_violations() {
		// Arrange & Act
		let result = ValidationRuleSet::new()
			.require_non_empty("name", "")
			.require_range("page", -1, 0, 100)
			.require_max_length("query", &"x".repeat(10000), 1000)
			.validate();

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert_eq!(err.violations().len(), 3);
	}

	#[rstest]
	fn proto_validator_trait_implementation() {
		// Arrange
		struct TestMessage {
			name: String,
			count: i32,
		}

		impl ProtoValidator for TestMessage {
			fn validate(&self) -> Result<(), ValidationError> {
				ValidationRuleSet::new()
					.require_non_empty("name", &self.name)
					.require_range("count", self.count as i64, 0, 1000)
					.validate()
			}
		}

		let valid = TestMessage {
			name: "test".to_string(),
			count: 42,
		};
		let invalid = TestMessage {
			name: String::new(),
			count: -1,
		};

		// Act & Assert
		assert!(valid.validate().is_ok());
		assert!(invalid.validate().is_err());
		assert_eq!(invalid.validate().unwrap_err().violations().len(), 2);
	}
}

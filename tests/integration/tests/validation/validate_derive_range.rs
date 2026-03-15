//! Integration tests for `#[validate(range(min, max))]` derive macro support

use reinhardt_core::validators::Validate;
use rstest::rstest;

// Struct with integer range validation (min and max)
#[derive(Validate)]
struct IntegerRange {
	#[validate(range(min = 0, max = 100))]
	pub value: i32,
}

// Struct with float range validation
#[derive(Validate)]
struct FloatRange {
	#[validate(range(min = 0.0, max = 1.0))]
	pub value: f64,
}

// Struct with min-only range validation
#[derive(Validate)]
struct MinOnly {
	#[validate(range(min = 0))]
	pub value: i32,
}

// Struct with max-only range validation
#[derive(Validate)]
struct MaxOnly {
	#[validate(range(max = 100))]
	pub value: i32,
}

// Struct with custom error message
#[derive(Validate)]
struct CustomMessage {
	#[validate(range(min = 5, max = 10, message = "value must be between 5 and 10"))]
	pub value: i32,
}

// Struct with optional range field
#[derive(Validate)]
struct OptionalRange {
	#[validate(range(min = 0, max = 100))]
	pub value: Option<i32>,
}

// ---------------------------------------------------------------------------
// Integer range tests
// ---------------------------------------------------------------------------

#[rstest]
fn integer_range_accepts_value_within_range() {
	// Arrange
	let item = IntegerRange { value: 50 };

	// Act
	let result = item.validate();

	// Assert
	assert!(result.is_ok());
}

#[rstest]
fn integer_range_accepts_min_boundary() {
	// Arrange
	let item = IntegerRange { value: 0 };

	// Act
	let result = item.validate();

	// Assert
	assert!(result.is_ok());
}

#[rstest]
fn integer_range_accepts_max_boundary() {
	// Arrange
	let item = IntegerRange { value: 100 };

	// Act
	let result = item.validate();

	// Assert
	assert!(result.is_ok());
}

#[rstest]
fn integer_range_rejects_below_min() {
	// Arrange
	let item = IntegerRange { value: -1 };

	// Act
	let result = item.validate();

	// Assert
	assert!(result.is_err());
	let errors = result.unwrap_err();
	assert!(errors.field_errors().contains_key("value"));
}

#[rstest]
fn integer_range_rejects_above_max() {
	// Arrange
	let item = IntegerRange { value: 101 };

	// Act
	let result = item.validate();

	// Assert
	assert!(result.is_err());
	let errors = result.unwrap_err();
	assert!(errors.field_errors().contains_key("value"));
}

// ---------------------------------------------------------------------------
// Float range tests
// ---------------------------------------------------------------------------

#[rstest]
fn float_range_accepts_value_within_range() {
	// Arrange
	let item = FloatRange { value: 0.5 };

	// Act
	let result = item.validate();

	// Assert
	assert!(result.is_ok());
}

#[rstest]
fn float_range_rejects_below_min() {
	// Arrange
	let item = FloatRange { value: -0.1 };

	// Act
	let result = item.validate();

	// Assert
	assert!(result.is_err());
	let errors = result.unwrap_err();
	assert!(errors.field_errors().contains_key("value"));
}

#[rstest]
fn float_range_rejects_above_max() {
	// Arrange
	let item = FloatRange { value: 1.1 };

	// Act
	let result = item.validate();

	// Assert
	assert!(result.is_err());
	let errors = result.unwrap_err();
	assert!(errors.field_errors().contains_key("value"));
}

// ---------------------------------------------------------------------------
// Min-only range tests
// ---------------------------------------------------------------------------

#[rstest]
fn min_only_accepts_value_at_min() {
	// Arrange
	let item = MinOnly { value: 0 };

	// Act
	let result = item.validate();

	// Assert
	assert!(result.is_ok());
}

#[rstest]
fn min_only_accepts_value_above_min() {
	// Arrange
	let item = MinOnly { value: 1000 };

	// Act
	let result = item.validate();

	// Assert
	assert!(result.is_ok());
}

#[rstest]
fn min_only_rejects_value_below_min() {
	// Arrange
	let item = MinOnly { value: -1 };

	// Act
	let result = item.validate();

	// Assert
	assert!(result.is_err());
	let errors = result.unwrap_err();
	assert!(errors.field_errors().contains_key("value"));
}

// ---------------------------------------------------------------------------
// Max-only range tests
// ---------------------------------------------------------------------------

#[rstest]
fn max_only_accepts_value_at_max() {
	// Arrange
	let item = MaxOnly { value: 100 };

	// Act
	let result = item.validate();

	// Assert
	assert!(result.is_ok());
}

#[rstest]
fn max_only_rejects_value_above_max() {
	// Arrange
	let item = MaxOnly { value: 101 };

	// Act
	let result = item.validate();

	// Assert
	assert!(result.is_err());
	let errors = result.unwrap_err();
	assert!(errors.field_errors().contains_key("value"));
}

// ---------------------------------------------------------------------------
// Custom message tests
// ---------------------------------------------------------------------------

#[rstest]
fn custom_message_uses_custom_error_text() {
	// Arrange
	let item = CustomMessage { value: 20 };

	// Act
	let result = item.validate();

	// Assert
	assert!(result.is_err());
	let errors = result.unwrap_err();
	let field_errors = errors.field_errors();
	let value_errors = field_errors
		.get("value")
		.expect("expected 'value' field error");
	assert_eq!(value_errors.len(), 1);
	assert_eq!(
		value_errors[0].to_string(),
		"Custom validation error: value must be between 5 and 10"
	);
}

// ---------------------------------------------------------------------------
// Optional field tests
// ---------------------------------------------------------------------------

#[rstest]
fn optional_range_accepts_none() {
	// Arrange
	let item = OptionalRange { value: None };

	// Act
	let result = item.validate();

	// Assert
	assert!(result.is_ok());
}

#[rstest]
fn optional_range_validates_some_in_range() {
	// Arrange
	let item = OptionalRange { value: Some(50) };

	// Act
	let result = item.validate();

	// Assert
	assert!(result.is_ok());
}

#[rstest]
fn optional_range_rejects_some_out_of_range() {
	// Arrange
	let item = OptionalRange { value: Some(101) };

	// Act
	let result = item.validate();

	// Assert
	assert!(result.is_err());
	let errors = result.unwrap_err();
	assert!(errors.field_errors().contains_key("value"));
}

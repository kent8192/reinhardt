//! Integration tests for serializers module
//!
//! Tests field validation, JSON serialization/deserialization, and validator
//! interactions across the serializers sub-modules.

use chrono::{Datelike, Timelike};
use reinhardt_core::serializers::fields::{
	BooleanField, CharField, ChoiceField, DateField, DateTimeField, EmailField, FieldError,
	FloatField, IntegerField, URLField,
};
use reinhardt_core::serializers::{
	FieldValidator, JsonSerializer, Serializer, SerializerError, ValidationError, ValidationResult,
	validate_fields,
};
use rstest::rstest;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// CharField validation
// ---------------------------------------------------------------------------

#[rstest]
fn char_field_validates_valid_string_within_bounds() {
	// Arrange
	let field = CharField::new().min_length(3).max_length(10);

	// Act
	let result = field.validate("hello");

	// Assert
	assert!(result.is_ok());
}

#[rstest]
fn char_field_rejects_string_below_min_length() {
	// Arrange
	let field = CharField::new().min_length(5);

	// Act
	let result = field.validate("hi");

	// Assert
	assert_eq!(result, Err(FieldError::TooShort(5)));
}

#[rstest]
fn char_field_rejects_string_above_max_length() {
	// Arrange
	let field = CharField::new().max_length(5);

	// Act
	let result = field.validate("hello world");

	// Assert
	assert_eq!(result, Err(FieldError::TooLong(5)));
}

#[rstest]
fn char_field_accepts_string_at_exact_min_length() {
	// Arrange
	let field = CharField::new().min_length(5);

	// Act
	let result = field.validate("hello");

	// Assert
	assert!(result.is_ok());
}

#[rstest]
fn char_field_accepts_string_at_exact_max_length() {
	// Arrange
	let field = CharField::new().max_length(5);

	// Act
	let result = field.validate("hello");

	// Assert
	assert!(result.is_ok());
}

#[rstest]
fn char_field_rejects_empty_string_when_blank_not_allowed() {
	// Arrange
	let field = CharField::new();

	// Act
	let result = field.validate("");

	// Assert
	assert_eq!(result, Err(FieldError::Required));
}

#[rstest]
fn char_field_accepts_empty_string_when_blank_allowed() {
	// Arrange
	let field = CharField::new().allow_blank(true);

	// Act
	let result = field.validate("");

	// Assert
	assert!(result.is_ok());
}

#[rstest]
fn char_field_default_value_is_stored() {
	// Act
	let field = CharField::new().default("fallback".into());

	// Assert
	assert_eq!(field.default, Some("fallback".into()));
}

// ---------------------------------------------------------------------------
// IntegerField validation
// ---------------------------------------------------------------------------

#[rstest]
fn integer_field_validates_value_within_range() {
	// Arrange
	let field = IntegerField::new().min_value(0).max_value(100);

	// Act
	let result = field.validate(50);

	// Assert
	assert!(result.is_ok());
}

#[rstest]
fn integer_field_rejects_value_below_min() {
	// Arrange
	let field = IntegerField::new().min_value(0);

	// Act
	let result = field.validate(-1);

	// Assert
	assert_eq!(result, Err(FieldError::TooSmall(0)));
}

#[rstest]
fn integer_field_rejects_value_above_max() {
	// Arrange
	let field = IntegerField::new().max_value(100);

	// Act
	let result = field.validate(101);

	// Assert
	assert_eq!(result, Err(FieldError::TooLarge(100)));
}

#[rstest]
fn integer_field_accepts_value_at_exact_min() {
	// Arrange
	let field = IntegerField::new().min_value(0);

	// Act
	let result = field.validate(0);

	// Assert
	assert!(result.is_ok());
}

#[rstest]
fn integer_field_accepts_value_at_exact_max() {
	// Arrange
	let field = IntegerField::new().max_value(100);

	// Act
	let result = field.validate(100);

	// Assert
	assert!(result.is_ok());
}

#[rstest]
fn integer_field_accepts_any_value_without_constraints() {
	// Arrange
	let field = IntegerField::new();

	// Act
	let min = field.validate(i64::MIN);
	let zero = field.validate(0);
	let max = field.validate(i64::MAX);

	// Assert
	assert!(min.is_ok());
	assert!(zero.is_ok());
	assert!(max.is_ok());
}

// ---------------------------------------------------------------------------
// FloatField validation
// ---------------------------------------------------------------------------

#[rstest]
fn float_field_validates_value_within_range() {
	// Arrange
	let field = FloatField::new().min_value(0.0).max_value(1.0);

	// Act
	let result = field.validate(0.5);

	// Assert
	assert!(result.is_ok());
}

#[rstest]
fn float_field_rejects_value_below_min() {
	// Arrange
	let field = FloatField::new().min_value(0.0);

	// Act
	let result = field.validate(-0.1);

	// Assert
	assert_eq!(result, Err(FieldError::TooSmallFloat(0.0)));
}

#[rstest]
fn float_field_rejects_value_above_max() {
	// Arrange
	let field = FloatField::new().max_value(1.0);

	// Act
	let result = field.validate(1.1);

	// Assert
	assert_eq!(result, Err(FieldError::TooLargeFloat(1.0)));
}

#[rstest]
fn float_field_accepts_value_at_exact_boundary() {
	// Arrange
	let field = FloatField::new().min_value(0.0).max_value(1.0);

	// Act
	let at_min = field.validate(0.0);
	let at_max = field.validate(1.0);

	// Assert
	assert!(at_min.is_ok());
	assert!(at_max.is_ok());
}

// ---------------------------------------------------------------------------
// FieldError Display messages
// ---------------------------------------------------------------------------

#[rstest]
#[case::required(FieldError::Required, "This field is required")]
#[case::null(FieldError::Null, "This field may not be null")]
#[case::too_short(FieldError::TooShort(3), "String is too short (min: 3)")]
#[case::too_long(FieldError::TooLong(10), "String is too long (max: 10)")]
#[case::too_small(FieldError::TooSmall(0), "Value is too small (min: 0)")]
#[case::too_large(FieldError::TooLarge(100), "Value is too large (max: 100)")]
#[case::too_small_float(FieldError::TooSmallFloat(0.5), "Value is too small (min: 0.5)")]
#[case::too_large_float(FieldError::TooLargeFloat(9.9), "Value is too large (max: 9.9)")]
#[case::invalid_email(FieldError::InvalidEmail, "Enter a valid email address")]
#[case::invalid_url(FieldError::InvalidUrl, "Enter a valid URL")]
#[case::invalid_choice(FieldError::InvalidChoice, "Invalid choice")]
#[case::invalid_date(FieldError::InvalidDate, "Invalid date format")]
#[case::invalid_datetime(FieldError::InvalidDateTime, "Invalid datetime format")]
#[case::custom(FieldError::Custom("oops".to_string()), "oops")]
fn field_error_display_contains_expected_message(
	#[case] error: FieldError,
	#[case] expected: &str,
) {
	// Act
	let message = error.to_string();

	// Assert
	assert_eq!(message, expected);
}

// ---------------------------------------------------------------------------
// JsonSerializer roundtrip
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct TestProduct {
	id: i64,
	name: String,
	price: f64,
	in_stock: bool,
}

#[rstest]
fn json_serializer_roundtrip_preserves_data() {
	// Arrange
	let product = TestProduct {
		id: 42,
		name: "Widget".into(),
		price: 9.99,
		in_stock: true,
	};
	let serializer = JsonSerializer::<TestProduct>::new();

	// Act
	let json = serializer.serialize(&product).unwrap();
	let restored = serializer.deserialize(&json).unwrap();

	// Assert
	assert_eq!(product, restored);
}

#[rstest]
fn json_serializer_serialize_produces_valid_json() {
	// Arrange
	let product = TestProduct {
		id: 1,
		name: "Gadget".into(),
		price: 19.95,
		in_stock: false,
	};
	let serializer = JsonSerializer::<TestProduct>::new();

	// Act
	let json = serializer.serialize(&product).unwrap();

	// Assert
	assert!(json.contains("\"id\":1"));
	assert!(json.contains("Gadget"));
	assert!(json.contains("\"in_stock\":false"));
}

#[rstest]
fn json_serializer_deserialize_rejects_invalid_json() {
	// Arrange
	let invalid = "{not valid json}".into();
	let serializer = JsonSerializer::<TestProduct>::new();

	// Act
	let result = serializer.deserialize(&invalid);

	// Assert
	assert!(result.is_err());
	if let Err(SerializerError::Serde { message }) = result {
		assert!(message.contains("Deserialization error"));
	} else {
		panic!("Expected SerializerError::Serde");
	}
}

// ---------------------------------------------------------------------------
// Combined field validation scenario
// ---------------------------------------------------------------------------

#[rstest]
fn combined_char_and_integer_field_validation_scenario() {
	// Arrange
	let name_field = CharField::new().min_length(2).max_length(50);
	let age_field = IntegerField::new().min_value(0).max_value(150);

	// Act - valid data
	let name_result = name_field.validate("Alice");
	let age_result = age_field.validate(30);

	// Assert
	assert!(name_result.is_ok());
	assert!(age_result.is_ok());

	// Act - invalid data
	let name_result = name_field.validate("A");
	let age_result = age_field.validate(-5);

	// Assert
	assert_eq!(name_result, Err(FieldError::TooShort(2)));
	assert_eq!(age_result, Err(FieldError::TooSmall(0)));
}

// ---------------------------------------------------------------------------
// EmailField validation
// ---------------------------------------------------------------------------

#[rstest]
#[case::standard_email("user@example.com", true)]
#[case::subdomain_email("admin@mail.example.org", true)]
#[case::missing_at("invalid-email", false)]
#[case::missing_local("@example.com", false)]
#[case::missing_domain("user@", false)]
#[case::missing_tld("user@localhost", false)]
fn email_field_validates_format(#[case] input: &str, #[case] should_pass: bool) {
	// Arrange
	let field = EmailField::new();

	// Act
	let result = field.validate(input);

	// Assert
	assert_eq!(result.is_ok(), should_pass);
}

#[rstest]
fn email_field_rejects_empty_when_required() {
	// Arrange
	let field = EmailField::new();

	// Act
	let result = field.validate("");

	// Assert
	assert_eq!(result, Err(FieldError::Required));
}

#[rstest]
fn email_field_allows_empty_when_blank_allowed() {
	// Arrange
	let field = EmailField::new().allow_blank(true);

	// Act
	let result = field.validate("");

	// Assert
	assert!(result.is_ok());
}

// ---------------------------------------------------------------------------
// DateField validation
// ---------------------------------------------------------------------------

#[rstest]
fn date_field_parses_valid_iso_date() {
	// Arrange
	let field = DateField::new();

	// Act
	let date = field.parse("2024-06-15").unwrap();

	// Assert
	assert_eq!(date.year(), 2024);
	assert_eq!(date.month(), 6);
	assert_eq!(date.day(), 15);
}

#[rstest]
fn date_field_rejects_invalid_date_string() {
	// Arrange
	let field = DateField::new();

	// Act
	let result = field.validate("not-a-date");

	// Assert
	assert_eq!(result, Err(FieldError::InvalidDate));
}

#[rstest]
fn date_field_supports_custom_format() {
	// Arrange
	let field = DateField::new().format("%d/%m/%Y");

	// Act
	let date = field.parse("25/12/2025").unwrap();

	// Assert
	assert_eq!(date.year(), 2025);
	assert_eq!(date.month(), 12);
	assert_eq!(date.day(), 25);
}

#[rstest]
fn date_field_rejects_empty_when_required() {
	// Arrange
	let field = DateField::new();

	// Act
	let result = field.parse("");

	// Assert
	assert_eq!(result, Err(FieldError::Required));
}

// ---------------------------------------------------------------------------
// DateTimeField validation
// ---------------------------------------------------------------------------

#[rstest]
fn datetime_field_parses_valid_iso_datetime() {
	// Arrange
	let field = DateTimeField::new();

	// Act
	let dt = field.parse("2024-06-15 10:30:45").unwrap();

	// Assert
	assert_eq!(dt.year(), 2024);
	assert_eq!(dt.month(), 6);
	assert_eq!(dt.hour(), 10);
	assert_eq!(dt.minute(), 30);
	assert_eq!(dt.second(), 45);
}

#[rstest]
fn datetime_field_rejects_invalid_datetime_string() {
	// Arrange
	let field = DateTimeField::new();

	// Act
	let result = field.validate("not-a-datetime");

	// Assert
	assert_eq!(result, Err(FieldError::InvalidDateTime));
}

// ---------------------------------------------------------------------------
// ChoiceField validation
// ---------------------------------------------------------------------------

#[rstest]
fn choice_field_accepts_valid_choice() {
	// Arrange
	let field = ChoiceField::new(vec!["small".into(), "medium".into(), "large".into()]);

	// Act
	let result = field.validate("medium");

	// Assert
	assert!(result.is_ok());
}

#[rstest]
fn choice_field_rejects_invalid_choice() {
	// Arrange
	let field = ChoiceField::new(vec!["red".into(), "green".into()]);

	// Act
	let result = field.validate("blue");

	// Assert
	assert_eq!(result, Err(FieldError::InvalidChoice));
}

#[rstest]
fn choice_field_rejects_empty_when_blank_not_allowed() {
	// Arrange
	let field = ChoiceField::new(vec!["a".into()]);

	// Act
	let result = field.validate("");

	// Assert
	assert_eq!(result, Err(FieldError::Required));
}

#[rstest]
fn choice_field_allows_empty_when_blank_allowed() {
	// Arrange
	let field = ChoiceField::new(vec!["a".into()]).allow_blank(true);

	// Act
	let result = field.validate("");

	// Assert
	assert!(result.is_ok());
}

// ---------------------------------------------------------------------------
// BooleanField validation
// ---------------------------------------------------------------------------

#[rstest]
#[case::true_value(true)]
#[case::false_value(false)]
fn boolean_field_accepts_all_booleans(#[case] value: bool) {
	// Arrange
	let field = BooleanField::new();

	// Act
	let result = field.validate(value);

	// Assert
	assert!(result.is_ok());
}

// ---------------------------------------------------------------------------
// URLField validation
// ---------------------------------------------------------------------------

#[rstest]
#[case::https("https://example.com", true)]
#[case::http("http://localhost:8000", true)]
#[case::no_protocol("example.com", false)]
#[case::ftp("ftp://files.example.com", false)]
fn url_field_validates_protocol(#[case] input: &str, #[case] should_pass: bool) {
	// Arrange
	let field = URLField::new();

	// Act
	let result = field.validate(input);

	// Assert
	assert_eq!(result.is_ok(), should_pass);
}

// ---------------------------------------------------------------------------
// Default values and required/optional behavior
// ---------------------------------------------------------------------------

#[rstest]
fn integer_field_stores_default_value() {
	// Act
	let field = IntegerField::new().default(42);

	// Assert
	assert_eq!(field.default, Some(42));
}

#[rstest]
fn float_field_stores_default_value() {
	// Act
	let field = FloatField::new().default(2.78);

	// Assert
	assert_eq!(field.default, Some(2.78));
}

#[rstest]
fn boolean_field_stores_default_value() {
	// Act
	let field = BooleanField::new().default(true);

	// Assert
	assert_eq!(field.default, Some(true));
}

#[rstest]
fn char_field_required_defaults_to_true() {
	// Act
	let field = CharField::new();

	// Assert
	assert!(field.required);
}

#[rstest]
fn integer_field_can_be_set_optional() {
	// Act
	let field = IntegerField::new().required(false);

	// Assert
	assert!(!field.required);
}

// ---------------------------------------------------------------------------
// validate_fields integration with FieldValidator trait
// ---------------------------------------------------------------------------

struct RangeValidator {
	min: i64,
	max: i64,
}

impl FieldValidator for RangeValidator {
	fn validate(&self, value: &Value) -> ValidationResult {
		if let Some(num) = value.as_i64() {
			if num < self.min || num > self.max {
				return Err(ValidationError::field_error(
					"value",
					format!("Must be between {} and {}", self.min, self.max),
				));
			}
			Ok(())
		} else {
			Err(ValidationError::field_error("value", "Must be a number"))
		}
	}
}

#[rstest]
fn validate_fields_passes_with_valid_data() {
	// Arrange
	let mut validators: HashMap<String, Box<dyn FieldValidator>> = HashMap::new();
	validators.insert(
		"score".into(),
		Box::new(RangeValidator { min: 0, max: 100 }),
	);

	let mut data = HashMap::new();
	data.insert("score".into(), json!(85));

	// Act
	let result = validate_fields(&data, &validators);

	// Assert
	assert!(result.is_ok());
}

#[rstest]
fn validate_fields_fails_with_out_of_range_value() {
	// Arrange
	let mut validators: HashMap<String, Box<dyn FieldValidator>> = HashMap::new();
	validators.insert(
		"score".into(),
		Box::new(RangeValidator { min: 0, max: 100 }),
	);

	let mut data = HashMap::new();
	data.insert("score".into(), json!(150));

	// Act
	let result = validate_fields(&data, &validators);

	// Assert
	assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// SerializerError construction helpers
// ---------------------------------------------------------------------------

#[rstest]
fn serializer_error_required_field_is_validation_error() {
	// Act
	let err = SerializerError::required_field("name".into(), "Name is required".into());

	// Assert
	assert!(err.is_validation_error());
	assert_eq!(err.message(), "Name is required");
}

#[rstest]
fn serializer_error_field_validation_contains_details() {
	// Act
	let err = SerializerError::field_validation(
		"age".into(),
		"-5".into(),
		"min_value".into(),
		"Must be non-negative".into(),
	);

	// Assert
	assert!(err.is_validation_error());
	let display = err.to_string();
	assert!(display.contains("age"));
	assert!(display.contains("-5"));
	assert!(display.contains("min_value"));
}

//! Integration tests for serializers module
//!
//! Tests field validation, JSON serialization/deserialization, and validator
//! interactions across the serializers sub-modules.

use chrono::{Datelike, NaiveDate, Timelike};
use reinhardt_core::serializers::fields::{
	BooleanField, CharField, ChoiceField, DateField, DateTimeField, EmailField, FieldError,
	FieldValue, FloatField, IntegerField, URLField,
};
use reinhardt_core::serializers::{
	FieldValidator, JsonSerializer, Serializer, SerializerError, ValidationError, ValidationResult,
	validate_fields,
};
use rstest::rstest;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

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
fn char_field_uses_custom_error_message() {
	// Arrange
	let field = CharField::new().max_length(5).error_messages(|error| {
		if let FieldError::TooLong(max) = error {
			Some(format!(
				"Ensure this field has no more than {max} characters."
			))
		} else {
			None
		}
	});

	// Act
	let error = field.validate("hello world").unwrap_err();

	// Assert
	assert_eq!(
		error.to_string(),
		"Ensure this field has no more than 5 characters."
	);
}

#[rstest]
fn char_field_custom_error_retains_original_error() {
	// Arrange
	let field = CharField::new()
		.max_length(5)
		.error_messages(|_| Some("Too long".to_string()));

	// Act
	let error = field.validate("hello world").unwrap_err();

	// Assert
	assert_eq!(error.original(), &FieldError::TooLong(5));
	assert_ne!(error, FieldError::TooLong(5));
	assert!(error.is(&FieldError::TooLong(5)));
	assert_eq!(
		std::error::Error::source(&error).and_then(|source| source.downcast_ref::<FieldError>()),
		Some(&FieldError::TooLong(5))
	);
}

#[rstest]
fn char_field_uses_default_error_when_formatter_declines() {
	// Arrange
	let field = CharField::new().max_length(5).error_messages(|_| None);

	// Act
	let error = field.validate("hello world").unwrap_err();

	// Assert
	assert_eq!(error, FieldError::TooLong(5));
	assert_eq!(error.to_string(), "String is too long (max: 5)");
}

#[rstest]
fn char_field_does_not_format_successful_validation() {
	// Arrange
	let formatter_called = Arc::new(AtomicBool::new(false));
	let called = Arc::clone(&formatter_called);
	let field = CharField::new().max_length(5).error_messages(move |_| {
		called.store(true, Ordering::SeqCst);
		Some("unexpected".to_string())
	});

	// Act
	let result = field.validate("hello");

	// Assert
	assert_eq!(result, Ok(()));
	assert!(!formatter_called.load(Ordering::SeqCst));
}

#[rstest]
fn char_field_customizes_required_error_message() {
	// Arrange
	let field = CharField::new().error_messages(|error| {
		error
			.is(&FieldError::Required)
			.then(|| "username is required".into())
	});

	// Act
	let error = field.validate("").unwrap_err();

	// Assert
	assert_eq!(error.to_string(), "username is required");
	assert!(error.is(&FieldError::Required));
}

#[rstest]
fn char_field_formatter_falls_back_for_unhandled_errors() {
	// Arrange
	let field = CharField::new()
		.min_length(3)
		.max_length(5)
		.error_messages(|error| matches!(error, FieldError::TooLong(_)).then(|| "too long".into()));

	// Act
	let too_short = field.validate("hi").unwrap_err();
	let required = field.validate("").unwrap_err();

	// Assert
	assert_eq!(too_short, FieldError::TooShort(3));
	assert_eq!(too_short.to_string(), "String is too short (min: 3)");
	assert_eq!(required, FieldError::Required);
}

#[rstest]
fn char_field_with_error_messages_stays_char_field() {
	// Arrange
	struct UserSerializer {
		username: CharField,
	}
	let serializer = UserSerializer {
		username: CharField::new()
			.max_length(5)
			.error_messages(|_| Some("too long".into())),
	};

	// Act
	let error = serializer.username.validate("hello world").unwrap_err();

	// Assert
	assert_eq!(serializer.username.max_length, Some(5));
	assert!(serializer.username.required);
	assert!(format!("{:?}", serializer.username).contains("CharField"));
	assert_eq!(error.to_string(), "too long");
}

#[rstest]
fn char_field_struct_update_syntax_remains_constructible() {
	// Arrange
	let field = CharField {
		max_length: Some(5),
		..Default::default()
	};

	// Act
	let error = field.validate("hello world").unwrap_err();

	// Assert
	assert_eq!(field.max_length, Some(5));
	assert!(field.required);
	assert_eq!(error, FieldError::TooLong(5));
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
#[case::integer(json!(3), 3)]
#[case::whole_float(json!(3.0), 3)]
#[case::numeric_string(json!(" 3 "), 3)]
#[case::decimal_string(json!("3.0"), 3)]
fn integer_field_coerces_json_values(#[case] input: Value, #[case] expected: i64) {
	// Arrange
	let field = IntegerField::new();

	// Act
	let result = field.to_internal_value(Some(&input));

	// Assert
	assert_eq!(result, Ok(FieldValue::Present(expected)));
}

#[rstest]
fn integer_field_preserves_absent_and_null_slots() {
	// Arrange
	let field = IntegerField::new().allow_null(true);
	let null = Value::Null;

	// Act and Assert
	assert_eq!(field.to_internal_value(None), Ok(FieldValue::Absent));
	assert_eq!(field.to_internal_value(Some(&null)), Ok(FieldValue::Null));
	assert_eq!(
		IntegerField::new().to_internal_value(Some(&null)),
		Err(FieldError::Null)
	);
}

#[rstest]
#[case::array(json!([]))]
#[case::boolean(json!(true))]
#[case::fractional_number(json!(1.5))]
fn integer_field_rejects_invalid_json_type(#[case] input: Value) {
	// Arrange
	let field = IntegerField::new();

	// Act
	let result = field.to_internal_value(Some(&input));

	// Assert
	assert_eq!(
		result,
		Err(FieldError::Custom("A valid integer is required".to_owned()))
	);
}

#[rstest]
fn integer_field_rejects_lossy_floating_integers() {
	// Arrange
	let field = IntegerField::new();
	let oversized_mantissa: Value =
		serde_json::from_str("9007199254740993.0").expect("lossy mantissa JSON");
	let below_i64_min: Value =
		serde_json::from_str("-9223372036854775809.0").expect("below i64::MIN JSON");

	// Act and Assert
	assert_eq!(
		field.to_internal_value(Some(&oversized_mantissa)),
		Err(FieldError::Custom("A valid integer is required".to_owned()))
	);
	assert_eq!(
		field.to_internal_value(Some(&below_i64_min)),
		Err(FieldError::Custom("A valid integer is required".to_owned()))
	);
}

#[rstest]
fn integer_field_accepts_exact_json_integers_beyond_float_mantissa() {
	// Arrange
	let field = IntegerField::new();
	let input = json!(9_007_199_254_740_993_i64);

	// Act
	let result = field.to_internal_value(Some(&input));

	// Assert
	assert_eq!(result, Ok(FieldValue::Present(9_007_199_254_740_993)));
}

#[rstest]
fn integer_field_accepts_safe_whole_float_at_f64_mantissa_limit() {
	// Arrange
	let field = IntegerField::new();
	let input = json!(9_007_199_254_740_992.0);

	// Act
	let result = field.to_internal_value(Some(&input));

	// Assert
	assert_eq!(result, Ok(FieldValue::Present(9_007_199_254_740_992)));
}

#[rstest]
fn integer_field_applies_constraints_after_conversion() {
	// Arrange
	let field = IntegerField::new().min_value(0);
	let input = json!("-1");

	// Act
	let result = field.to_internal_value(Some(&input));

	// Assert
	assert_eq!(result, Err(FieldError::TooSmall(0)));
}

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

#[rstest]
fn integer_field_uses_custom_error_message() {
	// Arrange
	let field = IntegerField::new().min_value(10).error_messages(|error| {
		if let FieldError::TooSmall(min) = error {
			Some(format!("Value must be at least {min}"))
		} else {
			None
		}
	});

	// Act
	let error = field.validate(5).unwrap_err();

	// Assert
	assert_eq!(error.to_string(), "Value must be at least 10");
	assert_eq!(error.original(), &FieldError::TooSmall(10));
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

#[rstest]
fn float_field_uses_custom_error_message() {
	// Arrange
	let field = FloatField::new().max_value(1.5).error_messages(|error| {
		if let FieldError::TooLargeFloat(max) = error {
			Some(format!("Value must not exceed {max}"))
		} else {
			None
		}
	});

	// Act
	let error = field.validate(2.0).unwrap_err();

	// Assert
	assert_eq!(error.to_string(), "Value must not exceed 1.5");
	assert_eq!(error.original(), &FieldError::TooLargeFloat(1.5));
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

#[rstest]
fn email_field_uses_custom_error_message() {
	// Arrange
	let field = EmailField::new().error_messages(|error| match error {
		FieldError::InvalidEmail => Some("Provide a contact email".to_string()),
		_ => None,
	});

	// Act
	let error = field.validate("invalid").unwrap_err();

	// Assert
	assert_eq!(error.to_string(), "Provide a contact email");
	assert_eq!(error.original(), &FieldError::InvalidEmail);
}

#[rstest]
fn email_field_customizes_required_error_message() {
	// Arrange
	let field = EmailField::new().error_messages(|error| {
		error
			.is(&FieldError::Required)
			.then(|| "email is required".into())
	});

	// Act
	let error = field.validate("").unwrap_err();

	// Assert
	assert_eq!(error.to_string(), "email is required");
	assert!(error.is(&FieldError::Required));
}

#[rstest]
fn url_field_uses_custom_error_message() {
	// Arrange
	let field = URLField::new().error_messages(|error| match error {
		FieldError::InvalidUrl => Some("Provide an HTTP or HTTPS URL".to_string()),
		_ => None,
	});

	// Act
	let error = field.validate("ftp://example.com").unwrap_err();

	// Assert
	assert_eq!(error.to_string(), "Provide an HTTP or HTTPS URL");
	assert_eq!(error.original(), &FieldError::InvalidUrl);
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

#[rstest]
fn date_field_parse_and_validate_use_custom_error_message() {
	// Arrange
	let field = DateField::new().error_messages(|error| match error {
		FieldError::InvalidDate => Some("Use an ISO date".to_string()),
		_ => None,
	});

	// Act
	let parse_error = field.parse("not-a-date").unwrap_err();
	let validation_error = field.validate("not-a-date").unwrap_err();

	// Assert
	assert_eq!(parse_error.to_string(), "Use an ISO date");
	assert_eq!(parse_error.original(), &FieldError::InvalidDate);
	assert_eq!(validation_error.to_string(), "Use an ISO date");
	assert_eq!(validation_error.original(), &FieldError::InvalidDate);
}

#[rstest]
fn date_field_customizes_required_error_message() {
	// Arrange
	let field = DateField::new().error_messages(|error| {
		error
			.is(&FieldError::Required)
			.then(|| "date is required".into())
	});

	// Act
	let error = field.validate("").unwrap_err();

	// Assert
	assert_eq!(error.to_string(), "date is required");
	assert!(error.is(&FieldError::Required));
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

#[rstest]
fn datetime_field_parse_and_validate_use_custom_error_message() {
	// Arrange
	let field = DateTimeField::new().error_messages(|error| match error {
		FieldError::InvalidDateTime => Some("Use an ISO date and time".to_string()),
		_ => None,
	});

	// Act
	let parse_error = field.parse("not-a-datetime").unwrap_err();
	let validation_error = field.validate("not-a-datetime").unwrap_err();

	// Assert
	assert_eq!(parse_error.to_string(), "Use an ISO date and time");
	assert_eq!(parse_error.original(), &FieldError::InvalidDateTime);
	assert_eq!(validation_error.to_string(), "Use an ISO date and time");
	assert_eq!(validation_error.original(), &FieldError::InvalidDateTime);
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
fn choice_field_uses_custom_error_message() {
	// Arrange
	let field =
		ChoiceField::new(vec!["red".into(), "green".into()]).error_messages(|error| match error {
			FieldError::InvalidChoice => Some("Choose a supported color".to_string()),
			_ => None,
		});

	// Act
	let error = field.validate("blue").unwrap_err();

	// Assert
	assert_eq!(error.to_string(), "Choose a supported color");
	assert_eq!(error.original(), &FieldError::InvalidChoice);
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
// JSON field extraction
// ---------------------------------------------------------------------------

#[rstest]
fn character_fields_convert_json_scalars_before_validation() {
	// Arrange
	let number = json!(42);
	let email = json!("person@example.com");
	let url = json!("https://example.com/path");
	let choice = json!("active");

	// Assert
	assert_eq!(
		CharField::new().to_internal_value(Some(&number)),
		Ok(FieldValue::Present("42".to_owned()))
	);
	assert_eq!(
		EmailField::new().to_internal_value(Some(&email)),
		Ok(FieldValue::Present("person@example.com".to_owned()))
	);
	assert_eq!(
		URLField::new().to_internal_value(Some(&url)),
		Ok(FieldValue::Present("https://example.com/path".to_owned()))
	);
	assert_eq!(
		ChoiceField::new(vec!["active".to_owned(), "inactive".to_owned()])
			.to_internal_value(Some(&choice)),
		Ok(FieldValue::Present("active".to_owned()))
	);
}

#[rstest]
fn char_field_rejects_json_boolean() {
	// Arrange
	let field = CharField::new();
	let input = json!(true);

	// Act
	let result = field.to_internal_value(Some(&input));

	// Assert
	assert_eq!(
		result,
		Err(FieldError::Custom("Not a valid string".to_owned()))
	);
}

#[rstest]
fn float_and_temporal_fields_convert_json_values() {
	// Arrange
	let float = json!("1.5");
	let date = json!("2026-08-20");
	let datetime = json!("2026-08-20 12:34:56");

	// Assert
	assert_eq!(
		FloatField::new().to_internal_value(Some(&float)),
		Ok(FieldValue::Present(1.5))
	);
	assert_eq!(
		DateField::new().to_internal_value(Some(&date)),
		Ok(FieldValue::Present(
			NaiveDate::from_ymd_opt(2026, 8, 20).unwrap()
		))
	);
	assert_eq!(
		DateTimeField::new().to_internal_value(Some(&datetime)),
		Ok(FieldValue::Present(
			NaiveDate::from_ymd_opt(2026, 8, 20)
				.unwrap()
				.and_hms_opt(12, 34, 56)
				.unwrap()
		))
	);
}

#[rstest]
#[case::boolean(json!(true), true)]
#[case::one(json!(1), true)]
#[case::uppercase_yes(json!("YES"), true)]
#[case::padded_true(json!(" true "), true)]
#[case::false_value(json!(false), false)]
#[case::zero(json!(0), false)]
#[case::off(json!("off"), false)]
#[case::padded_zero(json!(" 0 "), false)]
fn boolean_field_converts_drf_tokens(#[case] input: Value, #[case] expected: bool) {
	// Arrange
	let field = BooleanField::new();

	// Act
	let result = field.to_internal_value(Some(&input));

	// Assert
	assert_eq!(result, Ok(FieldValue::Present(expected)));
}

#[rstest]
fn boolean_field_treats_blank_string_as_null_when_allowed() {
	// Arrange
	let field = BooleanField::new().allow_null(true);
	let input = json!("");

	// Act
	let result = field.to_internal_value(Some(&input));

	// Assert
	assert_eq!(result, Ok(FieldValue::Null));
}

#[rstest]
fn float_field_rejects_non_finite_strings() {
	// Arrange
	let field = FloatField::new();

	// Assert
	assert_eq!(
		field.to_internal_value(Some(&json!("NaN"))),
		Err(FieldError::Custom("A valid number is required".to_owned()))
	);
	assert_eq!(
		field.to_internal_value(Some(&json!("inf"))),
		Err(FieldError::Custom("A valid number is required".to_owned()))
	);
	assert_eq!(
		field.to_internal_value(Some(&json!("1e9999"))),
		Err(FieldError::Custom("A valid number is required".to_owned()))
	);
}

#[rstest]
fn choice_field_preserves_exact_strings_during_json_extraction() {
	// Arrange
	let padded = ChoiceField::new(vec![" active ".to_owned()]);
	let compact = ChoiceField::new(vec!["active".to_owned()]);

	// Assert
	assert_eq!(
		padded.to_internal_value(Some(&json!(" active "))),
		Ok(FieldValue::Present(" active ".to_owned()))
	);
	assert_eq!(
		padded.to_internal_value(Some(&json!("active"))),
		Err(FieldError::InvalidChoice)
	);
	assert_eq!(
		compact.to_internal_value(Some(&json!(" active "))),
		Err(FieldError::InvalidChoice)
	);
	assert_eq!(
		padded.to_internal_value(Some(&json!(1))),
		Err(FieldError::InvalidChoice)
	);
	assert_eq!(
		ChoiceField::new(vec!["1".to_owned()]).to_internal_value(Some(&json!(1))),
		Ok(FieldValue::Present("1".to_owned()))
	);
}

#[rstest]
fn date_field_does_not_apply_default_to_empty_json_string() {
	// Arrange
	let default = NaiveDate::from_ymd_opt(2026, 8, 20).unwrap();
	let field = DateField::new().required(false).default(default);

	// Act
	let result = field.to_internal_value(Some(&json!("")));

	// Assert
	assert_eq!(result, Err(FieldError::InvalidDate));
	assert_ne!(result, Ok(FieldValue::Present(default)));
}

#[rstest]
fn datetime_field_does_not_apply_default_to_empty_json_string() {
	// Arrange
	let default = NaiveDate::from_ymd_opt(2026, 8, 20)
		.unwrap()
		.and_hms_opt(12, 0, 0)
		.unwrap();
	let field = DateTimeField::new().required(false).default(default);

	// Act
	let result = field.to_internal_value(Some(&json!("")));

	// Assert
	assert_eq!(result, Err(FieldError::InvalidDateTime));
}

#[rstest]
fn converted_values_preserve_field_constraint_errors() {
	// Arrange
	let too_short = json!("a");
	let too_large = json!("2.0");
	let invalid_email = json!("invalid");
	let invalid_url = json!("invalid");
	let invalid_choice = json!("inactive");
	let invalid_date = json!("invalid");
	let invalid_datetime = json!("invalid");

	// Assert
	assert_eq!(
		CharField::new()
			.min_length(2)
			.to_internal_value(Some(&too_short)),
		Err(FieldError::TooShort(2))
	);
	assert_eq!(
		FloatField::new()
			.max_value(1.0)
			.to_internal_value(Some(&too_large)),
		Err(FieldError::TooLargeFloat(1.0))
	);
	assert_eq!(
		EmailField::new().to_internal_value(Some(&invalid_email)),
		Err(FieldError::InvalidEmail)
	);
	assert_eq!(
		URLField::new().to_internal_value(Some(&invalid_url)),
		Err(FieldError::InvalidUrl)
	);
	assert_eq!(
		ChoiceField::new(vec!["active".to_owned()]).to_internal_value(Some(&invalid_choice)),
		Err(FieldError::InvalidChoice)
	);
	assert_eq!(
		DateField::new().to_internal_value(Some(&invalid_date)),
		Err(FieldError::InvalidDate)
	);
	assert_eq!(
		DateTimeField::new().to_internal_value(Some(&invalid_datetime)),
		Err(FieldError::InvalidDateTime)
	);
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

#[rstest]
fn validate_fields_uses_registration_keys_for_reused_validator() {
	// Arrange
	let mut validators: HashMap<String, Box<dyn FieldValidator>> = HashMap::new();
	validators.insert(
		"minimum".into(),
		Box::new(RangeValidator { min: 0, max: 10 }),
	);
	validators.insert(
		"maximum".into(),
		Box::new(RangeValidator { min: 0, max: 10 }),
	);
	let data = HashMap::from([
		("minimum".to_owned(), json!(-1)),
		("maximum".to_owned(), json!(11)),
	]);

	// Act
	let errors = validate_fields(&data, &validators)
		.unwrap_err()
		.field_errors();

	// Assert
	assert_eq!(
		errors.get("minimum"),
		Some(&vec!["Must be between 0 and 10".to_owned()])
	);
	assert_eq!(
		errors.get("maximum"),
		Some(&vec!["Must be between 0 and 10".to_owned()])
	);
	assert_eq!(errors.get("value"), None);
}

#[rstest]
fn validate_fields_accepts_built_in_fields_and_collects_all_errors() {
	// Arrange
	let mut validators: HashMap<String, Box<dyn FieldValidator>> = HashMap::new();
	validators.insert("age".into(), Box::new(IntegerField::new().min_value(0)));
	validators.insert("email".into(), Box::new(EmailField::new()));
	let data = HashMap::from([
		("age".to_owned(), json!(-1)),
		("email".to_owned(), json!("invalid")),
	]);

	// Act
	let errors = validate_fields(&data, &validators)
		.unwrap_err()
		.field_errors();

	// Assert
	assert_eq!(errors.len(), 2);
	assert_eq!(
		errors.get("age"),
		Some(&vec!["Value is too small (min: 0)".to_owned()])
	);
	assert_eq!(
		errors.get("email"),
		Some(&vec!["Enter a valid email address".to_owned()])
	);
}

struct MultipleErrorValidator;

impl FieldValidator for MultipleErrorValidator {
	fn validate(&self, _: &Value) -> ValidationResult {
		Err(ValidationError::multiple(vec![
			ValidationError::object_error("First message"),
			ValidationError::multiple(vec![
				ValidationError::field_error("ignored", "Second message"),
				ValidationError::object_error("Third message"),
			]),
		]))
	}
}

#[rstest]
fn validate_fields_replaces_nested_error_keys_and_preserves_message_order() {
	// Arrange
	let validators = HashMap::from([(
		"registered".to_owned(),
		Box::new(MultipleErrorValidator) as Box<dyn FieldValidator>,
	)]);
	let data = HashMap::from([("registered".to_owned(), json!(true))]);

	// Act
	let errors = validate_fields(&data, &validators)
		.unwrap_err()
		.field_errors();

	// Assert
	assert_eq!(errors.len(), 1);
	assert_eq!(
		errors.get("registered"),
		Some(&vec![
			"First message".to_owned(),
			"Second message".to_owned(),
			"Third message".to_owned(),
		])
	);
	assert_eq!(errors.get("ignored"), None);
}

#[rstest]
fn validate_fields_rejects_omitted_required_builtin_field() {
	// Arrange
	let mut validators: HashMap<String, Box<dyn FieldValidator>> = HashMap::new();
	validators.insert("age".into(), Box::new(IntegerField::new().min_value(0)));
	let data = HashMap::new();

	// Act
	let errors = validate_fields(&data, &validators)
		.unwrap_err()
		.field_errors();

	// Assert
	assert_eq!(
		errors.get("age"),
		Some(&vec!["This field is required".to_owned()])
	);
}

#[rstest]
fn validate_fields_skips_omitted_optional_builtin_field() {
	// Arrange
	let mut validators: HashMap<String, Box<dyn FieldValidator>> = HashMap::new();
	validators.insert(
		"age".into(),
		Box::new(IntegerField::new().required(false).min_value(0)),
	);

	// Act
	let result = validate_fields(&HashMap::new(), &validators);

	// Assert
	assert!(result.is_ok());
}

#[rstest]
fn typed_field_validator_direct_errors_do_not_publish_blank_keys() {
	// Arrange
	let field = IntegerField::new().min_value(0);

	// Act
	let errors = FieldValidator::validate(&field, &json!(-1))
		.unwrap_err()
		.field_errors();

	// Assert
	assert!(errors.is_empty());
	assert_eq!(errors.get(""), None);
}

#[rstest]
fn field_errors_omits_standalone_object_errors() {
	// Arrange
	let error = ValidationError::multiple(vec![
		ValidationError::object_error("Object message"),
		ValidationError::multiple(vec![ValidationError::field_error("email", "Invalid")]),
	]);

	// Act
	let errors = error.field_errors();

	// Assert
	assert_eq!(errors.len(), 1);
	assert_eq!(errors.get("email"), Some(&vec!["Invalid".to_owned()]));
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

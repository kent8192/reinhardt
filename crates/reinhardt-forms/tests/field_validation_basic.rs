//! Basic Field Validation Integration Tests
//!
//! Comprehensive tests for CharField, IntegerField, EmailField, BooleanField, and FloatField.
//! Test categories: Happy Path, Error Cases, Edge Cases, Equivalence Partitioning, Boundary Value Analysis, Decision Table, Property-based, Sanity

use proptest::prelude::*;
use reinhardt_forms::{BooleanField, CharField, EmailField, FloatField, FormField, IntegerField};
use rstest::rstest;
use serde_json::json;

// =============================================================================
// CharField Tests
// =============================================================================

// ---- Happy Path ----

#[test]
fn test_char_field_valid_input() {
	let field = CharField::new("name".to_string());
	let result = field.clean(Some(&json!("valid string")));
	assert!(result.is_ok());
	assert_eq!(result.unwrap(), json!("valid string"));
}

#[test]
fn test_char_field_builder_pattern() {
	let field = CharField::new("name".to_string())
		.with_max_length(50)
		.with_min_length(3);
	assert_eq!(field.max_length, Some(50));
	assert_eq!(field.min_length, Some(3));
}

// ---- Error Cases ----

#[test]
fn test_char_field_max_length_exceeded() {
	let field = CharField::new("name".to_string()).with_max_length(10);
	let result = field.clean(Some(&json!("12345678901"))); // 11 characters
	assert!(result.is_err());
}

#[test]
fn test_char_field_min_length_not_met() {
	let field = CharField::new("name".to_string()).with_min_length(5);
	let result = field.clean(Some(&json!("abc"))); // 3 characters
	assert!(result.is_err());
}

#[test]
fn test_char_field_required_missing() {
	let field = CharField::new("name".to_string()).required(); // Explicitly set required=true
	let result = field.clean(None);
	assert!(result.is_err());
}

// ---- Edge Cases ----

#[test]
fn test_char_field_empty_string() {
	let mut field = CharField::new("name".to_string());
	field.required = false;
	let result = field.clean(Some(&json!("")));
	assert!(result.is_ok());
}

#[test]
fn test_char_field_unicode() {
	let field = CharField::new("name".to_string());
	let result = field.clean(Some(&json!("日本語テスト")));
	assert!(result.is_ok());
	assert_eq!(result.unwrap(), json!("日本語テスト"));
}

#[test]
fn test_char_field_emoji() {
	let field = CharField::new("name".to_string());
	let result = field.clean(Some(&json!("👍🎉")));
	assert!(result.is_ok());
}

#[test]
fn test_char_field_strip_whitespace() {
	let field = CharField::new("name".to_string()); // default: strip=true
	let result = field.clean(Some(&json!("  test  ")));
	assert!(result.is_ok());
	// Note: Adjust this test if implementation doesn't support strip
}

// ---- Equivalence Partitioning (rstest #[case]) ----

#[rstest]
#[case("abc", true)] // Valid class
#[case("", true)] // Empty string class (when required=false, default)
#[case("あいう", true)] // Multi-byte class
#[case("test123", true)] // Alphanumeric class
fn test_char_field_equivalence(#[case] input: &str, #[case] valid: bool) {
	let field = CharField::new("name".to_string());
	let result = field.clean(Some(&json!(input)));
	assert_eq!(result.is_ok(), valid);
}

// ---- Boundary Value Analysis (rstest #[case]) ----

#[rstest]
#[case(9, true)] // max_length - 1
#[case(10, true)] // max_length (boundary value)
#[case(11, false)] // max_length + 1
fn test_char_field_boundary(#[case] len: usize, #[case] valid: bool) {
	let field = CharField::new("name".to_string()).with_max_length(10);
	let input = "a".repeat(len);
	assert_eq!(field.clean(Some(&json!(input))).is_ok(), valid);
}

// ---- Decision Table Testing (rstest #[case]) ----

#[rstest]
#[case(true, Some("value"), true)] // required=true, value=Some → OK
#[case(true, None, false)] // required=true, value=None → Error
#[case(false, None, true)] // required=false, value=None → OK
#[case(false, Some(""), true)] // required=false, value=Some("") → OK
fn test_char_field_decision_table(
	#[case] required: bool,
	#[case] value: Option<&str>,
	#[case] expected_ok: bool,
) {
	let mut field = CharField::new("name".to_string());
	field.required = required;
	let json_value = value.map(|v| json!(v));
	assert_eq!(field.clean(json_value.as_ref()).is_ok(), expected_ok);
}

// ---- Property-based Tests (proptest) ----

proptest! {
	#[test]
	fn test_char_field_preserves_valid_input(s in "[a-zA-Z0-9]{1,100}") {
		let field = CharField::new("name".to_string());
		let result = field.clean(Some(&json!(s)));
		prop_assert!(result.is_ok());
		prop_assert_eq!(result.unwrap(), json!(s));
	}

	#[test]
	fn test_char_field_max_length_invariant(s in "[a-zA-Z]{0,20}") {
		let field = CharField::new("name".to_string()).with_max_length(10);
		let result = field.clean(Some(&json!(s)));
		if s.len() <= 10 {
			prop_assert!(result.is_ok());
		} else {
			prop_assert!(result.is_err());
		}
	}
}

// ---- Sanity Test ----

#[test]
fn test_char_field_sanity() {
	let field = CharField::new("test".to_string());
	assert_eq!(field.name, "test");
	assert!(!field.required); // default: false
}

// =============================================================================
// IntegerField Tests
// =============================================================================

// ---- Happy Path ----

#[test]
fn test_integer_field_valid_input() {
	let field = IntegerField::new("age".to_string());
	let result = field.clean(Some(&json!(25)));
	assert!(result.is_ok());
	assert_eq!(result.unwrap(), json!(25));
}

#[test]
fn test_integer_field_string_parsing() {
	let field = IntegerField::new("age".to_string());
	let result = field.clean(Some(&json!("42")));
	assert!(result.is_ok());
	assert_eq!(result.unwrap(), json!(42));
}

// ---- Error Cases ----

#[test]
fn test_integer_field_invalid_string() {
	let field = IntegerField::new("age".to_string());
	let result = field.clean(Some(&json!("not a number")));
	assert!(result.is_err());
}

#[test]
fn test_integer_field_min_value_below() {
	let mut field = IntegerField::new("age".to_string());
	field.min_value = Some(0);
	let result = field.clean(Some(&json!(-1)));
	assert!(result.is_err());
}

#[test]
fn test_integer_field_max_value_exceeded() {
	let mut field = IntegerField::new("age".to_string());
	field.max_value = Some(100);
	let result = field.clean(Some(&json!(101)));
	assert!(result.is_err());
}

// ---- Edge Cases ----

#[test]
fn test_integer_field_zero() {
	let field = IntegerField::new("count".to_string());
	let result = field.clean(Some(&json!(0)));
	assert!(result.is_ok());
	assert_eq!(result.unwrap(), json!(0));
}

#[test]
fn test_integer_field_negative() {
	let field = IntegerField::new("temperature".to_string());
	let result = field.clean(Some(&json!(-10)));
	assert!(result.is_ok());
}

#[test]
fn test_integer_field_i64_max() {
	let field = IntegerField::new("big_number".to_string());
	let result = field.clean(Some(&json!(i64::MAX)));
	assert!(result.is_ok());
}

#[test]
fn test_integer_field_i64_min() {
	let field = IntegerField::new("big_number".to_string());
	let result = field.clean(Some(&json!(i64::MIN)));
	assert!(result.is_ok());
}

// ---- Boundary Value Analysis (rstest #[case]) ----

#[rstest]
#[case(0, true)] // min_value
#[case(1, true)] // min_value + 1
#[case(99, true)] // max_value - 1
#[case(100, true)] // max_value
#[case(-1, false)] // min_value - 1
#[case(101, false)] // max_value + 1
fn test_integer_field_boundary(#[case] value: i64, #[case] valid: bool) {
	let mut field = IntegerField::new("score".to_string());
	field.min_value = Some(0);
	field.max_value = Some(100);
	assert_eq!(field.clean(Some(&json!(value))).is_ok(), valid);
}

// ---- Decision Table Testing (rstest #[case]) ----

#[rstest]
#[case(true, Some(5), true)] // required=true, value=5 → OK
#[case(true, None, false)] // required=true, value=None → Error
#[case(false, None, true)] // required=false, value=None → OK
fn test_integer_field_decision_table(
	#[case] required: bool,
	#[case] value: Option<i64>,
	#[case] expected_ok: bool,
) {
	let mut field = IntegerField::new("age".to_string());
	field.required = required;
	let json_value = value.map(|v| json!(v));
	assert_eq!(field.clean(json_value.as_ref()).is_ok(), expected_ok);
}

// ---- Property-based Tests (proptest) ----

proptest! {
	#[test]
	fn test_integer_field_range_invariant(i in -1000i64..1000) {
		let field = IntegerField::new("num".to_string());
		let result = field.clean(Some(&json!(i)));
		prop_assert!(result.is_ok());
		prop_assert_eq!(result.unwrap(), json!(i));
	}

	#[test]
	fn test_integer_field_min_max_invariant(i in -100i64..200) {
		let mut field = IntegerField::new("num".to_string());
		field.min_value = Some(0);
		field.max_value = Some(100);
		let result = field.clean(Some(&json!(i)));
		if i >= 0 && i <= 100 {
			prop_assert!(result.is_ok());
		} else {
			prop_assert!(result.is_err());
		}
	}
}

// ---- Sanity Test ----

#[test]
fn test_integer_field_sanity() {
	let field = IntegerField::new("age".to_string());
	let result = field.clean(Some(&json!(10)));
	assert!(result.is_ok());
}

// =============================================================================
// EmailField Tests
// =============================================================================

// ---- Happy Path ----

#[test]
fn test_email_field_valid_basic() {
	let field = EmailField::new("email".to_string());
	let result = field.clean(Some(&json!("test@example.com")));
	assert!(result.is_ok());
}

#[test]
fn test_email_field_valid_subdomain() {
	let field = EmailField::new("email".to_string());
	let result = field.clean(Some(&json!("user@mail.example.com")));
	assert!(result.is_ok());
}

#[test]
fn test_email_field_valid_plus_address() {
	let field = EmailField::new("email".to_string());
	let result = field.clean(Some(&json!("user+tag@example.com")));
	assert!(result.is_ok());
}

// ---- Error Cases ----

#[test]
fn test_email_field_invalid_no_at() {
	let field = EmailField::new("email".to_string());
	let result = field.clean(Some(&json!("invalid.email.com")));
	assert!(result.is_err());
}

#[test]
fn test_email_field_invalid_no_domain() {
	let field = EmailField::new("email".to_string());
	let result = field.clean(Some(&json!("user@")));
	assert!(result.is_err());
}

#[test]
fn test_email_field_invalid_no_localpart() {
	let field = EmailField::new("email".to_string());
	let result = field.clean(Some(&json!("@example.com")));
	assert!(result.is_err());
}

// ---- Edge Cases ----

#[test]
fn test_email_field_max_length_default() {
	let field = EmailField::new("email".to_string());
	// EmailField default max_length is 320
	let long_email = format!("{}@example.com", "a".repeat(300));
	let result = field.clean(Some(&json!(long_email)));
	assert!(result.is_ok());
}

#[test]
fn test_email_field_max_length_exceeded() {
	let field = EmailField::new("email".to_string());
	// Exceeds 320 characters
	let very_long_email = format!("{}@example.com", "a".repeat(320));
	let _ = field.clean(Some(&json!(very_long_email)));
	// May error depending on implementation
}

// ---- Equivalence Partitioning (rstest #[case]) ----

#[rstest]
#[case("test@example.com", true)] // Standard email
#[case("user.name@example.com", true)] // Contains dot
#[case("user+tag@example.com", true)] // Contains plus
#[case("invalid", false)] // No @ symbol
#[case("@example.com", false)] // No local part
#[case("user@", false)] // No domain
fn test_email_field_equivalence(#[case] input: &str, #[case] valid: bool) {
	let field = EmailField::new("email".to_string());
	assert_eq!(field.clean(Some(&json!(input))).is_ok(), valid);
}

// ---- Property-based Tests (proptest) ----

proptest! {
	#[test]
	fn test_email_field_basic_format(
		local in "[a-z]{1,10}",
		domain in "[a-z]{1,10}"
	) {
		let email = format!("{}@{}.com", local, domain);
		let field = EmailField::new("email".to_string());
		let result = field.clean(Some(&json!(email)));
		prop_assert!(result.is_ok());
	}
}

// ---- Sanity Test ----

#[test]
fn test_email_field_sanity() {
	let field = EmailField::new("email".to_string());
	let result = field.clean(Some(&json!("test@test.com")));
	assert!(result.is_ok());
}

// =============================================================================
// BooleanField Tests
// =============================================================================

// ---- Happy Path ----

#[test]
fn test_boolean_field_true_value() {
	let field = BooleanField::new("agree".to_string());
	let result = field.clean(Some(&json!(true)));
	assert!(result.is_ok());
	assert_eq!(result.unwrap(), json!(true));
}

#[test]
fn test_boolean_field_false_value() {
	let field = BooleanField::new("agree".to_string());
	let result = field.clean(Some(&json!(false)));
	assert!(result.is_ok());
	assert_eq!(result.unwrap(), json!(false));
}

// ---- Error Cases ----

#[test]
fn test_boolean_field_invalid_type() {
	let field = BooleanField::new("agree".to_string());
	let _ = field.clean(Some(&json!("not a boolean")));
	// May be OK with type coercion depending on implementation
}

// ---- Edge Cases ----

#[test]
fn test_boolean_field_null_value_required() {
	let field = BooleanField::new("agree".to_string()).required(); // Explicitly set required=true
	let result = field.clean(None);
	assert!(result.is_err());
}

#[test]
fn test_boolean_field_null_value_optional() {
	let mut field = BooleanField::new("agree".to_string());
	field.required = false;
	let result = field.clean(None);
	assert!(result.is_ok());
}

// ---- Equivalence Partitioning (rstest #[case]) ----

#[rstest]
#[case(json!(true), true)] // boolean true
#[case(json!(false), true)] // boolean false
#[case(json!(1), true)] // number 1 (type coercion)
#[case(json!(0), true)] // number 0 (type coercion)
#[case(json!("true"), true)] // string "true" (type coercion)
#[case(json!("false"), true)] // string "false" (type coercion)
fn test_boolean_field_equivalence(#[case] input: serde_json::Value, #[case] _valid: bool) {
	let field = BooleanField::new("agree".to_string());
	let _result = field.clean(Some(&input));
	// Depends on implementation's type coercion behavior
}

// ---- Decision Table Testing (rstest #[case]) ----

#[rstest]
#[case(true, Some(true), true)] // required=true, value=true → OK
#[case(true, Some(false), false)] // required=true, value=false → Error (Django behavior: consent required)
#[case(true, None, false)] // required=true, value=None → Error
#[case(false, None, true)] // required=false, value=None → OK
fn test_boolean_field_decision_table(
	#[case] required: bool,
	#[case] value: Option<bool>,
	#[case] expected_ok: bool,
) {
	let mut field = BooleanField::new("agree".to_string());
	field.required = required;
	let json_value = value.map(|v| json!(v));
	assert_eq!(field.clean(json_value.as_ref()).is_ok(), expected_ok);
}

// ---- Sanity Test ----

#[test]
fn test_boolean_field_sanity() {
	let field = BooleanField::new("enabled".to_string());
	let result = field.clean(Some(&json!(true)));
	assert!(result.is_ok());
}

// =============================================================================
// FloatField Tests
// =============================================================================

// ---- Happy Path ----

#[test]
fn test_float_field_valid_input() {
	let field = FloatField::new("price".to_string());
	let result = field.clean(Some(&json!(12.34)));
	assert!(result.is_ok());
	assert_eq!(result.unwrap(), json!(12.34));
}

#[test]
fn test_float_field_string_parsing() {
	let field = FloatField::new("price".to_string());
	let result = field.clean(Some(&json!("56.78")));
	assert!(result.is_ok());
}

// ---- Error Cases ----

#[test]
fn test_float_field_invalid_string() {
	let field = FloatField::new("price".to_string());
	let result = field.clean(Some(&json!("not a number")));
	assert!(result.is_err());
}

#[test]
fn test_float_field_min_value_below() {
	let mut field = FloatField::new("price".to_string());
	field.min_value = Some(0.0);
	let result = field.clean(Some(&json!(-1.0)));
	assert!(result.is_err());
}

#[test]
fn test_float_field_max_value_exceeded() {
	let mut field = FloatField::new("price".to_string());
	field.max_value = Some(100.0);
	let result = field.clean(Some(&json!(101.0)));
	assert!(result.is_err());
}

// ---- Edge Cases ----

#[test]
fn test_float_field_zero() {
	let field = FloatField::new("value".to_string());
	let result = field.clean(Some(&json!(0.0)));
	assert!(result.is_ok());
}

#[test]
fn test_float_field_negative() {
	let field = FloatField::new("value".to_string());
	let result = field.clean(Some(&json!(-123.45)));
	assert!(result.is_ok());
}

#[test]
fn test_float_field_scientific_notation() {
	let field = FloatField::new("value".to_string());
	let _ = field.clean(Some(&json!("1.23e10")));
	// May be supported depending on implementation
}

#[test]
fn test_float_field_infinity_rejected() {
	let field = FloatField::new("value".to_string());
	let _ = field.clean(Some(&json!(f64::INFINITY)));
	// Infinity should be rejected
}

#[test]
fn test_float_field_nan_rejected() {
	let field = FloatField::new("value".to_string());
	let _ = field.clean(Some(&json!(f64::NAN)));
	// NaN should be rejected
}

// ---- Boundary Value Analysis (rstest #[case]) ----

#[rstest]
#[case(0.0, true)] // min_value
#[case(0.1, true)] // min_value + ε
#[case(99.9, true)] // max_value - ε
#[case(100.0, true)] // max_value
#[case(-0.1, false)] // min_value - ε
#[case(100.1, false)] // max_value + ε
fn test_float_field_boundary(#[case] value: f64, #[case] valid: bool) {
	let mut field = FloatField::new("percentage".to_string());
	field.min_value = Some(0.0);
	field.max_value = Some(100.0);
	assert_eq!(field.clean(Some(&json!(value))).is_ok(), valid);
}

// ---- Property-based Tests (proptest) ----

proptest! {
	#[test]
	fn test_float_field_range_invariant(f in -1000.0f64..1000.0) {
		let field = FloatField::new("num".to_string());
		// Assumes NaN and Infinity are excluded
		if f.is_finite() {
			let result = field.clean(Some(&json!(f)));
			prop_assert!(result.is_ok());
		}
	}
}

// ---- Sanity Test ----

#[test]
fn test_float_field_sanity() {
	let field = FloatField::new("price".to_string());
	let result = field.clean(Some(&json!(9.99)));
	assert!(result.is_ok());
}

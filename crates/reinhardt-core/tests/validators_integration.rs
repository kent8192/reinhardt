//! Integration tests for reinhardt-core validators module.
//!
//! These tests verify cross-validator interactions, composition patterns,
//! custom error messages, and error variant correctness.

use reinhardt_core::validators::{
	AndValidator, DateValidator, EmailValidator, IPAddressValidator, MaxLengthValidator,
	MaxValueValidator, MinLengthValidator, MinValueValidator, OrValidator, RangeValidator,
	SlugValidator, UUIDValidator, UrlValidator, ValidationError, Validator,
};
use rstest::rstest;

// ---------------------------------------------------------------------------
// 1. AndValidator composition tests
// ---------------------------------------------------------------------------

#[rstest]
fn and_validator_min_and_max_length_accepts_valid_input() {
	// Arrange
	let validator = AndValidator::new(vec![
		Box::new(MinLengthValidator::new(3)),
		Box::new(MaxLengthValidator::new(10)),
	]);

	// Act
	let result = validator.validate("hello");

	// Assert
	assert!(result.is_ok());
}

#[rstest]
fn and_validator_rejects_too_short_input() {
	// Arrange
	let validator = AndValidator::new(vec![
		Box::new(MinLengthValidator::new(3)),
		Box::new(MaxLengthValidator::new(10)),
	]);

	// Act
	let result = validator.validate("ab");

	// Assert
	assert!(result.is_err());
	assert!(matches!(
		result,
		Err(ValidationError::TooShort { length: 2, min: 3 })
	));
}

#[rstest]
fn and_validator_rejects_too_long_input() {
	// Arrange
	let validator = AndValidator::new(vec![
		Box::new(MinLengthValidator::new(3)),
		Box::new(MaxLengthValidator::new(10)),
	]);

	// Act
	let result = validator.validate("this string is too long");

	// Assert
	assert!(result.is_err());
	assert!(matches!(
		result,
		Err(ValidationError::TooLong { max: 10, .. })
	));
}

#[rstest]
fn and_validator_boundary_values() {
	// Arrange
	let validator = AndValidator::new(vec![
		Box::new(MinLengthValidator::new(3)),
		Box::new(MaxLengthValidator::new(10)),
	]);

	// Act
	let at_min = validator.validate("abc");
	let at_max = validator.validate("1234567890");

	// Assert
	assert!(at_min.is_ok()); // exactly min
	assert!(at_max.is_ok()); // exactly max
}

#[rstest]
fn and_validator_with_builder_pattern() {
	// Arrange
	let validator = AndValidator::new(vec![Box::new(MinLengthValidator::new(3))])
		.with_validator(Box::new(MaxLengthValidator::new(10)));

	// Act
	let valid = validator.validate("hello");
	let too_short = validator.validate("ab");
	let too_long = validator.validate("this is way too long");

	// Assert
	assert!(valid.is_ok());
	assert!(too_short.is_err());
	assert!(too_long.is_err());
}

// ---------------------------------------------------------------------------
// 2. OrValidator composition tests
// ---------------------------------------------------------------------------

#[rstest]
fn or_validator_email_or_url_accepts_valid_email() {
	// Arrange
	let validator = OrValidator::new(vec![
		Box::new(EmailValidator::new()),
		Box::new(UrlValidator::new()),
	]);

	// Act
	let result = validator.validate("user@example.com");

	// Assert
	assert!(result.is_ok());
}

#[rstest]
fn or_validator_email_or_url_accepts_valid_url() {
	// Arrange
	let validator = OrValidator::new(vec![
		Box::new(EmailValidator::new()),
		Box::new(UrlValidator::new()),
	]);

	// Act
	let result = validator.validate("https://example.com");

	// Assert
	assert!(result.is_ok());
}

#[rstest]
fn or_validator_email_or_url_rejects_invalid_input() {
	// Arrange
	let validator = OrValidator::new(vec![
		Box::new(EmailValidator::new()),
		Box::new(UrlValidator::new()),
	]);

	// Act
	let result = validator.validate("not-email-nor-url");

	// Assert
	assert!(result.is_err());
	assert!(matches!(
		result,
		Err(ValidationError::CompositeValidationFailed(_))
	));
}

#[rstest]
fn or_validator_with_error_collection_reports_all_failures() {
	// Arrange
	let validator = OrValidator::new(vec![
		Box::new(MinLengthValidator::new(100)),
		Box::new(MinLengthValidator::new(200)),
	])
	.with_error_collection(true);

	// Act
	let result = validator.validate("short");

	// Assert
	assert!(result.is_err());
	assert!(matches!(
		result,
		Err(ValidationError::AllValidatorsFailed { .. })
	));
}

// ---------------------------------------------------------------------------
// 3. EmailValidator: valid/invalid cases with rstest #[case]
// ---------------------------------------------------------------------------

#[rstest]
#[case("test@example.com")]
#[case("user.name@example.com")]
#[case("user+tag@example.co.uk")]
#[case("user_name@example.com")]
#[case("a@b.co")]
#[case("123@example.com")]
fn email_validator_accepts_valid_emails(#[case] email: &str) {
	// Arrange
	let validator = EmailValidator::new();

	// Act
	let result = validator.validate(email);

	// Assert
	assert!(result.is_ok(), "Expected '{}' to be valid", email);
}

#[rstest]
#[case("invalid-email")]
#[case("@example.com")]
#[case("user@")]
#[case("user..name@example.com")]
#[case(".user@example.com")]
#[case("user@example")]
#[case("user name@example.com")]
#[case("user@@example.com")]
fn email_validator_rejects_invalid_emails(#[case] email: &str) {
	// Arrange
	let validator = EmailValidator::new();

	// Act
	let result = validator.validate(email);

	// Assert
	assert!(result.is_err(), "Expected '{}' to be invalid", email);
}

#[rstest]
fn email_validator_returns_invalid_email_error_variant() {
	// Arrange
	let validator = EmailValidator::new();

	// Act
	let result = validator.validate("not-an-email");

	// Assert
	assert!(matches!(result, Err(ValidationError::InvalidEmail(_))));
}

// ---------------------------------------------------------------------------
// 4. IPAddressValidator: ipv4_only, ipv6_only, both modes
// ---------------------------------------------------------------------------

#[rstest]
#[case("192.168.1.1")]
#[case("10.0.0.1")]
#[case("127.0.0.1")]
#[case("::1")]
#[case("2001:db8::1")]
fn ip_validator_default_accepts_both_versions(#[case] ip: &str) {
	// Arrange
	let validator = IPAddressValidator::new();

	// Act
	let result = validator.validate(ip);

	// Assert
	assert!(result.is_ok(), "Expected '{}' to be valid", ip);
}

#[rstest]
#[case("192.168.1.1")]
#[case("10.0.0.1")]
#[case("255.255.255.255")]
fn ip_validator_ipv4_only_accepts_ipv4(#[case] ip: &str) {
	// Arrange
	let validator = IPAddressValidator::ipv4_only();

	// Act
	let result = validator.validate(ip);

	// Assert
	assert!(result.is_ok(), "Expected '{}' to be valid IPv4", ip);
}

#[rstest]
#[case("::1")]
#[case("2001:db8::1")]
#[case("fe80::1")]
fn ip_validator_ipv4_only_rejects_ipv6(#[case] ip: &str) {
	// Arrange
	let validator = IPAddressValidator::ipv4_only();

	// Act
	let result = validator.validate(ip);

	// Assert
	assert!(
		result.is_err(),
		"Expected '{}' to be rejected by ipv4_only",
		ip
	);
	assert!(matches!(result, Err(ValidationError::InvalidIPAddress(_))));
}

#[rstest]
#[case("::1")]
#[case("2001:db8::1")]
#[case("fe80::1")]
fn ip_validator_ipv6_only_accepts_ipv6(#[case] ip: &str) {
	// Arrange
	let validator = IPAddressValidator::ipv6_only();

	// Act
	let result = validator.validate(ip);

	// Assert
	assert!(result.is_ok(), "Expected '{}' to be valid IPv6", ip);
}

#[rstest]
#[case("192.168.1.1")]
#[case("10.0.0.1")]
fn ip_validator_ipv6_only_rejects_ipv4(#[case] ip: &str) {
	// Arrange
	let validator = IPAddressValidator::ipv6_only();

	// Act
	let result = validator.validate(ip);

	// Assert
	assert!(
		result.is_err(),
		"Expected '{}' to be rejected by ipv6_only",
		ip
	);
	assert!(matches!(result, Err(ValidationError::InvalidIPAddress(_))));
}

#[rstest]
fn ip_validator_rejects_invalid_addresses() {
	// Arrange
	let validator = IPAddressValidator::new();

	// Act
	let invalid_text = validator.validate("invalid-ip");
	let out_of_range = validator.validate("256.1.1.1");
	let empty = validator.validate("");

	// Assert
	assert!(invalid_text.is_err());
	assert!(out_of_range.is_err());
	assert!(empty.is_err());
}

// ---------------------------------------------------------------------------
// 5. MinValueValidator/MaxValueValidator with numeric types
// ---------------------------------------------------------------------------

#[rstest]
#[case(10, true)]
#[case(15, true)]
#[case(100, true)]
#[case(5, false)]
#[case(0, false)]
fn min_value_validator_i32(#[case] value: i32, #[case] expected_ok: bool) {
	// Arrange
	let validator = MinValueValidator::new(10);

	// Act
	let result = validator.validate(&value);

	// Assert
	assert_eq!(result.is_ok(), expected_ok);
}

#[rstest]
#[case(20, true)]
#[case(15, true)]
#[case(0, true)]
#[case(25, false)]
#[case(100, false)]
fn max_value_validator_i32(#[case] value: i32, #[case] expected_ok: bool) {
	// Arrange
	let validator = MaxValueValidator::new(20);

	// Act
	let result = validator.validate(&value);

	// Assert
	assert_eq!(result.is_ok(), expected_ok);
}

#[rstest]
fn min_value_validator_returns_too_small_error() {
	// Arrange
	let validator = MinValueValidator::new(10);

	// Act
	let result = validator.validate(&5);

	// Assert
	match result {
		Err(ValidationError::TooSmall { value, min }) => {
			assert_eq!(value, "5");
			assert_eq!(min, "10");
		}
		_ => panic!("Expected TooSmall error"),
	}
}

#[rstest]
fn max_value_validator_returns_too_large_error() {
	// Arrange
	let validator = MaxValueValidator::new(20);

	// Act
	let result = validator.validate(&25);

	// Assert
	match result {
		Err(ValidationError::TooLarge { value, max }) => {
			assert_eq!(value, "25");
			assert_eq!(max, "20");
		}
		_ => panic!("Expected TooLarge error"),
	}
}

#[rstest]
fn range_validator_within_range() {
	// Arrange
	let validator = RangeValidator::new(10, 20);

	// Act
	let at_min = validator.validate(&10);
	let in_middle = validator.validate(&15);
	let at_max = validator.validate(&20);

	// Assert
	assert!(at_min.is_ok());
	assert!(in_middle.is_ok());
	assert!(at_max.is_ok());
}

#[rstest]
fn range_validator_outside_range() {
	// Arrange
	let validator = RangeValidator::new(10, 20);

	// Act
	let below = validator.validate(&5);
	let above = validator.validate(&25);

	// Assert
	assert!(below.is_err());
	assert!(above.is_err());
}

#[rstest]
fn numeric_validators_with_f64() {
	// Arrange
	let min_validator = MinValueValidator::new(0.0f64);
	let max_validator = MaxValueValidator::new(1.0f64);

	// Act
	let min_ok = min_validator.validate(&0.5f64);
	let min_fail = min_validator.validate(&-0.1f64);
	let max_ok = max_validator.validate(&0.5f64);
	let max_fail = max_validator.validate(&1.1f64);

	// Assert
	assert!(min_ok.is_ok());
	assert!(min_fail.is_err());
	assert!(max_ok.is_ok());
	assert!(max_fail.is_err());
}

// ---------------------------------------------------------------------------
// 6. String validators: MinLengthValidator, MaxLengthValidator
// ---------------------------------------------------------------------------

#[rstest]
fn min_length_validator_with_str_and_string() {
	// Arrange
	let validator = MinLengthValidator::new(3);
	let s = String::from("hello");
	let s2 = String::from("ab");

	// Act
	let str_ok = validator.validate("hello");
	let str_err = validator.validate("ab");
	let string_ok = validator.validate(&s);
	let string_err = validator.validate(&s2);

	// Assert
	assert!(str_ok.is_ok());
	assert!(str_err.is_err());
	assert!(string_ok.is_ok());
	assert!(string_err.is_err());
}

#[rstest]
fn max_length_validator_with_str_and_string() {
	// Arrange
	let validator = MaxLengthValidator::new(5);
	let s = String::from("hi");
	let s2 = String::from("toolong");

	// Act
	let str_ok = validator.validate("hello");
	let str_err = validator.validate("toolong");
	let string_ok = validator.validate(&s);
	let string_err = validator.validate(&s2);

	// Assert
	assert!(str_ok.is_ok());
	assert!(str_err.is_err());
	assert!(string_ok.is_ok());
	assert!(string_err.is_err());
}

#[rstest]
fn min_length_returns_too_short_error_with_details() {
	// Arrange
	let validator = MinLengthValidator::new(10);

	// Act
	let result = validator.validate("short");

	// Assert
	match result {
		Err(ValidationError::TooShort { length, min }) => {
			assert_eq!(length, 5);
			assert_eq!(min, 10);
		}
		_ => panic!("Expected TooShort error"),
	}
}

#[rstest]
fn max_length_returns_too_long_error_with_details() {
	// Arrange
	let validator = MaxLengthValidator::new(3);

	// Act
	let result = validator.validate("toolong");

	// Assert
	match result {
		Err(ValidationError::TooLong { length, max }) => {
			assert_eq!(length, 7);
			assert_eq!(max, 3);
		}
		_ => panic!("Expected TooLong error"),
	}
}

// ---------------------------------------------------------------------------
// 7. SlugValidator, UUIDValidator, DateValidator
// ---------------------------------------------------------------------------

#[rstest]
#[case("my-valid-slug", true)]
#[case("my_slug_123", true)]
#[case("simple", true)]
#[case("invalid slug", false)]
#[case("invalid!slug", false)]
#[case("", false)]
fn slug_validator(#[case] input: &str, #[case] expected_ok: bool) {
	// Arrange
	let validator = SlugValidator::new();

	// Act
	let result = validator.validate(input);

	// Assert
	assert_eq!(
		result.is_ok(),
		expected_ok,
		"Slug '{}' expected {}",
		input,
		if expected_ok { "valid" } else { "invalid" }
	);
}

#[rstest]
fn slug_validator_returns_invalid_slug_error() {
	// Arrange
	let validator = SlugValidator::new();

	// Act
	let result = validator.validate("invalid slug!");

	// Assert
	assert!(matches!(result, Err(ValidationError::InvalidSlug(_))));
}

#[rstest]
#[case("550e8400-e29b-41d4-a716-446655440000", true)]
#[case("6ba7b810-9dad-11d1-80b4-00c04fd430c8", true)]
#[case("not-a-uuid", false)]
#[case("550e8400-e29b-41d4-a716", false)]
#[case("", false)]
fn uuid_validator(#[case] input: &str, #[case] expected_ok: bool) {
	// Arrange
	let validator = UUIDValidator::new();

	// Act
	let result = validator.validate(input);

	// Assert
	assert_eq!(
		result.is_ok(),
		expected_ok,
		"UUID '{}' expected {}",
		input,
		if expected_ok { "valid" } else { "invalid" }
	);
}

#[rstest]
fn uuid_validator_returns_invalid_uuid_error() {
	// Arrange
	let validator = UUIDValidator::new();

	// Act
	let result = validator.validate("not-a-uuid");

	// Assert
	assert!(matches!(result, Err(ValidationError::InvalidUUID(_))));
}

#[rstest]
#[case("2024-01-15", true)]
#[case("2024-12-31", true)]
#[case("2024-02-29", true)] // leap year
#[case("not-a-date", false)]
#[case("2024-13-01", false)] // invalid month
#[case("2024-01-32", false)] // invalid day
fn date_validator(#[case] input: &str, #[case] expected_ok: bool) {
	// Arrange
	let validator = DateValidator::new();

	// Act
	let result = validator.validate(input);

	// Assert
	assert_eq!(
		result.is_ok(),
		expected_ok,
		"Date '{}' expected {}",
		input,
		if expected_ok { "valid" } else { "invalid" }
	);
}

#[rstest]
fn date_validator_returns_invalid_date_error() {
	// Arrange
	let validator = DateValidator::new();

	// Act
	let result = validator.validate("not-a-date");

	// Assert
	assert!(matches!(result, Err(ValidationError::InvalidDate(_))));
}

#[rstest]
fn date_validator_custom_format() {
	// Arrange
	let validator = DateValidator::new().with_format("%d/%m/%Y");

	// Act
	let custom_format_ok = validator.validate("15/01/2024");
	let default_format_err = validator.validate("2024-01-15");

	// Assert
	assert!(custom_format_ok.is_ok());
	assert!(default_format_err.is_err());
}

// ---------------------------------------------------------------------------
// 8. Custom error messages with .with_message()
// ---------------------------------------------------------------------------

#[rstest]
fn email_validator_custom_message() {
	// Arrange
	let custom_msg = "Please enter a valid email address";
	let validator = EmailValidator::new().with_message(custom_msg);

	// Act
	let result = validator.validate("invalid");

	// Assert
	match result {
		Err(ValidationError::Custom(msg)) => {
			assert_eq!(msg, custom_msg);
		}
		_ => panic!("Expected Custom error with custom message"),
	}
}

#[rstest]
fn ip_address_validator_custom_message() {
	// Arrange
	let custom_msg = "Invalid IP address format";
	let validator = IPAddressValidator::new().with_message(custom_msg);

	// Act
	let result = validator.validate("invalid-ip");

	// Assert
	match result {
		Err(ValidationError::InvalidIPAddress(msg)) => {
			assert_eq!(msg, custom_msg);
		}
		_ => panic!("Expected InvalidIPAddress error with custom message"),
	}
}

#[rstest]
fn min_length_validator_custom_message() {
	// Arrange
	let custom_msg = "Username must be at least 5 characters";
	let validator = MinLengthValidator::new(5).with_message(custom_msg);

	// Act
	let result = validator.validate("hi");

	// Assert
	match result {
		Err(ValidationError::Custom(msg)) => {
			assert_eq!(msg, custom_msg);
		}
		_ => panic!("Expected Custom error with custom message"),
	}
}

#[rstest]
fn max_length_validator_custom_message() {
	// Arrange
	let custom_msg = "Username must be at most 10 characters";
	let validator = MaxLengthValidator::new(10).with_message(custom_msg);

	// Act
	let result = validator.validate("this is way too long for the field");

	// Assert
	match result {
		Err(ValidationError::Custom(msg)) => {
			assert_eq!(msg, custom_msg);
		}
		_ => panic!("Expected Custom error with custom message"),
	}
}

#[rstest]
fn min_value_validator_custom_message() {
	// Arrange
	let custom_msg = "Age must be at least 18";
	let validator = MinValueValidator::new(18).with_message(custom_msg);

	// Act
	let result = validator.validate(&10);

	// Assert
	match result {
		Err(ValidationError::Custom(msg)) => {
			assert_eq!(msg, custom_msg);
		}
		_ => panic!("Expected Custom error with custom message"),
	}
}

#[rstest]
fn max_value_validator_custom_message() {
	// Arrange
	let custom_msg = "Quantity must be at most 100";
	let validator = MaxValueValidator::new(100).with_message(custom_msg);

	// Act
	let result = validator.validate(&200);

	// Assert
	match result {
		Err(ValidationError::Custom(msg)) => {
			assert_eq!(msg, custom_msg);
		}
		_ => panic!("Expected Custom error with custom message"),
	}
}

#[rstest]
fn slug_validator_custom_message() {
	// Arrange
	let custom_msg = "Invalid URL slug format";
	let validator = SlugValidator::new().with_message(custom_msg);

	// Act
	let result = validator.validate("invalid slug!");

	// Assert
	match result {
		Err(ValidationError::Custom(msg)) => {
			assert_eq!(msg, custom_msg);
		}
		_ => panic!("Expected Custom error with custom message"),
	}
}

#[rstest]
fn date_validator_custom_message() {
	// Arrange
	let custom_msg = "Please use YYYY-MM-DD format";
	let validator = DateValidator::new().with_message(custom_msg);

	// Act
	let result = validator.validate("not-a-date");

	// Assert
	match result {
		Err(ValidationError::Custom(msg)) => {
			assert_eq!(msg, custom_msg);
		}
		_ => panic!("Expected Custom error with custom message"),
	}
}

// ---------------------------------------------------------------------------
// 9. ValidationError variant verification
// ---------------------------------------------------------------------------

#[rstest]
fn validation_error_invalid_email_display() {
	// Arrange
	let error = ValidationError::InvalidEmail("bad@".into());

	// Act
	let display = error.to_string();

	// Assert
	assert_eq!(display, "Invalid email: bad@");
}

#[rstest]
fn validation_error_too_short_display() {
	// Arrange
	let error = ValidationError::TooShort { length: 3, min: 5 };

	// Act
	let display = error.to_string();

	// Assert
	assert_eq!(display, "Length too short: 3 (minimum: 5)");
}

#[rstest]
fn validation_error_too_long_display() {
	// Arrange
	let error = ValidationError::TooLong {
		length: 20,
		max: 10,
	};

	// Act
	let display = error.to_string();

	// Assert
	assert_eq!(display, "Length too long: 20 (maximum: 10)");
}

#[rstest]
fn validation_error_too_small_display() {
	// Arrange
	let error = ValidationError::TooSmall {
		value: "5".into(),
		min: "10".into(),
	};

	// Act
	let display = error.to_string();

	// Assert
	assert_eq!(display, "Value too small: 5 (minimum: 10)");
}

#[rstest]
fn validation_error_too_large_display() {
	// Arrange
	let error = ValidationError::TooLarge {
		value: "100".into(),
		max: "50".into(),
	};

	// Act
	let display = error.to_string();

	// Assert
	assert_eq!(display, "Value too large: 100 (maximum: 50)");
}

#[rstest]
fn validation_error_invalid_ip_address_display() {
	// Arrange
	let error = ValidationError::InvalidIPAddress("bad-ip".into());

	// Act
	let display = error.to_string();

	// Assert
	assert_eq!(display, "Invalid IP address: bad-ip");
}

#[rstest]
fn validation_error_invalid_slug_display() {
	// Arrange
	let error = ValidationError::InvalidSlug("bad slug".into());

	// Act
	let display = error.to_string();

	// Assert
	assert_eq!(display, "Invalid slug: bad slug");
}

#[rstest]
fn validation_error_invalid_uuid_display() {
	// Arrange
	let error = ValidationError::InvalidUUID("bad-uuid".into());

	// Act
	let display = error.to_string();

	// Assert
	assert_eq!(display, "Invalid UUID: bad-uuid");
}

#[rstest]
fn validation_error_custom_display() {
	// Arrange
	let error = ValidationError::Custom("custom message".into());

	// Act
	let display = error.to_string();

	// Assert
	assert_eq!(display, "Custom validation error: custom message");
}

#[rstest]
fn validation_error_clone_and_eq() {
	// Arrange
	let error = ValidationError::InvalidEmail("test@".into());

	// Act
	let cloned = error.clone();

	// Assert
	assert_eq!(error, cloned);
}

#[rstest]
fn validation_error_composite_failed_display() {
	// Arrange
	let error = ValidationError::CompositeValidationFailed("All validators failed".into());

	// Act
	let display = error.to_string();

	// Assert
	assert_eq!(display, "Validation failed: All validators failed");
}

#[rstest]
fn validation_error_all_validators_failed_display() {
	// Arrange
	let error = ValidationError::AllValidatorsFailed {
		errors: "error1; error2".into(),
	};

	// Act
	let display = error.to_string();

	// Assert
	assert_eq!(display, "All validators failed: error1; error2");
}

// ---------------------------------------------------------------------------
// Additional integration: nested composition
// ---------------------------------------------------------------------------

#[rstest]
fn nested_and_in_or_composition() {
	// Arrange: (3-10 chars) OR (20+ chars)
	let short_range = AndValidator::new(vec![
		Box::new(MinLengthValidator::new(3)),
		Box::new(MaxLengthValidator::new(10)),
	]);
	let or_validator = OrValidator::new(vec![
		Box::new(short_range),
		Box::new(MinLengthValidator::new(20)),
	]);

	// Act
	let short_valid = or_validator.validate("hello"); // passes first (3-10 range)
	let long_valid = or_validator.validate("this is a very long string indeed"); // passes second (20+)
	let fails_both = or_validator.validate("ab"); // fails both

	// Assert
	assert!(short_valid.is_ok());
	assert!(long_valid.is_ok());
	assert!(fails_both.is_err());
}

#[rstest]
fn and_validator_with_mixed_validator_types() {
	// Arrange: string must be 3-50 chars AND a valid slug
	let validator = AndValidator::new(vec![
		Box::new(MinLengthValidator::new(3)),
		Box::new(MaxLengthValidator::new(50)),
		Box::new(SlugValidator::new()),
	]);

	// Act
	let valid = validator.validate("valid-slug");
	let too_short = validator.validate("ab");
	let not_a_slug = validator.validate("invalid slug with spaces");

	// Assert
	assert!(valid.is_ok());
	assert!(too_short.is_err()); // too short
	assert!(not_a_slug.is_err()); // not a slug
}

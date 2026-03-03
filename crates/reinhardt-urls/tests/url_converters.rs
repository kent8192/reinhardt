//! Integration tests for URL path parameter converters.
//!
//! Tests cover all converter types: IntegerConverter, UuidConverter,
//! SlugConverter, DateConverter, PathConverter, FloatConverter.

use chrono::Datelike;
use reinhardt_urls::routers::converters::{
	Converter, ConverterError, DateConverter, FloatConverter, IntegerConverter, PathConverter,
	SlugConverter, UuidConverter,
};
use rstest::rstest;

// ===================================================================
// IntegerConverter tests
// ===================================================================

#[rstest]
fn integer_converter_validate_valid_integers() {
	// Arrange
	let conv = IntegerConverter::new();

	// Act
	let results = [
		conv.validate("0"),
		conv.validate("1"),
		conv.validate("123"),
		conv.validate("-1"),
		conv.validate("-456"),
	];

	// Assert
	assert!(results.iter().all(|r| *r));
}

#[rstest]
fn integer_converter_validate_invalid_inputs() {
	// Arrange
	let conv = IntegerConverter::new();

	// Act
	let results = [
		conv.validate(""),
		conv.validate("abc"),
		conv.validate("12.5"),
		conv.validate("1.0"),
		conv.validate("1e5"),
		conv.validate(" 1"),
		conv.validate("1 "),
	];

	// Assert
	assert!(results.iter().all(|r| !*r));
}

#[rstest]
#[case("1", 1i64)]
#[case("0", 0i64)]
#[case("-1", -1i64)]
#[case("9999999", 9999999i64)]
#[case("-9999999", -9999999i64)]
fn integer_converter_convert_valid(#[case] input: &str, #[case] expected: i64) {
	// Arrange
	let conv = IntegerConverter::new();

	// Act
	let result = conv.convert(input);

	// Assert
	assert_eq!(result.unwrap(), expected);
}

#[rstest]
fn integer_converter_convert_invalid_returns_error() {
	// Arrange
	let conv = IntegerConverter::new();

	// Act
	let result = conv.convert("not-a-number");

	// Assert
	assert!(result.is_err());
	let err = result.unwrap_err();
	assert!(matches!(err, ConverterError::InvalidFormat(_)));
}

#[rstest]
fn integer_converter_with_range_valid() {
	// Arrange
	let conv = IntegerConverter::with_range(1, 100);

	// Act
	let results = [
		conv.validate("1"),
		conv.validate("50"),
		conv.validate("100"),
	];

	// Assert
	assert!(results.iter().all(|r| *r));
}

#[rstest]
fn integer_converter_with_range_out_of_range() {
	// Arrange
	let conv = IntegerConverter::with_range(1, 100);

	// Act
	let results = [
		conv.validate("0"),
		conv.validate("101"),
		conv.validate("-10"),
	];

	// Assert
	assert!(results.iter().all(|r| !*r));
}

#[rstest]
fn integer_converter_with_range_convert_out_of_range_returns_error() {
	// Arrange
	let conv = IntegerConverter::with_range(1, 100);

	// Act
	let result_below = conv.convert("0");
	let result_above = conv.convert("101");

	// Assert
	assert!(matches!(
		result_below.unwrap_err(),
		ConverterError::OutOfRange(_)
	));
	assert!(matches!(
		result_above.unwrap_err(),
		ConverterError::OutOfRange(_)
	));
}

#[rstest]
fn integer_converter_pattern() {
	// Arrange
	let conv = IntegerConverter::new();

	// Act
	let pattern = conv.pattern();

	// Assert
	assert_eq!(pattern, r"-?\d+");
}

#[rstest]
fn integer_converter_default_equals_new() {
	// Arrange
	let conv_default = IntegerConverter::default();
	let conv_new = IntegerConverter::new();

	// Act
	let result_default = conv_default.validate("42");
	let result_new = conv_new.validate("42");

	// Assert
	assert!(result_default);
	assert!(result_new);
}

// ===================================================================
// UuidConverter tests
// ===================================================================

#[rstest]
#[case("550e8400-e29b-41d4-a716-446655440000")]
#[case("6ba7b810-9dad-11d1-80b4-00c04fd430c8")]
#[case("00000000-0000-0000-0000-000000000000")]
#[case("ffffffff-ffff-ffff-ffff-ffffffffffff")]
fn uuid_converter_validate_valid(#[case] input: &str) {
	// Arrange
	let conv = UuidConverter;

	// Act
	let result = conv.validate(input);

	// Assert
	assert!(result, "Expected '{}' to be a valid UUID", input);
}

#[rstest]
#[case("not-a-uuid")]
#[case("550e8400-e29b-41d4-a716")]
#[case("550e8400-e29b-41d4-a716-446655440000-extra")]
#[case("550E8400-E29B-41D4-A716-446655440000")]
#[case("")]
#[case("550e8400e29b41d4a716446655440000")]
fn uuid_converter_validate_invalid(#[case] input: &str) {
	// Arrange
	let conv = UuidConverter;

	// Act
	let result = conv.validate(input);

	// Assert
	assert!(!result, "Expected '{}' to be an invalid UUID", input);
}

#[rstest]
fn uuid_converter_convert_valid() {
	// Arrange
	let conv = UuidConverter;
	let uuid = "550e8400-e29b-41d4-a716-446655440000";

	// Act
	let result = conv.convert(uuid);

	// Assert
	assert_eq!(result.unwrap(), uuid);
}

#[rstest]
fn uuid_converter_convert_invalid_returns_error() {
	// Arrange
	let conv = UuidConverter;

	// Act
	let result = conv.convert("not-a-uuid");

	// Assert
	assert!(result.is_err());
	assert!(matches!(
		result.unwrap_err(),
		ConverterError::InvalidFormat(_)
	));
}

#[rstest]
fn uuid_converter_pattern() {
	// Arrange
	let conv = UuidConverter;

	// Act
	let pattern = conv.pattern();

	// Assert
	assert_eq!(
		pattern,
		r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}"
	);
}

// ===================================================================
// SlugConverter tests
// ===================================================================

#[rstest]
#[case("my-blog-post")]
#[case("article-123")]
#[case("hello-world")]
#[case("simple")]
#[case("abc")]
#[case("a1b2c3")]
fn slug_converter_validate_valid(#[case] input: &str) {
	// Arrange
	let conv = SlugConverter;

	// Act
	let result = conv.validate(input);

	// Assert
	assert!(result, "Expected '{}' to be a valid slug", input);
}

#[rstest]
#[case("Invalid Slug!")]
#[case("no_underscores")]
#[case("NO-UPPERCASE")]
#[case("-starts-with-hyphen")]
#[case("ends-with-hyphen-")]
#[case("double--hyphens")]
#[case("")]
#[case("has space")]
fn slug_converter_validate_invalid(#[case] input: &str) {
	// Arrange
	let conv = SlugConverter;

	// Act
	let result = conv.validate(input);

	// Assert
	assert!(!result, "Expected '{}' to be an invalid slug", input);
}

#[rstest]
fn slug_converter_convert_valid() {
	// Arrange
	let conv = SlugConverter;
	let slug = "my-blog-post";

	// Act
	let result = conv.convert(slug);

	// Assert
	assert_eq!(result.unwrap(), slug);
}

#[rstest]
fn slug_converter_convert_invalid_returns_error() {
	// Arrange
	let conv = SlugConverter;

	// Act
	let result = conv.convert("Invalid Slug!");

	// Assert
	assert!(result.is_err());
	assert!(matches!(
		result.unwrap_err(),
		ConverterError::InvalidFormat(_)
	));
}

#[rstest]
fn slug_converter_pattern() {
	// Arrange
	let conv = SlugConverter;

	// Act
	let pattern = conv.pattern();

	// Assert
	assert_eq!(pattern, r"[a-z0-9]+(-[a-z0-9]+)*");
}

// ===================================================================
// DateConverter tests
// ===================================================================

#[rstest]
#[case("2024-01-15")]
#[case("2023-12-31")]
#[case("2000-02-29")]
#[case("1999-01-01")]
fn date_converter_validate_valid(#[case] input: &str) {
	// Arrange
	let conv = DateConverter;

	// Act
	let result = conv.validate(input);

	// Assert
	assert!(result, "Expected '{}' to be a valid date", input);
}

#[rstest]
#[case("2024-13-01")]
#[case("2024-01-32")]
#[case("2023-02-29")]
#[case("24-01-15")]
#[case("2024/01/15")]
#[case("not-a-date")]
#[case("")]
#[case("2024-1-5")]
fn date_converter_validate_invalid(#[case] input: &str) {
	// Arrange
	let conv = DateConverter;

	// Act
	let result = conv.validate(input);

	// Assert
	assert!(!result, "Expected '{}' to be an invalid date", input);
}

#[rstest]
fn date_converter_convert_valid() {
	// Arrange
	let conv = DateConverter;

	// Act
	let date = conv.convert("2024-01-15").unwrap();

	// Assert
	assert_eq!(date.year(), 2024);
	assert_eq!(date.month(), 1);
	assert_eq!(date.day(), 15);
}

#[rstest]
fn date_converter_convert_end_of_year() {
	// Arrange
	let conv = DateConverter;

	// Act
	let date = conv.convert("2023-12-31").unwrap();

	// Assert
	assert_eq!(date.year(), 2023);
	assert_eq!(date.month(), 12);
	assert_eq!(date.day(), 31);
}

#[rstest]
fn date_converter_convert_invalid_returns_error() {
	// Arrange
	let conv = DateConverter;

	// Act
	let result = conv.convert("2024-13-01");

	// Assert
	assert!(result.is_err());
	assert!(matches!(
		result.unwrap_err(),
		ConverterError::InvalidFormat(_)
	));
}

#[rstest]
fn date_converter_pattern() {
	// Arrange
	let conv = DateConverter;

	// Act
	let pattern = conv.pattern();

	// Assert
	assert_eq!(pattern, r"\d{4}-\d{2}-\d{2}");
}

// ===================================================================
// PathConverter tests
// ===================================================================

#[rstest]
#[case("path/to/file.txt")]
#[case("images/photo.jpg")]
#[case("documents/2024/report.pdf")]
#[case("simple.txt")]
#[case("a/b/c/d/e.txt")]
fn path_converter_validate_valid(#[case] input: &str) {
	// Arrange
	let conv = PathConverter;

	// Act
	let result = conv.validate(input);

	// Assert
	assert!(result, "Expected '{}' to be a valid path", input);
}

#[rstest]
#[case("../etc/passwd")]
#[case("path/../secret")]
#[case("path/to/../../file")]
#[case("..")]
#[case("path/..")]
#[case("../path")]
fn path_converter_validate_directory_traversal(#[case] input: &str) {
	// Arrange
	let conv = PathConverter;

	// Act
	let result = conv.validate(input);

	// Assert
	assert!(
		!result,
		"Expected '{}' to be rejected as directory traversal",
		input
	);
}

#[rstest]
fn path_converter_validate_empty_returns_false() {
	// Arrange
	let conv = PathConverter;

	// Act
	let result = conv.validate("");

	// Assert
	assert!(!result);
}

#[rstest]
fn path_converter_validate_null_byte_returns_false() {
	// Arrange
	let conv = PathConverter;

	// Act
	let result = conv.validate("path\0/file");

	// Assert
	assert!(!result);
}

#[rstest]
fn path_converter_validate_absolute_path_returns_false() {
	// Arrange
	let conv = PathConverter;

	// Act
	let result = conv.validate("/etc/passwd");

	// Assert
	assert!(!result);
}

#[rstest]
fn path_converter_validate_backslash_returns_false() {
	// Arrange
	let conv = PathConverter;

	// Act
	let result_forward = conv.validate("path\\to\\file");
	let result_traversal = conv.validate("..\\etc\\passwd");

	// Assert
	assert!(!result_forward);
	assert!(!result_traversal);
}

#[rstest]
#[case("%2e%2e/etc/passwd")]
#[case("foo/%2e%2e/bar")]
#[case("%2E%2E/secret")]
#[case("foo%2fbar")]
#[case("foo%5cbar")]
#[case("file%00.txt")]
fn path_converter_validate_encoded_traversal_returns_false(#[case] input: &str) {
	// Arrange
	let conv = PathConverter;

	// Act
	let result = conv.validate(input);

	// Assert
	assert!(!result, "Expected encoded path '{}' to be rejected", input);
}

#[rstest]
fn path_converter_convert_valid() {
	// Arrange
	let conv = PathConverter;

	// Act
	let result = conv.convert("documents/report.pdf").unwrap();

	// Assert
	assert_eq!(result, "documents/report.pdf");
}

#[rstest]
fn path_converter_convert_empty_returns_error() {
	// Arrange
	let conv = PathConverter;

	// Act
	let result = conv.convert("");

	// Assert
	assert!(result.is_err());
	assert!(matches!(
		result.unwrap_err(),
		ConverterError::InvalidFormat(_)
	));
}

#[rstest]
fn path_converter_convert_traversal_returns_error() {
	// Arrange
	let conv = PathConverter;

	// Act
	let result = conv.convert("../etc/passwd");

	// Assert
	assert!(result.is_err());
	assert!(matches!(
		result.unwrap_err(),
		ConverterError::InvalidFormat(_)
	));
}

#[rstest]
fn path_converter_pattern() {
	// Arrange
	let conv = PathConverter;

	// Act
	let pattern = conv.pattern();

	// Assert
	assert_eq!(pattern, r"[^/\0]+(?:/[^/\0]+)*");
}

// ===================================================================
// FloatConverter tests
// ===================================================================

#[rstest]
#[case("123.45")]
#[case("-67.89")]
#[case("0.0")]
#[case("3.14159")]
#[case("100")]
#[case("-200")]
#[case("0")]
fn float_converter_validate_valid(#[case] input: &str) {
	// Arrange
	let conv = FloatConverter::new();

	// Act
	let result = conv.validate(input);

	// Assert
	assert!(result, "Expected '{}' to be a valid float", input);
}

#[rstest]
#[case("abc")]
#[case("12.34.56")]
#[case("")]
#[case("inf")]
#[case("nan")]
#[case("NaN")]
#[case("Inf")]
fn float_converter_validate_invalid(#[case] input: &str) {
	// Arrange
	let conv = FloatConverter::new();

	// Act
	let result = conv.validate(input);

	// Assert
	assert!(!result, "Expected '{}' to be an invalid float", input);
}

#[rstest]
fn float_converter_convert_valid() {
	// Arrange
	let conv = FloatConverter::new();

	// Act
	let result = conv.convert("3.14159").unwrap();

	// Assert
	assert!((result - 3.14159).abs() < 1e-6);
}

#[rstest]
fn float_converter_convert_negative() {
	// Arrange
	let conv = FloatConverter::new();

	// Act
	let result = conv.convert("-67.89").unwrap();

	// Assert
	assert!((result - (-67.89)).abs() < 1e-6);
}

#[rstest]
fn float_converter_convert_integer_string() {
	// Arrange
	let conv = FloatConverter::new();

	// Act
	let result = conv.convert("100").unwrap();

	// Assert
	assert_eq!(result, 100.0f64);
}

#[rstest]
fn float_converter_convert_invalid_returns_error() {
	// Arrange
	let conv = FloatConverter::new();

	// Act
	let result = conv.convert("not-a-float");

	// Assert
	assert!(result.is_err());
	assert!(matches!(
		result.unwrap_err(),
		ConverterError::InvalidFormat(_)
	));
}

#[rstest]
fn float_converter_convert_inf_returns_error() {
	// Arrange
	let conv = FloatConverter::new();

	// Act
	let result = conv.convert("inf");

	// Assert
	assert!(result.is_err());
}

#[rstest]
fn float_converter_with_range_valid() {
	// Arrange
	let conv = FloatConverter::with_range(0.0, 100.0);

	// Act
	let results = [
		conv.validate("0.0"),
		conv.validate("50.5"),
		conv.validate("100.0"),
		conv.validate("0.001"),
		conv.validate("99.999"),
	];

	// Assert
	assert!(results.iter().all(|r| *r));
}

#[rstest]
fn float_converter_with_range_out_of_range() {
	// Arrange
	let conv = FloatConverter::with_range(0.0, 100.0);

	// Act
	let results = [
		conv.validate("150.5"),
		conv.validate("-10.0"),
		conv.validate("100.1"),
		conv.validate("-0.001"),
	];

	// Assert
	assert!(results.iter().all(|r| !*r));
}

#[rstest]
fn float_converter_with_range_convert_out_of_range_returns_error() {
	// Arrange
	let conv = FloatConverter::with_range(0.0, 100.0);

	// Act
	let result_below = conv.convert("-10.0");
	let result_above = conv.convert("150.5");

	// Assert
	assert!(matches!(
		result_below.unwrap_err(),
		ConverterError::OutOfRange(_)
	));
	assert!(matches!(
		result_above.unwrap_err(),
		ConverterError::OutOfRange(_)
	));
}

#[rstest]
fn float_converter_pattern() {
	// Arrange
	let conv = FloatConverter::new();

	// Act
	let pattern = conv.pattern();

	// Assert
	assert_eq!(pattern, r"-?\d+\.?\d*");
}

#[rstest]
fn float_converter_default_equals_new() {
	// Arrange
	let conv_default = FloatConverter::default();
	let conv_new = FloatConverter::new();

	// Act
	let result_default = conv_default.validate("42.0");
	let result_new = conv_new.validate("42.0");

	// Assert
	assert!(result_default);
	assert!(result_new);
}

// ===================================================================
// ConverterError tests
// ===================================================================

#[rstest]
fn converter_error_invalid_format_display() {
	// Arrange
	let err = ConverterError::InvalidFormat("bad input".to_string());

	// Act
	let msg = err.to_string();

	// Assert
	assert!(msg.contains("bad input"));
}

#[rstest]
fn converter_error_out_of_range_display() {
	// Arrange
	let err = ConverterError::OutOfRange("value 200 exceeds max 100".to_string());

	// Act
	let msg = err.to_string();

	// Assert
	assert!(msg.contains("value 200 exceeds max 100"));
}

#[rstest]
fn converter_error_equality() {
	// Arrange
	let err1 = ConverterError::InvalidFormat("same".to_string());
	let err2 = ConverterError::InvalidFormat("same".to_string());
	let err3 = ConverterError::InvalidFormat("different".to_string());

	// Act
	let eq_same = err1 == err2;
	let eq_diff = err1 == err3;

	// Assert
	assert!(eq_same);
	assert!(!eq_diff);
}

#[rstest]
fn converter_error_different_variants_not_equal() {
	// Arrange
	let invalid_format = ConverterError::InvalidFormat("msg".to_string());
	let out_of_range = ConverterError::OutOfRange("msg".to_string());

	// Act
	let result = invalid_format == out_of_range;

	// Assert
	assert!(!result);
}

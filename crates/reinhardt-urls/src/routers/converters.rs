//! Path parameter converters for type-specific validation and conversion.
//!
//! This module provides converters for common path parameter types:
//! - `IntegerConverter`: Validates and converts integer path parameters
//! - `UuidConverter`: Validates and converts UUID path parameters
//! - `SlugConverter`: Validates slug format (lowercase alphanumeric + hyphens)
//! - `DateConverter`: Validates and converts date parameters (YYYY-MM-DD)
//! - `PathConverter`: Validates and converts path parameters (with security checks)
//! - `FloatConverter`: Validates and converts floating-point parameters
//!
//! # Examples
//!
//! ```
//! use reinhardt_urls::routers::converters::{Converter, IntegerConverter, UuidConverter, SlugConverter};
//!
//! // Integer converter
//! let int_conv = IntegerConverter::new();
//! assert!(int_conv.validate("123"));
//! assert!(!int_conv.validate("abc"));
//!
//! // UUID converter
//! let uuid_conv = UuidConverter;
//! assert!(uuid_conv.validate("550e8400-e29b-41d4-a716-446655440000"));
//! assert!(!uuid_conv.validate("not-a-uuid"));
//!
//! // Slug converter
//! let slug_conv = SlugConverter;
//! assert!(slug_conv.validate("my-blog-post"));
//! assert!(!slug_conv.validate("Invalid Slug!"));
//! ```

use chrono::NaiveDate;
use regex::Regex;
use std::sync::OnceLock;
use thiserror::Error;

/// Error type for converter validation failures
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ConverterError {
	#[error("Invalid format: {0}")]
	InvalidFormat(String),
	#[error("Value out of range: {0}")]
	OutOfRange(String),
}

/// Result type for converter operations
pub type ConverterResult<T> = Result<T, ConverterError>;

/// Trait for path parameter converters
///
/// Converters validate and optionally transform path parameters
/// before they are used in route handlers.
pub trait Converter: Send + Sync {
	/// The target type this converter produces
	type Output;

	/// Validate a path parameter value
	///
	/// Returns `true` if the value is valid for this converter.
	fn validate(&self, value: &str) -> bool;

	/// Convert a validated path parameter to the target type
	///
	/// # Errors
	///
	/// Returns `ConverterError` if the value cannot be converted.
	fn convert(&self, value: &str) -> ConverterResult<Self::Output>;

	/// Get the regex pattern for this converter
	///
	/// Used for route pattern matching.
	fn pattern(&self) -> &str;
}

/// Integer converter with optional range validation
///
/// Validates that path parameters are valid integers, optionally
/// within a specified range.
///
/// # Examples
///
/// ```
/// use reinhardt_urls::routers::converters::{Converter, IntegerConverter};
///
/// // Without range limits
/// let conv = IntegerConverter::new();
/// assert!(conv.validate("123"));
/// assert!(conv.validate("-456"));
/// assert!(!conv.validate("abc"));
///
/// // With range limits
/// let conv = IntegerConverter::with_range(1, 100);
/// assert!(conv.validate("50"));
/// assert!(!conv.validate("150")); // Out of range
/// ```
#[derive(Debug, Clone)]
pub struct IntegerConverter {
	min: Option<i64>,
	max: Option<i64>,
}

impl IntegerConverter {
	/// Create a new integer converter without range limits
	pub fn new() -> Self {
		Self {
			min: None,
			max: None,
		}
	}

	/// Create an integer converter with range limits
	///
	/// # Arguments
	///
	/// * `min` - Minimum allowed value (inclusive)
	/// * `max` - Maximum allowed value (inclusive)
	pub fn with_range(min: i64, max: i64) -> Self {
		Self {
			min: Some(min),
			max: Some(max),
		}
	}
}

impl Default for IntegerConverter {
	fn default() -> Self {
		Self::new()
	}
}

impl Converter for IntegerConverter {
	type Output = i64;

	fn validate(&self, value: &str) -> bool {
		if let Ok(num) = value.parse::<i64>() {
			if let Some(min) = self.min
				&& num < min
			{
				return false;
			}
			if let Some(max) = self.max
				&& num > max
			{
				return false;
			}
			true
		} else {
			false
		}
	}

	fn convert(&self, value: &str) -> ConverterResult<Self::Output> {
		let num = value.parse::<i64>().map_err(|_| {
			ConverterError::InvalidFormat(format!("'{}' is not a valid integer", value))
		})?;

		if let Some(min) = self.min
			&& num < min
		{
			return Err(ConverterError::OutOfRange(format!(
				"{} is less than minimum {}",
				num, min
			)));
		}

		if let Some(max) = self.max
			&& num > max
		{
			return Err(ConverterError::OutOfRange(format!(
				"{} is greater than maximum {}",
				num, max
			)));
		}

		Ok(num)
	}

	fn pattern(&self) -> &str {
		r"-?\d+"
	}
}

/// UUID converter (version 4)
///
/// Validates that path parameters are valid UUIDs.
///
/// # Examples
///
/// ```
/// use reinhardt_urls::routers::converters::{Converter, UuidConverter};
///
/// let conv = UuidConverter;
/// assert!(conv.validate("550e8400-e29b-41d4-a716-446655440000"));
/// assert!(conv.validate("6ba7b810-9dad-11d1-80b4-00c04fd430c8"));
/// assert!(!conv.validate("not-a-uuid"));
/// assert!(!conv.validate("550e8400-e29b-41d4-a716")); // Invalid format
/// ```
#[derive(Debug, Clone, Copy)]
pub struct UuidConverter;

impl UuidConverter {
	fn regex() -> &'static Regex {
		static REGEX: OnceLock<Regex> = OnceLock::new();
		REGEX.get_or_init(|| {
			Regex::new(r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")
				.expect("Invalid UUID regex pattern")
		})
	}
}

impl Converter for UuidConverter {
	type Output = String;

	fn validate(&self, value: &str) -> bool {
		Self::regex().is_match(value)
	}

	fn convert(&self, value: &str) -> ConverterResult<Self::Output> {
		if self.validate(value) {
			Ok(value.to_string())
		} else {
			Err(ConverterError::InvalidFormat(format!(
				"'{}' is not a valid UUID",
				value
			)))
		}
	}

	fn pattern(&self) -> &str {
		r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}"
	}
}

/// Slug converter
///
/// Validates that path parameters are valid slugs (lowercase alphanumeric
/// characters and hyphens).
///
/// # Examples
///
/// ```
/// use reinhardt_urls::routers::converters::{Converter, SlugConverter};
///
/// let conv = SlugConverter;
/// assert!(conv.validate("my-blog-post"));
/// assert!(conv.validate("article-123"));
/// assert!(conv.validate("hello-world"));
/// assert!(!conv.validate("Invalid Slug!"));
/// assert!(!conv.validate("no_underscores"));
/// assert!(!conv.validate("NO-UPPERCASE"));
/// ```
#[derive(Debug, Clone, Copy)]
pub struct SlugConverter;

impl SlugConverter {
	fn regex() -> &'static Regex {
		static REGEX: OnceLock<Regex> = OnceLock::new();
		REGEX.get_or_init(|| {
			Regex::new(r"^[a-z0-9]+(-[a-z0-9]+)*$").expect("Invalid slug regex pattern")
		})
	}
}

impl Converter for SlugConverter {
	type Output = String;

	fn validate(&self, value: &str) -> bool {
		Self::regex().is_match(value)
	}

	fn convert(&self, value: &str) -> ConverterResult<Self::Output> {
		if self.validate(value) {
			Ok(value.to_string())
		} else {
			Err(ConverterError::InvalidFormat(format!(
				"'{}' is not a valid slug (must be lowercase alphanumeric with hyphens)",
				value
			)))
		}
	}

	fn pattern(&self) -> &str {
		r"[a-z0-9]+(-[a-z0-9]+)*"
	}
}

/// Date converter for ISO 8601 dates (YYYY-MM-DD)
///
/// Validates that path parameters are valid dates in YYYY-MM-DD format
/// and converts them to `chrono::NaiveDate`.
///
/// # Examples
///
/// ```
/// use reinhardt_urls::routers::converters::{Converter, DateConverter};
/// use chrono::Datelike;
///
/// let conv = DateConverter;
/// assert!(conv.validate("2024-01-15"));
/// assert!(conv.validate("2023-12-31"));
/// assert!(!conv.validate("2024-13-01")); // Invalid month
/// assert!(!conv.validate("2024-01-32")); // Invalid day
/// assert!(!conv.validate("24-01-15")); // Wrong format
/// assert!(!conv.validate("not-a-date"));
///
/// // Convert to NaiveDate
/// let date = conv.convert("2024-01-15").unwrap();
/// assert_eq!(date.year(), 2024);
/// assert_eq!(date.month(), 1);
/// assert_eq!(date.day(), 15);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct DateConverter;

impl DateConverter {
	fn regex() -> &'static Regex {
		static REGEX: OnceLock<Regex> = OnceLock::new();
		REGEX
			.get_or_init(|| Regex::new(r"^\d{4}-\d{2}-\d{2}$").expect("Invalid date regex pattern"))
	}
}

impl Converter for DateConverter {
	type Output = NaiveDate;

	fn validate(&self, value: &str) -> bool {
		if !Self::regex().is_match(value) {
			return false;
		}
		NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok()
	}

	fn convert(&self, value: &str) -> ConverterResult<Self::Output> {
		NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
			ConverterError::InvalidFormat(format!(
				"'{}' is not a valid date (expected YYYY-MM-DD format)",
				value
			))
		})
	}

	fn pattern(&self) -> &str {
		r"\d{4}-\d{2}-\d{2}"
	}
}

/// Path converter for file paths with security validation
///
/// Validates that path parameters are safe file paths without
/// directory traversal attempts (e.g., `../`) and converts them to `String`.
///
/// # Security
///
/// This converter prevents path traversal attacks by rejecting:
/// - Paths containing `../` (parent directory references)
/// - Paths containing `..` at the start or end
/// - Paths with null bytes
///
/// # Examples
///
/// ```
/// use reinhardt_urls::routers::converters::{Converter, PathConverter};
///
/// let conv = PathConverter;
/// assert!(conv.validate("path/to/file.txt"));
/// assert!(conv.validate("images/photo.jpg"));
/// assert!(conv.validate("documents/2024/report.pdf"));
/// assert!(!conv.validate("../etc/passwd")); // Directory traversal
/// assert!(!conv.validate("path/../secret")); // Directory traversal
/// assert!(!conv.validate("path/to/../../file")); // Directory traversal
///
/// // Convert to String
/// let path = conv.convert("documents/report.pdf").unwrap();
/// assert_eq!(path, "documents/report.pdf");
/// ```
#[derive(Debug, Clone, Copy)]
pub struct PathConverter;

impl PathConverter {
	/// Check if a path contains directory traversal attempts.
	///
	/// Rejects:
	/// - Null bytes (literal or percent-encoded `%00`)
	/// - `..` segments (forward-slash or backslash separated)
	/// - Percent-encoded traversal sequences (`%2e`, `%2f`, `%5c`)
	/// - Backslash path separators
	/// - Absolute paths starting with `/` or `\`
	fn is_safe_path(path: &str) -> bool {
		// Reject null bytes
		if path.contains('\0') {
			return false;
		}

		// Reject percent-encoded dangerous characters
		let lower = path.to_ascii_lowercase();
		if lower.contains("%2e")
			|| lower.contains("%2f")
			|| lower.contains("%5c")
			|| lower.contains("%00")
		{
			return false;
		}

		// Reject backslash path separators (Windows-style)
		if path.contains('\\') {
			return false;
		}

		// Reject absolute paths
		if path.starts_with('/') {
			return false;
		}

		// Check for `..` as a complete path segment
		for segment in path.split('/') {
			if segment == ".." {
				return false;
			}
		}

		true
	}
}

impl Converter for PathConverter {
	type Output = String;

	fn validate(&self, value: &str) -> bool {
		!value.is_empty() && Self::is_safe_path(value)
	}

	fn convert(&self, value: &str) -> ConverterResult<Self::Output> {
		if value.is_empty() {
			return Err(ConverterError::InvalidFormat(
				"Path cannot be empty".to_string(),
			));
		}

		if !Self::is_safe_path(value) {
			return Err(ConverterError::InvalidFormat(format!(
				"'{}' contains invalid path components (possible directory traversal attempt)",
				value
			)));
		}

		Ok(value.to_string())
	}

	fn pattern(&self) -> &str {
		r"[^/\0]+(?:/[^/\0]+)*"
	}
}

/// Float converter with optional range validation
///
/// Validates that path parameters are valid floating-point numbers,
/// optionally within a specified range.
///
/// # Examples
///
/// ```
/// use reinhardt_urls::routers::converters::{Converter, FloatConverter};
///
/// // Without range limits
/// let conv = FloatConverter::new();
/// assert!(conv.validate("123.45"));
/// assert!(conv.validate("-67.89"));
/// assert!(conv.validate("0.0"));
/// assert!(conv.validate("3.14159"));
/// assert!(!conv.validate("abc"));
/// assert!(!conv.validate("12.34.56")); // Invalid format
///
/// // With range limits
/// let conv = FloatConverter::with_range(0.0, 100.0);
/// assert!(conv.validate("50.5"));
/// assert!(conv.validate("0.0"));
/// assert!(conv.validate("100.0"));
/// assert!(!conv.validate("150.5")); // Out of range
/// assert!(!conv.validate("-10.0")); // Out of range
///
/// // Convert to f64
/// let value = FloatConverter::new().convert("3.14159").unwrap();
/// assert!((value - 3.14159).abs() < 1e-6);
/// ```
#[derive(Debug, Clone)]
pub struct FloatConverter {
	min: Option<f64>,
	max: Option<f64>,
}

impl FloatConverter {
	/// Create a new float converter without range limits
	pub fn new() -> Self {
		Self {
			min: None,
			max: None,
		}
	}

	/// Create a float converter with range limits
	///
	/// # Arguments
	///
	/// * `min` - Minimum allowed value (inclusive)
	/// * `max` - Maximum allowed value (inclusive)
	pub fn with_range(min: f64, max: f64) -> Self {
		Self {
			min: Some(min),
			max: Some(max),
		}
	}
}

impl Default for FloatConverter {
	fn default() -> Self {
		Self::new()
	}
}

impl Converter for FloatConverter {
	type Output = f64;

	fn validate(&self, value: &str) -> bool {
		if let Ok(num) = value.parse::<f64>() {
			if !num.is_finite() {
				return false;
			}
			if let Some(min) = self.min
				&& num < min
			{
				return false;
			}
			if let Some(max) = self.max
				&& num > max
			{
				return false;
			}
			true
		} else {
			false
		}
	}

	fn convert(&self, value: &str) -> ConverterResult<Self::Output> {
		let num = value.parse::<f64>().map_err(|_| {
			ConverterError::InvalidFormat(format!(
				"'{}' is not a valid floating-point number",
				value
			))
		})?;

		if !num.is_finite() {
			return Err(ConverterError::InvalidFormat(format!(
				"'{}' is not a finite number",
				value
			)));
		}

		if let Some(min) = self.min
			&& num < min
		{
			return Err(ConverterError::OutOfRange(format!(
				"{} is less than minimum {}",
				num, min
			)));
		}

		if let Some(max) = self.max
			&& num > max
		{
			return Err(ConverterError::OutOfRange(format!(
				"{} is greater than maximum {}",
				num, max
			)));
		}

		Ok(num)
	}

	fn pattern(&self) -> &str {
		r"-?\d+\.?\d*"
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use chrono::Datelike;

	#[test]
	fn test_integer_converter_basic() {
		let conv = IntegerConverter::new();

		// Valid integers
		assert!(conv.validate("123"));
		assert!(conv.validate("-456"));
		assert!(conv.validate("0"));

		// Invalid values
		assert!(!conv.validate("abc"));
		assert!(!conv.validate("12.5"));
		assert!(!conv.validate(""));
	}

	#[test]
	fn test_integer_converter_with_range() {
		let conv = IntegerConverter::with_range(1, 100);

		// Within range
		assert!(conv.validate("50"));
		assert!(conv.validate("1"));
		assert!(conv.validate("100"));

		// Out of range
		assert!(!conv.validate("0"));
		assert!(!conv.validate("101"));
		assert!(!conv.validate("-10"));
	}

	#[test]
	fn test_integer_converter_convert() {
		let conv = IntegerConverter::new();

		assert_eq!(conv.convert("123").unwrap(), 123);
		assert_eq!(conv.convert("-456").unwrap(), -456);
		assert!(conv.convert("abc").is_err());
	}

	#[test]
	fn test_integer_converter_convert_with_range() {
		let conv = IntegerConverter::with_range(1, 100);

		assert_eq!(conv.convert("50").unwrap(), 50);
		assert!(conv.convert("0").is_err());
		assert!(conv.convert("101").is_err());
	}

	#[test]
	fn test_uuid_converter() {
		let conv = UuidConverter;

		// Valid UUIDs
		assert!(conv.validate("550e8400-e29b-41d4-a716-446655440000"));
		assert!(conv.validate("6ba7b810-9dad-11d1-80b4-00c04fd430c8"));

		// Invalid UUIDs
		assert!(!conv.validate("not-a-uuid"));
		assert!(!conv.validate("550e8400-e29b-41d4-a716")); // Too short
		assert!(!conv.validate("550e8400-e29b-41d4-a716-446655440000-extra")); // Too long
		assert!(!conv.validate("550E8400-E29B-41D4-A716-446655440000")); // Uppercase
	}

	#[test]
	fn test_uuid_converter_convert() {
		let conv = UuidConverter;

		let uuid = "550e8400-e29b-41d4-a716-446655440000";
		assert_eq!(conv.convert(uuid).unwrap(), uuid);
		assert!(conv.convert("not-a-uuid").is_err());
	}

	#[test]
	fn test_slug_converter() {
		let conv = SlugConverter;

		// Valid slugs
		assert!(conv.validate("my-blog-post"));
		assert!(conv.validate("article-123"));
		assert!(conv.validate("hello-world"));
		assert!(conv.validate("simple"));

		// Invalid slugs
		assert!(!conv.validate("Invalid Slug!"));
		assert!(!conv.validate("no_underscores"));
		assert!(!conv.validate("NO-UPPERCASE"));
		assert!(!conv.validate("-starts-with-hyphen"));
		assert!(!conv.validate("ends-with-hyphen-"));
		assert!(!conv.validate("double--hyphens"));
	}

	#[test]
	fn test_slug_converter_convert() {
		let conv = SlugConverter;

		assert_eq!(conv.convert("my-blog-post").unwrap(), "my-blog-post");
		assert!(conv.convert("Invalid Slug!").is_err());
	}

	#[test]
	fn test_converter_patterns() {
		let int_conv = IntegerConverter::new();
		let uuid_conv = UuidConverter;
		let slug_conv = SlugConverter;

		assert_eq!(int_conv.pattern(), r"-?\d+");
		assert_eq!(
			uuid_conv.pattern(),
			r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}"
		);
		assert_eq!(slug_conv.pattern(), r"[a-z0-9]+(-[a-z0-9]+)*");
	}

	#[test]
	fn test_date_converter_validation() {
		let conv = DateConverter;

		// Valid dates
		assert!(conv.validate("2024-01-15"));
		assert!(conv.validate("2023-12-31"));
		assert!(conv.validate("2000-02-29")); // Leap year

		// Invalid dates
		assert!(!conv.validate("2024-13-01")); // Invalid month
		assert!(!conv.validate("2024-01-32")); // Invalid day
		assert!(!conv.validate("2023-02-29")); // Not a leap year
		assert!(!conv.validate("24-01-15")); // Wrong format
		assert!(!conv.validate("2024/01/15")); // Wrong separator
		assert!(!conv.validate("not-a-date"));
		assert!(!conv.validate(""));
	}

	#[test]
	fn test_date_converter_convert() {
		let conv = DateConverter;

		// Valid conversion
		let date = conv.convert("2024-01-15").unwrap();
		assert_eq!(date.year(), 2024);
		assert_eq!(date.month(), 1);
		assert_eq!(date.day(), 15);

		// Another valid date
		let date = conv.convert("2023-12-31").unwrap();
		assert_eq!(date.year(), 2023);
		assert_eq!(date.month(), 12);
		assert_eq!(date.day(), 31);

		// Invalid dates
		assert!(conv.convert("2024-13-01").is_err());
		assert!(conv.convert("not-a-date").is_err());
	}

	#[test]
	fn test_path_converter_validation() {
		let conv = PathConverter;

		// Valid paths
		assert!(conv.validate("path/to/file.txt"));
		assert!(conv.validate("images/photo.jpg"));
		assert!(conv.validate("documents/2024/report.pdf"));
		assert!(conv.validate("simple.txt"));
		assert!(conv.validate("a/b/c/d/e.txt"));

		// Invalid paths - directory traversal
		assert!(!conv.validate("../etc/passwd"));
		assert!(!conv.validate("path/../secret"));
		assert!(!conv.validate("path/to/../../file"));
		assert!(!conv.validate(".."));
		assert!(!conv.validate("path/.."));
		assert!(!conv.validate("../path"));

		// Empty path
		assert!(!conv.validate(""));

		// Null bytes
		assert!(!conv.validate("path\0/file"));
	}

	#[test]
	fn test_path_converter_convert() {
		let conv = PathConverter;

		// Valid conversions
		assert_eq!(
			conv.convert("documents/report.pdf").unwrap(),
			"documents/report.pdf"
		);
		assert_eq!(conv.convert("file.txt").unwrap(), "file.txt");

		// Invalid paths
		assert!(conv.convert("../etc/passwd").is_err());
		assert!(conv.convert("path/../file").is_err());
		assert!(conv.convert("").is_err());
	}

	#[test]
	fn test_float_converter_basic() {
		let conv = FloatConverter::new();

		// Valid floats
		assert!(conv.validate("123.45"));
		assert!(conv.validate("-67.89"));
		assert!(conv.validate("0.0"));
		assert!(conv.validate("3.14159"));
		assert!(conv.validate("100"));
		assert!(conv.validate("-200"));

		// Invalid values
		assert!(!conv.validate("abc"));
		assert!(!conv.validate("12.34.56"));
		assert!(!conv.validate(""));
		assert!(!conv.validate("inf"));
		assert!(!conv.validate("nan"));
	}

	#[test]
	fn test_float_converter_with_range() {
		let conv = FloatConverter::with_range(0.0, 100.0);

		// Within range
		assert!(conv.validate("50.5"));
		assert!(conv.validate("0.0"));
		assert!(conv.validate("100.0"));
		assert!(conv.validate("0.001"));
		assert!(conv.validate("99.999"));

		// Out of range
		assert!(!conv.validate("150.5"));
		assert!(!conv.validate("-10.0"));
		assert!(!conv.validate("100.1"));
		assert!(!conv.validate("-0.001"));
	}

	#[test]
	fn test_float_converter_convert() {
		let conv = FloatConverter::new();

		// Valid conversions
		let value = conv.convert("3.17").unwrap();
		assert!((value - 3.17).abs() < 1e-6);

		let value = conv.convert("-67.89").unwrap();
		assert!((value - (-67.89)).abs() < 1e-6);

		let value = conv.convert("100").unwrap();
		assert_eq!(value, 100.0);

		// Invalid values
		assert!(conv.convert("abc").is_err());
		assert!(conv.convert("inf").is_err());
	}

	#[test]
	fn test_float_converter_convert_with_range() {
		let conv = FloatConverter::with_range(0.0, 100.0);

		// Within range
		assert_eq!(conv.convert("50.5").unwrap(), 50.5);
		assert_eq!(conv.convert("0.0").unwrap(), 0.0);
		assert_eq!(conv.convert("100.0").unwrap(), 100.0);

		// Out of range
		assert!(conv.convert("150.5").is_err());
		assert!(conv.convert("-10.0").is_err());
	}

	#[test]
	fn test_new_converter_patterns() {
		let date_conv = DateConverter;
		let path_conv = PathConverter;
		let float_conv = FloatConverter::new();

		assert_eq!(date_conv.pattern(), r"\d{4}-\d{2}-\d{2}");
		assert_eq!(path_conv.pattern(), r"[^/\0]+(?:/[^/\0]+)*");
		assert_eq!(float_conv.pattern(), r"-?\d+\.?\d*");
	}

	// ===================================================================
	// PathConverter encoded traversal prevention tests (Issue #425)
	// ===================================================================

	#[test]
	fn test_path_converter_rejects_encoded_traversal() {
		// Arrange
		let conv = PathConverter;

		// Act & Assert - percent-encoded dot sequences
		assert!(!conv.validate("%2e%2e/etc/passwd"));
		assert!(!conv.validate("foo/%2e%2e/bar"));
		assert!(!conv.validate("%2E%2E/secret"));

		// Percent-encoded slash
		assert!(!conv.validate("foo%2fbar"));
		assert!(!conv.validate("..%2f..%2fetc%2fpasswd"));

		// Percent-encoded backslash
		assert!(!conv.validate("foo%5cbar"));

		// Percent-encoded null byte
		assert!(!conv.validate("file%00.txt"));
	}

	#[test]
	fn test_path_converter_rejects_backslash() {
		// Arrange
		let conv = PathConverter;

		// Act & Assert
		assert!(!conv.validate("path\\to\\file"));
		assert!(!conv.validate("..\\etc\\passwd"));
	}

	#[test]
	fn test_path_converter_rejects_absolute_paths() {
		// Arrange
		let conv = PathConverter;

		// Act & Assert
		assert!(!conv.validate("/etc/passwd"));
	}

	#[test]
	fn test_path_converter_convert_rejects_encoded() {
		// Arrange
		let conv = PathConverter;

		// Act & Assert
		assert!(conv.convert("%2e%2e/etc/passwd").is_err());
		assert!(conv.convert("foo%2fbar").is_err());
		assert!(conv.convert("foo%5cbar").is_err());
	}
}

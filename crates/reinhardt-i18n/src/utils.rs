//! Utility functions for internationalization

use chrono::{DateTime, Utc};

/// Maximum length for date format strings to prevent CPU exhaustion
/// from pathological strftime patterns
const MAX_FORMAT_LEN: usize = 128;

/// Format a date according to the current locale
///
/// # Example
/// ```
/// use reinhardt_i18n::utils::format_date;
/// use chrono::Utc;
///
/// let now = Utc::now();
/// let formatted = format_date(&now, "%Y-%m-%d");
/// assert!(!formatted.is_empty());
/// ```
pub fn format_date(date: &DateTime<Utc>, format: &str) -> String {
	if format.len() > MAX_FORMAT_LEN {
		// Fall back to a safe default format for overly long format strings
		return date.format("%Y-%m-%d").to_string();
	}
	date.format(format).to_string()
}

/// Format a number according to the current locale
///
/// Handles negative numbers correctly by stripping the sign before
/// applying thousand-separator logic, then prepending it back.
///
/// # Example
/// ```
/// use reinhardt_i18n::utils::format_number;
///
/// let formatted = format_number(1234567.89, 2);
/// assert_eq!(formatted, "1,234,567.89");
///
/// let negative = format_number(-123456.78, 2);
/// assert_eq!(negative, "-123,456.78");
/// ```
pub fn format_number(number: f64, decimal_places: usize) -> String {
	// Handle IEEE 754 special values explicitly
	if number.is_nan() {
		return "NaN".to_string();
	}
	if number.is_infinite() {
		return if number.is_sign_positive() {
			"\u{221E}".to_string()
		} else {
			"-\u{221E}".to_string()
		};
	}

	let is_negative = number.is_sign_negative() && number != 0.0;
	let abs_value = number.abs();
	let formatted = format!("{:.1$}", abs_value, decimal_places);

	// Add thousand separators
	let parts: Vec<&str> = formatted.split('.').collect();
	let integer_part = parts[0];
	let decimal_part = if parts.len() > 1 { parts[1] } else { "" };

	let mut result = String::new();
	if is_negative {
		result.push('-');
	}

	let chars: Vec<char> = integer_part.chars().collect();
	for (i, ch) in chars.iter().enumerate() {
		if i > 0 && (chars.len() - i).is_multiple_of(3) {
			result.push(',');
		}
		result.push(*ch);
	}

	if !decimal_part.is_empty() {
		result.push('.');
		result.push_str(decimal_part);
	}

	result
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	#[case(1234.56, 2, "1,234.56")]
	#[case(1000000.0, 0, "1,000,000")]
	#[case(42.0, 2, "42.00")]
	#[case(0.0, 2, "0.00")]
	#[case(999.99, 2, "999.99")]
	fn test_format_number_positive(
		#[case] number: f64,
		#[case] decimals: usize,
		#[case] expected: &str,
	) {
		// Arrange (inputs from parametrized case)

		// Act
		let result = format_number(number, decimals);

		// Assert
		assert_eq!(result, expected);
	}

	#[rstest]
	#[case(-123456.78, 2, "-123,456.78")]
	#[case(-1.0, 0, "-1")]
	#[case(-1000.0, 0, "-1,000")]
	#[case(-0.5, 2, "-0.50")]
	#[case(-1234567.89, 2, "-1,234,567.89")]
	fn test_format_number_negative(
		#[case] number: f64,
		#[case] decimals: usize,
		#[case] expected: &str,
	) {
		// Arrange (inputs from parametrized case)

		// Act
		let result = format_number(number, decimals);

		// Assert
		assert_eq!(result, expected);
	}

	#[rstest]
	fn test_format_number_nan() {
		// Arrange
		let number = f64::NAN;

		// Act
		let result = format_number(number, 2);

		// Assert
		assert_eq!(result, "NaN");
	}

	#[rstest]
	fn test_format_number_positive_infinity() {
		// Arrange
		let number = f64::INFINITY;

		// Act
		let result = format_number(number, 2);

		// Assert
		assert_eq!(result, "\u{221E}");
	}

	#[rstest]
	fn test_format_number_negative_infinity() {
		// Arrange
		let number = f64::NEG_INFINITY;

		// Act
		let result = format_number(number, 2);

		// Assert
		assert_eq!(result, "-\u{221E}");
	}

	#[rstest]
	fn test_format_number_negative_zero() {
		// Arrange: negative zero should be formatted as positive zero
		let number = -0.0;

		// Act
		let result = format_number(number, 2);

		// Assert
		assert_eq!(result, "0.00");
	}

	#[rstest]
	fn test_format_date() {
		// Arrange
		let date = Utc::now();

		// Act
		let formatted = format_date(&date, "%Y-%m-%d");

		// Assert
		assert!(!formatted.is_empty());
	}

	#[rstest]
	fn test_format_date_rejects_long_format_string() {
		// Arrange: format string exceeding MAX_FORMAT_LEN (128)
		let date = Utc::now();
		let long_format = "%Y-".repeat(50); // 200 characters

		// Act
		let formatted = format_date(&date, &long_format);

		// Assert: should fall back to safe default "%Y-%m-%d"
		assert_eq!(formatted, date.format("%Y-%m-%d").to_string());
	}

	#[rstest]
	fn test_format_date_accepts_max_length_format() {
		// Arrange: format string exactly at MAX_FORMAT_LEN (128)
		let date = Utc::now();
		let format = "a".repeat(128);

		// Act: should not fall back (128 == MAX_FORMAT_LEN, not exceeded)
		let result = format_date(&date, &format);

		// Assert: should not be the fallback format
		// (the output will be the literal characters since they are not format specifiers)
		assert_eq!(result, format);
	}
}

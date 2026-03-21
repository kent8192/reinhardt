//! String validators

use super::lazy_patterns::{SLUG_ASCII_REGEX, SLUG_UNICODE_REGEX, UUID_REGEX};
use super::{ValidationError, ValidationResult, Validator};
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use regex::Regex;

/// Minimum length validator
pub struct MinLengthValidator {
	min: usize,
	message: Option<String>,
}

impl MinLengthValidator {
	/// Creates a new MinLengthValidator with the specified minimum length.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::{MinLengthValidator, Validator};
	///
	/// let validator = MinLengthValidator::new(5);
	/// assert!(validator.validate("hello").is_ok());
	/// assert!(validator.validate("hi").is_err());
	/// ```
	pub fn new(min: usize) -> Self {
		Self { min, message: None }
	}

	/// Sets a custom error message for the validator.
	///
	/// When set, this message will be used instead of the default error message.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::{MinLengthValidator, Validator, ValidationError};
	///
	/// let validator = MinLengthValidator::new(5)
	///     .with_message("Username must be at least 5 characters");
	///
	/// let result = validator.validate("hi");
	/// assert!(result.is_err());
	/// if let Err(ValidationError::Custom(msg)) = result {
	///     assert_eq!(msg, "Username must be at least 5 characters");
	/// }
	/// ```
	pub fn with_message(mut self, message: impl Into<String>) -> Self {
		self.message = Some(message.into());
		self
	}
}

impl Validator<String> for MinLengthValidator {
	fn validate(&self, value: &String) -> ValidationResult<()> {
		let char_count = value.chars().count();
		if char_count >= self.min {
			Ok(())
		} else if let Some(ref msg) = self.message {
			Err(ValidationError::Custom(msg.clone()))
		} else {
			Err(ValidationError::TooShort {
				length: char_count,
				min: self.min,
			})
		}
	}
}

impl Validator<str> for MinLengthValidator {
	fn validate(&self, value: &str) -> ValidationResult<()> {
		let char_count = value.chars().count();
		if char_count >= self.min {
			Ok(())
		} else if let Some(ref msg) = self.message {
			Err(ValidationError::Custom(msg.clone()))
		} else {
			Err(ValidationError::TooShort {
				length: char_count,
				min: self.min,
			})
		}
	}
}

/// Maximum length validator
pub struct MaxLengthValidator {
	max: usize,
	message: Option<String>,
}

impl MaxLengthValidator {
	/// Creates a new MaxLengthValidator with the specified maximum length.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::{MaxLengthValidator, Validator};
	///
	/// let validator = MaxLengthValidator::new(10);
	/// assert!(validator.validate("hello").is_ok());
	/// assert!(validator.validate("hello world").is_err());
	/// ```
	pub fn new(max: usize) -> Self {
		Self { max, message: None }
	}

	/// Sets a custom error message for the validator.
	///
	/// When set, this message will be used instead of the default error message.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::{MaxLengthValidator, Validator, ValidationError};
	///
	/// let validator = MaxLengthValidator::new(10)
	///     .with_message("Username must be at most 10 characters");
	///
	/// let result = validator.validate("this is way too long");
	/// assert!(result.is_err());
	/// if let Err(ValidationError::Custom(msg)) = result {
	///     assert_eq!(msg, "Username must be at most 10 characters");
	/// }
	/// ```
	pub fn with_message(mut self, message: impl Into<String>) -> Self {
		self.message = Some(message.into());
		self
	}
}

impl Validator<String> for MaxLengthValidator {
	fn validate(&self, value: &String) -> ValidationResult<()> {
		let char_count = value.chars().count();
		if char_count <= self.max {
			Ok(())
		} else if let Some(ref msg) = self.message {
			Err(ValidationError::Custom(msg.clone()))
		} else {
			Err(ValidationError::TooLong {
				length: char_count,
				max: self.max,
			})
		}
	}
}

impl Validator<str> for MaxLengthValidator {
	fn validate(&self, value: &str) -> ValidationResult<()> {
		let char_count = value.chars().count();
		if char_count <= self.max {
			Ok(())
		} else if let Some(ref msg) = self.message {
			Err(ValidationError::Custom(msg.clone()))
		} else {
			Err(ValidationError::TooLong {
				length: char_count,
				max: self.max,
			})
		}
	}
}

/// Regex validator
pub struct RegexValidator {
	regex: Regex,
	message: String,
}

impl RegexValidator {
	/// Creates a new RegexValidator with the specified regex pattern.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::{RegexValidator, Validator};
	///
	/// let validator = RegexValidator::new(r"^\d{3}-\d{4}$").unwrap();
	/// assert!(validator.validate("123-4567").is_ok());
	/// assert!(validator.validate("invalid").is_err());
	/// ```
	pub fn new(pattern: &str) -> Result<Self, regex::Error> {
		Ok(Self {
			regex: Regex::new(pattern)?,
			message: format!("Value must match pattern: {}", pattern),
		})
	}
	/// Sets a custom error message for the validator.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::{RegexValidator, Validator};
	///
	/// let validator = RegexValidator::new(r"^\d+$")
	///     .unwrap()
	///     .with_message("Value must contain only digits");
	///
	/// assert!(validator.validate("12345").is_ok());
	/// assert!(validator.validate("abc").is_err());
	/// ```
	pub fn with_message(mut self, message: impl Into<String>) -> Self {
		self.message = message.into();
		self
	}
}

impl Validator<String> for RegexValidator {
	fn validate(&self, value: &String) -> ValidationResult<()> {
		if self.regex.is_match(value) {
			Ok(())
		} else {
			Err(ValidationError::PatternMismatch(self.message.clone()))
		}
	}
}

impl Validator<str> for RegexValidator {
	fn validate(&self, value: &str) -> ValidationResult<()> {
		if self.regex.is_match(value) {
			Ok(())
		} else {
			Err(ValidationError::PatternMismatch(self.message.clone()))
		}
	}
}

/// Slug validator - validates URL-safe slugs
///
/// Slugs can contain lowercase letters, numbers, hyphens, and underscores.
pub struct SlugValidator {
	allow_unicode: bool,
	message: Option<String>,
}

impl SlugValidator {
	/// Creates a new SlugValidator.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::{SlugValidator, Validator};
	///
	/// let validator = SlugValidator::new();
	/// assert!(validator.validate("my-valid-slug").is_ok());
	/// assert!(validator.validate("my_slug_123").is_ok());
	/// assert!(validator.validate("invalid slug").is_err());
	/// ```
	pub fn new() -> Self {
		Self {
			allow_unicode: false,
			message: None,
		}
	}

	/// Allows Unicode characters in the slug.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::{SlugValidator, Validator};
	///
	/// let validator = SlugValidator::new().allow_unicode(true);
	/// assert!(validator.validate("日本語-slug").is_ok());
	/// ```
	pub fn allow_unicode(mut self, allow: bool) -> Self {
		self.allow_unicode = allow;
		self
	}

	/// Sets a custom error message for the validator.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::{SlugValidator, Validator, ValidationError};
	///
	/// let validator = SlugValidator::new()
	///     .with_message("Invalid URL slug format");
	///
	/// let result = validator.validate("invalid slug!");
	/// assert!(result.is_err());
	/// ```
	pub fn with_message(mut self, message: impl Into<String>) -> Self {
		self.message = Some(message.into());
		self
	}
}

impl Default for SlugValidator {
	fn default() -> Self {
		Self::new()
	}
}

impl Validator<String> for SlugValidator {
	fn validate(&self, value: &String) -> ValidationResult<()> {
		self.validate(value.as_str())
	}
}

impl Validator<str> for SlugValidator {
	fn validate(&self, value: &str) -> ValidationResult<()> {
		if value.is_empty() {
			return if let Some(ref msg) = self.message {
				Err(ValidationError::Custom(msg.clone()))
			} else {
				Err(ValidationError::InvalidSlug(
					"Slug cannot be empty".to_string(),
				))
			};
		}

		let is_valid = if self.allow_unicode {
			SLUG_UNICODE_REGEX.is_match(value)
		} else {
			SLUG_ASCII_REGEX.is_match(value)
		};

		if is_valid {
			Ok(())
		} else if let Some(ref msg) = self.message {
			Err(ValidationError::Custom(msg.clone()))
		} else {
			Err(ValidationError::InvalidSlug(format!(
				"Slug must contain only letters, numbers, hyphens, and underscores{}",
				if self.allow_unicode {
					" (Unicode allowed)"
				} else {
					""
				}
			)))
		}
	}
}

/// UUID validator - validates UUID formats (v1-v5)
pub struct UUIDValidator {
	version: Option<u8>,
	message: Option<String>,
}

impl UUIDValidator {
	/// Creates a new UUIDValidator that accepts any UUID version.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::{UUIDValidator, Validator};
	///
	/// let validator = UUIDValidator::new();
	/// assert!(validator.validate("550e8400-e29b-41d4-a716-446655440000").is_ok());
	/// ```
	pub fn new() -> Self {
		Self {
			version: None,
			message: None,
		}
	}

	/// Specifies the UUID version to validate (1-5).
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::{UUIDValidator, Validator};
	///
	/// let validator = UUIDValidator::new().version(4);
	/// assert!(validator.validate("550e8400-e29b-41d4-a716-446655440000").is_ok());
	/// ```
	pub fn version(mut self, version: u8) -> Self {
		self.version = Some(version);
		self
	}

	/// Sets a custom error message for validation failures.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::{UUIDValidator, Validator};
	///
	/// let validator = UUIDValidator::new().with_message("Please enter a valid UUID");
	/// let result = validator.validate("not-a-uuid");
	/// assert!(result.is_err());
	/// ```
	pub fn with_message(mut self, message: impl Into<String>) -> Self {
		self.message = Some(message.into());
		self
	}
}

impl Default for UUIDValidator {
	fn default() -> Self {
		Self::new()
	}
}

impl Validator<String> for UUIDValidator {
	fn validate(&self, value: &String) -> ValidationResult<()> {
		self.validate(value.as_str())
	}
}

impl Validator<str> for UUIDValidator {
	fn validate(&self, value: &str) -> ValidationResult<()> {
		// UUID format: 8-4-4-4-12 hex digits (uses pre-compiled lazy pattern)
		if !UUID_REGEX.is_match(&value.to_lowercase()) {
			return if let Some(ref msg) = self.message {
				Err(ValidationError::Custom(msg.clone()))
			} else {
				Err(ValidationError::InvalidUUID(
					"Invalid UUID format".to_string(),
				))
			};
		}

		if let Some(version) = self.version {
			let parts: Vec<&str> = value.split('-').collect();
			if parts.len() != 5 {
				return if let Some(ref msg) = self.message {
					Err(ValidationError::Custom(msg.clone()))
				} else {
					Err(ValidationError::InvalidUUID(
						"Invalid UUID format".to_string(),
					))
				};
			}

			let version_part = parts[2];
			if let Some(first_char) = version_part.chars().next() {
				let uuid_version = first_char.to_digit(16).unwrap_or(0) as u8;
				if uuid_version != version {
					return if let Some(ref msg) = self.message {
						Err(ValidationError::Custom(msg.clone()))
					} else {
						Err(ValidationError::InvalidUUID(format!(
							"Expected UUID version {}, got version {}",
							version, uuid_version
						)))
					};
				}
			}
		}

		Ok(())
	}
}

/// Date validator - validates date strings
pub struct DateValidator {
	format: String,
	message: Option<String>,
}

impl DateValidator {
	/// Creates a new DateValidator with ISO 8601 format (YYYY-MM-DD).
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::{DateValidator, Validator};
	///
	/// let validator = DateValidator::new();
	/// assert!(validator.validate("2024-01-15").is_ok());
	/// assert!(validator.validate("invalid-date").is_err());
	/// ```
	pub fn new() -> Self {
		Self {
			format: "%Y-%m-%d".to_string(),
			message: None,
		}
	}

	/// Sets a custom date format (strftime format).
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::{DateValidator, Validator};
	///
	/// let validator = DateValidator::new().with_format("%d/%m/%Y");
	/// assert!(validator.validate("15/01/2024").is_ok());
	/// ```
	pub fn with_format(mut self, format: &str) -> Self {
		self.format = format.to_string();
		self
	}

	/// Sets a custom error message for validation failures.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::{DateValidator, Validator};
	///
	/// let validator = DateValidator::new().with_message("Invalid date format");
	/// let result = validator.validate("not-a-date");
	/// assert!(result.is_err());
	/// ```
	pub fn with_message(mut self, message: impl Into<String>) -> Self {
		self.message = Some(message.into());
		self
	}
}

impl Default for DateValidator {
	fn default() -> Self {
		Self::new()
	}
}

impl Validator<String> for DateValidator {
	fn validate(&self, value: &String) -> ValidationResult<()> {
		self.validate(value.as_str())
	}
}

impl Validator<str> for DateValidator {
	fn validate(&self, value: &str) -> ValidationResult<()> {
		NaiveDate::parse_from_str(value, &self.format)
			.map(|_| ())
			.map_err(|_| {
				if let Some(ref msg) = self.message {
					ValidationError::Custom(msg.clone())
				} else {
					ValidationError::InvalidDate(format!("Expected format: {}", self.format))
				}
			})
	}
}

/// Time validator - validates time strings
pub struct TimeValidator {
	format: String,
	message: Option<String>,
}

impl TimeValidator {
	/// Creates a new TimeValidator with 24-hour format (HH:MM:SS).
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::{TimeValidator, Validator};
	///
	/// let validator = TimeValidator::new();
	/// assert!(validator.validate("14:30:00").is_ok());
	/// assert!(validator.validate("invalid-time").is_err());
	/// ```
	pub fn new() -> Self {
		Self {
			format: "%H:%M:%S".to_string(),
			message: None,
		}
	}

	/// Sets a custom time format (strftime format).
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::{TimeValidator, Validator};
	///
	/// let validator = TimeValidator::new().with_format("%H:%M");
	/// assert!(validator.validate("14:30").is_ok());
	/// ```
	pub fn with_format(mut self, format: &str) -> Self {
		self.format = format.to_string();
		self
	}

	/// Sets a custom error message for validation failures.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::{TimeValidator, Validator};
	///
	/// let validator = TimeValidator::new().with_message("Invalid time format");
	/// let result = validator.validate("not-a-time");
	/// assert!(result.is_err());
	/// ```
	pub fn with_message(mut self, message: impl Into<String>) -> Self {
		self.message = Some(message.into());
		self
	}
}

impl Default for TimeValidator {
	fn default() -> Self {
		Self::new()
	}
}

impl Validator<String> for TimeValidator {
	fn validate(&self, value: &String) -> ValidationResult<()> {
		self.validate(value.as_str())
	}
}

impl Validator<str> for TimeValidator {
	fn validate(&self, value: &str) -> ValidationResult<()> {
		NaiveTime::parse_from_str(value, &self.format)
			.map(|_| ())
			.map_err(|_| {
				if let Some(ref msg) = self.message {
					ValidationError::Custom(msg.clone())
				} else {
					ValidationError::InvalidTime(format!("Expected format: {}", self.format))
				}
			})
	}
}

/// DateTime validator - validates datetime strings
pub struct DateTimeValidator {
	format: String,
	message: Option<String>,
}

impl DateTimeValidator {
	/// Creates a new DateTimeValidator with ISO 8601 format.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::{DateTimeValidator, Validator};
	///
	/// let validator = DateTimeValidator::new();
	/// assert!(validator.validate("2024-01-15 14:30:00").is_ok());
	/// assert!(validator.validate("invalid-datetime").is_err());
	/// ```
	pub fn new() -> Self {
		Self {
			format: "%Y-%m-%d %H:%M:%S".to_string(),
			message: None,
		}
	}

	/// Sets a custom datetime format (strftime format).
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::{DateTimeValidator, Validator};
	///
	/// let validator = DateTimeValidator::new().with_format("%d/%m/%Y %H:%M");
	/// assert!(validator.validate("15/01/2024 14:30").is_ok());
	/// ```
	pub fn with_format(mut self, format: &str) -> Self {
		self.format = format.to_string();
		self
	}

	/// Sets a custom error message for validation failures.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::{DateTimeValidator, Validator};
	///
	/// let validator = DateTimeValidator::new().with_message("Invalid datetime format");
	/// let result = validator.validate("not-a-datetime");
	/// assert!(result.is_err());
	/// ```
	pub fn with_message(mut self, message: impl Into<String>) -> Self {
		self.message = Some(message.into());
		self
	}
}

impl Default for DateTimeValidator {
	fn default() -> Self {
		Self::new()
	}
}

impl Validator<String> for DateTimeValidator {
	fn validate(&self, value: &String) -> ValidationResult<()> {
		self.validate(value.as_str())
	}
}

impl Validator<str> for DateTimeValidator {
	fn validate(&self, value: &str) -> ValidationResult<()> {
		NaiveDateTime::parse_from_str(value, &self.format)
			.map(|_| ())
			.map_err(|_| {
				if let Some(ref msg) = self.message {
					ValidationError::Custom(msg.clone())
				} else {
					ValidationError::InvalidDateTime(format!("Expected format: {}", self.format))
				}
			})
	}
}

/// JSON validator - validates JSON structure
pub struct JSONValidator {
	message: Option<String>,
}

impl JSONValidator {
	/// Creates a new JSONValidator.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::{JSONValidator, Validator};
	///
	/// let validator = JSONValidator::new();
	/// assert!(validator.validate(r#"{"key": "value"}"#).is_ok());
	/// assert!(validator.validate("invalid-json").is_err());
	/// ```
	pub fn new() -> Self {
		Self { message: None }
	}

	/// Sets a custom error message for validation failures.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::{JSONValidator, Validator};
	///
	/// let validator = JSONValidator::new().with_message("Invalid JSON format");
	/// let result = validator.validate("not-json");
	/// assert!(result.is_err());
	/// ```
	pub fn with_message(mut self, message: impl Into<String>) -> Self {
		self.message = Some(message.into());
		self
	}
}

impl Default for JSONValidator {
	fn default() -> Self {
		Self::new()
	}
}

impl Validator<String> for JSONValidator {
	fn validate(&self, value: &String) -> ValidationResult<()> {
		self.validate(value.as_str())
	}
}

impl Validator<str> for JSONValidator {
	fn validate(&self, value: &str) -> ValidationResult<()> {
		serde_json::from_str::<serde_json::Value>(value)
			.map(|_| ())
			.map_err(|e| {
				if let Some(ref msg) = self.message {
					ValidationError::Custom(msg.clone())
				} else {
					ValidationError::InvalidJSON(format!("JSON parse error: {}", e))
				}
			})
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	// Tests based on Django validators/tests.py
	#[test]
	fn test_min_length_validator_valid() {
		let validator = MinLengthValidator::new(5);
		assert!(validator.validate("hello").is_ok());
		assert!(validator.validate("hello world").is_ok());
		assert!(validator.validate("12345").is_ok());
	}

	#[test]
	fn test_min_length_validator_invalid() {
		let validator = MinLengthValidator::new(5);
		let result = validator.validate("hi");
		assert!(result.is_err());
		match result {
			Err(ValidationError::TooShort { length, min }) => {
				assert_eq!(length, 2);
				assert_eq!(min, 5);
			}
			_ => panic!("Expected TooShort error"),
		}
	}

	#[test]
	fn test_min_length_validator_edge_cases() {
		let validator = MinLengthValidator::new(0);
		assert!(validator.validate("").is_ok());

		let validator = MinLengthValidator::new(1);
		assert!(validator.validate("a").is_ok());
		assert!(validator.validate("").is_err());
	}

	#[test]
	fn test_min_length_validator_unicode() {
		let validator = MinLengthValidator::new(3);
		assert!(validator.validate("abc").is_ok());
		assert!(validator.validate("日本語").is_ok()); // 9 bytes, 3 chars — passes because char count is used
	}

	#[test]
	fn test_min_length_validator_uses_char_count_not_bytes() {
		// "あい" is 2 characters but 6 bytes in UTF-8
		let validator = MinLengthValidator::new(3);
		let result = validator.validate("あい");
		assert!(result.is_err());
		match result {
			Err(ValidationError::TooShort { length, min }) => {
				assert_eq!(length, 2); // character count, not byte length (6)
				assert_eq!(min, 3);
			}
			_ => panic!("Expected TooShort error"),
		}
	}

	#[test]
	fn test_max_length_validator_uses_char_count_not_bytes() {
		// "あいう" is 3 characters but 9 bytes in UTF-8
		let validator = MaxLengthValidator::new(5);
		assert!(validator.validate("あいう").is_ok()); // 3 chars <= 5: valid
		let result = validator.validate("あいうえおか"); // 6 chars > 5: invalid
		assert!(result.is_err());
		match result {
			Err(ValidationError::TooLong { length, max }) => {
				assert_eq!(length, 6); // character count, not byte length (18)
				assert_eq!(max, 5);
			}
			_ => panic!("Expected TooLong error"),
		}
	}

	#[test]
	fn test_max_length_validator_valid() {
		let validator = MaxLengthValidator::new(10);
		assert!(validator.validate("hello").is_ok());
		assert!(validator.validate("1234567890").is_ok());
		assert!(validator.validate("").is_ok());
	}

	#[test]
	fn test_max_length_validator_invalid() {
		let validator = MaxLengthValidator::new(10);
		let result = validator.validate("hello world");
		assert!(result.is_err());
		match result {
			Err(ValidationError::TooLong { length, max }) => {
				assert_eq!(length, 11);
				assert_eq!(max, 10);
			}
			_ => panic!("Expected TooLong error"),
		}
	}

	#[test]
	fn test_max_length_validator_edge_cases() {
		let validator = MaxLengthValidator::new(0);
		assert!(validator.validate("").is_ok());
		assert!(validator.validate("a").is_err());

		let validator = MaxLengthValidator::new(1);
		assert!(validator.validate("a").is_ok());
		assert!(validator.validate("ab").is_err());
	}

	// Based on Django test_regex_validator_flags
	#[test]
	fn test_regex_validator_basic() {
		let validator = RegexValidator::new(r"^\d{3}-\d{4}$").unwrap();
		assert!(validator.validate("123-4567").is_ok());
		assert!(validator.validate("invalid").is_err());
	}

	#[test]
	fn test_regex_validator_pattern_matching() {
		// URL-like pattern
		let validator = RegexValidator::new(r"^(?:[a-z0-9.-]*)://").unwrap();
		assert!(validator.validate("http://example.com").is_ok());
		assert!(validator.validate("https://example.com").is_ok());
		assert!(validator.validate("ftp://example.com").is_ok());
		assert!(validator.validate("invalid").is_err());
	}

	#[test]
	fn test_regex_validator_with_custom_message() {
		let validator = RegexValidator::new(r"^\d+$")
			.unwrap()
			.with_message("Value must contain only digits");

		assert!(validator.validate("12345").is_ok());

		let result = validator.validate("abc");
		assert!(result.is_err());
		match result {
			Err(ValidationError::PatternMismatch(msg)) => {
				assert_eq!(msg, "Value must contain only digits");
			}
			_ => panic!("Expected PatternMismatch error"),
		}
	}

	#[test]
	fn test_regex_validator_email_pattern() {
		let validator =
			RegexValidator::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap();
		assert!(validator.validate("test@example.com").is_ok());
		assert!(validator.validate("user.name+tag@example.co.uk").is_ok());
		assert!(validator.validate("invalid@").is_err());
		assert!(validator.validate("@example.com").is_err());
		assert!(validator.validate("invalid").is_err());
	}

	#[test]
	fn test_regex_validator_slug_pattern() {
		let validator = RegexValidator::new(r"^[-a-zA-Z0-9_]+$").unwrap();
		assert!(validator.validate("valid-slug").is_ok());
		assert!(validator.validate("valid_slug_123").is_ok());
		assert!(validator.validate("invalid slug").is_err());
		assert!(validator.validate("invalid@slug").is_err());
	}

	#[test]
	fn test_regex_validator_empty_string() {
		let validator = RegexValidator::new(r"^.*$").unwrap();
		assert!(validator.validate("").is_ok());
		assert!(validator.validate("anything").is_ok());
	}

	#[test]
	fn test_regex_validator_special_characters() {
		// Test escaping special regex characters
		let validator = RegexValidator::new(r"^\d+\.\d+$").unwrap();
		assert!(validator.validate("1.5").is_ok());
		assert!(validator.validate("123.456").is_ok());
		assert!(validator.validate("1a5").is_err());
	}

	// Test both String and str implementations
	#[test]
	fn test_validators_work_with_string_types() {
		let min_validator = MinLengthValidator::new(3);
		let max_validator = MaxLengthValidator::new(10);

		// Test with &str
		assert!(min_validator.validate("test").is_ok());
		assert!(max_validator.validate("test").is_ok());

		// Test with String
		let s = String::from("test");
		assert!(min_validator.validate(&s).is_ok());
		assert!(max_validator.validate(&s).is_ok());
	}

	// Based on Django test_max_length_validator_message
	#[test]
	fn test_min_length_error_contains_correct_values() {
		let validator = MinLengthValidator::new(16);
		match validator.validate("short") {
			Err(ValidationError::TooShort { length, min }) => {
				assert_eq!(length, 5);
				assert_eq!(min, 16);
			}
			_ => panic!("Expected TooShort error with correct values"),
		}
	}

	#[test]
	fn test_max_length_error_contains_correct_values() {
		let validator = MaxLengthValidator::new(5);
		match validator.validate("toolong") {
			Err(ValidationError::TooLong { length, max }) => {
				assert_eq!(length, 7);
				assert_eq!(max, 5);
			}
			_ => panic!("Expected TooLong error with correct values"),
		}
	}
}

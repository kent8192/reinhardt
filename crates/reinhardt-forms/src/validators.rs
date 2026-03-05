//! Page/URL validators for form fields
//!
//! This module provides validators for URL and URL slug validation
//! that integrate with the form field validation pipeline.

use crate::field::{FieldError, FieldResult};
use regex::Regex;
use std::sync::LazyLock;

// HTTP/HTTPS URL pattern.
//
// Validates URLs with:
// - http or https scheme only
// - Valid domain labels (no leading/trailing hyphens)
// - Optional port number (1-5 digits)
// - Optional path, query string, and fragment
static URL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
	Regex::new(
		r"^https?://[a-zA-Z0-9]([a-zA-Z0-9\-]{0,61}[a-zA-Z0-9])?(\.[a-zA-Z0-9]([a-zA-Z0-9\-]*[a-zA-Z0-9])?)*(:[0-9]{1,5})?(/[^\s?#]*)?(\?[^\s#]*)?(#[^\s]*)?$",
	)
	.expect("URL_REGEX: invalid regex pattern")
});

// ASCII slug pattern: lowercase letters, digits, hyphens, underscores.
//
// Does not allow hyphens at the start or end of the slug.
static SLUG_REGEX: LazyLock<Regex> = LazyLock::new(|| {
	Regex::new(r"^[a-z0-9][a-z0-9_-]*[a-z0-9]$|^[a-z0-9]$")
		.expect("SLUG_REGEX: invalid regex pattern")
});

/// Validates that a string value is a well-formed HTTP or HTTPS URL.
///
/// The validator checks:
/// - Scheme must be `http` or `https`
/// - Host must be non-empty and must not start or end with a hyphen
/// - Optional port, path, query string, and fragment are allowed
///
/// # Examples
///
/// ```
/// use reinhardt_forms::validators::UrlValidator;
///
/// let validator = UrlValidator::new();
/// assert!(validator.validate("https://example.com").is_ok());
/// assert!(validator.validate("http://localhost:8080/path").is_ok());
/// assert!(validator.validate("ftp://example.com").is_err());
/// assert!(validator.validate("not-a-url").is_err());
/// ```
#[derive(Debug, Clone)]
pub struct UrlValidator {
	/// Optional custom error message shown on validation failure
	message: Option<String>,
}

impl UrlValidator {
	/// Creates a new `UrlValidator` with default settings.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::validators::UrlValidator;
	///
	/// let validator = UrlValidator::new();
	/// assert!(validator.validate("https://example.com").is_ok());
	/// ```
	pub fn new() -> Self {
		Self { message: None }
	}

	/// Sets a custom error message returned on validation failure.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::validators::UrlValidator;
	///
	/// let validator = UrlValidator::new().with_message("Please enter a valid website URL");
	/// assert!(validator.validate("bad").is_err());
	/// ```
	pub fn with_message(mut self, message: impl Into<String>) -> Self {
		self.message = Some(message.into());
		self
	}

	/// Validates the given string slice as a URL.
	///
	/// Returns `Ok(())` when the URL is valid, or a [`FieldError::Validation`]
	/// containing an error message when it is not.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::validators::UrlValidator;
	///
	/// let validator = UrlValidator::new();
	/// assert!(validator.validate("https://www.example.com/path?q=1").is_ok());
	/// assert!(validator.validate("ftp://example.com").is_err());
	/// ```
	pub fn validate(&self, value: &str) -> FieldResult<()> {
		if URL_REGEX.is_match(value) {
			Ok(())
		} else {
			let msg = self.message.as_deref().unwrap_or("Enter a valid URL");
			Err(FieldError::Validation(msg.to_string()))
		}
	}
}

impl Default for UrlValidator {
	fn default() -> Self {
		Self::new()
	}
}

/// Validates that a string value is a valid URL slug.
///
/// A valid slug:
/// - Contains only lowercase ASCII letters (`a`-`z`), digits (`0`-`9`),
///   hyphens (`-`), and underscores (`_`)
/// - Is non-empty
/// - Does not start or end with a hyphen
///
/// # Examples
///
/// ```
/// use reinhardt_forms::validators::SlugValidator;
///
/// let validator = SlugValidator::new();
/// assert!(validator.validate("my-article").is_ok());
/// assert!(validator.validate("page_1").is_ok());
/// assert!(validator.validate("-invalid").is_err());
/// assert!(validator.validate("has space").is_err());
/// assert!(validator.validate("").is_err());
/// ```
#[derive(Debug, Clone)]
pub struct SlugValidator {
	/// Optional custom error message shown on validation failure
	message: Option<String>,
}

impl SlugValidator {
	/// Creates a new `SlugValidator` with default settings.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::validators::SlugValidator;
	///
	/// let validator = SlugValidator::new();
	/// assert!(validator.validate("valid-slug").is_ok());
	/// ```
	pub fn new() -> Self {
		Self { message: None }
	}

	/// Sets a custom error message returned on validation failure.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::validators::SlugValidator;
	///
	/// let validator = SlugValidator::new().with_message("Only lowercase letters, numbers, hyphens, and underscores are allowed");
	/// assert!(validator.validate("Bad Slug!").is_err());
	/// ```
	pub fn with_message(mut self, message: impl Into<String>) -> Self {
		self.message = Some(message.into());
		self
	}

	/// Validates the given string slice as a URL slug.
	///
	/// Returns `Ok(())` for a valid slug, or a [`FieldError::Validation`]
	/// containing an error message for an invalid one.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::validators::SlugValidator;
	///
	/// let validator = SlugValidator::new();
	/// assert!(validator.validate("my-slug-123").is_ok());
	/// assert!(validator.validate("trailing-").is_err());
	/// assert!(validator.validate("-leading").is_err());
	/// ```
	pub fn validate(&self, value: &str) -> FieldResult<()> {
		if value.is_empty() {
			let msg = self
				.message
				.as_deref()
				.unwrap_or("Enter a valid slug (non-empty)");
			return Err(FieldError::Validation(msg.to_string()));
		}

		if SLUG_REGEX.is_match(value) {
			Ok(())
		} else {
			let msg = self.message.as_deref().unwrap_or(
				"Enter a valid slug consisting of lowercase letters, numbers, hyphens, or underscores. \
				 The slug must not start or end with a hyphen.",
			);
			Err(FieldError::Validation(msg.to_string()))
		}
	}
}

impl Default for SlugValidator {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	// =========================================================================
	// UrlValidator tests
	// =========================================================================

	#[rstest]
	#[case("http://example.com")]
	#[case("https://example.com")]
	#[case("http://www.example.com")]
	#[case("https://www.example.com/")]
	#[case("http://localhost")]
	#[case("http://localhost:8080")]
	#[case("http://localhost:8080/path")]
	#[case("https://example.com/path/to/resource")]
	#[case("https://example.com/path?query=value")]
	#[case("https://example.com/path?query=value#section")]
	#[case("http://sub.example.com/")]
	#[case("http://example.com:3000")]
	#[case("http://valid-domain.com/")]
	#[case("https://example.com?q=1&page=2")]
	fn test_url_validator_valid(#[case] url: &str) {
		// Arrange
		let validator = UrlValidator::new();

		// Act
		let result = validator.validate(url);

		// Assert
		assert!(result.is_ok(), "Expected '{url}' to be a valid URL");
	}

	#[rstest]
	#[case("")]
	#[case("not-a-url")]
	#[case("ftp://example.com")]
	#[case("http://")]
	#[case("http://.com")]
	#[case("//example.com")]
	#[case("http://-invalid.com")]
	#[case("http://invalid-.com")]
	#[case("just text")]
	#[case("example.com")]
	fn test_url_validator_invalid(#[case] url: &str) {
		// Arrange
		let validator = UrlValidator::new();

		// Act
		let result = validator.validate(url);

		// Assert
		assert!(result.is_err(), "Expected '{url}' to be an invalid URL");
	}

	#[rstest]
	fn test_url_validator_error_type() {
		// Arrange
		let validator = UrlValidator::new();

		// Act
		let result = validator.validate("not-a-url");

		// Assert
		assert!(matches!(result, Err(FieldError::Validation(_))));
	}

	#[rstest]
	fn test_url_validator_custom_message() {
		// Arrange
		let validator = UrlValidator::new().with_message("Custom URL error");

		// Act
		let result = validator.validate("bad-url");

		// Assert
		match result {
			Err(FieldError::Validation(msg)) => {
				assert_eq!(msg, "Custom URL error");
			}
			_ => panic!("Expected Validation error with custom message"),
		}
	}

	#[rstest]
	fn test_url_validator_default() {
		// Arrange
		let validator = UrlValidator::default();

		// Act + Assert
		assert!(validator.validate("https://example.com").is_ok());
	}

	// =========================================================================
	// SlugValidator tests
	// =========================================================================

	#[rstest]
	#[case("a")]
	#[case("slug")]
	#[case("my-slug")]
	#[case("my_slug")]
	#[case("slug-123")]
	#[case("my-article-title")]
	#[case("page1")]
	#[case("a1b2c3")]
	#[case("under_score")]
	#[case("mix-ed_slug-1")]
	fn test_slug_validator_valid(#[case] slug: &str) {
		// Arrange
		let validator = SlugValidator::new();

		// Act
		let result = validator.validate(slug);

		// Assert
		assert!(result.is_ok(), "Expected '{slug}' to be a valid slug");
	}

	#[rstest]
	#[case("")]
	#[case("-starts-with-hyphen")]
	#[case("ends-with-hyphen-")]
	#[case("has space")]
	#[case("UPPERCASE")]
	#[case("Has-Upper")]
	#[case("special!char")]
	#[case("dot.in.slug")]
	#[case("unicode-日本語")]
	fn test_slug_validator_invalid(#[case] slug: &str) {
		// Arrange
		let validator = SlugValidator::new();

		// Act
		let result = validator.validate(slug);

		// Assert
		assert!(result.is_err(), "Expected '{slug}' to be an invalid slug");
	}

	#[rstest]
	fn test_slug_validator_empty_specific_error() {
		// Arrange
		let validator = SlugValidator::new();

		// Act
		let result = validator.validate("");

		// Assert
		assert!(matches!(result, Err(FieldError::Validation(_))));
	}

	#[rstest]
	fn test_slug_validator_invalid_error_type() {
		// Arrange
		let validator = SlugValidator::new();

		// Act
		let result = validator.validate("-bad-slug");

		// Assert
		assert!(matches!(result, Err(FieldError::Validation(_))));
	}

	#[rstest]
	fn test_slug_validator_custom_message() {
		// Arrange
		let validator = SlugValidator::new().with_message("Custom slug error");

		// Act
		let result = validator.validate("Bad Slug!");

		// Assert
		match result {
			Err(FieldError::Validation(msg)) => {
				assert_eq!(msg, "Custom slug error");
			}
			_ => panic!("Expected Validation error with custom message"),
		}
	}

	#[rstest]
	fn test_slug_validator_custom_message_on_empty() {
		// Arrange
		let validator = SlugValidator::new().with_message("Slug cannot be empty");

		// Act
		let result = validator.validate("");

		// Assert
		match result {
			Err(FieldError::Validation(msg)) => {
				assert_eq!(msg, "Slug cannot be empty");
			}
			_ => panic!("Expected Validation error with custom message"),
		}
	}

	#[rstest]
	fn test_slug_validator_default() {
		// Arrange
		let validator = SlugValidator::default();

		// Act + Assert
		assert!(validator.validate("valid-slug").is_ok());
	}
}

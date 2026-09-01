//! URL validator

use super::lazy_patterns::URL_REGEX;
use super::{ValidationError, ValidationResult, Validator};

/// URL validator
pub struct UrlValidator {
	message: Option<String>,
}

impl UrlValidator {
	/// Creates a new UrlValidator that validates HTTP and HTTPS URLs.
	///
	/// Supports URLs with:
	/// - HTTP and HTTPS schemes
	/// - Optional ports (e.g., :8080)
	/// - Paths, query strings, and fragments
	/// - Subdomains and hyphens in domain names
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::{UrlValidator, Validator};
	///
	/// let validator = UrlValidator::new();
	/// assert!(validator.validate("http://example.com").is_ok());
	/// assert!(validator.validate("https://example.com:8080/path?query=value").is_ok());
	/// assert!(validator.validate("not-a-url").is_err());
	/// ```
	pub fn new() -> Self {
		Self { message: None }
	}

	/// Sets a custom error message for validation failures.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::{UrlValidator, Validator};
	///
	/// let validator = UrlValidator::new().with_message("Invalid URL");
	/// let result = validator.validate("not-a-url");
	/// assert!(result.is_err());
	/// ```
	pub fn with_message(mut self, message: impl Into<String>) -> Self {
		self.message = Some(message.into());
		self
	}
}

impl Default for UrlValidator {
	fn default() -> Self {
		Self::new()
	}
}

impl Validator<String> for UrlValidator {
	fn validate(&self, value: &String) -> ValidationResult<()> {
		if URL_REGEX.is_match(value) {
			Ok(())
		} else if let Some(ref msg) = self.message {
			Err(ValidationError::Custom(msg.clone()))
		} else {
			Err(ValidationError::InvalidUrl(value.clone()))
		}
	}
}

impl Validator<str> for UrlValidator {
	fn validate(&self, value: &str) -> ValidationResult<()> {
		if URL_REGEX.is_match(value) {
			Ok(())
		} else if let Some(ref msg) = self.message {
			Err(ValidationError::Custom(msg.clone()))
		} else {
			Err(ValidationError::InvalidUrl(value.to_string()))
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	/// Tests based on Django validators/tests.py URL validation tests
	#[test]
	fn test_url_validator_valid_urls() {
		let validator = UrlValidator::new();

		let valid_urls = vec![
			"http://www.djangoproject.com/",
			"http://localhost/",
			"http://example.com/",
			"http://www.example.com/",
			"http://valid-with-hyphens.com/",
			"http://subdomain.example.com/",
			"https://example.com/",
			"http://foo.com/blah_blah",
			"http://foo.com/blah_blah/",
			"http://example.com/path/to/resource",
		];

		for url in valid_urls {
			assert!(
				validator.validate(url).is_ok(),
				"Expected {} to be valid",
				url
			);
		}
	}

	#[test]
	fn test_url_validator_invalid_urls() {
		let validator = UrlValidator::new();

		let invalid_urls = vec![
			"no_scheme",
			"ftp://example.com/",  // Only http/https supported
			"http://",             // No domain
			"http://.com",         // Invalid domain
			"http://invalid-.com", // Domain ends with hyphen
			"http://-invalid.com", // Domain starts with hyphen
			"//example.com",       // Protocol-relative URL
			"http://",
			"http://..",
			"http://../",
			"http://?",
			"http://#",
		];

		for url in invalid_urls {
			assert!(
				validator.validate(url).is_err(),
				"Expected {} to be invalid",
				url
			);
		}
	}

	#[rstest]
	fn test_url_validator_with_ports() {
		let validator = UrlValidator::new();
		assert!(validator.validate("http://example.com:8080/").is_ok());
		assert!(validator.validate("http://example.com:80/").is_ok());
		assert!(validator.validate("https://example.com:443/").is_ok());
		assert!(validator.validate("http://localhost:3000/").is_ok());
		assert_eq!(
			validator.validate("http://example.com:123456/"),
			Err(ValidationError::InvalidUrl(
				"http://example.com:123456/".to_owned()
			))
		);
	}

	#[test]
	fn test_url_validator_with_paths() {
		let validator = UrlValidator::new();
		assert!(validator.validate("http://example.com/").is_ok());
		assert!(validator.validate("http://example.com/path").is_ok());
		assert!(
			validator
				.validate("http://example.com/path/to/resource")
				.is_ok()
		);
		assert!(
			validator
				.validate("http://example.com/path/to/resource/")
				.is_ok()
		);
	}

	#[rstest]
	fn test_url_validator_with_query_strings() {
		let validator = UrlValidator::new();
		assert_eq!(validator.validate("http://example.com?query=value"), Ok(()));
		assert!(
			validator
				.validate("http://example.com/?query=value")
				.is_ok()
		);
		assert!(
			validator
				.validate("http://example.com/path?query=value&other=value2")
				.is_ok()
		);
	}

	#[rstest]
	fn test_url_validator_with_fragments() {
		let validator = UrlValidator::new();
		assert_eq!(validator.validate("http://example.com#section"), Ok(()));
		assert!(validator.validate("http://example.com/#section").is_ok());
		assert!(
			validator
				.validate("http://example.com/path#section")
				.is_ok()
		);
		assert!(
			validator
				.validate("http://example.com/path?query=value#section")
				.is_ok()
		);
	}

	#[test]
	fn test_url_validator_with_subdomains() {
		let validator = UrlValidator::new();
		assert!(validator.validate("http://subdomain.example.com/").is_ok());
		assert!(
			validator
				.validate("http://sub.subdomain.example.com/")
				.is_ok()
		);
		assert!(validator.validate("http://a.b.c.example.com/").is_ok());
	}

	#[test]
	fn test_url_validator_https() {
		let validator = UrlValidator::new();
		assert!(validator.validate("https://example.com/").is_ok());
		assert!(validator.validate("https://www.example.com/").is_ok());
		assert!(
			validator
				.validate("https://secure.example.com/login")
				.is_ok()
		);
	}

	#[test]
	fn test_url_validator_with_hyphens() {
		let validator = UrlValidator::new();
		assert!(validator.validate("http://valid-domain.com/").is_ok());
		assert!(
			validator
				.validate("http://my-long-domain-name.com/")
				.is_ok()
		);
		assert!(validator.validate("http://sub-domain.example.com/").is_ok());

		// Invalid: hyphens at start or end
		assert!(validator.validate("http://-invalid.com/").is_err());
		assert!(validator.validate("http://invalid-.com/").is_err());
	}

	#[test]
	fn test_url_validator_with_numbers() {
		let validator = UrlValidator::new();
		assert!(validator.validate("http://example123.com/").is_ok());
		assert!(validator.validate("http://123example.com/").is_ok());
		assert!(validator.validate("http://123.com/").is_ok());
	}

	#[test]
	fn test_url_validator_localhost() {
		let validator = UrlValidator::new();
		assert!(validator.validate("http://localhost/").is_ok());
		assert!(validator.validate("http://localhost:8000/").is_ok());
		assert!(validator.validate("http://localhost/path").is_ok());
	}

	#[test]
	fn test_url_validator_case_sensitivity() {
		let validator = UrlValidator::new();
		// URLs are case-sensitive in path/query, but not in domain
		assert!(validator.validate("http://Example.COM/").is_ok());
		assert!(validator.validate("http://EXAMPLE.COM/PATH").is_ok());
		assert!(validator.validate("http://example.com/").is_ok());
	}

	#[test]
	fn test_url_validator_returns_correct_error() {
		let validator = UrlValidator::new();
		let invalid_url = "not-a-url";
		match validator.validate(invalid_url) {
			Err(ValidationError::InvalidUrl(url)) => {
				assert_eq!(url, invalid_url);
			}
			_ => panic!("Expected InvalidUrl error"),
		}
	}

	#[test]
	fn test_url_validator_with_string_type() {
		let validator = UrlValidator::new();
		let url = String::from("http://example.com/");
		assert!(validator.validate(&url).is_ok());

		let invalid = String::from("invalid");
		assert!(validator.validate(&invalid).is_err());
	}

	#[test]
	fn test_url_validator_special_characters_in_path() {
		let validator = UrlValidator::new();
		assert!(
			validator
				.validate("http://example.com/path_with_underscore")
				.is_ok()
		);
		assert!(
			validator
				.validate("http://example.com/path-with-dash")
				.is_ok()
		);
		assert!(
			validator
				.validate("http://example.com/path.with.dots")
				.is_ok()
		);
	}

	#[test]
	fn test_url_validator_default_and_custom_messages() {
		let validator = UrlValidator::default().with_message("Use an HTTP URL");

		assert_eq!(
			validator.validate(&String::from("invalid")),
			Err(ValidationError::Custom("Use an HTTP URL".to_owned()))
		);
		assert_eq!(
			validator.validate("invalid"),
			Err(ValidationError::Custom("Use an HTTP URL".to_owned()))
		);
	}
}

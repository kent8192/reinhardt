//! Custom validation hooks for parsers.
//!
//! This module provides traits and utilities for adding custom validation logic
//! before and after parsing operations.

use async_trait::async_trait;
use bytes::Bytes;

use super::parser::{ParseResult, ParsedData};

/// Trait for custom parser validation hooks.
///
/// Implement this trait to add custom validation logic that runs before or after
/// parsing operations. This allows you to enforce business rules, size limits,
/// or other constraints on parsed data.
///
/// # Examples
///
/// ```
/// use async_trait::async_trait;
/// use bytes::Bytes;
/// use reinhardt_core::parsers::validator::ParserValidator;
/// use reinhardt_core::parsers::parser::{ParseResult, ParsedData};
/// use reinhardt_core::exception::Error;
///
/// struct SizeLimitValidator {
///     max_size: usize,
/// }
///
/// #[async_trait]
/// impl ParserValidator for SizeLimitValidator {
///     async fn before_parse(&self, _content_type: Option<&str>, body: &Bytes) -> ParseResult<()> {
///         if body.len() > self.max_size {
///             return Err(Error::Validation(format!(
///                 "Body size {} exceeds maximum {}",
///                 body.len(),
///                 self.max_size
///             )));
///         }
///         Ok(())
///     }
///
///     async fn after_parse(&self, _data: &ParsedData) -> ParseResult<()> {
///         Ok(())
///     }
/// }
/// ```
#[async_trait]
pub trait ParserValidator: Send + Sync {
	/// Validate before parsing.
	///
	/// This hook is called before the parser processes the request body.
	/// Use it to validate content type, body size, or other pre-conditions.
	///
	/// # Arguments
	///
	/// * `content_type` - The Content-Type header value, if present
	/// * `body` - The raw request body bytes
	///
	/// # Returns
	///
	/// `Ok(())` if validation passes, `Err` otherwise
	async fn before_parse(&self, content_type: Option<&str>, body: &Bytes) -> ParseResult<()>;

	/// Validate after parsing.
	///
	/// This hook is called after the parser successfully processes the request body.
	/// Use it to validate the structure or content of the parsed data.
	///
	/// # Arguments
	///
	/// * `data` - The parsed data structure
	///
	/// # Returns
	///
	/// `Ok(())` if validation passes, `Err` otherwise
	async fn after_parse(&self, data: &ParsedData) -> ParseResult<()>;
}

/// Validator that enforces a maximum body size limit.
///
/// # Examples
///
/// ```
/// use reinhardt_core::parsers::validator::SizeLimitValidator;
///
/// // Limit requests to 1MB
/// let validator = SizeLimitValidator::new(1024 * 1024);
/// ```
#[derive(Debug, Clone)]
pub struct SizeLimitValidator {
	max_size: usize,
}

impl SizeLimitValidator {
	/// Create a new SizeLimitValidator with the specified maximum size in bytes.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::parsers::validator::SizeLimitValidator;
	///
	/// // Limit to 10KB
	/// let validator = SizeLimitValidator::new(10 * 1024);
	/// ```
	pub fn new(max_size: usize) -> Self {
		Self { max_size }
	}
}

#[async_trait]
impl ParserValidator for SizeLimitValidator {
	async fn before_parse(&self, _content_type: Option<&str>, body: &Bytes) -> ParseResult<()> {
		use crate::exception::Error;

		if body.len() > self.max_size {
			return Err(Error::Validation(format!(
				"Request body size {} exceeds maximum allowed size {}",
				body.len(),
				self.max_size
			)));
		}
		Ok(())
	}

	async fn after_parse(&self, _data: &ParsedData) -> ParseResult<()> {
		Ok(())
	}
}

/// Validator that checks for required content type.
///
/// # Examples
///
/// ```
/// use reinhardt_core::parsers::validator::ContentTypeValidator;
///
/// // Require application/json
/// let validator = ContentTypeValidator::new(vec!["application/json".to_string()]);
/// ```
#[derive(Debug, Clone)]
pub struct ContentTypeValidator {
	allowed_types: Vec<String>,
}

impl ContentTypeValidator {
	/// Create a new ContentTypeValidator with allowed content types.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::parsers::validator::ContentTypeValidator;
	///
	/// let validator = ContentTypeValidator::new(vec![
	///     "application/json".to_string(),
	///     "application/xml".to_string(),
	/// ]);
	/// ```
	pub fn new(allowed_types: Vec<String>) -> Self {
		Self { allowed_types }
	}
}

#[async_trait]
impl ParserValidator for ContentTypeValidator {
	async fn before_parse(&self, content_type: Option<&str>, _body: &Bytes) -> ParseResult<()> {
		use crate::exception::Error;

		if let Some(ct) = content_type {
			// Extract the media type (before any parameters like charset)
			// e.g., "application/json; charset=utf-8" -> "application/json"
			let media_type = ct.split(';').next().unwrap_or(ct).trim().to_lowercase();

			// Use exact matching on the media type portion instead of
			// substring matching to prevent bypass via crafted content types
			// (e.g., "application/not-json-at-all" should not match "json")
			for allowed in &self.allowed_types {
				if media_type == allowed.to_lowercase() {
					return Ok(());
				}
			}
			return Err(Error::Validation(format!(
				"Content-Type '{}' is not allowed. Allowed types: {:?}",
				ct, self.allowed_types
			)));
		}

		Err(Error::Validation(
			"Content-Type header is required".to_string(),
		))
	}

	async fn after_parse(&self, _data: &ParsedData) -> ParseResult<()> {
		Ok(())
	}
}

/// Composite validator that runs multiple validators in sequence.
///
/// # Examples
///
/// ```
/// use reinhardt_core::parsers::validator::{CompositeValidator, SizeLimitValidator, ContentTypeValidator};
///
/// let validator = CompositeValidator::new()
///     .add(SizeLimitValidator::new(1024 * 1024))
///     .add(ContentTypeValidator::new(vec!["application/json".to_string()]));
/// ```
#[derive(Default)]
pub struct CompositeValidator {
	validators: Vec<Box<dyn ParserValidator>>,
}

impl CompositeValidator {
	/// Create a new empty CompositeValidator.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::parsers::validator::CompositeValidator;
	///
	/// let validator = CompositeValidator::new();
	/// ```
	pub fn new() -> Self {
		Self::default()
	}

	/// Add a validator to the composite.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::parsers::validator::{CompositeValidator, SizeLimitValidator};
	///
	/// let validator = CompositeValidator::new()
	///     .add(SizeLimitValidator::new(1024));
	/// ```
	#[allow(clippy::should_implement_trait)]
	pub fn add<V: ParserValidator + 'static>(mut self, validator: V) -> Self {
		self.validators.push(Box::new(validator));
		self
	}
}

#[async_trait]
impl ParserValidator for CompositeValidator {
	async fn before_parse(&self, content_type: Option<&str>, body: &Bytes) -> ParseResult<()> {
		for validator in &self.validators {
			validator.before_parse(content_type, body).await?;
		}
		Ok(())
	}

	async fn after_parse(&self, data: &ParsedData) -> ParseResult<()> {
		for validator in &self.validators {
			validator.after_parse(data).await?;
		}
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use serde_json::json;

	#[rstest]
	#[tokio::test]
	async fn test_size_limit_validator_within_limit() {
		// Arrange
		let validator = SizeLimitValidator::new(100);
		let body = Bytes::from("small body");

		// Act
		let result = validator.before_parse(None, &body).await;

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_size_limit_validator_exceeds_limit() {
		// Arrange
		let validator = SizeLimitValidator::new(10);
		let body = Bytes::from("this is a very long body that exceeds the limit");

		// Act
		let result = validator.before_parse(None, &body).await;

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	#[tokio::test]
	async fn test_size_limit_validator_after_parse() {
		// Arrange
		let validator = SizeLimitValidator::new(100);
		let data = ParsedData::Json(json!({"key": "value"}));

		// Act
		let result = validator.after_parse(&data).await;

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_content_type_validator_allowed() {
		// Arrange
		let validator = ContentTypeValidator::new(vec!["application/json".to_string()]);
		let body = Bytes::new();

		// Act
		let result = validator
			.before_parse(Some("application/json"), &body)
			.await;

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_content_type_validator_not_allowed() {
		// Arrange
		let validator = ContentTypeValidator::new(vec!["application/json".to_string()]);
		let body = Bytes::new();

		// Act
		let result = validator.before_parse(Some("text/plain"), &body).await;

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	#[tokio::test]
	async fn test_content_type_validator_missing() {
		// Arrange
		let validator = ContentTypeValidator::new(vec!["application/json".to_string()]);
		let body = Bytes::new();

		// Act
		let result = validator.before_parse(None, &body).await;

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	#[tokio::test]
	async fn test_content_type_validator_with_charset() {
		// Arrange
		let validator = ContentTypeValidator::new(vec!["application/json".to_string()]);
		let body = Bytes::new();

		// Act
		let result = validator
			.before_parse(Some("application/json; charset=utf-8"), &body)
			.await;

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_content_type_validator_rejects_substring_match() {
		// Arrange - crafted content type that contains "json" as substring
		// but is not a valid JSON media type
		let validator = ContentTypeValidator::new(vec!["application/json".to_string()]);
		let body = Bytes::new();

		// Act - "not-json-at-all" contains "json" but should be rejected
		let result = validator
			.before_parse(Some("application/not-json-at-all"), &body)
			.await;

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	#[tokio::test]
	async fn test_content_type_validator_rejects_prefix_substring() {
		// Arrange - content type where allowed type is a prefix substring
		let validator = ContentTypeValidator::new(vec!["text/plain".to_string()]);
		let body = Bytes::new();

		// Act - "text/plaintext" starts with "text/plain" but should be rejected
		let result = validator.before_parse(Some("text/plaintext"), &body).await;

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	#[tokio::test]
	async fn test_content_type_validator_rejects_suffix_substring() {
		// Arrange - content type where allowed type is a suffix substring
		let validator = ContentTypeValidator::new(vec!["application/xml".to_string()]);
		let body = Bytes::new();

		// Act - "application/soap+xml" ends with "xml" but is a different type
		let result = validator
			.before_parse(Some("application/soap+xml"), &body)
			.await;

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	#[tokio::test]
	async fn test_content_type_validator_case_insensitive() {
		// Arrange
		let validator = ContentTypeValidator::new(vec!["application/json".to_string()]);
		let body = Bytes::new();

		// Act - uppercase variant should be accepted
		let result = validator
			.before_parse(Some("Application/JSON"), &body)
			.await;

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_content_type_validator_multiple_allowed_types() {
		// Arrange
		let validator = ContentTypeValidator::new(vec![
			"application/json".to_string(),
			"application/xml".to_string(),
			"text/plain".to_string(),
		]);
		let body = Bytes::new();

		// Act & Assert - all allowed types should pass
		assert!(
			validator
				.before_parse(Some("application/json"), &body)
				.await
				.is_ok()
		);
		assert!(
			validator
				.before_parse(Some("application/xml"), &body)
				.await
				.is_ok()
		);
		assert!(
			validator
				.before_parse(Some("text/plain"), &body)
				.await
				.is_ok()
		);

		// Act & Assert - non-allowed type should fail
		assert!(
			validator
				.before_parse(Some("text/html"), &body)
				.await
				.is_err()
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_content_type_validator_with_multiple_parameters() {
		// Arrange
		let validator = ContentTypeValidator::new(vec!["application/json".to_string()]);
		let body = Bytes::new();

		// Act - content type with multiple parameters should still match
		let result = validator
			.before_parse(
				Some("application/json; charset=utf-8; boundary=something"),
				&body,
			)
			.await;

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_content_type_validator_whitespace_handling() {
		// Arrange
		let validator = ContentTypeValidator::new(vec!["application/json".to_string()]);
		let body = Bytes::new();

		// Act - media type with extra whitespace before semicolon
		let result = validator
			.before_parse(Some("  application/json  ; charset=utf-8"), &body)
			.await;

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_composite_validator_all_pass() {
		// Arrange
		let validator = CompositeValidator::new()
			.add(SizeLimitValidator::new(100))
			.add(ContentTypeValidator::new(vec![
				"application/json".to_string(),
			]));
		let body = Bytes::from("small");

		// Act
		let result = validator
			.before_parse(Some("application/json"), &body)
			.await;

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_composite_validator_first_fails() {
		// Arrange
		let validator = CompositeValidator::new()
			.add(SizeLimitValidator::new(3))
			.add(ContentTypeValidator::new(vec![
				"application/json".to_string(),
			]));
		let body = Bytes::from("this is too long");

		// Act
		let result = validator
			.before_parse(Some("application/json"), &body)
			.await;

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	#[tokio::test]
	async fn test_composite_validator_second_fails() {
		// Arrange
		let validator = CompositeValidator::new()
			.add(SizeLimitValidator::new(100))
			.add(ContentTypeValidator::new(vec![
				"application/json".to_string(),
			]));
		let body = Bytes::from("small");

		// Act
		let result = validator.before_parse(Some("text/plain"), &body).await;

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	#[tokio::test]
	async fn test_composite_validator_after_parse() {
		// Arrange
		let validator = CompositeValidator::new()
			.add(SizeLimitValidator::new(100))
			.add(ContentTypeValidator::new(vec![
				"application/json".to_string(),
			]));
		let data = ParsedData::Json(json!({"key": "value"}));

		// Act
		let result = validator.after_parse(&data).await;

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_composite_validator_empty() {
		// Arrange
		let validator = CompositeValidator::new();
		let body = Bytes::from("test");

		// Act & Assert - before_parse
		let result = validator.before_parse(None, &body).await;
		assert!(result.is_ok());

		// Act & Assert - after_parse
		let data = ParsedData::Json(json!({"key": "value"}));
		let result = validator.after_parse(&data).await;
		assert!(result.is_ok());
	}
}

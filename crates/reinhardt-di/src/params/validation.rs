//! Validation support for parameter extraction
//!
//! This module provides validation capabilities for extracted parameters,
//! integrating with the `reinhardt-validators` crate.
//!
//! # Overview
//!
//! Reinhardt provides a powerful validation system that allows you to declaratively
//! specify constraints on path, query, and form parameters. The validation system
//! supports:
//!
//! - **Length constraints**: `min_length()`, `max_length()`
//! - **Numeric ranges**: `min_value()`, `max_value()`
//! - **Pattern matching**: `regex()`
//! - **Format validation**: `email()`, `url()`
//! - **Constraint chaining**: Combine multiple constraints with builder pattern
//!
//! # Quick Start
//!
//! ```rust,no_run
//! # use reinhardt_di::params::{Path, Query, WithValidation};
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let req = ();
//! # let ctx = ();
//! // Extract and validate a path parameter
//! // let id = Path::<i32>::from_request(req, ctx).await?;
//! // let validated_id = id.min_value(1).max_value(1000);
//! // validated_id.validate_number(&validated_id.0)?;
//!
//! // Extract and validate a query parameter
//! // let email = Query::<String>::from_request(req, ctx).await?;
//! // let validated_email = email.min_length(5).max_length(100).email();
//! // validated_email.validate_string(&validated_email.0)?;
//! # Ok(())
//! # }
//! ```
//!
//! # Type Aliases
//!
//! For convenience, the module provides type aliases:
//!
//! - `ValidatedPath<T>` - Validated path parameters
//! - `ValidatedQuery<T>` - Validated query parameters
//! - `ValidatedForm<T>` - Validated form parameters
//!
//! # Examples
//!
//! ## Numeric Range Validation
//!
//! ```rust
//! # use reinhardt_di::params::{Path, WithValidation};
//! let age = Path(25);
//! let validated = age.min_value(0).max_value(120);
//!
//! assert!(validated.validate_number(&25).is_ok());
//! assert!(validated.validate_number(&150).is_err());
//! assert!(validated.validate_number(&-10).is_err());
//! ```
//!
//! ## Email Validation
//!
//! ```rust
//! # use reinhardt_di::params::{Query, WithValidation};
//! let email = Query("user@example.com".to_string());
//! let validated = email.email();
//!
//! assert!(validated.validate_string("user@example.com").is_ok());
//! assert!(validated.validate_string("invalid").is_err());
//! assert!(validated.validate_string("test@test.com").is_ok());
//! ```
//!
//! ## Combined Constraints
//!
//! ```rust
//! # use reinhardt_di::params::{Path, WithValidation};
//! let username = Path("alice".to_string());
//! let validated = username
//!     .min_length(3)
//!     .max_length(20)
//!     .regex(r"^[a-zA-Z0-9_]+$");
//!
//! assert!(validated.validate_string(&validated.0).is_ok());
//! assert!(validated.validate_string("ab").is_err()); // Too short
//! assert!(validated.validate_string("invalid-chars!").is_err()); // Invalid chars
//! ```
//!
//! # Error Handling
//!
//! Validation errors are returned as `ValidationError` from the `reinhardt-validators`
//! crate, which provides detailed error messages including:
//!
//! - The constraint that failed (e.g., "too short", "too large")
//! - The actual value
//! - The expected constraint (e.g., minimum, maximum)
//!
//! Example error message:
//! ```text
//! Validation error for 'email': Length too short: 3 (minimum: 5)
//! ```

#[cfg(feature = "validation")]
use reinhardt_core::validators::{Validate, ValidationResult, Validator};
#[cfg(feature = "validation")]
use std::fmt::{self, Debug};
use std::ops::Deref;

/// Wrapper extractor that auto-validates the inner value after extraction.
///
/// `Validated<E>` extracts `E` from the request via `FromRequest`, then calls
/// `Validate::validate()` on the inner value. If validation fails, the request
/// is rejected with structured `ValidationErrors`.
///
/// Works with any extractor implementing `HasInner` where the inner type
/// implements `Validate`: `Form<T>`, `Json<T>`, `Query<T>`.
///
/// # Examples
///
/// ```rust,no_run
/// # use reinhardt_di::params::{Validated, Form};
/// # use reinhardt_core::validators::Validate;
/// // In a server_fn handler:
/// // async fn login(form: Validated<Form<LoginRequest>>) -> Result<(), String> {
/// //     let login = form.into_inner().into_inner(); // already validated
/// //     Ok(())
/// // }
/// ```
#[cfg(feature = "validation")]
pub struct Validated<T>(T);

#[cfg(feature = "validation")]
impl<T> Validated<T> {
	/// Unwrap and return the inner extractor.
	pub fn into_inner(self) -> T {
		self.0
	}
}

#[cfg(feature = "validation")]
impl<T> Deref for Validated<T> {
	type Target = T;

	fn deref(&self) -> &T {
		&self.0
	}
}

#[cfg(feature = "validation")]
impl<T: Debug> Debug for Validated<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_tuple("Validated").field(&self.0).finish()
	}
}

#[cfg(feature = "validation")]
#[async_trait::async_trait]
impl<E> super::extract::FromRequest for Validated<E>
where
	E: super::extract::FromRequest + super::has_inner::HasInner + Send,
	E::Inner: Validate,
{
	async fn from_request(
		req: &super::Request,
		ctx: &super::ParamContext,
	) -> super::ParamResult<Self> {
		let extractor = E::from_request(req, ctx).await?;
		extractor
			.inner_ref()
			.validate()
			.map_err(|errors| super::ParamError::ValidationFailed(Box::new(errors)))?;
		Ok(Validated(extractor))
	}
}

// Feature-gated trait is defined at the end of the file for non-validation builds

/// Validation constraints for a parameter
#[cfg(feature = "validation")]
pub struct ValidationConstraints<T> {
	inner: T,
	min_length: Option<usize>,
	max_length: Option<usize>,
	min_value: Option<String>,
	max_value: Option<String>,
	regex: Option<String>,
	email: bool,
	url: bool,
}

#[cfg(feature = "validation")]
impl<T> ValidationConstraints<T> {
	/// Add another min_length constraint
	pub fn min_length(mut self, min: usize) -> Self {
		self.min_length = Some(min);
		self
	}

	/// Add another max_length constraint
	pub fn max_length(mut self, max: usize) -> Self {
		self.max_length = Some(max);
		self
	}

	/// Add another min_value constraint
	pub fn min_value<V: ToString>(mut self, min: V) -> Self {
		self.min_value = Some(min.to_string());
		self
	}

	/// Add another max_value constraint
	pub fn max_value<V: ToString>(mut self, max: V) -> Self {
		self.max_value = Some(max.to_string());
		self
	}

	/// Add regex constraint
	pub fn regex(mut self, pattern: impl Into<String>) -> Self {
		self.regex = Some(pattern.into());
		self
	}

	/// Add email validation
	pub fn email(mut self) -> Self {
		self.email = true;
		self
	}

	/// Add URL validation
	pub fn url(mut self) -> Self {
		self.url = true;
		self
	}

	/// Maximum allowed length for user-supplied regex patterns (in bytes).
	/// Limits regex complexity to prevent ReDoS attacks via excessively large patterns.
	const MAX_REGEX_PATTERN_LENGTH: usize = 1024;

	/// Validate a string value against the constraints
	pub fn validate_string(&self, value: &str) -> ValidationResult<()> {
		// Length constraints
		if let Some(min) = self.min_length {
			reinhardt_core::validators::MinLengthValidator::new(min).validate(value)?;
		}
		if let Some(max) = self.max_length {
			reinhardt_core::validators::MaxLengthValidator::new(max).validate(value)?;
		}

		// Regex constraint with pattern length limit to prevent ReDoS
		if let Some(ref pattern) = self.regex {
			if pattern.len() > Self::MAX_REGEX_PATTERN_LENGTH {
				return Err(reinhardt_core::validators::ValidationError::Custom(
					format!(
						"Regex pattern length {} exceeds maximum allowed length {}",
						pattern.len(),
						Self::MAX_REGEX_PATTERN_LENGTH
					),
				));
			}
			reinhardt_core::validators::RegexValidator::new(pattern)
				.map_err(|e| {
					reinhardt_core::validators::ValidationError::Custom(format!(
						"Invalid regex pattern: {}",
						e
					))
				})?
				.validate(value)?;
		}

		// Email constraint
		if self.email {
			reinhardt_core::validators::EmailValidator::new().validate(value)?;
		}

		// URL constraint
		if self.url {
			reinhardt_core::validators::UrlValidator::new().validate(value)?;
		}

		Ok(())
	}

	/// Validate a numeric value against the constraints
	pub fn validate_number<N>(&self, value: &N) -> ValidationResult<()>
	where
		N: PartialOrd + std::fmt::Display + Clone + std::str::FromStr,
		<N as std::str::FromStr>::Err: std::fmt::Display,
	{
		if let Some(ref min_str) = self.min_value
			&& let Ok(min) = min_str.parse::<N>()
		{
			reinhardt_core::validators::MinValueValidator::new(min).validate(value)?;
		}
		if let Some(ref max_str) = self.max_value
			&& let Ok(max) = max_str.parse::<N>()
		{
			reinhardt_core::validators::MaxValueValidator::new(max).validate(value)?;
		}
		Ok(())
	}

	/// Get the inner value
	pub fn into_inner(self) -> T {
		self.inner
	}
}

#[cfg(feature = "validation")]
impl<T> Deref for ValidationConstraints<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.inner
	}
}

// ============================================================================
// WithValidation Trait (feature-gated)
// ============================================================================

/// Trait for adding validation constraints to parameters
///
/// This trait is enabled with the `validation` feature flag.
#[cfg(feature = "validation")]
pub trait WithValidation: Sized {
	/// Add minimum length constraint
	fn min_length(self, min: usize) -> ValidationConstraints<Self> {
		ValidationConstraints {
			inner: self,
			min_length: Some(min),
			max_length: None,
			min_value: None,
			max_value: None,
			regex: None,
			email: false,
			url: false,
		}
	}

	/// Add maximum length constraint
	fn max_length(self, max: usize) -> ValidationConstraints<Self> {
		ValidationConstraints {
			inner: self,
			min_length: None,
			max_length: Some(max),
			min_value: None,
			max_value: None,
			regex: None,
			email: false,
			url: false,
		}
	}

	/// Add minimum value constraint
	fn min_value<V: ToString>(self, min: V) -> ValidationConstraints<Self> {
		ValidationConstraints {
			inner: self,
			min_length: None,
			max_length: None,
			min_value: Some(min.to_string()),
			max_value: None,
			regex: None,
			email: false,
			url: false,
		}
	}

	/// Add maximum value constraint
	fn max_value<V: ToString>(self, max: V) -> ValidationConstraints<Self> {
		ValidationConstraints {
			inner: self,
			min_length: None,
			max_length: None,
			min_value: None,
			max_value: Some(max.to_string()),
			regex: None,
			email: false,
			url: false,
		}
	}

	/// Add regex pattern constraint
	fn regex(self, pattern: impl Into<String>) -> ValidationConstraints<Self> {
		ValidationConstraints {
			inner: self,
			min_length: None,
			max_length: None,
			min_value: None,
			max_value: None,
			regex: Some(pattern.into()),
			email: false,
			url: false,
		}
	}

	/// Add email validation
	fn email(self) -> ValidationConstraints<Self> {
		ValidationConstraints {
			inner: self,
			min_length: None,
			max_length: None,
			min_value: None,
			max_value: None,
			regex: None,
			email: true,
			url: false,
		}
	}

	/// Add URL validation
	fn url(self) -> ValidationConstraints<Self> {
		ValidationConstraints {
			inner: self,
			min_length: None,
			max_length: None,
			min_value: None,
			max_value: None,
			regex: None,
			email: false,
			url: true,
		}
	}
}

// WithValidation implementations are provided in their respective modules:
// - Path<T>: path.rs
// - Query<T>: query.rs
// - Form<T>: form.rs

// ============================================================================
// Type Aliases for Validated Parameters
// ============================================================================

/// Type alias for validated path parameters
///
/// This is a convenience type that wraps a `Path<T>` with validation constraints.
///
/// # Examples
///
/// ```rust,no_run
/// # use reinhardt_di::params::{ValidatedPath, WithValidation, Path};
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let req = ();
/// # let ctx = ();
/// // In your handler:
/// // async fn handler(
/// //     // Extract path parameter "id" and validate it
/// //     id: ValidatedPath<i32>,
/// // ) {
/// //     // Use the validated value
/// //     let value = id.0;
/// // }
///
// Usage pattern:
// 1. Extract Path<T> from request
// 2. Apply validation constraints
// 3. Validate
/// // let path = Path::<i32>::from_request(req, ctx).await?;
/// // let validated = path.min_value(1).max_value(100);
/// // validated.validate_number(&validated.0)?;
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "validation")]
pub type ValidatedPath<T> = ValidationConstraints<super::Path<T>>;

/// Type alias for validated query parameters
///
/// This is a convenience type that wraps a `Query<T>` with validation constraints.
#[cfg(feature = "validation")]
pub type ValidatedQuery<T> = ValidationConstraints<super::Query<T>>;

/// Type alias for validated form parameters
///
/// This is a convenience type that wraps a `Form<T>` with validation constraints.
#[cfg(feature = "validation")]
pub type ValidatedForm<T> = ValidationConstraints<super::Form<T>>;

// ============================================================================
// Non-feature-gated versions for testing
// ============================================================================

/// Validation constraints wrapper for parameter types.
///
/// Wraps an extracted parameter with configurable validation rules
/// including length limits, value ranges, regex patterns, and format checks.
#[cfg(not(feature = "validation"))]
pub struct ValidationConstraints<T> {
	/// The wrapped parameter value.
	pub inner: T,
	/// Minimum required string length.
	pub min_length: Option<usize>,
	/// Maximum allowed string length.
	pub max_length: Option<usize>,
	/// Minimum allowed value (as string for generic comparison).
	pub min_value: Option<String>,
	/// Maximum allowed value (as string for generic comparison).
	pub max_value: Option<String>,
	/// Regular expression pattern that the value must match.
	pub regex: Option<String>,
	/// Whether the value must be a valid email address.
	pub email: bool,
	/// Whether the value must be a valid URL.
	pub url: bool,
}

#[cfg(not(feature = "validation"))]
impl<T> ValidationConstraints<T> {
	/// Sets the minimum string length constraint.
	pub fn min_length(mut self, min: usize) -> Self {
		self.min_length = Some(min);
		self
	}

	/// Sets the maximum string length constraint.
	pub fn max_length(mut self, max: usize) -> Self {
		self.max_length = Some(max);
		self
	}

	/// Sets the minimum value constraint.
	pub fn min_value<V: ToString>(mut self, min: V) -> Self {
		self.min_value = Some(min.to_string());
		self
	}

	/// Sets the maximum value constraint.
	pub fn max_value<V: ToString>(mut self, max: V) -> Self {
		self.max_value = Some(max.to_string());
		self
	}

	/// Sets a regex pattern that the value must match.
	pub fn regex(mut self, pattern: impl Into<String>) -> Self {
		self.regex = Some(pattern.into());
		self
	}

	/// Enables email format validation.
	pub fn email(mut self) -> Self {
		self.email = true;
		self
	}

	/// Enables URL format validation.
	pub fn url(mut self) -> Self {
		self.url = true;
		self
	}

	/// Maximum allowed length for user-supplied regex patterns (in bytes).
	/// Limits regex complexity to prevent ReDoS attacks via excessively large patterns.
	const MAX_REGEX_PATTERN_LENGTH: usize = 1024;

	/// Validates a string value against the configured constraints.
	pub fn validate_string(&self, value: &str) -> Result<(), String> {
		if let Some(min) = self.min_length
			&& value.len() < min
		{
			return Err(format!(
				"String length {} is less than minimum {}",
				value.len(),
				min
			));
		}
		if let Some(max) = self.max_length
			&& value.len() > max
		{
			return Err(format!(
				"String length {} exceeds maximum {}",
				value.len(),
				max
			));
		}
		if let Some(ref pattern) = self.regex {
			if pattern.len() > Self::MAX_REGEX_PATTERN_LENGTH {
				return Err(format!(
					"Regex pattern length {} exceeds maximum allowed length {}",
					pattern.len(),
					Self::MAX_REGEX_PATTERN_LENGTH
				));
			}
			use regex::Regex;
			let regex = Regex::new(pattern).map_err(|e| format!("Invalid regex: {}", e))?;
			if !regex.is_match(value) {
				return Err(format!("String does not match pattern: {}", pattern));
			}
		}
		if self.email {
			if !value.contains('@') || !value.contains('.') {
				return Err("Invalid email format".to_string());
			}
			let parts: Vec<&str> = value.split('@').collect();
			if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
				return Err("Invalid email format".to_string());
			}
		}
		if self.url && !value.starts_with("http://") && !value.starts_with("https://") {
			return Err("URL must start with http:// or https://".to_string());
		}
		Ok(())
	}

	/// Validates a numeric value against the configured min/max constraints.
	pub fn validate_number<N>(&self, value: &N) -> Result<(), String>
	where
		N: PartialOrd + std::fmt::Display + Clone + std::str::FromStr,
		<N as std::str::FromStr>::Err: std::fmt::Display,
	{
		if let Some(ref min_str) = self.min_value
			&& let Ok(min) = min_str.parse::<N>()
			&& value < &min
		{
			return Err(format!("Value {} is less than minimum {}", value, min));
		}
		if let Some(ref max_str) = self.max_value
			&& let Ok(max) = max_str.parse::<N>()
			&& value > &max
		{
			return Err(format!("Value {} exceeds maximum {}", value, max));
		}
		Ok(())
	}

	/// Consumes the wrapper and returns the inner value.
	pub fn into_inner(self) -> T {
		self.inner
	}
}

#[cfg(not(feature = "validation"))]
impl<T> Deref for ValidationConstraints<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.inner
	}
}

/// A `Path<T>` parameter wrapped with validation constraints.
#[cfg(not(feature = "validation"))]
pub type ValidatedPath<T> = ValidationConstraints<super::Path<T>>;

/// A `Query<T>` parameter wrapped with validation constraints.
#[cfg(not(feature = "validation"))]
pub type ValidatedQuery<T> = ValidationConstraints<super::Query<T>>;

/// A `Form<T>` parameter wrapped with validation constraints.
#[cfg(not(feature = "validation"))]
pub type ValidatedForm<T> = ValidationConstraints<super::Form<T>>;

// Implement WithValidation trait for Path and Query
#[cfg(not(feature = "validation"))]
impl<T> WithValidation for super::Path<T> {}

#[cfg(not(feature = "validation"))]
impl<T> WithValidation for super::Query<T> {}

/// Extension trait for adding validation constraints to parameter types.
#[cfg(not(feature = "validation"))]
pub trait WithValidation: Sized {
	/// Creates a `ValidationConstraints` wrapper with a minimum length.
	fn min_length(self, min: usize) -> ValidationConstraints<Self> {
		ValidationConstraints {
			inner: self,
			min_length: Some(min),
			max_length: None,
			min_value: None,
			max_value: None,
			regex: None,
			email: false,
			url: false,
		}
	}

	/// Creates a `ValidationConstraints` wrapper with a maximum length.
	fn max_length(self, max: usize) -> ValidationConstraints<Self> {
		ValidationConstraints {
			inner: self,
			min_length: None,
			max_length: Some(max),
			min_value: None,
			max_value: None,
			regex: None,
			email: false,
			url: false,
		}
	}

	/// Creates a `ValidationConstraints` wrapper with a minimum value.
	fn min_value<V: ToString>(self, min: V) -> ValidationConstraints<Self> {
		ValidationConstraints {
			inner: self,
			min_length: None,
			max_length: None,
			min_value: Some(min.to_string()),
			max_value: None,
			regex: None,
			email: false,
			url: false,
		}
	}

	/// Creates a `ValidationConstraints` wrapper with a maximum value.
	fn max_value<V: ToString>(self, max: V) -> ValidationConstraints<Self> {
		ValidationConstraints {
			inner: self,
			min_length: None,
			max_length: None,
			min_value: None,
			max_value: Some(max.to_string()),
			regex: None,
			email: false,
			url: false,
		}
	}

	/// Creates a `ValidationConstraints` wrapper with a regex pattern.
	fn regex(self, pattern: impl Into<String>) -> ValidationConstraints<Self> {
		ValidationConstraints {
			inner: self,
			min_length: None,
			max_length: None,
			min_value: None,
			max_value: None,
			regex: Some(pattern.into()),
			email: false,
			url: false,
		}
	}

	/// Creates a `ValidationConstraints` wrapper with email format validation.
	fn email(self) -> ValidationConstraints<Self> {
		ValidationConstraints {
			inner: self,
			min_length: None,
			max_length: None,
			min_value: None,
			max_value: None,
			regex: None,
			email: true,
			url: false,
		}
	}

	/// Creates a `ValidationConstraints` wrapper with URL format validation.
	fn url(self) -> ValidationConstraints<Self> {
		ValidationConstraints {
			inner: self,
			min_length: None,
			max_length: None,
			min_value: None,
			max_value: None,
			regex: None,
			email: false,
			url: true,
		}
	}
}

#[cfg(test)]
#[cfg(feature = "validation")]
mod tests {
	use super::*;
	use crate::params::extract::FromRequest;
	use crate::params::{Form, HasInner, ParamContext, ParamError, Path};
	use bytes::Bytes;
	use reinhardt_core::validators::{Validate, ValidationError, ValidationErrors};
	use reinhardt_http::Request;
	use rstest::rstest;

	// Allow dead_code: fields are accessed via Deserialize derive, not directly in code
	#[allow(dead_code)]
	#[derive(Debug, serde::Deserialize)]
	struct TestForm {
		email: String,
	}

	impl Validate for TestForm {
		fn validate(&self) -> Result<(), ValidationErrors> {
			let mut errors = ValidationErrors::new();
			if !self.email.contains('@') {
				errors.add(
					"email",
					ValidationError::Custom("must contain @".to_string()),
				);
			}
			if errors.is_empty() {
				Ok(())
			} else {
				Err(errors)
			}
		}
	}

	fn make_form_request(body: &str) -> Request {
		use hyper::{HeaderMap, Method, Version, header};
		let mut headers = HeaderMap::new();
		headers.insert(
			header::CONTENT_TYPE,
			"application/x-www-form-urlencoded".parse().unwrap(),
		);
		Request::builder()
			.method(Method::POST)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::from(body.to_string()))
			.build()
			.unwrap()
	}

	#[rstest]
	fn test_has_inner_form_valid_data() {
		// Arrange
		let form = Form(TestForm {
			email: "user@example.com".to_string(),
		});

		// Act
		let result = form.inner_ref().validate();

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_has_inner_form_invalid_data() {
		// Arrange
		let form = Form(TestForm {
			email: "invalid".to_string(),
		});

		// Act
		let result = form.inner_ref().validate();

		// Assert
		assert!(result.is_err());
		let errors = result.unwrap_err();
		assert!(errors.field_errors().contains_key("email"));
	}

	#[rstest]
	#[tokio::test]
	async fn test_validated_form_extraction_valid() {
		// Arrange
		let req = make_form_request("email=user%40example.com");
		let ctx = ParamContext::new();

		// Act
		let result = Validated::<Form<TestForm>>::from_request(&req, &ctx).await;

		// Assert
		assert!(result.is_ok());
		let validated = result.unwrap();
		assert_eq!(validated.into_inner().0.email, "user@example.com");
	}

	#[rstest]
	#[tokio::test]
	async fn test_validated_form_extraction_invalid() {
		// Arrange
		let req = make_form_request("email=invalid");
		let ctx = ParamContext::new();

		// Act
		let result = Validated::<Form<TestForm>>::from_request(&req, &ctx).await;

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(
			matches!(err, ParamError::ValidationFailed(_)),
			"expected ValidationFailed, got: {:?}",
			err
		);
	}

	#[rstest]
	fn test_validation_constraints_builder() {
		// Arrange
		let path = Path(42i32);
		let constrained = path.min_value(0).max_value(100);

		// Act & Assert
		assert!(constrained.validate_number(&42).is_ok());
		assert!(constrained.validate_number(&-1).is_err());
		assert!(constrained.validate_number(&101).is_err());
	}

	#[rstest]
	fn test_string_validation_constraints() {
		// Arrange
		let path = Path("test".to_string());
		let constrained = path.min_length(2).max_length(10);

		// Act & Assert
		assert!(constrained.validate_string("test").is_ok());
		assert!(constrained.validate_string("a").is_err());
		assert!(constrained.validate_string("this is too long").is_err());
	}

	#[rstest]
	fn test_regex_pattern_length_limit_rejects_oversized_patterns() {
		// Arrange
		let path = Path("test".to_string());
		let oversized_pattern = "a".repeat(2048);
		let constrained = path.regex(oversized_pattern);

		// Act
		let result = constrained.validate_string("test");

		// Assert
		assert!(result.is_err());
		let err_msg = format!("{}", result.unwrap_err());
		assert!(
			err_msg.contains("exceeds maximum allowed length"),
			"Expected pattern length error, got: {}",
			err_msg
		);
	}

	#[rstest]
	fn test_regex_pattern_within_limit_succeeds() {
		// Arrange
		let path = Path("hello123".to_string());
		let valid_pattern = r"^[a-zA-Z0-9]+$";
		let constrained = path.regex(valid_pattern);

		// Act
		let result = constrained.validate_string("hello123");

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_regex_pattern_just_over_limit_is_rejected() {
		// Arrange
		let path = Path("a".to_string());
		let pattern_over_limit =
			"a".repeat(ValidationConstraints::<Path<String>>::MAX_REGEX_PATTERN_LENGTH + 1);
		let constrained = path.regex(pattern_over_limit);

		// Act
		let result = constrained.validate_string("a");

		// Assert
		assert!(result.is_err());
		let err_msg = format!("{}", result.unwrap_err());
		assert!(
			err_msg.contains("exceeds maximum allowed length"),
			"Expected pattern length error, got: {}",
			err_msg
		);
	}
}

#[cfg(test)]
#[cfg(not(feature = "validation"))]
mod tests_non_validation {
	use super::*;
	use crate::params::Path;
	use rstest::rstest;

	#[rstest]
	fn test_regex_pattern_length_limit_rejects_oversized_patterns() {
		// Arrange
		let path = Path("test".to_string());
		let oversized_pattern = "a".repeat(2048);
		let constrained = path.regex(oversized_pattern);

		// Act
		let result = constrained.validate_string("test");

		// Assert
		assert!(result.is_err());
		let err_msg = result.unwrap_err();
		assert!(
			err_msg.contains("exceeds maximum allowed length"),
			"Expected pattern length error, got: {}",
			err_msg
		);
	}

	#[rstest]
	fn test_regex_pattern_within_limit_succeeds() {
		// Arrange
		let path = Path("hello123".to_string());
		let valid_pattern = r"^[a-zA-Z0-9]+$";
		let constrained = path.regex(valid_pattern);

		// Act
		let result = constrained.validate_string("hello123");

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_regex_pattern_just_over_limit_is_rejected() {
		// Arrange
		let path = Path("a".to_string());
		let pattern_over_limit =
			"a".repeat(ValidationConstraints::<Path<String>>::MAX_REGEX_PATTERN_LENGTH + 1);
		let constrained = path.regex(pattern_over_limit);

		// Act
		let result = constrained.validate_string("a");

		// Assert
		assert!(result.is_err());
		let err_msg = result.unwrap_err();
		assert!(
			err_msg.contains("exceeds maximum allowed length"),
			"Expected pattern length error, got: {}",
			err_msg
		);
	}
}

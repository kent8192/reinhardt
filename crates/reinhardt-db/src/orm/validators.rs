/// Field validators similar to Django's validators
use once_cell::sync::Lazy;
use regex::Regex;
use reinhardt_core::exception::Result;
use reinhardt_core::validators::{
	self as validators_crate, OrmValidator, ValidationError as BaseValidationError,
	ValidationResult,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Validation error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
	pub field: String,
	pub message: String,
	pub code: String,
}

impl ValidationError {
	/// Create a new validation error
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::validators::ValidationError;
	///
	/// let error = ValidationError::new(
	///     "email",
	///     "Enter a valid email address",
	///     "invalid_email"
	/// );
	/// assert_eq!(error.field, "email");
	/// assert_eq!(error.code, "invalid_email");
	/// ```
	pub fn new(
		field: impl Into<String>,
		message: impl Into<String>,
		code: impl Into<String>,
	) -> Self {
		Self {
			field: field.into(),
			message: message.into(),
			code: code.into(),
		}
	}
}

impl std::fmt::Display for ValidationError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}: {} ({})", self.field, self.message, self.code)
	}
}

impl std::error::Error for ValidationError {}

/// Base trait for validators
///
/// This is the ORM-specific validator trait. For the base validator trait,
/// see `reinhardt_core::validators::Validator`.
pub trait Validator: Send + Sync {
	fn validate(&self, value: &str) -> Result<()>;
	fn message(&self) -> String;
}

/// Required field validator
#[derive(Debug, Clone)]
pub struct RequiredValidator {
	pub message: String,
}

impl RequiredValidator {
	/// Create a new required field validator
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::validators::{RequiredValidator, Validator};
	///
	/// let validator = RequiredValidator::new();
	/// assert!(validator.validate("some text").is_ok());
	/// assert!(validator.validate("").is_err());
	/// assert!(validator.validate("   ").is_err());
	/// ```
	pub fn new() -> Self {
		Self {
			message: "This field is required".to_string(),
		}
	}
	/// Create a required validator with custom error message
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::validators::{RequiredValidator, Validator};
	///
	/// let validator = RequiredValidator::with_message("Username is required");
	/// assert_eq!(validator.message(), "Username is required");
	/// ```
	pub fn with_message(message: impl Into<String>) -> Self {
		Self {
			message: message.into(),
		}
	}
}

impl Default for RequiredValidator {
	fn default() -> Self {
		Self::new()
	}
}

impl Validator for RequiredValidator {
	fn validate(&self, value: &str) -> Result<()> {
		if value.trim().is_empty() {
			return Err(reinhardt_core::exception::Error::Validation(
				self.message.clone(),
			));
		}
		Ok(())
	}

	fn message(&self) -> String {
		self.message.clone()
	}
}

impl validators_crate::Validator<str> for RequiredValidator {
	fn validate(&self, value: &str) -> ValidationResult<()> {
		if value.trim().is_empty() {
			return Err(BaseValidationError::Custom(self.message.clone()));
		}
		Ok(())
	}
}

impl OrmValidator for RequiredValidator {
	fn message(&self) -> String {
		self.message.clone()
	}
}

/// Max length validator
#[derive(Debug, Clone)]
pub struct MaxLengthValidator {
	pub max_length: usize,
	pub message: String,
}

impl MaxLengthValidator {
	/// Create a new max length validator
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::validators::{MaxLengthValidator, Validator};
	///
	/// let validator = MaxLengthValidator::new(10);
	/// assert!(validator.validate("hello").is_ok());
	/// assert!(validator.validate("hello world").is_err());
	/// ```
	pub fn new(max_length: usize) -> Self {
		Self {
			max_length,
			message: format!("Ensure this value has at most {} characters", max_length),
		}
	}
	/// Create a max length validator with custom error message
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::validators::{MaxLengthValidator, Validator};
	///
	/// let validator = MaxLengthValidator::with_message(20, "Name too long");
	/// assert_eq!(validator.message(), "Name too long");
	/// ```
	pub fn with_message(max_length: usize, message: impl Into<String>) -> Self {
		Self {
			max_length,
			message: message.into(),
		}
	}
}

impl Validator for MaxLengthValidator {
	fn validate(&self, value: &str) -> Result<()> {
		if value.len() > self.max_length {
			return Err(reinhardt_core::exception::Error::Validation(
				self.message.clone(),
			));
		}
		Ok(())
	}

	fn message(&self) -> String {
		self.message.clone()
	}
}

impl validators_crate::Validator<str> for MaxLengthValidator {
	fn validate(&self, value: &str) -> ValidationResult<()> {
		if value.len() > self.max_length {
			return Err(BaseValidationError::Custom(self.message.clone()));
		}
		Ok(())
	}
}

impl OrmValidator for MaxLengthValidator {
	fn message(&self) -> String {
		self.message.clone()
	}
}

/// Min length validator
#[derive(Debug, Clone)]
pub struct MinLengthValidator {
	pub min_length: usize,
	pub message: String,
}

impl MinLengthValidator {
	/// Create a new min length validator
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::validators::{MinLengthValidator, Validator};
	///
	/// let validator = MinLengthValidator::new(3);
	/// assert!(validator.validate("hello").is_ok());
	/// assert!(validator.validate("hi").is_err());
	/// ```
	pub fn new(min_length: usize) -> Self {
		Self {
			min_length,
			message: format!("Ensure this value has at least {} characters", min_length),
		}
	}
	/// Create a min length validator with custom error message
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::validators::{MinLengthValidator, Validator};
	///
	/// let validator = MinLengthValidator::with_message(8, "Password too short");
	/// assert_eq!(validator.message(), "Password too short");
	/// ```
	pub fn with_message(min_length: usize, message: impl Into<String>) -> Self {
		Self {
			min_length,
			message: message.into(),
		}
	}
}

impl Validator for MinLengthValidator {
	fn validate(&self, value: &str) -> Result<()> {
		if value.len() < self.min_length {
			return Err(reinhardt_core::exception::Error::Validation(
				self.message.clone(),
			));
		}
		Ok(())
	}

	fn message(&self) -> String {
		self.message.clone()
	}
}

impl validators_crate::Validator<str> for MinLengthValidator {
	fn validate(&self, value: &str) -> ValidationResult<()> {
		if value.len() < self.min_length {
			return Err(BaseValidationError::Custom(self.message.clone()));
		}
		Ok(())
	}
}

impl OrmValidator for MinLengthValidator {
	fn message(&self) -> String {
		self.message.clone()
	}
}

/// Email validator
#[derive(Debug, Clone)]
pub struct EmailValidator {
	pub message: String,
}

impl EmailValidator {
	/// Create a new RFC 5322 compliant email validator
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::validators::{EmailValidator, Validator};
	///
	/// let validator = EmailValidator::new();
	/// assert!(validator.validate("user@example.com").is_ok());
	/// assert!(validator.validate("user.name+tag@example.co.uk").is_ok());
	/// assert!(validator.validate("invalid-email").is_err());
	/// assert!(validator.validate("@example.com").is_err());
	/// ```
	pub fn new() -> Self {
		Self {
			message: "Enter a valid email address".to_string(),
		}
	}
	/// Create an email validator with custom error message
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::validators::{EmailValidator, Validator};
	///
	/// let validator = EmailValidator::with_message("Please provide a valid email");
	/// assert_eq!(validator.message(), "Please provide a valid email");
	/// ```
	pub fn with_message(message: impl Into<String>) -> Self {
		Self {
			message: message.into(),
		}
	}
}

impl Default for EmailValidator {
	fn default() -> Self {
		Self::new()
	}
}

impl Validator for EmailValidator {
	fn validate(&self, value: &str) -> Result<()> {
		// RFC 5322 compliant email validation
		// This implementation follows the addr-spec format from RFC 5322
		// See: https://datatracker.ietf.org/doc/html/rfc5322#section-3.4.1

		// Split email into local and domain parts
		let parts: Vec<&str> = value.rsplitn(2, '@').collect();
		if parts.len() != 2 {
			return Err(reinhardt_core::exception::Error::Validation(
				self.message.clone(),
			));
		}

		let domain = parts[0];
		let local = parts[1];

		// Validate local part (before @)
		if !Self::validate_local_part(local) {
			return Err(reinhardt_core::exception::Error::Validation(
				self.message.clone(),
			));
		}

		// Validate domain part (after @)
		if !Self::validate_domain_part(domain) {
			return Err(reinhardt_core::exception::Error::Validation(
				self.message.clone(),
			));
		}

		Ok(())
	}

	fn message(&self) -> String {
		self.message.clone()
	}
}

impl validators_crate::Validator<str> for EmailValidator {
	fn validate(&self, value: &str) -> ValidationResult<()> {
		let parts: Vec<&str> = value.rsplitn(2, '@').collect();
		if parts.len() != 2 {
			return Err(BaseValidationError::Custom(self.message.clone()));
		}

		let domain = parts[0];
		let local = parts[1];

		if !Self::validate_local_part(local) || !Self::validate_domain_part(domain) {
			return Err(BaseValidationError::Custom(self.message.clone()));
		}

		Ok(())
	}
}

impl OrmValidator for EmailValidator {
	fn message(&self) -> String {
		self.message.clone()
	}
}

impl EmailValidator {
	/// Validate the local part of an email (before @)
	/// RFC 5322: local-part = dot-atom / quoted-string / obs-local-part
	fn validate_local_part(local: &str) -> bool {
		if local.is_empty() || local.len() > 64 {
			return false;
		}

		// Check for quoted strings
		if local.starts_with('"') && local.ends_with('"') {
			return Self::validate_quoted_string(local);
		}

		// Validate dot-atom format
		Self::validate_dot_atom(local)
	}

	/// Validate quoted string format
	fn validate_quoted_string(s: &str) -> bool {
		if s.len() < 2 {
			return false;
		}

		let inner = &s[1..s.len() - 1];
		let mut escaped = false;

		for ch in inner.chars() {
			if escaped {
				// After backslash, any ASCII char is allowed
				if !ch.is_ascii() {
					return false;
				}
				escaped = false;
			} else if ch == '\\' {
				escaped = true;
			} else if ch == '"' {
				// Unescaped quote inside quoted string is invalid
				return false;
			} else if !Self::is_qtext(ch) {
				return false;
			}
		}

		// Must not end with an escape character
		!escaped
	}

	/// Check if character is valid qtext (RFC 5322)
	fn is_qtext(ch: char) -> bool {
		ch == ' ' || ch == '\t' || (('!'..='~').contains(&ch) && ch != '"' && ch != '\\')
	}

	/// Validate dot-atom format (RFC 5322)
	fn validate_dot_atom(s: &str) -> bool {
		if s.is_empty() || s.starts_with('.') || s.ends_with('.') {
			return false;
		}

		// Check for consecutive dots
		if s.contains("..") {
			return false;
		}

		// Each atom must be valid
		s.split('.').all(Self::validate_atom)
	}

	/// Validate atom characters (RFC 5322)
	fn validate_atom(atom: &str) -> bool {
		if atom.is_empty() {
			return false;
		}

		atom.chars().all(Self::is_atext)
	}

	/// Check if character is valid atext (RFC 5322)
	fn is_atext(ch: char) -> bool {
		ch.is_ascii_alphanumeric() || "!#$%&'*+-/=?^_`{|}~".contains(ch)
	}

	/// Validate domain part (after @)
	/// RFC 5322: domain = dot-atom / domain-literal / obs-domain
	fn validate_domain_part(domain: &str) -> bool {
		if domain.is_empty() || domain.len() > 255 {
			return false;
		}

		// Check for domain literal [IPv4 or IPv6]
		if domain.starts_with('[') && domain.ends_with(']') {
			return Self::validate_domain_literal(domain);
		}

		// Validate dot-atom domain format
		Self::validate_domain_name(domain)
	}

	/// Validate domain literal (IP address in brackets)
	fn validate_domain_literal(domain: &str) -> bool {
		if domain.len() < 3 {
			return false;
		}

		let inner = &domain[1..domain.len() - 1];

		// Check for IPv6 prefix
		if inner.to_lowercase().starts_with("ipv6:") {
			// Basic IPv6 validation - can be enhanced
			return inner.len() > 5 && inner[5..].contains(':');
		}

		// IPv4 validation
		Self::validate_ipv4(inner)
	}

	/// Basic IPv4 validation
	fn validate_ipv4(s: &str) -> bool {
		let parts: Vec<&str> = s.split('.').collect();
		if parts.len() != 4 {
			return false;
		}

		parts.iter().all(|part| part.parse::<u8>().is_ok())
	}

	/// Validate domain name (dot-separated labels)
	fn validate_domain_name(domain: &str) -> bool {
		if domain.starts_with('.') || domain.ends_with('.') || domain.contains("..") {
			return false;
		}

		let labels: Vec<&str> = domain.split('.').collect();

		// Must have at least 2 labels (e.g., example.com)
		if labels.len() < 2 {
			return false;
		}

		// Each label must be valid
		labels
			.iter()
			.all(|label| Self::validate_domain_label(label))
	}

	/// Validate a single domain label
	fn validate_domain_label(label: &str) -> bool {
		if label.is_empty() || label.len() > 63 {
			return false;
		}

		// Label cannot start or end with hyphen
		if label.starts_with('-') || label.ends_with('-') {
			return false;
		}

		// Label must contain only alphanumeric and hyphen
		label
			.chars()
			.all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
	}
}

/// URL validator
#[derive(Debug, Clone)]
pub struct URLValidator {
	pub message: String,
}

impl URLValidator {
	/// Create a new URL validator
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::validators::{URLValidator, Validator};
	///
	/// let validator = URLValidator::new();
	/// assert!(validator.validate("https://example.com").is_ok());
	/// assert!(validator.validate("http://example.com/path").is_ok());
	/// assert!(validator.validate("ftp://example.com").is_err());
	/// assert!(validator.validate("example.com").is_err());
	/// ```
	pub fn new() -> Self {
		Self {
			message: "Enter a valid URL".to_string(),
		}
	}
	/// Create a URL validator with custom error message
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::validators::{URLValidator, Validator};
	///
	/// let validator = URLValidator::with_message("Please provide a valid URL");
	/// assert_eq!(validator.message(), "Please provide a valid URL");
	/// ```
	pub fn with_message(message: impl Into<String>) -> Self {
		Self {
			message: message.into(),
		}
	}
}

impl Default for URLValidator {
	fn default() -> Self {
		Self::new()
	}
}

impl Validator for URLValidator {
	fn validate(&self, value: &str) -> Result<()> {
		// URL validation with proper scheme, domain, and optional path
		static URL_REGEX: Lazy<Regex> = Lazy::new(|| {
			Regex::new(r"^https?://[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}(/.*)?$")
				.expect("Invalid URL regex pattern")
		});

		if !URL_REGEX.is_match(value) {
			return Err(reinhardt_core::exception::Error::Validation(
				self.message.clone(),
			));
		}
		Ok(())
	}

	fn message(&self) -> String {
		self.message.clone()
	}
}

impl validators_crate::Validator<str> for URLValidator {
	fn validate(&self, value: &str) -> ValidationResult<()> {
		static URL_REGEX: Lazy<Regex> = Lazy::new(|| {
			Regex::new(r"^https?://[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}(/.*)?$")
				.expect("Invalid URL regex pattern")
		});

		if !URL_REGEX.is_match(value) {
			return Err(BaseValidationError::Custom(self.message.clone()));
		}
		Ok(())
	}
}

impl OrmValidator for URLValidator {
	fn message(&self) -> String {
		self.message.clone()
	}
}

/// Regex validator with compile-time pattern validation and runtime caching
#[derive(Debug)]
pub struct RegexValidator {
	pattern: String,
	compiled_regex: Regex,
	message: String,
}

impl Clone for RegexValidator {
	fn clone(&self) -> Self {
		Self {
			pattern: self.pattern.clone(),
			compiled_regex: Regex::new(&self.pattern)
				.expect("Regex should be valid since it was validated at construction"),
			message: self.message.clone(),
		}
	}
}

impl RegexValidator {
	/// Create a new RegexValidator with compile-time pattern validation
	///
	/// # Panics
	/// Panics if the regex pattern is invalid. This is intentional to catch
	/// regex errors at initialization time rather than at validation time.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::validators::{RegexValidator, Validator};
	///
	/// let validator = RegexValidator::new(r"^\d{3}-\d{4}$");
	/// assert!(validator.validate("123-4567").is_ok());
	/// assert!(validator.validate("abc-defg").is_err());
	/// ```
	pub fn new(pattern: impl Into<String>) -> Self {
		let pattern = pattern.into();
		let compiled_regex = Regex::new(&pattern)
			.unwrap_or_else(|e| panic!("Invalid regex pattern '{}': {}", pattern, e));

		Self {
			message: format!("Value does not match pattern: {}", pattern),
			pattern,
			compiled_regex,
		}
	}
	/// Create a new RegexValidator with a custom error message
	///
	/// # Panics
	/// Panics if the regex pattern is invalid.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::validators::{RegexValidator, Validator};
	///
	/// let validator = RegexValidator::with_message(r"^\d{5}$", "ZIP code must be 5 digits");
	/// assert_eq!(validator.message(), "ZIP code must be 5 digits");
	/// assert!(validator.validate("12345").is_ok());
	/// assert!(validator.validate("1234").is_err());
	/// ```
	pub fn with_message(pattern: impl Into<String>, message: impl Into<String>) -> Self {
		let pattern = pattern.into();
		let compiled_regex = Regex::new(&pattern)
			.unwrap_or_else(|e| panic!("Invalid regex pattern '{}': {}", pattern, e));

		Self {
			pattern,
			compiled_regex,
			message: message.into(),
		}
	}
	/// Try to create a new RegexValidator, returning an error if the pattern is invalid
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::validators::RegexValidator;
	///
	/// // Valid pattern
	/// let result = RegexValidator::try_new(r"^\d+$");
	/// assert!(result.is_ok());
	///
	/// // Invalid pattern
	/// let result = RegexValidator::try_new(r"[invalid(regex");
	/// assert!(result.is_err());
	/// ```
	pub fn try_new(pattern: impl Into<String>) -> std::result::Result<Self, regex::Error> {
		let pattern = pattern.into();
		let compiled_regex = Regex::new(&pattern)?;

		Ok(Self {
			message: format!("Value does not match pattern: {}", pattern),
			pattern,
			compiled_regex,
		})
	}
	/// Get the regex pattern
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::validators::RegexValidator;
	///
	/// let validator = RegexValidator::new(r"^\d{3}-\d{4}$");
	/// assert_eq!(validator.pattern(), r"^\d{3}-\d{4}$");
	/// ```
	pub fn pattern(&self) -> &str {
		&self.pattern
	}
}

impl Validator for RegexValidator {
	fn validate(&self, value: &str) -> Result<()> {
		if !self.compiled_regex.is_match(value) {
			return Err(reinhardt_core::exception::Error::Validation(
				self.message.clone(),
			));
		}
		Ok(())
	}

	fn message(&self) -> String {
		self.message.clone()
	}
}

impl validators_crate::Validator<str> for RegexValidator {
	fn validate(&self, value: &str) -> ValidationResult<()> {
		if !self.compiled_regex.is_match(value) {
			return Err(BaseValidationError::Custom(self.message.clone()));
		}
		Ok(())
	}
}

impl OrmValidator for RegexValidator {
	fn message(&self) -> String {
		self.message.clone()
	}
}

/// Numeric range validator
#[derive(Debug, Clone)]
pub struct RangeValidator {
	pub min: Option<i64>,
	pub max: Option<i64>,
	pub message: String,
}

impl RangeValidator {
	/// Create a new numeric range validator
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::validators::{RangeValidator, Validator};
	///
	/// let validator = RangeValidator::new(Some(0), Some(100));
	/// assert!(validator.validate("50").is_ok());
	/// assert!(validator.validate("0").is_ok());
	/// assert!(validator.validate("100").is_ok());
	/// assert!(validator.validate("-1").is_err());
	/// assert!(validator.validate("101").is_err());
	/// ```
	pub fn new(min: Option<i64>, max: Option<i64>) -> Self {
		let message = match (min, max) {
			(Some(min), Some(max)) => format!("Value must be between {} and {}", min, max),
			(Some(min), None) => format!("Value must be at least {}", min),
			(None, Some(max)) => format!("Value must be at most {}", max),
			(None, None) => "Invalid range".to_string(),
		};
		Self { min, max, message }
	}
	/// Create a range validator with custom error message
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::validators::{RangeValidator, Validator};
	///
	/// let validator = RangeValidator::with_message(Some(18), Some(65), "Age must be 18-65");
	/// assert_eq!(validator.message(), "Age must be 18-65");
	/// ```
	pub fn with_message(min: Option<i64>, max: Option<i64>, message: impl Into<String>) -> Self {
		Self {
			min,
			max,
			message: message.into(),
		}
	}
}

impl Validator for RangeValidator {
	fn validate(&self, value: &str) -> Result<()> {
		let num: i64 = value.parse().map_err(|_| {
			reinhardt_core::exception::Error::Validation("Invalid number".to_string())
		})?;

		if let Some(min) = self.min
			&& num < min
		{
			return Err(reinhardt_core::exception::Error::Validation(
				self.message.clone(),
			));
		}

		if let Some(max) = self.max
			&& num > max
		{
			return Err(reinhardt_core::exception::Error::Validation(
				self.message.clone(),
			));
		}

		Ok(())
	}

	fn message(&self) -> String {
		self.message.clone()
	}
}

impl validators_crate::Validator<str> for RangeValidator {
	fn validate(&self, value: &str) -> ValidationResult<()> {
		let num: i64 = value
			.parse()
			.map_err(|_| BaseValidationError::Custom("Invalid number".to_string()))?;

		if let Some(min) = self.min
			&& num < min
		{
			return Err(BaseValidationError::Custom(self.message.clone()));
		}

		if let Some(max) = self.max
			&& num > max
		{
			return Err(BaseValidationError::Custom(self.message.clone()));
		}

		Ok(())
	}
}

impl OrmValidator for RangeValidator {
	fn message(&self) -> String {
		self.message.clone()
	}
}

/// Validator collection for a field
pub struct FieldValidators {
	pub validators: Vec<Box<dyn Validator>>,
}

impl FieldValidators {
	/// Create a new empty field validators collection
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::validators::{FieldValidators, RequiredValidator};
	///
	/// let validators = FieldValidators::new()
	///     .with_validator(Box::new(RequiredValidator::new()));
	/// // Verify validator was added (type check passes)
	/// let _: FieldValidators = validators;
	/// ```
	pub fn new() -> Self {
		Self {
			validators: Vec::new(),
		}
	}
	/// Add a validator to this field's validator chain
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::validators::{FieldValidators, RequiredValidator, MaxLengthValidator};
	///
	/// let validators = FieldValidators::new()
	///     .with_validator(Box::new(RequiredValidator::new()))
	///     .with_validator(Box::new(MaxLengthValidator::new(100)));
	/// // Verify both validators were added successfully
	/// assert!(validators.validate("hello").is_ok());
	/// assert!(validators.validate("").is_err()); // Fails RequiredValidator
	/// ```
	pub fn with_validator(mut self, validator: Box<dyn Validator>) -> Self {
		self.validators.push(validator);
		self
	}
	/// Validate a value against all validators in this collection
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::validators::{FieldValidators, RequiredValidator, MaxLengthValidator, Validator};
	///
	/// let validators = FieldValidators::new()
	///     .with_validator(Box::new(RequiredValidator::new()))
	///     .with_validator(Box::new(MaxLengthValidator::new(10)));
	///
	/// assert!(validators.validate("hello").is_ok());
	/// assert!(validators.validate("").is_err()); // Required
	/// assert!(validators.validate("hello world long text").is_err()); // Too long
	/// ```
	pub fn validate(&self, value: &str) -> Result<()> {
		for validator in &self.validators {
			validator.validate(value)?;
		}
		Ok(())
	}
}

impl Default for FieldValidators {
	fn default() -> Self {
		Self::new()
	}
}

/// Model validators collection
pub struct ModelValidators {
	pub field_validators: HashMap<String, FieldValidators>,
}

impl ModelValidators {
	/// Create a new model validators collection
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::validators::ModelValidators;
	///
	/// let mut model_validators = ModelValidators::new();
	/// assert_eq!(model_validators.field_validators.len(), 0);
	/// ```
	pub fn new() -> Self {
		Self {
			field_validators: HashMap::new(),
		}
	}
	/// Register validators for a specific field
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::validators::{ModelValidators, FieldValidators, EmailValidator};
	///
	/// let mut model_validators = ModelValidators::new();
	/// let email_validators = FieldValidators::new()
	///     .with_validator(Box::new(EmailValidator::new()));
	/// model_validators.add_field_validator("email".to_string(), email_validators);
	/// assert_eq!(model_validators.field_validators.len(), 1);
	/// assert!(model_validators.field_validators.contains_key("email"));
	/// ```
	pub fn add_field_validator(&mut self, field: String, validators: FieldValidators) {
		self.field_validators.insert(field, validators);
	}
	/// Validate a single field's value
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::validators::{ModelValidators, FieldValidators, EmailValidator};
	///
	/// let mut model_validators = ModelValidators::new();
	/// let email_validators = FieldValidators::new()
	///     .with_validator(Box::new(EmailValidator::new()));
	/// model_validators.add_field_validator("email".to_string(), email_validators);
	///
	/// assert!(model_validators.validate("email", "test@example.com").is_ok());
	/// assert!(model_validators.validate("email", "invalid").is_err());
	/// ```
	pub fn validate(&self, field: &str, value: &str) -> Result<()> {
		if let Some(validators) = self.field_validators.get(field) {
			validators.validate(value)?;
		}
		Ok(())
	}
	/// Validate all fields in a data map and return all validation errors
	///
	/// # Examples
	///
	/// ```
	/// use std::collections::HashMap;
	/// use reinhardt_db::orm::validators::{ModelValidators, FieldValidators, MinLengthValidator, EmailValidator};
	///
	/// let mut model_validators = ModelValidators::new();
	///
	/// let username_validators = FieldValidators::new()
	///     .with_validator(Box::new(MinLengthValidator::new(3)));
	/// let email_validators = FieldValidators::new()
	///     .with_validator(Box::new(EmailValidator::new()));
	///
	/// model_validators.add_field_validator("username".to_string(), username_validators);
	/// model_validators.add_field_validator("email".to_string(), email_validators);
	///
	/// let mut data = HashMap::new();
	/// data.insert("username".to_string(), "ab".to_string());
	/// data.insert("email".to_string(), "invalid".to_string());
	///
	/// let errors = model_validators.validate_all(&data);
	/// assert_eq!(errors.len(), 2);
	/// ```
	pub fn validate_all(&self, data: &HashMap<String, String>) -> Vec<ValidationError> {
		let mut errors = Vec::new();

		for (field, validators) in &self.field_validators {
			if let Some(value) = data.get(field)
				&& let Err(e) = validators.validate(value)
			{
				errors.push(ValidationError::new(
					field,
					e.to_string(),
					"validation_error",
				));
			}
		}

		errors
	}
}

impl Default for ModelValidators {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_orm_validators_required() {
		let validator = RequiredValidator::new();
		assert!(validator.validate("test").is_ok());
		assert!(validator.validate("").is_err());
		assert!(validator.validate("   ").is_err());
	}

	#[test]
	fn test_orm_validators_max_length() {
		let validator = MaxLengthValidator::new(5);
		assert!(validator.validate("test").is_ok());
		assert!(validator.validate("test1").is_ok());
		assert!(validator.validate("test12").is_err());
	}

	#[test]
	fn test_orm_validators_min_length() {
		let validator = MinLengthValidator::new(3);
		assert!(validator.validate("test").is_ok());
		assert!(validator.validate("te").is_err());
	}

	#[test]
	fn test_email_validator() {
		let validator = EmailValidator::new();

		// Valid emails - standard format
		assert!(validator.validate("test@example.com").is_ok());
		assert!(validator.validate("user.name@example.com").is_ok());
		assert!(validator.validate("user+tag@example.co.uk").is_ok());
		assert!(validator.validate("first.last@sub.example.com").is_ok());

		// Valid emails - special characters allowed in local part
		assert!(validator.validate("user!tag@example.com").is_ok());
		assert!(validator.validate("user#tag@example.com").is_ok());
		assert!(validator.validate("user$tag@example.com").is_ok());
		assert!(validator.validate("user%tag@example.com").is_ok());
		assert!(validator.validate("user&tag@example.com").is_ok());
		assert!(validator.validate("user*tag@example.com").is_ok());
		assert!(validator.validate("user=tag@example.com").is_ok());
		assert!(validator.validate("user?tag@example.com").is_ok());
		assert!(validator.validate("user^tag@example.com").is_ok());
		assert!(validator.validate("user_tag@example.com").is_ok());
		assert!(validator.validate("user`tag@example.com").is_ok());
		assert!(validator.validate("user{tag@example.com").is_ok());
		assert!(validator.validate("user|tag@example.com").is_ok());
		assert!(validator.validate("user}tag@example.com").is_ok());
		assert!(validator.validate("user~tag@example.com").is_ok());

		// Valid emails - quoted strings
		assert!(validator.validate(r#""user name"@example.com"#).is_ok());
		assert!(validator.validate(r#""user@name"@example.com"#).is_ok());
		assert!(validator.validate(r#""user\"name"@example.com"#).is_ok());

		// Valid emails - IP addresses
		assert!(validator.validate("user@[192.168.1.1]").is_ok());
		assert!(validator.validate("user@[127.0.0.1]").is_ok());

		// Valid emails - IPv6 (basic)
		assert!(validator.validate("user@[IPv6:2001:db8::1]").is_ok());

		// Invalid emails - basic format errors
		assert!(validator.validate("invalid").is_err());
		assert!(validator.validate("no-at-sign.com").is_err());
		assert!(validator.validate("@example.com").is_err());
		assert!(validator.validate("user@").is_err());
		assert!(validator.validate("user @example.com").is_err());

		// Invalid emails - dot-atom errors
		assert!(validator.validate(".user@example.com").is_err());
		assert!(validator.validate("user.@example.com").is_err());
		assert!(validator.validate("user..name@example.com").is_err());

		// Invalid emails - domain errors
		assert!(validator.validate("user@.example.com").is_err());
		assert!(validator.validate("user@example.com.").is_err());
		assert!(validator.validate("user@example..com").is_err());
		assert!(validator.validate("user@-example.com").is_err());
		assert!(validator.validate("user@example-.com").is_err());
		assert!(validator.validate("user@example").is_err()); // Must have at least 2 labels

		// Invalid emails - length errors
		let long_local = "a".repeat(65);
		assert!(
			validator
				.validate(&format!("{}@example.com", long_local))
				.is_err()
		);

		// Invalid emails - quoted string errors
		assert!(validator.validate(r#""user"name"@example.com"#).is_err()); // Unescaped quote
		assert!(validator.validate(r#""user\"@example.com"#).is_err()); // Unclosed quote
	}

	#[test]
	fn test_url_validator() {
		let validator = URLValidator::new();
		// Valid URLs
		assert!(validator.validate("http://example.com").is_ok());
		assert!(validator.validate("https://example.com").is_ok());
		assert!(validator.validate("https://example.com/path").is_ok());
		assert!(validator.validate("http://sub.example.com").is_ok());

		// Invalid URLs
		assert!(validator.validate("example.com").is_err());
		assert!(validator.validate("ftp://example.com").is_err());
		assert!(validator.validate("http://").is_err());
	}

	#[test]
	fn test_regex_validator_valid_pattern() {
		let validator = RegexValidator::new(r"^\d{3}-\d{4}$");
		assert!(validator.validate("123-4567").is_ok());
		assert!(validator.validate("999-0000").is_ok());
		assert!(validator.validate("12-4567").is_err());
		assert!(validator.validate("123-45678").is_err());
		assert!(validator.validate("abc-defg").is_err());
	}

	#[test]
	fn test_regex_validator_alphanumeric() {
		let validator = RegexValidator::new(r"^[a-zA-Z0-9]+$");
		assert!(validator.validate("abc123").is_ok());
		assert!(validator.validate("ABC").is_ok());
		assert!(validator.validate("123").is_ok());
		assert!(validator.validate("abc-123").is_err());
		assert!(validator.validate("abc 123").is_err());
	}

	#[test]
	#[should_panic(expected = "Invalid regex pattern")]
	fn test_regex_validator_invalid_pattern() {
		// This should panic at construction time
		RegexValidator::new(r"[invalid(regex");
	}

	#[test]
	fn test_regex_validator_try_new_valid() {
		let result = RegexValidator::try_new(r"^\d+$");
		let validator = result.unwrap();
		assert!(validator.validate("123").is_ok());
		assert!(validator.validate("abc").is_err());
	}

	#[test]
	fn test_regex_validator_try_new_invalid() {
		let result = RegexValidator::try_new(r"[invalid(regex");
		assert!(result.is_err());
	}

	#[test]
	fn test_regex_validator_with_message() {
		use reinhardt_core::validators::OrmValidator;
		let validator = RegexValidator::with_message(r"^\d{5}$", "ZIP code must be 5 digits");
		assert_eq!(
			OrmValidator::message(&validator),
			"ZIP code must be 5 digits"
		);
		assert!(validator.validate("12345").is_ok());
		assert!(validator.validate("1234").is_err());
	}

	#[test]
	fn test_regex_validator_pattern() {
		let validator = RegexValidator::new(r"^\d+$");
		assert_eq!(validator.pattern(), r"^\d+$");
	}

	#[test]
	fn test_orm_range_validator() {
		let validator = RangeValidator::new(Some(0), Some(100));
		assert!(validator.validate("50").is_ok());
		assert!(validator.validate("0").is_ok());
		assert!(validator.validate("100").is_ok());
		assert!(validator.validate("-1").is_err());
		assert!(validator.validate("101").is_err());
	}

	#[test]
	fn test_field_validators() {
		let validators = FieldValidators::new()
			.with_validator(Box::new(RequiredValidator::new()))
			.with_validator(Box::new(MaxLengthValidator::new(10)));

		assert!(validators.validate("test").is_ok());
		assert!(validators.validate("").is_err());
		assert!(validators.validate("12345678901").is_err());
	}

	#[test]
	fn test_model_validators() {
		let mut model_validators = ModelValidators::new();

		let email_validators = FieldValidators::new()
			.with_validator(Box::new(RequiredValidator::new()))
			.with_validator(Box::new(EmailValidator::new()));

		model_validators.add_field_validator("email".to_string(), email_validators);

		assert!(
			model_validators
				.validate("email", "test@example.com")
				.is_ok()
		);
		assert!(model_validators.validate("email", "invalid").is_err());
	}

	#[test]
	fn test_validate_all() {
		let mut model_validators = ModelValidators::new();

		let username_validators = FieldValidators::new()
			.with_validator(Box::new(MinLengthValidator::new(3)))
			.with_validator(Box::new(MaxLengthValidator::new(20)));

		let email_validators =
			FieldValidators::new().with_validator(Box::new(EmailValidator::new()));

		model_validators.add_field_validator("username".to_string(), username_validators);
		model_validators.add_field_validator("email".to_string(), email_validators);

		let mut data = HashMap::new();
		data.insert("username".to_string(), "ab".to_string());
		data.insert("email".to_string(), "invalid".to_string());

		let errors = model_validators.validate_all(&data);
		assert_eq!(errors.len(), 2);
	}
}

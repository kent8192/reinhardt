//! Field types for serializers
//!
//! This module provides Django REST Framework-inspired field types for data validation
//! and transformation in serializers.

use chrono::{NaiveDate, NaiveDateTime};
use std::fmt;

/// Errors that can occur during field validation
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum FieldError {
	/// Value is required but was not provided
	Required,
	/// Value is null but field does not allow null
	Null,
	/// String is too short (contains min_length)
	TooShort(usize),
	/// String is too long (contains max_length)
	TooLong(usize),
	/// Integer is below minimum (contains min_value)
	TooSmall(i64),
	/// Integer is above maximum (contains max_value)
	TooLarge(i64),
	/// Float is below minimum (contains min_value)
	TooSmallFloat(f64),
	/// Float is above maximum (contains max_value)
	TooLargeFloat(f64),
	/// Invalid email format
	InvalidEmail,
	/// Invalid URL format
	InvalidUrl,
	/// Invalid choice (value not in choices list)
	InvalidChoice,
	/// Invalid date format
	InvalidDate,
	/// Invalid datetime format
	InvalidDateTime,
	/// Custom validation error with message
	Custom(String),
}

impl fmt::Display for FieldError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			FieldError::Required => write!(f, "This field is required"),
			FieldError::Null => write!(f, "This field may not be null"),
			FieldError::TooShort(min) => write!(f, "String is too short (min: {})", min),
			FieldError::TooLong(max) => write!(f, "String is too long (max: {})", max),
			FieldError::TooSmall(min) => write!(f, "Value is too small (min: {})", min),
			FieldError::TooLarge(max) => write!(f, "Value is too large (max: {})", max),
			FieldError::TooSmallFloat(min) => write!(f, "Value is too small (min: {})", min),
			FieldError::TooLargeFloat(max) => write!(f, "Value is too large (max: {})", max),
			FieldError::InvalidEmail => write!(f, "Enter a valid email address"),
			FieldError::InvalidUrl => write!(f, "Enter a valid URL"),
			FieldError::InvalidChoice => write!(f, "Invalid choice"),
			FieldError::InvalidDate => write!(f, "Invalid date format"),
			FieldError::InvalidDateTime => write!(f, "Invalid datetime format"),
			FieldError::Custom(msg) => write!(f, "{}", msg),
		}
	}
}

impl std::error::Error for FieldError {}

/// String field with length validation
///
/// # Example
///
/// ```rust
/// use reinhardt_core::serializers::fields::CharField;
///
/// let field = CharField::new()
///     .min_length(3)
///     .max_length(10)
///     .required(true);
///
/// // Valid string
/// assert!(field.validate("hello").is_ok());
///
/// // Too short
/// assert!(field.validate("hi").is_err());
///
/// // Too long
/// assert!(field.validate("hello world").is_err());
/// ```
#[derive(Debug, Clone)]
pub struct CharField {
	/// Whether this field is required.
	pub required: bool,
	/// Whether null values are allowed.
	pub allow_null: bool,
	/// Whether blank (empty) strings are allowed.
	pub allow_blank: bool,
	/// Minimum character length constraint.
	pub min_length: Option<usize>,
	/// Maximum character length constraint.
	pub max_length: Option<usize>,
	/// Default value when none is provided.
	pub default: Option<String>,
}

impl CharField {
	/// Create a new CharField with default settings
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::serializers::fields::CharField;
	///
	/// let field = CharField::new();
	/// assert!(field.validate("test").is_ok());
	/// ```
	pub fn new() -> Self {
		Self {
			required: true,
			allow_null: false,
			allow_blank: false,
			min_length: None,
			max_length: None,
			default: None,
		}
	}

	/// Set whether the field is required
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::serializers::fields::CharField;
	///
	/// let field = CharField::new().required(false).allow_blank(true);
	/// assert!(field.validate("").is_ok());
	/// ```
	pub fn required(mut self, required: bool) -> Self {
		self.required = required;
		self
	}

	/// Set whether null values are allowed
	pub fn allow_null(mut self, allow_null: bool) -> Self {
		self.allow_null = allow_null;
		self
	}

	/// Set whether blank strings are allowed
	pub fn allow_blank(mut self, allow_blank: bool) -> Self {
		self.allow_blank = allow_blank;
		self
	}

	/// Set minimum length
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::serializers::fields::CharField;
	///
	/// let field = CharField::new().min_length(5);
	/// assert!(field.validate("hello").is_ok());
	/// assert!(field.validate("hi").is_err());
	/// ```
	pub fn min_length(mut self, min_length: usize) -> Self {
		self.min_length = Some(min_length);
		self
	}

	/// Set maximum length
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::serializers::fields::CharField;
	///
	/// let field = CharField::new().max_length(10);
	/// assert!(field.validate("hello").is_ok());
	/// assert!(field.validate("hello world").is_err());
	/// ```
	pub fn max_length(mut self, max_length: usize) -> Self {
		self.max_length = Some(max_length);
		self
	}

	/// Set default value
	pub fn default(mut self, default: String) -> Self {
		self.default = Some(default);
		self
	}

	/// Validate a string value
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::serializers::fields::CharField;
	///
	/// let field = CharField::new()
	///     .min_length(3)
	///     .max_length(10);
	///
	/// assert!(field.validate("hello").is_ok());
	/// assert!(field.validate("hi").is_err()); // Too short
	/// assert!(field.validate("hello world").is_err()); // Too long
	/// ```
	pub fn validate(&self, value: &str) -> Result<(), FieldError> {
		if value.is_empty() && !self.allow_blank {
			return Err(FieldError::Required);
		}

		let char_count = value.chars().count();

		if let Some(min) = self.min_length
			&& char_count < min
		{
			return Err(FieldError::TooShort(min));
		}

		if let Some(max) = self.max_length
			&& char_count > max
		{
			return Err(FieldError::TooLong(max));
		}

		Ok(())
	}
}

impl Default for CharField {
	fn default() -> Self {
		Self::new()
	}
}

/// Integer field with range validation
///
/// # Example
///
/// ```rust
/// use reinhardt_core::serializers::fields::IntegerField;
///
/// let field = IntegerField::new()
///     .min_value(0)
///     .max_value(100);
///
/// assert!(field.validate(50).is_ok());
/// assert!(field.validate(-1).is_err()); // Below min
/// assert!(field.validate(101).is_err()); // Above max
/// ```
#[derive(Debug, Clone)]
pub struct IntegerField {
	/// Whether this field is required.
	pub required: bool,
	/// Whether null values are allowed.
	pub allow_null: bool,
	/// Minimum value constraint.
	pub min_value: Option<i64>,
	/// Maximum value constraint.
	pub max_value: Option<i64>,
	/// Default value when none is provided.
	pub default: Option<i64>,
}

impl IntegerField {
	/// Create a new IntegerField with default settings
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::serializers::fields::IntegerField;
	///
	/// let field = IntegerField::new();
	/// assert!(field.validate(42).is_ok());
	/// ```
	pub fn new() -> Self {
		Self {
			required: true,
			allow_null: false,
			min_value: None,
			max_value: None,
			default: None,
		}
	}

	/// Set whether the field is required
	pub fn required(mut self, required: bool) -> Self {
		self.required = required;
		self
	}

	/// Set whether null values are allowed
	pub fn allow_null(mut self, allow_null: bool) -> Self {
		self.allow_null = allow_null;
		self
	}

	/// Set minimum value
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::serializers::fields::IntegerField;
	///
	/// let field = IntegerField::new().min_value(0);
	/// assert!(field.validate(10).is_ok());
	/// assert!(field.validate(-1).is_err());
	/// ```
	pub fn min_value(mut self, min_value: i64) -> Self {
		self.min_value = Some(min_value);
		self
	}

	/// Set maximum value
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::serializers::fields::IntegerField;
	///
	/// let field = IntegerField::new().max_value(100);
	/// assert!(field.validate(50).is_ok());
	/// assert!(field.validate(101).is_err());
	/// ```
	pub fn max_value(mut self, max_value: i64) -> Self {
		self.max_value = Some(max_value);
		self
	}

	/// Set default value
	pub fn default(mut self, default: i64) -> Self {
		self.default = Some(default);
		self
	}

	/// Validate an integer value
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::serializers::fields::IntegerField;
	///
	/// let field = IntegerField::new()
	///     .min_value(0)
	///     .max_value(100);
	///
	/// assert!(field.validate(50).is_ok());
	/// assert!(field.validate(-1).is_err());
	/// assert!(field.validate(101).is_err());
	/// ```
	pub fn validate(&self, value: i64) -> Result<(), FieldError> {
		if let Some(min) = self.min_value
			&& value < min
		{
			return Err(FieldError::TooSmall(min));
		}

		if let Some(max) = self.max_value
			&& value > max
		{
			return Err(FieldError::TooLarge(max));
		}

		Ok(())
	}
}

impl Default for IntegerField {
	fn default() -> Self {
		Self::new()
	}
}

/// Float field with range validation
///
/// # Example
///
/// ```rust
/// use reinhardt_core::serializers::fields::FloatField;
///
/// let field = FloatField::new()
///     .min_value(0.0)
///     .max_value(1.0);
///
/// assert!(field.validate(0.5).is_ok());
/// assert!(field.validate(-0.1).is_err()); // Below min
/// assert!(field.validate(1.1).is_err()); // Above max
/// ```
#[derive(Debug, Clone)]
pub struct FloatField {
	/// Whether this field is required.
	pub required: bool,
	/// Whether null values are allowed.
	pub allow_null: bool,
	/// Minimum value constraint.
	pub min_value: Option<f64>,
	/// Maximum value constraint.
	pub max_value: Option<f64>,
	/// Default value when none is provided.
	pub default: Option<f64>,
}

impl FloatField {
	/// Create a new FloatField with default settings
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::serializers::fields::FloatField;
	///
	/// let field = FloatField::new();
	/// assert!(field.validate(3.15).is_ok());
	/// ```
	pub fn new() -> Self {
		Self {
			required: true,
			allow_null: false,
			min_value: None,
			max_value: None,
			default: None,
		}
	}

	/// Set whether the field is required
	pub fn required(mut self, required: bool) -> Self {
		self.required = required;
		self
	}

	/// Set whether null values are allowed
	pub fn allow_null(mut self, allow_null: bool) -> Self {
		self.allow_null = allow_null;
		self
	}

	/// Set minimum value
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::serializers::fields::FloatField;
	///
	/// let field = FloatField::new().min_value(0.0);
	/// assert!(field.validate(1.5).is_ok());
	/// assert!(field.validate(-0.5).is_err());
	/// ```
	pub fn min_value(mut self, min_value: f64) -> Self {
		self.min_value = Some(min_value);
		self
	}

	/// Set maximum value
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::serializers::fields::FloatField;
	///
	/// let field = FloatField::new().max_value(10.0);
	/// assert!(field.validate(5.0).is_ok());
	/// assert!(field.validate(10.1).is_err());
	/// ```
	pub fn max_value(mut self, max_value: f64) -> Self {
		self.max_value = Some(max_value);
		self
	}

	/// Set default value
	pub fn default(mut self, default: f64) -> Self {
		self.default = Some(default);
		self
	}

	/// Validate a float value
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::serializers::fields::FloatField;
	///
	/// let field = FloatField::new()
	///     .min_value(0.0)
	///     .max_value(1.0);
	///
	/// assert!(field.validate(0.5).is_ok());
	/// assert!(field.validate(-0.1).is_err());
	/// assert!(field.validate(1.1).is_err());
	/// ```
	pub fn validate(&self, value: f64) -> Result<(), FieldError> {
		if let Some(min) = self.min_value
			&& value < min
		{
			return Err(FieldError::TooSmallFloat(min));
		}

		if let Some(max) = self.max_value
			&& value > max
		{
			return Err(FieldError::TooLargeFloat(max));
		}

		Ok(())
	}
}

impl Default for FloatField {
	fn default() -> Self {
		Self::new()
	}
}

/// Boolean field
///
/// # Example
///
/// ```rust
/// use reinhardt_core::serializers::fields::BooleanField;
///
/// let field = BooleanField::new();
/// assert!(field.validate(true).is_ok());
/// assert!(field.validate(false).is_ok());
/// ```
#[derive(Debug, Clone)]
pub struct BooleanField {
	/// Whether this field is required.
	pub required: bool,
	/// Whether null values are allowed.
	pub allow_null: bool,
	/// Default value when none is provided.
	pub default: Option<bool>,
}

impl BooleanField {
	/// Create a new BooleanField with default settings
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::serializers::fields::BooleanField;
	///
	/// let field = BooleanField::new();
	/// assert!(field.validate(true).is_ok());
	/// ```
	pub fn new() -> Self {
		Self {
			required: true,
			allow_null: false,
			default: None,
		}
	}

	/// Set whether the field is required
	pub fn required(mut self, required: bool) -> Self {
		self.required = required;
		self
	}

	/// Set whether null values are allowed
	pub fn allow_null(mut self, allow_null: bool) -> Self {
		self.allow_null = allow_null;
		self
	}

	/// Set default value
	pub fn default(mut self, default: bool) -> Self {
		self.default = Some(default);
		self
	}

	/// Validate a boolean value
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::serializers::fields::BooleanField;
	///
	/// let field = BooleanField::new();
	/// assert!(field.validate(true).is_ok());
	/// assert!(field.validate(false).is_ok());
	/// ```
	pub fn validate(&self, _value: bool) -> Result<(), FieldError> {
		// Boolean values are always valid
		Ok(())
	}
}

impl Default for BooleanField {
	fn default() -> Self {
		Self::new()
	}
}

/// Email field with format validation
///
/// # Example
///
/// ```rust
/// use reinhardt_core::serializers::fields::EmailField;
///
/// let field = EmailField::new();
/// assert!(field.validate("user@example.com").is_ok());
/// assert!(field.validate("invalid-email").is_err());
/// ```
#[derive(Debug, Clone)]
pub struct EmailField {
	/// Whether this field is required.
	pub required: bool,
	/// Whether null values are allowed.
	pub allow_null: bool,
	/// Whether blank (empty) strings are allowed.
	pub allow_blank: bool,
	/// Default value when none is provided.
	pub default: Option<String>,
}

impl EmailField {
	/// Create a new EmailField with default settings
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::serializers::fields::EmailField;
	///
	/// let field = EmailField::new();
	/// assert!(field.validate("test@example.com").is_ok());
	/// ```
	pub fn new() -> Self {
		Self {
			required: true,
			allow_null: false,
			allow_blank: false,
			default: None,
		}
	}

	/// Set whether the field is required
	pub fn required(mut self, required: bool) -> Self {
		self.required = required;
		self
	}

	/// Set whether null values are allowed
	pub fn allow_null(mut self, allow_null: bool) -> Self {
		self.allow_null = allow_null;
		self
	}

	/// Set whether blank strings are allowed
	pub fn allow_blank(mut self, allow_blank: bool) -> Self {
		self.allow_blank = allow_blank;
		self
	}

	/// Set default value
	pub fn default(mut self, default: String) -> Self {
		self.default = Some(default);
		self
	}

	/// Validate an email address
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::serializers::fields::EmailField;
	///
	/// let field = EmailField::new();
	/// assert!(field.validate("user@example.com").is_ok());
	/// assert!(field.validate("invalid").is_err());
	/// ```
	pub fn validate(&self, value: &str) -> Result<(), FieldError> {
		if value.is_empty() {
			if !self.allow_blank {
				return Err(FieldError::Required);
			}
			return Ok(());
		}

		// Basic email validation: contains @ and has text before and after
		if !value.contains('@') {
			return Err(FieldError::InvalidEmail);
		}

		let parts: Vec<&str> = value.split('@').collect();
		if parts.len() != 2 {
			return Err(FieldError::InvalidEmail);
		}

		let local = parts[0];
		let domain = parts[1];

		if local.is_empty() || domain.is_empty() {
			return Err(FieldError::InvalidEmail);
		}

		if !domain.contains('.') {
			return Err(FieldError::InvalidEmail);
		}

		Ok(())
	}
}

impl Default for EmailField {
	fn default() -> Self {
		Self::new()
	}
}

/// URL field with format validation
///
/// # Example
///
/// ```rust
/// use reinhardt_core::serializers::fields::URLField;
///
/// let field = URLField::new();
/// assert!(field.validate("https://example.com").is_ok());
/// assert!(field.validate("not-a-url").is_err());
/// ```
#[derive(Debug, Clone)]
pub struct URLField {
	/// Whether this field is required.
	pub required: bool,
	/// Whether null values are allowed.
	pub allow_null: bool,
	/// Whether blank (empty) strings are allowed.
	pub allow_blank: bool,
	/// Default value when none is provided.
	pub default: Option<String>,
}

impl URLField {
	/// Create a new URLField with default settings
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::serializers::fields::URLField;
	///
	/// let field = URLField::new();
	/// assert!(field.validate("https://example.com").is_ok());
	/// ```
	pub fn new() -> Self {
		Self {
			required: true,
			allow_null: false,
			allow_blank: false,
			default: None,
		}
	}

	/// Set whether the field is required
	pub fn required(mut self, required: bool) -> Self {
		self.required = required;
		self
	}

	/// Set whether null values are allowed
	pub fn allow_null(mut self, allow_null: bool) -> Self {
		self.allow_null = allow_null;
		self
	}

	/// Set whether blank strings are allowed
	pub fn allow_blank(mut self, allow_blank: bool) -> Self {
		self.allow_blank = allow_blank;
		self
	}

	/// Set default value
	pub fn default(mut self, default: String) -> Self {
		self.default = Some(default);
		self
	}

	/// Validate a URL
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::serializers::fields::URLField;
	///
	/// let field = URLField::new();
	/// assert!(field.validate("https://example.com").is_ok());
	/// assert!(field.validate("http://localhost:8000").is_ok());
	/// assert!(field.validate("invalid").is_err());
	/// ```
	pub fn validate(&self, value: &str) -> Result<(), FieldError> {
		if value.is_empty() {
			if !self.allow_blank {
				return Err(FieldError::Required);
			}
			return Ok(());
		}

		// Basic URL validation: starts with http:// or https://
		if !value.starts_with("http://") && !value.starts_with("https://") {
			return Err(FieldError::InvalidUrl);
		}

		// Must have something after the protocol
		let without_protocol = value
			.strip_prefix("https://")
			.or_else(|| value.strip_prefix("http://"))
			.expect("URL must start with http:// or https://");

		if without_protocol.is_empty() {
			return Err(FieldError::InvalidUrl);
		}

		Ok(())
	}
}

impl Default for URLField {
	fn default() -> Self {
		Self::new()
	}
}

/// Choice field with enumerated values
///
/// # Example
///
/// ```rust
/// use reinhardt_core::serializers::fields::ChoiceField;
///
/// let field = ChoiceField::new(vec!["red".to_string(), "green".to_string(), "blue".to_string()]);
/// assert!(field.validate("red").is_ok());
/// assert!(field.validate("yellow").is_err());
/// ```
#[derive(Debug, Clone)]
pub struct ChoiceField {
	/// Whether this field is required.
	pub required: bool,
	/// Whether null values are allowed.
	pub allow_null: bool,
	/// Whether blank (empty) strings are allowed.
	pub allow_blank: bool,
	/// List of valid choice values.
	pub choices: Vec<String>,
	/// Default value when none is provided.
	pub default: Option<String>,
}

impl ChoiceField {
	/// Create a new ChoiceField with the given choices
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::serializers::fields::ChoiceField;
	///
	/// let field = ChoiceField::new(vec!["small".to_string(), "medium".to_string(), "large".to_string()]);
	/// assert!(field.validate("medium").is_ok());
	/// ```
	pub fn new(choices: Vec<String>) -> Self {
		Self {
			required: true,
			allow_null: false,
			allow_blank: false,
			choices,
			default: None,
		}
	}

	/// Set whether the field is required
	pub fn required(mut self, required: bool) -> Self {
		self.required = required;
		self
	}

	/// Set whether null values are allowed
	pub fn allow_null(mut self, allow_null: bool) -> Self {
		self.allow_null = allow_null;
		self
	}

	/// Set whether blank strings are allowed
	pub fn allow_blank(mut self, allow_blank: bool) -> Self {
		self.allow_blank = allow_blank;
		self
	}

	/// Set default value
	pub fn default(mut self, default: String) -> Self {
		self.default = Some(default);
		self
	}

	/// Validate a choice value
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::serializers::fields::ChoiceField;
	///
	/// let field = ChoiceField::new(vec!["red".to_string(), "green".to_string(), "blue".to_string()]);
	/// assert!(field.validate("red").is_ok());
	/// assert!(field.validate("yellow").is_err());
	/// ```
	pub fn validate(&self, value: &str) -> Result<(), FieldError> {
		if value.is_empty() {
			if !self.allow_blank {
				return Err(FieldError::Required);
			}
			return Ok(());
		}

		if !self.choices.iter().any(|c| c == value) {
			return Err(FieldError::InvalidChoice);
		}

		Ok(())
	}
}

/// Date field with chrono integration
///
/// Supports date validation and parsing using chrono's NaiveDate.
///
/// # Example
///
/// ```rust
/// use reinhardt_core::serializers::fields::DateField;
/// use chrono::{NaiveDate, Datelike};
///
/// let field = DateField::new();
/// let date = field.parse("2024-01-15").unwrap();
/// assert_eq!(date.year(), 2024);
/// assert_eq!(date.month(), 1);
/// assert_eq!(date.day(), 15);
/// ```
#[derive(Debug, Clone)]
pub struct DateField {
	/// Whether this field is required.
	pub required: bool,
	/// Whether null values are allowed.
	pub allow_null: bool,
	/// Date format string (strftime format, e.g., "%Y-%m-%d").
	pub format: String,
	/// Default value when none is provided.
	pub default: Option<NaiveDate>,
}

impl DateField {
	/// Create a new DateField with default settings (ISO 8601 format: YYYY-MM-DD)
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::serializers::fields::DateField;
	///
	/// let field = DateField::new();
	/// assert!(field.parse("2024-01-15").is_ok());
	/// ```
	pub fn new() -> Self {
		Self {
			required: true,
			allow_null: false,
			format: "%Y-%m-%d".to_string(),
			default: None,
		}
	}

	/// Set whether the field is required
	pub fn required(mut self, required: bool) -> Self {
		self.required = required;
		self
	}

	/// Set whether null values are allowed
	pub fn allow_null(mut self, allow_null: bool) -> Self {
		self.allow_null = allow_null;
		self
	}

	/// Set custom date format (strftime format)
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::serializers::fields::DateField;
	///
	/// let field = DateField::new().format("%d/%m/%Y");
	/// assert!(field.parse("15/01/2024").is_ok());
	/// ```
	pub fn format(mut self, format: &str) -> Self {
		self.format = format.to_string();
		self
	}

	/// Set default value
	pub fn default(mut self, default: NaiveDate) -> Self {
		self.default = Some(default);
		self
	}

	/// Parse a date string
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::serializers::fields::DateField;
	/// use chrono::Datelike;
	///
	/// let field = DateField::new();
	/// let date = field.parse("2024-01-15").unwrap();
	/// assert_eq!(date.year(), 2024);
	/// ```
	pub fn parse(&self, value: &str) -> Result<NaiveDate, FieldError> {
		if value.is_empty() {
			if !self.required
				&& let Some(default) = self.default
			{
				return Ok(default);
			}
			return Err(FieldError::Required);
		}

		NaiveDate::parse_from_str(value, &self.format).map_err(|_| FieldError::InvalidDate)
	}

	/// Validate a date string
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::serializers::fields::DateField;
	///
	/// let field = DateField::new();
	/// assert!(field.validate("2024-01-15").is_ok());
	/// assert!(field.validate("invalid-date").is_err());
	/// ```
	pub fn validate(&self, value: &str) -> Result<(), FieldError> {
		self.parse(value)?;
		Ok(())
	}
}

impl Default for DateField {
	fn default() -> Self {
		Self::new()
	}
}

/// DateTime field with chrono integration
///
/// Supports datetime validation and parsing using chrono's NaiveDateTime.
///
/// # Example
///
/// ```rust
/// use reinhardt_core::serializers::fields::DateTimeField;
/// use chrono::{NaiveDateTime, Datelike, Timelike};
///
/// let field = DateTimeField::new();
/// let dt = field.parse("2024-01-15 14:30:00").unwrap();
/// assert_eq!(dt.year(), 2024);
/// assert_eq!(dt.month(), 1);
/// assert_eq!(dt.day(), 15);
/// assert_eq!(dt.hour(), 14);
/// assert_eq!(dt.minute(), 30);
/// ```
#[derive(Debug, Clone)]
pub struct DateTimeField {
	/// Whether this field is required.
	pub required: bool,
	/// Whether null values are allowed.
	pub allow_null: bool,
	/// DateTime format string (strftime format, e.g., "%Y-%m-%d %H:%M:%S").
	pub format: String,
	/// Default value when none is provided.
	pub default: Option<NaiveDateTime>,
}

impl DateTimeField {
	/// Create a new DateTimeField with default settings (ISO 8601 format)
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::serializers::fields::DateTimeField;
	///
	/// let field = DateTimeField::new();
	/// assert!(field.parse("2024-01-15 14:30:00").is_ok());
	/// ```
	pub fn new() -> Self {
		Self {
			required: true,
			allow_null: false,
			format: "%Y-%m-%d %H:%M:%S".to_string(),
			default: None,
		}
	}

	/// Set whether the field is required
	pub fn required(mut self, required: bool) -> Self {
		self.required = required;
		self
	}

	/// Set whether null values are allowed
	pub fn allow_null(mut self, allow_null: bool) -> Self {
		self.allow_null = allow_null;
		self
	}

	/// Set custom datetime format (strftime format)
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::serializers::fields::DateTimeField;
	///
	/// let field = DateTimeField::new().format("%d/%m/%Y %H:%M");
	/// assert!(field.parse("15/01/2024 14:30").is_ok());
	/// ```
	pub fn format(mut self, format: &str) -> Self {
		self.format = format.to_string();
		self
	}

	/// Set default value
	pub fn default(mut self, default: NaiveDateTime) -> Self {
		self.default = Some(default);
		self
	}

	/// Parse a datetime string
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::serializers::fields::DateTimeField;
	/// use chrono::Timelike;
	///
	/// let field = DateTimeField::new();
	/// let dt = field.parse("2024-01-15 14:30:00").unwrap();
	/// assert_eq!(dt.hour(), 14);
	/// ```
	pub fn parse(&self, value: &str) -> Result<NaiveDateTime, FieldError> {
		if value.is_empty() {
			if !self.required
				&& let Some(default) = self.default
			{
				return Ok(default);
			}
			return Err(FieldError::Required);
		}

		NaiveDateTime::parse_from_str(value, &self.format).map_err(|_| FieldError::InvalidDateTime)
	}

	/// Validate a datetime string
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::serializers::fields::DateTimeField;
	///
	/// let field = DateTimeField::new();
	/// assert!(field.validate("2024-01-15 14:30:00").is_ok());
	/// assert!(field.validate("invalid-datetime").is_err());
	/// ```
	pub fn validate(&self, value: &str) -> Result<(), FieldError> {
		self.parse(value)?;
		Ok(())
	}
}

impl Default for DateTimeField {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use chrono::{Datelike, Timelike};
	use rstest::rstest;

	#[test]
	fn test_char_field_valid() {
		let field = CharField::new().min_length(3).max_length(10);
		assert!(field.validate("hello").is_ok());
	}

	#[test]
	fn test_char_field_too_short() {
		let field = CharField::new().min_length(5);
		assert_eq!(field.validate("hi"), Err(FieldError::TooShort(5)));
	}

	#[test]
	fn test_char_field_too_long() {
		let field = CharField::new().max_length(5);
		assert_eq!(field.validate("hello world"), Err(FieldError::TooLong(5)));
	}

	#[rstest]
	#[case::cjk_within_max("こんにちは世", 10, true)]
	#[case::cjk_exceeding_max("こんにちはこんにちはあ", 10, false)]
	#[case::emoji_within_max("🎉🎊🎈", 5, true)]
	#[case::emoji_exceeding_max("🎉🎊🎈🎁🎀🎆", 5, false)]
	#[case::mixed_ascii_cjk("hello世界", 10, true)]
	#[case::boundary_exact_max("あいうえお", 5, true)]
	#[case::boundary_one_over_max("あいうえおか", 5, false)]
	fn test_char_field_unicode_max_length(
		#[case] input: &str,
		#[case] max_length: usize,
		#[case] should_pass: bool,
	) {
		// Arrange
		let field = CharField::new().max_length(max_length);

		// Act
		let result = field.validate(input);

		// Assert
		assert_eq!(result.is_ok(), should_pass);
	}

	#[rstest]
	#[case::cjk_meets_min("こんにちは", 5, true)]
	#[case::cjk_below_min("こん", 5, false)]
	#[case::boundary_exact_min("あいう", 3, true)]
	fn test_char_field_unicode_min_length(
		#[case] input: &str,
		#[case] min_length: usize,
		#[case] should_pass: bool,
	) {
		// Arrange
		let field = CharField::new().min_length(min_length);

		// Act
		let result = field.validate(input);

		// Assert
		assert_eq!(result.is_ok(), should_pass);
	}

	#[test]
	fn test_integer_field_valid() {
		let field = IntegerField::new().min_value(0).max_value(100);
		assert!(field.validate(50).is_ok());
	}

	#[test]
	fn test_integer_field_too_small() {
		let field = IntegerField::new().min_value(0);
		assert_eq!(field.validate(-1), Err(FieldError::TooSmall(0)));
	}

	#[test]
	fn test_integer_field_too_large() {
		let field = IntegerField::new().max_value(100);
		assert_eq!(field.validate(101), Err(FieldError::TooLarge(100)));
	}

	#[test]
	fn test_float_field_valid() {
		let field = FloatField::new().min_value(0.0).max_value(1.0);
		assert!(field.validate(0.5).is_ok());
	}

	#[test]
	fn test_boolean_field() {
		let field = BooleanField::new();
		assert!(field.validate(true).is_ok());
		assert!(field.validate(false).is_ok());
	}

	#[test]
	fn test_email_field_valid() {
		let field = EmailField::new();
		assert!(field.validate("user@example.com").is_ok());
	}

	#[test]
	fn test_email_field_invalid() {
		let field = EmailField::new();
		assert_eq!(field.validate("invalid"), Err(FieldError::InvalidEmail));
		assert_eq!(
			field.validate("@example.com"),
			Err(FieldError::InvalidEmail)
		);
		assert_eq!(field.validate("user@"), Err(FieldError::InvalidEmail));
	}

	#[test]
	fn test_url_field_valid() {
		let field = URLField::new();
		assert!(field.validate("https://example.com").is_ok());
		assert!(field.validate("http://localhost:8000").is_ok());
	}

	#[test]
	fn test_url_field_invalid() {
		let field = URLField::new();
		assert_eq!(field.validate("invalid"), Err(FieldError::InvalidUrl));
		assert_eq!(
			field.validate("ftp://example.com"),
			Err(FieldError::InvalidUrl)
		);
	}

	#[test]
	fn test_choice_field_valid() {
		let field = ChoiceField::new(vec![
			"red".to_string(),
			"green".to_string(),
			"blue".to_string(),
		]);
		assert!(field.validate("red").is_ok());
		assert!(field.validate("green").is_ok());
	}

	#[test]
	fn test_choice_field_invalid() {
		let field = ChoiceField::new(vec!["red".to_string(), "green".to_string()]);
		assert_eq!(field.validate("blue"), Err(FieldError::InvalidChoice));
	}

	#[test]
	fn test_date_field_valid() {
		let field = DateField::new();
		let date = field.parse("2024-01-15").unwrap();
		assert_eq!(date.year(), 2024);
		assert_eq!(date.month(), 1);
		assert_eq!(date.day(), 15);
	}

	#[test]
	fn test_date_field_invalid() {
		let field = DateField::new();
		assert_eq!(field.validate("invalid-date"), Err(FieldError::InvalidDate));
	}

	#[test]
	fn test_date_field_custom_format() {
		let field = DateField::new().format("%d/%m/%Y");
		let date = field.parse("15/01/2024").unwrap();
		assert_eq!(date.year(), 2024);
		assert_eq!(date.month(), 1);
		assert_eq!(date.day(), 15);
	}

	#[test]
	fn test_datetime_field_valid() {
		let field = DateTimeField::new();
		let dt = field.parse("2024-01-15 14:30:00").unwrap();
		assert_eq!(dt.year(), 2024);
		assert_eq!(dt.month(), 1);
		assert_eq!(dt.day(), 15);
		assert_eq!(dt.hour(), 14);
		assert_eq!(dt.minute(), 30);
		assert_eq!(dt.second(), 0);
	}

	#[test]
	fn test_datetime_field_invalid() {
		let field = DateTimeField::new();
		assert_eq!(
			field.validate("invalid-datetime"),
			Err(FieldError::InvalidDateTime)
		);
	}

	#[test]
	fn test_datetime_field_custom_format() {
		let field = DateTimeField::new().format("%d/%m/%Y %H:%M");
		let dt = field.parse("15/01/2024 14:30").unwrap();
		assert_eq!(dt.year(), 2024);
		assert_eq!(dt.hour(), 14);
		assert_eq!(dt.minute(), 30);
	}
}

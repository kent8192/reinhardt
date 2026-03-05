//! # Internationalization (i18n) Support
//!
//! This module provides localization support for validation error messages.
//!
//! ## Features
//!
//! - Fluent-based message formatting
//! - Multiple language support (English, Japanese)
//! - Easy integration with existing validators
//! - Thread-safe message bundles
//!
//! ## Example
//!
//! ```rust
//! use reinhardt_core::validators::i18n::{ValidationMessages, LocalizedValidator};
//! use reinhardt_core::validators::string::MinLengthValidator;
//! use reinhardt_core::validators::Validator;
//!
//! // Create a message bundle for Japanese
//! let messages = ValidationMessages::new("ja").unwrap();
//!
//! // Create a localized validator
//! let validator = LocalizedValidator::new(MinLengthValidator::new(5), messages);
//!
//! // Validate with localized messages
//! let result = validator.validate("hi");
//! // Error message will be in Japanese
//! ```

use super::Validator;
use super::errors::{ValidationError, ValidationResult};
use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use std::collections::HashMap;
use std::sync::Arc;
use unic_langid::LanguageIdentifier;

/// Built-in English validation messages
const EN_MESSAGES: &str = include_str!("../resources/validation_en.ftl");

/// Built-in Japanese validation messages
const JA_MESSAGES: &str = include_str!("../resources/validation_ja.ftl");

/// Error type for i18n operations.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum I18nError {
	/// The requested language is not supported.
	UnsupportedLanguage(String),
	/// Failed to parse the language identifier.
	InvalidLanguageId(String),
	/// Failed to load the Fluent resource.
	ResourceLoadError(String),
	/// Failed to format the message.
	FormatError(String),
}

impl std::fmt::Display for I18nError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			I18nError::UnsupportedLanguage(lang) => write!(f, "Unsupported language: {}", lang),
			I18nError::InvalidLanguageId(id) => write!(f, "Invalid language identifier: {}", id),
			I18nError::ResourceLoadError(msg) => write!(f, "Failed to load resource: {}", msg),
			I18nError::FormatError(msg) => write!(f, "Message format error: {}", msg),
		}
	}
}

impl std::error::Error for I18nError {}

/// Supported languages for validation messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Language {
	/// English (default)
	#[default]
	English,
	/// Japanese
	Japanese,
}

impl Language {
	/// Creates a Language from a language code string.
	///
	/// # Arguments
	///
	/// * `code` - Language code (e.g., "en", "ja", "en-US", "ja-JP")
	///
	/// # Returns
	///
	/// Returns `Ok(Language)` if the code is recognized, `Err(I18nError)` otherwise.
	pub fn from_code(code: &str) -> Result<Self, I18nError> {
		let code_lower = code.to_lowercase();
		match code_lower.as_str() {
			"en" | "en-us" | "en-gb" | "english" => Ok(Language::English),
			"ja" | "ja-jp" | "japanese" => Ok(Language::Japanese),
			_ => Err(I18nError::UnsupportedLanguage(code.to_string())),
		}
	}

	/// Returns the language identifier string.
	pub fn code(&self) -> &'static str {
		match self {
			Language::English => "en",
			Language::Japanese => "ja",
		}
	}

	/// Returns all supported languages.
	pub fn all() -> &'static [Language] {
		&[Language::English, Language::Japanese]
	}
}

/// A container for localized validation messages.
///
/// `ValidationMessages` wraps a Fluent bundle and provides convenient
/// methods for formatting validation error messages.
#[derive(Clone)]
pub struct ValidationMessages {
	bundle: Arc<FluentBundle<FluentResource>>,
	language: Language,
}

impl std::fmt::Debug for ValidationMessages {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ValidationMessages")
			.field("language", &self.language)
			.finish()
	}
}

impl ValidationMessages {
	/// Creates a new ValidationMessages instance for the specified language.
	///
	/// # Arguments
	///
	/// * `language_code` - Language code (e.g., "en", "ja")
	///
	/// # Returns
	///
	/// Returns `Ok(ValidationMessages)` if successful, `Err(I18nError)` otherwise.
	pub fn new(language_code: &str) -> Result<Self, I18nError> {
		let language = Language::from_code(language_code)?;
		Self::for_language(language)
	}

	/// Creates a new ValidationMessages instance for the specified Language enum.
	pub fn for_language(language: Language) -> Result<Self, I18nError> {
		let ftl_content = match language {
			Language::English => EN_MESSAGES,
			Language::Japanese => JA_MESSAGES,
		};

		let resource = FluentResource::try_new(ftl_content.to_string())
			.map_err(|(_, errors)| I18nError::ResourceLoadError(format!("{:?}", errors)))?;

		let lang_id: LanguageIdentifier = language
			.code()
			.parse()
			.map_err(|e| I18nError::InvalidLanguageId(format!("{:?}", e)))?;

		let mut bundle = FluentBundle::new_concurrent(vec![lang_id]);
		bundle
			.add_resource(resource)
			.map_err(|errors| I18nError::ResourceLoadError(format!("{:?}", errors)))?;

		Ok(Self {
			bundle: Arc::new(bundle),
			language,
		})
	}

	/// Creates ValidationMessages with default language (English).
	pub fn default_language() -> Result<Self, I18nError> {
		Self::for_language(Language::English)
	}

	/// Returns the language of this message bundle.
	pub fn language(&self) -> Language {
		self.language
	}

	/// Formats a message with the given message ID and arguments.
	///
	/// # Arguments
	///
	/// * `message_id` - The Fluent message ID
	/// * `args` - Optional arguments for the message
	///
	/// # Returns
	///
	/// The formatted message string, or a fallback if the message is not found.
	pub fn format(&self, message_id: &str, args: Option<&HashMap<&str, FluentValue>>) -> String {
		let fluent_args = args.map(|a| {
			let mut fa = FluentArgs::new();
			for (k, v) in a {
				fa.set(*k, v.clone());
			}
			fa
		});

		if let Some(msg) = self.bundle.get_message(message_id)
			&& let Some(pattern) = msg.value()
		{
			let mut errors = vec![];
			let result = self
				.bundle
				.format_pattern(pattern, fluent_args.as_ref(), &mut errors);
			if errors.is_empty() {
				return result.into_owned();
			}
		}

		// Fallback to message_id if not found
		message_id.to_string()
	}

	/// Formats a message with a simple string argument.
	pub fn format_with_value(&self, message_id: &str, key: &str, value: &str) -> String {
		let mut args = HashMap::new();
		args.insert(key, FluentValue::from(value));
		self.format(message_id, Some(&args))
	}

	/// Formats a message with multiple string arguments.
	pub fn format_with_values(&self, message_id: &str, pairs: &[(&str, &str)]) -> String {
		let mut args = HashMap::new();
		for (k, v) in pairs {
			args.insert(*k, FluentValue::from(*v));
		}
		self.format(message_id, Some(&args))
	}

	/// Formats a message with numeric arguments (usize).
	pub fn format_with_numbers_usize(&self, message_id: &str, pairs: &[(&str, usize)]) -> String {
		let mut args = HashMap::new();
		for (k, v) in pairs {
			args.insert(*k, FluentValue::from(*v as i64));
		}
		self.format(message_id, Some(&args))
	}

	/// Formats a message with numeric arguments (u32).
	pub fn format_with_numbers_u32(&self, message_id: &str, pairs: &[(&str, u32)]) -> String {
		let mut args = HashMap::new();
		for (k, v) in pairs {
			args.insert(*k, FluentValue::from(*v as i64));
		}
		self.format(message_id, Some(&args))
	}

	/// Formats a message with numeric arguments (u64).
	pub fn format_with_numbers_u64(&self, message_id: &str, pairs: &[(&str, u64)]) -> String {
		let mut args = HashMap::new();
		for (k, v) in pairs {
			args.insert(*k, FluentValue::from(*v as i64));
		}
		self.format(message_id, Some(&args))
	}

	/// Localizes a ValidationError to the configured language.
	pub fn localize_error(&self, error: &ValidationError) -> String {
		match error {
			ValidationError::TooShort { length, min } => self.format_with_numbers_usize(
				"validation-too-short",
				&[("length", *length), ("min", *min)],
			),
			ValidationError::TooLong { length, max } => self.format_with_numbers_usize(
				"validation-too-long",
				&[("length", *length), ("max", *max)],
			),
			ValidationError::TooSmall { value, min } => {
				self.format_with_values("validation-too-small", &[("value", value), ("min", min)])
			}
			ValidationError::TooLarge { value, max } => {
				self.format_with_values("validation-too-large", &[("value", value), ("max", max)])
			}
			ValidationError::InvalidEmail(value) => {
				self.format_with_value("validation-invalid-email", "value", value)
			}
			ValidationError::InvalidUrl(value) => {
				self.format_with_value("validation-invalid-url", "value", value)
			}
			ValidationError::InvalidIPAddress(value) => {
				self.format_with_value("validation-invalid-ip", "value", value)
			}
			ValidationError::PatternMismatch(_) => self.format("validation-pattern-mismatch", None),
			ValidationError::InvalidSlug(value) => {
				self.format_with_value("validation-invalid-slug", "value", value)
			}
			ValidationError::InvalidUUID(value) => {
				self.format_with_value("validation-invalid-uuid", "value", value)
			}
			ValidationError::InvalidDate(value) => {
				self.format_with_value("validation-invalid-date", "value", value)
			}
			ValidationError::InvalidTime(value) => {
				self.format_with_value("validation-invalid-time", "value", value)
			}
			ValidationError::InvalidDateTime(value) => {
				self.format_with_value("validation-invalid-datetime", "value", value)
			}
			ValidationError::InvalidJSON(error) => {
				self.format_with_value("validation-invalid-json", "error", error)
			}
			ValidationError::InvalidCreditCard(_) => {
				self.format("validation-invalid-credit-card", None)
			}
			ValidationError::CardTypeNotAllowed {
				card_type,
				allowed_types,
			} => self.format_with_values(
				"validation-card-type-not-allowed",
				&[("card_type", card_type), ("allowed", allowed_types)],
			),
			ValidationError::InvalidPhoneNumber(value) => {
				self.format_with_value("validation-invalid-phone", "value", value)
			}
			ValidationError::CountryCodeNotAllowed {
				country_code,
				allowed_countries,
			} => self.format_with_values(
				"validation-country-not-allowed",
				&[("country", country_code), ("allowed", allowed_countries)],
			),
			ValidationError::InvalidIBAN(value) => {
				self.format_with_value("validation-invalid-iban", "value", value)
			}
			ValidationError::IBANCountryNotAllowed {
				country_code,
				allowed_codes,
			} => self.format_with_values(
				"validation-iban-country-not-allowed",
				&[("country", country_code), ("allowed", allowed_codes)],
			),
			ValidationError::InvalidFileExtension {
				extension,
				allowed_extensions,
			} => self.format_with_values(
				"validation-invalid-extension",
				&[("extension", extension), ("allowed", allowed_extensions)],
			),
			ValidationError::InvalidMimeType {
				mime_type,
				allowed_mime_types,
			} => self.format_with_values(
				"validation-invalid-mime-type",
				&[("mime_type", mime_type), ("allowed", allowed_mime_types)],
			),
			ValidationError::FileSizeTooSmall {
				size_bytes,
				min_bytes,
			} => self.format_with_numbers_u64(
				"validation-file-too-small",
				&[("size", *size_bytes), ("min", *min_bytes)],
			),
			ValidationError::FileSizeTooLarge {
				size_bytes,
				max_bytes,
			} => self.format_with_numbers_u64(
				"validation-file-too-large",
				&[("size", *size_bytes), ("max", *max_bytes)],
			),
			ValidationError::ImageWidthTooSmall { width, min_width } => self
				.format_with_numbers_u32(
					"validation-image-width-too-small",
					&[("width", *width), ("min", *min_width)],
				),
			ValidationError::ImageWidthTooLarge { width, max_width } => self
				.format_with_numbers_u32(
					"validation-image-width-too-large",
					&[("width", *width), ("max", *max_width)],
				),
			ValidationError::ImageHeightTooSmall { height, min_height } => self
				.format_with_numbers_u32(
					"validation-image-height-too-small",
					&[("height", *height), ("min", *min_height)],
				),
			ValidationError::ImageHeightTooLarge { height, max_height } => self
				.format_with_numbers_u32(
					"validation-image-height-too-large",
					&[("height", *height), ("max", *max_height)],
				),
			ValidationError::InvalidAspectRatio {
				actual_width,
				actual_height,
				expected_width,
				expected_height,
			} => self.format_with_numbers_u32(
				"validation-invalid-aspect-ratio",
				&[
					("actual_width", *actual_width),
					("actual_height", *actual_height),
					("expected_width", *expected_width),
					("expected_height", *expected_height),
				],
			),
			ValidationError::ImageReadError(error) => {
				self.format_with_value("validation-image-read-error", "error", error)
			}
			ValidationError::InvalidPostalCode { postal_code } => {
				self.format_with_value("validation-invalid-postal-code", "value", postal_code)
			}
			ValidationError::PostalCodeCountryNotRecognized { postal_code } => self
				.format_with_value(
					"validation-postal-country-not-recognized",
					"value",
					postal_code,
				),
			ValidationError::PostalCodeCountryNotAllowed {
				country,
				allowed_countries,
			} => self.format_with_values(
				"validation-postal-country-not-allowed",
				&[("country", country), ("allowed", allowed_countries)],
			),
			ValidationError::NotUnique { field, value } => self.format_with_values(
				"validation-not-unique",
				&[("field", field), ("value", value)],
			),
			ValidationError::ForeignKeyNotFound {
				field,
				value,
				table,
			} => self.format_with_values(
				"validation-fk-not-found",
				&[("field", field), ("value", value), ("table", table)],
			),
			ValidationError::AllValidatorsFailed { errors } => {
				self.format_with_value("validation-all-failed", "errors", errors)
			}
			ValidationError::CompositeValidationFailed(error) => {
				self.format_with_value("validation-composite-failed", "error", error)
			}
			ValidationError::Custom(message) => {
				self.format_with_value("validation-custom", "message", message)
			}
		}
	}
}

/// A wrapper that provides localized error messages for any validator.
///
/// `LocalizedValidator` wraps an existing validator and localizes
/// the error messages to the specified language.
#[derive(Debug, Clone)]
pub struct LocalizedValidator<V> {
	inner: V,
	messages: ValidationMessages,
}

impl<V> LocalizedValidator<V> {
	/// Creates a new localized validator.
	///
	/// # Arguments
	///
	/// * `validator` - The inner validator
	/// * `messages` - The localized messages bundle
	pub fn new(validator: V, messages: ValidationMessages) -> Self {
		Self {
			inner: validator,
			messages,
		}
	}

	/// Creates a new localized validator with the specified language.
	///
	/// # Arguments
	///
	/// * `validator` - The inner validator
	/// * `language_code` - The language code (e.g., "en", "ja")
	pub fn with_language(validator: V, language_code: &str) -> Result<Self, I18nError> {
		let messages = ValidationMessages::new(language_code)?;
		Ok(Self::new(validator, messages))
	}

	/// Returns a reference to the inner validator.
	pub fn inner(&self) -> &V {
		&self.inner
	}

	/// Returns the localized messages bundle.
	pub fn messages(&self) -> &ValidationMessages {
		&self.messages
	}
}

impl<T, V> Validator<T> for LocalizedValidator<V>
where
	T: ?Sized,
	V: Validator<T>,
{
	fn validate(&self, value: &T) -> ValidationResult<()> {
		self.inner.validate(value).map_err(|e| {
			let localized_message = self.messages.localize_error(&e);
			ValidationError::Custom(localized_message)
		})
	}
}

/// A builder for creating LocalizedValidators with custom language settings.
#[derive(Debug, Clone)]
pub struct LocalizedValidatorBuilder {
	language: Language,
	messages: Option<ValidationMessages>,
}

impl Default for LocalizedValidatorBuilder {
	fn default() -> Self {
		Self::new()
	}
}

impl LocalizedValidatorBuilder {
	/// Creates a new builder with English as the default language.
	pub fn new() -> Self {
		Self {
			language: Language::English,
			messages: None,
		}
	}

	/// Sets the language for the validator.
	pub fn language(mut self, language: Language) -> Self {
		self.language = language;
		self
	}

	/// Sets the language using a language code.
	pub fn language_code(mut self, code: &str) -> Result<Self, I18nError> {
		self.language = Language::from_code(code)?;
		Ok(self)
	}

	/// Uses a custom ValidationMessages instance.
	pub fn messages(mut self, messages: ValidationMessages) -> Self {
		self.messages = Some(messages);
		self
	}

	/// Builds a LocalizedValidator for the given validator.
	pub fn build<V>(self, validator: V) -> Result<LocalizedValidator<V>, I18nError> {
		let messages = match self.messages {
			Some(m) => m,
			None => ValidationMessages::for_language(self.language)?,
		};
		Ok(LocalizedValidator::new(validator, messages))
	}
}

/// Convenience function to create a localized validator with English messages.
pub fn localize_en<V>(validator: V) -> Result<LocalizedValidator<V>, I18nError> {
	LocalizedValidator::with_language(validator, "en")
}

/// Convenience function to create a localized validator with Japanese messages.
pub fn localize_ja<V>(validator: V) -> Result<LocalizedValidator<V>, I18nError> {
	LocalizedValidator::with_language(validator, "ja")
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::validators::string::MinLengthValidator;

	#[test]
	fn test_language_from_code() {
		assert_eq!(Language::from_code("en").unwrap(), Language::English);
		assert_eq!(Language::from_code("EN").unwrap(), Language::English);
		assert_eq!(Language::from_code("en-US").unwrap(), Language::English);
		assert_eq!(Language::from_code("ja").unwrap(), Language::Japanese);
		assert_eq!(Language::from_code("ja-JP").unwrap(), Language::Japanese);
		assert!(Language::from_code("fr").is_err());
	}

	#[test]
	fn test_language_code() {
		assert_eq!(Language::English.code(), "en");
		assert_eq!(Language::Japanese.code(), "ja");
	}

	#[test]
	fn test_language_all() {
		let all = Language::all();
		assert_eq!(all.len(), 2);
		assert!(all.contains(&Language::English));
		assert!(all.contains(&Language::Japanese));
	}

	#[test]
	fn test_validation_messages_new() {
		let messages = ValidationMessages::new("en").unwrap();
		assert_eq!(messages.language(), Language::English);

		let messages = ValidationMessages::new("ja").unwrap();
		assert_eq!(messages.language(), Language::Japanese);
	}

	#[test]
	fn test_validation_messages_format() {
		let messages = ValidationMessages::new("en").unwrap();
		let result = messages
			.format_with_numbers_usize("validation-too-short", &[("length", 2), ("min", 5)]);
		assert!(result.contains("2"));
		assert!(result.contains("5"));
	}

	#[test]
	fn test_validation_messages_format_ja() {
		let messages = ValidationMessages::new("ja").unwrap();
		let result = messages
			.format_with_numbers_usize("validation-too-short", &[("length", 2), ("min", 5)]);
		assert!(result.contains("2"));
		assert!(result.contains("5"));
		assert!(result.contains("文字")); // Japanese for "characters"
	}

	#[test]
	fn test_localize_error() {
		let messages = ValidationMessages::new("en").unwrap();

		let error = ValidationError::TooShort { length: 2, min: 5 };
		let localized = messages.localize_error(&error);
		assert!(localized.contains("2"));
		assert!(localized.contains("5"));
	}

	#[test]
	fn test_localize_error_ja() {
		let messages = ValidationMessages::new("ja").unwrap();

		let error = ValidationError::TooShort { length: 2, min: 5 };
		let localized = messages.localize_error(&error);
		assert!(localized.contains("2"));
		assert!(localized.contains("5"));
		assert!(localized.contains("短すぎ")); // Japanese for "too short"
	}

	#[test]
	fn test_localized_validator() {
		let messages = ValidationMessages::new("ja").unwrap();
		let validator = LocalizedValidator::new(MinLengthValidator::new(5), messages);

		let result = validator.validate("hi");
		assert!(result.is_err());

		if let Err(ValidationError::Custom(msg)) = result {
			assert!(msg.contains("短すぎ")); // Japanese for "too short"
		} else {
			panic!("Expected Custom error");
		}
	}

	#[test]
	fn test_localized_validator_valid() {
		let messages = ValidationMessages::new("en").unwrap();
		let validator = LocalizedValidator::new(MinLengthValidator::new(2), messages);

		let result = validator.validate("hello");
		assert!(result.is_ok());
	}

	#[test]
	fn test_localized_validator_with_language() {
		let validator =
			LocalizedValidator::with_language(MinLengthValidator::new(5), "ja").unwrap();

		let result = validator.validate("hi");
		assert!(result.is_err());
	}

	#[test]
	fn test_localized_validator_builder() {
		let validator = LocalizedValidatorBuilder::new()
			.language(Language::Japanese)
			.build(MinLengthValidator::new(5))
			.unwrap();

		let result = validator.validate("hi");
		assert!(result.is_err());
	}

	#[test]
	fn test_localized_validator_builder_with_code() {
		let validator = LocalizedValidatorBuilder::new()
			.language_code("ja")
			.unwrap()
			.build(MinLengthValidator::new(5))
			.unwrap();

		let result = validator.validate("hi");
		assert!(result.is_err());
	}

	#[test]
	fn test_convenience_functions() {
		let validator_en = localize_en(MinLengthValidator::new(5)).unwrap();
		let validator_ja = localize_ja(MinLengthValidator::new(5)).unwrap();

		assert_eq!(validator_en.messages().language(), Language::English);
		assert_eq!(validator_ja.messages().language(), Language::Japanese);
	}

	#[test]
	fn test_i18n_error_display() {
		let error = I18nError::UnsupportedLanguage("fr".to_string());
		assert!(error.to_string().contains("fr"));

		let error = I18nError::InvalidLanguageId("invalid".to_string());
		assert!(error.to_string().contains("invalid"));
	}

	#[test]
	fn test_localize_various_errors() {
		let messages = ValidationMessages::new("en").unwrap();

		// Test email error
		let error = ValidationError::InvalidEmail("test".to_string());
		let localized = messages.localize_error(&error);
		assert!(localized.contains("email") || localized.contains("test"));

		// Test URL error
		let error = ValidationError::InvalidUrl("not-a-url".to_string());
		let localized = messages.localize_error(&error);
		assert!(localized.contains("URL") || localized.contains("not-a-url"));

		// Test custom error
		let error = ValidationError::Custom("custom message".to_string());
		let localized = messages.localize_error(&error);
		assert!(localized.contains("custom message"));
	}

	#[test]
	fn test_fallback_on_missing_message() {
		let messages = ValidationMessages::new("en").unwrap();
		let result = messages.format("nonexistent-message-id", None);
		// Should return the message ID as fallback
		assert_eq!(result, "nonexistent-message-id");
	}
}

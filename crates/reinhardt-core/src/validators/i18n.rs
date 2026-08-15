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
			return match errors.is_empty() {
				true => result.into_owned(),
				false => message_id.to_string(),
			};
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

		let error = validator.validate("hi").unwrap_err();
		assert!(matches!(error, ValidationError::Custom(msg) if msg.contains("短すぎ")));
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
	fn test_localized_validator_builder_with_custom_messages_and_inner() {
		let messages = ValidationMessages::new("ja").unwrap();
		let validator = LocalizedValidatorBuilder::default()
			.messages(messages)
			.build(MinLengthValidator::new(5))
			.unwrap();

		assert_eq!(validator.messages().language(), Language::Japanese);
		assert_eq!(
			validator.inner().validate("hi"),
			Err(ValidationError::TooShort { length: 2, min: 5 })
		);
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
		assert_eq!(error.to_string(), "Unsupported language: fr");

		let error = I18nError::InvalidLanguageId("invalid".to_string());
		assert_eq!(error.to_string(), "Invalid language identifier: invalid");

		let error = I18nError::ResourceLoadError("parse failure".to_string());
		assert_eq!(error.to_string(), "Failed to load resource: parse failure");

		let error = I18nError::FormatError("missing value".to_string());
		assert_eq!(error.to_string(), "Message format error: missing value");
	}

	#[test]
	fn test_validation_messages_debug_and_default_language() {
		let messages = ValidationMessages::default_language().unwrap();
		assert_eq!(messages.language(), Language::English);
		assert_eq!(
			format!("{messages:?}"),
			"ValidationMessages { language: English }"
		);
	}

	#[test]
	fn test_format_helpers_render_exact_english_messages() {
		let messages = ValidationMessages::new("en").unwrap();

		assert_eq!(
			messages.format_with_value("validation-invalid-email", "value", "bad@example"),
			"Invalid email address: \u{2068}bad@example\u{2069}"
		);
		assert_eq!(
			messages.format_with_values(
				"validation-card-type-not-allowed",
				&[("card_type", "Diners"), ("allowed", "Visa, Mastercard")]
			),
			"Card type \u{2068}Diners\u{2069} is not allowed (allowed: \u{2068}Visa, Mastercard\u{2069})"
		);
		assert_eq!(
			messages
				.format_with_numbers_usize("validation-too-short", &[("length", 2), ("min", 5)]),
			"Value is too short: \u{2068}2\u{2069} characters (minimum: \u{2068}5\u{2069})"
		);
		assert_eq!(
			messages.format_with_numbers_u32(
				"validation-image-width-too-small",
				&[("width", 320), ("min", 640)]
			),
			"Image width is too small: \u{2068}320\u{2069}px (minimum: \u{2068}640\u{2069}px)"
		);
		assert_eq!(
			messages.format_with_numbers_u64(
				"validation-file-too-large",
				&[("size", 8192), ("max", 4096)]
			),
			"File is too large: \u{2068}8192\u{2069} bytes (maximum: \u{2068}4096\u{2069} bytes)"
		);
	}

	#[test]
	fn test_localize_all_validation_errors_in_english() {
		let messages = ValidationMessages::new("en").unwrap();
		let cases = [
			(
				ValidationError::TooShort { length: 2, min: 5 },
				"Value is too short: \u{2068}2\u{2069} characters (minimum: \u{2068}5\u{2069})",
			),
			(
				ValidationError::TooLong { length: 8, max: 5 },
				"Value is too long: \u{2068}8\u{2069} characters (maximum: \u{2068}5\u{2069})",
			),
			(
				ValidationError::TooSmall {
					value: "3".to_string(),
					min: "4".to_string(),
				},
				"Value is too small: \u{2068}3\u{2069} (minimum: \u{2068}4\u{2069})",
			),
			(
				ValidationError::TooLarge {
					value: "9".to_string(),
					max: "8".to_string(),
				},
				"Value is too large: \u{2068}9\u{2069} (maximum: \u{2068}8\u{2069})",
			),
			(
				ValidationError::InvalidEmail("bad@example".to_string()),
				"Invalid email address: \u{2068}bad@example\u{2069}",
			),
			(
				ValidationError::InvalidUrl("not-a-url".to_string()),
				"Invalid URL: \u{2068}not-a-url\u{2069}",
			),
			(
				ValidationError::InvalidIPAddress("127.0.0.1".to_string()),
				"Invalid IP address: \u{2068}127.0.0.1\u{2069}",
			),
			(
				ValidationError::PatternMismatch("expected".to_string()),
				"Value does not match the required pattern",
			),
			(
				ValidationError::InvalidSlug("bad slug".to_string()),
				"Invalid slug format: \u{2068}bad slug\u{2069}",
			),
			(
				ValidationError::InvalidUUID("bad uuid".to_string()),
				"Invalid UUID format: \u{2068}bad uuid\u{2069}",
			),
			(
				ValidationError::InvalidDate("tomorrow".to_string()),
				"Invalid date format: \u{2068}tomorrow\u{2069}",
			),
			(
				ValidationError::InvalidTime("noon".to_string()),
				"Invalid time format: \u{2068}noon\u{2069}",
			),
			(
				ValidationError::InvalidDateTime("now".to_string()),
				"Invalid datetime format: \u{2068}now\u{2069}",
			),
			(
				ValidationError::InvalidJSON("malformed".to_string()),
				"Invalid JSON: \u{2068}malformed\u{2069}",
			),
			(
				ValidationError::InvalidCreditCard("4111".to_string()),
				"Invalid credit card number",
			),
			(
				ValidationError::CardTypeNotAllowed {
					card_type: "Diners".to_string(),
					allowed_types: "Visa, Mastercard".to_string(),
				},
				"Card type \u{2068}Diners\u{2069} is not allowed (allowed: \u{2068}Visa, Mastercard\u{2069})",
			),
			(
				ValidationError::InvalidPhoneNumber("555".to_string()),
				"Invalid phone number: \u{2068}555\u{2069}",
			),
			(
				ValidationError::CountryCodeNotAllowed {
					country_code: "US".to_string(),
					allowed_countries: "JP".to_string(),
				},
				"Country code \u{2068}US\u{2069} is not allowed (allowed: \u{2068}JP\u{2069})",
			),
			(
				ValidationError::InvalidIBAN("bad-iban".to_string()),
				"Invalid IBAN: \u{2068}bad-iban\u{2069}",
			),
			(
				ValidationError::IBANCountryNotAllowed {
					country_code: "GB".to_string(),
					allowed_codes: "DE,FR".to_string(),
				},
				"IBAN country \u{2068}GB\u{2069} is not allowed (allowed: \u{2068}DE,FR\u{2069})",
			),
			(
				ValidationError::InvalidFileExtension {
					extension: ".exe".to_string(),
					allowed_extensions: ".txt,.csv".to_string(),
				},
				"File extension \"\u{2068}.exe\u{2069}\" is not allowed (allowed: \u{2068}.txt,.csv\u{2069})",
			),
			(
				ValidationError::InvalidMimeType {
					mime_type: "application/x-msdownload".to_string(),
					allowed_mime_types: "text/plain".to_string(),
				},
				"MIME type \"\u{2068}application/x-msdownload\u{2069}\" is not allowed (allowed: \u{2068}text/plain\u{2069})",
			),
			(
				ValidationError::FileSizeTooSmall {
					size_bytes: 128,
					min_bytes: 512,
				},
				"File is too small: \u{2068}128\u{2069} bytes (minimum: \u{2068}512\u{2069} bytes)",
			),
			(
				ValidationError::FileSizeTooLarge {
					size_bytes: 8192,
					max_bytes: 4096,
				},
				"File is too large: \u{2068}8192\u{2069} bytes (maximum: \u{2068}4096\u{2069} bytes)",
			),
			(
				ValidationError::ImageWidthTooSmall {
					width: 320,
					min_width: 640,
				},
				"Image width is too small: \u{2068}320\u{2069}px (minimum: \u{2068}640\u{2069}px)",
			),
			(
				ValidationError::ImageWidthTooLarge {
					width: 1920,
					max_width: 1280,
				},
				"Image width is too large: \u{2068}1920\u{2069}px (maximum: \u{2068}1280\u{2069}px)",
			),
			(
				ValidationError::ImageHeightTooSmall {
					height: 240,
					min_height: 480,
				},
				"Image height is too small: \u{2068}240\u{2069}px (minimum: \u{2068}480\u{2069}px)",
			),
			(
				ValidationError::ImageHeightTooLarge {
					height: 2160,
					max_height: 1080,
				},
				"Image height is too large: \u{2068}2160\u{2069}px (maximum: \u{2068}1080\u{2069}px)",
			),
			(
				ValidationError::InvalidAspectRatio {
					actual_width: 16,
					actual_height: 9,
					expected_width: 4,
					expected_height: 3,
				},
				"Invalid aspect ratio: \u{2068}16\u{2069}:\u{2068}9\u{2069} (expected: \u{2068}4\u{2069}:\u{2068}3\u{2069})",
			),
			(
				ValidationError::ImageReadError("truncated".to_string()),
				"Cannot read image: \u{2068}truncated\u{2069}",
			),
			(
				ValidationError::InvalidPostalCode {
					postal_code: "00000".to_string(),
				},
				"Invalid postal code: \u{2068}00000\u{2069}",
			),
			(
				ValidationError::PostalCodeCountryNotRecognized {
					postal_code: "00000".to_string(),
				},
				"Postal code country not recognized: \u{2068}00000\u{2069}",
			),
			(
				ValidationError::PostalCodeCountryNotAllowed {
					country: "CA".to_string(),
					allowed_countries: "US".to_string(),
				},
				"Country \u{2068}CA\u{2069} is not allowed (allowed: \u{2068}US\u{2069})",
			),
			(
				ValidationError::NotUnique {
					field: "username".to_string(),
					value: "alice".to_string(),
				},
				"Value must be unique. \"\u{2068}alice\u{2069}\" already exists in field \"\u{2068}username\u{2069}\"",
			),
			(
				ValidationError::ForeignKeyNotFound {
					field: "owner_id".to_string(),
					value: "42".to_string(),
					table: "users".to_string(),
				},
				"Reference not found: \u{2068}owner_id\u{2069} with value \u{2068}42\u{2069} does not exist in \u{2068}users\u{2069}",
			),
			(
				ValidationError::AllValidatorsFailed {
					errors: "email and username".to_string(),
				},
				"All validators failed: \u{2068}email and username\u{2069}",
			),
			(
				ValidationError::CompositeValidationFailed("nested invalid".to_string()),
				"Validation failed: \u{2068}nested invalid\u{2069}",
			),
			(
				ValidationError::Custom("custom text".to_string()),
				"custom text",
			),
		];

		for (error, expected) in cases {
			assert_eq!(messages.localize_error(&error), expected);
		}
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

		let missing_argument = messages.format("validation-too-short", None);
		assert_eq!(missing_argument, "validation-too-short");
	}
}

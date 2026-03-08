//! Validation error types

use thiserror::Error;

/// Validation errors produced by validators.
#[non_exhaustive]
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
	/// Invalid email address format.
	#[error("Invalid email: {0}")]
	InvalidEmail(String),

	/// Invalid URL format.
	#[error("Invalid URL: {0}")]
	InvalidUrl(String),

	/// Numeric value is below the minimum.
	#[error("Value too small: {value} (minimum: {min})")]
	TooSmall {
		/// The value that was too small.
		value: String,
		/// The minimum allowed value.
		min: String,
	},

	/// Numeric value exceeds the maximum.
	#[error("Value too large: {value} (maximum: {max})")]
	TooLarge {
		/// The value that was too large.
		value: String,
		/// The maximum allowed value.
		max: String,
	},

	/// String length is below the minimum.
	#[error("Length too short: {length} (minimum: {min})")]
	TooShort {
		/// The actual length.
		length: usize,
		/// The minimum required length.
		min: usize,
	},

	/// String length exceeds the maximum.
	#[error("Length too long: {length} (maximum: {max})")]
	TooLong {
		/// The actual length.
		length: usize,
		/// The maximum allowed length.
		max: usize,
	},

	/// Value does not match the expected pattern.
	#[error("Pattern mismatch: {0}")]
	PatternMismatch(String),

	/// Unique constraint violation.
	#[error("Field '{field}' must be unique. Value '{value}' already exists")]
	NotUnique {
		/// Name of the field with the unique constraint.
		field: String,
		/// The duplicate value.
		value: String,
	},

	/// Invalid slug format.
	#[error("Invalid slug: {0}")]
	InvalidSlug(String),

	/// Invalid UUID format.
	#[error("Invalid UUID: {0}")]
	InvalidUUID(String),

	/// Invalid IP address format.
	#[error("Invalid IP address: {0}")]
	InvalidIPAddress(String),

	/// Invalid date format.
	#[error("Invalid date: {0}")]
	InvalidDate(String),

	/// Invalid time format.
	#[error("Invalid time: {0}")]
	InvalidTime(String),

	/// Invalid datetime format.
	#[error("Invalid datetime: {0}")]
	InvalidDateTime(String),

	/// Invalid JSON format.
	#[error("Invalid JSON: {0}")]
	InvalidJSON(String),

	/// Invalid credit card number.
	#[error("Invalid credit card number: {0}")]
	InvalidCreditCard(String),

	/// Credit card type is not in the allowed list.
	#[error("Credit card type not allowed: {card_type} (allowed: {allowed_types})")]
	CardTypeNotAllowed {
		/// The detected card type.
		card_type: String,
		/// Comma-separated list of allowed card types.
		allowed_types: String,
	},

	/// Invalid phone number format.
	#[error("Invalid phone number: {0}")]
	InvalidPhoneNumber(String),

	/// Country code is not in the allowed list.
	#[error("Country code not allowed: {country_code} (allowed: {allowed_countries})")]
	CountryCodeNotAllowed {
		/// The disallowed country code.
		country_code: String,
		/// Comma-separated list of allowed countries.
		allowed_countries: String,
	},

	/// Invalid IBAN format.
	#[error("Invalid IBAN: {0}")]
	InvalidIBAN(String),

	/// IBAN country code is not in the allowed list.
	#[error("IBAN country code not allowed: {country_code} (allowed: {allowed_codes})")]
	IBANCountryNotAllowed {
		/// The disallowed IBAN country code.
		country_code: String,
		/// Comma-separated list of allowed IBAN country codes.
		allowed_codes: String,
	},

	/// File extension is not in the allowed list.
	#[error("Invalid file extension: {extension} (allowed: {allowed_extensions})")]
	InvalidFileExtension {
		/// The disallowed file extension.
		extension: String,
		/// Comma-separated list of allowed extensions.
		allowed_extensions: String,
	},

	/// MIME type is not in the allowed list.
	#[error("Invalid MIME type: {mime_type} (allowed: {allowed_mime_types})")]
	InvalidMimeType {
		/// The disallowed MIME type.
		mime_type: String,
		/// Comma-separated list of allowed MIME types.
		allowed_mime_types: String,
	},

	/// Foreign key reference does not exist.
	#[error(
		"Foreign key reference not found: {field} with value {value} does not exist in {table}"
	)]
	ForeignKeyNotFound {
		/// Name of the foreign key field.
		field: String,
		/// The value that was not found.
		value: String,
		/// Name of the referenced table.
		table: String,
	},

	/// File size is below the minimum.
	#[error("File size too small: {size_bytes} bytes (minimum: {min_bytes} bytes)")]
	FileSizeTooSmall {
		/// Actual file size in bytes.
		size_bytes: u64,
		/// Minimum allowed size in bytes.
		min_bytes: u64,
	},

	/// File size exceeds the maximum.
	#[error("File size too large: {size_bytes} bytes (maximum: {max_bytes} bytes)")]
	FileSizeTooLarge {
		/// Actual file size in bytes.
		size_bytes: u64,
		/// Maximum allowed size in bytes.
		max_bytes: u64,
	},

	/// All validators in an OR composition failed.
	#[error("All validators failed: {errors}")]
	AllValidatorsFailed {
		/// Combined error messages from all failed validators.
		errors: String,
	},

	/// Composite validation failure.
	#[error("Validation failed: {0}")]
	CompositeValidationFailed(String),

	/// Invalid postal code format.
	#[error("Invalid postal code: {postal_code}")]
	InvalidPostalCode {
		/// The invalid postal code.
		postal_code: String,
	},

	/// Postal code country could not be determined.
	#[error("Postal code country not recognized: {postal_code}")]
	PostalCodeCountryNotRecognized {
		/// The postal code whose country was not recognized.
		postal_code: String,
	},

	/// Postal code country is not in the allowed list.
	#[error("Country not allowed: {country} (allowed: {allowed_countries})")]
	PostalCodeCountryNotAllowed {
		/// The disallowed country.
		country: String,
		/// Comma-separated list of allowed countries.
		allowed_countries: String,
	},

	/// Image width is below the minimum.
	#[error("Image width too small: {width}px (minimum: {min_width}px)")]
	ImageWidthTooSmall {
		/// Actual image width in pixels.
		width: u32,
		/// Minimum required width in pixels.
		min_width: u32,
	},

	/// Image width exceeds the maximum.
	#[error("Image width too large: {width}px (maximum: {max_width}px)")]
	ImageWidthTooLarge {
		/// Actual image width in pixels.
		width: u32,
		/// Maximum allowed width in pixels.
		max_width: u32,
	},

	/// Image height is below the minimum.
	#[error("Image height too small: {height}px (minimum: {min_height}px)")]
	ImageHeightTooSmall {
		/// Actual image height in pixels.
		height: u32,
		/// Minimum required height in pixels.
		min_height: u32,
	},

	/// Image height exceeds the maximum.
	#[error("Image height too large: {height}px (maximum: {max_height}px)")]
	ImageHeightTooLarge {
		/// Actual image height in pixels.
		height: u32,
		/// Maximum allowed height in pixels.
		max_height: u32,
	},

	/// Image aspect ratio does not match the expected ratio.
	#[error(
		"Invalid aspect ratio: {actual_width}:{actual_height} (expected: {expected_width}:{expected_height})"
	)]
	InvalidAspectRatio {
		/// Actual image width.
		actual_width: u32,
		/// Actual image height.
		actual_height: u32,
		/// Expected width for the aspect ratio.
		expected_width: u32,
		/// Expected height for the aspect ratio.
		expected_height: u32,
	},

	/// Failed to read image data.
	#[error("Cannot read image: {0}")]
	ImageReadError(String),

	/// Custom validation error with user-defined message.
	#[error("Custom validation error: {0}")]
	Custom(String),
}

/// Result type for validation operations.
pub type ValidationResult<T> = Result<T, ValidationError>;

#[cfg(test)]
mod tests {
	use super::*;

	// Tests based on Django validators/tests.py - test_single_message, test_message_list, test_message_dict
	#[test]
	fn test_validation_error_display() {
		let error = ValidationError::Custom("Not Valid".to_string());
		assert_eq!(error.to_string(), "Custom validation error: Not Valid");
	}

	#[test]
	fn test_invalid_email_error() {
		let error = ValidationError::InvalidEmail("test@".to_string());
		assert_eq!(error.to_string(), "Invalid email: test@");
	}

	#[test]
	fn test_invalid_url_error() {
		let error = ValidationError::InvalidUrl("invalid-url".to_string());
		assert_eq!(error.to_string(), "Invalid URL: invalid-url");
	}

	#[test]
	fn test_too_small_error() {
		let error = ValidationError::TooSmall {
			value: "5".to_string(),
			min: "10".to_string(),
		};
		assert_eq!(error.to_string(), "Value too small: 5 (minimum: 10)");
	}

	#[test]
	fn test_too_large_error() {
		let error = ValidationError::TooLarge {
			value: "100".to_string(),
			max: "50".to_string(),
		};
		assert_eq!(error.to_string(), "Value too large: 100 (maximum: 50)");
	}

	#[test]
	fn test_too_short_error() {
		let error = ValidationError::TooShort { length: 3, min: 5 };
		assert_eq!(error.to_string(), "Length too short: 3 (minimum: 5)");
	}

	#[test]
	fn test_too_long_error() {
		let error = ValidationError::TooLong {
			length: 20,
			max: 10,
		};
		assert_eq!(error.to_string(), "Length too long: 20 (maximum: 10)");
	}

	#[test]
	fn test_pattern_mismatch_error() {
		let error = ValidationError::PatternMismatch("Value must be numeric".to_string());
		assert_eq!(error.to_string(), "Pattern mismatch: Value must be numeric");
	}

	#[test]
	fn test_error_debug_format() {
		let error = ValidationError::Custom("Test error".to_string());
		let debug_str = format!("{:?}", error);
		assert!(debug_str.contains("Custom"));
		assert!(debug_str.contains("Test error"));
	}

	#[test]
	fn test_error_clone() {
		let error = ValidationError::InvalidEmail("test@invalid".to_string());
		let cloned = error.clone();
		assert_eq!(error.to_string(), cloned.to_string());
	}

	#[test]
	fn test_validation_result_ok() {
		let result: ValidationResult<i32> = Ok(42);
		assert!(result.is_ok());
		assert_eq!(result, Ok(42));
	}

	#[test]
	fn test_validation_result_err() {
		let result: ValidationResult<i32> = Err(ValidationError::Custom("error".to_string()));
		assert!(result.is_err());
	}

	#[test]
	fn test_not_unique_error() {
		let error = ValidationError::NotUnique {
			field: "username".to_string(),
			value: "existinguser".to_string(),
		};
		assert_eq!(
			error.to_string(),
			"Field 'username' must be unique. Value 'existinguser' already exists"
		);
	}
}

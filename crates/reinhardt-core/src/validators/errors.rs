//! Validation error types

use thiserror::Error;

#[non_exhaustive]
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
	#[error("Invalid email: {0}")]
	InvalidEmail(String),

	#[error("Invalid URL: {0}")]
	InvalidUrl(String),

	#[error("Value too small: {value} (minimum: {min})")]
	TooSmall { value: String, min: String },

	#[error("Value too large: {value} (maximum: {max})")]
	TooLarge { value: String, max: String },

	#[error("Length too short: {length} (minimum: {min})")]
	TooShort { length: usize, min: usize },

	#[error("Length too long: {length} (maximum: {max})")]
	TooLong { length: usize, max: usize },

	#[error("Pattern mismatch: {0}")]
	PatternMismatch(String),

	#[error("Field '{field}' must be unique. Value '{value}' already exists")]
	NotUnique { field: String, value: String },

	#[error("Invalid slug: {0}")]
	InvalidSlug(String),

	#[error("Invalid UUID: {0}")]
	InvalidUUID(String),

	#[error("Invalid IP address: {0}")]
	InvalidIPAddress(String),

	#[error("Invalid date: {0}")]
	InvalidDate(String),

	#[error("Invalid time: {0}")]
	InvalidTime(String),

	#[error("Invalid datetime: {0}")]
	InvalidDateTime(String),

	#[error("Invalid JSON: {0}")]
	InvalidJSON(String),

	#[error("Invalid credit card number: {0}")]
	InvalidCreditCard(String),

	#[error("Credit card type not allowed: {card_type} (allowed: {allowed_types})")]
	CardTypeNotAllowed {
		card_type: String,
		allowed_types: String,
	},

	#[error("Invalid phone number: {0}")]
	InvalidPhoneNumber(String),

	#[error("Country code not allowed: {country_code} (allowed: {allowed_countries})")]
	CountryCodeNotAllowed {
		country_code: String,
		allowed_countries: String,
	},

	#[error("Invalid IBAN: {0}")]
	InvalidIBAN(String),

	#[error("IBAN country code not allowed: {country_code} (allowed: {allowed_codes})")]
	IBANCountryNotAllowed {
		country_code: String,
		allowed_codes: String,
	},

	#[error("Invalid file extension: {extension} (allowed: {allowed_extensions})")]
	InvalidFileExtension {
		extension: String,
		allowed_extensions: String,
	},

	#[error("Invalid MIME type: {mime_type} (allowed: {allowed_mime_types})")]
	InvalidMimeType {
		mime_type: String,
		allowed_mime_types: String,
	},

	#[error(
		"Foreign key reference not found: {field} with value {value} does not exist in {table}"
	)]
	ForeignKeyNotFound {
		field: String,
		value: String,
		table: String,
	},

	#[error("File size too small: {size_bytes} bytes (minimum: {min_bytes} bytes)")]
	FileSizeTooSmall { size_bytes: u64, min_bytes: u64 },

	#[error("File size too large: {size_bytes} bytes (maximum: {max_bytes} bytes)")]
	FileSizeTooLarge { size_bytes: u64, max_bytes: u64 },

	#[error("All validators failed: {errors}")]
	AllValidatorsFailed { errors: String },

	#[error("Validation failed: {0}")]
	CompositeValidationFailed(String),

	#[error("Invalid postal code: {postal_code}")]
	InvalidPostalCode { postal_code: String },

	#[error("Postal code country not recognized: {postal_code}")]
	PostalCodeCountryNotRecognized { postal_code: String },

	#[error("Country not allowed: {country} (allowed: {allowed_countries})")]
	PostalCodeCountryNotAllowed {
		country: String,
		allowed_countries: String,
	},

	#[error("Image width too small: {width}px (minimum: {min_width}px)")]
	ImageWidthTooSmall { width: u32, min_width: u32 },

	#[error("Image width too large: {width}px (maximum: {max_width}px)")]
	ImageWidthTooLarge { width: u32, max_width: u32 },

	#[error("Image height too small: {height}px (minimum: {min_height}px)")]
	ImageHeightTooSmall { height: u32, min_height: u32 },

	#[error("Image height too large: {height}px (maximum: {max_height}px)")]
	ImageHeightTooLarge { height: u32, max_height: u32 },

	#[error(
		"Invalid aspect ratio: {actual_width}:{actual_height} (expected: {expected_width}:{expected_height})"
	)]
	InvalidAspectRatio {
		actual_width: u32,
		actual_height: u32,
		expected_width: u32,
		expected_height: u32,
	},

	#[error("Cannot read image: {0}")]
	ImageReadError(String),

	#[error("Custom validation error: {0}")]
	Custom(String),
}

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

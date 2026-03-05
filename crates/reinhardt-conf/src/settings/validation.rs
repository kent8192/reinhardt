//! Configuration validation framework
//!
//! Provides validation rules and checks for settings to ensure security
//! and correctness before application startup.

use super::profile::Profile;
use serde_json::Value;
use std::collections::HashMap;

// Import base SettingsValidator trait from reinhardt-core
use reinhardt_core::validators::SettingsValidator as BaseSettingsValidator;

/// Validation result
pub type ValidationResult = Result<(), ValidationError>;

/// Validation error
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
	#[error("Security error: {0}")]
	Security(String),

	#[error("Invalid value for '{key}': {message}")]
	InvalidValue { key: String, message: String },

	#[error("Missing required field: {0}")]
	MissingRequired(String),

	#[error("Constraint violation: {0}")]
	Constraint(String),

	#[error("Multiple validation errors: {0:?}")]
	Multiple(Vec<ValidationError>),
}

impl From<ValidationError> for reinhardt_core::validators::ValidationError {
	fn from(error: ValidationError) -> Self {
		reinhardt_core::validators::ValidationError::Custom(error.to_string())
	}
}

/// Trait for validation rules
pub trait Validator: Send + Sync {
	/// Validate a specific key-value pair
	fn validate(&self, key: &str, value: &Value) -> ValidationResult;

	/// Get validator description
	fn description(&self) -> String;
}

/// Trait for settings validators that can validate entire settings
pub trait SettingsValidator: Send + Sync {
	/// Validate the entire settings map
	fn validate_settings(&self, settings: &HashMap<String, Value>) -> ValidationResult;

	/// Get validator description
	fn description(&self) -> String;
}

/// Required field validator
pub struct RequiredValidator {
	fields: Vec<String>,
}

impl RequiredValidator {
	/// Create a new required field validator
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::validation::RequiredValidator;
	///
	/// let validator = RequiredValidator::new(vec![
	///     "secret_key".to_string(),
	///     "database_url".to_string(),
	/// ]);
	/// // Validator will check that these fields exist in settings
	/// ```
	pub fn new(fields: Vec<String>) -> Self {
		Self { fields }
	}
}

impl SettingsValidator for RequiredValidator {
	fn validate_settings(&self, settings: &HashMap<String, Value>) -> ValidationResult {
		let mut errors = Vec::new();

		for field in &self.fields {
			if !settings.contains_key(field) {
				errors.push(ValidationError::MissingRequired(field.clone()));
			}
		}

		if errors.is_empty() {
			Ok(())
		} else {
			Err(ValidationError::Multiple(errors))
		}
	}

	fn description(&self) -> String {
		format!("Required fields: {:?}", self.fields)
	}
}

impl BaseSettingsValidator for RequiredValidator {
	fn validate_setting(
		&self,
		_key: &str,
		_value: &Value,
	) -> reinhardt_core::validators::ValidationResult<()> {
		// This validator checks presence, not individual values
		// Always pass for individual settings
		Ok(())
	}

	fn description(&self) -> String {
		format!("Required fields: {:?}", self.fields)
	}
}

/// Security validator for production environments
pub struct SecurityValidator {
	profile: Profile,
}

impl SecurityValidator {
	/// Create a new security validator for the given profile
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::validation::SecurityValidator;
	/// use reinhardt_conf::settings::profile::Profile;
	///
	/// let validator = SecurityValidator::new(Profile::Production);
	/// // Validator will enforce production security requirements
	/// ```
	pub fn new(profile: Profile) -> Self {
		Self { profile }
	}
}

impl SettingsValidator for SecurityValidator {
	fn validate_settings(&self, settings: &HashMap<String, Value>) -> ValidationResult {
		if !self.profile.is_production() {
			return Ok(());
		}

		let mut errors = Vec::new();

		// Check DEBUG is false in production
		if let Some(debug) = settings.get("debug")
			&& debug.as_bool() == Some(true)
		{
			errors.push(ValidationError::Security(
				"DEBUG must be false in production".to_string(),
			));
		}

		// Check SECRET_KEY is not default value
		if let Some(secret_key) = settings.get("secret_key")
			&& let Some(key_str) = secret_key.as_str()
			&& (key_str.contains("insecure") || key_str == "change-this" || key_str.len() < 32)
		{
			errors.push(ValidationError::Security(
				"SECRET_KEY must be a strong random value in production".to_string(),
			));
		}

		// Check ALLOWED_HOSTS is set
		if let Some(allowed_hosts) = settings.get("allowed_hosts") {
			if let Some(hosts) = allowed_hosts.as_array()
				&& (hosts.is_empty() || hosts.iter().any(|h| h.as_str() == Some("*")))
			{
				errors.push(ValidationError::Security(
					"ALLOWED_HOSTS must be properly configured in production (no wildcards)"
						.to_string(),
				));
			}
		} else {
			errors.push(ValidationError::Security(
				"ALLOWED_HOSTS must be set in production".to_string(),
			));
		}

		// Check HTTPS settings
		if let Some(secure_ssl) = settings.get("secure_ssl_redirect")
			&& secure_ssl.as_bool() != Some(true)
		{
			errors.push(ValidationError::Security(
				"SECURE_SSL_REDIRECT should be true in production".to_string(),
			));
		}

		if errors.is_empty() {
			Ok(())
		} else {
			Err(ValidationError::Multiple(errors))
		}
	}

	fn description(&self) -> String {
		format!("Security validation for {} environment", self.profile)
	}
}

impl BaseSettingsValidator for SecurityValidator {
	fn validate_setting(
		&self,
		key: &str,
		value: &Value,
	) -> reinhardt_core::validators::ValidationResult<()> {
		if !self.profile.is_production() {
			return Ok(());
		}

		match key {
			"debug" => {
				if value.as_bool() == Some(true) {
					return Err(reinhardt_core::validators::ValidationError::Custom(
						"DEBUG must be false in production".to_string(),
					));
				}
			}
			"secret_key" => {
				if let Some(key_str) = value.as_str()
					&& (key_str.contains("insecure")
						|| key_str == "change-this"
						|| key_str.len() < 32)
				{
					return Err(reinhardt_core::validators::ValidationError::Custom(
						"SECRET_KEY must be a strong random value in production".to_string(),
					));
				}
			}
			"allowed_hosts" => {
				if let Some(hosts) = value.as_array() {
					if hosts.is_empty() || hosts.iter().any(|h| h.as_str() == Some("*")) {
						return Err(reinhardt_core::validators::ValidationError::Custom(
                            "ALLOWED_HOSTS must be properly configured in production (no wildcards)".to_string(),
                        ));
					}
				} else {
					return Err(reinhardt_core::validators::ValidationError::Custom(
						"ALLOWED_HOSTS must be an array".to_string(),
					));
				}
			}
			"secure_ssl_redirect" => {
				if value.as_bool() != Some(true) {
					return Err(reinhardt_core::validators::ValidationError::Custom(
						"SECURE_SSL_REDIRECT should be true in production".to_string(),
					));
				}
			}
			_ => {}
		}

		Ok(())
	}

	fn description(&self) -> String {
		format!("Security validation for {} environment", self.profile)
	}
}

/// Range validator for numeric values
pub struct RangeValidator {
	min: Option<f64>,
	max: Option<f64>,
}

impl RangeValidator {
	/// Create a range validator with optional min and max
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::validation::RangeValidator;
	///
	/// let validator = RangeValidator::new(Some(0.0), Some(100.0));
	/// // Validator will check values are between 0 and 100
	/// ```
	pub fn new(min: Option<f64>, max: Option<f64>) -> Self {
		Self { min, max }
	}
	/// Create a validator with only a minimum value
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::validation::RangeValidator;
	///
	/// let validator = RangeValidator::min(0.0);
	/// // Values must be >= 0
	/// ```
	pub fn min(min: f64) -> Self {
		Self {
			min: Some(min),
			max: None,
		}
	}
	/// Create a validator with only a maximum value
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::validation::RangeValidator;
	///
	/// let validator = RangeValidator::max(100.0);
	/// // Values must be <= 100
	/// ```
	pub fn max(max: f64) -> Self {
		Self {
			min: None,
			max: Some(max),
		}
	}
	/// Create a validator for a range between min and max
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::validation::RangeValidator;
	///
	/// let validator = RangeValidator::between(1.0, 10.0);
	/// // Values must be between 1 and 10 (inclusive)
	/// ```
	pub fn between(min: f64, max: f64) -> Self {
		Self {
			min: Some(min),
			max: Some(max),
		}
	}
}

impl Validator for RangeValidator {
	fn validate(&self, key: &str, value: &Value) -> ValidationResult {
		if let Some(num) = value.as_f64() {
			if let Some(min) = self.min
				&& num < min
			{
				return Err(ValidationError::InvalidValue {
					key: key.to_string(),
					message: format!("Value {} is less than minimum {}", num, min),
				});
			}

			if let Some(max) = self.max
				&& num > max
			{
				return Err(ValidationError::InvalidValue {
					key: key.to_string(),
					message: format!("Value {} is greater than maximum {}", num, max),
				});
			}

			Ok(())
		} else {
			Err(ValidationError::InvalidValue {
				key: key.to_string(),
				message: "Expected numeric value".to_string(),
			})
		}
	}

	fn description(&self) -> String {
		match (self.min, self.max) {
			(Some(min), Some(max)) => format!("Range: {} to {}", min, max),
			(Some(min), None) => format!("Minimum: {}", min),
			(None, Some(max)) => format!("Maximum: {}", max),
			(None, None) => "Range validator".to_string(),
		}
	}
}

impl BaseSettingsValidator for RangeValidator {
	fn validate_setting(
		&self,
		key: &str,
		value: &Value,
	) -> reinhardt_core::validators::ValidationResult<()> {
		if let Some(num) = value.as_f64() {
			if let Some(min) = self.min
				&& num < min
			{
				return Err(reinhardt_core::validators::ValidationError::Custom(
					format!("Value {} for '{}' is less than minimum {}", num, key, min),
				));
			}

			if let Some(max) = self.max
				&& num > max
			{
				return Err(reinhardt_core::validators::ValidationError::Custom(
					format!(
						"Value {} for '{}' is greater than maximum {}",
						num, key, max
					),
				));
			}

			Ok(())
		} else {
			Err(reinhardt_core::validators::ValidationError::Custom(
				format!("Expected numeric value for '{}'", key),
			))
		}
	}

	fn description(&self) -> String {
		match (self.min, self.max) {
			(Some(min), Some(max)) => format!("Range: {} to {}", min, max),
			(Some(min), None) => format!("Minimum: {}", min),
			(None, Some(max)) => format!("Maximum: {}", max),
			(None, None) => "Range validator".to_string(),
		}
	}
}

/// String pattern validator
pub struct PatternValidator {
	pattern: regex::Regex,
}

impl PatternValidator {
	/// Create a pattern validator with a regex pattern
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::validation::PatternValidator;
	///
	/// let validator = PatternValidator::new(r"^\d{3}-\d{3}-\d{4}$").unwrap();
	/// // Validates phone number format
	/// ```
	pub fn new(pattern: &str) -> Result<Self, regex::Error> {
		Ok(Self {
			pattern: regex::Regex::new(pattern)?,
		})
	}
}

impl Validator for PatternValidator {
	fn validate(&self, key: &str, value: &Value) -> ValidationResult {
		if let Some(s) = value.as_str() {
			if self.pattern.is_match(s) {
				Ok(())
			} else {
				Err(ValidationError::InvalidValue {
					key: key.to_string(),
					message: format!("Value does not match pattern: {}", self.pattern.as_str()),
				})
			}
		} else {
			Err(ValidationError::InvalidValue {
				key: key.to_string(),
				message: "Expected string value".to_string(),
			})
		}
	}

	fn description(&self) -> String {
		format!("Pattern: {}", self.pattern.as_str())
	}
}

impl BaseSettingsValidator for PatternValidator {
	fn validate_setting(
		&self,
		key: &str,
		value: &Value,
	) -> reinhardt_core::validators::ValidationResult<()> {
		if let Some(s) = value.as_str() {
			if self.pattern.is_match(s) {
				Ok(())
			} else {
				Err(reinhardt_core::validators::ValidationError::Custom(
					format!(
						"Value for '{}' does not match pattern: {}",
						key,
						self.pattern.as_str()
					),
				))
			}
		} else {
			Err(reinhardt_core::validators::ValidationError::Custom(
				format!("Expected string value for '{}'", key),
			))
		}
	}

	fn description(&self) -> String {
		format!("Pattern: {}", self.pattern.as_str())
	}
}

/// Choice validator (enum-like)
pub struct ChoiceValidator {
	choices: Vec<String>,
}

impl ChoiceValidator {
	/// Create a choice validator with allowed values
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::validation::ChoiceValidator;
	///
	/// let validator = ChoiceValidator::new(vec![
	///     "development".to_string(),
	///     "staging".to_string(),
	///     "production".to_string(),
	/// ]);
	/// // Value must be one of the allowed choices
	/// ```
	pub fn new(choices: Vec<String>) -> Self {
		Self { choices }
	}
}

impl Validator for ChoiceValidator {
	fn validate(&self, key: &str, value: &Value) -> ValidationResult {
		if let Some(s) = value.as_str() {
			if self.choices.contains(&s.to_string()) {
				Ok(())
			} else {
				Err(ValidationError::InvalidValue {
					key: key.to_string(),
					message: format!(
						"Value '{}' is not in allowed choices: {:?}",
						s, self.choices
					),
				})
			}
		} else {
			Err(ValidationError::InvalidValue {
				key: key.to_string(),
				message: "Expected string value".to_string(),
			})
		}
	}

	fn description(&self) -> String {
		format!("Choices: {:?}", self.choices)
	}
}

impl BaseSettingsValidator for ChoiceValidator {
	fn validate_setting(
		&self,
		key: &str,
		value: &Value,
	) -> reinhardt_core::validators::ValidationResult<()> {
		if let Some(s) = value.as_str() {
			if self.choices.contains(&s.to_string()) {
				Ok(())
			} else {
				Err(reinhardt_core::validators::ValidationError::Custom(
					format!(
						"Value '{}' for '{}' is not in allowed choices: {:?}",
						s, key, self.choices
					),
				))
			}
		} else {
			Err(reinhardt_core::validators::ValidationError::Custom(
				format!("Expected string value for '{}'", key),
			))
		}
	}

	fn description(&self) -> String {
		format!("Choices: {:?}", self.choices)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_settings_validation_required() {
		let validator = RequiredValidator::new(vec!["key1".to_string(), "key2".to_string()]);

		let mut settings = HashMap::new();
		settings.insert("key1".to_string(), Value::String("value".to_string()));

		assert!(validator.validate_settings(&settings).is_err());

		settings.insert("key2".to_string(), Value::String("value".to_string()));
		assert!(validator.validate_settings(&settings).is_ok());
	}

	#[test]
	fn test_security_validator_production() {
		let validator = SecurityValidator::new(Profile::Production);

		let mut settings = HashMap::new();
		settings.insert("debug".to_string(), Value::Bool(true));
		settings.insert(
			"secret_key".to_string(),
			Value::String("insecure".to_string()),
		);

		let result = validator.validate_settings(&settings);
		assert!(result.is_err());
	}

	#[test]
	fn test_security_validator_development() {
		let validator = SecurityValidator::new(Profile::Development);

		let mut settings = HashMap::new();
		settings.insert("debug".to_string(), Value::Bool(true));
		settings.insert(
			"secret_key".to_string(),
			Value::String("insecure".to_string()),
		);

		// Should pass in development
		assert!(validator.validate_settings(&settings).is_ok());
	}

	#[test]
	fn test_settings_range_validator() {
		let validator = RangeValidator::between(0.0, 100.0);

		assert!(validator.validate("key", &Value::Number(50.into())).is_ok());
		assert!(
			validator
				.validate("key", &Value::Number((-10).into()))
				.is_err()
		);
		assert!(
			validator
				.validate("key", &Value::Number(150.into()))
				.is_err()
		);
	}

	#[test]
	fn test_settings_validation_choice() {
		let validator =
			ChoiceValidator::new(vec!["a".to_string(), "b".to_string(), "c".to_string()]);

		assert!(
			validator
				.validate("key", &Value::String("a".to_string()))
				.is_ok()
		);
		assert!(
			validator
				.validate("key", &Value::String("d".to_string()))
				.is_err()
		);
	}
}

//! Client-Side Validator Registry (Phase 2-A Step 4)
//!
//! This module provides a registry for client-side validators that can be
//! referenced by ValidationRule::ValidatorRef.
//!
//! ## Architecture
//!
//! ```mermaid
//! flowchart LR
//!     subgraph Server["Server (reinhardt-forms)"]
//!         Form["Form<br/>.add_validator_rule()<br/>(&quot;email&quot;, ...)"]
//!     end
//!
//!     subgraph Client["Client (reinhardt-pages)"]
//!         Registry["ValidatorRegistry<br/>.register()<br/>.validate()"]
//!     end
//!
//!     Form -->|JSON| Registry
//! ```
//!
//! ## Security Note
//!
//! Client-side validators are for UX enhancement only and MUST NOT be relied
//! upon for security. Server-side validation is always required.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// Client-side validator trait (Phase 2-A)
///
/// Validators implement this trait to provide client-side validation logic.
pub trait ClientValidator: Send + Sync {
	/// Validate a field value
	///
	/// # Arguments
	///
	/// - `value`: The field value to validate
	/// - `params`: Validator parameters as JSON
	///
	/// # Returns
	///
	/// `Ok(())` if validation passes, `Err(error_message)` if validation fails
	fn validate(&self, value: &str, params: &serde_json::Value) -> Result<(), String>;
}

/// Global validator registry singleton (Phase 2-A)
static VALIDATOR_REGISTRY: OnceLock<Arc<Mutex<ValidatorRegistry>>> = OnceLock::new();

/// Validator registry (Phase 2-A)
///
/// This registry stores client-side validators indexed by their ID.
pub struct ValidatorRegistry {
	validators: HashMap<String, Arc<dyn ClientValidator>>,
}

impl ValidatorRegistry {
	/// Create a new empty registry
	fn new() -> Self {
		Self {
			validators: HashMap::new(),
		}
	}

	/// Get the global validator registry instance
	pub fn global() -> Arc<Mutex<ValidatorRegistry>> {
		VALIDATOR_REGISTRY
			.get_or_init(|| {
				let registry = Arc::new(Mutex::new(Self::new()));
				// Initialize default validators
				initialize_default_validators(&registry);
				registry
			})
			.clone()
	}

	/// Register a validator
	///
	/// # Arguments
	///
	/// - `id`: Validator identifier (e.g., "email", "min_length")
	/// - `validator`: Validator implementation
	pub fn register(&mut self, id: impl Into<String>, validator: Arc<dyn ClientValidator>) {
		self.validators.insert(id.into(), validator);
	}

	/// Get a validator by ID
	///
	/// # Arguments
	///
	/// - `id`: Validator identifier
	///
	/// # Returns
	///
	/// Validator if found, None otherwise
	pub fn get(&self, id: &str) -> Option<Arc<dyn ClientValidator>> {
		self.validators.get(id).cloned()
	}

	/// Validate a value using a registered validator
	///
	/// # Arguments
	///
	/// - `id`: Validator identifier
	/// - `value`: Field value to validate
	/// - `params`: Validator parameters
	///
	/// # Returns
	///
	/// `Ok(())` if validation passes, `Err(error_message)` if validation fails
	pub fn validate(
		&self,
		id: &str,
		value: &str,
		params: &serde_json::Value,
	) -> Result<(), String> {
		let validator = self
			.get(id)
			.ok_or_else(|| format!("Validator '{}' not found", id))?;
		validator.validate(value, params)
	}
}

// ============================================================================
// Built-in Validators
// ============================================================================

/// Email validator (Phase 2-A)
///
/// Validates that a string is a valid email address.
struct EmailValidator;

impl ClientValidator for EmailValidator {
	fn validate(&self, value: &str, _params: &serde_json::Value) -> Result<(), String> {
		// Simple email validation (more sophisticated validation should be done server-side)
		let email_regex = regex::Regex::new(r"^[^\s@]+@[^\s@]+\.[^\s@]+$")
			.map_err(|e| format!("Email regex error: {}", e))?;

		if email_regex.is_match(value) {
			Ok(())
		} else {
			Err("Enter a valid email address".to_string())
		}
	}
}

/// MinLength validator (Phase 2-A)
///
/// Validates that a string has a minimum length.
struct MinLengthValidator;

impl ClientValidator for MinLengthValidator {
	fn validate(&self, value: &str, params: &serde_json::Value) -> Result<(), String> {
		let min = params
			.get("min")
			.and_then(|v| v.as_u64())
			.ok_or_else(|| "MinLength validator requires 'min' parameter".to_string())?
			as usize;

		if value.len() >= min {
			Ok(())
		} else {
			Err(format!(
				"This field must be at least {} characters long",
				min
			))
		}
	}
}

/// MaxLength validator (Phase 2-A)
///
/// Validates that a string does not exceed a maximum length.
struct MaxLengthValidator;

impl ClientValidator for MaxLengthValidator {
	fn validate(&self, value: &str, params: &serde_json::Value) -> Result<(), String> {
		let max = params
			.get("max")
			.and_then(|v| v.as_u64())
			.ok_or_else(|| "MaxLength validator requires 'max' parameter".to_string())?
			as usize;

		if value.len() <= max {
			Ok(())
		} else {
			Err(format!(
				"This field must be at most {} characters long",
				max
			))
		}
	}
}

/// URL validator (Phase 2-A)
///
/// Validates that a string is a valid URL.
struct UrlValidator;

impl ClientValidator for UrlValidator {
	fn validate(&self, value: &str, _params: &serde_json::Value) -> Result<(), String> {
		// Simple URL validation (more sophisticated validation should be done server-side)
		let url_regex = regex::Regex::new(r"^https?://[^\s/$.?#].[^\s]*$")
			.map_err(|e| format!("URL regex error: {}", e))?;

		if url_regex.is_match(value) {
			Ok(())
		} else {
			Err("Enter a valid URL".to_string())
		}
	}
}

/// Initialize default validators (Phase 2-A)
///
/// Registers built-in validators: email, min_length, max_length, url
fn initialize_default_validators(registry: &Arc<Mutex<ValidatorRegistry>>) {
	let mut registry = registry.lock().unwrap_or_else(|e| e.into_inner());

	// Register built-in validators
	registry.register("email", Arc::new(EmailValidator));
	registry.register("min_length", Arc::new(MinLengthValidator));
	registry.register("max_length", Arc::new(MaxLengthValidator));
	registry.register("url", Arc::new(UrlValidator));
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;

	#[test]
	fn test_email_validator_valid() {
		let validator = EmailValidator;
		assert!(validator.validate("user@example.com", &json!({})).is_ok());
	}

	#[test]
	fn test_email_validator_invalid() {
		let validator = EmailValidator;
		assert!(validator.validate("invalid-email", &json!({})).is_err());
		assert!(validator.validate("@example.com", &json!({})).is_err());
		assert!(validator.validate("user@", &json!({})).is_err());
	}

	#[test]
	fn test_min_length_validator() {
		let validator = MinLengthValidator;

		// Valid: meets minimum
		assert!(validator.validate("12345", &json!({"min": 5})).is_ok());
		assert!(validator.validate("123456", &json!({"min": 5})).is_ok());

		// Invalid: below minimum
		assert!(validator.validate("1234", &json!({"min": 5})).is_err());
	}

	#[test]
	fn test_max_length_validator() {
		let validator = MaxLengthValidator;

		// Valid: within maximum
		assert!(validator.validate("12345", &json!({"max": 5})).is_ok());
		assert!(validator.validate("1234", &json!({"max": 5})).is_ok());

		// Invalid: exceeds maximum
		assert!(validator.validate("123456", &json!({"max": 5})).is_err());
	}

	#[test]
	fn test_url_validator_valid() {
		let validator = UrlValidator;
		assert!(
			validator
				.validate("https://example.com", &json!({}))
				.is_ok()
		);
		assert!(
			validator
				.validate("http://example.com/path", &json!({}))
				.is_ok()
		);
	}

	#[test]
	fn test_url_validator_invalid() {
		let validator = UrlValidator;
		assert!(validator.validate("not-a-url", &json!({})).is_err());
		assert!(validator.validate("ftp://example.com", &json!({})).is_err());
	}

	#[test]
	fn test_validator_registry() {
		let registry = Arc::new(Mutex::new(ValidatorRegistry::new()));
		initialize_default_validators(&registry);

		let registry = registry.lock().unwrap_or_else(|e| e.into_inner());

		// Test email validator
		assert!(
			registry
				.validate("email", "user@example.com", &json!({}))
				.is_ok()
		);
		assert!(registry.validate("email", "invalid", &json!({})).is_err());

		// Test min_length validator
		assert!(
			registry
				.validate("min_length", "12345", &json!({"min": 5}))
				.is_ok()
		);
		assert!(
			registry
				.validate("min_length", "1234", &json!({"min": 5}))
				.is_err()
		);

		// Test max_length validator
		assert!(
			registry
				.validate("max_length", "12345", &json!({"max": 5}))
				.is_ok()
		);
		assert!(
			registry
				.validate("max_length", "123456", &json!({"max": 5}))
				.is_err()
		);

		// Test url validator
		assert!(
			registry
				.validate("url", "https://example.com", &json!({}))
				.is_ok()
		);
		assert!(registry.validate("url", "not-a-url", &json!({})).is_err());
	}

	#[test]
	fn test_validator_registry_global() {
		let registry = ValidatorRegistry::global();
		let registry = registry.lock().unwrap_or_else(|e| e.into_inner());

		// Global registry should have default validators
		assert!(registry.get("email").is_some());
		assert!(registry.get("min_length").is_some());
		assert!(registry.get("max_length").is_some());
		assert!(registry.get("url").is_some());
	}
}

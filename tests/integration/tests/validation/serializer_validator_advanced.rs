//! Advanced Serializer + Validator Integration Tests
//!
//! Tests advanced validation scenarios including conditional validation,
//! async database validation, custom error formatting, short-circuit behavior,
//! and list field validation.
//!
//! ## Test Coverage
//!
//! This test file covers:
//! - **Conditional Validation**: Validation logic dependent on other field values
//! - **Async Database Validation**: Uniqueness checks requiring database queries
//! - **Custom Error Formatting**: Customized error messages with field context
//! - **Short-Circuit Behavior**: Early termination vs full error collection
//! - **List Field Validation**: Applying validators to each item in `Vec<T>`
//!
//! ## Test Categories
//!
//! 1. **Conditional Validation**: Field validation based on runtime conditions
//! 2. **Async Validation**: Database-backed validation with async operations
//! 3. **Error Formatting**: Custom error message generation and localization
//! 4. **Validation Strategy**: Short-circuit vs comprehensive error collection
//! 5. **Collection Validation**: Validating items within collections
//!
//! ## Fixtures Used
//!
//! - `postgres_container`: For async database uniqueness validation
//!
//! ## What These Tests Verify
//!
//! ✅ Conditional validation based on other field values works correctly
//! ✅ Async database validation executes properly with TestContainers
//! ✅ Custom error messages can be formatted with field context
//! ✅ Validation can stop at first error or collect all errors
//! ✅ Validators apply correctly to collection items (`Vec<T>`)
//!
//! ## What These Tests Don't Cover
//!
//! ❌ Validation performance benchmarking
//! ❌ Concurrent validation execution
//! ❌ I18n error message translation
//! ❌ Validation caching strategies

use reinhardt_test::fixtures::*;
use rstest::*;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;

// ============ Test Helper Structs ============

/// Payment method with conditional validation
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PaymentData {
	payment_method: String, // "card" or "bank_transfer"
	card_number: Option<String>,
	card_expiry: Option<String>,
	bank_account: Option<String>,
	bank_code: Option<String>,
}

impl PaymentData {
	fn new(
		payment_method: &str,
		card_number: Option<&str>,
		card_expiry: Option<&str>,
		bank_account: Option<&str>,
		bank_code: Option<&str>,
	) -> Self {
		Self {
			payment_method: payment_method.to_string(),
			card_number: card_number.map(|s| s.to_string()),
			card_expiry: card_expiry.map(|s| s.to_string()),
			bank_account: bank_account.map(|s| s.to_string()),
			bank_code: bank_code.map(|s| s.to_string()),
		}
	}

	/// Conditional validation based on payment_method
	fn validate(&self) -> Result<(), HashMap<String, String>> {
		let mut errors = HashMap::new();

		match self.payment_method.as_str() {
			"card" => {
				if self.card_number.is_none() {
					errors.insert(
						"card_number".to_string(),
						"Card number is required for card payment".to_string(),
					);
				} else if let Some(ref number) = self.card_number {
					if number.len() < 13 || number.len() > 19 {
						errors.insert(
							"card_number".to_string(),
							"Card number must be between 13 and 19 digits".to_string(),
						);
					}
				}

				if self.card_expiry.is_none() {
					errors.insert(
						"card_expiry".to_string(),
						"Card expiry is required for card payment".to_string(),
					);
				}
			}
			"bank_transfer" => {
				if self.bank_account.is_none() {
					errors.insert(
						"bank_account".to_string(),
						"Bank account is required for bank transfer".to_string(),
					);
				}

				if self.bank_code.is_none() {
					errors.insert(
						"bank_code".to_string(),
						"Bank code is required for bank transfer".to_string(),
					);
				} else if let Some(ref code) = self.bank_code {
					if code.len() != 4 {
						errors.insert(
							"bank_code".to_string(),
							"Bank code must be exactly 4 characters".to_string(),
						);
					}
				}
			}
			_ => {
				errors.insert(
					"payment_method".to_string(),
					"Invalid payment method. Use 'card' or 'bank_transfer'".to_string(),
				);
			}
		}

		if errors.is_empty() {
			Ok(())
		} else {
			Err(errors)
		}
	}
}

/// User model for async uniqueness validation
#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
	id: Option<i64>,
	username: String,
	email: String,
}

impl User {
	fn new(username: &str, email: &str) -> Self {
		Self {
			id: None,
			username: username.to_string(),
			email: email.to_string(),
		}
	}

	/// Async uniqueness validation for username
	async fn validate_username_unique(&self, pool: &PgPool) -> Result<(), String> {
		let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE username = $1")
			.bind(&self.username)
			.fetch_one(pool)
			.await
			.map_err(|e| format!("Database error: {}", e))?;

		if count > 0 {
			Err(format!("Username '{}' is already taken", self.username))
		} else {
			Ok(())
		}
	}

	/// Async uniqueness validation for email
	async fn validate_email_unique(&self, pool: &PgPool) -> Result<(), String> {
		let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE email = $1")
			.bind(&self.email)
			.fetch_one(pool)
			.await
			.map_err(|e| format!("Database error: {}", e))?;

		if count > 0 {
			Err(format!("Email '{}' is already registered", self.email))
		} else {
			Ok(())
		}
	}
}

/// Custom error formatter
struct ValidationErrorFormatter;

impl ValidationErrorFormatter {
	fn format_field_error(field: &str, error: &str) -> String {
		format!("[{}] {}", field.to_uppercase(), error)
	}

	fn format_multiple_errors(errors: &HashMap<String, String>) -> String {
		let mut formatted = String::from("Validation failed:\n");
		for (field, error) in errors {
			formatted.push_str(&format!("  - {}: {}\n", field, error));
		}
		formatted
	}
}

/// Validation strategy enum
#[derive(Debug, Clone, Copy)]
enum ValidationStrategy {
	ShortCircuit,  // Stop at first error
	Comprehensive, // Collect all errors
}

/// Registration data with validation strategy support
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RegistrationData {
	username: String,
	email: String,
	password: String,
	age: Option<i32>,
}

impl RegistrationData {
	fn new(username: &str, email: &str, password: &str, age: Option<i32>) -> Self {
		Self {
			username: username.to_string(),
			email: email.to_string(),
			password: password.to_string(),
			age,
		}
	}

	/// Validate with specified strategy
	fn validate(&self, strategy: ValidationStrategy) -> Result<(), HashMap<String, String>> {
		let mut errors = HashMap::new();

		// Username validation
		if self.username.len() < 3 {
			errors.insert(
				"username".to_string(),
				"Username must be at least 3 characters".to_string(),
			);
			if matches!(strategy, ValidationStrategy::ShortCircuit) {
				return Err(errors);
			}
		}

		// Email validation
		if !self.email.contains('@') {
			errors.insert(
				"email".to_string(),
				"Email must contain @ symbol".to_string(),
			);
			if matches!(strategy, ValidationStrategy::ShortCircuit) && !errors.is_empty() {
				return Err(errors);
			}
		}

		// Password validation
		if self.password.len() < 8 {
			errors.insert(
				"password".to_string(),
				"Password must be at least 8 characters".to_string(),
			);
			if matches!(strategy, ValidationStrategy::ShortCircuit) && !errors.is_empty() {
				return Err(errors);
			}
		}

		// Age validation
		if let Some(age) = self.age {
			if age < 18 {
				errors.insert(
					"age".to_string(),
					"User must be at least 18 years old".to_string(),
				);
				if matches!(strategy, ValidationStrategy::ShortCircuit) && !errors.is_empty() {
					return Err(errors);
				}
			}
		}

		if errors.is_empty() {
			Ok(())
		} else {
			Err(errors)
		}
	}
}

/// Tag list with item validation
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TagList {
	tags: Vec<String>,
}

impl TagList {
	fn new(tags: Vec<&str>) -> Self {
		Self {
			tags: tags.iter().map(|s| s.to_string()).collect(),
		}
	}

	/// Validate each tag in the list
	fn validate(&self) -> Result<(), HashMap<String, String>> {
		let mut errors = HashMap::new();

		for (index, tag) in self.tags.iter().enumerate() {
			let field_name = format!("tags[{}]", index);

			// Tag must not be empty
			if tag.is_empty() {
				errors.insert(field_name.clone(), "Tag cannot be empty".to_string());
				continue;
			}

			// Tag must be between 2 and 20 characters
			if tag.len() < 2 {
				errors.insert(
					field_name.clone(),
					"Tag must be at least 2 characters".to_string(),
				);
			} else if tag.len() > 20 {
				errors.insert(
					field_name.clone(),
					"Tag must not exceed 20 characters".to_string(),
				);
			}

			// Tag must only contain alphanumeric and hyphens
			if !tag.chars().all(|c| c.is_alphanumeric() || c == '-') {
				errors.insert(
					field_name,
					"Tag can only contain letters, numbers, and hyphens".to_string(),
				);
			}
		}

		if errors.is_empty() {
			Ok(())
		} else {
			Err(errors)
		}
	}
}

// ============ Test Cases ============

/// Test conditional validation based on field value
///
/// Verifies:
/// - Card payment requires card fields
/// - Bank transfer requires bank fields
/// - Field validation depends on payment_method value
/// - Invalid payment method is rejected
#[test]
fn test_conditional_validation_based_on_field_value() {
	// Valid card payment
	let card_payment =
		PaymentData::new("card", Some("4111111111111111"), Some("12/25"), None, None);
	assert!(card_payment.validate().is_ok());

	// Invalid card payment - missing card_number
	let invalid_card = PaymentData::new("card", None, Some("12/25"), None, None);
	let result = invalid_card.validate();
	assert!(result.is_err());
	let errors = result.unwrap_err();
	assert!(errors.contains_key("card_number"));
	assert_eq!(
		errors.get("card_number").unwrap(),
		"Card number is required for card payment"
	);

	// Invalid card payment - invalid card_number length
	let invalid_card_length = PaymentData::new("card", Some("123"), Some("12/25"), None, None);
	let result = invalid_card_length.validate();
	assert!(result.is_err());
	let errors = result.unwrap_err();
	assert!(errors.contains_key("card_number"));
	assert_eq!(
		errors.get("card_number").unwrap(),
		"Card number must be between 13 and 19 digits"
	);

	// Valid bank transfer
	let bank_payment = PaymentData::new(
		"bank_transfer",
		None,
		None,
		Some("1234567890"),
		Some("ABCD"),
	);
	assert!(bank_payment.validate().is_ok());

	// Invalid bank transfer - missing bank_account
	let invalid_bank = PaymentData::new("bank_transfer", None, None, None, Some("ABCD"));
	let result = invalid_bank.validate();
	assert!(result.is_err());
	let errors = result.unwrap_err();
	assert!(errors.contains_key("bank_account"));
	assert_eq!(
		errors.get("bank_account").unwrap(),
		"Bank account is required for bank transfer"
	);

	// Invalid bank transfer - invalid bank_code length
	let invalid_bank_code =
		PaymentData::new("bank_transfer", None, None, Some("1234567890"), Some("AB"));
	let result = invalid_bank_code.validate();
	assert!(result.is_err());
	let errors = result.unwrap_err();
	assert!(errors.contains_key("bank_code"));
	assert_eq!(
		errors.get("bank_code").unwrap(),
		"Bank code must be exactly 4 characters"
	);

	// Invalid payment method
	let invalid_method = PaymentData::new("crypto", None, None, None, None);
	let result = invalid_method.validate();
	assert!(result.is_err());
	let errors = result.unwrap_err();
	assert!(errors.contains_key("payment_method"));
	assert_eq!(
		errors.get("payment_method").unwrap(),
		"Invalid payment method. Use 'card' or 'bank_transfer'"
	);
}

/// Test async uniqueness validation with database
///
/// Verifies:
/// - Username uniqueness check via database query
/// - Email uniqueness check via database query
/// - Proper error messages for duplicate entries
/// - Async validation executes correctly with TestContainers
#[rstest]
#[tokio::test]
async fn test_async_uniqueness_validation(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_container.await;

	// Create users table
	sqlx::query(
		"CREATE TABLE users (
			id BIGSERIAL PRIMARY KEY,
			username VARCHAR(100) UNIQUE NOT NULL,
			email VARCHAR(255) UNIQUE NOT NULL
		)",
	)
	.execute(pool.as_ref())
	.await
	.expect("Failed to create users table");

	// Insert existing user
	sqlx::query("INSERT INTO users (username, email) VALUES ($1, $2)")
		.bind("alice")
		.bind("alice@example.com")
		.execute(pool.as_ref())
		.await
		.expect("Failed to insert test user");

	// Test username uniqueness - duplicate
	let duplicate_username = User::new("alice", "bob@example.com");
	let result = duplicate_username
		.validate_username_unique(pool.as_ref())
		.await;
	assert!(result.is_err());
	assert_eq!(result.unwrap_err(), "Username 'alice' is already taken");

	// Test username uniqueness - available
	let new_username = User::new("bob", "bob@example.com");
	let result = new_username.validate_username_unique(pool.as_ref()).await;
	assert!(result.is_ok());

	// Test email uniqueness - duplicate
	let duplicate_email = User::new("bob", "alice@example.com");
	let result = duplicate_email.validate_email_unique(pool.as_ref()).await;
	assert!(result.is_err());
	assert_eq!(
		result.unwrap_err(),
		"Email 'alice@example.com' is already registered"
	);

	// Test email uniqueness - available
	let new_email = User::new("bob", "bob@example.com");
	let result = new_email.validate_email_unique(pool.as_ref()).await;
	assert!(result.is_ok());
}

/// Test custom validator error message formatting
///
/// Verifies:
/// - Custom error formatter applies field context
/// - Multiple error messages are aggregated correctly
/// - Error formatting maintains readability
#[test]
fn test_custom_validator_error_message_formatting() {
	// Single field error
	let field_error =
		ValidationErrorFormatter::format_field_error("username", "Username is too short");
	assert_eq!(field_error, "[USERNAME] Username is too short");

	// Multiple field errors
	let mut errors = HashMap::new();
	errors.insert("username".to_string(), "Username is required".to_string());
	errors.insert("email".to_string(), "Email format is invalid".to_string());
	errors.insert("password".to_string(), "Password is too weak".to_string());

	let formatted = ValidationErrorFormatter::format_multiple_errors(&errors);

	// Verify formatted string contains all errors
	assert!(formatted.contains("Validation failed:"));
	assert!(formatted.contains("username: Username is required"));
	assert!(formatted.contains("email: Email format is invalid"));
	assert!(formatted.contains("password: Password is too weak"));

	// Verify each error is on a separate line
	let lines: Vec<&str> = formatted.lines().collect();
	assert_eq!(lines.len(), 4); // Header + 3 errors
}

/// Test validation short-circuit on first error vs comprehensive error collection
///
/// Verifies:
/// - ShortCircuit strategy stops at first error
/// - Comprehensive strategy collects all errors
/// - Both strategies return correct error details
#[test]
fn test_validation_short_circuit_on_first_error() {
	// Create data with multiple validation errors
	let invalid_data = RegistrationData::new("ab", "invalid-email", "short", Some(15));

	// Short-circuit validation - should stop at first error
	let result = invalid_data.validate(ValidationStrategy::ShortCircuit);
	assert!(result.is_err());
	let errors = result.unwrap_err();
	// Should contain only the first error (username)
	assert_eq!(errors.len(), 1);
	assert!(errors.contains_key("username"));

	// Comprehensive validation - should collect all errors
	let result = invalid_data.validate(ValidationStrategy::Comprehensive);
	assert!(result.is_err());
	let errors = result.unwrap_err();
	// Should contain all 4 errors
	assert_eq!(errors.len(), 4);
	assert!(errors.contains_key("username"));
	assert!(errors.contains_key("email"));
	assert!(errors.contains_key("password"));
	assert!(errors.contains_key("age"));

	// Verify error messages
	assert_eq!(
		errors.get("username").unwrap(),
		"Username must be at least 3 characters"
	);
	assert_eq!(errors.get("email").unwrap(), "Email must contain @ symbol");
	assert_eq!(
		errors.get("password").unwrap(),
		"Password must be at least 8 characters"
	);
	assert_eq!(
		errors.get("age").unwrap(),
		"User must be at least 18 years old"
	);

	// Valid data should pass both strategies
	let valid_data = RegistrationData::new("alice", "alice@example.com", "password123", Some(25));
	assert!(
		valid_data
			.validate(ValidationStrategy::ShortCircuit)
			.is_ok()
	);
	assert!(
		valid_data
			.validate(ValidationStrategy::Comprehensive)
			.is_ok()
	);
}

/// Test list field item validation
///
/// Verifies:
/// - Validator applies to each item in `Vec<T>`
/// - Individual item errors are indexed correctly
/// - Empty items are detected
/// - Length constraints apply per item
/// - Character constraints apply per item
#[test]
fn test_list_field_item_validation() {
	// Valid tag list
	let valid_tags = TagList::new(vec!["rust", "web-dev", "api-design"]);
	assert!(valid_tags.validate().is_ok());

	// Tag list with empty tag
	let empty_tag = TagList::new(vec!["rust", "", "api"]);
	let result = empty_tag.validate();
	assert!(result.is_err());
	let errors = result.unwrap_err();
	assert!(errors.contains_key("tags[1]"));
	assert_eq!(errors.get("tags[1]").unwrap(), "Tag cannot be empty");

	// Tag list with too short tag
	let short_tag = TagList::new(vec!["rust", "a", "api"]);
	let result = short_tag.validate();
	assert!(result.is_err());
	let errors = result.unwrap_err();
	assert!(errors.contains_key("tags[1]"));
	assert_eq!(
		errors.get("tags[1]").unwrap(),
		"Tag must be at least 2 characters"
	);

	// Tag list with too long tag
	let long_tag = TagList::new(vec![
		"rust",
		"this-is-a-very-long-tag-name-exceeding-limit",
		"api",
	]);
	let result = long_tag.validate();
	assert!(result.is_err());
	let errors = result.unwrap_err();
	assert!(errors.contains_key("tags[1]"));
	assert_eq!(
		errors.get("tags[1]").unwrap(),
		"Tag must not exceed 20 characters"
	);

	// Tag list with invalid characters
	let invalid_chars = TagList::new(vec!["rust", "web dev", "api"]);
	let result = invalid_chars.validate();
	assert!(result.is_err());
	let errors = result.unwrap_err();
	assert!(errors.contains_key("tags[1]"));
	assert_eq!(
		errors.get("tags[1]").unwrap(),
		"Tag can only contain letters, numbers, and hyphens"
	);

	// Tag list with multiple errors
	let multiple_errors = TagList::new(vec!["rust", "", "this-is-too-long-tag-name", "web dev"]);
	let result = multiple_errors.validate();
	assert!(result.is_err());
	let errors = result.unwrap_err();
	// Should have 3 errors: empty, too long, invalid chars
	assert_eq!(errors.len(), 3);
	assert!(errors.contains_key("tags[1]"));
	assert!(errors.contains_key("tags[2]"));
	assert!(errors.contains_key("tags[3]"));
}

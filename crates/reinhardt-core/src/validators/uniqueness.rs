//! Uniqueness validators for database fields
//!
//! Based on Django REST Framework's UniqueValidator and UniqueTogetherValidator.
//! These validators check for unique values in the database, with support for
//! excluding the current instance during updates.

use super::errors::{ValidationError, ValidationResult};
use std::future::Future;
use std::pin::Pin;

/// Type alias for async uniqueness check function
pub type UniquenessCheckFn =
	Box<dyn Fn(String, Option<i32>) -> Pin<Box<dyn Future<Output = bool> + Send>> + Send + Sync>;

/// Validator for checking uniqueness of a field value in the database
///
/// This validator is inspired by Django REST Framework's UniqueValidator.
/// It checks if a value already exists in the database, with support for
/// excluding the current instance during updates.
///
/// # Examples
///
/// ```rust
/// use reinhardt_core::validators::UniqueValidator;
///
/// # async fn example() {
/// // Create a validator with a database check function
/// let validator = UniqueValidator::new(
///     "username",
///     Box::new(|value, exclude_id| {
///         Box::pin(async move {
///             // Database check logic here
///             // Return true if value exists (excluding exclude_id if provided)
///             false
///         })
///     })
/// );
///
/// // Validate a new value (no instance to exclude)
/// let result = validator.validate_async("newuser", None).await;
/// assert!(result.is_ok());
///
/// // Validate during update (exclude current instance)
/// let result = validator.validate_async("existinguser", Some(42)).await;
/// # }
/// ```
pub struct UniqueValidator {
	/// Field name for error messages
	field_name: String,
	/// Async function to check if value exists in database
	/// Parameters: (value, exclude_id) -> exists
	check_fn: UniquenessCheckFn,
}

impl UniqueValidator {
	/// Create a new uniqueness validator
	///
	/// # Parameters
	///
	/// * `field_name` - Name of the field being validated (for error messages)
	/// * `check_fn` - Async function that checks if the value exists in the database.
	///   Returns true if the value already exists (excluding the instance if provided)
	pub fn new(field_name: impl Into<String>, check_fn: UniquenessCheckFn) -> Self {
		Self {
			field_name: field_name.into(),
			check_fn,
		}
	}

	/// Validate a value asynchronously, optionally excluding an instance
	///
	/// # Parameters
	///
	/// * `value` - The value to check for uniqueness
	/// * `exclude_id` - Optional ID of the instance to exclude from the check.
	///   Used during updates to allow keeping the same value
	///
	/// # Returns
	///
	/// * `Ok(())` if the value is unique (or is the current instance's value)
	/// * `Err(ValidationError::NotUnique)` if the value already exists
	pub async fn validate_async(
		&self,
		value: impl Into<String>,
		exclude_id: Option<i32>,
	) -> ValidationResult<()> {
		let value_str = value.into();
		let exists = (self.check_fn)(value_str.clone(), exclude_id).await;

		if exists {
			Err(ValidationError::NotUnique {
				field: self.field_name.clone(),
				value: value_str,
			})
		} else {
			Ok(())
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::collections::HashSet;
	use std::sync::{Arc, Mutex};

	#[tokio::test]
	async fn test_unique_validator_new_value() {
		let existing_values = Arc::new(Mutex::new(HashSet::from(["existinguser".to_string()])));

		let validator = UniqueValidator::new(
			"username",
			Box::new(move |value, exclude_id| {
				let existing = existing_values.clone();
				Box::pin(async move {
					let values = existing.lock().unwrap();
					if let Some(_id) = exclude_id {
						// For this test, we don't track IDs, so just check existence
						values.contains(&value)
					} else {
						values.contains(&value)
					}
				})
			}),
		);

		// New value should pass
		let result = validator.validate_async("newuser", None).await;
		assert!(result.is_ok());

		// Existing value should fail
		let result = validator.validate_async("existinguser", None).await;
		assert!(result.is_err());
		if let Err(ValidationError::NotUnique { field, value }) = result {
			assert_eq!(field, "username");
			assert_eq!(value, "existinguser");
		} else {
			panic!("Expected NotUnique error");
		}
	}

	#[tokio::test]
	async fn test_unique_validator_excludes_instance() {
		// Simulate a database with users: id=1 -> "user1", id=2 -> "user2"
		let users = Arc::new(Mutex::new(vec![
			(1, "user1".to_string()),
			(2, "user2".to_string()),
		]));

		let validator = UniqueValidator::new(
			"username",
			Box::new(move |value, exclude_id| {
				let users_clone = users.clone();
				Box::pin(async move {
					let users = users_clone.lock().unwrap();
					// Check if value exists, excluding the specified ID
					users
						.iter()
						.any(|(id, username)| username == &value && Some(*id) != exclude_id)
				})
			}),
		);

		// Updating user1 with the same username should succeed (exclude_id=1)
		let result = validator.validate_async("user1", Some(1)).await;
		assert!(result.is_ok());

		// Updating user1 to user2's username should fail
		let result = validator.validate_async("user2", Some(1)).await;
		assert!(result.is_err());

		// Creating a new user with existing username should fail
		let result = validator.validate_async("user1", None).await;
		assert!(result.is_err());
	}

	#[tokio::test]
	async fn test_unique_validator_empty_database() {
		let validator = UniqueValidator::new(
			"email",
			Box::new(|_value, _exclude_id| Box::pin(async move { false })),
		);

		let result = validator.validate_async("test@example.com", None).await;
		assert!(result.is_ok());
	}
}

//! Cache key validation for throttle backends
//!
//! Provides validation and sanitization of cache keys to prevent injection
//! attacks, key collisions, and other security issues when keys are derived
//! from user input.

use super::ThrottleError;

/// Maximum allowed length for a single key component (scope name, identifier, etc.)
const MAX_KEY_COMPONENT_LENGTH: usize = 256;

/// Validates a cache key component (e.g., scope name, user identifier, IP address).
///
/// Rejects keys that:
/// - Are empty
/// - Contain control characters (ASCII 0x00-0x1F, 0x7F)
/// - Contain the `:` delimiter (which is used internally to build composite keys)
/// - Exceed the maximum allowed length
///
/// # Errors
///
/// Returns [`ThrottleError::InvalidKey`] if validation fails.
///
/// # Examples
///
/// ```
/// use reinhardt_throttling::key_validation::validate_key_component;
///
/// assert!(validate_key_component("user123").is_ok());
/// assert!(validate_key_component("192.168.1.1").is_ok());
/// assert!(validate_key_component("").is_err());
/// assert!(validate_key_component("key\0value").is_err());
/// assert!(validate_key_component("scope:name").is_err());
/// ```
pub fn validate_key_component(key: &str) -> Result<(), ThrottleError> {
	if key.is_empty() {
		return Err(ThrottleError::InvalidKey(
			"key component must not be empty".to_string(),
		));
	}

	if key.len() > MAX_KEY_COMPONENT_LENGTH {
		return Err(ThrottleError::InvalidKey(format!(
			"key component exceeds maximum length of {} bytes",
			MAX_KEY_COMPONENT_LENGTH,
		)));
	}

	if key.contains(':') {
		return Err(ThrottleError::InvalidKey(
			"key component must not contain ':' delimiter".to_string(),
		));
	}

	if key.chars().any(|c| c.is_ascii_control()) {
		return Err(ThrottleError::InvalidKey(
			"key component must not contain control characters".to_string(),
		));
	}

	Ok(())
}

/// Validates a scoped throttle key in the format `"scope:identifier"`.
///
/// Splits the key on the first `:` and validates both the scope name and
/// the identifier component individually. Returns the validated scope and
/// identifier as a tuple on success.
///
/// # Errors
///
/// Returns [`ThrottleError::InvalidKey`] if:
/// - The key does not contain exactly one `:` separator at the top level
/// - Either the scope or identifier fails component validation
///
/// # Examples
///
/// ```
/// use reinhardt_throttling::key_validation::validate_scope_key;
///
/// let (scope, id) = validate_scope_key("api:user123").unwrap();
/// assert_eq!(scope, "api");
/// assert_eq!(id, "user123");
///
/// assert!(validate_scope_key("invalid_format").is_err());
/// assert!(validate_scope_key("scope:\0bad").is_err());
/// ```
pub fn validate_scope_key(key: &str) -> Result<(&str, &str), ThrottleError> {
	let Some((scope, identifier)) = key.split_once(':') else {
		return Err(ThrottleError::InvalidKey(
			"scoped key must be in 'scope:identifier' format".to_string(),
		));
	};

	validate_key_component(scope)?;
	validate_key_component(identifier)?;

	Ok((scope, identifier))
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	// -- validate_key_component tests --

	#[rstest]
	fn test_valid_alphanumeric_key() {
		// Arrange
		let key = "user123";

		// Act
		let result = validate_key_component(key);

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_valid_ip_address_key() {
		// Arrange
		let key = "192.168.1.1";

		// Act
		let result = validate_key_component(key);

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_valid_uuid_key() {
		// Arrange
		let key = "550e8400-e29b-41d4-a716-446655440000";

		// Act
		let result = validate_key_component(key);

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_valid_key_with_special_chars() {
		// Arrange
		let key = "user@example.com";

		// Act
		let result = validate_key_component(key);

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_reject_empty_key() {
		// Arrange
		let key = "";

		// Act
		let result = validate_key_component(key);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(matches!(err, ThrottleError::InvalidKey(_)));
		assert_eq!(
			err.to_string(),
			"Invalid key: key component must not be empty"
		);
	}

	#[rstest]
	fn test_reject_key_with_null_byte() {
		// Arrange
		let key = "user\0id";

		// Act
		let result = validate_key_component(key);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(matches!(err, ThrottleError::InvalidKey(_)));
		assert_eq!(
			err.to_string(),
			"Invalid key: key component must not contain control characters"
		);
	}

	#[rstest]
	fn test_reject_key_with_newline() {
		// Arrange
		let key = "user\nid";

		// Act
		let result = validate_key_component(key);

		// Assert
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), ThrottleError::InvalidKey(_)));
	}

	#[rstest]
	fn test_reject_key_with_carriage_return() {
		// Arrange
		let key = "user\rid";

		// Act
		let result = validate_key_component(key);

		// Assert
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), ThrottleError::InvalidKey(_)));
	}

	#[rstest]
	fn test_reject_key_with_tab() {
		// Arrange
		let key = "user\tid";

		// Act
		let result = validate_key_component(key);

		// Assert
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), ThrottleError::InvalidKey(_)));
	}

	#[rstest]
	fn test_reject_key_with_colon_delimiter() {
		// Arrange
		let key = "scope:name";

		// Act
		let result = validate_key_component(key);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(matches!(err, ThrottleError::InvalidKey(_)));
		assert_eq!(
			err.to_string(),
			"Invalid key: key component must not contain ':' delimiter"
		);
	}

	#[rstest]
	fn test_reject_key_exceeding_max_length() {
		// Arrange
		let key = "a".repeat(MAX_KEY_COMPONENT_LENGTH + 1);

		// Act
		let result = validate_key_component(&key);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(matches!(err, ThrottleError::InvalidKey(_)));
		assert_eq!(
			err.to_string(),
			format!(
				"Invalid key: key component exceeds maximum length of {} bytes",
				MAX_KEY_COMPONENT_LENGTH,
			)
		);
	}

	#[rstest]
	fn test_accept_key_at_max_length() {
		// Arrange
		let key = "a".repeat(MAX_KEY_COMPONENT_LENGTH);

		// Act
		let result = validate_key_component(&key);

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_reject_key_with_escape_sequence() {
		// Arrange
		let key = "user\x1bid"; // ESC character

		// Act
		let result = validate_key_component(key);

		// Assert
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), ThrottleError::InvalidKey(_)));
	}

	#[rstest]
	fn test_reject_key_with_del_character() {
		// Arrange
		let key = "user\x7fid"; // DEL character

		// Act
		let result = validate_key_component(key);

		// Assert
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), ThrottleError::InvalidKey(_)));
	}

	#[rstest]
	fn test_valid_unicode_key() {
		// Arrange
		let key = "user_\u{00E9}"; // e with acute accent (non-control unicode)

		// Act
		let result = validate_key_component(key);

		// Assert
		assert!(result.is_ok());
	}

	// -- validate_scope_key tests --

	#[rstest]
	fn test_valid_scope_key() {
		// Arrange
		let key = "api:user123";

		// Act
		let result = validate_scope_key(key);

		// Assert
		assert!(result.is_ok());
		let (scope, id) = result.unwrap();
		assert_eq!(scope, "api");
		assert_eq!(id, "user123");
	}

	#[rstest]
	fn test_reject_scope_key_without_delimiter() {
		// Arrange
		let key = "invalid_format";

		// Act
		let result = validate_scope_key(key);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(matches!(err, ThrottleError::InvalidKey(_)));
		assert_eq!(
			err.to_string(),
			"Invalid key: scoped key must be in 'scope:identifier' format"
		);
	}

	#[rstest]
	fn test_reject_scope_key_with_empty_scope() {
		// Arrange
		let key = ":user123";

		// Act
		let result = validate_scope_key(key);

		// Assert
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), ThrottleError::InvalidKey(_)));
	}

	#[rstest]
	fn test_reject_scope_key_with_empty_identifier() {
		// Arrange
		let key = "api:";

		// Act
		let result = validate_scope_key(key);

		// Assert
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), ThrottleError::InvalidKey(_)));
	}

	#[rstest]
	fn test_reject_scope_key_with_control_chars_in_scope() {
		// Arrange
		let key = "api\0:user123";

		// Act
		let result = validate_scope_key(key);

		// Assert
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), ThrottleError::InvalidKey(_)));
	}

	#[rstest]
	fn test_reject_scope_key_with_control_chars_in_identifier() {
		// Arrange
		let key = "api:user\x00abc";

		// Act
		let result = validate_scope_key(key);

		// Assert
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), ThrottleError::InvalidKey(_)));
	}

	#[rstest]
	fn test_reject_scope_key_with_extra_colons_in_identifier() {
		// Arrange - the identifier part contains a colon, which is rejected
		let key = "api:scope:user123";

		// Act
		let result = validate_scope_key(key);

		// Assert - split_once splits on first `:`, so identifier is "scope:user123"
		// which contains `:` and should be rejected
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), ThrottleError::InvalidKey(_)));
	}

	#[rstest]
	fn test_reject_scope_key_crafted_for_collision() {
		// Arrange - attacker tries to craft a key that collides with another scope's key
		// by including the internal key format pattern
		let key = "api:throttle:scope:admin:user1";

		// Act
		let result = validate_scope_key(key);

		// Assert - identifier contains `:`, rejected
		assert!(result.is_err());
	}

	#[rstest]
	fn test_scope_key_with_long_scope() {
		// Arrange
		let scope = "a".repeat(MAX_KEY_COMPONENT_LENGTH + 1);
		let key = format!("{}:user123", scope);

		// Act
		let result = validate_scope_key(&key);

		// Assert
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), ThrottleError::InvalidKey(_)));
	}

	#[rstest]
	fn test_scope_key_with_long_identifier() {
		// Arrange
		let identifier = "a".repeat(MAX_KEY_COMPONENT_LENGTH + 1);
		let key = format!("api:{}", identifier);

		// Act
		let result = validate_scope_key(&key);

		// Assert
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), ThrottleError::InvalidKey(_)));
	}
}

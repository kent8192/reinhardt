//! Idempotency key generation and management.

use uuid::Uuid;

/// Idempotency key generator.
///
/// Generates UUID v4 based idempotency keys for safe payment retry.
///
/// # Example
///
/// ```rust
/// use reinhardt_payment::idempotency::IdempotencyKeyGenerator;
///
/// let key = IdempotencyKeyGenerator::generate();
/// assert_eq!(key.len(), 36); // UUID format: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
/// ```
pub struct IdempotencyKeyGenerator;

impl IdempotencyKeyGenerator {
	/// Generates a new idempotency key.
	///
	/// # Returns
	///
	/// A UUID v4 string suitable for use as an idempotency key.
	pub fn generate() -> String {
		Uuid::new_v4().to_string()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_generate_idempotency_key() {
		let key1 = IdempotencyKeyGenerator::generate();
		let key2 = IdempotencyKeyGenerator::generate();

		// Keys should be valid UUIDs
		assert_eq!(key1.len(), 36);
		assert_eq!(key2.len(), 36);

		// Keys should be unique
		assert_ne!(key1, key2);
	}

	#[test]
	fn test_key_format() {
		let key = IdempotencyKeyGenerator::generate();
		// UUID v4 format: xxxxxxxx-xxxx-4xxx-xxxx-xxxxxxxxxxxx
		let parts: Vec<&str> = key.split('-').collect();
		assert_eq!(parts.len(), 5);
		assert_eq!(parts[0].len(), 8);
		assert_eq!(parts[1].len(), 4);
		assert_eq!(parts[2].len(), 4);
		assert_eq!(parts[3].len(), 4);
		assert_eq!(parts[4].len(), 12);
	}
}

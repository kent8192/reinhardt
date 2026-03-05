//! Test helper functions.

use hmac::{Hmac, Mac};
use reinhardt_payment::PaymentIntentStatus;
use sha2::Sha256;

/// Creates a signed webhook payload.
///
/// # Arguments
///
/// * `payload` - The webhook payload as a JSON string
/// * `secret` - The webhook signing secret
///
/// # Returns
///
/// A tuple of (payload bytes, signature header string)
pub fn create_signed_webhook(payload: &str, secret: &str) -> (Vec<u8>, String) {
	let timestamp = chrono::Utc::now().timestamp().to_string();
	let signed_payload = format!("{}.{}", timestamp, payload);

	let signature = compute_hmac_sha256(&signed_payload, secret);

	(
		payload.as_bytes().to_vec(),
		format!("t={},v1={}", timestamp, signature),
	)
}

/// Computes HMAC-SHA256 signature.
///
/// # Arguments
///
/// * `data` - The data to sign
/// * `secret` - The signing secret
///
/// # Returns
///
/// Hex-encoded signature string
pub fn compute_hmac_sha256(data: &str, secret: &str) -> String {
	let mut mac =
		Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("Invalid secret key length");
	mac.update(data.as_bytes());
	hex::encode(mac.finalize().into_bytes())
}

/// Creates a timestamp offset by specified seconds.
///
/// # Arguments
///
/// * `seconds_ago` - Number of seconds ago (positive) or in future (negative)
///
/// # Returns
///
/// Timestamp string
pub fn create_timestamp_offset(seconds_ago: i64) -> String {
	(chrono::Utc::now().timestamp() - seconds_ago).to_string()
}

/// Verifies payment intent status transition is valid.
///
/// # Arguments
///
/// * `from` - The current status
/// * `to` - The target status
///
/// # Panics
///
/// Panics if the transition is invalid
pub fn assert_valid_payment_intent_transition(from: PaymentIntentStatus, to: PaymentIntentStatus) {
	let valid = matches!(
		(from, to),
		(
			PaymentIntentStatus::RequiresPaymentMethod,
			PaymentIntentStatus::RequiresConfirmation
		) | (
			PaymentIntentStatus::RequiresPaymentMethod,
			PaymentIntentStatus::RequiresAction
		) | (
			PaymentIntentStatus::RequiresConfirmation,
			PaymentIntentStatus::RequiresAction
		) | (
			PaymentIntentStatus::RequiresConfirmation,
			PaymentIntentStatus::Processing
		) | (
			PaymentIntentStatus::RequiresConfirmation,
			PaymentIntentStatus::Succeeded
		) | (
			PaymentIntentStatus::RequiresAction,
			PaymentIntentStatus::Processing
		) | (
			PaymentIntentStatus::Processing,
			PaymentIntentStatus::Succeeded
		) | (
			PaymentIntentStatus::RequiresCapture,
			PaymentIntentStatus::Succeeded
		) | (_, PaymentIntentStatus::Canceled)
	);

	assert!(valid, "Invalid status transition: {:?} -> {:?}", from, to);
}

/// Asserts error message contains expected text.
///
/// # Arguments
///
/// * `error` - The error message to check
/// * `expected` - The expected substring
///
/// # Panics
///
/// Panics if the error message doesn't contain the expected text
pub fn assert_error_message_contains(error: &str, expected: &str) {
	assert!(
		error.contains(expected),
		"Expected error to contain '{}', got: '{}'",
		expected,
		error
	);
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_create_signed_webhook() {
		let payload = r#"{"type":"payment_succeeded"}"#;
		let (bytes, signature) = create_signed_webhook(payload, "test_secret");

		assert!(!bytes.is_empty());
		assert!(signature.starts_with("t="));
		assert!(signature.contains(",v1="));
	}

	#[test]
	fn test_compute_hmac_sha256() {
		let signature = compute_hmac_sha256("test_data", "test_secret");
		assert_eq!(signature.len(), 64); // SHA256 produces 32 bytes = 64 hex chars
	}

	#[test]
	fn test_create_timestamp_offset() {
		let past = create_timestamp_offset(300); // 5 minutes ago
		let future = create_timestamp_offset(-300); // 5 minutes in future

		assert!(!past.is_empty());
		assert!(!future.is_empty());
	}

	#[test]
	fn test_assert_valid_payment_intent_transition() {
		// Valid transitions
		assert_valid_payment_intent_transition(
			PaymentIntentStatus::RequiresPaymentMethod,
			PaymentIntentStatus::RequiresConfirmation,
		);
		assert_valid_payment_intent_transition(
			PaymentIntentStatus::RequiresConfirmation,
			PaymentIntentStatus::Succeeded,
		);
		assert_valid_payment_intent_transition(
			PaymentIntentStatus::Processing,
			PaymentIntentStatus::Succeeded,
		);

		// Invalid transition should panic
		let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
			assert_valid_payment_intent_transition(
				PaymentIntentStatus::Succeeded,
				PaymentIntentStatus::RequiresConfirmation,
			);
		}));
		assert!(result.is_err());
	}

	#[test]
	fn test_assert_error_message_contains() {
		assert_error_message_contains("Error: Invalid parameters", "Invalid parameters");
	}

	#[test]
	#[should_panic(expected = "Expected error to contain")]
	fn test_assert_error_message_contains_panics() {
		assert_error_message_contains("Error: Invalid parameters", "not found");
	}
}

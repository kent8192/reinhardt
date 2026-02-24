//! Cursor encoding strategies for cursor-based pagination

use crate::exception::{Error, Result};
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Trait for encoding and decoding pagination cursors
///
/// Custom implementations can provide different encoding strategies,
/// such as encryption, compression, or signed tokens.
///
/// # Examples
///
/// ```
/// use reinhardt_core::pagination::cursor::{CursorEncoder, Base64CursorEncoder};
///
/// let encoder = Base64CursorEncoder::new();
/// let cursor = encoder.encode(42).unwrap();
/// let position = encoder.decode(&cursor).unwrap();
/// assert_eq!(position, 42);
/// ```
pub trait CursorEncoder: Send + Sync {
	/// Encode a position into a cursor string
	fn encode(&self, position: usize) -> Result<String>;

	/// Decode a cursor string back to a position
	fn decode(&self, cursor: &str) -> Result<usize>;
}

/// Base64 cursor encoder with timestamp and HMAC-SHA256 integrity validation
///
/// Encodes cursors as base64(position:timestamp:hmac) to prevent tampering
/// and provide automatic expiration. Uses HMAC-SHA256 with a secret key for
/// cryptographically secure integrity verification.
///
/// # Examples
///
/// ```
/// use reinhardt_core::pagination::cursor::{CursorEncoder, Base64CursorEncoder};
///
/// let encoder = Base64CursorEncoder::new();
/// let cursor = encoder.encode(100).unwrap();
///
/// // Cursor can be decoded back
/// let position = encoder.decode(&cursor).unwrap();
/// assert_eq!(position, 100);
/// ```
#[derive(Debug, Clone)]
pub struct Base64CursorEncoder {
	/// Cursor expiry time in seconds (default: 24 hours)
	pub expiry_seconds: u64,
	/// Secret key for HMAC-SHA256 integrity validation
	secret_key: Vec<u8>,
}

impl Base64CursorEncoder {
	/// Create a new Base64 cursor encoder with default expiry (24 hours)
	/// and a randomly generated secret key
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::cursor::Base64CursorEncoder;
	///
	/// let encoder = Base64CursorEncoder::new();
	/// assert_eq!(encoder.expiry_seconds, 86400);
	/// ```
	pub fn new() -> Self {
		use rand::RngCore;
		let mut key = vec![0u8; 32];
		rand::rng().fill_bytes(&mut key);
		Self {
			expiry_seconds: 86400, // 24 hours
			secret_key: key,
		}
	}

	/// Create a new Base64 cursor encoder with a specific secret key
	///
	/// The key should be at least 32 bytes for adequate security.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::cursor::Base64CursorEncoder;
	///
	/// let key = b"my-secret-key-at-least-32-bytes!";
	/// let encoder = Base64CursorEncoder::with_secret_key(key);
	/// assert_eq!(encoder.expiry_seconds, 86400);
	/// ```
	pub fn with_secret_key(key: &[u8]) -> Self {
		Self {
			expiry_seconds: 86400,
			secret_key: key.to_vec(),
		}
	}

	/// Set custom expiry time in seconds
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::cursor::Base64CursorEncoder;
	///
	/// let encoder = Base64CursorEncoder::new().expiry_seconds(3600); // 1 hour
	/// assert_eq!(encoder.expiry_seconds, 3600);
	/// ```
	pub fn expiry_seconds(mut self, seconds: u64) -> Self {
		self.expiry_seconds = seconds;
		self
	}

	/// Compute HMAC-SHA256 over the given message using the encoder's secret key
	fn compute_hmac(&self, message: &[u8]) -> Vec<u8> {
		let mut mac =
			HmacSha256::new_from_slice(&self.secret_key).expect("HMAC accepts any key length");
		mac.update(message);
		mac.finalize().into_bytes().to_vec()
	}
}

impl Default for Base64CursorEncoder {
	fn default() -> Self {
		Self::new()
	}
}

impl CursorEncoder for Base64CursorEncoder {
	fn encode(&self, position: usize) -> Result<String> {
		use base64::{Engine as _, engine::general_purpose};

		let timestamp = std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.unwrap()
			.as_secs();

		// Build message payload for HMAC signing
		let payload = format!("{}:{}", position, timestamp);
		let hmac_bytes = self.compute_hmac(payload.as_bytes());
		let hmac_hex = hex::encode(&hmac_bytes);

		let cursor_data = format!("{}:{}:{}", position, timestamp, hmac_hex);
		Ok(general_purpose::URL_SAFE_NO_PAD.encode(cursor_data.as_bytes()))
	}

	fn decode(&self, cursor: &str) -> Result<usize> {
		use base64::{Engine as _, engine::general_purpose};

		let decoded = general_purpose::URL_SAFE_NO_PAD
			.decode(cursor)
			.map_err(|_| Error::InvalidPage("Invalid cursor".to_string()))?;
		let cursor_data = String::from_utf8(decoded)
			.map_err(|_| Error::InvalidPage("Invalid cursor encoding".to_string()))?;

		// Parse cursor components: position:timestamp:hmac_hex
		let parts: Vec<&str> = cursor_data.splitn(3, ':').collect();
		if parts.len() != 3 {
			return Err(Error::InvalidPage("Malformed cursor".to_string()));
		}

		let position: usize = parts[0]
			.parse()
			.map_err(|_| Error::InvalidPage("Invalid cursor value".to_string()))?;
		let timestamp: u64 = parts[1]
			.parse()
			.map_err(|_| Error::InvalidPage("Invalid cursor timestamp".to_string()))?;
		let provided_hmac = hex::decode(parts[2])
			.map_err(|_| Error::InvalidPage("Invalid cursor signature".to_string()))?;

		// Verify HMAC-SHA256 signature
		let payload = format!("{}:{}", position, timestamp);
		let mut mac =
			HmacSha256::new_from_slice(&self.secret_key).expect("HMAC accepts any key length");
		mac.update(payload.as_bytes());
		mac.verify_slice(&provided_hmac)
			.map_err(|_| Error::InvalidPage("Cursor integrity check failed".to_string()))?;

		// Check if cursor is expired
		let now = std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.unwrap()
			.as_secs();
		// Use saturating_sub to prevent underflow when timestamp > now
		// (e.g., due to clock skew or NTP adjustments)
		if now.saturating_sub(timestamp) > self.expiry_seconds {
			return Err(Error::Validation("Cursor expired".to_string()));
		}

		Ok(position)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use base64::Engine as _;
	use rstest::rstest;

	#[rstest]
	fn test_base64_encoder_encode_decode() {
		// Arrange
		let encoder = Base64CursorEncoder::with_secret_key(b"test-secret-key-for-unit-tests!!");
		let position = 42;

		// Act
		let cursor = encoder.encode(position).unwrap();
		let decoded = encoder.decode(&cursor).unwrap();

		// Assert
		assert_eq!(decoded, position);
	}

	#[rstest]
	fn test_base64_encoder_invalid_cursor() {
		// Arrange
		let encoder = Base64CursorEncoder::with_secret_key(b"test-secret-key-for-unit-tests!!");

		// Act
		let result = encoder.decode("not-valid-base64!!!");

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_base64_encoder_tampered_cursor() {
		// Arrange
		let encoder = Base64CursorEncoder::with_secret_key(b"test-secret-key-for-unit-tests!!");
		let cursor = encoder.encode(42).unwrap();

		// Act - tamper with the cursor by appending data
		let mut tampered = cursor.clone();
		tampered.push('X');
		let result = encoder.decode(&tampered);

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_base64_encoder_different_key_rejects_cursor() {
		// Arrange
		let encoder_a = Base64CursorEncoder::with_secret_key(b"secret-key-a-for-testing-only!!");
		let encoder_b = Base64CursorEncoder::with_secret_key(b"secret-key-b-for-testing-only!!");

		// Act - encode with key A, decode with key B
		let cursor = encoder_a.encode(42).unwrap();
		let result = encoder_b.decode(&cursor);

		// Assert
		assert!(result.is_err());
		if let Err(Error::InvalidPage(msg)) = result {
			assert_eq!(msg, "Cursor integrity check failed");
		} else {
			panic!("Expected InvalidPage error for key mismatch");
		}
	}

	#[rstest]
	fn test_base64_encoder_custom_expiry() {
		// Arrange
		let encoder = Base64CursorEncoder::with_secret_key(b"test-secret-key-for-unit-tests!!")
			.expiry_seconds(1);
		let cursor = encoder.encode(42).unwrap();

		// Act - wait for expiry
		std::thread::sleep(std::time::Duration::from_secs(2));
		let result = encoder.decode(&cursor);

		// Assert
		assert!(result.is_err());
		if let Err(Error::Validation(msg)) = result {
			assert_eq!(msg, "Cursor expired");
		} else {
			panic!("Expected Validation error");
		}
	}

	#[rstest]
	fn test_base64_encoder_with_secret_key() {
		// Arrange
		let key = b"my-secret-key-at-least-32-bytes!";
		let encoder = Base64CursorEncoder::with_secret_key(key);

		// Act
		let cursor = encoder.encode(100).unwrap();
		let decoded = encoder.decode(&cursor).unwrap();

		// Assert
		assert_eq!(decoded, 100);
	}

	#[rstest]
	fn test_base64_encoder_multiple_positions() {
		// Arrange
		let encoder = Base64CursorEncoder::with_secret_key(b"test-secret-key-for-unit-tests!!");

		// Act & Assert - verify multiple positions round-trip correctly
		for position in [0, 1, 100, 999, 10000, usize::MAX / 2] {
			let cursor = encoder.encode(position).unwrap();
			let decoded = encoder.decode(&cursor).unwrap();
			assert_eq!(decoded, position);
		}
	}

	#[rstest]
	fn test_base64_encoder_future_timestamp_no_underflow() {
		// Arrange
		// Simulate clock skew: cursor created on a server with a slightly ahead clock
		let encoder = Base64CursorEncoder::with_secret_key(b"test-secret-key-for-unit-tests!!");
		let position: usize = 42;
		let future_timestamp = std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.unwrap()
			.as_secs()
			+ 3600; // 1 hour in the future

		// Manually construct a valid cursor with a future timestamp
		let payload = format!("{}:{}", position, future_timestamp);
		let hmac_bytes = encoder.compute_hmac(payload.as_bytes());
		let hmac_hex = hex::encode(&hmac_bytes);
		let cursor_data = format!("{}:{}:{}", position, future_timestamp, hmac_hex);
		let cursor =
			base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(cursor_data.as_bytes());

		// Act
		let result = encoder.decode(&cursor);

		// Assert
		// With saturating_sub, future timestamps yield 0 elapsed time,
		// so the cursor should not be treated as expired
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), position);
	}
}

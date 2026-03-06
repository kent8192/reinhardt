//! Webhook signature verification.

use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

/// Parses Stripe-Signature header.
///
/// # Format
/// ```text
/// t=timestamp,v1=signature
/// ```
pub fn parse_signature_header(signature: &str) -> Result<(String, String), String> {
	let mut timestamp = String::new();
	let mut v1_signature = String::new();

	for part in signature.split(',') {
		let kv: Vec<&str> = part.split('=').collect();
		if kv.len() != 2 {
			continue;
		}

		match kv[0] {
			"t" => timestamp = kv[1].to_string(),
			"v1" => v1_signature = kv[1].to_string(),
			_ => {}
		}
	}

	if timestamp.is_empty() || v1_signature.is_empty() {
		return Err("Invalid signature header format".to_string());
	}

	Ok((timestamp, v1_signature))
}

/// Verifies webhook signature using HMAC-SHA256.
///
/// # Security
///
/// - Uses constant-time comparison to prevent timing attacks
/// - Validates timestamp to prevent replay attacks (5-minute tolerance)
///
/// # Arguments
///
/// * `payload` - Raw request body
/// * `signature` - Stripe-Signature header value
/// * `secret` - Webhook endpoint secret
pub fn verify_signature(payload: &[u8], signature: &str, secret: &str) -> Result<bool, String> {
	// Parse signature header
	let (timestamp, v1_sig) = parse_signature_header(signature)?;

	// Validate timestamp (5-minute tolerance)
	let timestamp_num: i64 = timestamp.parse().map_err(|_| "Invalid timestamp")?;
	let now = chrono::Utc::now().timestamp();
	let diff = (now - timestamp_num).abs();

	if diff > 300 {
		// 5 minutes
		return Err("Timestamp outside tolerance window".to_string());
	}

	// Construct signed payload: timestamp.payload
	let payload_str = std::str::from_utf8(payload).map_err(|_| "Invalid UTF-8 payload")?;
	let signed_payload = format!("{}.{}", timestamp, payload_str);

	// Compute HMAC-SHA256
	let mut mac =
		Hmac::<Sha256>::new_from_slice(secret.as_bytes()).map_err(|_| "Invalid secret key")?;
	mac.update(signed_payload.as_bytes());
	let expected = hex::encode(mac.finalize().into_bytes());

	// Constant-time comparison
	Ok(expected.as_bytes().ct_eq(v1_sig.as_bytes()).into())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_parse_signature_header() {
		let sig = "t=1609459200,v1=abcdef1234567890";
		let (timestamp, v1) = parse_signature_header(sig).unwrap();
		assert_eq!(timestamp, "1609459200");
		assert_eq!(v1, "abcdef1234567890");
	}

	#[test]
	fn test_parse_signature_header_invalid() {
		let sig = "invalid";
		assert!(parse_signature_header(sig).is_err());
	}
}

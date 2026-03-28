//! Sensitive data sanitization

/// List of sensitive header/cookie names (case-insensitive)
const SENSITIVE_KEYS: &[&str] = &[
	"password",
	"token",
	"secret",
	"api_key",
	"apikey",
	"api-key",
	"authorization",
	"auth",
	"session",
	"cookie",
	"csrf",
];

/// Sanitize header value if the key is sensitive
///
/// Returns a tuple of (key, value) where value is redacted if key is sensitive.
pub fn sanitize_headers(key: &str, value: &str) -> (String, String) {
	let key_lower = key.to_lowercase();
	let is_sensitive = SENSITIVE_KEYS.iter().any(|&s| key_lower.contains(s));

	if is_sensitive {
		(key.to_string(), "***REDACTED***".to_string())
	} else {
		(key.to_string(), value.to_string())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	#[case("Content-Type", "application/json", "application/json")]
	#[case("Authorization", "Bearer token123", "***REDACTED***")]
	#[case("X-API-Key", "secret-key", "***REDACTED***")]
	#[case("Session-Token", "session123", "***REDACTED***")]
	#[case("User-Agent", "Mozilla/5.0", "Mozilla/5.0")]
	fn test_sanitize_headers(#[case] key: &str, #[case] value: &str, #[case] expected_value: &str) {
		let (sanitized_key, sanitized_value) = sanitize_headers(key, value);
		assert_eq!(sanitized_key, key);
		assert_eq!(sanitized_value, expected_value);
	}
}

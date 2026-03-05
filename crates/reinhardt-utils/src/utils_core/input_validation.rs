//! Input validation and sanitization utilities
//!
//! Provides helpers for validating and sanitizing user input to prevent
//! common security vulnerabilities such as open redirects, log injection,
//! and identifier-based attacks.

/// Errors returned by [`validate_identifier`].
#[derive(Debug, thiserror::Error)]
pub enum IdentifierError {
	#[error("Identifier is empty")]
	Empty,
	#[error("Identifier exceeds maximum length of {max_length} characters")]
	TooLong { max_length: usize },
	#[error("Identifier contains invalid character: '{ch}'")]
	InvalidCharacter { ch: char },
	#[error("Identifier must start with alphanumeric or underscore, got: '{ch}'")]
	InvalidStartCharacter { ch: char },
}

/// Validates a URL for safe redirect usage.
///
/// Allows:
/// - Relative paths starting with `/` (absolute paths on same origin)
/// - Same-origin relative paths starting with `./`
/// - Anchor links starting with `#`
/// - `http://` and `https://` URLs
///
/// Rejects:
/// - Path traversal (`../`)
/// - Dangerous protocols (`javascript:`, `data:`, `vbscript:`)
/// - Unknown URL schemes
/// - URLs with embedded credentials (`http://user:pass@host`)
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::input_validation::validate_redirect_url;
///
/// assert!(validate_redirect_url("/dashboard"));
/// assert!(validate_redirect_url("https://example.com/page"));
/// assert!(!validate_redirect_url("javascript:alert(1)"));
/// assert!(!validate_redirect_url("../secret"));
/// ```
pub fn validate_redirect_url(url: &str) -> bool {
	let trimmed = url.trim();

	if trimmed.is_empty() {
		return false;
	}

	// Reject path traversal
	if trimmed.starts_with("../") || trimmed.contains("/../") || trimmed.ends_with("/..") {
		return false;
	}

	// Allow anchor links
	if trimmed.starts_with('#') {
		return true;
	}

	// Allow same-origin relative paths
	if trimmed.starts_with("./") {
		return true;
	}

	// Allow absolute paths on same origin (must start with single /)
	// Reject protocol-relative URLs (//) to prevent open redirect
	if trimmed.starts_with('/') {
		return !trimmed.starts_with("//");
	}

	let lower = trimmed.to_lowercase();

	// Reject dangerous protocols
	let dangerous_protocols = ["javascript:", "data:", "vbscript:"];
	for proto in &dangerous_protocols {
		if lower.starts_with(proto) {
			return false;
		}
	}

	// Allow only http:// and https://
	if lower.starts_with("http://") || lower.starts_with("https://") {
		// Reject URLs with embedded credentials (user:pass@host)
		let after_scheme = if lower.starts_with("https://") {
			&trimmed[8..]
		} else {
			&trimmed[7..]
		};

		// Check for @ before the first / (indicates credentials)
		if let Some(path_start) = after_scheme.find('/') {
			let authority = &after_scheme[..path_start];
			if authority.contains('@') {
				return false;
			}
		} else if after_scheme.contains('@') {
			return false;
		}

		return true;
	}

	// Reject all other schemes / unknown formats
	false
}

/// Sanitizes user input for safe inclusion in log messages.
///
/// Replaces control characters, newlines, and other characters
/// that could be used for log injection attacks. Truncates
/// the result to `max_length` characters.
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::input_validation::sanitize_log_input;
///
/// let input = "normal text\ninjected line";
/// let sanitized = sanitize_log_input(input, 100);
/// assert!(!sanitized.contains('\n'));
/// ```
pub fn sanitize_log_input(input: &str, max_length: usize) -> String {
	let mut result = String::with_capacity(input.len().min(max_length));

	for (char_count, ch) in input.chars().enumerate() {
		if char_count >= max_length {
			break;
		}

		match ch {
			// Replace newlines and carriage returns with spaces
			'\n' | '\r' => result.push(' '),
			// Replace tabs with spaces
			'\t' => result.push(' '),
			// Replace other control characters with Unicode replacement character
			c if c.is_control() => result.push('\u{FFFD}'),
			// Keep printable characters as-is
			c => result.push(c),
		}
	}

	result
}

/// Validates that a string is a safe identifier.
///
/// Allows: ASCII alphanumeric, hyphens, underscores.
/// First character must be alphanumeric or underscore.
/// Max length is enforced.
///
/// # Errors
///
/// Returns [`IdentifierError`] if the identifier is empty, too long,
/// starts with an invalid character, or contains invalid characters.
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::input_validation::validate_identifier;
///
/// assert!(validate_identifier("my-plugin", 64).is_ok());
/// assert!(validate_identifier("_internal", 64).is_ok());
/// assert!(validate_identifier("", 64).is_err());
/// assert!(validate_identifier("-invalid", 64).is_err());
/// ```
pub fn validate_identifier(input: &str, max_length: usize) -> Result<(), IdentifierError> {
	if input.is_empty() {
		return Err(IdentifierError::Empty);
	}

	if input.len() > max_length {
		return Err(IdentifierError::TooLong { max_length });
	}

	// First character must be alphanumeric or underscore
	let first = input.chars().next().expect("non-empty string");
	if !first.is_ascii_alphanumeric() && first != '_' {
		return Err(IdentifierError::InvalidStartCharacter { ch: first });
	}

	// Remaining characters: alphanumeric, hyphens, underscores
	for ch in input.chars() {
		if !ch.is_ascii_alphanumeric() && ch != '-' && ch != '_' {
			return Err(IdentifierError::InvalidCharacter { ch });
		}
	}

	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	// ===================================================================
	// validate_redirect_url tests
	// ===================================================================

	#[rstest]
	#[case("/dashboard", true)]
	#[case("/path/to/page", true)]
	#[case("./relative", true)]
	#[case("#section", true)]
	#[case("#", true)]
	#[case("https://example.com", true)]
	#[case("http://example.com/page", true)]
	#[case("https://example.com/path?q=1", true)]
	fn test_validate_redirect_url_allows_safe_urls(#[case] url: &str, #[case] expected: bool) {
		// Act
		let result = validate_redirect_url(url);

		// Assert
		assert_eq!(result, expected, "URL {:?} should be allowed", url);
	}

	#[rstest]
	#[case("javascript:alert(1)", false)]
	#[case("JAVASCRIPT:alert(1)", false)]
	#[case("data:text/html,<script>", false)]
	#[case("vbscript:msgbox", false)]
	#[case("../secret", false)]
	#[case("/path/../secret", false)]
	#[case("/path/..", false)]
	#[case("//evil.com", false)]
	#[case("", false)]
	#[case("   ", false)]
	#[case("ftp://files.example.com", false)]
	#[case("http://user:pass@host.com", false)]
	#[case("https://admin:secret@host.com/path", false)]
	fn test_validate_redirect_url_rejects_unsafe_urls(#[case] url: &str, #[case] expected: bool) {
		// Act
		let result = validate_redirect_url(url);

		// Assert
		assert_eq!(result, expected, "URL {:?} should be rejected", url);
	}

	#[rstest]
	fn test_validate_redirect_url_trims_whitespace() {
		// Arrange
		let url = "  /dashboard  ";

		// Act
		let result = validate_redirect_url(url);

		// Assert
		assert!(result);
	}

	// ===================================================================
	// sanitize_log_input tests
	// ===================================================================

	#[rstest]
	fn test_sanitize_log_input_replaces_newlines() {
		// Arrange
		let input = "line1\nline2\rline3\r\nline4";

		// Act
		let result = sanitize_log_input(input, 100);

		// Assert
		assert_eq!(result, "line1 line2 line3  line4");
	}

	#[rstest]
	fn test_sanitize_log_input_replaces_tabs() {
		// Arrange
		let input = "col1\tcol2\tcol3";

		// Act
		let result = sanitize_log_input(input, 100);

		// Assert
		assert_eq!(result, "col1 col2 col3");
	}

	#[rstest]
	fn test_sanitize_log_input_replaces_control_characters() {
		// Arrange
		let input = "before\x00\x01\x07after";

		// Act
		let result = sanitize_log_input(input, 100);

		// Assert
		assert_eq!(result, "before\u{FFFD}\u{FFFD}\u{FFFD}after");
	}

	#[rstest]
	fn test_sanitize_log_input_truncates_to_max_length() {
		// Arrange
		let input = "a".repeat(200);

		// Act
		let result = sanitize_log_input(&input, 50);

		// Assert
		assert_eq!(result.len(), 50);
	}

	#[rstest]
	fn test_sanitize_log_input_preserves_normal_text() {
		// Arrange
		let input = "Hello, World! 123 @#$";

		// Act
		let result = sanitize_log_input(input, 100);

		// Assert
		assert_eq!(result, input);
	}

	#[rstest]
	fn test_sanitize_log_input_empty_input() {
		// Act
		let result = sanitize_log_input("", 100);

		// Assert
		assert_eq!(result, "");
	}

	#[rstest]
	fn test_sanitize_log_input_zero_max_length() {
		// Act
		let result = sanitize_log_input("some text", 0);

		// Assert
		assert_eq!(result, "");
	}

	// ===================================================================
	// validate_identifier tests
	// ===================================================================

	#[rstest]
	#[case("my-plugin", 64)]
	#[case("MyPlugin", 64)]
	#[case("plugin_v2", 64)]
	#[case("_internal", 64)]
	#[case("a", 64)]
	#[case("A123-test_name", 64)]
	fn test_validate_identifier_accepts_valid(#[case] input: &str, #[case] max_len: usize) {
		// Act
		let result = validate_identifier(input, max_len);

		// Assert
		assert!(result.is_ok(), "Identifier {:?} should be valid", input);
	}

	#[rstest]
	fn test_validate_identifier_rejects_empty() {
		// Act
		let result = validate_identifier("", 64);

		// Assert
		assert!(matches!(result, Err(IdentifierError::Empty)));
	}

	#[rstest]
	fn test_validate_identifier_rejects_too_long() {
		// Arrange
		let input = "a".repeat(65);

		// Act
		let result = validate_identifier(&input, 64);

		// Assert
		assert!(matches!(
			result,
			Err(IdentifierError::TooLong { max_length: 64 })
		));
	}

	#[rstest]
	#[case("-starts-with-hyphen")]
	fn test_validate_identifier_rejects_invalid_start(#[case] input: &str) {
		// Act
		let result = validate_identifier(input, 64);

		// Assert
		assert!(matches!(
			result,
			Err(IdentifierError::InvalidStartCharacter { .. })
		));
	}

	#[rstest]
	#[case("has space", ' ')]
	#[case("has.dot", '.')]
	#[case("has/slash", '/')]
	#[case("has@at", '@')]
	fn test_validate_identifier_rejects_invalid_characters(
		#[case] input: &str,
		#[case] expected_ch: char,
	) {
		// Act
		let result = validate_identifier(input, 64);

		// Assert
		match result {
			Err(IdentifierError::InvalidCharacter { ch }) => {
				assert_eq!(ch, expected_ch);
			}
			other => panic!("Expected InvalidCharacter, got {:?}", other),
		}
	}

	// ===================================================================
	// IdentifierError Display tests
	// ===================================================================

	#[rstest]
	fn test_sanitize_log_input_multibyte_truncation_does_not_panic() {
		// Fixes #762: Use character count instead of byte length for truncation
		// to prevent cutting in the middle of multi-byte UTF-8 characters.
		let input = "あいうえおかきくけこ"; // 10 chars, 30 bytes

		// Act
		let result = sanitize_log_input(input, 5);

		// Assert
		assert_eq!(result.chars().count(), 5);
		assert_eq!(result, "あいうえお");
	}

	#[rstest]
	fn test_sanitize_log_input_mixed_multibyte_truncation() {
		// Fixes #762: Mixed ASCII and multibyte characters
		let input = "aあbいcうdえeお";

		// Act
		let result = sanitize_log_input(input, 6);

		// Assert
		assert_eq!(result.chars().count(), 6);
		assert_eq!(result, "aあbいcう");
	}

	#[rstest]
	fn test_identifier_error_display_messages() {
		// Assert
		assert_eq!(IdentifierError::Empty.to_string(), "Identifier is empty");
		assert_eq!(
			IdentifierError::TooLong { max_length: 32 }.to_string(),
			"Identifier exceeds maximum length of 32 characters"
		);
		assert_eq!(
			IdentifierError::InvalidCharacter { ch: '@' }.to_string(),
			"Identifier contains invalid character: '@'"
		);
		assert_eq!(
			IdentifierError::InvalidStartCharacter { ch: '-' }.to_string(),
			"Identifier must start with alphanumeric or underscore, got: '-'"
		);
	}
}

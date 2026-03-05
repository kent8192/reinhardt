//! Integration tests for reinhardt-core security module
//!
//! Tests cover CSRF token generation/verification, XSS escaping/sanitization,
//! redirect URL validation, and SecurityError variants.

use std::collections::HashSet;

use reinhardt_core::security::SecurityError;
use reinhardt_core::security::csrf::{
	CSRF_SECRET_LENGTH, CSRF_TOKEN_LENGTH, CsrfConfig, CsrfMiddleware, CsrfToken, check_token_hmac,
	generate_token_hmac, get_secret_bytes, get_token_hmac, verify_token_hmac,
};
use reinhardt_core::security::redirect::{
	RedirectValidationError, is_safe_redirect, validate_redirect_url,
};
use reinhardt_core::security::xss::{
	escape_css_selector, escape_html, escape_html_attr, escape_html_content, escape_javascript,
	sanitize_html, strip_tags_safe, validate_css_selector,
};
use rstest::rstest;

// ============================================================================
// CSRF token generation/verification roundtrip
// ============================================================================

#[rstest]
fn csrf_generate_and_verify_roundtrip() {
	// Arrange
	let secret = b"test-secret-key-for-csrf-testing";
	let message = "session-id-12345";

	// Act
	let token = generate_token_hmac(secret, message);
	let verified = verify_token_hmac(&token, secret, message);

	// Assert
	assert!(
		verified,
		"Token generated for a message should verify against the same secret and message"
	);
}

#[rstest]
fn csrf_different_messages_produce_different_tokens() {
	// Arrange
	let secret = b"shared-secret-key-for-csrf-test";
	let message_a = "session-alpha";
	let message_b = "session-beta";

	// Act
	let token_a = generate_token_hmac(secret, message_a);
	let token_b = generate_token_hmac(secret, message_b);

	// Assert
	assert_ne!(
		token_a, token_b,
		"Tokens for different messages must be different"
	);
}

#[rstest]
fn csrf_wrong_secret_fails_verification() {
	// Arrange
	let correct_secret = b"correct-secret-key-for-testing!";
	let wrong_secret = b"wrong-secret-key-will-not-work!";
	let message = "session-xyz";

	// Act
	let token = generate_token_hmac(correct_secret, message);
	let verified = verify_token_hmac(&token, wrong_secret, message);

	// Assert
	assert!(!verified, "Token must not verify with a different secret");
}

#[rstest]
fn csrf_wrong_message_fails_verification() {
	// Arrange
	let secret = b"test-secret-key-for-csrf-verify";
	let original_message = "original-session";
	let tampered_message = "tampered-session";

	// Act
	let token = generate_token_hmac(secret, original_message);
	let verified = verify_token_hmac(&token, secret, tampered_message);

	// Assert
	assert!(
		!verified,
		"Token must not verify against a different message"
	);
}

// ============================================================================
// CSRF: get_secret_bytes and get_token_hmac lengths
// ============================================================================

#[rstest]
fn csrf_get_secret_bytes_returns_correct_length() {
	// Act
	let secret = get_secret_bytes();

	// Assert
	assert_eq!(
		secret.len(),
		CSRF_SECRET_LENGTH,
		"Secret bytes must be {CSRF_SECRET_LENGTH} bytes long"
	);
}

#[rstest]
fn csrf_get_secret_bytes_is_random() {
	// Act
	let secret_a = get_secret_bytes();
	let secret_b = get_secret_bytes();

	// Assert
	assert_ne!(
		secret_a, secret_b,
		"Successive calls to get_secret_bytes should produce different values"
	);
}

#[rstest]
fn csrf_get_token_hmac_returns_correct_length() {
	// Arrange
	let secret = get_secret_bytes();
	let session_id = "test-session-id";

	// Act
	let token = get_token_hmac(&secret, session_id);

	// Assert
	assert_eq!(
		token.len(),
		CSRF_TOKEN_LENGTH,
		"HMAC token must be {CSRF_TOKEN_LENGTH} characters long"
	);
}

#[rstest]
fn csrf_generate_token_hmac_returns_hex_string() {
	// Arrange
	let secret = b"hex-format-test-secret-key-here";
	let message = "session-for-hex-check";

	// Act
	let token = generate_token_hmac(secret, message);

	// Assert
	assert_eq!(token.len(), CSRF_TOKEN_LENGTH);
	assert!(
		token.chars().all(|c| c.is_ascii_hexdigit()),
		"Token must consist of hexadecimal characters only, got: {token}"
	);
}

#[rstest]
fn csrf_check_token_hmac_accepts_valid_token() {
	// Arrange
	let secret = get_secret_bytes();
	let session_id = "valid-session";
	let token = get_token_hmac(&secret, session_id);

	// Act
	let result = check_token_hmac(&token, &secret, session_id);

	// Assert
	assert!(result.is_ok(), "Valid token should pass check_token_hmac");
}

#[rstest]
fn csrf_check_token_hmac_rejects_invalid_token() {
	// Arrange
	let secret = get_secret_bytes();
	let session_id = "test-session";
	let invalid_token = "0".repeat(CSRF_TOKEN_LENGTH);

	// Act
	let result = check_token_hmac(&invalid_token, &secret, session_id);

	// Assert
	assert!(
		result.is_err(),
		"Invalid token should be rejected by check_token_hmac"
	);
}

// ============================================================================
// CsrfConfig: default vs production
// ============================================================================

#[rstest]
fn csrf_config_default_settings() {
	// Act
	let config = CsrfConfig::default();

	// Assert
	assert_eq!(config.cookie_name, "csrftoken");
	assert_eq!(config.header_name, "X-CSRFToken");
	assert!(
		!config.cookie_httponly,
		"Default config should not set HttpOnly (JS needs access)"
	);
	assert!(
		!config.cookie_secure,
		"Default config should not require HTTPS"
	);
	assert!(
		!config.enable_token_rotation,
		"Default config should not enable rotation"
	);
}

#[rstest]
fn csrf_config_production_settings() {
	// Act
	let config = CsrfConfig::production();

	// Assert
	assert_eq!(config.cookie_name, "csrftoken");
	assert_eq!(config.header_name, "X-CSRFToken");
	assert!(config.cookie_secure, "Production config must require HTTPS");
	assert!(
		config.enable_token_rotation,
		"Production config must enable token rotation"
	);
	assert!(
		config.cookie_max_age.is_some(),
		"Production config should set max age"
	);
	assert!(
		config.token_rotation_interval.is_some(),
		"Production config should set rotation interval"
	);
}

#[rstest]
fn csrf_config_with_token_rotation() {
	// Arrange
	let interval = 1800u64;

	// Act
	let config = CsrfConfig::default().with_token_rotation(Some(interval));

	// Assert
	assert!(config.enable_token_rotation);
	assert_eq!(config.token_rotation_interval, Some(interval));
}

#[rstest]
fn csrf_token_struct_roundtrip() {
	// Arrange
	let raw: String = "abcdef1234567890".into();

	// Act
	let token = CsrfToken::new(raw.clone());

	// Assert
	assert_eq!(token.as_str(), raw);
}

#[rstest]
fn csrf_middleware_can_be_created() {
	// Arrange
	let production_config = CsrfConfig::production();

	// Verify production config has security-hardened settings before middleware creation
	assert!(
		production_config.cookie_secure,
		"Production config must require HTTPS"
	);
	assert!(
		production_config.enable_token_rotation,
		"Production config must enable token rotation"
	);
	assert!(
		production_config.cookie_max_age.is_some(),
		"Production config must set cookie max age"
	);
	assert!(
		production_config.token_rotation_interval.is_some(),
		"Production config must set rotation interval"
	);
	assert!(
		!production_config.cookie_httponly,
		"CSRF cookie must not be HttpOnly (JavaScript needs access)"
	);

	// Act
	let _mw_default = CsrfMiddleware::new();
	let _mw_custom = CsrfMiddleware::with_config(production_config);

	// Assert
	// Middleware construction from both default and production config must succeed
}

// ============================================================================
// XSS: escape_html
// ============================================================================

#[rstest]
fn xss_escape_html_with_script_tag() {
	// Arrange
	let input = "<script>alert('xss')</script>";

	// Act
	let escaped = escape_html(input);

	// Assert
	assert!(
		!escaped.contains('<'),
		"Escaped output must not contain raw '<'"
	);
	assert!(
		!escaped.contains('>'),
		"Escaped output must not contain raw '>'"
	);
	assert!(escaped.contains("&lt;"), "Must escape '<' to '&lt;'");
	assert!(escaped.contains("&gt;"), "Must escape '>' to '&gt;'");
}

#[rstest]
fn xss_escape_html_with_special_chars() {
	// Arrange
	let input = "Tom & Jerry's \"adventure\" <fun>";

	// Act
	let escaped = escape_html(input);

	// Assert
	assert!(escaped.contains("&amp;"), "Must escape '&' to '&amp;'");
	assert!(escaped.contains("&lt;"), "Must escape '<' to '&lt;'");
	assert!(escaped.contains("&gt;"), "Must escape '>' to '&gt;'");
}

#[rstest]
fn xss_escape_html_preserves_safe_text() {
	// Arrange
	let input = "Hello, World!";

	// Act
	let escaped = escape_html(input);

	// Assert
	assert_eq!(escaped, input, "Safe text should remain unchanged");
}

#[rstest]
fn xss_escape_html_content_works() {
	// Arrange
	let input = "<div>content & more</div>";

	// Act
	let escaped = escape_html_content(input);

	// Assert
	assert!(!escaped.contains('<'));
	assert!(!escaped.contains('>'));
	assert!(escaped.contains("&amp;"));
}

// ============================================================================
// XSS: escape_html_attr and escape_javascript
// ============================================================================

#[rstest]
fn xss_escape_html_attr_escapes_quotes() {
	// Arrange
	let input = r#"value" onclick="alert(1)"#;

	// Act
	let escaped = escape_html_attr(input);

	// Assert
	assert!(
		!escaped.contains('"'),
		"Must escape double quotes in attributes"
	);
}

#[rstest]
fn xss_escape_javascript_escapes_special_chars() {
	// Arrange
	let input = "alert('xss');\nvar x = \"test\";";

	// Act
	let escaped = escape_javascript(input);

	// Assert
	// Single quotes are escaped to \', so raw unescaped single quotes should not remain
	assert!(
		escaped.contains("\\'"),
		"Single quotes must be escaped to \\'"
	);
	assert!(escaped.contains("\\n"), "Newlines must be escaped to \\n");
	assert!(
		!escaped.contains('\n'),
		"Raw newline character must not remain"
	);
}

#[rstest]
fn xss_escape_javascript_handles_backslash() {
	// Arrange
	let input = "c:\\temp";

	// Act
	let escaped = escape_javascript(input);

	// Assert
	assert!(
		escaped.contains("\\\\"),
		"Backslashes must be doubled to prevent escape sequences"
	);
}

// ============================================================================
// XSS: sanitize_html and strip_tags_safe
// ============================================================================

#[rstest]
fn xss_sanitize_html_escapes_script_tags() {
	// Arrange
	let input = "<script>alert('xss')</script>";

	// Act
	let sanitized = sanitize_html(input);

	// Assert
	assert!(
		!sanitized.contains("<script>"),
		"sanitize_html must escape script tags"
	);
	assert!(
		sanitized.contains("&lt;script&gt;"),
		"Script tags must be HTML-escaped"
	);
}

#[rstest]
fn xss_sanitize_html_preserves_text_content() {
	// Arrange
	let input = "<p>Hello <strong>World</strong></p>";

	// Act
	let sanitized = sanitize_html(input);

	// Assert
	assert!(
		sanitized.contains("Hello"),
		"sanitize_html should preserve text content"
	);
	assert!(
		sanitized.contains("World"),
		"sanitize_html should preserve text content"
	);
	// All tags are escaped, not stripped
	assert!(
		!sanitized.contains("<p>"),
		"sanitize_html should escape all HTML tags"
	);
}

#[rstest]
fn xss_strip_tags_safe_removes_all_tags() {
	// Arrange
	let input = "<p>Hello <b>World</b></p>";

	// Act
	let stripped = strip_tags_safe(input);

	// Assert
	assert_eq!(stripped, "Hello World");
	assert!(
		!stripped.contains('<'),
		"strip_tags_safe must remove all HTML tags"
	);
	assert!(
		!stripped.contains('>'),
		"strip_tags_safe must remove all closing brackets"
	);
}

#[rstest]
fn xss_strip_tags_safe_returns_plain_text_unchanged() {
	// Arrange
	let input = "No tags here, just plain text.";

	// Act
	let stripped = strip_tags_safe(input);

	// Assert
	assert_eq!(stripped, input);
}

// ============================================================================
// XSS: CSS selector escaping and validation
// ============================================================================

#[rstest]
fn xss_escape_css_selector_escapes_special_chars() {
	// Arrange
	let input = ".my-class #id > child";

	// Act
	let escaped = escape_css_selector(input);

	// Assert
	// The escaped form should not contain raw selector combinators that could be injected
	assert_ne!(
		escaped, input,
		"CSS selector with special chars should be escaped"
	);
}

#[rstest]
fn xss_validate_css_selector_accepts_valid() {
	// Act
	let simple_valid = validate_css_selector("my-class");
	let hyphenated_valid = validate_css_selector("header-nav");

	// Assert
	assert!(simple_valid, "Simple class name should be valid");
	assert!(hyphenated_valid, "Hyphenated name should be valid");
}

// ============================================================================
// Redirect validation: allowed and disallowed hosts
// ============================================================================

fn make_allowed_hosts(hosts: &[&str]) -> HashSet<String> {
	hosts.iter().map(|s| s.to_string()).collect()
}

#[rstest]
#[case("/dashboard", true)]
#[case("/path/to/resource", true)]
#[case("/", true)]
fn redirect_allows_relative_urls(#[case] url: &str, #[case] expected: bool) {
	// Arrange
	let hosts = make_allowed_hosts(&["example.com"]);

	// Act
	let result = validate_redirect_url(url, &hosts);

	// Assert
	assert_eq!(
		result.is_ok(),
		expected,
		"Relative URL '{url}' should be allowed"
	);
}

#[rstest]
fn redirect_allows_trusted_host() {
	// Arrange
	let hosts = make_allowed_hosts(&["example.com", "api.example.com"]);

	// Act
	let result = validate_redirect_url("https://example.com/callback", &hosts);

	// Assert
	assert!(result.is_ok(), "Trusted host should be allowed");
}

#[rstest]
fn redirect_rejects_untrusted_host() {
	// Arrange
	let hosts = make_allowed_hosts(&["example.com"]);

	// Act
	let result = validate_redirect_url("https://evil.com/phishing", &hosts);

	// Assert
	assert!(
		matches!(result, Err(RedirectValidationError::UntrustedHost(_))),
		"Untrusted host must be rejected with UntrustedHost error"
	);
}

#[rstest]
#[case("javascript:alert(1)")]
#[case("data:text/html,<script>alert(1)</script>")]
#[case("vbscript:MsgBox(1)")]
fn redirect_rejects_dangerous_protocols(#[case] url: &str) {
	// Arrange
	let hosts = make_allowed_hosts(&["example.com"]);

	// Act
	let result = validate_redirect_url(url, &hosts);

	// Assert
	assert!(
		result.is_err(),
		"Dangerous protocol '{url}' must be rejected"
	);
}

#[rstest]
fn redirect_rejects_protocol_relative_url() {
	// Arrange
	let hosts = make_allowed_hosts(&["example.com"]);

	// Act
	let result = validate_redirect_url("//evil.com/path", &hosts);

	// Assert
	assert!(
		matches!(result, Err(RedirectValidationError::ProtocolRelative)),
		"Protocol-relative URL must be rejected"
	);
}

#[rstest]
fn redirect_detects_encoded_bypass_attempt() {
	// Arrange
	let hosts = make_allowed_hosts(&["example.com"]);

	// Act - URL-encoded "javascript:" prefix
	let result = validate_redirect_url("%6Aavascript:alert(1)", &hosts);

	// Assert
	assert!(
		result.is_err(),
		"Encoded bypass attempt must be detected and rejected"
	);
}

// ============================================================================
// is_safe_redirect convenience function
// ============================================================================

#[rstest]
fn is_safe_redirect_returns_true_for_safe_urls() {
	// Arrange
	let hosts = make_allowed_hosts(&["example.com"]);

	// Act
	let relative = is_safe_redirect("/dashboard", &hosts);
	let absolute_trusted = is_safe_redirect("https://example.com/page", &hosts);
	let empty = is_safe_redirect("", &hosts);

	// Assert
	assert!(relative);
	assert!(absolute_trusted);
	assert!(empty);
}

#[rstest]
fn is_safe_redirect_returns_false_for_unsafe_urls() {
	// Arrange
	let hosts = make_allowed_hosts(&["example.com"]);

	// Act
	let untrusted = is_safe_redirect("https://evil.com", &hosts);
	let javascript = is_safe_redirect("javascript:alert(1)", &hosts);
	let protocol_relative = is_safe_redirect("//evil.com/path", &hosts);

	// Assert
	assert!(!untrusted);
	assert!(!javascript);
	assert!(!protocol_relative);
}

// ============================================================================
// SecurityError variants
// ============================================================================

#[rstest]
fn security_error_csrf_validation_failed() {
	// Arrange
	let error = SecurityError::CsrfValidationFailed("token mismatch".into());

	// Act
	let msg = error.to_string();

	// Assert
	assert!(msg.contains("CSRF"), "Error message should mention CSRF");
	assert!(
		msg.contains("token mismatch"),
		"Error message should contain the detail"
	);
}

#[rstest]
fn security_error_missing_csrf_token() {
	// Arrange
	let error = SecurityError::MissingCsrfToken;

	// Act
	let msg = error.to_string();

	// Assert
	assert!(
		msg.contains("Missing"),
		"Error message should indicate missing token"
	);
}

#[rstest]
fn security_error_invalid_configuration() {
	// Arrange
	let error = SecurityError::InvalidConfiguration("bad setting".into());

	// Act
	let msg = error.to_string();

	// Assert
	assert!(
		msg.contains("configuration"),
		"Error message should mention configuration"
	);
	assert!(
		msg.contains("bad setting"),
		"Error message should contain the detail"
	);
}

#[rstest]
fn security_error_xss_detected() {
	// Arrange
	let error = SecurityError::XssDetected("script injection".into());

	// Act
	let msg = error.to_string();

	// Assert
	assert!(msg.contains("XSS"), "Error message should mention XSS");
	assert!(
		msg.contains("script injection"),
		"Error message should contain the detail"
	);
}

#[rstest]
fn security_error_is_debug_printable() {
	// Arrange
	let errors = vec![
		SecurityError::CsrfValidationFailed("test".into()),
		SecurityError::MissingCsrfToken,
		SecurityError::InvalidConfiguration("test".into()),
		SecurityError::XssDetected("test".into()),
	];

	// Act
	let debug_strings: Vec<String> = errors.iter().map(|e| format!("{:?}", e)).collect();

	// Assert
	for debug_str in &debug_strings {
		assert!(
			!debug_str.is_empty(),
			"All SecurityError variants must be Debug-printable"
		);
	}
}

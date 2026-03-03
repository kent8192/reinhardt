//! Security utilities for admin panel
//!
//! This module provides security-related functionality including:
//! - CSP and security response headers
//! - CSRF token generation and validation
//! - XSS prevention helpers for mutation data

use std::collections::HashMap;

/// Security headers for admin panel responses.
///
/// Provides a set of HTTP security headers recommended by OWASP for
/// web application security hardening.
///
/// # Headers included
///
/// - `Content-Security-Policy`: Restricts resource loading origins
/// - `X-Content-Type-Options`: Prevents MIME-type sniffing
/// - `X-Frame-Options`: Prevents clickjacking
/// - `X-XSS-Protection`: Legacy XSS protection (for older browsers)
/// - `Referrer-Policy`: Controls referrer information
/// - `Permissions-Policy`: Restricts browser features
///
/// # Examples
///
/// ```
/// use reinhardt_admin::server::security::SecurityHeaders;
///
/// let headers = SecurityHeaders::default();
/// let header_map = headers.to_header_map();
/// assert!(header_map.contains_key("Content-Security-Policy"));
/// assert!(header_map.contains_key("X-Content-Type-Options"));
/// ```
#[derive(Debug, Clone)]
pub struct SecurityHeaders {
	/// Content Security Policy directives
	pub csp: ContentSecurityPolicy,
	/// X-Frame-Options value
	pub frame_options: FrameOptions,
	/// Referrer-Policy value
	pub referrer_policy: ReferrerPolicy,
}

impl Default for SecurityHeaders {
	fn default() -> Self {
		Self {
			csp: ContentSecurityPolicy::admin_default(),
			frame_options: FrameOptions::Deny,
			referrer_policy: ReferrerPolicy::StrictOriginWhenCrossOrigin,
		}
	}
}

impl SecurityHeaders {
	/// Convert security headers to a map of header name -> value pairs.
	///
	/// This can be used to apply headers to HTTP responses.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::server::security::SecurityHeaders;
	///
	/// let headers = SecurityHeaders::default();
	/// let map = headers.to_header_map();
	/// assert_eq!(map.get("X-Content-Type-Options").unwrap(), "nosniff");
	/// assert_eq!(map.get("X-Frame-Options").unwrap(), "DENY");
	/// ```
	pub fn to_header_map(&self) -> HashMap<&'static str, String> {
		let mut headers = HashMap::new();

		headers.insert("Content-Security-Policy", self.csp.to_header_value());
		headers.insert("X-Content-Type-Options", "nosniff".to_string());
		headers.insert("X-Frame-Options", self.frame_options.to_string());
		headers.insert("X-XSS-Protection", "1; mode=block".to_string());
		headers.insert("Referrer-Policy", self.referrer_policy.to_string());
		headers.insert(
			"Permissions-Policy",
			"camera=(), microphone=(), geolocation=(), payment=()".to_string(),
		);

		headers
	}
}

/// Content Security Policy configuration for admin panel.
///
/// Provides a structured way to build CSP headers with appropriate
/// directives for the admin panel.
#[derive(Debug, Clone)]
pub struct ContentSecurityPolicy {
	/// default-src directive
	pub default_src: Vec<String>,
	/// script-src directive
	pub script_src: Vec<String>,
	/// style-src directive
	pub style_src: Vec<String>,
	/// img-src directive
	pub img_src: Vec<String>,
	/// font-src directive
	pub font_src: Vec<String>,
	/// connect-src directive
	pub connect_src: Vec<String>,
	/// frame-ancestors directive
	pub frame_ancestors: Vec<String>,
	/// base-uri directive
	pub base_uri: Vec<String>,
	/// form-action directive
	pub form_action: Vec<String>,
}

impl ContentSecurityPolicy {
	/// Creates a default CSP suitable for admin panel usage.
	///
	/// Allows:
	/// - Scripts and styles from same origin
	/// - Inline styles (required for admin UI components)
	/// - Images from same origin and data URIs (for favicons)
	/// - Connections to same origin only (for API calls)
	/// - No framing allowed
	pub fn admin_default() -> Self {
		Self {
			default_src: vec!["'self'".to_string()],
			script_src: vec!["'self'".to_string()],
			style_src: vec!["'self'".to_string(), "'unsafe-inline'".to_string()],
			img_src: vec!["'self'".to_string(), "data:".to_string()],
			font_src: vec!["'self'".to_string()],
			connect_src: vec!["'self'".to_string()],
			frame_ancestors: vec!["'none'".to_string()],
			base_uri: vec!["'self'".to_string()],
			form_action: vec!["'self'".to_string()],
		}
	}

	/// Converts the CSP configuration to a header value string.
	fn to_header_value(&self) -> String {
		let mut directives = Vec::new();

		if !self.default_src.is_empty() {
			directives.push(format!("default-src {}", self.default_src.join(" ")));
		}
		if !self.script_src.is_empty() {
			directives.push(format!("script-src {}", self.script_src.join(" ")));
		}
		if !self.style_src.is_empty() {
			directives.push(format!("style-src {}", self.style_src.join(" ")));
		}
		if !self.img_src.is_empty() {
			directives.push(format!("img-src {}", self.img_src.join(" ")));
		}
		if !self.font_src.is_empty() {
			directives.push(format!("font-src {}", self.font_src.join(" ")));
		}
		if !self.connect_src.is_empty() {
			directives.push(format!("connect-src {}", self.connect_src.join(" ")));
		}
		if !self.frame_ancestors.is_empty() {
			directives.push(format!(
				"frame-ancestors {}",
				self.frame_ancestors.join(" ")
			));
		}
		if !self.base_uri.is_empty() {
			directives.push(format!("base-uri {}", self.base_uri.join(" ")));
		}
		if !self.form_action.is_empty() {
			directives.push(format!("form-action {}", self.form_action.join(" ")));
		}

		directives.join("; ")
	}
}

/// X-Frame-Options header values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameOptions {
	/// Completely prevents framing
	Deny,
	/// Allows framing only from same origin
	SameOrigin,
}

impl std::fmt::Display for FrameOptions {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			FrameOptions::Deny => write!(f, "DENY"),
			FrameOptions::SameOrigin => write!(f, "SAMEORIGIN"),
		}
	}
}

/// Referrer-Policy header values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferrerPolicy {
	/// No referrer information sent
	NoReferrer,
	/// Only send origin on cross-origin, full URL on same-origin
	StrictOriginWhenCrossOrigin,
	/// Only send origin on same-origin, nothing on cross-origin
	SameOrigin,
}

impl std::fmt::Display for ReferrerPolicy {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			ReferrerPolicy::NoReferrer => write!(f, "no-referrer"),
			ReferrerPolicy::StrictOriginWhenCrossOrigin => {
				write!(f, "strict-origin-when-cross-origin")
			}
			ReferrerPolicy::SameOrigin => write!(f, "same-origin"),
		}
	}
}

/// CSRF token length in bytes (before base64 encoding)
const CSRF_TOKEN_BYTES: usize = 32;

/// Generates a cryptographically secure CSRF token.
///
/// Uses the operating system's secure random number generator to produce
/// a 256-bit (32-byte) random value, encoded as URL-safe base64.
///
/// # Examples
///
/// ```
/// use reinhardt_admin::server::security::generate_csrf_token;
///
/// let token = generate_csrf_token();
/// assert!(!token.is_empty());
/// assert!(token.len() >= 32);
///
/// // Each call produces a unique token
/// let token2 = generate_csrf_token();
/// assert_ne!(token, token2);
/// ```
pub fn generate_csrf_token() -> String {
	use base64::Engine;
	let mut bytes = vec![0u8; CSRF_TOKEN_BYTES];
	// Use getrandom for cryptographically secure randomness
	getrandom::fill(&mut bytes).expect("Failed to generate random bytes for CSRF token");
	base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&bytes)
}

/// Validates a CSRF token against the expected value using constant-time comparison.
///
/// This prevents timing side-channel attacks that could be used to
/// guess the CSRF token byte by byte.
///
/// # Arguments
///
/// * `provided` - The CSRF token provided in the request
/// * `expected` - The expected CSRF token stored in the session
///
/// # Examples
///
/// ```
/// use reinhardt_admin::server::security::{generate_csrf_token, validate_csrf_token};
///
/// let token = generate_csrf_token();
/// assert!(validate_csrf_token(&token, &token));
/// assert!(!validate_csrf_token("invalid", &token));
/// assert!(!validate_csrf_token("", &token));
/// ```
pub fn validate_csrf_token(provided: &str, expected: &str) -> bool {
	// Reject empty tokens immediately
	if provided.is_empty() || expected.is_empty() {
		return false;
	}

	// Constant-time comparison to prevent timing attacks
	constant_time_eq(provided.as_bytes(), expected.as_bytes())
}

/// Constant-time byte comparison to prevent timing attacks.
///
/// Returns true only if both slices are equal in length and content.
/// The comparison always examines all bytes regardless of where
/// differences occur.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
	if a.len() != b.len() {
		return false;
	}

	let mut result = 0u8;
	for (x, y) in a.iter().zip(b.iter()) {
		result |= x ^ y;
	}
	result == 0
}

/// Sanitizes mutation data values to prevent stored XSS.
///
/// Checks all string values in the mutation data for dangerous HTML/JavaScript
/// patterns and escapes them before storage. This provides defense-in-depth
/// on top of the output escaping done by the rendering framework.
///
/// # Arguments
///
/// * `data` - Mutable reference to the mutation data to sanitize
///
/// # Examples
///
/// ```
/// use reinhardt_admin::server::security::sanitize_mutation_values;
/// use std::collections::HashMap;
///
/// let mut data = HashMap::new();
/// data.insert("name".to_string(), serde_json::json!("<script>alert('xss')</script>"));
/// data.insert("age".to_string(), serde_json::json!(25));
///
/// sanitize_mutation_values(&mut data);
///
/// // String values are escaped
/// assert_eq!(
///     data.get("name").unwrap().as_str().unwrap(),
///     "&lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;"
/// );
/// // Non-string values are unchanged
/// assert_eq!(data.get("age").unwrap().as_i64().unwrap(), 25);
/// ```
pub fn sanitize_mutation_values(data: &mut HashMap<String, serde_json::Value>) {
	for value in data.values_mut() {
		sanitize_json_value(value);
	}
}

/// Recursively sanitizes a JSON value, escaping HTML in strings.
fn sanitize_json_value(value: &mut serde_json::Value) {
	match value {
		serde_json::Value::String(s) => {
			if needs_html_escaping(s) {
				*s = escape_html(s);
			}
		}
		serde_json::Value::Array(arr) => {
			for item in arr.iter_mut() {
				sanitize_json_value(item);
			}
		}
		serde_json::Value::Object(obj) => {
			for val in obj.values_mut() {
				sanitize_json_value(val);
			}
		}
		// Numbers, booleans, null are safe
		_ => {}
	}
}

/// Checks if a string contains characters that need HTML escaping.
fn needs_html_escaping(s: &str) -> bool {
	s.contains('<') || s.contains('>') || s.contains('&') || s.contains('"') || s.contains('\'')
}

/// Escapes HTML special characters in a string.
fn escape_html(input: &str) -> String {
	input
		.replace('&', "&amp;")
		.replace('<', "&lt;")
		.replace('>', "&gt;")
		.replace('"', "&quot;")
		.replace('\'', "&#x27;")
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	// ============================================================
	// SecurityHeaders tests
	// ============================================================

	#[rstest]
	fn test_security_headers_default_contains_all_headers() {
		// Arrange
		let headers = SecurityHeaders::default();

		// Act
		let map = headers.to_header_map();

		// Assert
		assert!(map.contains_key("Content-Security-Policy"));
		assert!(map.contains_key("X-Content-Type-Options"));
		assert!(map.contains_key("X-Frame-Options"));
		assert!(map.contains_key("X-XSS-Protection"));
		assert!(map.contains_key("Referrer-Policy"));
		assert!(map.contains_key("Permissions-Policy"));
	}

	#[rstest]
	fn test_security_headers_x_content_type_options() {
		// Arrange
		let headers = SecurityHeaders::default();

		// Act
		let map = headers.to_header_map();

		// Assert
		assert_eq!(map.get("X-Content-Type-Options").unwrap(), "nosniff");
	}

	#[rstest]
	fn test_security_headers_x_frame_options_deny() {
		// Arrange
		let headers = SecurityHeaders::default();

		// Act
		let map = headers.to_header_map();

		// Assert
		assert_eq!(map.get("X-Frame-Options").unwrap(), "DENY");
	}

	#[rstest]
	fn test_security_headers_x_xss_protection() {
		// Arrange
		let headers = SecurityHeaders::default();

		// Act
		let map = headers.to_header_map();

		// Assert
		assert_eq!(map.get("X-XSS-Protection").unwrap(), "1; mode=block");
	}

	#[rstest]
	fn test_security_headers_referrer_policy() {
		// Arrange
		let headers = SecurityHeaders::default();

		// Act
		let map = headers.to_header_map();

		// Assert
		assert_eq!(
			map.get("Referrer-Policy").unwrap(),
			"strict-origin-when-cross-origin"
		);
	}

	#[rstest]
	fn test_security_headers_permissions_policy() {
		// Arrange
		let headers = SecurityHeaders::default();

		// Act
		let map = headers.to_header_map();

		// Assert
		let pp = map.get("Permissions-Policy").unwrap();
		assert!(pp.contains("camera=()"));
		assert!(pp.contains("microphone=()"));
		assert!(pp.contains("geolocation=()"));
	}

	// ============================================================
	// CSP tests
	// ============================================================

	#[rstest]
	fn test_csp_admin_default_contains_self() {
		// Arrange
		let csp = ContentSecurityPolicy::admin_default();

		// Act
		let csp_string = csp.to_header_value();

		// Assert
		assert!(csp_string.contains("default-src 'self'"));
		assert!(csp_string.contains("script-src 'self'"));
	}

	#[rstest]
	fn test_csp_admin_default_prevents_framing() {
		// Arrange
		let csp = ContentSecurityPolicy::admin_default();

		// Act
		let csp_string = csp.to_header_value();

		// Assert
		assert!(csp_string.contains("frame-ancestors 'none'"));
	}

	#[rstest]
	fn test_csp_admin_default_allows_inline_styles() {
		// Arrange
		let csp = ContentSecurityPolicy::admin_default();

		// Act
		let csp_string = csp.to_header_value();

		// Assert
		assert!(csp_string.contains("style-src 'self' 'unsafe-inline'"));
	}

	#[rstest]
	fn test_csp_admin_default_restricts_form_action() {
		// Arrange
		let csp = ContentSecurityPolicy::admin_default();

		// Act
		let csp_string = csp.to_header_value();

		// Assert
		assert!(csp_string.contains("form-action 'self'"));
	}

	// ============================================================
	// FrameOptions tests
	// ============================================================

	#[rstest]
	fn test_frame_options_deny() {
		// Assert
		assert_eq!(FrameOptions::Deny.to_string(), "DENY");
	}

	#[rstest]
	fn test_frame_options_same_origin() {
		// Assert
		assert_eq!(FrameOptions::SameOrigin.to_string(), "SAMEORIGIN");
	}

	// ============================================================
	// ReferrerPolicy tests
	// ============================================================

	#[rstest]
	fn test_referrer_policy_no_referrer() {
		// Assert
		assert_eq!(ReferrerPolicy::NoReferrer.to_string(), "no-referrer");
	}

	#[rstest]
	fn test_referrer_policy_strict_origin() {
		// Assert
		assert_eq!(
			ReferrerPolicy::StrictOriginWhenCrossOrigin.to_string(),
			"strict-origin-when-cross-origin"
		);
	}

	#[rstest]
	fn test_referrer_policy_same_origin() {
		// Assert
		assert_eq!(ReferrerPolicy::SameOrigin.to_string(), "same-origin");
	}

	// ============================================================
	// CSRF token tests
	// ============================================================

	#[rstest]
	fn test_generate_csrf_token_is_non_empty() {
		// Act
		let token = generate_csrf_token();

		// Assert
		assert!(!token.is_empty());
		assert!(token.len() >= 32);
	}

	#[rstest]
	fn test_generate_csrf_token_is_unique() {
		// Act
		let token1 = generate_csrf_token();
		let token2 = generate_csrf_token();

		// Assert
		assert_ne!(token1, token2);
	}

	#[rstest]
	fn test_validate_csrf_token_with_matching_tokens() {
		// Arrange
		let token = generate_csrf_token();

		// Act & Assert
		assert!(validate_csrf_token(&token, &token));
	}

	#[rstest]
	fn test_validate_csrf_token_with_mismatching_tokens() {
		// Arrange
		let token = generate_csrf_token();

		// Act & Assert
		assert!(!validate_csrf_token("invalid-token", &token));
	}

	#[rstest]
	fn test_validate_csrf_token_rejects_empty_provided() {
		// Arrange
		let token = generate_csrf_token();

		// Act & Assert
		assert!(!validate_csrf_token("", &token));
	}

	#[rstest]
	fn test_validate_csrf_token_rejects_empty_expected() {
		// Act & Assert
		assert!(!validate_csrf_token("some-token", ""));
	}

	#[rstest]
	fn test_validate_csrf_token_rejects_both_empty() {
		// Act & Assert
		assert!(!validate_csrf_token("", ""));
	}

	// ============================================================
	// Constant-time comparison tests
	// ============================================================

	#[rstest]
	fn test_constant_time_eq_equal() {
		// Assert
		assert!(constant_time_eq(b"hello", b"hello"));
	}

	#[rstest]
	fn test_constant_time_eq_different_content() {
		// Assert
		assert!(!constant_time_eq(b"hello", b"world"));
	}

	#[rstest]
	fn test_constant_time_eq_different_length() {
		// Assert
		assert!(!constant_time_eq(b"hello", b"hi"));
	}

	#[rstest]
	fn test_constant_time_eq_empty() {
		// Assert
		assert!(constant_time_eq(b"", b""));
	}

	// ============================================================
	// Mutation data sanitization tests
	// ============================================================

	#[rstest]
	fn test_sanitize_mutation_values_escapes_script_tags() {
		// Arrange
		let mut data = HashMap::new();
		data.insert(
			"name".to_string(),
			serde_json::json!("<script>alert('xss')</script>"),
		);

		// Act
		sanitize_mutation_values(&mut data);

		// Assert
		let name = data.get("name").unwrap().as_str().unwrap();
		assert!(!name.contains("<script>"));
		assert!(name.contains("&lt;script&gt;"));
	}

	#[rstest]
	fn test_sanitize_mutation_values_preserves_non_string_values() {
		// Arrange
		let mut data = HashMap::new();
		data.insert("age".to_string(), serde_json::json!(25));
		data.insert("active".to_string(), serde_json::json!(true));
		data.insert("tags".to_string(), serde_json::json!(null));

		// Act
		sanitize_mutation_values(&mut data);

		// Assert
		assert_eq!(data.get("age").unwrap().as_i64().unwrap(), 25);
		assert_eq!(data.get("active").unwrap().as_bool().unwrap(), true);
		assert!(data.get("tags").unwrap().is_null());
	}

	#[rstest]
	fn test_sanitize_mutation_values_handles_nested_arrays() {
		// Arrange
		let mut data = HashMap::new();
		data.insert(
			"items".to_string(),
			serde_json::json!(["<b>bold</b>", "safe text"]),
		);

		// Act
		sanitize_mutation_values(&mut data);

		// Assert
		let items = data.get("items").unwrap().as_array().unwrap();
		assert_eq!(items[0].as_str().unwrap(), "&lt;b&gt;bold&lt;/b&gt;");
		assert_eq!(items[1].as_str().unwrap(), "safe text");
	}

	#[rstest]
	fn test_sanitize_mutation_values_handles_nested_objects() {
		// Arrange
		let mut data = HashMap::new();
		data.insert(
			"metadata".to_string(),
			serde_json::json!({"bio": "<img onerror=alert(1)>"}),
		);

		// Act
		sanitize_mutation_values(&mut data);

		// Assert
		let meta = data.get("metadata").unwrap().as_object().unwrap();
		let bio = meta.get("bio").unwrap().as_str().unwrap();
		assert!(!bio.contains("<img"));
		assert!(bio.contains("&lt;img"));
	}

	#[rstest]
	fn test_sanitize_mutation_values_safe_strings_unchanged() {
		// Arrange
		let mut data = HashMap::new();
		data.insert("name".to_string(), serde_json::json!("Alice Johnson"));
		data.insert("email".to_string(), serde_json::json!("alice@example.com"));

		// Act
		sanitize_mutation_values(&mut data);

		// Assert
		assert_eq!(data.get("name").unwrap().as_str().unwrap(), "Alice Johnson");
		assert_eq!(
			data.get("email").unwrap().as_str().unwrap(),
			"alice@example.com"
		);
	}

	#[rstest]
	fn test_escape_html_special_characters() {
		// Assert
		assert_eq!(escape_html("<"), "&lt;");
		assert_eq!(escape_html(">"), "&gt;");
		assert_eq!(escape_html("&"), "&amp;");
		assert_eq!(escape_html("\""), "&quot;");
		assert_eq!(escape_html("'"), "&#x27;");
	}

	#[rstest]
	fn test_needs_html_escaping_detects_dangerous_chars() {
		// Assert
		assert!(needs_html_escaping("<script>"));
		assert!(needs_html_escaping("a > b"));
		assert!(needs_html_escaping("a & b"));
		assert!(needs_html_escaping("a\"b"));
		assert!(needs_html_escaping("a'b"));
		assert!(!needs_html_escaping("safe text"));
		assert!(!needs_html_escaping("hello world 123"));
	}
}

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
	/// Permissions-Policy header value
	pub permissions_policy: String,
}

impl Default for SecurityHeaders {
	fn default() -> Self {
		Self {
			csp: ContentSecurityPolicy::admin_default(),
			frame_options: FrameOptions::Deny,
			referrer_policy: ReferrerPolicy::StrictOriginWhenCrossOrigin,
			permissions_policy: "camera=(), microphone=(), geolocation=(), payment=()".to_string(),
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
		headers.insert("Permissions-Policy", self.permissions_policy.clone());

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
	/// - Scripts from same origin with WASM evaluation
	/// - Inline styles (required for admin UI components)
	/// - Images from same origin and data URIs (for favicons)
	/// - Connections to same origin only (for API calls)
	/// - No framing allowed
	pub fn admin_default() -> Self {
		Self {
			default_src: vec!["'self'".to_string()],
			script_src: vec!["'self'".to_string(), "'wasm-unsafe-eval'".to_string()],
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

impl std::str::FromStr for FrameOptions {
	type Err = std::convert::Infallible;

	/// Parse from a string, falling back to `Deny` for unrecognized values.
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Ok(match s.to_lowercase().as_str() {
			"deny" => Self::Deny,
			"sameorigin" => Self::SameOrigin,
			_ => Self::Deny,
		})
	}
}

impl std::str::FromStr for ReferrerPolicy {
	type Err = std::convert::Infallible;

	/// Parse from a string, falling back to `StrictOriginWhenCrossOrigin`
	/// for unrecognized values.
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Ok(match s.to_lowercase().as_str() {
			"no-referrer" => Self::NoReferrer,
			"strict-origin-when-cross-origin" => Self::StrictOriginWhenCrossOrigin,
			"same-origin" => Self::SameOrigin,
			_ => Self::StrictOriginWhenCrossOrigin,
		})
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

/// Constant-time comparison to prevent timing attacks.
///
/// Hashes both inputs with SHA-256 to produce fixed-length digests,
/// then compares the digests in constant time using `subtle`. This
/// prevents leaking the length of either input through timing.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
	use sha2::{Digest, Sha256};
	use subtle::ConstantTimeEq;

	let hash_a = Sha256::digest(a);
	let hash_b = Sha256::digest(b);
	hash_a.ct_eq(&hash_b).into()
}

/// The header name used for CSRF token submission.
pub const CSRF_HEADER_NAME: &str = "x-csrf-token";

/// The cookie name used for CSRF token storage (double-submit cookie pattern).
pub const CSRF_COOKIE_NAME: &str = "csrftoken";

/// Extracts the CSRF token from the `X-CSRF-Token` request header.
///
/// Returns `None` if the header is missing or not valid UTF-8.
///
/// # Arguments
///
/// * `headers` - The HTTP request headers
#[cfg(server)]
pub fn extract_csrf_header(headers: &hyper::HeaderMap) -> Option<String> {
	headers
		.get(CSRF_HEADER_NAME)
		.and_then(|v| v.to_str().ok())
		.map(|s| s.to_string())
}

/// Extracts the CSRF token from the `csrftoken` cookie.
///
/// Parses the `Cookie` header and returns the value of the `csrftoken`
/// cookie if present.
///
/// # Arguments
///
/// * `headers` - The HTTP request headers
#[cfg(server)]
pub fn extract_csrf_cookie(headers: &hyper::HeaderMap) -> Option<String> {
	headers
		.get("cookie")
		.and_then(|v| v.to_str().ok())
		.and_then(|cookie_header| {
			cookie_header.split(';').find_map(|pair| {
				let pair = pair.trim();
				let (name, value) = pair.split_once('=')?;
				if name.trim() == CSRF_COOKIE_NAME {
					Some(value.trim().to_string())
				} else {
					None
				}
			})
		})
}

/// Builds a `Set-Cookie` header value for the CSRF token.
///
/// Sets the cookie with security attributes:
/// - `SameSite=Strict`: prevents cross-site cookie sending
/// - `Secure`: only sent over HTTPS (skipped for localhost)
/// - `HttpOnly` is NOT set so client-side JS/WASM can read the cookie
/// - `Path=/admin`: scoped to admin panel routes
///
/// # Arguments
///
/// * `token` - The CSRF token value
/// * `is_secure` - Whether to add the `Secure` flag (false for localhost)
pub fn build_csrf_cookie(token: &str, is_secure: bool) -> String {
	let secure_flag = if is_secure { "; Secure" } else { "" };
	format!(
		"{}={}; SameSite=Strict; Path=/admin{}",
		CSRF_COOKIE_NAME, token, secure_flag
	)
}

/// Validates CSRF tokens using the double-submit cookie pattern.
///
/// Compares the token submitted in the request body (or `X-CSRF-Token` header)
/// against the token stored in the `csrftoken` cookie. The cookie is set by the
/// server and cannot be read or forged by a cross-origin attacker, making this
/// pattern secure against CSRF attacks.
///
/// # Arguments
///
/// * `body_token` - The CSRF token from the request body
/// * `headers` - The HTTP request headers (to extract the cookie token)
///
/// # Errors
///
/// Returns a `ServerFnError` with status 403 if:
/// - The `csrftoken` cookie is missing
/// - The body token is empty
/// - The tokens do not match
#[cfg(server)]
pub fn require_csrf_token(
	body_token: &str,
	headers: &hyper::HeaderMap,
) -> Result<(), reinhardt_pages::server_fn::ServerFnError> {
	// Double-submit cookie pattern: compare body token with cookie token.
	// Fallback: accept the X-CSRFToken header when cookies are unavailable
	// (e.g. WASM reqwest client which does not send browser cookies).
	let expected_token = extract_csrf_cookie(headers)
		.or_else(|| extract_csrf_header(headers))
		.ok_or_else(|| {
			reinhardt_pages::server_fn::ServerFnError::server(
				403,
				"CSRF token missing from cookie and header",
			)
		})?;

	if !validate_csrf_token(body_token, &expected_token) {
		return Err(reinhardt_pages::server_fn::ServerFnError::server(
			403,
			"CSRF token validation failed",
		));
	}

	Ok(())
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
	reinhardt_core::security::escape_html(input)
}

// --- Admin authentication cookie ---

/// The cookie name used for admin JWT authentication.
///
/// This cookie stores the JWT token as an HTTP-Only, `SameSite=Strict` cookie,
/// preventing JavaScript access (XSS protection) and cross-origin sending
/// (CSRF protection).
pub const ADMIN_AUTH_COOKIE_NAME: &str = "reinhardt_admin_token";

/// Builds a `Set-Cookie` header value for the admin authentication JWT.
///
/// Cookie attributes:
/// - `HttpOnly`: not accessible via JavaScript (XSS protection)
/// - `SameSite=Strict`: never sent on cross-origin requests (CSRF protection)
/// - `Secure`: HTTPS-only (skipped for localhost development)
/// - `Path=/admin`: scoped to admin panel routes only
/// - `Max-Age=86400`: 24-hour expiry (aligned with JWT expiry)
///
/// # Arguments
///
/// * `token` - The JWT token string
/// * `is_secure` - Whether to add the `Secure` flag (false for localhost)
///
/// # Examples
///
/// ```
/// use reinhardt_admin::server::security::build_admin_auth_cookie;
///
/// let cookie = build_admin_auth_cookie("eyJhbGciOiJIUzI1NiJ9.test", true);
/// assert!(cookie.contains("HttpOnly"));
/// assert!(cookie.contains("SameSite=Strict"));
/// assert!(cookie.contains("Secure"));
/// assert!(cookie.contains("Path=/admin"));
/// ```
pub fn build_admin_auth_cookie(token: &str, is_secure: bool) -> String {
	let secure_flag = if is_secure { "; Secure" } else { "" };
	format!(
		"{}={}; HttpOnly; SameSite=Strict; Path=/admin; Max-Age=86400{}",
		ADMIN_AUTH_COOKIE_NAME, token, secure_flag
	)
}

/// Builds a `Set-Cookie` header value that clears the admin authentication cookie.
///
/// Sets `Max-Age=0` to instruct the browser to delete the cookie immediately.
///
/// # Examples
///
/// ```
/// use reinhardt_admin::server::security::build_admin_auth_cookie_clear;
///
/// let cookie = build_admin_auth_cookie_clear();
/// assert!(cookie.contains("Max-Age=0"));
/// ```
pub fn build_admin_auth_cookie_clear() -> String {
	format!(
		"{}=; HttpOnly; SameSite=Strict; Path=/admin; Max-Age=0",
		ADMIN_AUTH_COOKIE_NAME
	)
}

/// Extracts the admin JWT token from the `Cookie` header.
///
/// Parses the `Cookie` header and returns the value of the
/// `reinhardt_admin_token` cookie if present.
///
/// # Arguments
///
/// * `headers` - The HTTP request headers
#[cfg(not(target_arch = "wasm32"))]
pub fn extract_admin_auth_cookie(headers: &hyper::HeaderMap) -> Option<String> {
	headers
		.get("cookie")
		.and_then(|v| v.to_str().ok())
		.and_then(|cookie_header| {
			cookie_header.split(';').find_map(|pair| {
				let pair = pair.trim();
				let (name, value) = pair.split_once('=')?;
				if name.trim() == ADMIN_AUTH_COOKIE_NAME {
					Some(value.trim().to_string())
				} else {
					None
				}
			})
		})
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

	#[rstest]
	fn test_csp_admin_default_allows_wasm_eval() {
		// Arrange
		let csp = ContentSecurityPolicy::admin_default();

		// Act
		let csp_string = csp.to_header_value();

		// Assert
		assert!(
			csp_string.contains("'wasm-unsafe-eval'"),
			"CSP should allow WASM evaluation for admin SPA, got: {}",
			csp_string
		);
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

	// ============================================================
	// CSRF header extraction tests
	// ============================================================

	#[rstest]
	fn test_extract_csrf_header_present() {
		// Arrange
		let mut headers = hyper::HeaderMap::new();
		headers.insert("x-csrf-token", "test-token".parse().unwrap());

		// Act
		let result = extract_csrf_header(&headers);

		// Assert
		assert_eq!(result, Some("test-token".to_string()));
	}

	#[rstest]
	fn test_extract_csrf_header_missing() {
		// Arrange
		let headers = hyper::HeaderMap::new();

		// Act
		let result = extract_csrf_header(&headers);

		// Assert
		assert_eq!(result, None);
	}

	// ============================================================
	// CSRF cookie extraction tests
	// ============================================================

	#[rstest]
	fn test_extract_csrf_cookie_present() {
		// Arrange
		let mut headers = hyper::HeaderMap::new();
		headers.insert(
			"cookie",
			"session=abc; csrftoken=test-token-value; other=xyz"
				.parse()
				.unwrap(),
		);

		// Act
		let result = extract_csrf_cookie(&headers);

		// Assert
		assert_eq!(result, Some("test-token-value".to_string()));
	}

	#[rstest]
	fn test_extract_csrf_cookie_missing() {
		// Arrange
		let mut headers = hyper::HeaderMap::new();
		headers.insert("cookie", "session=abc; other=xyz".parse().unwrap());

		// Act
		let result = extract_csrf_cookie(&headers);

		// Assert
		assert_eq!(result, None);
	}

	#[rstest]
	fn test_extract_csrf_cookie_no_cookie_header() {
		// Arrange
		let headers = hyper::HeaderMap::new();

		// Act
		let result = extract_csrf_cookie(&headers);

		// Assert
		assert_eq!(result, None);
	}

	#[rstest]
	fn test_extract_csrf_cookie_only_csrf() {
		// Arrange
		let mut headers = hyper::HeaderMap::new();
		headers.insert("cookie", "csrftoken=solo-value".parse().unwrap());

		// Act
		let result = extract_csrf_cookie(&headers);

		// Assert
		assert_eq!(result, Some("solo-value".to_string()));
	}

	// ============================================================
	// build_csrf_cookie tests
	// ============================================================

	#[rstest]
	fn test_build_csrf_cookie_secure() {
		// Act
		let cookie = build_csrf_cookie("token123", true);

		// Assert
		assert_eq!(
			cookie,
			"csrftoken=token123; SameSite=Strict; Path=/admin; Secure"
		);
	}

	#[rstest]
	fn test_build_csrf_cookie_insecure() {
		// Act
		let cookie = build_csrf_cookie("token123", false);

		// Assert
		assert_eq!(cookie, "csrftoken=token123; SameSite=Strict; Path=/admin");
	}

	// ============================================================
	// require_csrf_token (cookie-based) tests
	// ============================================================

	#[rstest]
	fn test_require_csrf_token_matching_cookie() {
		// Arrange
		let token = generate_csrf_token();
		let mut headers = hyper::HeaderMap::new();
		let cookie_value = format!("csrftoken={}", token);
		headers.insert("cookie", cookie_value.parse().unwrap());

		// Act & Assert
		// unwrap() verifies success; panics with the error if validation fails
		require_csrf_token(&token, &headers).unwrap();
	}

	#[rstest]
	fn test_require_csrf_token_mismatching_cookie() {
		// Arrange
		let body_token = generate_csrf_token();
		let cookie_token = generate_csrf_token();
		let mut headers = hyper::HeaderMap::new();
		let cookie_value = format!("csrftoken={}", cookie_token);
		headers.insert("cookie", cookie_value.parse().unwrap());

		// Act
		let result = require_csrf_token(&body_token, &headers);

		// Assert
		let err = result.unwrap_err();
		match err {
			reinhardt_pages::server_fn::ServerFnError::Server { status, message } => {
				assert_eq!(status, 403);
				assert_eq!(message, "CSRF token validation failed");
			}
			other => panic!("Expected Server error with status 403, got: {:?}", other),
		}
	}

	#[rstest]
	fn test_require_csrf_token_missing_cookie() {
		// Arrange
		let body_token = generate_csrf_token();
		let headers = hyper::HeaderMap::new();

		// Act
		let result = require_csrf_token(&body_token, &headers);

		// Assert
		let err = result.unwrap_err();
		match err {
			reinhardt_pages::server_fn::ServerFnError::Server { status, message } => {
				assert_eq!(status, 403);
				assert_eq!(message, "CSRF token missing from cookie and header");
			}
			other => panic!("Expected Server error with status 403, got: {:?}", other),
		}
	}

	#[rstest]
	fn test_require_csrf_token_empty_body_token() {
		// Arrange
		let cookie_token = generate_csrf_token();
		let mut headers = hyper::HeaderMap::new();
		let cookie_value = format!("csrftoken={}", cookie_token);
		headers.insert("cookie", cookie_value.parse().unwrap());

		// Act
		let result = require_csrf_token("", &headers);

		// Assert
		let err = result.unwrap_err();
		match err {
			reinhardt_pages::server_fn::ServerFnError::Server { status, message } => {
				assert_eq!(status, 403);
				assert_eq!(message, "CSRF token validation failed");
			}
			other => panic!("Expected Server error with status 403, got: {:?}", other),
		}
	}

	// ============================================================
	// CSRF token uniqueness and entropy tests
	// ============================================================

	#[rstest]
	fn test_csrf_token_generation_uniqueness() {
		// Arrange
		let mut tokens = std::collections::HashSet::new();

		// Act
		for _ in 0..100 {
			let token = generate_csrf_token();
			tokens.insert(token);
		}

		// Assert
		assert_eq!(
			tokens.len(),
			100,
			"All 100 generated CSRF tokens should be unique"
		);
	}

	#[rstest]
	fn test_csrf_token_minimum_entropy() {
		// Act
		let token = generate_csrf_token();

		// Assert
		// CSRF_TOKEN_BYTES is 32, base64 encoding of 32 bytes = 43 chars (URL_SAFE_NO_PAD)
		assert!(
			token.len() >= 32,
			"CSRF token length {} should be at least 32 characters for sufficient entropy",
			token.len()
		);
	}

	// ============================================================
	// CSRF validation edge case tests
	// ============================================================

	#[rstest]
	fn test_csrf_validation_accepts_matching_tokens() {
		// Arrange
		let token = generate_csrf_token();
		let mut headers = hyper::HeaderMap::new();
		let cookie_value = format!("csrftoken={}", token);
		headers.insert("cookie", cookie_value.parse().unwrap());

		// Act
		let result = require_csrf_token(&token, &headers);

		// Assert
		assert!(
			result.is_ok(),
			"Matching tokens should pass CSRF validation"
		);
	}

	#[rstest]
	fn test_csrf_validation_rejects_empty_token() {
		// Arrange
		let cookie_token = generate_csrf_token();
		let mut headers = hyper::HeaderMap::new();
		let cookie_value = format!("csrftoken={}", cookie_token);
		headers.insert("cookie", cookie_value.parse().unwrap());

		// Act
		let result = require_csrf_token("", &headers);

		// Assert
		assert!(result.is_err(), "Empty body token should be rejected");
		let err = result.unwrap_err();
		match err {
			reinhardt_pages::server_fn::ServerFnError::Server { status, .. } => {
				assert_eq!(status, 403);
			}
			other => panic!("Expected Server error with status 403, got: {:?}", other),
		}
	}

	#[rstest]
	fn test_csrf_validation_rejects_whitespace_only_token() {
		// Arrange
		let cookie_token = generate_csrf_token();
		let mut headers = hyper::HeaderMap::new();
		let cookie_value = format!("csrftoken={}", cookie_token);
		headers.insert("cookie", cookie_value.parse().unwrap());

		// Act
		let result = require_csrf_token("   ", &headers);

		// Assert
		assert!(
			result.is_err(),
			"Whitespace-only body token should be rejected"
		);
	}

	// ============================================================
	// Sanitization additional tests
	// ============================================================

	#[rstest]
	fn test_sanitize_html_removes_script_tags() {
		// Arrange
		let mut data = HashMap::new();
		data.insert(
			"content".to_string(),
			serde_json::json!("<script>document.cookie</script>"),
		);

		// Act
		sanitize_mutation_values(&mut data);

		// Assert
		let content = data.get("content").unwrap().as_str().unwrap();
		assert!(
			!content.contains("<script>"),
			"Script tags should be escaped, got: {}",
			content
		);
		assert!(
			content.contains("&lt;script&gt;"),
			"Script tags should be HTML-escaped, got: {}",
			content
		);
	}

	#[rstest]
	#[case("hello world", "hello world")]
	#[case("", "")]
	#[case(
		"normal text without special chars",
		"normal text without special chars"
	)]
	fn test_sanitize_html_idempotent_safe_strings(#[case] input: &str, #[case] expected: &str) {
		// Arrange
		let mut data = HashMap::new();
		data.insert("val".to_string(), serde_json::json!(input));

		// Act — first pass
		sanitize_mutation_values(&mut data);
		let after_first = data.get("val").unwrap().as_str().unwrap().to_string();

		// Act — second pass on already-sanitized output
		let mut data2 = HashMap::new();
		data2.insert("val".to_string(), serde_json::json!(after_first));
		sanitize_mutation_values(&mut data2);
		let after_second = data2.get("val").unwrap().as_str().unwrap().to_string();

		// Assert — safe strings are unchanged through both passes
		assert_eq!(after_first, expected);
		assert_eq!(after_first, after_second);
	}

	#[rstest]
	#[case("<b>bold</b>", "&lt;b&gt;bold&lt;/b&gt;")]
	#[case("<script>alert(1)</script>", "&lt;script&gt;alert(1)&lt;/script&gt;")]
	fn test_sanitize_html_escapes_dangerous_input(
		#[case] input: &str,
		#[case] expected_escaped: &str,
	) {
		// Arrange
		let mut data = HashMap::new();
		data.insert("val".to_string(), serde_json::json!(input));

		// Act
		sanitize_mutation_values(&mut data);

		// Assert
		let result = data.get("val").unwrap().as_str().unwrap();
		assert_eq!(result, expected_escaped);
	}

	// ============================================================
	// Security headers count test
	// ============================================================

	#[rstest]
	fn test_security_headers_count() {
		// Arrange
		let headers = SecurityHeaders::default();

		// Act
		let map = headers.to_header_map();

		// Assert
		assert_eq!(
			map.len(),
			6,
			"SecurityHeaders should produce exactly 6 headers: CSP, X-Content-Type-Options, X-Frame-Options, X-XSS-Protection, Referrer-Policy, Permissions-Policy"
		);
	}

	// ============================================================
	// FrameOptions from_str tests
	// ============================================================

	#[rstest]
	fn test_frame_options_from_str_deny() {
		// Assert
		assert_eq!("deny".parse::<FrameOptions>().unwrap(), FrameOptions::Deny);
	}

	#[rstest]
	fn test_frame_options_from_str_deny_uppercase() {
		// Assert
		assert_eq!("DENY".parse::<FrameOptions>().unwrap(), FrameOptions::Deny);
	}

	#[rstest]
	fn test_frame_options_from_str_sameorigin() {
		// Assert
		assert_eq!(
			"sameorigin".parse::<FrameOptions>().unwrap(),
			FrameOptions::SameOrigin
		);
	}

	#[rstest]
	fn test_frame_options_from_str_unknown_falls_back_to_deny() {
		// Assert
		assert_eq!(
			"invalid".parse::<FrameOptions>().unwrap(),
			FrameOptions::Deny
		);
	}

	// ============================================================
	// ReferrerPolicy from_str tests
	// ============================================================

	#[rstest]
	fn test_referrer_policy_from_str_no_referrer() {
		// Assert
		assert_eq!(
			"no-referrer".parse::<ReferrerPolicy>().unwrap(),
			ReferrerPolicy::NoReferrer
		);
	}

	#[rstest]
	fn test_referrer_policy_from_str_strict_origin() {
		// Assert
		assert_eq!(
			"strict-origin-when-cross-origin"
				.parse::<ReferrerPolicy>()
				.unwrap(),
			ReferrerPolicy::StrictOriginWhenCrossOrigin
		);
	}

	#[rstest]
	fn test_referrer_policy_from_str_same_origin() {
		// Assert
		assert_eq!(
			"same-origin".parse::<ReferrerPolicy>().unwrap(),
			ReferrerPolicy::SameOrigin
		);
	}

	#[rstest]
	fn test_referrer_policy_from_str_unknown_falls_back() {
		// Assert
		assert_eq!(
			"invalid".parse::<ReferrerPolicy>().unwrap(),
			ReferrerPolicy::StrictOriginWhenCrossOrigin
		);
	}
}

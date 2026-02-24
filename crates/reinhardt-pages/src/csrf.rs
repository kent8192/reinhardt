//! CSRF Protection for Client-side WASM
//!
//! This module provides utilities for retrieving and injecting CSRF tokens
//! in client-side WASM applications. It integrates with Django's CSRF protection
//! mechanism and the `reinhardt-auth` session system.
//!
//! ## Token Retrieval
//!
//! CSRF tokens can be retrieved from multiple sources:
//! 1. **Cookie**: The `csrftoken` cookie set by Django
//! 2. **Meta tag**: `<meta name="csrf-token" content="...">` in the HTML head
//! 3. **Hidden input**: `<input name="csrfmiddlewaretoken">` in forms
//!
//! ## Server-side Verification
//!
//! CSRF tokens retrieved on the client must be verified on the server.
//! This module provides [`verify_csrf_token`] for constant-time token comparison,
//! which should be used in server-side middleware to validate tokens submitted
//! with requests.
//!
//! The typical verification flow is:
//!
//! 1. Server generates a CSRF token and stores it in the session/cookie
//! 2. Client retrieves the token via [`get_csrf_token`] or [`CsrfManager`]
//! 3. Client includes the token in requests (header or form field)
//! 4. Server-side middleware extracts the token from the request
//! 5. Server calls [`verify_csrf_token`] to compare the request token against
//!    the stored session token using constant-time comparison
//!
//! For HMAC-based token verification (recommended for production),
//! see `reinhardt_core::security::csrf`.
//!
//! ## Usage
//!
//! ```ignore
//! use reinhardt_pages::csrf::{get_csrf_token, CsrfManager, verify_csrf_token};
//!
//! // Client-side: retrieve token
//! if let Some(token) = get_csrf_token() {
//!     // Include token in API calls via X-CSRFToken header
//! }
//!
//! // Server-side: verify token
//! let is_valid = verify_csrf_token(&request_token, &session_token);
//! ```

use subtle::ConstantTimeEq;

use crate::reactive::Signal;

/// The cookie name used by Django for CSRF tokens.
pub const CSRF_COOKIE_NAME: &str = "csrftoken";

/// The meta tag name for CSRF tokens.
pub const CSRF_META_NAME: &str = "csrf-token";

/// The header name used to send CSRF tokens in AJAX requests.
pub const CSRF_HEADER_NAME: &str = "X-CSRFToken";

/// The form field name for CSRF tokens.
pub const CSRF_FORM_FIELD: &str = "csrfmiddlewaretoken";

/// CSRF token manager for client-side token handling.
///
/// This struct provides a reactive interface to CSRF token management,
/// with automatic caching and multiple retrieval strategies.
#[derive(Debug, Clone)]
pub struct CsrfManager {
	/// Cached CSRF token as a reactive Signal.
	token: Signal<Option<String>>,
}

impl Default for CsrfManager {
	fn default() -> Self {
		Self::new()
	}
}

impl CsrfManager {
	/// Creates a new CSRF manager.
	///
	/// The token is not fetched until explicitly requested.
	pub fn new() -> Self {
		Self {
			token: Signal::new(None),
		}
	}

	/// Gets the cached token without fetching from the browser.
	pub fn cached_token(&self) -> Option<String> {
		self.token.get()
	}

	/// Gets or fetches the CSRF token.
	///
	/// This method will:
	/// 1. Return the cached token if available
	/// 2. Otherwise, try to fetch from cookie, meta tag, or form input
	/// 3. Cache the result for future use
	#[cfg(target_arch = "wasm32")]
	pub fn get_or_fetch_token(&self) -> Option<String> {
		// Return cached if available
		if let Some(token) = self.token.get() {
			return Some(token);
		}

		// Try to fetch and cache
		if let Some(token) = get_csrf_token() {
			self.token.set(Some(token.clone()));
			return Some(token);
		}

		None
	}

	/// Gets or fetches the CSRF token (non-WASM stub).
	#[cfg(not(target_arch = "wasm32"))]
	pub fn get_or_fetch_token(&self) -> Option<String> {
		self.token.get()
	}

	/// Forces a refresh of the CSRF token from the browser.
	#[cfg(target_arch = "wasm32")]
	pub fn refresh(&self) -> Option<String> {
		if let Some(token) = get_csrf_token() {
			self.token.set(Some(token.clone()));
			return Some(token);
		}
		self.token.set(None);
		None
	}

	/// Forces a refresh of the CSRF token (non-WASM stub).
	#[cfg(not(target_arch = "wasm32"))]
	pub fn refresh(&self) -> Option<String> {
		None
	}

	/// Sets the token manually.
	///
	/// This is useful when the token is received from the server
	/// (e.g., in an initial page response).
	pub fn set_token(&self, token: impl Into<String>) {
		self.token.set(Some(token.into()));
	}

	/// Clears the cached token.
	pub fn clear(&self) {
		self.token.set(None);
	}

	/// Returns a Signal that tracks the current token.
	///
	/// This can be used for reactive UI updates when the token changes.
	pub fn token_signal(&self) -> Signal<Option<String>> {
		self.token.clone()
	}
}

/// Retrieves the CSRF token from the browser.
///
/// This function tries multiple sources in order:
/// 1. Cookie (`csrftoken`)
/// 2. Meta tag (`<meta name="csrf-token">`)
/// 3. Hidden form input (`<input name="csrfmiddlewaretoken">`)
///
/// Returns `None` if no token is found.
#[cfg(target_arch = "wasm32")]
pub fn get_csrf_token() -> Option<String> {
	// Try cookie first (most common)
	if let Some(token) = get_csrf_token_from_cookie() {
		return Some(token);
	}

	// Try meta tag
	if let Some(token) = get_csrf_token_from_meta() {
		return Some(token);
	}

	// Try hidden input as last resort
	get_csrf_token_from_input()
}

/// Retrieves the CSRF token (non-WASM stub).
#[cfg(not(target_arch = "wasm32"))]
pub fn get_csrf_token() -> Option<String> {
	None
}

/// Retrieves the CSRF token from the cookie.
#[cfg(target_arch = "wasm32")]
pub fn get_csrf_token_from_cookie() -> Option<String> {
	use wasm_bindgen::JsCast;
	use web_sys::{HtmlDocument, window};

	let window = window()?;
	let document = window.document()?;
	let html_doc = document.dyn_ref::<HtmlDocument>()?;
	let cookie_str = html_doc.cookie().ok()?;

	parse_cookie_value(&cookie_str, CSRF_COOKIE_NAME)
}

/// Retrieves the CSRF token from cookie (non-WASM stub).
#[cfg(not(target_arch = "wasm32"))]
pub fn get_csrf_token_from_cookie() -> Option<String> {
	None
}

/// Retrieves the CSRF token from a meta tag.
#[cfg(target_arch = "wasm32")]
pub fn get_csrf_token_from_meta() -> Option<String> {
	use web_sys::window;

	let window = window()?;
	let document = window.document()?;

	// Try <meta name="csrf-token" content="...">
	let selector = format!("meta[name=\"{}\"]", CSRF_META_NAME);
	let meta = document.query_selector(&selector).ok()??;
	meta.get_attribute("content")
}

/// Retrieves the CSRF token from meta tag (non-WASM stub).
#[cfg(not(target_arch = "wasm32"))]
pub fn get_csrf_token_from_meta() -> Option<String> {
	None
}

/// Retrieves the CSRF token from a hidden form input.
#[cfg(target_arch = "wasm32")]
pub fn get_csrf_token_from_input() -> Option<String> {
	use wasm_bindgen::JsCast;
	use web_sys::{HtmlInputElement, window};

	let window = window()?;
	let document = window.document()?;

	// Try <input name="csrfmiddlewaretoken">
	let selector = format!("input[name=\"{}\"]", CSRF_FORM_FIELD);
	let input = document.query_selector(&selector).ok()??;
	let input: HtmlInputElement = input.dyn_into().ok()?;
	Some(input.value())
}

/// Retrieves the CSRF token from hidden input (non-WASM stub).
#[cfg(not(target_arch = "wasm32"))]
pub fn get_csrf_token_from_input() -> Option<String> {
	None
}

/// Parses a cookie value from a cookie string.
///
/// The cookie string format is: "name1=value1; name2=value2; ..."
// Exposed as public for testing purposes
pub fn parse_cookie_value(cookie_str: &str, name: &str) -> Option<String> {
	for part in cookie_str.split(';') {
		let part = part.trim();
		if let Some((key, value)) = part.split_once('=')
			&& key.trim() == name
		{
			return Some(value.trim().to_string());
		}
	}
	None
}

/// Creates HTTP headers with CSRF token for AJAX requests.
///
/// Returns a tuple of (header_name, header_value) if a token is available.
#[cfg(target_arch = "wasm32")]
pub fn csrf_headers() -> Option<(&'static str, String)> {
	get_csrf_token().map(|token| (CSRF_HEADER_NAME, token))
}

/// Creates HTTP headers with CSRF token (non-WASM stub).
#[cfg(not(target_arch = "wasm32"))]
pub fn csrf_headers() -> Option<(&'static str, String)> {
	None
}

// ============================================================================
// Server-side verification utilities
// ============================================================================

/// Error returned when CSRF token verification fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CsrfVerificationError {
	/// The request token is empty.
	EmptyRequestToken,
	/// The session token is empty.
	EmptySessionToken,
	/// The tokens do not match.
	TokenMismatch,
}

impl std::fmt::Display for CsrfVerificationError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::EmptyRequestToken => write!(f, "CSRF request token is empty"),
			Self::EmptySessionToken => write!(f, "CSRF session token is empty"),
			Self::TokenMismatch => write!(f, "CSRF token mismatch"),
		}
	}
}

impl std::error::Error for CsrfVerificationError {}

/// Verifies a CSRF token using constant-time comparison.
///
/// This function compares the request token against the expected session token
/// using constant-time equality to prevent timing side-channel attacks.
///
/// # Arguments
///
/// * `request_token` - The CSRF token submitted with the request
/// * `session_token` - The expected CSRF token stored in the session/cookie
///
/// # Returns
///
/// `Ok(())` if the tokens match, or a [`CsrfVerificationError`] if verification fails.
///
/// # Security
///
/// This function uses the `subtle` crate's constant-time comparison to prevent
/// timing attacks. It should be used in server-side middleware to validate
/// CSRF tokens before processing state-changing requests.
///
/// For HMAC-based token verification (recommended for production), see
/// `reinhardt_core::security::csrf::verify_token_hmac`.
///
/// # Example
///
/// ```
/// use reinhardt_pages::csrf::{verify_csrf_token, CsrfVerificationError};
///
/// // Valid token
/// let session_token = "abc123def456";
/// let request_token = "abc123def456";
/// assert!(verify_csrf_token(request_token, session_token).is_ok());
///
/// // Invalid token
/// let bad_token = "wrong_token";
/// assert_eq!(
///     verify_csrf_token(bad_token, session_token),
///     Err(CsrfVerificationError::TokenMismatch),
/// );
///
/// // Empty token
/// assert_eq!(
///     verify_csrf_token("", session_token),
///     Err(CsrfVerificationError::EmptyRequestToken),
/// );
/// ```
pub fn verify_csrf_token(
	request_token: &str,
	session_token: &str,
) -> Result<(), CsrfVerificationError> {
	if request_token.is_empty() {
		return Err(CsrfVerificationError::EmptyRequestToken);
	}
	if session_token.is_empty() {
		return Err(CsrfVerificationError::EmptySessionToken);
	}

	// Use constant-time comparison to prevent timing attacks.
	// Compare raw bytes of the token strings.
	let request_bytes = request_token.as_bytes();
	let session_bytes = session_token.as_bytes();

	// Length difference alone leaks information, but checking length
	// before constant-time comparison is acceptable because the token
	// format (fixed-length hex) is public knowledge. For variable-length
	// tokens, the length mismatch itself reveals mismatch.
	if request_bytes.len() != session_bytes.len() {
		return Err(CsrfVerificationError::TokenMismatch);
	}

	if request_bytes.ct_eq(session_bytes).into() {
		Ok(())
	} else {
		Err(CsrfVerificationError::TokenMismatch)
	}
}

/// Trait for server-side CSRF verification middleware integration.
///
/// Implement this trait in your server-side middleware to provide CSRF
/// token extraction and verification. This bridges the client-side token
/// management in `reinhardt-pages` with server-side verification.
///
/// # Example
///
/// ```ignore
/// use reinhardt_pages::csrf::CsrfVerifier;
///
/// struct MyCsrfMiddleware;
///
/// impl CsrfVerifier for MyCsrfMiddleware {
///     fn extract_request_token(&self, request: &Request) -> Option<String> {
///         // Extract from X-CSRFToken header or form field
///         request.headers().get("X-CSRFToken")
///             .map(|v| v.to_str().ok().map(String::from))
///             .flatten()
///     }
///
///     fn extract_session_token(&self, request: &Request) -> Option<String> {
///         // Extract from session or cookie
///         request.cookies().get("csrftoken")
///             .map(|c| c.value().to_string())
///     }
/// }
/// ```
pub trait CsrfVerifier {
	/// Extracts the CSRF token from the incoming request.
	///
	/// Typically reads from the `X-CSRFToken` header or the
	/// `csrfmiddlewaretoken` form field.
	fn extract_request_token(&self, request: &[u8]) -> Option<String>;

	/// Extracts the expected CSRF token from the session or cookie.
	fn extract_session_token(&self, request: &[u8]) -> Option<String>;

	/// Verifies the CSRF token for the given request.
	///
	/// Default implementation extracts both tokens and uses
	/// [`verify_csrf_token`] for constant-time comparison.
	fn verify(&self, request: &[u8]) -> Result<(), CsrfVerificationError> {
		let request_token = self
			.extract_request_token(request)
			.ok_or(CsrfVerificationError::EmptyRequestToken)?;
		let session_token = self
			.extract_session_token(request)
			.ok_or(CsrfVerificationError::EmptySessionToken)?;
		verify_csrf_token(&request_token, &session_token)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_parse_cookie_value() {
		let cookie_str = "sessionid=abc123; csrftoken=xyz789; other=value";
		assert_eq!(
			parse_cookie_value(cookie_str, "csrftoken"),
			Some("xyz789".to_string())
		);
		assert_eq!(
			parse_cookie_value(cookie_str, "sessionid"),
			Some("abc123".to_string())
		);
		assert_eq!(parse_cookie_value(cookie_str, "nonexistent"), None);
	}

	#[test]
	fn test_parse_cookie_value_with_spaces() {
		let cookie_str = " csrftoken = token123 ; other = value ";
		assert_eq!(
			parse_cookie_value(cookie_str, "csrftoken"),
			Some("token123".to_string())
		);
	}

	#[test]
	fn test_csrf_manager_creation() {
		let manager = CsrfManager::new();
		assert!(manager.cached_token().is_none());
	}

	#[test]
	fn test_csrf_manager_set_token() {
		let manager = CsrfManager::new();
		manager.set_token("test-token");
		assert_eq!(manager.cached_token(), Some("test-token".to_string()));
	}

	#[test]
	fn test_csrf_manager_clear() {
		let manager = CsrfManager::new();
		manager.set_token("test-token");
		manager.clear();
		assert!(manager.cached_token().is_none());
	}

	#[test]
	fn test_csrf_manager_default() {
		let manager = CsrfManager::default();
		assert!(manager.cached_token().is_none());
	}

	#[test]
	fn test_verify_csrf_token_valid() {
		let token = "abc123def456";
		assert!(verify_csrf_token(token, token).is_ok());
	}

	#[test]
	fn test_verify_csrf_token_mismatch() {
		assert_eq!(
			verify_csrf_token("token_a", "token_b"),
			Err(CsrfVerificationError::TokenMismatch),
		);
	}

	#[test]
	fn test_verify_csrf_token_different_length() {
		assert_eq!(
			verify_csrf_token("short", "much_longer_token"),
			Err(CsrfVerificationError::TokenMismatch),
		);
	}

	#[test]
	fn test_verify_csrf_token_empty_request() {
		assert_eq!(
			verify_csrf_token("", "session_token"),
			Err(CsrfVerificationError::EmptyRequestToken),
		);
	}

	#[test]
	fn test_verify_csrf_token_empty_session() {
		assert_eq!(
			verify_csrf_token("request_token", ""),
			Err(CsrfVerificationError::EmptySessionToken),
		);
	}

	#[test]
	fn test_verify_csrf_token_both_empty() {
		assert_eq!(
			verify_csrf_token("", ""),
			Err(CsrfVerificationError::EmptyRequestToken),
		);
	}

	#[test]
	fn test_csrf_verification_error_display() {
		assert_eq!(
			CsrfVerificationError::EmptyRequestToken.to_string(),
			"CSRF request token is empty",
		);
		assert_eq!(
			CsrfVerificationError::EmptySessionToken.to_string(),
			"CSRF session token is empty",
		);
		assert_eq!(
			CsrfVerificationError::TokenMismatch.to_string(),
			"CSRF token mismatch",
		);
	}
}

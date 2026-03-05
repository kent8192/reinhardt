//! Authentication Security Integration Tests
//!
//! Tests HTTP authentication mechanisms
//! Based on FastAPI's test_security_*.py tests

use reinhardt_test::http::*;

use base64::{Engine as _, engine::general_purpose};
use hyper::StatusCode;
use hyper::header::{AUTHORIZATION, HeaderName, HeaderValue, WWW_AUTHENTICATE};

/// Create Basic Auth header value
fn create_basic_auth(username: &str, password: &str) -> String {
	let credentials = format!("{}:{}", username, password);
	let encoded = general_purpose::STANDARD.encode(credentials.as_bytes());
	format!("Basic {}", encoded)
}

/// Create Bearer Auth header value
fn create_bearer_token(token: &str) -> String {
	format!("Bearer {}", token)
}

/// Extract credentials from Basic Auth header
fn parse_basic_auth(auth_header: &str) -> Option<(String, String)> {
	let encoded = auth_header.strip_prefix("Basic ")?;
	let decoded = general_purpose::STANDARD.decode(encoded).ok()?;
	let credentials = String::from_utf8(decoded).ok()?;
	let mut parts = credentials.splitn(2, ':');
	let username = parts.next()?.to_string();
	let password = parts.next()?.to_string();
	Some((username, password))
}

#[test]
fn test_basic_auth_encoding() {
	// Test intent: Verify create_basic_auth() properly base64-encodes
	// username:password credentials with "Basic " prefix
	// Not intent: Decoding/parsing, special characters, actual HTTP auth
	let auth = create_basic_auth("john", "secret");
	assert!(auth.starts_with("Basic "));
	assert!(auth.len() > 6); // "Basic " + base64
}

#[test]
fn test_basic_auth_parsing() {
	// Test intent: Verify parse_basic_auth() correctly decodes base64
	// credentials and extracts username and password separated by colon
	// Not intent: Invalid formats, special characters, empty values
	let auth = create_basic_auth("alice", "password123");
	let (username, password) = parse_basic_auth(&auth).unwrap();
	assert_eq!(username, "alice");
	assert_eq!(password, "password123");
}

#[test]
fn test_basic_auth_with_special_chars() {
	// Test intent: Verify Basic auth encoding/decoding handles special
	// characters in password including @ : ! symbols
	// Not intent: Unicode characters, extremely long passwords, username special chars
	let auth = create_basic_auth("user", "p@ss:w0rd!");
	let (username, password) = parse_basic_auth(&auth).unwrap();
	assert_eq!(username, "user");
	assert_eq!(password, "p@ss:w0rd!");
}

#[test]
fn test_basic_auth_invalid_format() {
	// Test intent: Verify parse_basic_auth() returns None for invalid
	// base64 encoding (contains invalid characters like !)
	// Not intent: Missing prefix, empty string, correct base64 with wrong structure
	let invalid_auth = "Basic notbase64!!!";
	let result = parse_basic_auth(invalid_auth);
	assert!(result.is_none());
}

#[test]
fn test_basic_auth_missing_colon() {
	// Test intent: Verify parse_basic_auth() returns None when decoded
	// credentials lack colon separator between username and password
	// Not intent: Multiple colons, empty credentials, whitespace handling
	let credentials = "johnsecret"; // Missing colon
	let encoded = general_purpose::STANDARD.encode(credentials.as_bytes());
	let auth = format!("Basic {}", encoded);
	let result = parse_basic_auth(&auth);
	assert!(result.is_none());
}

#[test]
fn test_bearer_token_format() {
	// Test intent: Verify create_bearer_token() correctly formats
	// token with "Bearer " prefix and exact token value
	// Not intent: Token validation, JWT parsing, token expiry
	let token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9";
	let auth = create_bearer_token(token);
	assert_eq!(auth, format!("Bearer {}", token));
}

#[test]
fn test_bearer_token_extraction() {
	// Test intent: Verify Bearer token can be extracted by stripping
	// "Bearer " prefix from authorization header value
	// Not intent: Case sensitivity, missing prefix, token validation
	let token = "abc123def456";
	let auth = create_bearer_token(token);
	let extracted = auth.strip_prefix("Bearer ").unwrap();
	assert_eq!(extracted, token);
}

#[test]
fn test_authorization_header_basic() {
	// Test intent: Verify Basic auth can be added to HTTP request
	// Authorization header and retrieved correctly
	// Not intent: Server-side validation, actual authentication, header conflicts
	let mut request = create_test_request("GET", "/api/users", true);
	let auth = create_basic_auth("testuser", "testpass");
	request
		.headers
		.insert(AUTHORIZATION, HeaderValue::from_str(&auth).unwrap());

	let auth_header = request.headers.get(AUTHORIZATION).unwrap();
	assert!(auth_header.to_str().unwrap().starts_with("Basic "));
}

#[test]
fn test_authorization_header_bearer() {
	// Test intent: Verify Bearer token can be added to HTTP request
	// Authorization header and retrieved with correct prefix
	// Not intent: Token validation, JWT verification, header overwriting
	let mut request = create_test_request("GET", "/api/users", true);
	let auth = create_bearer_token("my_token_123");
	request
		.headers
		.insert(AUTHORIZATION, HeaderValue::from_str(&auth).unwrap());

	let auth_header = request.headers.get(AUTHORIZATION).unwrap();
	assert!(auth_header.to_str().unwrap().starts_with("Bearer "));
}

#[test]
fn test_www_authenticate_header() {
	// Test intent: Verify WWW-Authenticate header can be added to 401
	// response and contains Basic realm specification
	// Not intent: Multiple auth schemes, challenge parsing, response body
	let mut response = create_response_with_status(StatusCode::UNAUTHORIZED);
	response.headers.insert(
		WWW_AUTHENTICATE,
		HeaderValue::from_static("Basic realm=\"API\""),
	);

	assert_has_header(&response, "www-authenticate");
	assert_header_contains(&response, "www-authenticate", "Basic");
}

#[test]
fn test_multiple_auth_schemes() {
	// Test intent: Verify WWW-Authenticate header can advertise multiple
	// authentication schemes (Basic and Bearer) in single header value
	// Not intent: Scheme preference order, client-side parsing, scheme parameters
	let mut response = create_response_with_status(StatusCode::UNAUTHORIZED);
	response.headers.insert(
		WWW_AUTHENTICATE,
		HeaderValue::from_static("Basic realm=\"API\", Bearer"),
	);

	let auth_header = get_header(&response, "www-authenticate").unwrap();
	assert!(auth_header.contains("Basic"));
	assert!(auth_header.contains("Bearer"));
}

#[test]
fn test_api_key_in_header() {
	// Test intent: Verify custom X-API-Key header can be added to
	// request and retrieved with exact value
	// Not intent: API key validation, rate limiting, header case sensitivity
	let mut request = create_test_request("GET", "/api/data", true);
	request.headers.insert(
		HeaderName::from_static("x-api-key"),
		HeaderValue::from_static("my-api-key-123"),
	);

	assert!(request.headers.contains_key("x-api-key"));
	let api_key = request.headers.get("x-api-key").unwrap();
	assert_eq!(api_key.to_str().unwrap(), "my-api-key-123");
}

#[test]
fn test_api_key_in_query() {
	// Test intent: Verify API key can be passed via URL query parameter
	// and retrieved from request URI
	// Not intent: Query parameter parsing, URL encoding, key validation
	let request = create_test_request("GET", "/api/data?api_key=secret123", true);
	let query = request.uri.query().unwrap();
	assert!(query.contains("api_key=secret123"));
}

#[test]
fn test_api_key_in_cookie() {
	// Test intent: Verify API key can be passed via Cookie header
	// and retrieved from request headers
	// Not intent: Cookie parsing, secure/httponly flags, cookie expiry
	let mut request = create_test_request("GET", "/api/data", true);
	request.headers.insert(
		hyper::header::COOKIE,
		HeaderValue::from_static("api_key=cookie_key_456"),
	);

	let cookie = request.headers.get("cookie").unwrap();
	assert!(cookie.to_str().unwrap().contains("api_key="));
}

#[test]
fn test_empty_authorization_header() {
	// Test intent: Verify empty Authorization header can be set and
	// retrieved as empty string
	// Not intent: Server-side validation, error handling, default values
	let mut request = create_test_request("GET", "/api/test", true);
	request
		.headers
		.insert(AUTHORIZATION, HeaderValue::from_static(""));

	let auth = request.headers.get(AUTHORIZATION).unwrap();
	assert_eq!(auth.to_str().unwrap(), "");
}

#[test]
fn test_malformed_basic_auth() {
	// Test intent: Verify malformed Basic auth header (missing space
	// and credentials) can be set and retrieved as-is
	// Not intent: Server-side validation, error messages, auto-correction
	let mut request = create_test_request("GET", "/api/test", true);
	request
		.headers
		.insert(AUTHORIZATION, HeaderValue::from_static("Basic"));

	let auth = request
		.headers
		.get(AUTHORIZATION)
		.unwrap()
		.to_str()
		.unwrap();
	assert_eq!(auth, "Basic"); // Missing space and credentials
}

#[test]
fn test_case_sensitivity_auth_scheme() {
	// Test intent: Verify auth scheme names (Basic, Bearer) can be
	// case-insensitive and normalized to lowercase for comparison
	// Not intent: Server-side parsing, RFC compliance, mixed-case handling
	let schemes = ["Basic", "basic", "BASIC", "Bearer", "bearer", "BEARER"];
	for scheme in &schemes {
		let auth = format!("{} credentials", scheme);
		assert!(
			auth.to_lowercase().starts_with("basic") || auth.to_lowercase().starts_with("bearer")
		);
	}
}

#[test]
fn test_basic_auth_with_empty_password() {
	// Test intent: Verify Basic auth encoding/decoding handles empty
	// password string correctly with username:colon:empty pattern
	// Not intent: Password validation, security implications, null vs empty
	let auth = create_basic_auth("user", "");
	let (username, password) = parse_basic_auth(&auth).unwrap();
	assert_eq!(username, "user");
	assert_eq!(password, "");
}

#[test]
fn test_basic_auth_with_empty_username() {
	// Test intent: Verify Basic auth encoding/decoding handles empty
	// username string correctly with empty:colon:password pattern
	// Not intent: Username validation, security implications, anonymous access
	let auth = create_basic_auth("", "password");
	let (username, password) = parse_basic_auth(&auth).unwrap();
	assert_eq!(username, "");
	assert_eq!(password, "password");
}

#[test]
fn test_bearer_token_with_special_chars() {
	// Test intent: Verify Bearer token creation handles JWT-style tokens
	// with dots, underscores, and dashes in token value
	// Not intent: JWT validation, signature verification, token decoding
	let token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
	let auth = create_bearer_token(token);
	assert!(auth.contains(token));
}

#[test]
fn test_authorization_header_with_extra_spaces() {
	// Test intent: Verify authorization header string with extra spaces
	// between scheme and credentials still starts with "Basic"
	// Not intent: Whitespace normalization, parsing validation, error handling
	let auth = "Basic  dXNlcjpwYXNz"; // Extra space
	// This should be invalid or handled gracefully
	assert!(auth.starts_with("Basic"));
}

#[test]
fn test_unauthorized_response_format() {
	// Test intent: Verify 401 Unauthorized response can include
	// WWW-Authenticate header with realm specification
	// Not intent: Multiple challenges, response body, authentication flow
	let mut response = create_response_with_status(StatusCode::UNAUTHORIZED);
	response.headers.insert(
		WWW_AUTHENTICATE,
		HeaderValue::from_static("Basic realm=\"Secure Area\""),
	);

	assert_status(&response, StatusCode::UNAUTHORIZED);
	assert_has_header(&response, "www-authenticate");
}

#[test]
fn test_forbidden_vs_unauthorized() {
	// Test intent: Verify distinction between 401 (no/invalid authentication)
	// and 403 (valid auth but insufficient permissions) status codes
	// Not intent: Authorization logic, permission checking, error messages
	let unauthorized = create_response_with_status(StatusCode::UNAUTHORIZED);
	let forbidden = create_response_with_status(StatusCode::FORBIDDEN);

	assert_eq!(unauthorized.status, StatusCode::UNAUTHORIZED); // No/invalid auth
	assert_eq!(forbidden.status, StatusCode::FORBIDDEN); // Valid auth, insufficient permissions
}

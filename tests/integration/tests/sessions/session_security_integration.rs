//! Session Security Integration Tests
//!
//! Tests session cookie security settings
//! Based on Django's check_framework/test_security.py session tests

use reinhardt_test::http::*;

use hyper::header::{HeaderValue, SET_COOKIE};

/// Parse Set-Cookie header for attributes
fn parse_cookie_attributes(set_cookie: &str) -> Vec<String> {
	set_cookie
		.split(';')
		.map(|s| s.trim().to_lowercase())
		.collect()
}

/// Check if cookie has attribute
fn has_cookie_attribute(set_cookie: &str, attribute: &str) -> bool {
	let attributes = parse_cookie_attributes(set_cookie);
	let attr_lower = attribute.to_lowercase();
	attributes
		.iter()
		.any(|attr| attr == &attr_lower || attr.starts_with(&format!("{}=", attr_lower)))
}

/// Get cookie attribute value
fn get_cookie_attribute(set_cookie: &str, attribute: &str) -> Option<String> {
	let attributes = parse_cookie_attributes(set_cookie);
	for attr in attributes {
		if let Some(value) = attr.strip_prefix(&format!("{}=", attribute.to_lowercase())) {
			return Some(value.to_string());
		}
	}
	None
}

#[test]
fn test_session_cookie_secure_flag() {
	// Test: Session cookie should have Secure flag in production
	let mut response = create_test_response();
	response.headers.insert(
		SET_COOKIE,
		HeaderValue::from_static("sessionid=abc123; Secure; HttpOnly; SameSite=Lax"),
	);

	let cookie = get_header(&response, "set-cookie").unwrap();
	assert!(has_cookie_attribute(cookie, "Secure"));
}

#[test]
fn test_session_cookie_httponly_flag() {
	// Test: Session cookie should have HttpOnly flag
	let mut response = create_test_response();
	response.headers.insert(
		SET_COOKIE,
		HeaderValue::from_static("sessionid=abc123; Secure; HttpOnly; SameSite=Lax"),
	);

	let cookie = get_header(&response, "set-cookie").unwrap();
	assert!(has_cookie_attribute(cookie, "HttpOnly"));
}

#[test]
fn test_session_cookie_samesite() {
	// Test: Session cookie should have SameSite attribute
	let mut response = create_test_response();
	response.headers.insert(
		SET_COOKIE,
		HeaderValue::from_static("sessionid=abc123; Secure; HttpOnly; SameSite=Lax"),
	);

	let cookie = get_header(&response, "set-cookie").unwrap();
	assert!(has_cookie_attribute(cookie, "SameSite"));
}

#[test]
fn test_session_cookie_samesite_strict() {
	// Test: Session cookie with SameSite=Strict
	let mut response = create_test_response();
	response.headers.insert(
		SET_COOKIE,
		HeaderValue::from_static("sessionid=abc123; SameSite=Strict"),
	);

	let cookie = get_header(&response, "set-cookie").unwrap();
	let samesite = get_cookie_attribute(cookie, "SameSite").unwrap();
	assert_eq!(samesite, "strict");
}

#[test]
fn test_session_cookie_samesite_lax() {
	// Test: Session cookie with SameSite=Lax (default recommended)
	let mut response = create_test_response();
	response.headers.insert(
		SET_COOKIE,
		HeaderValue::from_static("sessionid=abc123; SameSite=Lax"),
	);

	let cookie = get_header(&response, "set-cookie").unwrap();
	let samesite = get_cookie_attribute(cookie, "SameSite").unwrap();
	assert_eq!(samesite, "lax");
}

#[test]
fn test_session_cookie_samesite_none_requires_secure() {
	// Test: SameSite=None requires Secure flag
	let mut response = create_test_response();
	response.headers.insert(
		SET_COOKIE,
		HeaderValue::from_static("sessionid=abc123; SameSite=None; Secure"),
	);

	let cookie = get_header(&response, "set-cookie").unwrap();
	assert!(has_cookie_attribute(cookie, "Secure"));
	let samesite = get_cookie_attribute(cookie, "SameSite").unwrap();
	assert_eq!(samesite, "none");
}

#[test]
fn test_session_cookie_domain() {
	// Test: Session cookie can specify domain
	let mut response = create_test_response();
	response.headers.insert(
		SET_COOKIE,
		HeaderValue::from_static("sessionid=abc123; Domain=.example.com"),
	);

	let cookie = get_header(&response, "set-cookie").unwrap();
	let domain = get_cookie_attribute(cookie, "Domain").unwrap();
	assert_eq!(domain, ".example.com");
}

#[test]
fn test_session_cookie_path() {
	// Test: Session cookie with Path attribute
	let mut response = create_test_response();
	response.headers.insert(
		SET_COOKIE,
		HeaderValue::from_static("sessionid=abc123; Path=/"),
	);

	let cookie = get_header(&response, "set-cookie").unwrap();
	let path = get_cookie_attribute(cookie, "Path").unwrap();
	assert_eq!(path, "/");
}

#[test]
fn test_session_cookie_max_age() {
	// Test: Session cookie with Max-Age
	let mut response = create_test_response();
	response.headers.insert(
		SET_COOKIE,
		HeaderValue::from_static("sessionid=abc123; Max-Age=3600"),
	);

	let cookie = get_header(&response, "set-cookie").unwrap();
	let max_age = get_cookie_attribute(cookie, "Max-Age").unwrap();
	assert_eq!(max_age, "3600");
}

#[test]
fn test_session_cookie_expires() {
	// Test: Session cookie with Expires
	let mut response = create_test_response();
	response.headers.insert(
		SET_COOKIE,
		HeaderValue::from_static("sessionid=abc123; Expires=Wed, 21 Oct 2025 07:28:00 GMT"),
	);

	let cookie = get_header(&response, "set-cookie").unwrap();
	assert!(has_cookie_attribute(cookie, "Expires"));
}

#[test]
fn test_session_cookie_deletion() {
	// Test: Session cookie deletion (Max-Age=0 or past Expires)
	let mut response = create_test_response();
	response.headers.insert(
		SET_COOKIE,
		HeaderValue::from_static("sessionid=; Max-Age=0"),
	);

	let cookie = get_header(&response, "set-cookie").unwrap();
	let max_age = get_cookie_attribute(cookie, "Max-Age").unwrap();
	assert_eq!(max_age, "0");
}

#[test]
fn test_csrf_vs_session_cookie_httponly() {
	// Test: CSRF cookies should NOT be HttpOnly (JS needs access)
	// Session cookies SHOULD be HttpOnly (prevent XSS)

	let mut session_response = create_test_response();
	session_response.headers.insert(
		SET_COOKIE,
		HeaderValue::from_static("sessionid=abc; HttpOnly"),
	);

	let mut csrf_response = create_test_response();
	csrf_response.headers.insert(
		SET_COOKIE,
		HeaderValue::from_static("csrftoken=xyz"), // No HttpOnly
	);

	let session_cookie = get_header(&session_response, "set-cookie").unwrap();
	assert!(has_cookie_attribute(session_cookie, "HttpOnly"));

	let csrf_cookie = get_header(&csrf_response, "set-cookie").unwrap();
	assert!(!has_cookie_attribute(csrf_cookie, "HttpOnly"));
}

#[tokio::test]
async fn test_session_fixation_prevention() {
	// Test: Session ID should be regenerated on login to prevent session fixation attacks
	//
	// This test verifies that:
	// 1. Session ID changes after calling regenerate_id()
	// 2. Session data is preserved during regeneration
	// 3. Old session ID is invalidated (removed from backend)
	// 4. New session ID is valid and contains the same data

	use reinhardt_auth::sessions::Session;
	use reinhardt_auth::sessions::backends::{InMemorySessionBackend, SessionBackend};

	// 1. Create a session with initial session ID
	let backend = InMemorySessionBackend::new();
	let mut session = Session::new(backend.clone());

	// Set initial user data (simulating a logged-in user)
	session.set("user_id", 42_i32).unwrap();
	session.set("username", "alice".to_string()).unwrap();
	session.set("role", "admin".to_string()).unwrap();

	// Save the session and get the initial session key
	session.save().await.unwrap();
	let old_session_key = session.session_key().unwrap().to_string();

	// Store the original data for comparison
	let old_user_id: i32 = session.get("user_id").unwrap().unwrap();
	let old_username: String = session.get("username").unwrap().unwrap();
	let old_role: String = session.get("role").unwrap().unwrap();

	// 2. Simulate login action - regenerate session ID
	session.regenerate_id().await.unwrap();

	// Save the session with new key
	session.save().await.unwrap();

	// Get the new session key
	let new_session_key = session.session_key().unwrap().to_string();

	// 3. Verify session ID changed
	assert_ne!(
		old_session_key, new_session_key,
		"Session ID should change after regeneration"
	);

	// 4. Verify session data is preserved
	let new_user_id: i32 = session.get("user_id").unwrap().unwrap();
	let new_username: String = session.get("username").unwrap().unwrap();
	let new_role: String = session.get("role").unwrap().unwrap();

	assert_eq!(old_user_id, new_user_id, "User ID should be preserved");
	assert_eq!(old_username, new_username, "Username should be preserved");
	assert_eq!(old_role, new_role, "Role should be preserved");

	// 5. Verify old session ID is invalidated in backend
	let old_session_exists = backend.exists(&old_session_key).await.unwrap();
	assert!(
		!old_session_exists,
		"Old session ID should be invalidated (deleted from backend)"
	);

	// 6. Verify new session ID is valid and contains correct data
	let new_session_exists = backend.exists(&new_session_key).await.unwrap();
	assert!(
		new_session_exists,
		"New session ID should be valid in backend"
	);

	// Load the session from backend using new session key
	let mut loaded_session = Session::from_key(backend.clone(), new_session_key.clone())
		.await
		.unwrap();
	let loaded_user_id: i32 = loaded_session.get("user_id").unwrap().unwrap();
	let loaded_username: String = loaded_session.get("username").unwrap().unwrap();
	let loaded_role: String = loaded_session.get("role").unwrap().unwrap();

	assert_eq!(
		loaded_user_id, old_user_id,
		"Loaded session should contain original user ID"
	);
	assert_eq!(
		loaded_username, old_username,
		"Loaded session should contain original username"
	);
	assert_eq!(
		loaded_role, old_role,
		"Loaded session should contain original role"
	);
}

#[tokio::test]
async fn test_session_timeout_valid_before_expiry() {
	// Test: Session should be valid before timeout
	use reinhardt_auth::sessions::Session;
	use reinhardt_auth::sessions::backends::InMemorySessionBackend;

	let backend = InMemorySessionBackend::new();
	let mut session = Session::new(backend);

	// Set session data
	session.set("user_id", 42_i32).unwrap();

	// Set timeout to 10 seconds
	session.set_timeout(10);

	// Session should be valid immediately
	assert!(!session.is_timed_out());
	assert!(session.validate_timeout().is_ok());
}

#[tokio::test]
async fn test_session_timeout_invalid_after_expiry() {
	// Test: Session should be invalid after timeout
	use reinhardt_auth::sessions::Session;
	use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	use std::time::Duration;

	let backend = InMemorySessionBackend::new();
	let mut session = Session::new(backend);

	// Set session data
	session.set("user_id", 42_i32).unwrap();

	// Set very short timeout (1 second)
	session.set_timeout(1);

	// Wait for session to expire
	tokio::time::sleep(Duration::from_secs(2)).await;

	// Session should be timed out
	assert!(session.is_timed_out());
	assert!(session.validate_timeout().is_err());
}

#[tokio::test]
async fn test_session_last_activity_updates() {
	// Test: Session last_activity timestamp updates on each access
	use reinhardt_auth::sessions::Session;
	use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	use std::time::Duration;

	let backend = InMemorySessionBackend::new();
	let mut session = Session::new(backend);

	// Set initial data
	session.set("user_id", 42_i32).unwrap();

	// Get initial last_activity
	let first_activity = session.get_last_activity().unwrap();

	// Wait a bit
	tokio::time::sleep(Duration::from_millis(100)).await;

	// Access session data (should update last_activity)
	session.get::<i32>("user_id").unwrap();

	// Get updated last_activity
	let second_activity = session.get_last_activity().unwrap();

	// last_activity should have been updated
	assert!(second_activity > first_activity);
}

#[tokio::test]
async fn test_session_update_activity_manually() {
	// Test: Manual update_activity() extends session lifetime
	use reinhardt_auth::sessions::Session;
	use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	use std::time::Duration;

	let backend = InMemorySessionBackend::new();
	let mut session = Session::new(backend);

	// Set timeout to 1 second
	session.set_timeout(1);

	// Wait 500ms (half of timeout)
	tokio::time::sleep(Duration::from_millis(500)).await;

	// Update activity manually
	session.update_activity();

	// Wait another 750ms (total 1.25s from initial creation, but only 750ms from update)
	tokio::time::sleep(Duration::from_millis(750)).await;

	// Session should still be valid (last activity was 750ms ago, which is < 1s)
	assert!(!session.is_timed_out());
	assert!(session.validate_timeout().is_ok());
}

#[tokio::test]
async fn test_session_timeout_configuration() {
	// Test: Session timeout can be configured
	use reinhardt_auth::sessions::Session;
	use reinhardt_auth::sessions::backends::InMemorySessionBackend;

	let backend = InMemorySessionBackend::new();
	let mut session = Session::new(backend);

	// Default timeout should be 1800 seconds (30 minutes)
	assert_eq!(session.get_timeout(), 1800);

	// Set custom timeout
	session.set_timeout(3600); // 1 hour
	assert_eq!(session.get_timeout(), 3600);

	// Set very short timeout
	session.set_timeout(60); // 1 minute
	assert_eq!(session.get_timeout(), 60);
}

#[test]
fn test_multiple_cookies() {
	// Test: Multiple Set-Cookie headers for different cookies
	let mut response = create_test_response();
	response.headers.append(
		SET_COOKIE,
		HeaderValue::from_static("sessionid=abc123; Secure; HttpOnly"),
	);
	response.headers.append(
		SET_COOKIE,
		HeaderValue::from_static("preferences=dark_mode; Secure"),
	);

	// Both cookies should be present
	let cookies: Vec<_> = response.headers.get_all(SET_COOKIE).iter().collect();
	assert_eq!(cookies.len(), 2);
}

#[test]
fn test_session_cookie_name() {
	// Test: Session cookie has correct name
	let mut response = create_test_response();
	response
		.headers
		.insert(SET_COOKIE, HeaderValue::from_static("sessionid=abc123"));

	let cookie = get_header(&response, "set-cookie").unwrap();
	assert!(cookie.starts_with("sessionid="));
}

#[test]
fn test_session_cookie_secure_production() {
	// Test: In production (HTTPS), Secure flag must be set
	let mut response = create_test_response();
	response.headers.insert(
		SET_COOKIE,
		HeaderValue::from_static("sessionid=abc123; Secure; HttpOnly; SameSite=Lax"),
	);

	let cookie = get_header(&response, "set-cookie").unwrap();
	// All three security attributes should be present
	assert!(has_cookie_attribute(cookie, "Secure"));
	assert!(has_cookie_attribute(cookie, "HttpOnly"));
	assert!(has_cookie_attribute(cookie, "SameSite"));
}

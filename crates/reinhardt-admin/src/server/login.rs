//! Admin Login Server Function
//!
//! Provides JWT-based authentication for the admin WASM SPA.
//!
//! On successful login, the JWT token is set as an HTTP-Only cookie
//! (`reinhardt_admin_token`) instead of being returned in the response body.
//! This prevents XSS attacks from stealing the token.

use crate::adapters::LoginResponse;
use reinhardt_pages::server_fn::{ServerFnError, server_fn};

#[cfg(server)]
use super::admin_auth::AdminLoginAuthenticator;
#[cfg(server)]
use super::security::{build_admin_auth_cookie, require_csrf_token};
#[cfg(server)]
use crate::adapters::AdminSite;
#[cfg(server)]
use reinhardt_auth::JwtAuth;
#[cfg(server)]
use reinhardt_db::orm::DatabaseConnection;
#[cfg(server)]
use reinhardt_di::Depends;
#[cfg(server)]
use reinhardt_pages::server_fn::ServerFnRequest;

/// Authenticate an admin user and set a JWT cookie.
///
/// This server function validates the provided credentials against the
/// database and, on success, sets the JWT token as an HTTP-Only cookie.
/// The browser automatically attaches this cookie to subsequent requests,
/// eliminating the need for sessionStorage token management.
///
/// # Authentication Flow
///
/// 1. Validate CSRF token (double-submit cookie pattern)
/// 2. Look up user by username in the database
/// 3. Verify password using Argon2id
/// 4. Check that the user is active and has staff privileges
/// 5. Generate JWT token and set as HTTP-Only cookie
/// 6. Return user info (without the token) in the response body
///
/// # Security
///
/// - CSRF protection via the double-submit cookie pattern
/// - Password verification uses constant-time Argon2id comparison
/// - Generic error messages prevent username enumeration
/// - Only active staff users can obtain tokens
/// - JWT stored in HTTP-Only cookie (not accessible via JavaScript)
/// - `SameSite=Strict` prevents cross-origin cookie sending
///
/// # Example
///
/// ```ignore
/// use reinhardt_admin::server::login::admin_login;
///
/// // Client-side usage (automatically generates HTTP request)
/// let response = admin_login(
///     "admin".to_string(),
///     "password123".to_string(),
///     csrf_token.to_string(),
/// ).await?;
/// // No need to store token — browser handles it via cookie
/// ```
#[server_fn]
pub async fn admin_login(
	username: String,
	password: String,
	csrf_token: String,
	#[inject] http_request: ServerFnRequest,
	#[inject] db: Depends<DatabaseConnection>,
	#[inject] site: Depends<AdminSite>,
	#[inject] authenticator: Depends<AdminLoginAuthenticator>,
) -> Result<LoginResponse, ServerFnError> {
	// Validate CSRF token
	require_csrf_token(&csrf_token, &http_request.inner().headers)?;

	// Verify JWT secret is configured
	let jwt_secret = site.jwt_secret().ok_or_else(|| {
		::tracing::error!("admin_login: JWT secret not configured on AdminSite");
		ServerFnError::server(500, "Admin login is not configured")
	})?;
	let jwt_auth = JwtAuth::new(jwt_secret);

	// Authenticate user (username lookup + password verification + staff check)
	let user_info = (authenticator.0)(username.clone(), password, db.as_arc().clone())
		.await
		.map_err(|e| {
			::tracing::warn!(error = ?e, "admin_login: Authentication failed");
			ServerFnError::server(500, "Internal authentication error")
		})?;

	let user_info = user_info.ok_or_else(|| {
		// Generic message to prevent username enumeration
		ServerFnError::server(401, "Invalid username or password")
	})?;

	// Generate JWT token
	let token = jwt_auth
		.generate_token(
			user_info.user_id.clone(),
			user_info.username.clone(),
			user_info.is_staff,
			user_info.is_superuser,
		)
		.map_err(|e| {
			::tracing::error!(error = ?e, "admin_login: JWT token generation failed");
			ServerFnError::server(500, "Token generation failed")
		})?;

	// Set JWT as HTTP-Only cookie via the shared cookie jar.
	// The server_fn router (router_ext.rs) reads SharedResponseCookies
	// and applies them as Set-Cookie response headers.
	let is_secure = http_request.inner().is_secure;
	let cookie = build_admin_auth_cookie(&token, is_secure);
	http_request.add_response_cookie(cookie);

	// Return user info without the token — the browser receives the token
	// via the Set-Cookie header, not the response body.
	Ok(LoginResponse {
		token: String::new(),
		username: user_info.username,
		user_id: user_info.user_id,
		is_staff: user_info.is_staff,
		is_superuser: user_info.is_superuser,
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_login_response_serialization() {
		// Arrange
		let response = LoginResponse {
			token: "eyJhbGciOiJIUzI1NiJ9.test".to_string(),
			username: "admin".to_string(),
			user_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
			is_staff: true,
			is_superuser: false,
		};

		// Act
		let json = serde_json::to_string(&response).expect("serialization should succeed");
		let deserialized: LoginResponse =
			serde_json::from_str(&json).expect("deserialization should succeed");

		// Assert
		assert_eq!(deserialized.token, response.token);
		assert_eq!(deserialized.username, response.username);
		assert_eq!(deserialized.user_id, response.user_id);
		assert!(deserialized.is_staff);
		assert!(!deserialized.is_superuser);
	}
}

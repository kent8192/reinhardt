//! Authentication shared types
//!
//! Types used by both client and server for authentication.
//! These types are serializable and can be sent between the WASM client
//! and the Rust server via server functions.

#[cfg(native)]
use reinhardt::Validate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// OpenAPI schema generation (server-side only)
#[cfg(native)]
use reinhardt::rest::openapi::{Schema, ToSchema};

/// User information (shared between client and server)
#[cfg_attr(native, derive(Schema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
	pub id: Uuid,
	pub username: String,
	pub email: String,
	pub is_active: bool,
}

/// Conversion from server-side User model to shared UserInfo
#[cfg(native)]
impl From<crate::apps::auth::models::User> for UserInfo {
	fn from(user: crate::apps::auth::models::User) -> Self {
		UserInfo {
			id: user.id(),
			username: user.username().to_string(),
			email: user.email().to_string(),
			is_active: user.is_active(),
		}
	}
}

/// Login request
#[cfg_attr(native, derive(Schema))]
#[cfg_attr(native, derive(Validate))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
	#[cfg_attr(native, validate(email(message = "Invalid email address")))]
	pub email: String,

	#[cfg_attr(native, validate(length(min = 1, message = "Password is required")))]
	pub password: String,
}

/// Register request
#[cfg_attr(native, derive(Schema))]
#[cfg_attr(native, derive(Validate))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
	#[cfg_attr(
		native,
		validate(length(
			min = 3,
			max = 150,
			message = "Username must be between 3 and 150 characters"
		))
	)]
	pub username: String,

	#[cfg_attr(native, validate(email(message = "Invalid email address")))]
	pub email: String,

	#[cfg_attr(
		native,
		validate(length(min = 8, message = "Password must be at least 8 characters"))
	)]
	pub password: String,

	#[cfg_attr(
		native,
		validate(length(
			min = 8,
			message = "Password confirmation must be at least 8 characters"
		))
	)]
	pub password_confirmation: String,
}

impl RegisterRequest {
	/// Validate that password and password_confirmation match
	pub fn validate_passwords_match(&self) -> Result<(), String> {
		if self.password != self.password_confirmation {
			return Err("Passwords do not match".to_string());
		}
		Ok(())
	}
}

/// Session data containing authenticated user information.
///
/// Used for both client-side authentication state and server-side
/// session validation in tests.
#[cfg_attr(native, derive(Schema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionData {
	/// The authenticated user's ID
	pub user_id: Uuid,
	/// The authenticated user's username
	pub username: String,
	/// The authenticated user's email
	pub email: String,
}

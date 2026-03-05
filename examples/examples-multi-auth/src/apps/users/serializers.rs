//! Request and response types for authentication endpoints

use serde::{Deserialize, Serialize};

/// Registration request payload
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
	pub username: String,
	pub email: String,
	pub password: String,
	pub first_name: Option<String>,
	pub last_name: Option<String>,
}

/// Login request payload
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
	pub username: String,
	pub password: String,
}

/// Authentication response with JWT token
#[derive(Debug, Serialize)]
pub struct AuthResponse {
	pub token: String,
	pub user: UserResponse,
}

/// API token generation response
#[derive(Debug, Serialize)]
pub struct TokenResponse {
	pub api_token: String,
}

/// User profile response
#[derive(Debug, Serialize)]
pub struct UserResponse {
	pub id: String,
	pub username: String,
	pub email: String,
	pub first_name: String,
	pub last_name: String,
	pub is_active: bool,
	pub is_staff: bool,
	pub is_superuser: bool,
}

impl From<&crate::apps::users::models::AppUser> for UserResponse {
	fn from(user: &crate::apps::users::models::AppUser) -> Self {
		Self {
			id: user.id.to_string(),
			username: user.username.clone(),
			email: user.email.clone(),
			first_name: user.first_name.clone(),
			last_name: user.last_name.clone(),
			is_active: user.is_active,
			is_staff: user.is_staff,
			is_superuser: user.is_superuser,
		}
	}
}

/// Logout response
#[derive(Debug, Serialize)]
pub struct LogoutResponse {
	pub message: String,
}

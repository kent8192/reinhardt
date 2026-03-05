//! Application settings

use std::env;

/// Returns the JWT secret key from environment or a default for development
pub fn jwt_secret() -> Vec<u8> {
	env::var("JWT_SECRET")
		.unwrap_or_else(|_| "dev-secret-change-in-production".to_string())
		.into_bytes()
}

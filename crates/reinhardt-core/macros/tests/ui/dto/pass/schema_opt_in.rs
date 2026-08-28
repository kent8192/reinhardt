//! Verifies that `#[dto(schema)]` is accepted and remains inert on non-native
//! builds, where the OpenAPI feature graph is intentionally unavailable.

// This standalone trybuild fixture intentionally exercises macro-generated
// `cfg(native)` without the facade build script's `check-cfg` declaration.
#![allow(unexpected_cfgs)]

use reinhardt_macros::dto;

#[dto(schema)]
#[schema(title = "Login request")]
pub struct LoginRequest {
	#[validate(length(min = 1))]
	#[schema(description = "Email address")]
	pub email: String,
	pub password: String,
}

fn main() {
	let _ = LoginRequest {
		email: String::from("user@example.com"),
		password: String::from("password"),
	};
}

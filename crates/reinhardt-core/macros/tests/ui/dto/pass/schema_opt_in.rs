//! Verifies that `#[dto(schema)]` is accepted and remains inert on non-native
//! builds, where the OpenAPI feature graph is intentionally unavailable.

#![allow(unexpected_cfgs)]

use reinhardt_macros::dto;

#[dto(schema)]
pub struct LoginRequest {
	#[validate(length(min = 1))]
	pub email: String,
	pub password: String,
}

fn main() {
	let _ = LoginRequest {
		email: String::from("user@example.com"),
		password: String::from("password"),
	};
}

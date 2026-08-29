//! Verifies that `#[dto(schema)]` keeps shared validation active while its
//! OpenAPI output remains inert on non-native builds.

// This standalone trybuild fixture intentionally exercises macro-generated
// `cfg(native)` without the facade build script's `check-cfg` declaration.
#![allow(unexpected_cfgs)]

extern crate self as reinhardt_core;

#[path = "../support.rs"]
mod support;

pub use reinhardt_macros::Validate;
pub use support::validators;

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
	let value = LoginRequest {
		email: String::from("user@example.com"),
		password: String::from("password"),
	};
	assert!(reinhardt_core::validators::Validate::validate(&value).is_ok());
}

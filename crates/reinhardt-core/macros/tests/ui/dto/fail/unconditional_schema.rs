//! Verifies that `#[dto(schema)]` rejects an unconditional `Schema` derive.

#![allow(unexpected_cfgs)]

use reinhardt_macros::dto;

// Synthetic name to test the attribute validation without requiring OpenAPI.
#[derive()]
pub struct Schema;

#[dto(schema)]
#[derive(Schema)]
pub struct LoginRequest {
	pub email: String,
}

fn main() {}

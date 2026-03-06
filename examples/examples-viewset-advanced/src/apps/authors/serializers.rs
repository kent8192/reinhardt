//! Author serializers for request/response handling.

use serde::{Deserialize, Serialize};
use validator::Validate;

/// Serializer for author responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorSerializer {
	pub id: i64,
	pub name: String,
	pub bio: String,
	pub is_active: bool,
}

/// Serializer for creating authors.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CreateAuthorSerializer {
	#[validate(length(min = 1, max = 255))]
	pub name: String,
	pub bio: String,
}

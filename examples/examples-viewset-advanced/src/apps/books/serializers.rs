//! Book serializers for response handling.

use serde::{Deserialize, Serialize};

/// Serializer for book responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookSerializer {
	pub id: i64,
	pub title: String,
	pub isbn: String,
	pub published_year: i32,
	pub author_id: i64,
}

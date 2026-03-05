//! Book model definition.

use serde::{Deserialize, Serialize};

/// Book model for a read-only catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Book {
	pub id: i64,
	pub title: String,
	pub isbn: String,
	pub published_year: i32,
	pub author_id: i64,
}

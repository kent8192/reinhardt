//! Article model definition.

use serde::{Deserialize, Serialize};

/// Article model with full CRUD support.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Article {
	pub id: i64,
	pub title: String,
	pub content: String,
	pub author_id: i64,
	pub status: String,
	pub published_at: Option<String>,
}

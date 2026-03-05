//! Article serializers for request/response handling.

use serde::{Deserialize, Serialize};
use validator::Validate;

/// Serializer for article responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArticleSerializer {
	pub id: i64,
	pub title: String,
	pub content: String,
	pub author_id: i64,
	pub status: String,
	pub published_at: Option<String>,
}

/// Serializer for creating articles.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CreateArticleSerializer {
	#[validate(length(min = 1, max = 500))]
	pub title: String,
	pub content: String,
	pub author_id: i64,
	#[validate(length(min = 1))]
	pub status: String,
}

/// Serializer for partial updates (PATCH).
///
/// All fields are optional - only provided fields are updated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchArticleSerializer {
	pub title: Option<String>,
	pub content: Option<String>,
	pub status: Option<String>,
	pub published_at: Option<String>,
}

/// Serializer for batch create requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchCreateArticleSerializer {
	pub articles: Vec<CreateArticleSerializer>,
}

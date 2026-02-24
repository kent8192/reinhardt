//! Serializers for api app
//!
//! Data serialization and validation

use serde::{Deserialize, Serialize};
use validator::Validate;

/// Article creation request serializer
#[derive(Serialize, Deserialize, Validate, Debug)]
pub struct CreateArticleRequest {
	/// Article title (3-255 characters)
	#[validate(length(min = 3, max = 255))]
	pub title: String,

	/// Article content (min 10 characters)
	#[validate(length(min = 10))]
	pub content: String,

	/// Author name (3-100 characters)
	#[validate(length(min = 3, max = 100))]
	pub author: String,

	/// Publication status
	#[serde(default)]
	pub published: bool,
}

/// Article response serializer
#[derive(Serialize, Deserialize, Debug)]
pub struct ArticleResponse {
	pub id: i64,
	pub title: String,
	pub content: String,
	pub author: String,
	pub published: bool,
	pub created_at: chrono::DateTime<chrono::Utc>,
	pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<super::models::Article> for ArticleResponse {
	fn from(article: super::models::Article) -> Self {
		Self {
			id: article.id,
			title: article.title,
			content: article.content,
			author: article.author,
			published: article.published,
			created_at: article.created_at,
			updated_at: article.updated_at,
		}
	}
}

/// Article list response
#[derive(Serialize, Deserialize, Debug)]
pub struct ArticleListResponse {
	pub count: usize,
	pub results: Vec<ArticleResponse>,
}

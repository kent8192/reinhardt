//! Serializers for api app
//!
//! Data serialization and validation

use reinhardt::Validate;
use reinhardt::core::serde::{Deserialize, Serialize};

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

/// Article update request serializer (partial update)
#[derive(Serialize, Deserialize, Validate, Debug, Default)]
pub struct UpdateArticleRequest {
	/// Article title (3-255 characters, optional)
	#[validate(length(min = 3, max = 255))]
	pub title: Option<String>,

	/// Article content (min 10 characters, optional)
	#[validate(length(min = 10))]
	pub content: Option<String>,

	/// Author name (3-100 characters, optional)
	#[validate(length(min = 3, max = 100))]
	pub author: Option<String>,

	/// Publication status (optional)
	pub published: Option<bool>,
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

#[cfg(test)]
mod tests {
	use super::*;
	use reinhardt::Validate;
	use rstest::rstest;

	#[rstest]
	fn test_create_article_request_valid() {
		// Arrange
		let req = CreateArticleRequest {
			title: "Valid Title".to_string(),
			content: "This is valid content with enough length".to_string(),
			author: "Author Name".to_string(),
			published: false,
		};

		// Act
		let result = req.validate();

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_create_article_request_title_too_short() {
		// Arrange
		let req = CreateArticleRequest {
			title: "AB".to_string(),
			content: "This is valid content with enough length".to_string(),
			author: "Author Name".to_string(),
			published: false,
		};

		// Act
		let result = req.validate();

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_create_article_request_content_too_short() {
		// Arrange
		let req = CreateArticleRequest {
			title: "Valid Title".to_string(),
			content: "Short".to_string(),
			author: "Author Name".to_string(),
			published: false,
		};

		// Act
		let result = req.validate();

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_update_article_request_all_none_valid() {
		// Arrange
		let req = UpdateArticleRequest::default();

		// Act
		let result = req.validate();

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_update_article_request_valid_partial() {
		// Arrange
		let req = UpdateArticleRequest {
			title: Some("Updated Title".to_string()),
			..Default::default()
		};

		// Act
		let result = req.validate();

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_update_article_request_invalid_title() {
		// Arrange
		let req = UpdateArticleRequest {
			title: Some("AB".to_string()),
			..Default::default()
		};

		// Act
		let result = req.validate();

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_update_article_request_invalid_content() {
		// Arrange
		let req = UpdateArticleRequest {
			content: Some("Short".to_string()),
			..Default::default()
		};

		// Act
		let result = req.validate();

		// Assert
		assert!(result.is_err());
	}
}

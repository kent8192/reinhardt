//! Views for api app
//!
//! RESTful API endpoints

use chrono::Utc;
use reinhardt::core::serde::json;
use reinhardt::http::ViewResult;
use reinhardt::{Json, Path, Response, StatusCode};
use reinhardt::{delete, get, post, put};
use validator::Validate;

use super::models::Article;
use super::serializers::{ArticleListResponse, ArticleResponse, CreateArticleRequest};
use super::storage;

/// List all articles
///
/// GET /articles/
/// # Example Response
/// ```json
/// {
///   "count": 2,
///   "results": [
///     {
///       "id": 1,
///       "title": "Introduction to Reinhardt",
///       "content": "Reinhardt is a batteries-included web framework...",
///       "author": "John Doe",
///       "published": true,
///       "created_at": "2025-01-01T00:00:00Z",
///       "updated_at": "2025-01-01T00:00:00Z"
///     }
///   ]
/// }
/// ```
#[get("/articles/", name = "articles_list")]
pub async fn list_articles() -> ViewResult<Response> {
	// Get all articles from in-memory storage
	let articles = storage::get_all_articles();

	let results: Vec<ArticleResponse> = articles.into_iter().map(Into::into).collect();

	let response = ArticleListResponse {
		count: results.len(),
		results,
	};

	Ok(Response::new(StatusCode::OK).with_body(json::to_vec(&response)?))
}

/// Create a new article
///
/// POST /articles/
/// # Request Body
/// ```json
/// {
///   "title": "Introduction to Reinhardt",
///   "content": "Reinhardt is a batteries-included web framework for Rust...",
///   "author": "John Doe",
///   "published": true
/// }
/// ```
#[post("/articles/", name = "articles_create")]
pub async fn create_article(Json(create_req): Json<CreateArticleRequest>) -> ViewResult<Response> {
	// Validate request
	create_req.validate()?;

	// Create new article using in-memory storage
	let now = Utc::now();
	let article = Article {
		id: 0, // Will be assigned by storage
		title: create_req.title,
		content: create_req.content,
		author: create_req.author,
		published: create_req.published,
		created_at: now,
		updated_at: now,
	};

	let created_article = storage::create_article(article);

	let response: ArticleResponse = created_article.into();

	Ok(Response::new(StatusCode::CREATED).with_body(json::to_vec(&response)?))
}

/// Get a specific article by ID
///
/// GET /articles/{id}/
/// # Path Parameters
/// - `id`: Article ID (e.g., `/articles/1`)
#[get("/articles/{id}/", name = "articles_get")]
pub async fn get_article(Path(id): Path<i64>) -> ViewResult<Response> {
	// Get article from in-memory storage
	let article = match storage::get_article(id) {
		Some(article) => article,
		None => {
			return Ok(Response::new(StatusCode::NOT_FOUND).with_body(
				format!(r#"{{"error": "Article with id {} not found"}}"#, id).into_bytes(),
			));
		}
	};

	let response: ArticleResponse = article.into();

	Ok(Response::new(StatusCode::OK).with_body(json::to_vec(&response)?))
}

/// Update an article
///
/// PUT /articles/{id}/
/// # Path Parameters
/// - `id`: Article ID (e.g., `/articles/1`)
///
/// # Request Body
/// Partial update supported - all fields are optional:
/// ```json
/// {
///   "title": "Updated Title",
///   "content": "Updated content...",
///   "published": false
/// }
/// ```
#[put("/articles/{id}/", name = "articles_update")]
pub async fn update_article(
	Path(id): Path<i64>,
	Json(update_data): Json<json::Value>,
) -> ViewResult<Response> {
	// Get existing article from storage
	let mut article = match storage::get_article(id) {
		Some(article) => article,
		None => {
			return Ok(Response::new(StatusCode::NOT_FOUND).with_body(
				format!(r#"{{"error": "Article with id {} not found"}}"#, id).into_bytes(),
			));
		}
	};

	// Apply partial updates
	if let Some(title) = update_data.get("title").and_then(|v| v.as_str()) {
		article.title = title.to_string();
	}
	if let Some(content) = update_data.get("content").and_then(|v| v.as_str()) {
		article.content = content.to_string();
	}
	if let Some(author) = update_data.get("author").and_then(|v| v.as_str()) {
		article.author = author.to_string();
	}
	if let Some(published) = update_data.get("published").and_then(|v| v.as_bool()) {
		article.published = published;
	}
	article.updated_at = Utc::now();

	// Save updated article
	let updated_article = match storage::update_article(article) {
		Some(article) => article,
		None => {
			return Ok(Response::new(StatusCode::INTERNAL_SERVER_ERROR).with_body(
				format!(r#"{{"error": "Failed to update article with id {}"}}"#, id).into_bytes(),
			));
		}
	};

	let response: ArticleResponse = updated_article.into();

	Ok(Response::new(StatusCode::OK).with_body(json::to_vec(&response)?))
}

/// Delete an article
///
/// DELETE /articles/{id}/
/// # Path Parameters
/// - `id`: Article ID (e.g., `/articles/1`)
///
/// # Response
/// Returns 204 No Content on success
#[delete("/articles/{id}/", name = "articles_delete")]
pub async fn delete_article(Path(id): Path<i64>) -> ViewResult<Response> {
	// Delete article from in-memory storage
	if !storage::delete_article(id) {
		return Ok(Response::new(StatusCode::NOT_FOUND).with_body(
			format!(r#"{{"error": "Article with id {} not found"}}"#, id).into_bytes(),
		));
	}

	Ok(Response::new(StatusCode::NO_CONTENT).with_body(Vec::new()))
}

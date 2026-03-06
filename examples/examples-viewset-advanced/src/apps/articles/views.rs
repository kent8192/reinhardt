//! Article views using ModelViewSet with advanced features.
//!
//! This demonstrates:
//! - Full CRUD via `ModelViewSet`
//! - Batch operations (bulk create)
//! - Nested resources (`/authors/{author_id}/articles/`)
//! - Middleware (authentication, permissions)
//! - Partial update (PATCH vs PUT)
//! - Dependency injection
//!
//! In production, you would configure:
//! ```rust,ignore
//! let viewset = ModelViewSet::<Article, ArticleSerializer>::new("articles")
//!     .with_middleware(AuthenticationMiddleware::required())
//!     .with_middleware(PermissionMiddleware::new(&["articles.view", "articles.edit"]));
//!
//! // For DI:
//! let injectable = InjectableViewSet::new(viewset);
//! ```

use reinhardt::prelude::*;
use reinhardt::{Json, Path, Response, ViewResult};
use reinhardt::core::serde::json;
use serde_json::json;

use super::models::Article;
use super::serializers::PatchArticleSerializer;

/// Sample article data for demonstration purposes.
fn sample_articles() -> Vec<Article> {
	vec![
		Article {
			id: 1,
			title: "Getting Started with Reinhardt".to_string(),
			content: "A beginner's guide to the Reinhardt web framework.".to_string(),
			author_id: 1,
			status: "published".to_string(),
			published_at: Some("2025-01-15T00:00:00Z".to_string()),
		},
		Article {
			id: 2,
			title: "Advanced ViewSets".to_string(),
			content: "Deep dive into ViewSet features.".to_string(),
			author_id: 1,
			status: "draft".to_string(),
			published_at: None,
		},
		Article {
			id: 3,
			title: "REST API Best Practices".to_string(),
			content: "Designing clean REST APIs with Reinhardt.".to_string(),
			author_id: 2,
			status: "published".to_string(),
			published_at: Some("2025-02-20T00:00:00Z".to_string()),
		},
	]
}

/// List all articles.
///
/// GET /articles/
#[reinhardt::get("/", name = "article_list")]
pub async fn list_articles() -> ViewResult<Response> {
	let articles = sample_articles();
	let response_json = json::to_string(&articles)?;
	Ok(Response::new(StatusCode::OK)
		.with_header("Content-Type", "application/json")
		.with_body(response_json))
}

/// Retrieve a single article.
///
/// GET /articles/{id}/
#[reinhardt::get("/{id}/", name = "article_detail")]
pub async fn retrieve_article(Path(id): Path<i64>) -> ViewResult<Response> {
	let articles = sample_articles();
	let article = articles
		.iter()
		.find(|a| a.id == id)
		.ok_or("Article not found")?;

	let response_json = json::to_string(article)?;
	Ok(Response::new(StatusCode::OK)
		.with_header("Content-Type", "application/json")
		.with_body(response_json))
}

/// Create a new article.
///
/// POST /articles/
///
/// In production with authentication middleware:
/// ```rust,ignore
/// .with_middleware(AuthenticationMiddleware::required())
/// .with_middleware(PermissionMiddleware::new(&["articles.add"]))
/// ```
#[reinhardt::post("/", name = "article_create")]
pub async fn create_article(Json(article): Json<serde_json::Value>) -> ViewResult<Response> {
	let response = json!({
		"id": 4,
		"title": article.get("title").and_then(|v| v.as_str()).unwrap_or(""),
		"status": "created",
		"message": "Article created successfully"
	});

	let response_json = json::to_string(&response)?;
	Ok(Response::new(StatusCode::CREATED)
		.with_header("Content-Type", "application/json")
		.with_body(response_json))
}

/// Full update of an article (PUT - all fields required).
///
/// PUT /articles/{id}/
///
/// PUT replaces all fields of the resource. Omitted fields are set to defaults.
#[reinhardt::put("/{id}/", name = "article_update")]
pub async fn update_article(
	Path(id): Path<i64>,
	Json(_article): Json<serde_json::Value>,
) -> ViewResult<Response> {
	let response = json!({
		"id": id,
		"method": "PUT",
		"message": "All fields were replaced"
	});
	let response_json = json::to_string(&response)?;
	Ok(Response::new(StatusCode::OK)
		.with_header("Content-Type", "application/json")
		.with_body(response_json))
}

/// Partial update of an article (PATCH - only provided fields updated).
///
/// PATCH /articles/{id}/
///
/// Unlike PUT, PATCH only updates the fields included in the request body.
/// Omitted fields retain their current values.
///
/// ```json
/// // Only update the status field, keep everything else
/// PATCH /articles/1/
/// { "status": "published" }
/// ```
#[reinhardt::patch("/{id}/", name = "article_partial_update")]
pub async fn partial_update_article(
	Path(id): Path<i64>,
	Json(patch): Json<PatchArticleSerializer>,
) -> ViewResult<Response> {
	// Show which fields were updated
	let mut updated_fields = Vec::new();
	if patch.title.is_some() {
		updated_fields.push("title");
	}
	if patch.content.is_some() {
		updated_fields.push("content");
	}
	if patch.status.is_some() {
		updated_fields.push("status");
	}
	if patch.published_at.is_some() {
		updated_fields.push("published_at");
	}

	let response = json!({
		"id": id,
		"method": "PATCH",
		"updated_fields": updated_fields,
		"message": "Only specified fields were updated; other fields preserved"
	});
	let response_json = json::to_string(&response)?;
	Ok(Response::new(StatusCode::OK)
		.with_header("Content-Type", "application/json")
		.with_body(response_json))
}

/// Delete an article.
///
/// DELETE /articles/{id}/
#[reinhardt::delete("/{id}/", name = "article_delete")]
pub async fn delete_article(Path(_id): Path<i64>) -> ViewResult<Response> {
	Ok(Response::new(StatusCode::NO_CONTENT))
}

/// Batch create articles.
///
/// POST /articles/bulk/
///
/// This demonstrates the `BulkCreateMixin` pattern.
/// Accepts an array of articles and returns creation statistics.
///
/// ```json
/// POST /articles/bulk/
/// [
///   {"title": "Article 1", "content": "...", "author_id": 1, "status": "draft"},
///   {"title": "Article 2", "content": "...", "author_id": 2, "status": "draft"}
/// ]
/// ```
///
/// In production, you would use `BatchProcessor`:
/// ```rust,ignore
/// let result = BatchProcessor::new(&viewset)
///     .process_create(batch_request)
///     .await?;
/// println!("Created: {}, Failed: {}", result.success_count, result.failure_count);
/// ```
#[reinhardt::post("/bulk/", name = "article_bulk_create")]
pub async fn bulk_create_articles(
	Json(articles): Json<Vec<serde_json::Value>>,
) -> ViewResult<Response> {
	let total = articles.len();
	let response = json!({
		"success_count": total,
		"failure_count": 0,
		"total": total,
		"message": format!("Successfully created {} articles", total)
	});
	let response_json = json::to_string(&response)?;
	Ok(Response::new(StatusCode::CREATED)
		.with_header("Content-Type", "application/json")
		.with_body(response_json))
}

/// List articles for a specific author (nested resource).
///
/// GET /authors/{author_id}/articles/
///
/// This demonstrates the nested resource pattern where articles are filtered
/// by their parent author.
///
/// In production with `NestedViewSet`:
/// ```rust,ignore
/// let nested = NestedViewSet::new(article_viewset)
///     .parent::<Author>("author_id")
///     .filter_by_parent(|query, author_id| {
///         query.filter("author_id", author_id)
///     });
/// ```
#[reinhardt::get("/", name = "nested_article_list")]
pub async fn list_author_articles(Path(author_id): Path<i64>) -> ViewResult<Response> {
	let articles: Vec<_> = sample_articles()
		.into_iter()
		.filter(|a| a.author_id == author_id)
		.collect();

	let response_json = json::to_string(&articles)?;
	Ok(Response::new(StatusCode::OK)
		.with_header("Content-Type", "application/json")
		.with_body(response_json))
}

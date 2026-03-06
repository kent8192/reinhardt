//! Author views using GenericViewSet with composable Mixins and custom actions.
//!
//! This demonstrates:
//! - Selective mixin composition (List + Retrieve only, no create/update/delete)
//! - Custom detail action: `/authors/{id}/activate/`
//! - Custom list action: `/authors/recent/`

use reinhardt::prelude::*;
use reinhardt::{Path, Response, ViewResult};
use reinhardt::core::serde::json;
use serde_json::json;

use super::models::Author;

/// Sample author data for demonstration purposes.
fn sample_authors() -> Vec<Author> {
	vec![
		Author {
			id: 1,
			name: "Alice Johnson".to_string(),
			bio: "Rust enthusiast and systems programmer".to_string(),
			is_active: true,
		},
		Author {
			id: 2,
			name: "Bob Smith".to_string(),
			bio: "Web developer and open source contributor".to_string(),
			is_active: true,
		},
		Author {
			id: 3,
			name: "Charlie Brown".to_string(),
			bio: "Database architect".to_string(),
			is_active: false,
		},
	]
}

/// List all authors (ListMixin behavior).
///
/// In a real app, you would use:
/// ```rust,ignore
/// let viewset = GenericViewSet::new("authors", AuthorSerializer::default())
///     // Only add List + Retrieve mixins (no Create, Update, Destroy)
/// ```
#[reinhardt::get("/", name = "author_list")]
pub async fn list_authors() -> ViewResult<Response> {
	let authors = sample_authors();
	let response_json = json::to_string(&authors)?;
	Ok(Response::new(StatusCode::OK)
		.with_header("Content-Type", "application/json")
		.with_body(response_json))
}

/// Retrieve a single author by ID (RetrieveMixin behavior).
#[reinhardt::get("/{id}/", name = "author_detail")]
pub async fn retrieve_author(Path(id): Path<i64>) -> ViewResult<Response> {
	let authors = sample_authors();
	let author = authors
		.iter()
		.find(|a| a.id == id)
		.ok_or("Author not found")?;

	let response_json = json::to_string(author)?;
	Ok(Response::new(StatusCode::OK)
		.with_header("Content-Type", "application/json")
		.with_body(response_json))
}

/// Custom detail action: Activate an author.
///
/// POST /authors/{id}/activate/
///
/// This demonstrates a custom action that operates on a single resource.
/// In production, you would register this action:
/// ```rust,ignore
/// register_action("activate", ActionType::Detail, &["POST"]);
/// ```
#[reinhardt::post("/{id}/activate/", name = "author_activate")]
pub async fn activate_author(Path(id): Path<i64>) -> ViewResult<Response> {
	let response = json!({
		"id": id,
		"status": "activated",
		"message": "Author has been activated"
	});

	let response_json = json::to_string(&response)?;
	Ok(Response::new(StatusCode::OK)
		.with_header("Content-Type", "application/json")
		.with_body(response_json))
}

/// Custom list action: Get recently active authors.
///
/// GET /authors/recent/
///
/// This demonstrates a custom action that operates on the collection.
/// In production, you would register this action:
/// ```rust,ignore
/// register_action("recent", ActionType::List, &["GET"]);
/// ```
#[reinhardt::get("/recent/", name = "author_recent")]
pub async fn recent_authors() -> ViewResult<Response> {
	let authors: Vec<_> = sample_authors()
		.into_iter()
		.filter(|a| a.is_active)
		.collect();

	let response_json = json::to_string(&authors)?;
	Ok(Response::new(StatusCode::OK)
		.with_header("Content-Type", "application/json")
		.with_body(response_json))
}

//! Book views using ReadOnlyModelViewSet with caching.
//!
//! This demonstrates:
//! - `ReadOnlyModelViewSet` providing only list and retrieve actions
//! - `CacheConfig` with TTL-based caching (5 minutes)
//! - POST/PUT/DELETE returning 405 Method Not Allowed
//!
//! In production, you would configure the viewset:
//! ```rust,ignore
//! let viewset = ReadOnlyModelViewSet::<Book, BookSerializer>::new("books");
//! let cached = CachedViewSet::new(viewset, CacheConfig {
//!     ttl_seconds: 300,
//!     vary_headers: vec!["Authorization"],
//!     cache_methods: vec!["GET", "HEAD"],
//! });
//! ```

use reinhardt::prelude::*;
use reinhardt::{Path, Response, ViewResult};
use reinhardt::core::serde::json;

use super::models::Book;

/// Sample book catalog for demonstration purposes.
fn sample_books() -> Vec<Book> {
	vec![
		Book {
			id: 1,
			title: "The Rust Programming Language".to_string(),
			isbn: "978-1-7185-0044-0".to_string(),
			published_year: 2019,
			author_id: 1,
		},
		Book {
			id: 2,
			title: "Programming Rust".to_string(),
			isbn: "978-1-4920-5259-8".to_string(),
			published_year: 2021,
			author_id: 2,
		},
		Book {
			id: 3,
			title: "Rust in Action".to_string(),
			isbn: "978-1-6172-9413-9".to_string(),
			published_year: 2021,
			author_id: 1,
		},
	]
}

/// List all books (read-only, cached).
///
/// GET /books/
///
/// In a production application, this response would be cached for 5 minutes.
#[reinhardt::get("/", name = "book_list")]
pub async fn list_books() -> ViewResult<Response> {
	let books = sample_books();
	let response_json = json::to_string(&books)?;
	Ok(Response::new(StatusCode::OK)
		.with_header("Content-Type", "application/json")
		.with_body(response_json))
}

/// Retrieve a single book by ID (read-only, cached).
///
/// GET /books/{id}/
#[reinhardt::get("/{id}/", name = "book_detail")]
pub async fn retrieve_book(Path(id): Path<i64>) -> ViewResult<Response> {
	let books = sample_books();
	let book = books.iter().find(|b| b.id == id).ok_or("Book not found")?;

	let response_json = json::to_string(book)?;
	Ok(Response::new(StatusCode::OK)
		.with_header("Content-Type", "application/json")
		.with_body(response_json))
}

/// Reject write operations on the read-only book catalog.
///
/// POST /books/ -> 405 Method Not Allowed
///
/// This demonstrates that `ReadOnlyModelViewSet` does not support create operations.
#[reinhardt::post("/", name = "book_create_rejected")]
pub async fn reject_create() -> ViewResult<Response> {
	Ok(Response::new(StatusCode::METHOD_NOT_ALLOWED)
		.with_header("Content-Type", "application/json")
		.with_body(
			r#"{"detail": "Method not allowed. This is a read-only resource."}"#,
		))
}

//! In-memory storage for api app
//!
//! Provides thread-safe in-memory storage for the REST API example.
//! This allows the example to work without a real database connection.

use crate::apps::api::models::Article;
use std::collections::HashMap;
use std::sync::RwLock;
use std::sync::atomic::{AtomicI64, Ordering};

/// Global in-memory storage for articles
static ARTICLES: RwLock<Option<HashMap<i64, Article>>> = RwLock::new(None);

/// Auto-incrementing ID counter
static NEXT_ID: AtomicI64 = AtomicI64::new(1);

/// Initialize storage if not already initialized
fn ensure_initialized() {
	let mut storage = ARTICLES.write().unwrap();
	if storage.is_none() {
		*storage = Some(HashMap::new());
	}
}

/// Get all articles from storage
pub fn get_all_articles() -> Vec<Article> {
	ensure_initialized();
	let storage = ARTICLES.read().unwrap();
	storage
		.as_ref()
		.map(|map| map.values().cloned().collect())
		.unwrap_or_default()
}

/// Get a single article by ID
pub fn get_article(id: i64) -> Option<Article> {
	ensure_initialized();
	let storage = ARTICLES.read().unwrap();
	storage.as_ref().and_then(|map| map.get(&id).cloned())
}

/// Create a new article (assigns ID automatically)
pub fn create_article(mut article: Article) -> Article {
	ensure_initialized();
	let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
	article.id = id;

	let mut storage = ARTICLES.write().unwrap();
	if let Some(map) = storage.as_mut() {
		map.insert(id, article.clone());
	}

	article
}

/// Update an existing article
pub fn update_article(article: Article) -> Option<Article> {
	ensure_initialized();
	let mut storage = ARTICLES.write().unwrap();
	if let Some(map) = storage.as_mut()
		&& let std::collections::hash_map::Entry::Occupied(mut e) = map.entry(article.id)
	{
		e.insert(article.clone());
		return Some(article);
	}
	None
}

/// Delete an article by ID
pub fn delete_article(id: i64) -> bool {
	ensure_initialized();
	let mut storage = ARTICLES.write().unwrap();
	if let Some(map) = storage.as_mut() {
		return map.remove(&id).is_some();
	}
	false
}

/// Clear all articles (useful for tests)
pub fn clear_articles() {
	ensure_initialized();
	let mut storage = ARTICLES.write().unwrap();
	if let Some(map) = storage.as_mut() {
		map.clear();
	}
	NEXT_ID.store(1, Ordering::SeqCst);
}

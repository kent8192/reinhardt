//! Query filter tests
//!
//! Tests for `QueryFilter` from reinhardt-rest.

use reinhardt_db::orm::Field;
use reinhardt_rest::filters::field_extensions::FieldOrderingExt;
use reinhardt_rest::filters::{FilterBackend, QueryFilter};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestPost {
	id: Option<i64>,
	title: String,
	content: String,
	age: i32,
	created_at: String,
}

reinhardt_test::impl_test_model!(TestPost, i64, "test_posts");

#[tokio::test]
async fn test_single_lookup() {
	let lookup = Field::<TestPost, String>::new(vec!["title"]).eq("Test".to_string());
	let filter = QueryFilter::new().with_lookup(lookup);

	let sql = "SELECT * FROM test_posts".to_string();
	let result = filter.filter_queryset(&HashMap::new(), sql).await.unwrap();

	assert!(result.contains("WHERE"));
	assert!(result.contains("title = 'Test'"));
}

#[tokio::test]
async fn test_multiple_lookups() {
	let filter = QueryFilter::new()
		.with_lookup(Field::<TestPost, String>::new(vec!["title"]).icontains("rust"))
		.with_lookup(Field::<TestPost, i32>::new(vec!["age"]).gte(18));

	let sql = "SELECT * FROM test_posts".to_string();
	let result = filter.filter_queryset(&HashMap::new(), sql).await.unwrap();

	assert!(result.contains("WHERE"));
	assert!(result.contains("title"));
	assert!(result.contains("age >= 18"));
	assert!(result.contains(" AND "));
}

#[tokio::test]
async fn test_ordering() {
	let filter = QueryFilter::new().order_by(Field::<TestPost, String>::new(vec!["title"]).asc());

	let sql = "SELECT * FROM test_posts".to_string();
	let result = filter.filter_queryset(&HashMap::new(), sql).await.unwrap();

	assert!(result.contains("ORDER BY"));
	assert!(result.contains("title ASC"));
}

#[tokio::test]
async fn test_lookup_and_ordering() {
	let filter = QueryFilter::new()
		.with_lookup(Field::<TestPost, String>::new(vec!["title"]).icontains("rust"))
		.order_by(Field::<TestPost, String>::new(vec!["created_at"]).desc());

	let sql = "SELECT * FROM test_posts".to_string();
	let result = filter.filter_queryset(&HashMap::new(), sql).await.unwrap();

	assert!(result.contains("WHERE"));
	assert!(result.contains("ORDER BY"));
	assert!(result.contains("created_at DESC"));
}

#[tokio::test]
async fn test_append_to_existing_order_by() {
	let filter = QueryFilter::new().order_by(Field::<TestPost, String>::new(vec!["title"]).asc());

	let sql = "SELECT * FROM test_posts ORDER BY created_at DESC".to_string();
	let result = filter.filter_queryset(&HashMap::new(), sql).await.unwrap();

	assert!(result.contains("ORDER BY"));
	assert!(result.contains("created_at DESC"));
	assert!(result.contains("title ASC"));
	assert!(result.contains("created_at DESC, title ASC"));
}

#[tokio::test]
async fn test_append_order_with_limit() {
	let filter = QueryFilter::new().order_by(Field::<TestPost, String>::new(vec!["title"]).asc());

	let sql = "SELECT * FROM test_posts ORDER BY created_at DESC LIMIT 10".to_string();
	let result = filter.filter_queryset(&HashMap::new(), sql).await.unwrap();

	assert!(result.contains("ORDER BY"));
	assert!(result.contains("created_at DESC, title ASC"));
	assert!(result.contains("LIMIT 10"));
	// Ensure LIMIT comes after ORDER BY
	let order_pos = result.find("ORDER BY").unwrap();
	let limit_pos = result.find("LIMIT").unwrap();
	assert!(order_pos < limit_pos);
}

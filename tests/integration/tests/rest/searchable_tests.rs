//! Searchable model tests
//!
//! Tests for `SearchableModel` trait from reinhardt-rest.

use reinhardt_db::orm::Field;
use reinhardt_rest::filters::field_extensions::FieldOrderingExt;
use reinhardt_rest::filters::{OrderingField, SearchableModel};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestPost {
	id: Option<i64>,
	title: String,
	content: String,
	created_at: String,
}

reinhardt_test::impl_test_model!(TestPost, i64, "test_posts");

impl SearchableModel for TestPost {
	fn searchable_fields() -> Vec<Field<Self, String>> {
		vec![Field::new(vec!["title"]), Field::new(vec!["content"])]
	}

	fn default_ordering() -> Vec<OrderingField<Self>> {
		vec![Field::<Self, String>::new(vec!["created_at"]).desc()]
	}
}

#[test]
fn test_searchable_fields() {
	let fields = TestPost::searchable_fields();
	assert_eq!(fields.len(), 2);
	assert_eq!(fields[0].path(), &["title"]);
	assert_eq!(fields[1].path(), &["content"]);
}

#[test]
fn test_searchable_field_names() {
	let names = TestPost::searchable_field_names();
	assert_eq!(names, vec!["title", "content"]);
}

#[test]
fn test_default_ordering() {
	let ordering = TestPost::default_ordering();
	assert_eq!(ordering.len(), 1);
	assert_eq!(ordering[0].field_path(), &["created_at"]);
}

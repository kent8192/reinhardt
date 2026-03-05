//! Ordering field tests
//!
//! Tests for `OrderingField` and `OrderDirection` from reinhardt-rest.

use reinhardt_db::orm::Field;
use reinhardt_rest::filters::OrderDirection;
use reinhardt_rest::filters::field_extensions::FieldOrderingExt;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestPost {
	id: Option<i64>,
	title: String,
	created_at: String,
}

reinhardt_test::impl_test_model!(TestPost, i64, "test_posts");

#[test]
fn test_asc_ordering() {
	let field = Field::<TestPost, String>::new(vec!["title"]);
	let order = field.asc();

	assert_eq!(order.field_path(), &["title"]);
	assert_eq!(order.direction(), OrderDirection::Asc);
	assert_eq!(order.to_sql(), "title ASC");
}

#[test]
fn test_desc_ordering() {
	let field = Field::<TestPost, String>::new(vec!["created_at"]);
	let order = field.desc();

	assert_eq!(order.field_path(), &["created_at"]);
	assert_eq!(order.direction(), OrderDirection::Desc);
	assert_eq!(order.to_sql(), "created_at DESC");
}

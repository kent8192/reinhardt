//! Nested serializer ORM integration tests
//!
//! Tests for `NestedSaveContext` and `ManyToManyManager` from reinhardt-rest.

use reinhardt_rest::serializers::nested_orm::{ManyToManyManager, NestedSaveContext};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestUser {
	id: Option<i64>,
	name: String,
}

reinhardt_test::impl_test_model!(TestUser, i64, "users");

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestGroup {
	id: Option<i64>,
	name: String,
}

reinhardt_test::impl_test_model!(TestGroup, i64, "groups");

#[test]
fn test_nested_save_context_creation() {
	let context = NestedSaveContext::new();
	assert_eq!(context.depth, 0);
	assert_eq!(context.max_depth, 10);
	assert!(context.transaction.is_none());
	assert!(context.parent_data.is_empty());
}

#[test]
fn test_nested_save_context_with_parent_data() {
	let context = NestedSaveContext::new()
		.with_parent_data("user_id".to_string(), serde_json::json!(123))
		.with_parent_data("tenant_id".to_string(), serde_json::json!(456));

	assert_eq!(context.parent_data.len(), 2);
	assert_eq!(
		context.get_parent_value("user_id").unwrap(),
		&serde_json::json!(123)
	);
	assert_eq!(
		context.get_parent_value("tenant_id").unwrap(),
		&serde_json::json!(456)
	);
}

#[test]
fn test_nested_save_context_depth_limit() {
	let mut context = NestedSaveContext::new().with_max_depth(2);

	assert!(context.increment_depth().is_ok());
	assert_eq!(context.depth, 1);

	assert!(context.increment_depth().is_ok());
	assert_eq!(context.depth, 2);

	// Exceeds limit
	let result = context.increment_depth();
	assert!(result.is_err());
}

#[test]
fn test_nested_save_context_child_context() {
	let parent = NestedSaveContext::new()
		.with_parent_data("key".to_string(), serde_json::json!("value"))
		.with_max_depth(5);

	let child = parent.child_context().unwrap();
	assert_eq!(child.depth, 1);
	assert_eq!(child.max_depth, 5);
	assert_eq!(child.parent_data.len(), 1);
	assert_eq!(
		child.get_parent_value("key").unwrap(),
		&serde_json::json!("value")
	);
}

#[test]
fn test_nested_save_context_child_exceeds_depth() {
	let mut parent = NestedSaveContext::new().with_max_depth(1);
	parent.depth = 1;

	let result = parent.child_context();
	assert!(result.is_err());
}

#[test]
fn test_many_to_many_manager_creation() {
	let manager =
		ManyToManyManager::<TestUser, TestGroup>::new("user_groups", "user_id", "group_id");

	assert_eq!(manager.junction_table, "user_groups");
	assert_eq!(manager.source_fk, "user_id");
	assert_eq!(manager.target_fk, "group_id");
}

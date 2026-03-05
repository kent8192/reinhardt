//! Nested serializer tests
//!
//! Tests for `NestedSerializer`, `ListSerializer`, and `WritableNestedSerializer`
//! from reinhardt-rest.

use reinhardt_rest::serializers::{
	ListSerializer, NestedSerializer, Serializer, WritableNestedSerializer,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Post {
	id: Option<i64>,
	title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Author {
	id: Option<i64>,
	name: String,
}

reinhardt_test::impl_test_model!(Post, i64, "posts");
reinhardt_test::impl_test_model!(Author, i64, "authors");

#[test]
fn test_nested_serializer_creation() {
	// Verify the serializer works by serializing a post
	let serializer = NestedSerializer::<Post, Author>::new("author");
	let post = Post {
		id: Some(1),
		title: "Test".to_string(),
	};
	let result = serializer.serialize(&post);
	assert!(result.is_ok());
}

#[test]
fn test_nested_serializer_custom_depth() {
	// Verify depth configuration works by serializing successfully
	let serializer = NestedSerializer::<Post, Author>::new("author").depth(3);
	let post = Post {
		id: Some(1),
		title: "Test".to_string(),
	};
	let result = serializer.serialize(&post);
	assert!(result.is_ok());
}

#[test]
fn test_list_serializer_creation() {
	let serializer = ListSerializer::<Post>::new();
	let posts = vec![
		Post {
			id: Some(1),
			title: String::from("First"),
		},
		Post {
			id: Some(2),
			title: String::from("Second"),
		},
	];

	let result = serializer.serialize(&posts).unwrap();
	let value: Value = serde_json::from_str(&result).unwrap();
	assert!(value.is_array());
	assert_eq!(value.as_array().unwrap().len(), 2);
}

#[test]
fn test_writable_nested_serializer_creation() {
	// Verify create and update permissions work via behavior
	let serializer = WritableNestedSerializer::<Post, Author>::new("author")
		.allow_create(true)
		.allow_update(true);

	// Create operation should be allowed
	let json = r#"{
		"id": 1,
		"title": "Test Post",
		"author": {
			"id": null,
			"name": "New Author"
		}
	}"#;
	assert!(serializer.deserialize(&json.to_string()).is_ok());

	// Update operation should be allowed
	let json = r#"{
		"id": 1,
		"title": "Test Post",
		"author": {
			"id": 42,
			"name": "Existing Author"
		}
	}"#;
	assert!(serializer.deserialize(&json.to_string()).is_ok());
}

#[test]
fn test_writable_nested_default_permissions() {
	// Verify default permissions reject both create and update
	let serializer = WritableNestedSerializer::<Post, Author>::new("author");

	// Create should be rejected by default
	let json = r#"{
		"id": 1,
		"title": "Test Post",
		"author": {
			"id": null,
			"name": "New Author"
		}
	}"#;
	assert!(serializer.deserialize(&json.to_string()).is_err());

	// Update should be rejected by default
	let json = r#"{
		"id": 1,
		"title": "Test Post",
		"author": {
			"id": 42,
			"name": "Existing Author"
		}
	}"#;
	assert!(serializer.deserialize(&json.to_string()).is_err());
}

#[test]
fn test_writable_nested_deserialize_rejects_create_when_not_allowed() {
	let serializer = WritableNestedSerializer::<Post, Author>::new("author");

	// JSON with nested author without id (create operation)
	let json = r#"{
		"id": 1,
		"title": "Test Post",
		"author": {
			"id": null,
			"name": "New Author"
		}
	}"#;

	let result = serializer.deserialize(&json.to_string());
	assert!(result.is_err());
	assert!(
		result
			.unwrap_err()
			.message()
			.contains("Creating nested instances is not allowed")
	);
}

#[test]
fn test_writable_nested_deserialize_rejects_update_when_not_allowed() {
	let serializer = WritableNestedSerializer::<Post, Author>::new("author");

	// JSON with nested author with id (update operation)
	let json = r#"{
		"id": 1,
		"title": "Test Post",
		"author": {
			"id": 42,
			"name": "Existing Author"
		}
	}"#;

	let result = serializer.deserialize(&json.to_string());
	assert!(result.is_err());
	assert!(
		result
			.unwrap_err()
			.message()
			.contains("Updating nested instances is not allowed")
	);
}

#[test]
fn test_writable_nested_deserialize_allows_create_when_enabled() {
	let serializer = WritableNestedSerializer::<Post, Author>::new("author").allow_create(true);

	let json = r#"{
		"id": 1,
		"title": "Test Post",
		"author": {
			"id": null,
			"name": "New Author"
		}
	}"#;

	// Should not error - actual creation requires ORM integration
	let result = serializer.deserialize(&json.to_string());
	assert!(result.is_ok());
}

#[test]
fn test_writable_nested_deserialize_allows_update_when_enabled() {
	let serializer = WritableNestedSerializer::<Post, Author>::new("author").allow_update(true);

	let json = r#"{
		"id": 1,
		"title": "Test Post",
		"author": {
			"id": 42,
			"name": "Updated Author"
		}
	}"#;

	// Should not error - actual update requires ORM integration
	let result = serializer.deserialize(&json.to_string());
	assert!(result.is_ok());
}

#[test]
fn test_writable_nested_deserialize_array_rejects_create() {
	let serializer = WritableNestedSerializer::<Author, Post>::new("posts");

	let json = r#"{
		"id": 1,
		"name": "Author",
		"posts": [
			{"id": null, "title": "New Post"}
		]
	}"#;

	let result = serializer.deserialize(&json.to_string());
	assert!(result.is_err());
	assert!(
		result
			.unwrap_err()
			.message()
			.contains("Creating nested instances is not allowed")
	);
}

#[test]
fn test_writable_nested_deserialize_without_nested_data() {
	let serializer = WritableNestedSerializer::<Post, Author>::new("author");

	// JSON without nested data - should work fine
	let json = r#"{
		"id": 1,
		"title": "Test Post"
	}"#;

	let result = serializer.deserialize(&json.to_string());
	assert!(result.is_ok());
}

#[test]
fn test_extract_nested_data_with_nested_object() {
	let serializer = WritableNestedSerializer::<Post, Author>::new("author");

	let json = r#"{
		"id": 1,
		"title": "Test Post",
		"author": {
			"id": 42,
			"name": "Alice"
		}
	}"#;

	let result = serializer.extract_nested_data(json).unwrap();
	let nested = result.unwrap();
	assert!(nested.is_object());
	assert_eq!(nested.get("id").unwrap().as_i64().unwrap(), 42);
	assert_eq!(nested.get("name").unwrap().as_str().unwrap(), "Alice");
}

#[test]
fn test_extract_nested_data_without_nested_field() {
	let serializer = WritableNestedSerializer::<Post, Author>::new("author");

	let json = r#"{
		"id": 1,
		"title": "Test Post"
	}"#;

	let result = serializer.extract_nested_data(json).unwrap();
	assert!(result.is_none());
}

#[test]
fn test_extract_nested_data_with_null_nested_field() {
	let serializer = WritableNestedSerializer::<Post, Author>::new("author");

	let json = r#"{
		"id": 1,
		"title": "Test Post",
		"author": null
	}"#;

	let result = serializer.extract_nested_data(json).unwrap();
	assert!(result.is_none());
}

#[test]
fn test_is_create_operation_with_null_pk() {
	let data = serde_json::json!({
		"id": null,
		"name": "New Author"
	});

	assert!(WritableNestedSerializer::<Post, Author>::is_create_operation(&data));
}

#[test]
fn test_is_create_operation_with_existing_pk() {
	let data = serde_json::json!({
		"id": 42,
		"name": "Existing Author"
	});

	assert!(!WritableNestedSerializer::<Post, Author>::is_create_operation(&data));
}

#[test]
fn test_is_create_operation_without_pk_field() {
	let data = serde_json::json!({
		"name": "Author Without ID"
	});

	assert!(WritableNestedSerializer::<Post, Author>::is_create_operation(&data));
}

#[test]
fn test_extract_nested_data_with_array() {
	let serializer = WritableNestedSerializer::<Author, Post>::new("posts");

	let json = r#"{
		"id": 1,
		"name": "Alice",
		"posts": [
			{"id": 1, "title": "First Post"},
			{"id": 2, "title": "Second Post"}
		]
	}"#;

	let result = serializer.extract_nested_data(json).unwrap();
	let nested = result.unwrap();
	assert!(nested.is_array());
	assert_eq!(nested.as_array().unwrap().len(), 2);
}

// Arena allocation tests
#[test]
fn test_nested_serializer_with_arena() {
	let serializer = NestedSerializer::<Post, Author>::new("author");
	let post = Post {
		id: Some(1),
		title: "Test Post".to_string(),
	};

	let result = serializer.serialize(&post);
	let json_str = result.unwrap();
	let value: Value = serde_json::from_str(&json_str).unwrap();
	assert_eq!(value["id"], 1);
	assert_eq!(value["title"], "Test Post");
}

#[test]
fn test_nested_serializer_without_arena() {
	let serializer = NestedSerializer::<Post, Author>::new("author").without_arena();
	let post = Post {
		id: Some(1),
		title: "Test Post".to_string(),
	};

	let result = serializer.serialize(&post);
	let json_str = result.unwrap();
	let value: Value = serde_json::from_str(&json_str).unwrap();
	assert_eq!(value["id"], 1);
	assert_eq!(value["title"], "Test Post");
}

#[test]
fn test_nested_serializer_arena_vs_non_arena() {
	let post = Post {
		id: Some(1),
		title: "Test Post".to_string(),
	};

	let arena_serializer = NestedSerializer::<Post, Author>::new("author");
	let non_arena_serializer = NestedSerializer::<Post, Author>::new("author").without_arena();

	let arena_result = arena_serializer.serialize(&post).unwrap();
	let non_arena_result = non_arena_serializer.serialize(&post).unwrap();

	// Both should produce the same JSON
	let arena_value: Value = serde_json::from_str(&arena_result).unwrap();
	let non_arena_value: Value = serde_json::from_str(&non_arena_result).unwrap();

	assert_eq!(arena_value, non_arena_value);
}

#[test]
fn test_nested_serializer_with_depth() {
	let serializer = NestedSerializer::<Post, Author>::new("author").depth(5);

	let post = Post {
		id: Some(1),
		title: "Test Post".to_string(),
	};

	let result = serializer.serialize(&post);
	assert!(result.is_ok());
}

//! Integration tests for reinhardt-apps registry functionality
//!
//! These tests use distributed_slice for model registration and must be run as integration tests
//! (separate binaries) to avoid linkme conflicts.
//!
//! Note: With OnceLock-based caching, caches are initialized once and cannot be cleared.
//! Tests verify read operations which remain valid.

use linkme::distributed_slice;
use reinhardt_apps::registry::{
	MODELS, ModelMetadata, RELATIONSHIPS, RelationshipMetadata, RelationshipType, find_model,
	get_models_for_app, get_registered_models, get_registered_relationships,
	get_relationships_for_model, get_relationships_to_model,
};
use rstest::rstest;
use serial_test::serial;
use std::collections::HashSet;

// Test model registrations for integration tests
#[distributed_slice(MODELS)]
static TEST_USER_MODEL: ModelMetadata = ModelMetadata {
	app_label: "auth",
	model_name: "User",
	table_name: "auth_users",
};

#[distributed_slice(MODELS)]
static TEST_POST_MODEL: ModelMetadata = ModelMetadata {
	app_label: "blog",
	model_name: "Post",
	table_name: "blog_posts",
};

#[distributed_slice(MODELS)]
static TEST_COMMENT_MODEL: ModelMetadata = ModelMetadata {
	app_label: "blog",
	model_name: "Comment",
	table_name: "blog_comments",
};

#[rstest]
#[serial(app_registry)]
fn test_get_registered_models() {
	let models = get_registered_models();
	// Should have at least our test models
	assert!(models.len() >= 3);

	// Check that our test models are present
	assert!(models.iter().any(|m| m.model_name == "User"));
	assert!(models.iter().any(|m| m.model_name == "Post"));
	assert!(models.iter().any(|m| m.model_name == "Comment"));
}

#[rstest]
#[serial(app_registry)]
fn test_get_models_for_app() {
	let blog_models = get_models_for_app("blog");
	assert_eq!(blog_models.len(), 2);

	let model_names: HashSet<&str> = blog_models.iter().map(|m| m.model_name).collect();
	assert_eq!(model_names, HashSet::from(["Post", "Comment"]));

	let auth_models = get_models_for_app("auth");
	assert_eq!(auth_models.len(), 1);
	assert_eq!(auth_models[0].model_name, "User");
}

#[rstest]
#[serial(app_registry)]
fn test_get_models_for_app_cached() {
	// First call - initializes cache (with OnceLock, this is lazy and permanent)
	let models1 = get_models_for_app("blog");
	assert_eq!(models1.len(), 2);

	// Second call - should use same cached data
	let models2 = get_models_for_app("blog");
	assert_eq!(models2.len(), 2);

	// Results should be the same
	assert_eq!(models1.len(), models2.len());
}

#[rstest]
#[serial(app_registry)]
fn test_get_models_for_nonexistent_app() {
	let models = get_models_for_app("nonexistent");
	assert_eq!(models.len(), 0);
}

#[rstest]
#[serial(app_registry)]
fn test_find_model() {
	let model = find_model("auth.User");
	assert!(model.is_some());
	assert_eq!(model.unwrap().model_name, "User");
	assert_eq!(model.unwrap().table_name, "auth_users");

	let model = find_model("blog.Post");
	assert!(model.is_some());
	assert_eq!(model.unwrap().model_name, "Post");
}

#[rstest]
#[serial(app_registry)]
fn test_find_nonexistent_model() {
	let model = find_model("nonexistent.Model");
	assert!(model.is_none());
}

// Test relationship registrations
#[distributed_slice(RELATIONSHIPS)]
static TEST_POST_AUTHOR: RelationshipMetadata = RelationshipMetadata {
	from_model: "blog.Post",
	to_model: "auth.User",
	relationship_type: RelationshipType::ForeignKey,
	field_name: "author",
	related_name: Some("posts"),
	db_column: Some("author_id"),
	through_table: None,
};

#[distributed_slice(RELATIONSHIPS)]
static TEST_POST_TAGS: RelationshipMetadata = RelationshipMetadata {
	from_model: "blog.Post",
	to_model: "blog.Tag",
	relationship_type: RelationshipType::ManyToMany,
	field_name: "tags",
	related_name: Some("posts"),
	db_column: None,
	through_table: Some("blog_post_tags"),
};

#[rstest]
#[serial(app_registry)]
fn test_get_registered_relationships() {
	let relationships = get_registered_relationships();
	// Should have at least our test relationships
	assert!(relationships.len() >= 2);

	// Check that our test relationships are present
	assert!(
		relationships
			.iter()
			.any(|r| r.field_name == "author" && r.from_model == "blog.Post")
	);
	assert!(
		relationships
			.iter()
			.any(|r| r.field_name == "tags" && r.from_model == "blog.Post")
	);
}

#[rstest]
#[serial(app_registry)]
fn test_get_relationships_for_model() {
	let post_rels = get_relationships_for_model("blog.Post");
	assert_eq!(post_rels.len(), 2);

	let field_names: HashSet<&str> = post_rels.iter().map(|r| r.field_name).collect();
	assert_eq!(field_names, HashSet::from(["author", "tags"]));
}

#[rstest]
#[serial(app_registry)]
fn test_get_relationships_for_nonexistent_model() {
	let rels = get_relationships_for_model("nonexistent.Model");
	assert_eq!(rels.len(), 0);
}

#[rstest]
#[serial(app_registry)]
fn test_get_relationships_to_model() {
	let user_rels = get_relationships_to_model("auth.User");
	assert!(!user_rels.is_empty());

	assert!(
		user_rels
			.iter()
			.any(|r| r.field_name == "author" && r.from_model == "blog.Post")
	);
}

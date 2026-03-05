//! Advanced integration tests for relationship registration
//!
//! Tests OneToOne relationships, related_name reverse lookups, and ManyToMany through table validation.
//!
//! Note: With OnceLock-based caching, caches are initialized once and cannot be cleared.
//! Tests verify read operations which remain valid.

use linkme::distributed_slice;
use reinhardt_apps::registry::{
	RELATIONSHIPS, RelationshipMetadata, RelationshipType, get_registered_relationships,
	get_relationships_to_model,
};
use rstest::rstest;
use serial_test::serial;

// Test relationship registrations for advanced scenarios

/// OneToOne relationship: Profile belongs to User (1:1)
#[distributed_slice(RELATIONSHIPS)]
static TEST_PROFILE_USER: RelationshipMetadata = RelationshipMetadata {
	from_model: "profiles.UserProfile",
	to_model: "auth.User",
	relationship_type: RelationshipType::OneToOne,
	field_name: "user",
	related_name: Some("profile"),
	db_column: Some("user_id"),
	through_table: None,
};

/// ForeignKey relationship with custom related_name
#[distributed_slice(RELATIONSHIPS)]
static TEST_COMMENT_AUTHOR: RelationshipMetadata = RelationshipMetadata {
	from_model: "blog.Comment",
	to_model: "auth.User",
	relationship_type: RelationshipType::ForeignKey,
	field_name: "author",
	related_name: Some("authored_comments"),
	db_column: Some("author_id"),
	through_table: None,
};

/// ManyToMany relationship with through table
#[distributed_slice(RELATIONSHIPS)]
static TEST_ARTICLE_CATEGORIES: RelationshipMetadata = RelationshipMetadata {
	from_model: "blog.Article",
	to_model: "blog.Category",
	relationship_type: RelationshipType::ManyToMany,
	field_name: "categories",
	related_name: Some("articles"),
	db_column: None,
	through_table: Some("blog_article_categories"),
};

/// Test OneToOne relationship registration, retrieval, and validation
#[rstest]
#[serial(app_registry)]
fn test_one_to_one_relationship_registration() {
	let all_relationships = get_registered_relationships();

	// Find the OneToOne relationship
	let profile_user = all_relationships
		.iter()
		.find(|r| r.from_model == "profiles.UserProfile" && r.field_name == "user");

	assert!(profile_user.is_some(), "OneToOne relationship not found");

	let profile_user = profile_user.unwrap();

	// Verify relationship type
	assert_eq!(
		profile_user.relationship_type,
		RelationshipType::OneToOne,
		"Relationship type should be OneToOne"
	);

	// Verify target model
	assert_eq!(
		profile_user.to_model, "auth.User",
		"Target model should be auth.User"
	);

	// Verify related_name
	assert_eq!(
		profile_user.related_name,
		Some("profile"),
		"Related name should be 'profile'"
	);

	// Verify db_column
	assert_eq!(
		profile_user.db_column,
		Some("user_id"),
		"Database column should be 'user_id'"
	);

	// Verify through_table is None (not applicable for OneToOne)
	assert!(
		profile_user.through_table.is_none(),
		"OneToOne relationship should not have through_table"
	);
}

/// Test related_name reverse lookup functionality
#[rstest]
#[serial(app_registry)]
fn test_related_name_reverse_lookup() {
	// Get all relationships pointing to auth.User
	let user_relationships = get_relationships_to_model("auth.User");

	// Verify that the OneToOne relationship is included
	let profile_rel = user_relationships
		.iter()
		.find(|r| r.from_model == "profiles.UserProfile" && r.field_name == "user");

	assert!(
		profile_rel.is_some(),
		"Reverse lookup should find UserProfile -> User relationship"
	);

	let profile_rel = profile_rel.unwrap();
	assert_eq!(
		profile_rel.related_name,
		Some("profile"),
		"Related name should be 'profile' for reverse access"
	);

	// Verify that the ForeignKey relationship with custom related_name is included
	let comment_rel = user_relationships
		.iter()
		.find(|r| r.from_model == "blog.Comment" && r.field_name == "author");

	assert!(
		comment_rel.is_some(),
		"Reverse lookup should find Comment -> User relationship"
	);

	let comment_rel = comment_rel.unwrap();
	assert_eq!(
		comment_rel.related_name,
		Some("authored_comments"),
		"Related name should be 'authored_comments' for custom reverse access"
	);
}

/// Test ManyToMany relationship with through table validation
#[rstest]
#[serial(app_registry)]
fn test_many_to_many_through_table_validation() {
	let all_relationships = get_registered_relationships();

	// Find the ManyToMany relationship
	let article_categories = all_relationships
		.iter()
		.find(|r| r.from_model == "blog.Article" && r.field_name == "categories");

	assert!(
		article_categories.is_some(),
		"ManyToMany relationship not found"
	);

	let article_categories = article_categories.unwrap();

	// Verify relationship type
	assert_eq!(
		article_categories.relationship_type,
		RelationshipType::ManyToMany,
		"Relationship type should be ManyToMany"
	);

	// Verify target model
	assert_eq!(
		article_categories.to_model, "blog.Category",
		"Target model should be blog.Category"
	);

	// Verify related_name
	assert_eq!(
		article_categories.related_name,
		Some("articles"),
		"Related name should be 'articles'"
	);

	// Verify through_table is set
	assert_eq!(
		article_categories.through_table,
		Some("blog_article_categories"),
		"Through table should be 'blog_article_categories'"
	);

	// Verify db_column is None (not applicable for ManyToMany)
	assert!(
		article_categories.db_column.is_none(),
		"ManyToMany relationship should not have db_column"
	);

	// Verify reverse lookup works
	let category_relationships = get_relationships_to_model("blog.Category");
	let reverse_rel = category_relationships
		.iter()
		.find(|r| r.from_model == "blog.Article" && r.field_name == "categories");

	assert!(
		reverse_rel.is_some(),
		"Reverse lookup should find Article -> Category relationship"
	);

	let reverse_rel = reverse_rel.unwrap();
	assert_eq!(
		reverse_rel.through_table,
		Some("blog_article_categories"),
		"Through table should be consistent in reverse lookup"
	);
}

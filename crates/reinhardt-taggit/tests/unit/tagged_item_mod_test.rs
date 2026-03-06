//! Unit tests for TaggedItem model
//!
//! Tests the TaggedItem model constructor and field access.

use reinhardt_taggit::TaggedItem;
use rstest::rstest;

/// Test TaggedItem::new() constructor with various inputs
#[rstest]
#[case(1, "Food", 42)]
#[case(2, "Recipe", 100)]
#[case(3, "Article", 200)]
fn test_tagged_item_new_constructor(
	#[case] tag_id: i64,
	#[case] content_type: &str,
	#[case] object_id: i64,
) {
	// Arrange & Act
	let item = TaggedItem::new(tag_id, content_type, object_id);

	// Assert
	assert_eq!(item.tag_id, tag_id);
	assert_eq!(item.content_type, content_type);
	assert_eq!(item.object_id, object_id);
	assert!(item.id.is_none());
}

/// Test TaggedItem created_at field is initialized
#[test]
fn test_tagged_item_created_at_initialized() {
	// Arrange & Act
	let item = TaggedItem::new(1, "Food", 42);

	// Assert
	// created_at should be set to a timestamp (not checked for exact value)
	// Just verify it's not the default Unix epoch
	assert!(item.created_at.timestamp() > 0);
}

/// Test TaggedItem with different content types
#[rstest]
#[case("Food")]
#[case("Recipe")]
#[case("Article")]
#[case("Product")]
#[case("User")]
#[case("Category")]
fn test_tagged_item_content_types(#[case] content_type: &str) {
	// Arrange & Act
	let item = TaggedItem::new(1, content_type, 42);

	// Assert
	assert_eq!(item.content_type, content_type);
}

/// Test TaggedItem with various object IDs
#[rstest]
#[case(1)]
#[case(42)]
#[case(100)]
#[case(999)]
#[case(1000)]
#[case(i64::MAX)]
fn test_tagged_item_object_ids(#[case] object_id: i64) {
	// Arrange & Act
	let item = TaggedItem::new(1, "Food", object_id);

	// Assert
	assert_eq!(item.object_id, object_id);
}

/// Test TaggedItem with various tag IDs
#[rstest]
#[case(1)]
#[case(10)]
#[case(100)]
#[case(i64::MAX)]
fn test_tagged_item_tag_ids(#[case] tag_id: i64) {
	// Arrange & Act
	let item = TaggedItem::new(tag_id, "Food", 42);

	// Assert
	assert_eq!(item.tag_id, tag_id);
}

/// Test TaggedItem equality
#[test]
fn test_tagged_item_equality() {
	// Arrange
	let item1 = TaggedItem::new(1, "Food", 42);
	let item2 = TaggedItem::new(1, "Food", 42);
	let item3 = TaggedItem::new(2, "Food", 42);

	// Assert - compare fields excluding created_at (which differs between instances)
	assert_eq!(item1.id, item2.id);
	assert_eq!(item1.tag_id, item2.tag_id);
	assert_eq!(item1.content_type, item2.content_type);
	assert_eq!(item1.object_id, item2.object_id);

	assert_ne!(item1.tag_id, item3.tag_id);
}

/// Test TaggedItem clone produces identical copy
#[test]
fn test_tagged_item_clone() {
	// Arrange
	let item = TaggedItem::new(1, "Food", 42);

	// Act
	let cloned = item.clone();

	// Assert - field-by-field comparison (clone is instant, so created_at matches)
	assert_eq!(item.id, cloned.id);
	assert_eq!(item.tag_id, cloned.tag_id);
	assert_eq!(item.content_type, cloned.content_type);
	assert_eq!(item.object_id, cloned.object_id);
	assert_eq!(item.created_at, cloned.created_at);
}

/// Test TaggedItem debug format
#[test]
fn test_tagged_item_debug_format() {
	// Arrange
	let item = TaggedItem::new(1, "Food", 42);

	// Act
	let debug_str = format!("{:?}", item);

	// Assert
	assert!(debug_str.contains("Food"));
	assert!(debug_str.contains("42"));
}

/// Test TaggedItem with same object different tags
#[test]
fn test_tagged_item_same_object_different_tags() {
	// Arrange & Act
	let item1 = TaggedItem::new(1, "Food", 42);
	let item2 = TaggedItem::new(2, "Food", 42);
	let item3 = TaggedItem::new(3, "Food", 42);

	// Assert
	assert_eq!(item1.object_id, 42);
	assert_eq!(item2.object_id, 42);
	assert_eq!(item3.object_id, 42);
	assert_eq!(item1.tag_id, 1);
	assert_eq!(item2.tag_id, 2);
	assert_eq!(item3.tag_id, 3);
}

/// Test TaggedItem with different content types same tag
#[test]
fn test_tagged_item_polymorphic_content_types() {
	// Arrange & Act
	let food_item = TaggedItem::new(1, "Food", 42);
	let recipe_item = TaggedItem::new(1, "Recipe", 100);

	// Assert
	assert_eq!(food_item.tag_id, 1);
	assert_eq!(recipe_item.tag_id, 1);
	assert_eq!(food_item.content_type, "Food");
	assert_eq!(recipe_item.content_type, "Recipe");
}

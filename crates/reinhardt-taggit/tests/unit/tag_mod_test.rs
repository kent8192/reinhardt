//! Unit tests for Tag model
//!
//! Tests the Tag model constructor and field access.

use reinhardt_taggit::Tag;
use rstest::rstest;

/// Test Tag::new() constructor with various inputs
#[rstest]
#[case("rust", "rust")]
#[case("python", "python")]
#[case("web-development", "web-development")]
#[case("javascript", "javascript")]
fn test_tag_new_constructor(#[case] name: &str, #[case] expected_slug: &str) {
	// Arrange & Act
	let tag = Tag::new(name, expected_slug);

	// Assert
	assert_eq!(tag.name, name);
	assert_eq!(tag.slug, expected_slug);
	assert!(tag.id.is_none());
}

/// Test Tag created_at field is initialized
#[test]
fn test_tag_created_at_initialized() {
	// Arrange & Act
	let tag = Tag::new("test", "test");

	// Assert
	// created_at should be set to a timestamp (not checked for exact value)
	// Just verify it's not the default Unix epoch
	assert!(tag.created_at.timestamp() > 0);
}

/// Test Tag with hyphenated name
#[rstest]
#[case("web-development", "web-development")]
#[case("machine-learning", "machine-learning")]
#[case("full-stack", "full-stack")]
fn test_tag_hyphenated_name(#[case] name: &str, #[case] slug: &str) {
	// Arrange & Act
	let tag = Tag::new(name, slug);

	// Assert
	assert_eq!(tag.name, name);
	assert_eq!(tag.slug, slug);
}

/// Test Tag with numbers
#[rstest]
#[case("html5", "html5")]
#[case("css3", "css3")]
#[case("web2.0", "web2.0")]
fn test_tag_with_numbers(#[case] name: &str, #[case] slug: &str) {
	// Arrange & Act
	let tag = Tag::new(name, slug);

	// Assert
	assert_eq!(tag.name, name);
	assert_eq!(tag.slug, slug);
}

/// Test Tag with mixed case
#[rstest]
#[case("Rust", "Rust")]
#[case("Python", "Python")]
#[case("JavaScript", "JavaScript")]
fn test_tag_mixed_case(#[case] name: &str, #[case] slug: &str) {
	// Arrange & Act
	let tag = Tag::new(name, slug);

	// Assert
	assert_eq!(tag.name, name);
	assert_eq!(tag.slug, slug);
}

/// Test Tag with underscores
#[rstest]
#[case("web_development", "web_development")]
#[case("data_science", "data_science")]
fn test_tag_with_underscores(#[case] name: &str, #[case] slug: &str) {
	// Arrange & Act
	let tag = Tag::new(name, slug);

	// Assert
	assert_eq!(tag.name, name);
	assert_eq!(tag.slug, slug);
}

/// Test Tag equality
#[test]
fn test_tag_equality() {
	// Arrange
	let tag1 = Tag::new("rust", "rust");
	let tag2 = Tag::new("rust", "rust");
	let tag3 = Tag::new("python", "python");

	// Assert - compare fields excluding created_at (which differs between instances)
	assert_eq!(tag1.id, tag2.id);
	assert_eq!(tag1.name, tag2.name);
	assert_eq!(tag1.slug, tag2.slug);

	assert_ne!(tag1.name, tag3.name);
	assert_ne!(tag1.slug, tag3.slug);
}

/// Test Tag clone produces identical copy
#[test]
fn test_tag_clone() {
	// Arrange
	let tag = Tag::new("rust", "rust");

	// Act
	let cloned = tag.clone();

	// Assert - field-by-field comparison (clone is instant, so created_at matches)
	assert_eq!(tag.id, cloned.id);
	assert_eq!(tag.name, cloned.name);
	assert_eq!(tag.slug, cloned.slug);
	assert_eq!(tag.created_at, cloned.created_at);
}

/// Test Tag debug format
#[test]
fn test_tag_debug_format() {
	// Arrange
	let tag = Tag::new("rust", "rust");

	// Act
	let debug_str = format!("{:?}", tag);

	// Assert
	assert!(debug_str.contains("rust"));
}

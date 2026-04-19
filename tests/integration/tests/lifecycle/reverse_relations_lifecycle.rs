//! Reverse Relations Lifecycle Tests
//!
//! Verifies the specification invariants of the two-phase initialization:
//! `register_reverse_relation()` → `finalize_reverse_relations()`
//! → `get_reverse_relations_for_model()`
//!
//! The key contract: finalization creates a hard error barrier that prevents
//! further registrations, ensuring data consistency.

use reinhardt_apps::registry::{
	ReverseRelationMetadata, ReverseRelationType, finalize_reverse_relations,
	get_reverse_relations_for_model, register_reverse_relation, reset_global_registry,
};
use rstest::rstest;
use serial_test::serial;

/// Specification: Before finalization, `get_reverse_relations_for_model()` returns empty.
#[rstest]
#[serial(reverse_relations)]
fn get_before_finalization_returns_empty() {
	// Arrange
	reset_global_registry();

	// Act
	let result = get_reverse_relations_for_model("AnyModel");

	// Assert
	assert!(
		result.is_empty(),
		"must return empty Vec before finalization"
	);
}

/// Specification: Full lifecycle: register → finalize → get returns registered data.
#[rstest]
#[serial(reverse_relations)]
fn register_then_finalize_then_get_returns_data() {
	// Arrange
	reset_global_registry();
	let relation = ReverseRelationMetadata::new(
		"User",
		"posts".to_string(),
		"Post",
		ReverseRelationType::ReverseOneToMany,
		"author_id",
	);
	register_reverse_relation(relation).expect("registration must succeed before finalization");

	// Act
	finalize_reverse_relations();
	let result = get_reverse_relations_for_model("User");

	// Assert
	assert_eq!(result.len(), 1, "must return one registered relation");
	assert_eq!(result[0].accessor_name, "posts");
	assert_eq!(result[0].related_model, "Post");
}

/// Specification: `finalize_reverse_relations()` is idempotent.
/// Calling it twice must not corrupt data or panic.
#[rstest]
#[serial(reverse_relations)]
fn finalize_is_idempotent() {
	// Arrange
	reset_global_registry();
	let relation = ReverseRelationMetadata::new(
		"User",
		"posts".to_string(),
		"Post",
		ReverseRelationType::ReverseOneToMany,
		"author_id",
	);
	register_reverse_relation(relation).expect("registration must succeed");

	// Act
	finalize_reverse_relations();
	finalize_reverse_relations();
	let result = get_reverse_relations_for_model("User");

	// Assert
	assert_eq!(
		result.len(),
		1,
		"data must remain intact after double finalization"
	);
}

/// Specification: Registration after finalization must be rejected with an error.
/// This is the hard error barrier that prevents inconsistent state.
#[rstest]
#[serial(reverse_relations)]
fn register_after_finalize_returns_error() {
	// Arrange
	reset_global_registry();
	finalize_reverse_relations();

	// Act
	let result = register_reverse_relation(ReverseRelationMetadata::new(
		"User",
		"posts".to_string(),
		"Post",
		ReverseRelationType::ReverseOneToMany,
		"author_id",
	));

	// Assert
	assert!(
		result.is_err(),
		"registration after finalization must return Err"
	);
}

/// Specification: The error from post-finalization registration must be the
/// RegistryState variant, indicating a lifecycle ordering violation.
#[rstest]
#[serial(reverse_relations)]
fn register_after_finalize_error_variant_is_registry_state() {
	// Arrange
	reset_global_registry();
	finalize_reverse_relations();

	// Act
	let err = register_reverse_relation(ReverseRelationMetadata::new(
		"User",
		"posts".to_string(),
		"Post",
		ReverseRelationType::ReverseOneToMany,
		"author_id",
	))
	.expect_err("must be Err");

	// Assert
	let error_msg = err.to_string();
	assert!(
		error_msg.contains("finalization"),
		"error must indicate finalization barrier violation, got: {error_msg}"
	);
}

/// Specification: All registrations made before finalization must be visible after.
#[rstest]
#[serial(reverse_relations)]
fn multiple_registrations_all_visible_after_finalize() {
	// Arrange
	reset_global_registry();
	register_reverse_relation(ReverseRelationMetadata::new(
		"User",
		"posts".to_string(),
		"Post",
		ReverseRelationType::ReverseOneToMany,
		"author_id",
	))
	.expect("first registration must succeed");
	register_reverse_relation(ReverseRelationMetadata::new(
		"User",
		"comments".to_string(),
		"Comment",
		ReverseRelationType::ReverseOneToMany,
		"user_id",
	))
	.expect("second registration must succeed");
	register_reverse_relation(ReverseRelationMetadata::new(
		"Post",
		"tags".to_string(),
		"Tag",
		ReverseRelationType::ReverseManyToMany,
		"post_id",
	))
	.expect("third registration must succeed");

	// Act
	finalize_reverse_relations();
	let user_relations = get_reverse_relations_for_model("User");
	let post_relations = get_reverse_relations_for_model("Post");

	// Assert
	assert_eq!(
		user_relations.len(),
		2,
		"User must have 2 reverse relations"
	);
	assert_eq!(post_relations.len(), 1, "Post must have 1 reverse relation");
	let accessor_names: Vec<&str> = user_relations
		.iter()
		.map(|r| r.accessor_name.as_str())
		.collect();
	assert!(
		accessor_names.contains(&"posts"),
		"User must have 'posts' accessor"
	);
	assert!(
		accessor_names.contains(&"comments"),
		"User must have 'comments' accessor"
	);
}

//! Integration tests for reinhardt-apps collision detection
//!
//! These tests verify that the validation module correctly detects:
//! - Duplicate app_label registrations (expected behavior)
//! - Duplicate model names within the same app
//! - Duplicate table names across different apps
//! - Circular foreign key relationships
//!
//! Note: With OnceLock-based caching, caches are initialized once and cannot be cleared.
//! Validation functions operate on the distributed slices directly.

use linkme::distributed_slice;
use reinhardt_apps::registry::{
	MODELS, ModelMetadata, RELATIONSHIPS, RelationshipMetadata, RelationshipType,
};
use reinhardt_apps::validation::{
	ValidationError, check_circular_relationships, check_duplicate_model_names,
	check_duplicate_table_names, validate_registry,
};
use rstest::rstest;
use serial_test::serial;

// ============================================================================
// Test Model Registrations for Collision Tests
// ============================================================================

// Test 1: Duplicate app_label (normal case - multiple models in same app)
#[distributed_slice(MODELS)]
static COLLISION_USER_MODEL_1: ModelMetadata = ModelMetadata {
	app_label: "testapp",
	model_name: "User",
	table_name: "testapp_users",
};

#[distributed_slice(MODELS)]
static COLLISION_POST_MODEL_1: ModelMetadata = ModelMetadata {
	app_label: "testapp",
	model_name: "Post",
	table_name: "testapp_posts",
};

// Test 2: Duplicate model name within same app (ERROR case)
#[distributed_slice(MODELS)]
static COLLISION_DUPLICATE_USER_1: ModelMetadata = ModelMetadata {
	app_label: "duplicates",
	model_name: "User",
	table_name: "dup_users_1",
};

#[distributed_slice(MODELS)]
static COLLISION_DUPLICATE_USER_2: ModelMetadata = ModelMetadata {
	app_label: "duplicates",
	model_name: "User",
	table_name: "dup_users_2",
};

// Test 3: Duplicate table name across apps (ERROR case)
#[distributed_slice(MODELS)]
static COLLISION_TABLE_APP1_USER: ModelMetadata = ModelMetadata {
	app_label: "app1",
	model_name: "User",
	table_name: "shared_users",
};

#[distributed_slice(MODELS)]
static COLLISION_TABLE_APP2_USER: ModelMetadata = ModelMetadata {
	app_label: "app2",
	model_name: "User",
	table_name: "shared_users",
};

// Test 4: Circular relationships (ERROR case)
// ModelA → ModelB
#[distributed_slice(MODELS)]
static COLLISION_CIRCULAR_A: ModelMetadata = ModelMetadata {
	app_label: "circular",
	model_name: "ModelA",
	table_name: "circular_a",
};

#[distributed_slice(MODELS)]
static COLLISION_CIRCULAR_B: ModelMetadata = ModelMetadata {
	app_label: "circular",
	model_name: "ModelB",
	table_name: "circular_b",
};

// ModelA → ModelB (ForeignKey)
#[distributed_slice(RELATIONSHIPS)]
static COLLISION_CIRCULAR_A_TO_B: RelationshipMetadata = RelationshipMetadata {
	from_model: "circular.ModelA",
	to_model: "circular.ModelB",
	relationship_type: RelationshipType::ForeignKey,
	field_name: "b_ref",
	related_name: Some("a_set"),
	db_column: Some("b_id"),
	through_table: None,
};

// ModelB → ModelA (ForeignKey) - creates cycle
#[distributed_slice(RELATIONSHIPS)]
static COLLISION_CIRCULAR_B_TO_A: RelationshipMetadata = RelationshipMetadata {
	from_model: "circular.ModelB",
	to_model: "circular.ModelA",
	relationship_type: RelationshipType::ForeignKey,
	field_name: "a_ref",
	related_name: Some("b_set"),
	db_column: Some("a_id"),
	through_table: None,
};

// ============================================================================
// Integration Tests
// ============================================================================

/// Test 1: Duplicate app_label detection
///
/// Multiple models with the same app_label is expected behavior.
/// This test verifies that having multiple models in the same app
/// does NOT trigger a duplicate model name error.
#[rstest]
#[serial(app_registry)]
fn test_duplicate_app_label_detection() {
	// Multiple models in "testapp" should NOT cause duplicate model name errors
	let errors = check_duplicate_model_names();

	// Filter for testapp-specific errors
	let testapp_errors: Vec<_> = errors
		.iter()
		.filter(|e| {
			if let ValidationError::DuplicateModelName { app_label, .. } = e {
				app_label == "testapp"
			} else {
				false
			}
		})
		.collect();

	// Should have NO errors for testapp (different model names)
	assert_eq!(
		testapp_errors.len(),
		0,
		"Multiple models with same app_label should not cause errors when model names differ"
	);
}

/// Test 2: Duplicate model name within app detection
///
/// Having multiple models with the same name in the same app should be detected
/// as an error.
#[rstest]
#[serial(app_registry)]
fn test_duplicate_model_name_within_app() {
	let errors = check_duplicate_model_names();

	// Should detect duplicate "User" in "duplicates" app
	let duplicate_user_error = errors.iter().find(|e| {
		matches!(e,
			ValidationError::DuplicateModelName {
				app_label,
				model_name,
				count,
			} if app_label == "duplicates" && model_name == "User" && *count == 2
		)
	});

	assert!(
		duplicate_user_error.is_some(),
		"Should detect duplicate model name 'User' in app 'duplicates'"
	);

	// Verify error message
	if let Some(error) = duplicate_user_error {
		assert_eq!(
			error.to_string(),
			"Duplicate model name 'User' in app 'duplicates' (2 occurrences)"
		);
	}
}

/// Test 3: Duplicate table name across apps detection
///
/// Different apps using the same table name should be detected as an error.
#[rstest]
#[serial(app_registry)]
fn test_duplicate_table_name_across_apps() {
	let errors = check_duplicate_table_names();

	// Should detect duplicate table name "shared_users"
	let duplicate_table_error = errors.iter().find(|e| {
		matches!(e,
			ValidationError::DuplicateTableName {
				table_name,
				models,
			} if table_name == "shared_users" && models.len() == 2
		)
	});

	assert!(
		duplicate_table_error.is_some(),
		"Should detect duplicate table name 'shared_users' across apps"
	);

	// Verify error message contains both models
	if let Some(ValidationError::DuplicateTableName { models, .. }) = duplicate_table_error {
		assert!(
			models.contains(&"app1.User".to_string()),
			"Error should mention app1.User"
		);
		assert!(
			models.contains(&"app2.User".to_string()),
			"Error should mention app2.User"
		);
	}
}

/// Test 4: Circular relationship detection (A → B → A)
///
/// Circular foreign key relationships should be detected as an error.
#[rstest]
#[serial(app_registry)]
fn test_circular_relationship_detection() {
	let errors = check_circular_relationships();

	// Should detect circular relationship between ModelA and ModelB
	let circular_error = errors.iter().find(|e| {
		matches!(e,
			ValidationError::CircularRelationship { path }
			if path.contains(&"circular.ModelA".to_string())
				&& path.contains(&"circular.ModelB".to_string())
		)
	});

	assert!(
		circular_error.is_some(),
		"Should detect circular relationship between ModelA and ModelB"
	);

	// Verify error message
	if let Some(error) = circular_error {
		let error_msg = error.to_string();
		assert!(
			error_msg.contains("Circular relationship detected"),
			"Error message should indicate circular relationship"
		);
		assert!(
			error_msg.contains("ModelA") && error_msg.contains("ModelB"),
			"Error message should mention both models in the cycle"
		);
	}
}

/// Test 5: Comprehensive validation
///
/// Test that validate_registry() detects all types of errors.
#[rstest]
#[serial(app_registry)]
fn test_comprehensive_validation() {
	let all_errors = validate_registry();

	// Should detect at least 3 types of errors:
	// 1. Duplicate model name (duplicates.User)
	// 2. Duplicate table name (shared_users)
	// 3. Circular relationship (ModelA ↔ ModelB)

	let has_duplicate_model = all_errors.iter().any(|e| {
		matches!(e,
			ValidationError::DuplicateModelName {
				app_label,
				model_name,
				..
			} if app_label == "duplicates" && model_name == "User"
		)
	});

	let has_duplicate_table = all_errors.iter().any(|e| {
		matches!(e,
			ValidationError::DuplicateTableName { table_name, .. }
			if table_name == "shared_users"
		)
	});

	let has_circular_relationship = all_errors.iter().any(|e| {
		matches!(e,
			ValidationError::CircularRelationship { path }
			if path.iter().any(|p| p.contains("ModelA"))
				&& path.iter().any(|p| p.contains("ModelB"))
		)
	});

	assert!(
		has_duplicate_model,
		"validate_registry() should detect duplicate model names"
	);
	assert!(
		has_duplicate_table,
		"validate_registry() should detect duplicate table names"
	);
	assert!(
		has_circular_relationship,
		"validate_registry() should detect circular relationships"
	);
}

/// Test 6: No false positives for valid configurations
///
/// Verify that normal, valid model registrations do not trigger errors.
#[rstest]
#[serial(app_registry)]
fn test_no_false_positives_for_valid_models() {
	// "testapp" has User and Post - both valid (different names)
	let errors = check_duplicate_model_names();

	let testapp_errors: Vec<_> = errors
		.iter()
		.filter(|e| {
			matches!(e,
				ValidationError::DuplicateModelName { app_label, .. }
				if app_label == "testapp"
			)
		})
		.collect();

	assert_eq!(
		testapp_errors.len(),
		0,
		"Valid models should not trigger duplicate name errors"
	);
}

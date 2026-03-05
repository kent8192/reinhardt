//! Integration tests for reinhardt-apps registry performance
//!
//! These tests verify performance characteristics of the registry system.
//!
//! Note: With OnceLock-based caching, caches are initialized once and cannot be cleared.
//! Tests verify read operations and performance characteristics.

use linkme::distributed_slice;
use reinhardt_apps::registry::{
	MODELS, ModelMetadata, RELATIONSHIPS, RelationshipMetadata, RelationshipType,
	get_models_for_app, get_registered_models, get_registered_relationships,
};
use rstest::rstest;
use serial_test::serial;
use std::time::{Duration, Instant};

// ============================================================================
// Test Model Registrations for Performance Tests
// ============================================================================

// Register 100+ models for large-scale performance testing
macro_rules! register_perf_models {
	($($n:expr),*) => {
		$(
			paste::paste! {
				#[distributed_slice(MODELS)]
				static [<PERF_MODEL_ $n>]: ModelMetadata = ModelMetadata {
					app_label: "perf_test",
					model_name: concat!("Model", stringify!($n)),
					table_name: concat!("perf_test_model_", stringify!($n)),
				};
			}
		)*
	};
}

// Register 110 models for performance testing
register_perf_models!(
	0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
	26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49,
	50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73,
	74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97,
	98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109
);

// Test models for registry verification
#[distributed_slice(MODELS)]
static TEST_CLEANUP_APP1_MODEL1: ModelMetadata = ModelMetadata {
	app_label: "test_cleanup_app1",
	model_name: "CleanupModel1",
	table_name: "test_cleanup_app1_model1",
};

#[distributed_slice(MODELS)]
static TEST_CLEANUP_APP1_MODEL2: ModelMetadata = ModelMetadata {
	app_label: "test_cleanup_app1",
	model_name: "CleanupModel2",
	table_name: "test_cleanup_app1_model2",
};

#[distributed_slice(RELATIONSHIPS)]
static TEST_CLEANUP_APP1_REL: RelationshipMetadata = RelationshipMetadata {
	from_model: "test_cleanup_app1.CleanupModel1",
	to_model: "test_cleanup_app1.CleanupModel2",
	relationship_type: RelationshipType::ForeignKey,
	field_name: "related_model",
	related_name: Some("cleanup_model1_set"),
	db_column: Some("related_model_id"),
	through_table: None,
};

// Test model for metadata completeness
#[distributed_slice(MODELS)]
static TEST_METADATA_MODEL: ModelMetadata = ModelMetadata {
	app_label: "test_metadata",
	model_name: "MetadataTest",
	table_name: "test_metadata_metadatatest",
};

// ============================================================================
// Integration Tests
// ============================================================================

/// Test 1: Large-scale model registration performance
///
/// Verifies that registering and retrieving 100+ models completes
/// within reasonable time constraints.
#[rstest]
#[serial(app_registry)]
fn test_large_scale_model_registration_performance() {
	// Measure time to retrieve all registered models
	let start = Instant::now();
	let all_models = get_registered_models();
	let duration = start.elapsed();

	// Filter to count only our performance test models
	let perf_models: Vec<_> = all_models
		.iter()
		.filter(|m| m.app_label == "perf_test")
		.collect();

	// Verify that at least 100 models are registered
	assert!(
		perf_models.len() >= 100,
		"Expected at least 100 perf_test models, found {}",
		perf_models.len()
	);

	// Should complete within 100ms even with many models
	assert!(
		duration < Duration::from_millis(100),
		"Large scale model retrieval took {:?}, expected < 100ms",
		duration
	);

	// Verify efficient subsequent access (OnceLock cached)
	let start_cached = Instant::now();
	let models_cached = get_registered_models();
	let duration_cached = start_cached.elapsed();

	assert_eq!(
		models_cached.len(),
		all_models.len(),
		"Subsequent retrieval should return same number of models"
	);

	// Cached access should be significantly faster (lock-free with OnceLock)
	assert!(
		duration_cached < Duration::from_millis(10),
		"Cached retrieval took {:?}, expected < 10ms",
		duration_cached
	);

	println!(
		"Performance test: {} total models ({} perf_test models)",
		all_models.len(),
		perf_models.len()
	);
	println!(
		"Initial retrieval: {:?}, Cached retrieval: {:?}",
		duration, duration_cached
	);
}

/// Test 2: Verify registry data integrity
///
/// Verifies that registered models and relationships are present
/// and correctly indexed.
#[rstest]
#[serial(app_registry)]
fn test_registry_data_integrity() {
	// Step 1: Verify models are registered
	let models = get_models_for_app("test_cleanup_app1");
	assert_eq!(
		models.len(),
		2,
		"Expected exactly 2 models for test_cleanup_app1"
	);

	let model_names: Vec<&str> = models.iter().map(|m| m.model_name).collect();
	assert!(
		model_names.contains(&"CleanupModel1"),
		"Expected CleanupModel1 in registered models"
	);
	assert!(
		model_names.contains(&"CleanupModel2"),
		"Expected CleanupModel2 in registered models"
	);

	// Step 2: Verify relationships exist
	let relationships = get_registered_relationships();
	let test_app1_rels: Vec<_> = relationships
		.iter()
		.filter(|r| {
			r.from_model.starts_with("test_cleanup_app1.")
				|| r.to_model.starts_with("test_cleanup_app1.")
		})
		.collect();

	assert_eq!(
		test_app1_rels.len(),
		1,
		"Expected exactly 1 relationship for test_cleanup_app1"
	);

	let rel = test_app1_rels[0];
	assert_eq!(
		rel.from_model, "test_cleanup_app1.CleanupModel1",
		"Relationship from_model mismatch"
	);
	assert_eq!(
		rel.to_model, "test_cleanup_app1.CleanupModel2",
		"Relationship to_model mismatch"
	);
	assert_eq!(
		rel.field_name, "related_model",
		"Relationship field_name mismatch"
	);

	println!(
		"Registry data integrity verified: {} models, {} relationships",
		models.len(),
		test_app1_rels.len()
	);
}

/// Test 3: Model metadata completeness
///
/// Verifies that all required fields (app_label, model_name, table_name)
/// are present and non-empty in ModelMetadata.
#[rstest]
#[serial(app_registry)]
fn test_model_metadata_completeness() {
	let models = get_registered_models();

	// Find our test model
	let test_model = models
		.iter()
		.find(|m| m.app_label == "test_metadata" && m.model_name == "MetadataTest")
		.expect("Test model 'test_metadata.MetadataTest' not found in registry");

	// Verify all required fields are non-empty
	assert!(
		!test_model.app_label.is_empty(),
		"ModelMetadata.app_label must not be empty"
	);
	assert!(
		!test_model.model_name.is_empty(),
		"ModelMetadata.model_name must not be empty"
	);
	assert!(
		!test_model.table_name.is_empty(),
		"ModelMetadata.table_name must not be empty"
	);

	// Verify exact values match expected
	assert_eq!(
		test_model.app_label, "test_metadata",
		"app_label should be 'test_metadata'"
	);
	assert_eq!(
		test_model.model_name, "MetadataTest",
		"model_name should be 'MetadataTest'"
	);
	assert_eq!(
		test_model.table_name, "test_metadata_metadatatest",
		"table_name should follow convention: {{app_label}}_{{model_name_lowercase}}"
	);

	// Verify all models in registry have complete metadata
	for model in models.iter() {
		assert!(
			!model.app_label.is_empty(),
			"All models must have non-empty app_label. Found empty in: {:?}",
			model
		);
		assert!(
			!model.model_name.is_empty(),
			"All models must have non-empty model_name. Found empty in: {:?}",
			model
		);
		assert!(
			!model.table_name.is_empty(),
			"All models must have non-empty table_name. Found empty in: {:?}",
			model
		);
	}

	println!(
		"Model metadata verified: {}.{} -> {}",
		test_model.app_label, test_model.model_name, test_model.table_name
	);
	println!(
		"All {} registered models have complete metadata",
		models.len()
	);
}

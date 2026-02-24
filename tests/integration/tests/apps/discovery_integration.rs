//! Integration tests for reinhardt-apps discovery functionality
//!
//! These tests use distributed_slice for model registration and must be run as integration tests
//! (separate binaries) to avoid linkme conflicts.

use linkme::distributed_slice;
use reinhardt_apps::discovery::{discover_all_models, discover_models};
use reinhardt_apps::registry::{MODELS, ModelMetadata};
use serial_test::serial;
use std::collections::HashSet;

// Test model registrations for discovery tests
#[distributed_slice(MODELS)]
static DISCOVERY_TEST_USER: ModelMetadata = ModelMetadata {
	app_label: "discovery_test",
	model_name: "User",
	table_name: "discovery_test_users",
};

#[distributed_slice(MODELS)]
static DISCOVERY_TEST_POST: ModelMetadata = ModelMetadata {
	app_label: "discovery_test",
	model_name: "Post",
	table_name: "discovery_test_posts",
};

#[test]
#[serial(app_registry)]
fn test_discover_models() {
	let models = discover_models("discovery_test");
	assert_eq!(models.len(), 2);

	let model_names: HashSet<&str> = models.iter().map(|m| m.model_name).collect();
	assert_eq!(model_names, HashSet::from(["User", "Post"]));
}

#[test]
#[serial(app_registry)]
fn test_discover_all_models() {
	let models = discover_all_models();
	// Should have at least our test models
	assert!(models.len() >= 2);

	assert!(
		models
			.iter()
			.any(|m| m.app_label == "discovery_test" && m.model_name == "User")
	);
}

#[test]
#[serial(app_registry)]
fn test_discover_models_empty() {
	let models = discover_models("nonexistent_app");
	assert_eq!(models.len(), 0);
}

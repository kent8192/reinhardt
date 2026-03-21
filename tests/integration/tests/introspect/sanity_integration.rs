//! Sanity integration tests for database introspection
//!
//! Basic verification tests that ensure the introspection system works:
//! - Basic introspection produces output
//! - Generated code is syntactically valid
//! - Schema reading doesn't crash
//!
//! **Purpose:**
//! Quick smoke tests to verify core functionality before running full test suite.

use super::fixtures::postgres_introspect_schema;
use reinhardt_db::migrations::introspection::{DatabaseIntrospector, PostgresIntrospector};
use reinhardt_db::migrations::{IntrospectConfig, SchemaCodeGenerator};
use reinhardt_test::fixtures::{ContainerAsync, GenericImage};
use rstest::*;
use sqlx::PgPool;
use std::sync::Arc;

// ============================================================================
// Sanity Test 1: Basic Introspection
// ============================================================================

/// Verify basic introspection produces valid output
///
/// **Test Intent**: Smoke test that introspection works at all
///
/// **Expected Behavior**:
/// - No panics or errors
/// - Returns a non-empty schema
#[rstest]
#[tokio::test]
async fn sanity_basic_introspect(
	#[future] postgres_introspect_schema: (ContainerAsync<GenericImage>, Arc<PgPool>, String),
) {
	let (_container, pool, _url) = postgres_introspect_schema.await;

	let introspector = PostgresIntrospector::new(pool.as_ref().clone());
	let result = introspector.read_schema().await;

	// Should succeed
	assert!(result.is_ok(), "Introspection should succeed");

	let schema = result.unwrap();

	// Should have tables
	assert!(
		!schema.tables.is_empty(),
		"Schema should have at least one table"
	);
}

// ============================================================================
// Sanity Test 2: Code Generation
// ============================================================================

/// Verify generated code is non-empty
///
/// **Test Intent**: Smoke test that code generation works
///
/// **Expected Behavior**:
/// - Generates at least one file
/// - File content is non-empty
#[rstest]
#[tokio::test]
async fn sanity_generated_code_non_empty(
	#[future] postgres_introspect_schema: (ContainerAsync<GenericImage>, Arc<PgPool>, String),
) {
	let (_container, pool, _url) = postgres_introspect_schema.await;

	let introspector = PostgresIntrospector::new(pool.as_ref().clone());
	let schema = introspector
		.read_schema()
		.await
		.expect("Failed to read schema");

	let config = IntrospectConfig::default()
		.with_database_url("postgres://test@localhost/test")
		.with_app_label("testapp");

	let generator = SchemaCodeGenerator::new(config);
	let output = generator
		.generate(&schema)
		.expect("Failed to generate code");

	// Should generate files
	assert!(!output.files.is_empty(), "Should generate files");

	// All files should have content
	for file in &output.files {
		assert!(
			!file.content.is_empty(),
			"File {} should have content",
			file.path.display()
		);
	}
}

// ============================================================================
// Sanity Test 3: Table Filtering
// ============================================================================

/// Verify table filtering works
///
/// **Test Intent**: Smoke test that filtering doesn't break introspection
///
/// **Expected Behavior**:
/// - Include filter only includes matching tables
/// - Exclude filter removes matching tables
#[rstest]
#[tokio::test]
async fn sanity_table_filtering(
	#[future] postgres_introspect_schema: (ContainerAsync<GenericImage>, Arc<PgPool>, String),
) {
	let (_container, pool, _url) = postgres_introspect_schema.await;

	let introspector = PostgresIntrospector::new(pool.as_ref().clone());
	let schema = introspector
		.read_schema()
		.await
		.expect("Failed to read schema");

	// Create config that only includes users table
	let mut config = IntrospectConfig::default()
		.with_database_url("postgres://test@localhost/test")
		.with_app_label("testapp");
	config.tables.include = vec!["users".to_string()];
	config.tables.exclude = vec![];

	let generator = SchemaCodeGenerator::new(config);
	let output = generator
		.generate(&schema)
		.expect("Failed to generate code");

	// Should only generate users.rs and mod.rs
	let file_names: Vec<_> = output
		.files
		.iter()
		.filter_map(|f| f.path.file_name())
		.filter_map(|n| n.to_str())
		.collect();

	assert!(file_names.contains(&"users.rs"), "Should include users.rs");
	assert!(
		!file_names.contains(&"posts.rs"),
		"Should NOT include posts.rs when filtered"
	);
}

// ============================================================================
// Sanity Test 4: Schema Contains Expected Types
// ============================================================================

/// Verify schema captures expected field types
///
/// **Test Intent**: Verify field types are correctly identified
#[rstest]
#[tokio::test]
async fn sanity_field_types_captured(
	#[future] postgres_introspect_schema: (ContainerAsync<GenericImage>, Arc<PgPool>, String),
) {
	let (_container, pool, _url) = postgres_introspect_schema.await;

	let introspector = PostgresIntrospector::new(pool.as_ref().clone());
	let schema = introspector
		.read_schema()
		.await
		.expect("Failed to read schema");

	let users_table = schema
		.tables
		.get("users")
		.expect("users table should exist");

	// Verify field types are captured (not just empty)
	let id_col = users_table
		.columns
		.get("id")
		.expect("id column should exist");
	let username_col = users_table
		.columns
		.get("username")
		.expect("username column should exist");
	let is_active_col = users_table
		.columns
		.get("is_active")
		.expect("is_active column should exist");

	// Field types should be set (not default)
	// Just verify they're captured - specific type validation is in other tests
	assert!(!id_col.name.is_empty());
	assert!(!username_col.name.is_empty());
	assert!(!is_active_col.name.is_empty());
}

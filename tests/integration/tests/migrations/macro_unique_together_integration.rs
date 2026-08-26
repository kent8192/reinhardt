//! Integration tests for `#[model(unique_together = ...)]` macro propagation.
//!
//! Verifies that the `#[model(...)]` derive macro correctly registers
//! `unique_together` declarations into `ModelMetadata.constraints`, which is
//! the source of truth consumed by `MigrationAutodetector` via
//! `to_model_state()`.
//!
//! Regression test for kent8192/reinhardt-web#4022: previously, the macro
//! parsed `unique_together` and emitted ORM-side metadata, but never pushed
//! the corresponding `ConstraintDefinition` into the migration registry.
//! That left `ModelState.constraints` empty for composite UNIQUE constraints,
//! so `cargo make makemigrations` did not emit any `AddConstraint` operation
//! even after PR #3998 taught the autodetector to consume the new entries.
//!
//! These tests assert two layers:
//!
//! 1. The constructor-time registration in `global_registry()` carries the
//!    parsed `unique_together` constraints on `ModelMetadata`.
//! 2. The `to_model_state()` conversion preserves the constraints on its way
//!    to the autodetector.

use reinhardt_db::migrations::model_registry::global_registry;
use reinhardt_db::migrations::{Constraint, MigrationAutodetector, Operation, ProjectState};
use reinhardt_macros::model;
use rstest::*;
use serde::{Deserialize, Serialize};
use serial_test::serial;

// ---------------------------------------------------------------------------
// Test fixtures: minimal models that exercise the `unique_together` parser.
// ---------------------------------------------------------------------------

#[allow(dead_code)]
// The fixture is registered by the macro; its fields are read through metadata.
#[model(
	app_label = "macro_unique_together_test",
	table_name = "macro_unique_together_test_membership",
	unique_together = ("organization_id", "user_id")
)]
#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct Membership {
	#[field(primary_key = true)]
	pub id: i64,
	pub organization_id: i64,
	pub user_id: i64,
}

#[allow(dead_code)]
// The fixture is registered by the macro; its fields are read through metadata.
#[model(
	app_label = "macro_unique_together_test",
	table_name = "macro_unique_together_test_no_constraint"
)]
#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct PlainModel {
	#[field(primary_key = true)]
	pub id: i64,
	#[field(max_length = 255)]
	pub name: String,
}

#[allow(dead_code)]
// The fixture is registered by the macro; its fields are read through metadata.
#[model(
	app_label = "macro_unique_together_test",
	table_name = "macro_unique_together_test_indexed"
)]
#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct IndexedModel {
	#[field(primary_key = true)]
	pub id: i64,
	#[field(max_length = 255, index = true)]
	pub email: String,
}

// The derive macro registers this fixture in the global model registry.
#[allow(dead_code)]
#[model(
	app_label = "macro_field_check_test",
	table_name = "macro_field_check_test_account"
)]
#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct Account {
	#[field(primary_key = true)]
	pub id: i64,
	#[field(max_length = 20, check = "role IN ('admin', 'member')")]
	pub role: String,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[rstest]
#[serial(global_registry)]
fn unique_together_propagates_into_model_metadata() {
	// Arrange
	let registry = global_registry();
	let metadata = registry
		.get_model("macro_unique_together_test", "Membership")
		.expect("Membership model should be registered by the #[model] macro");

	// Act
	let constraints = metadata.constraints();

	// Assert
	assert_eq!(
		constraints.len(),
		1,
		"exactly one model-level constraint should be emitted from the single \
		 `unique_together` declaration, got {constraints:?}"
	);
	let c = &constraints[0];
	assert_eq!(c.constraint_type, "unique");
	assert_eq!(
		c.fields,
		vec!["organization_id".to_string(), "user_id".to_string()],
		"field order must match the declaration so that auto-generated names \
		 stay deterministic"
	);
	assert_eq!(
		c.name, "macro_unique_together_test_membership_organization_id_user_id_uniq",
		"constraint name must follow the `{{table}}_{{f1}}_{{f2}}_uniq` rule \
		 already used by the ORM-side ConstraintInfo so that downstream tools \
		 see a single name"
	);
	assert!(c.expression.is_none());
	assert!(c.foreign_key_info.is_none());
}

#[rstest]
#[serial(global_registry)]
fn to_model_state_carries_unique_together_constraints() {
	// Arrange
	let registry = global_registry();
	let metadata = registry
		.get_model("macro_unique_together_test", "Membership")
		.expect("Membership model should be registered by the #[model] macro");

	// Act
	let model_state = metadata.to_model_state();

	// Assert
	let unique_constraints: Vec<_> = model_state
		.constraints
		.iter()
		.filter(|c| c.fields == vec!["organization_id".to_string(), "user_id".to_string()])
		.collect();
	assert_eq!(
		unique_constraints.len(),
		1,
		"exactly one composite UNIQUE constraint should reach ModelState; got \
		 all constraints = {:?}",
		model_state.constraints
	);
	assert_eq!(unique_constraints[0].constraint_type, "unique");
}

#[rstest]
#[serial(global_registry)]
fn models_without_unique_together_emit_no_extra_constraints() {
	// Arrange
	let registry = global_registry();
	let metadata = registry
		.get_model("macro_unique_together_test", "PlainModel")
		.expect("PlainModel should be registered by the #[model] macro");

	// Act / Assert
	assert!(
		metadata.constraints().is_empty(),
		"ModelMetadata.constraints() must stay empty when no unique_together \
		 attribute is declared, got {:?}",
		metadata.constraints()
	);
}

#[rstest]
#[serial(global_registry)]
fn field_index_propagates_into_migration_metadata() {
	// Arrange
	let registry = global_registry();
	let metadata = registry
		.get_model("macro_unique_together_test", "IndexedModel")
		.expect("IndexedModel should be registered by the #[model] macro");

	// Act
	let model_state = metadata.to_model_state();

	// Assert
	assert_eq!(metadata.indexes().len(), 1);
	assert_eq!(model_state.indexes.len(), 1);
	assert_eq!(model_state.indexes[0].fields, vec!["email"]);
	assert!(!model_state.indexes[0].unique);
}

#[rstest]
fn field_check_reaches_initial_migration_and_stabilizes() {
	// Arrange
	let registry = global_registry();
	let metadata = registry
		.get_model("macro_field_check_test", "Account")
		.expect("Account model should be registered by the #[model] macro");

	// Act
	let model_state = metadata.to_model_state();
	let mut target_state = ProjectState::new();
	target_state.add_model(model_state);
	let operations =
		MigrationAutodetector::new(ProjectState::new(), target_state.clone()).generate_operations();

	// Assert: the initial CreateTable operation contains the declared CHECK.
	let constraints = operations
		.iter()
		.find_map(|operation| match operation {
			Operation::CreateTable {
				name, constraints, ..
			} if name == "macro_field_check_test_account" => Some(constraints),
			_ => None,
		})
		.expect("initial migration should create the Account table");
	assert_eq!(
		constraints,
		&vec![Constraint::Check {
			name: "role_check".to_string(),
			expression: "role IN ('admin', 'member')".to_string(),
		}],
		"field-level CHECK metadata must be included in CreateTable"
	);

	// Replay the generated migration and ensure a second autodetection is a no-op.
	let mut replayed_state = ProjectState::new();
	replayed_state.apply_migration_operations(&operations, "macro_field_check_test");
	let second_operations =
		MigrationAutodetector::new(replayed_state, target_state).generate_operations();
	assert!(
		second_operations.is_empty(),
		"re-running makemigrations after the generated CHECK migration should be a no-op: \
			{second_operations:?}"
	);
}

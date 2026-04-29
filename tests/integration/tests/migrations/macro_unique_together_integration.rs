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
use reinhardt_macros::model;
use rstest::*;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Test fixtures: minimal models that exercise the `unique_together` parser.
// ---------------------------------------------------------------------------

#[allow(dead_code)]
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[rstest]
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

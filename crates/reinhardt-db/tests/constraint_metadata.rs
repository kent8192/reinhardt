// The model macro emits feature cfgs not declared by this standalone integration-test crate.
#![allow(unexpected_cfgs)]

use reinhardt_core::macros::model;
use reinhardt_db::{migrations::model_registry::global_registry, orm::Model};
use serde::{Deserialize, Serialize};

#[model(
	app_label = "constraint_metadata_runtime",
	table_name = "constraint_metadata_records",
	constraints = [unique(
		fields = ["tenant_id", "slug"],
		name = "constraint_metadata_active_tenant_slug_unique",
		condition = "archived_at IS NULL"
	)]
)]
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ConstraintMetadataRecord {
	#[field(primary_key = true)]
	id: i64,
	#[field(db_column = "tenant_key")]
	tenant_id: i64,
	#[field(db_column = "slug_key", max_length = 255)]
	slug: String,
}

#[test]
fn derived_unique_constraint_keeps_logical_and_physical_metadata_distinct() {
	let constraint = ConstraintMetadataRecord::constraint_metadata()
		.into_iter()
		.find(|constraint| constraint.name == "constraint_metadata_active_tenant_slug_unique")
		.expect("derived unique constraint metadata");

	assert_eq!(constraint.fields, ["tenant_id", "slug"]);
	assert_eq!(
		constraint.definition,
		"UNIQUE (tenant_key, slug_key) WHERE archived_at IS NULL"
	);
	assert_eq!(constraint.condition.as_deref(), Some("archived_at IS NULL"));
	assert!(!constraint.deferrable);
	assert_eq!(constraint.nulls_distinct, None);

	let registered_model = global_registry()
		.get_model("constraint_metadata_runtime", "ConstraintMetadataRecord")
		.expect("derived model migration registration");
	let registered_constraint = registered_model
		.constraints()
		.iter()
		.find(|constraint| constraint.name == "constraint_metadata_active_tenant_slug_unique")
		.expect("registered unique constraint");

	assert_eq!(registered_constraint.fields, ["tenant_key", "slug_key"]);
	assert_eq!(registered_constraint.expression, None);
}

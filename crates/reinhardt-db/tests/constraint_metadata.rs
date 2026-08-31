// The model macro emits feature cfgs not declared by this standalone integration-test crate.
#![allow(unexpected_cfgs)]

use reinhardt_core::macros::model;
use reinhardt_db::{
	associations::ForeignKeyField, migrations::model_registry::global_registry, orm::Model,
};
use serde::{Deserialize, Serialize};

#[model(
	app_label = "constraint_metadata_runtime",
	table_name = "constraint_metadata_owners"
)]
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ConstraintMetadataOwner {
	#[field(primary_key = true)]
	id: i64,
}

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
	#[field(db_column = "email_addr", unique = true, max_length = 255)]
	email: String,
	#[field(check = "age >= 18")]
	age: i32,
	#[field(db_column = "tenant_key")]
	tenant_id: i64,
	#[field(db_column = "slug_key", max_length = 255)]
	slug: String,
	#[rel(foreign_key, db_column = "owner_key")]
	owner: ForeignKeyField<ConstraintMetadataOwner>,
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

#[test]
fn derived_constraint_fields_match_registered_physical_constraints() {
	let registered_model = global_registry()
		.get_model("constraint_metadata_runtime", "ConstraintMetadataRecord")
		.expect("derived model migration registration");

	for constraint in registered_model.constraints().iter().filter(|constraint| {
		constraint.constraint_type == "unique" || constraint.constraint_type == "foreign_key"
	}) {
		let expected = match constraint.fields.as_slice() {
			["tenant_key", "slug_key"] => vec!["tenant_id", "slug"],
			["email_addr"] => vec!["email"],
			["owner_key"] => vec!["owner_id"],
			fields => panic!("unexpected registered constraint fields: {fields:?}"),
		};
		assert_eq!(
			ConstraintMetadataRecord::constraint_fields(&constraint.name),
			Some(expected),
			"constraint {} must resolve through the derived model",
			constraint.name
		);
	}

	assert_eq!(
		ConstraintMetadataRecord::constraint_fields(
			"constraint_metadata_active_tenant_slug_unique"
		),
		Some(vec!["tenant_id", "slug"]),
	);
	assert_eq!(
		ConstraintMetadataRecord::constraint_fields("age_check"),
		Some(Vec::new()),
	);
	assert_eq!(ConstraintMetadataRecord::constraint_fields("unknown"), None);

	let owner_id = ConstraintMetadataRecord::field_metadata()
		.into_iter()
		.find(|field| field.db_column.as_deref() == Some("owner_key"))
		.expect("derived foreign key field metadata");
	assert_eq!(owner_id.name, "owner_id");
	assert_eq!(owner_id.db_column.as_deref(), Some("owner_key"));
}

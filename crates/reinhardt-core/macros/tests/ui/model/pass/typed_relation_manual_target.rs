use reinhardt_macros::model;
use serde::{Deserialize, Serialize};

include!("../support.rs");

#[derive(Debug, Clone, PartialEq)]
struct ManualTarget {
	id: i64,
}

impl crate::model_info::InfoModel for ManualTarget {
	type PrimaryKey = i64;
}

impl ManualTarget {
	const fn field_id() -> db::orm::expressions::FieldRef<Self, i64> {
		// SAFETY: `id` is the persisted primary-key column declared below.
		unsafe { db::orm::expressions::FieldRef::from_model_field("id") }
	}
}

struct ManualTargetFields;

impl db::orm::FieldSelector for ManualTargetFields {}

impl db::orm::Model for ManualTarget {
	type PrimaryKey = i64;
	type Fields = ManualTargetFields;
	type Objects = db::orm::Manager<Self>;

	fn table_name() -> &'static str {
		"manual_targets"
	}

	fn new_fields() -> Self::Fields {
		ManualTargetFields
	}

	fn app_label() -> &'static str {
		"manual"
	}

	fn primary_key_field() -> &'static str {
		"id"
	}

	fn primary_key_column() -> &'static str {
		"id"
	}

	fn primary_key(&self) -> Option<Self::PrimaryKey> {
		Some(self.id)
	}

	fn set_primary_key(&mut self, value: Self::PrimaryKey) {
		self.id = value;
	}

	fn field_is_none(&self, _field_name: &str) -> bool {
		false
	}

	fn field_metadata() -> Vec<db::orm::inspection::FieldInfo> {
		Vec::new()
	}

	fn index_metadata() -> Vec<db::orm::inspection::IndexInfo> {
		Vec::new()
	}

	fn constraint_metadata() -> Vec<db::orm::inspection::ConstraintInfo> {
		Vec::new()
	}

	fn relationship_metadata() -> Vec<db::orm::inspection::RelationInfo> {
		Vec::new()
	}

	fn generated_field_names() -> &'static [&'static str] {
		&[]
	}
}

#[model(app_label = "derived", table_name = "derived_sources")]
#[derive(Serialize, Deserialize)]
struct DerivedSource {
	#[field(primary_key = true)]
	id: i64,
	#[rel(foreign_key)]
	manual_target: db::associations::ForeignKeyField<ManualTarget>,
}

fn main() {
	use db::orm::relations::RelationPathLike;

	let relation = DerivedSource::rel_manual_target();
	assert_eq!(relation.steps()[0].target_table, "manual_targets");
	let _ = DerivedSource::field_id();
}

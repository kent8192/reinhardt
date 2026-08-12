// The pass fixture intentionally exercises generated cfg branches that are not
// declared by this standalone trybuild crate.
#![allow(unexpected_cfgs)]

use reinhardt::model;
use reinhardt_db::associations::ForeignKeyField;
use reinhardt_db::orm::expressions::{FieldRef, GeneratedModelField};
use reinhardt_macros::admin;
use serde::{Deserialize, Serialize};

#[model(app_label = "admin_relation_ui", table_name = "relation_targets")]
#[derive(Serialize, Deserialize)]
struct RelationTarget {
	#[field(primary_key = true)]
	id: i64,
}

#[model(app_label = "admin_relation_ui", table_name = "relation_sources")]
#[derive(Serialize, Deserialize)]
struct RelationSource {
	#[field(primary_key = true)]
	id: i64,
	#[rel(foreign_key, db_column = "target_key")]
	target: ForeignKeyField<RelationTarget>,
	#[rel(foreign_key, db_column = "reviewer_key")]
	reviewer: ForeignKeyField<RelationTarget>,
}

#[admin(model,
	for = RelationSource,
	name = "Relation source",
	autocomplete_fields = [target_key],
	raw_id_fields = [reviewer_key]
)]
struct RelationSourceAdmin;

fn main() {
	let _: FieldRef<RelationSource, i64, GeneratedModelField> = RelationSource::field_target_key();
}

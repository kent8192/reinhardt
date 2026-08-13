use reinhardt_macros::model;
use serde::{Deserialize, Serialize};

include!("../support.rs");

#[model(app_label = "projects", table_name = "projects")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Project {
	#[field(primary_key = true)]
	id: i64,
}

#[model(app_label = "documents", table_name = "documents")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Document {
	#[field(primary_key = true)]
	id: i64,
	#[rel(foreign_key)]
	project: db::associations::ForeignKeyField<Project>,
}

fn main() {
	use db::orm::relations::{GeneratedRelationPath, RelationPath};

	let _ = RelationPath::<Document, Project, GeneratedRelationPath>::new(&[]);
}

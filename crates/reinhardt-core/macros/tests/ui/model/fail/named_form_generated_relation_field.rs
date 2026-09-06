use reinhardt_macros::model;

include!("../support.rs");

struct Organization;

#[model(
	app_label = "clusters",
	form(name = ClusterCreateForm, fields(organization_id))
)]
struct Cluster {
	#[field(primary_key = true)]
	id: i64,
	#[rel(foreign_key)]
	organization: db::associations::ForeignKeyField<Organization>,
}

fn main() {}

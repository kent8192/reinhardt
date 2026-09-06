use reinhardt_macros::model;

include!("../support.rs");

#[model(
	app_label = "clusters",
	form = true,
	form(name = ClusterCreateForm, fields(name))
)]
struct Cluster {
	#[field(primary_key = true)]
	id: i64,
	#[field]
	name: String,
}

fn main() {}

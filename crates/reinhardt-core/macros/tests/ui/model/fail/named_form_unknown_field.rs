use reinhardt_macros::model;

include!("../support.rs");

#[model(
	app_label = "clusters",
	form(name = ClusterCreateForm, fields(missing))
)]
struct Cluster {
	#[field(primary_key = true)]
	id: i64,
	name: String,
}

fn main() {}

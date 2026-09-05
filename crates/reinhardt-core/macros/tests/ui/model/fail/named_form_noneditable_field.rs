use reinhardt_macros::model;

include!("../support.rs");

#[model(
	app_label = "clusters",
	form(name = ClusterCreateForm, fields(secret))
)]
struct Cluster {
	#[field(primary_key = true)]
	id: i64,
	#[field(editable = false)]
	secret: String,
}

fn main() {}

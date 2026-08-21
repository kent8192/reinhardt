use reinhardt_macros::model;
use serde::Serialize;

include!("../support.rs");

#[model(table_name = "secrets", server_only)]
#[derive(Serialize)]
struct Secret {
	#[field(primary_key = true)]
	id: i64,
}

fn assert_info_model<T: model_info::InfoModel<PrimaryKey = i64>>() {}

fn main() {
	assert_info_model::<Secret>();
	let _ = SecretInfo { id: 1 };
}

use reinhardt_macros::model;
use serde::{Deserialize, Serialize};

include!("../support.rs");

// Database serialization still requires serde; derive it so this snapshot
// only asserts that `server_only` omits SecretInfo.
#[model(table_name = "secrets", server_only)]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Secret {
	#[field(primary_key = true)]
	id: i64,
}

fn assert_info_model<T: model_info::InfoModel<PrimaryKey = i64>>() {}

fn main() {
	assert_info_model::<Secret>();
	let _ = SecretInfo { id: 1 };
}

use reinhardt_macros::model;
use serde::{Deserialize, Serialize};

include!("../model/support.rs");
include!("structured_index_support.rs");

#[model(app_label = "search")]
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Document {
	#[field(primary_key = true)]
	id: i64,
	#[field(index(
		name = "",
		method = "hnsw",
		opclass = "vector_l2_ops"
	))]
	embedding: Vector<1536>,
}

fn main() {}

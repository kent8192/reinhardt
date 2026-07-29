use reinhardt_macros::model;
use serde::{Deserialize, Serialize};

include!("../model/support.rs");

#[model(app_label = "search")]
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Document {
	#[field(primary_key = true)]
	id: i64,
	#[field(
		max_length = 255,
		index(
			name = "documents_title_hnsw",
			method = "hnsw",
			opclass = "vector_l2_ops"
		)
	)]
	title: String,
}

fn main() {}

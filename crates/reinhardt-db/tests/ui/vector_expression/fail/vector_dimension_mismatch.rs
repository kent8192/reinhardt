#[path = "../support.rs"]
mod support;

use reinhardt_db::orm::Model;
use support::{Document, vector4};

fn main() {
	let _expression = Document::new_fields()
		.embedding
		.cosine_distance(vector4());
}

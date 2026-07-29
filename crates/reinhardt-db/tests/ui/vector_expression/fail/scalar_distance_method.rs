#[path = "../support.rs"]
mod support;

use reinhardt_db::orm::Model;
use support::{Document, vector3};

fn main() {
	let _expression = Document::new_fields().title.cosine_distance(vector3());
}

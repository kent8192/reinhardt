#[path = "../support.rs"]
mod support;

use reinhardt_db::orm::{Model, QuerySet};
use support::{Document, OtherDocument, vector3};

fn main() {
	let predicate = OtherDocument::new_fields()
		.embedding
		.cosine_distance(vector3())
		.lt(0.5);
	let _query = QuerySet::<Document>::new().filter(predicate);
}

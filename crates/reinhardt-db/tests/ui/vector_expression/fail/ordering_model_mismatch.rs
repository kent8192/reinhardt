#[path = "../support.rs"]
mod support;

use reinhardt_db::orm::{Model, QuerySet};
use support::{Document, OtherDocument, vector3};

fn main() {
	let ordering = OtherDocument::new_fields()
		.embedding
		.cosine_distance(vector3())
		.asc();
	let _query = QuerySet::<Document>::new().order_by(ordering);
}

#[path = "../support.rs"]
mod support;

use reinhardt_db::orm::QuerySet;
use support::ModelRecord;

fn main() {
	let _ = QuerySet::<ModelRecord>::new().aggregate(
		ModelRecord::field_i64()
			.into_expression()
			.label("value")
			.expect("valid label"),
	);
}

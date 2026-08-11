#[path = "../support.rs"]
mod support;

use reinhardt_db::orm::QuerySet;
use support::ModelRecord;

fn main() {
	let _ = QuerySet::<ModelRecord>::new().having(ModelRecord::field_i64().into_expression().gt(1_i64));
}

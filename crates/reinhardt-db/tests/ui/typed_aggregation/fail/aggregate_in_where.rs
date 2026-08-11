#[path = "../support.rs"]
mod support;

use reinhardt_db::orm::{QuerySet, func};
use support::ModelRecord;

fn main() {
	let _ = QuerySet::<ModelRecord>::new().filter(func::sum(ModelRecord::field_i64()).gt(1_i64));
}

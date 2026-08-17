#[path = "../support.rs"]
mod support;

use reinhardt_db::orm::{QuerySet, func};
use support::ModelRecord;

fn main() {
	let _ = QuerySet::<ModelRecord>::new().aggregate(func::count_all::<ModelRecord>());
}

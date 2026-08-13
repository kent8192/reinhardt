#[path = "../support.rs"]
mod support;

use reinhardt_db::orm::{FieldRef, func};
use support::ModelRecord;

fn main() {
	let _ = func::sum(FieldRef::<ModelRecord, i64>::new("value"));
}

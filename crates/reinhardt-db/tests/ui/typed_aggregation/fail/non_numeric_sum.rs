#[path = "../support.rs"]
mod support;

use reinhardt_db::orm::func;
use support::ModelRecord;

fn main() {
	let _ = func::sum(ModelRecord::field_name());
}

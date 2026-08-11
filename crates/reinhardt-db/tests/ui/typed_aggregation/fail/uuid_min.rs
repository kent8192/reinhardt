#[path = "../support.rs"]
mod support;

use reinhardt_db::orm::func;
use support::ModelRecord;

fn main() {
	let _ = func::min(ModelRecord::field_uuid());
}

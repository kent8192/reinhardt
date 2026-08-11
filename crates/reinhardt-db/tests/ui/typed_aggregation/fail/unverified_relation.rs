#[path = "../support.rs"]
mod support;

use reinhardt_db::orm::{RelationPath, func};
use support::{ModelRecord, RelatedRecord};

fn main() {
	let _ = func::count(RelationPath::<ModelRecord, RelatedRecord>::new(&[]));
}

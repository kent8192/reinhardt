#[path = "../support.rs"]
mod support;

use reinhardt_db::orm::QuerySet;
use support::Article;

fn main() {
	let _query = QuerySet::<Article>::new()
		.select_for_update()
		.nowait()
		.skip_locked();
}

#[path = "../support.rs"]
mod support;

use reinhardt_db::orm::QuerySet;
use support::{Article, comment_author};

fn main() {
	let _query = QuerySet::<Article>::new()
		.select_for_update()
		.of_relation(comment_author());
}

#[path = "ui/select_for_update/support.rs"]
mod support;

use reinhardt_db::orm::QuerySet;
use support::{Article, article_author};

#[test]
fn typed_row_lock_targets_compile() {
	let _root_target = QuerySet::<Article>::new().select_for_update().of_model();
	let _relation_target = QuerySet::<Article>::new()
		.select_for_update()
		.of_relation(article_author());
}

#[test]
fn invalid_select_for_update_types_do_not_compile() {
	let tests = trybuild::TestCases::new();
	tests.compile_fail("tests/ui/select_for_update/fail/*.rs");
}

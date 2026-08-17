//! Full-expansion compile-time tests for admin list relation selection.

#[test]
fn admin_list_select_related_ui() {
	let tests = trybuild::TestCases::new();
	tests.pass("tests/macros/ui/pass/admin_list_select_related_foreign_key.rs");
}

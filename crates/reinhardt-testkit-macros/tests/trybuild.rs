//! Run with `cargo test -p reinhardt-testkit-macros --test trybuild`.

use rstest::*;

#[rstest]
fn macro_ui_tests() {
	let t = trybuild::TestCases::new();
	t.pass("tests/ui/pass_singleton.rs");
	t.pass("tests/ui/pass_factory.rs");
	t.pass("tests/ui/pass_all_scopes_and_hygiene.rs");
	t.compile_fail("tests/ui/fail_unknown_kind.rs");
	t.compile_fail("tests/ui/fail_missing_kind.rs");
	t.compile_fail("tests/ui/fail_missing_type.rs");
	t.compile_fail("tests/ui/fail_transient_value.rs");
	t.compile_fail("tests/ui/fail_non_path_struct_value.rs");
	t.compile_fail("tests/ui/fail_missing_comma.rs");
	t.compile_fail("tests/ui/fail_invalid_factory_expression.rs");
	t.compile_fail("tests/ui/fail_invalid_struct_field.rs");
	t.compile_fail("tests/ui/fail_struct_update_without_base.rs");
	t.compile_fail("tests/ui/fail_non_async_call.rs");
}

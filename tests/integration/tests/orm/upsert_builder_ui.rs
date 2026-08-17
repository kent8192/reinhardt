//! Compile-time tests for typed get-or-create and update-or-create builders.

#[test]
fn typed_upsert_builders_compile_pass() {
	let tests = trybuild::TestCases::new();
	tests.pass("tests/orm/ui/pass/typed_upsert_builders.rs");
}

#[test]
fn typed_upsert_builders_compile_fail() {
	let tests = trybuild::TestCases::new();
	tests.compile_fail("tests/orm/ui/fail/upsert_wrong_model_field.rs");
	tests.compile_fail("tests/orm/ui/fail/upsert_wrong_value_type.rs");
	tests.compile_fail("tests/orm/ui/fail/update_or_create_plain_executor.rs");
}

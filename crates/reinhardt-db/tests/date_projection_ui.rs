#[test]
fn date_projection_field_types_are_checked_at_compile_time() {
	let tests = trybuild::TestCases::new();
	tests.pass("tests/ui/date_projection/pass.rs");
	tests.compile_fail("tests/ui/date_projection/wrong_field.rs");
}

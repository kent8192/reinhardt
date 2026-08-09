//! Compile-time coverage for the `#[admin]` fieldset grammar.

use rstest::rstest;

#[rstest]
fn model_admin_fieldsets_ui() {
	// Arrange
	let tests = trybuild::TestCases::new();

	// Act & Assert
	tests.pass("tests/admin/ui/pass/fieldsets.rs");
	tests.compile_fail("tests/admin/ui/fail/fieldsets_unknown_field.rs");
	tests.compile_fail("tests/admin/ui/fail/fields_and_fieldsets.rs");
	tests.compile_fail("tests/admin/ui/fail/fieldsets_empty_group.rs");
	tests.compile_fail("tests/admin/ui/fail/fieldsets_duplicate_field.rs");
}

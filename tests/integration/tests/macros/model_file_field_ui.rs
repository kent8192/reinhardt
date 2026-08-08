//! Compile-time tests for storage-backed model file fields.

#[test]
fn model_file_field_pass_cases_compile() {
	let tests = trybuild::TestCases::new();
	tests.pass("tests/macros/ui/pass/model_file_field.rs");
	tests.pass("tests/macros/ui/pass/model_optional_file_field.rs");
}

#[test]
fn model_file_field_validation_failures_are_diagnostic() {
	let tests = trybuild::TestCases::new();
	for fixture in [
		"model_file_field_wrong_type",
		"model_file_field_non_literal",
		"model_file_field_unknown_token",
		"model_file_field_unsafe_template",
		"model_file_field_invalid_alias",
		"model_file_field_short_max_length",
		"model_file_field_storage_name",
	] {
		tests.compile_fail(format!("tests/macros/ui/fail/{fixture}.rs"));
	}
}

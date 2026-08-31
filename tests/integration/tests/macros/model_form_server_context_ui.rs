//! Native compile-time tests for generated model-form server contexts.

use rstest::rstest;

#[rstest]
fn complete_model_form_server_context_compiles() {
	let tests = trybuild::TestCases::new();
	tests.pass("tests/macros/ui/pass/model_form_server_context_complete.rs");
}

#[rstest]
fn model_form_construction_boundaries_fail_to_compile() {
	let tests = trybuild::TestCases::new();
	for fixture in [
		"model_form_raw_payload_into_model",
		"model_form_server_context_incomplete",
		"model_form_server_context_new_collision",
		"model_form_server_context_state_collision",
	] {
		tests.compile_fail(format!("tests/macros/ui/fail/{fixture}.rs"));
	}
}
